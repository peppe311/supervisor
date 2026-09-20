use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentProviderKind {
    // A distinct persisted ID prevents old Codex adapter state from being resumed.
    CodexAppServer,
    #[default]
    ClaudeCode,
    Cursor,
    GithubCopilot,
    GoogleAntigravity,
    OpencodeGo,
}

impl AgentProviderKind {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::CodexAppServer => "Codex",
            Self::ClaudeCode => "Claude Code",
            Self::Cursor => "Cursor",
            Self::GithubCopilot => "GitHub Copilot",
            Self::GoogleAntigravity => "Google AI Pro / Ultra",
            Self::OpencodeGo => "OpenCode Go",
        }
    }

    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::CodexAppServer => "codex_app_server",
            Self::ClaudeCode => "claude_code",
            Self::Cursor => "cursor",
            Self::GithubCopilot => "github_copilot",
            Self::GoogleAntigravity => "google_antigravity",
            Self::OpencodeGo => "opencode_go",
        }
    }

    pub(crate) const fn uses_subscription_adapter(self) -> bool {
        matches!(
            self,
            Self::Cursor | Self::GithubCopilot | Self::GoogleAntigravity | Self::OpencodeGo
        )
    }

    pub(crate) const fn supports_interactive_permission_bridge(self) -> bool {
        !matches!(self, Self::GoogleAntigravity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_defaults_to_claude_and_rejects_the_removed_provider() {
        assert_eq!(AgentProviderKind::default(), AgentProviderKind::ClaudeCode);
        assert!(serde_json::from_str::<AgentProviderKind>(r#""codex""#).is_err());
        for provider in [
            AgentProviderKind::CodexAppServer,
            AgentProviderKind::ClaudeCode,
            AgentProviderKind::Cursor,
            AgentProviderKind::GithubCopilot,
            AgentProviderKind::GoogleAntigravity,
            AgentProviderKind::OpencodeGo,
        ] {
            let encoded = serde_json::to_string(&provider).unwrap();
            assert_eq!(
                serde_json::from_str::<AgentProviderKind>(&encoded).unwrap(),
                provider
            );
        }
    }

    #[test]
    fn provider_ids_are_stable_for_session_persistence() {
        assert_eq!(
            serde_json::to_string(&AgentProviderKind::ClaudeCode).unwrap(),
            r#""claude_code""#
        );
        assert_eq!(
            serde_json::to_string(&AgentProviderKind::GithubCopilot).unwrap(),
            r#""github_copilot""#
        );
        assert_eq!(
            serde_json::to_string(&AgentProviderKind::GoogleAntigravity).unwrap(),
            r#""google_antigravity""#
        );
        assert_eq!(
            serde_json::to_string(&AgentProviderKind::OpencodeGo).unwrap(),
            r#""opencode_go""#
        );
    }
}
