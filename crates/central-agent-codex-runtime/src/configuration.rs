//! Public preference projection of native configuration. No raw config, prompts,
//! credentials or arbitrary key/value editor is exposed to a WebView. Codex owns
//! layering, version conflicts, policy enforcement and TOML mutation.
use crate::api::{self, Call};
use serde::Serialize;
use serde_json::{Value, json};
use std::path::Path;

const KEYS: &[&str] = &[
    "model",
    "review_model",
    "model_reasoning_effort",
    "model_reasoning_summary",
    "model_verbosity",
    "service_tier",
    "web_search",
    "model_context_window",
    "model_auto_compact_token_limit",
    "model_auto_compact_token_limit_scope",
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub kind: String,
    pub location: Option<String>,
    pub profile: Option<String>,
    pub version: String,
    pub disabled_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preference {
    pub key: String,
    /// Decimal strings retain exact i64 context values across JavaScript IPC.
    pub effective: Option<String>,
    pub user_value: Option<String>,
    pub origin: Option<Layer>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub file: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub preferences: Vec<Preference>,
    pub layers: Vec<Layer>,
    pub target: Option<Target>,
}

fn required_text(value: &Value) -> Result<String, String> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && !s.contains('\0'))
        .map(str::to_owned)
        .ok_or_else(|| "Incomplete native configuration metadata".into())
}

fn layer(value: &Value) -> Result<Layer, String> {
    let source = &value["name"];
    if !value["disabledReason"].is_null() && !value["disabledReason"].is_string()
        || !source["profile"].is_null() && !source["profile"].is_string()
    {
        return Err("Malformed native configuration layer metadata".into());
    }
    let kind = required_text(&source["type"])?;
    let location = match kind.as_str() {
        "user" | "system" | "packagedDefaults" | "legacyManagedConfigTomlFromFile" => {
            let file = required_text(&source["file"])?;
            if !Path::new(&file).is_absolute() {
                return Err("Native config path is not absolute".into());
            }
            Some(file)
        }
        "project" => {
            let path = required_text(&source["dotCodexFolder"])?;
            if !Path::new(&path).is_absolute() {
                return Err("Native project config path is not absolute".into());
            }
            Some(path)
        }
        "mdm" => Some(format!(
            "{} / {}",
            required_text(&source["domain"])?,
            required_text(&source["key"])?
        )),
        "enterpriseManaged" => Some(required_text(&source["name"])?),
        "sessionFlags" | "legacyManagedConfigTomlFromMdm" => None,
        _ => {
            return Err(
                "Unrecognized native configuration layer; update the client before editing".into(),
            );
        }
    };
    Ok(Layer {
        kind,
        location,
        profile: source["profile"].as_str().map(str::to_owned),
        version: required_text(&value["version"])?,
        disabled_reason: value["disabledReason"].as_str().map(str::to_owned),
    })
}

fn integer_key(key: &str) -> bool {
    matches!(
        key,
        "model_context_window" | "model_auto_compact_token_limit"
    )
}

fn public_value(key: &str, value: &Value) -> Result<Option<String>, String> {
    if value.is_null() {
        return Ok(None);
    }
    if integer_key(key) {
        return value
            .as_i64()
            .map(|n| Some(n.to_string()))
            .ok_or_else(|| format!("Native {key} is not an integer"));
    }
    value
        .as_str()
        .map(|s| Some(s.to_owned()))
        .ok_or_else(|| format!("Native {key} is not a string"))
}

impl Snapshot {
    pub fn read(value: &Value) -> Result<Self, String> {
        let config = value["config"]
            .as_object()
            .ok_or("Missing effective native configuration")?;
        let origins = value["origins"]
            .as_object()
            .ok_or("Missing native configuration origins")?;
        let raw_layers = value["layers"]
            .as_array()
            .ok_or("Native configuration layers were not returned")?;
        let layers = raw_layers
            .iter()
            .map(layer)
            .collect::<Result<Vec<_>, _>>()?;
        let base_users: Vec<usize> = layers
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                (l.kind == "user"
                    && raw_layers[i]["name"]
                        .get("profile")
                        .is_some_and(Value::is_null))
                .then_some(i)
            })
            .collect();
        if base_users.len() > 1 {
            return Err("Ambiguous native user configuration layers".into());
        }
        let base = base_users.first().copied();
        if base.is_some_and(|i| !raw_layers[i]["config"].is_object()) {
            return Err("Missing native user configuration object".into());
        }
        let target = base
            .filter(|&i| layers[i].disabled_reason.is_none())
            .map(|i| Target {
                file: layers[i]
                    .location
                    .clone()
                    .expect("User layer has a validated file"),
                version: layers[i].version.clone(),
            });
        let preferences = KEYS
            .iter()
            .map(|&key| {
                Ok(Preference {
                    key: key.into(),
                    effective: public_value(key, config.get(key).unwrap_or(&Value::Null))?,
                    user_value: public_value(
                        key,
                        base.map(|i| &raw_layers[i]["config"][key])
                            .unwrap_or(&Value::Null),
                    )?,
                    origin: origins
                        .get(key)
                        .filter(|v| !v.is_null())
                        .map(layer)
                        .transpose()?,
                })
            })
            .collect::<Result<_, String>>()?;
        Ok(Self {
            preferences,
            layers,
            target,
        })
    }

    /// A confirmed, single preference edit. The caller owns connection, view,
    /// project and in-flight identity validation; it must never auto-retry writes.
    /// Model/effort availability and managed requirements remain native decisions.
    pub fn edit(&self, key: &str, text: &str) -> Result<Call, String> {
        if !KEYS.contains(&key) {
            return Err("This is not a supported public Codex preference".into());
        }
        let target = self
            .target
            .as_ref()
            .ok_or("No enabled base user configuration layer is available")?;
        let value = if integer_key(key) {
            let number: i64 = text
                .parse()
                .map_err(|_| "Enter a whole token count, without separators")?;
            if number <= 0 {
                return Err("Enter a positive token count".into());
            }
            json!(number)
        } else {
            if text.trim().is_empty() || text.contains('\0') {
                return Err("Enter a nonempty preference value".into());
            }
            let choices: &[&str] = match key {
                "model_reasoning_summary" => &["auto", "concise", "detailed", "none"],
                "model_verbosity" => &["low", "medium", "high"],
                "web_search" => &["disabled", "cached", "live"],
                "model_auto_compact_token_limit_scope" => &["total", "body_after_prefix"],
                _ => &[],
            };
            if !choices.is_empty() && !choices.contains(&text) {
                return Err("Unsupported native preference value".into());
            }
            json!(text)
        };
        Ok(api::config_write(&target.file, &target.version, key, value))
    }

    /// Remove exactly one observed base-user override through the native writer.
    /// Null deletion was verified against isolated official runtime 0.153.4;
    /// this is not a reset of project/managed layers or loaded conversations.
    pub fn clear(&self, key: &str) -> Result<Call, String> {
        if !KEYS.contains(&key) {
            return Err("This is not a supported public Codex preference".into());
        }
        let target = self
            .target
            .as_ref()
            .ok_or("No enabled base user configuration layer is available")?;
        if !self
            .preferences
            .iter()
            .any(|p| p.key == key && p.user_value.is_some())
        {
            return Err(
                "There is no observed saved user value to clear; refresh native preferences".into(),
            );
        }
        Ok(api::config_write(
            &target.file,
            &target.version,
            key,
            Value::Null,
        ))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteOutcome {
    pub overridden: bool,
    pub file: String,
    pub version: String,
    pub overriding_layer: Option<Layer>,
    pub effective: Option<String>,
}

impl WriteOutcome {
    /// Native ACK is saved-config evidence, not proof that a loaded thread has
    /// changed. Only project config/read can refresh the displayed preferences.
    pub fn read(value: &Value, target: &Target, key: &str) -> Result<Self, String> {
        if !KEYS.contains(&key) {
            return Err("Unknown written preference".into());
        }
        let file = required_text(&value["filePath"])?;
        if Path::new(&file) != Path::new(&target.file) {
            return Err(
                "Native configuration reply names a different file; refresh before continuing"
                    .into(),
            );
        }
        let overridden = match value["status"].as_str() {
            Some("ok") => false,
            Some("okOverridden") => true,
            _ => {
                return Err(
                    "Unknown native configuration write result; refresh before continuing".into(),
                );
            }
        };
        let metadata = &value["overriddenMetadata"];
        if overridden && !metadata.is_object() {
            return Err("Missing native override details; refresh before continuing".into());
        }
        Ok(Self {
            overridden,
            file,
            version: required_text(&value["version"])?,
            overriding_layer: if overridden {
                Some(layer(&metadata["overridingLayer"])?)
            } else {
                None
            },
            effective: if overridden {
                public_value(key, &metadata["effectiveValue"])?
            } else {
                None
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Value {
        let root = std::env::current_dir().unwrap();
        let user = json!({"name":{"type":"user","file":root.join("config.toml"),"profile":null},"version":"native-v1","config":{"model":"user-model","env":{"TOKEN":"secret-layer"}},"disabledReason":null});
        json!({"config":{"model":"effective-model","model_context_window":9007199254740993_i64,"developer_instructions":"secret-instructions","model_providers":{"private":{"api_key":"secret-key"}}},
            "origins":{"model":{"name":{"type":"sessionFlags"},"version":"flags-v1"},"developer_instructions":{"arbitrary":"secret-origin"}},
            "layers":[user]})
    }
    #[test]
    fn projects_effective_user_and_origins_without_private_configuration() {
        let snapshot = Snapshot::read(&fixture()).unwrap();
        assert_eq!(snapshot.preferences.len(), KEYS.len());
        assert_eq!(
            snapshot.preferences[0].effective.as_deref(),
            Some("effective-model")
        );
        assert_eq!(
            snapshot.preferences[0].user_value.as_deref(),
            Some("user-model")
        );
        assert_eq!(
            snapshot.preferences[0].origin.as_ref().unwrap().kind,
            "sessionFlags"
        );
        assert_eq!(
            snapshot.preferences[7].effective.as_deref(),
            Some("9007199254740993")
        );
        assert_eq!(snapshot.preferences[1].effective, None);
        let serialized = serde_json::to_string(&snapshot).unwrap();
        for private in ["secret-", "developer_instructions", "api_key", "TOKEN"] {
            assert!(!serialized.contains(private));
        }
    }
    #[test]
    fn only_enabled_base_user_layer_is_a_write_target() {
        let mut value = fixture();
        let mut profile = value["layers"][0].clone();
        profile["name"]["profile"] = json!("review");
        profile["version"] = json!("profile-revision");
        value["layers"]
            .as_array_mut()
            .unwrap()
            .push(profile.clone());
        assert_eq!(
            Snapshot::read(&value).unwrap().target.unwrap().version,
            "native-v1"
        );
        value["layers"][0]["disabledReason"] = json!("managed policy");
        assert!(
            Snapshot::read(&value)
                .unwrap()
                .edit("model", "new-model")
                .is_err()
        );
        value["layers"] = json!([profile]);
        assert!(Snapshot::read(&value).unwrap().target.is_none());
        value["layers"] = json!([]);
        assert!(Snapshot::read(&value).unwrap().target.is_none());
    }
    #[test]
    fn malformed_or_ambiguous_native_metadata_fails_closed() {
        for field in ["config", "origins", "layers"] {
            let mut v = fixture();
            v[field] = Value::Null;
            assert!(Snapshot::read(&v).is_err());
        }
        let mut v = fixture();
        let duplicate = v["layers"][0].clone();
        v["layers"].as_array_mut().unwrap().push(duplicate);
        assert!(Snapshot::read(&v).is_err());
        for (path, bad) in [("file", "relative.toml"), ("type", "future-kind")] {
            let mut v = fixture();
            v["layers"][0]["name"][path] = json!(bad);
            assert!(Snapshot::read(&v).is_err());
        }
        let mut v = fixture();
        v["layers"][0]["version"] = json!("");
        assert!(Snapshot::read(&v).is_err());
    }
    #[test]
    fn writes_use_the_observed_native_target_and_revision_without_reload() {
        let snapshot = Snapshot::read(&fixture()).unwrap();
        let call = snapshot.edit("model_verbosity", "high").unwrap();
        assert_eq!(call.method, "config/batchWrite");
        assert_eq!(
            call.params["filePath"],
            snapshot.target.as_ref().unwrap().file
        );
        assert_eq!(call.params["expectedVersion"], "native-v1");
        assert_eq!(call.params["reloadUserConfig"], false);
        assert_eq!(
            call.params["edits"],
            json!([{"keyPath":"model_verbosity","value":"high","mergeStrategy":"upsert"}])
        );
        assert_eq!(
            snapshot
                .edit("model_context_window", "9007199254740993")
                .unwrap()
                .params["edits"][0]["value"],
            9007199254740993_i64
        );
    }

    #[test]
    fn clearing_requires_an_observed_public_user_override_and_preserves_scope() {
        let mut value = fixture();
        let snapshot = Snapshot::read(&value).unwrap();
        let call = snapshot.clear("model").unwrap();
        assert_eq!(call.method, "config/batchWrite");
        assert_eq!(
            call.params["filePath"],
            snapshot.target.as_ref().unwrap().file
        );
        assert_eq!(call.params["expectedVersion"], "native-v1");
        assert_eq!(call.params["reloadUserConfig"], false);
        assert_eq!(
            call.params["edits"],
            json!([{"keyPath":"model","value":null,"mergeStrategy":"upsert"}])
        );
        for key in [
            "model_verbosity",
            "env",
            "developer_instructions",
            "model.providers",
            "",
        ] {
            assert!(snapshot.clear(key).is_err(), "{key}");
        }
        // Empty input is not an implicit delete operation.
        assert!(snapshot.edit("model", "").is_err());
        value["layers"][0]["disabledReason"] = json!("managed policy");
        assert!(Snapshot::read(&value).unwrap().clear("model").is_err());
    }
    #[test]
    fn rejects_arbitrary_configuration_and_malformed_preferences() {
        let snapshot = Snapshot::read(&fixture()).unwrap();
        for (key, text) in [
            ("developer_instructions", "inject"),
            ("env.TOKEN", "secret"),
            ("model", ""),
            ("model", "bad\0"),
            ("model_verbosity", "ultra"),
            ("web_search", "true"),
            ("model_context_window", "1.5"),
            ("model_context_window", "0"),
            ("model_auto_compact_token_limit", "-1"),
            ("model_auto_compact_token_limit_scope", "all"),
            ("model_auto_compact_token_limit_scope", "TOTAL"),
            ("model_auto_compact_token_limit_scope", " body_after_prefix"),
            ("model_context_window", "9223372036854775808"),
        ] {
            assert!(snapshot.edit(key, text).is_err(), "{key}={text}");
        }
    }
    #[test]
    fn compaction_scope_uses_native_enum_and_preserves_layer_and_write_identity() {
        let key = "model_auto_compact_token_limit_scope";
        let mut value = fixture();
        value["config"][key] = json!("total");
        value["layers"][0]["config"][key] = json!("body_after_prefix");
        let snapshot = Snapshot::read(&value).unwrap();
        let preference = snapshot.preferences.iter().find(|p| p.key == key).unwrap();
        assert_eq!(preference.effective.as_deref(), Some("total"));
        assert_eq!(preference.user_value.as_deref(), Some("body_after_prefix"));
        for option in ["total", "body_after_prefix"] {
            let call = snapshot.edit(key, option).unwrap();
            assert_eq!(call.params["reloadUserConfig"], false);
            assert_eq!(
                call.params["filePath"],
                snapshot.target.as_ref().unwrap().file
            );
            assert_eq!(call.params["expectedVersion"], "native-v1");
            assert_eq!(
                call.params["edits"],
                json!([{"keyPath":key,"value":option,"mergeStrategy":"upsert"}])
            );
        }
        assert_eq!(
            snapshot.clear(key).unwrap().params["edits"],
            json!([{"keyPath":key,"value":null,"mergeStrategy":"upsert"}])
        );
        assert!(Snapshot::read(&fixture()).unwrap().clear(key).is_err());
    }
    #[test]
    fn native_override_is_not_reported_as_effective_requested_value() {
        let snapshot = Snapshot::read(&fixture()).unwrap();
        let target = snapshot.target.unwrap();
        let response = json!({"status":"okOverridden","filePath":target.file,"version":"native-v2", "overriddenMetadata":{"message":"not propagated arbitrary secret","overridingLayer":{"name":{"type":"sessionFlags"},"version":"flags"},"effectiveValue":"low"}});
        let outcome = WriteOutcome::read(&response, &target, "model_verbosity").unwrap();
        assert!(outcome.overridden);
        assert_eq!(outcome.effective.as_deref(), Some("low"));
        assert!(!serde_json::to_string(&outcome).unwrap().contains("secret"));
        let ok = json!({"status":"ok","filePath":target.file,"version":"native-v2","overriddenMetadata":null});
        assert!(
            WriteOutcome::read(&ok, &target, "model")
                .unwrap()
                .effective
                .is_none()
        );
        for key in ["filePath", "version", "status", "overriddenMetadata"] {
            let mut invalid = response.clone();
            invalid[key] = Value::Null;
            assert!(WriteOutcome::read(&invalid, &target, "model_verbosity").is_err());
        }
    }
}
