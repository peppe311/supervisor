//! Provider-neutral catalog and input snapshots. No provider transport or credentials.
use serde::{Deserialize, Serialize};

/// Explicit tool targets only. A command, search query or prose is not a file target.
#[derive(Clone, Debug)]
pub(crate) struct ToolFileActivity {
    pub paths: Vec<String>,
    pub editing: bool,
}

pub(crate) fn tool_file_activity(
    kind: &str,
    input: &serde_json::Value,
) -> Option<ToolFileActivity> {
    let editing = match kind.to_ascii_lowercase().as_str() {
        "read" | "read_file" | "readfile" | "view_file" => false,
        "edit" | "write" | "multiedit" | "notebookedit" | "edit_file" | "write_file"
        | "replace" | "delete" | "move" => true,
        _ => return None,
    };
    let mut paths = Vec::new();
    let mut add = |value: &serde_json::Value| {
        if let Some(path) = value.as_str()
            && !path.is_empty()
            && path.len() <= 4096
            && !path.chars().any(char::is_control)
            && paths.len() < 256
            && !paths.iter().any(|existing| existing == path)
        {
            paths.push(path.to_owned());
        }
    };
    for location in input["locations"]
        .as_array()
        .into_iter()
        .flatten()
        .take(256)
    {
        add(&location["path"]);
    }
    let arguments = input.get("rawInput").unwrap_or(input);
    for field in [
        "file_path",
        "filePath",
        "path",
        "notebook_path",
        "target_file",
    ] {
        add(&arguments[field]);
    }
    (!paths.is_empty()).then_some(ToolFileActivity { paths, editing })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentProviderPhase {
    Checking,
    Ready,
    Planning,
    Unavailable,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentProviderView {
    pub phase: AgentProviderPhase,
    pub name: &'static str,
    pub detail: String,
    pub authenticated: bool,
    pub version: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct AgentSelection {
    pub model: String,
    pub effort: String,
    pub service_tier: Option<String>,
    pub context_window: Option<u64>,
    pub personality: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct AgentReasoningEffortOption {
    pub reasoning_effort: String,
    pub description: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct AgentServiceTierOption {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct AgentModelUpgradeInfo {
    pub model: String,
    pub upgrade_copy: Option<String>,
    #[serde(skip_serializing)]
    pub model_link: Option<String>,
    #[serde(skip_serializing)]
    pub migration_markdown: Option<String>,
    pub retirement_at: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct AgentModelOption {
    pub supports_personality: bool,
    pub id: String,
    pub model: String,
    pub upgrade: Option<String>,
    pub upgrade_info: Option<AgentModelUpgradeInfo>,
    pub display_name: String,
    pub description: String,
    pub supported_reasoning_efforts: Vec<AgentReasoningEffortOption>,
    pub default_reasoning_effort: String,
    pub input_modalities: Vec<String>,
    pub service_tiers: Vec<AgentServiceTierOption>,
    pub default_service_tier: Option<String>,
    pub is_default: bool,
    #[serde(skip_serializing)]
    pub additional_speed_tiers: Vec<String>,
}

impl AgentModelOption {
    pub(crate) fn supports_image_input(&self) -> bool {
        self.input_modalities.is_empty()
            || self
                .input_modalities
                .iter()
                .any(|modality| modality == "image")
    }

    pub(crate) fn supports_audio_input(&self) -> bool {
        self.input_modalities
            .iter()
            .any(|modality| modality == "audio")
    }
}

pub(crate) fn normalize_selection(
    models: &[AgentModelOption],
    requested: &AgentSelection,
) -> Option<AgentSelection> {
    let model = models
        .iter()
        .find(|model| model.model == requested.model || model.id == requested.model)
        .or_else(|| models.iter().find(|model| model.is_default))
        .or_else(|| models.first())?;
    let model_changed = requested.model != model.model && requested.model != model.id;
    let effort = (!model_changed
        && model
            .supported_reasoning_efforts
            .iter()
            .any(|option| option.reasoning_effort == requested.effort))
    .then(|| requested.effort.clone())
    .or_else(|| {
        model
            .supported_reasoning_efforts
            .iter()
            .find(|option| option.reasoning_effort == model.default_reasoning_effort)
            .map(|option| option.reasoning_effort.clone())
    })
    .or_else(|| {
        model
            .supported_reasoning_efforts
            .first()
            .map(|option| option.reasoning_effort.clone())
    })?;
    let service_tier = if model_changed {
        model
            .default_service_tier
            .clone()
            .filter(|tier| model.service_tiers.iter().any(|option| option.id == *tier))
    } else {
        requested
            .service_tier
            .clone()
            .filter(|tier| model.service_tiers.iter().any(|option| option.id == *tier))
    };

    Some(AgentSelection {
        model: model.model.clone(),
        effort,
        service_tier,
        context_window: requested.context_window,
        personality: requested
            .personality
            .clone()
            .filter(|personality| model.supports_personality && valid_personality(personality)),
    })
}

pub(crate) fn validate_selection<'a>(
    models: &'a [AgentModelOption],
    selection: &AgentSelection,
) -> Result<&'a AgentModelOption, String> {
    let model = models
        .iter()
        .find(|model| model.model == selection.model)
        .ok_or_else(|| "The selected model is no longer available".to_owned())?;
    if !model
        .supported_reasoning_efforts
        .iter()
        .any(|option| option.reasoning_effort == selection.effort)
    {
        return Err("The selected reasoning effort is not supported by this model".to_owned());
    }
    if let Some(service_tier) = selection.service_tier.as_deref()
        && !model
            .service_tiers
            .iter()
            .any(|option| option.id == service_tier)
    {
        return Err("The selected speed is not supported by this model".to_owned());
    }
    if let Some(context_window) = selection.context_window
        && !(16_000..=2_000_000).contains(&context_window)
    {
        return Err("The selected context window must be between 16K and 2M tokens".to_owned());
    }
    if let Some(personality) = selection.personality.as_deref()
        && (!model.supports_personality || !valid_personality(personality))
    {
        return Err("The selected personality is not supported by this model".to_owned());
    }
    Ok(model)
}

fn valid_personality(personality: &str) -> bool {
    matches!(personality, "none" | "friendly" | "pragmatic")
}

impl AgentProviderView {
    #[cfg(test)]
    pub(crate) fn ready(version: Option<String>, detail: impl Into<String>) -> Self {
        Self {
            phase: AgentProviderPhase::Ready,
            name: "AI provider",
            detail: detail.into(),
            authenticated: true,
            version,
        }
    }

    pub(crate) fn unavailable(detail: impl Into<String>, version: Option<String>) -> Self {
        Self {
            phase: AgentProviderPhase::Unavailable,
            name: "AI provider",
            detail: detail.into(),
            authenticated: false,
            version,
        }
    }

    pub(crate) fn error(detail: impl Into<String>, version: Option<String>) -> Self {
        Self {
            phase: AgentProviderPhase::Error,
            name: "AI provider",
            detail: detail.into(),
            authenticated: true,
            version,
        }
    }

    pub(crate) fn can_run(&self) -> bool {
        matches!(
            self.phase,
            AgentProviderPhase::Ready | AgentProviderPhase::Planning
        ) && self.authenticated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_activity_accepts_explicit_targets_but_never_shell_commands_or_search_results() {
        use serde_json::json;
        let file = tool_file_activity("Read", &json!({"file_path":"src/main.rs"})).unwrap();
        assert!(!file.editing);
        assert_eq!(file.paths, ["src/main.rs"]);
        let edit = tool_file_activity("edit", &json!({"locations":[{"path":"a.rs"},{"path":"b.rs"},{"path":"a.rs"}],"rawInput":{"path":"a.rs"}})).unwrap();
        assert!(edit.editing);
        assert_eq!(edit.paths, ["a.rs", "b.rs"]);
        assert!(tool_file_activity("Bash", &json!({"command":"cat a.rs","path":"a.rs"})).is_none());
        assert!(tool_file_activity("search", &json!({"path":"src"})).is_none());
        assert!(tool_file_activity("Read", &json!({"file_path":"bad\npath"})).is_none());
    }

    #[test]
    fn legacy_modalities_keep_image_compatibility_but_audio_requires_advertisement() {
        let legacy = AgentModelOption::default();
        assert!(legacy.supports_image_input());
        assert!(!legacy.supports_audio_input());

        let audio = AgentModelOption {
            input_modalities: vec!["text".into(), "audio".into()],
            ..AgentModelOption::default()
        };
        assert!(audio.supports_audio_input());
        assert!(!audio.supports_image_input());
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentBrowserTab {
    pub id: u64,
    pub title: String,
    pub url: String,
    pub active: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentContextElement {
    pub role: String,
    pub name: String,
    pub disabled: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentContextLink {
    pub text: String,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentContextImage {
    pub alt: String,
    pub url: String,
    pub kind: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentTabContext {
    pub tab_id: u64,
    pub title: String,
    pub url: String,
    pub description: String,
    pub language: String,
    pub text: String,
    pub source_text_char_count: usize,
    pub elements: Vec<AgentContextElement>,
    pub source_element_count: usize,
    pub links: Vec<AgentContextLink>,
    pub source_link_count: usize,
    pub images: Vec<AgentContextImage>,
    pub source_image_count: usize,
    pub visual_included: bool,
    pub estimated_token_count: usize,
    pub truncated: bool,
    pub redaction_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentTerminalContext {
    pub session_id: u64,
    pub label: String,
    pub kind: String,
    pub profile_id: Option<String>,
    pub shell: String,
    pub cwd: String,
    pub status: String,
    pub phase: String,
    pub busy: bool,
    pub output: String,
    pub source_output_char_count: usize,
    pub output_revision: u64,
    pub last_exit_code: Option<i32>,
    pub captured_at_ms: u128,
    pub estimated_token_count: usize,
    pub redaction_count: usize,
    pub truncated: bool,
    pub followed_live: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentSshProfile {
    pub id: String,
    pub name: String,
    /// Frozen graph destination, never a local cwd or text supplied by a tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentRemoteDesktop {
    pub profile_id: String,
    pub profile_name: String,
    pub protocol: String,
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentConversationMessage {
    pub role: String,
    pub text: String,
}

#[derive(Clone, Debug)]
pub(crate) struct AgentRunRequest {
    pub run_id: u64,
    pub prompt: String,
    pub conversation_history: Vec<AgentConversationMessage>,
    pub active_tab_id: Option<u64>,
    pub tabs: Vec<AgentBrowserTab>,
    pub contexts: Vec<AgentTabContext>,
    pub terminal_contexts: Vec<AgentTerminalContext>,

    pub ssh_profiles: Vec<AgentSshProfile>,
    pub remote_desktop: Option<AgentRemoteDesktop>,
    pub selection: AgentSelection,
    pub workspace_enabled: bool,
    pub window_access_enabled: bool,
}
