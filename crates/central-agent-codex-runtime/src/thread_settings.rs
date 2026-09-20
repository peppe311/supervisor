//! Display-only allowlist for native thread/settings/updated. Never feeds
//! prompts, permission consent, composer selections or saved configuration.
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSettings {
    pub cwd: String,
    pub model: String,
    pub model_provider: String,
    pub service_tier: Option<String>,
    pub effort: Option<String>,
    pub summary: Option<String>,
    pub personality: Option<String>,
    pub approvals_reviewer: String,
    #[serde(skip_deserializing)]
    pub approval_policy: String,
    #[serde(skip_deserializing)]
    pub sandbox_kind: String,
}

impl ThreadSettings {
    pub fn read(value: &Value) -> Result<Self, String> {
        // Deserializing this allowlist drops collaboration instructions,
        // permission internals and unknown/private fields before IPC or Debug.
        let mut settings: Self = serde_json::from_value(value.clone())
            .map_err(|_| "Malformed native thread settings".to_owned())?;
        for text in [&settings.cwd, &settings.model, &settings.model_provider] {
            if text.is_empty() || text.contains('\0') {
                return Err("Incomplete native thread settings".into());
            }
        }
        settings.cwd = display_local_path(&settings.cwd);
        if !matches!(
            settings.approvals_reviewer.as_str(),
            "user" | "auto_review" | "guardian_subagent"
        ) {
            return Err("Unknown native approval reviewer".into());
        }
        settings.approval_policy = match &value["approvalPolicy"] {
            Value::String(policy)
                if matches!(policy.as_str(), "untrusted" | "on-request" | "never") =>
            {
                policy.clone()
            }
            Value::Object(policy)
                if policy.get("granular").is_some_and(|granular| {
                    [
                        "sandbox_approval",
                        "rules",
                        "skill_approval",
                        "request_permissions",
                        "mcp_elicitations",
                    ]
                    .iter()
                    .all(|key| granular.get(key).is_some_and(Value::is_boolean))
                }) =>
            {
                "granular (per-category policy)".into()
            }
            _ => return Err("Malformed native approval policy".into()),
        };
        settings.sandbox_kind = match value["sandboxPolicy"]["type"].as_str() {
            Some(
                kind @ ("readOnly" | "workspaceWrite" | "dangerFullAccess" | "externalSandbox"),
            ) => kind.into(),
            _ => return Err("Unknown native sandbox kind".into()),
        };
        Ok(settings)
    }

    /// Project the settings fields that are part of the stable
    /// thread/start, thread/resume and thread/fork response envelope. Fields
    /// that only exist in thread/settings/updated remain explicitly unknown.
    pub fn read_session_response(value: &Value) -> Result<Self, String> {
        Self::read(&serde_json::json!({
            "cwd": value.get("cwd").cloned().unwrap_or(Value::Null),
            "model": value.get("model").cloned().unwrap_or(Value::Null),
            "modelProvider": value.get("modelProvider").cloned().unwrap_or(Value::Null),
            "serviceTier": value.get("serviceTier").cloned().unwrap_or(Value::Null),
            "effort": value.get("reasoningEffort").cloned().unwrap_or(Value::Null),
            "summary": Value::Null,
            "personality": Value::Null,
            "approvalPolicy": value.get("approvalPolicy").cloned().unwrap_or(Value::Null),
            "approvalsReviewer": value.get("approvalsReviewer").cloned().unwrap_or(Value::Null),
            "sandboxPolicy": value.get("sandbox").cloned().unwrap_or(Value::Null),
        }))
    }
}

fn display_local_path(value: &str) -> String {
    #[cfg(windows)]
    {
        if let Some(rest) = value.strip_prefix("\\\\?\\UNC\\") {
            return format!("\\\\{rest}");
        }
        if let Some(rest) = value.strip_prefix("\\\\?\\") {
            return rest.to_owned();
        }
    }
    value.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_projection_excludes_instructions_and_private_fields() {
        let fixture: Value =
            serde_json::from_str(include_str!("../tests/thread-settings.json")).unwrap();
        let mut raw = fixture["threadSettings"].clone();
        raw["privateField"] = serde_json::json!("SYNTHETIC_PRIVATE unknown field");
        let settings = ThreadSettings::read(&raw).unwrap();
        assert_eq!(settings.model, "fixture-model");
        assert_eq!(settings.sandbox_kind, "workspaceWrite");
        assert_eq!(settings.effort.as_deref(), Some("high"));
        let output = serde_json::to_string(&settings).unwrap();
        for private in [
            "SYNTHETIC_PRIVATE",
            "developer_instructions",
            "collaborationMode",
            "privateField",
        ] {
            assert!(!output.contains(private));
            assert!(!format!("{settings:?}").contains(private));
        }
        for (field, invalid) in [
            ("cwd", Value::Null),
            ("model", Value::Bool(true)),
            ("effort", serde_json::json!([])),
            ("approvalPolicy", serde_json::json!({"granular":{}})),
            ("sandboxPolicy", serde_json::json!({"type":"unknown"})),
        ] {
            let mut value = fixture["threadSettings"].clone();
            value[field] = invalid;
            assert!(ThreadSettings::read(&value).is_err(), "{field}");
        }
    }

    #[test]
    fn stable_session_response_supplies_an_initial_settings_report() {
        let response = serde_json::json!({
            "cwd":"C:\\fixture",
            "model":"fixture-model",
            "modelProvider":"openai",
            "serviceTier":null,
            "reasoningEffort":"high",
            "approvalPolicy":"on-request",
            "approvalsReviewer":"user",
            "sandbox":{"type":"workspaceWrite","writableRoots":["C:\\fixture"],"networkAccess":false}
        });
        let settings = ThreadSettings::read_session_response(&response).unwrap();
        assert_eq!(settings.model, "fixture-model");
        assert_eq!(settings.effort.as_deref(), Some("high"));
        assert_eq!(settings.sandbox_kind, "workspaceWrite");
        assert!(settings.summary.is_none());
        assert!(settings.personality.is_none());
    }
}
