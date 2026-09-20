//! Adapter presentation for the existing provider picker, not another runtime.
use super::*;
use crate::provider_types::AgentProviderPhase;

pub(super) struct Profile {
    pub(super) provider: AgentProviderView,
    pub(super) selection: AgentSelection,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            provider: AgentProviderView {
                phase: AgentProviderPhase::Unavailable,
                name: "Codex App Server",
                detail: "Connect the official runtime in Settings → AI provider".into(),
                authenticated: false,
                version: None,
            },
            selection: AgentSelection::default(),
        }
    }
}

impl State {
    pub(in crate::browser) fn configuration(
        &self,
    ) -> (&AgentProviderView, &[AgentModelOption], &AgentSelection) {
        (
            &self.profile.provider,
            &self.view.models,
            &self.profile.selection,
        )
    }
    pub(super) fn sync_profile(&mut self) {
        self.view.mcp = self.mcp.view.clone();
        self.view.p2 = self.p2.view.clone();
        self.view.account_state = self.account_state.view.clone();
        self.view.chat_reload = self.chat_reload.view.clone();
        let authenticated = self
            .view
            .account
            .as_ref()
            .is_some_and(|account| account.supported);
        // A usage-counter refresh after a turn does not invalidate the loaded
        // account, model catalog or permission requirements. Keep operations
        // usable while that read is pending; all configuration reads still gate.
        let checking_configuration = self.view.refreshing
            && (self.reads.is_empty() || self.reads.iter().any(|key| *key != "rate-limits"));
        let ready = self.view.connected
            && !self.view.sandbox_setup.busy()
            && authenticated
            && !checking_configuration
            && self.view.requirements_loaded
            && !self.view.models.is_empty()
            && self.binding_store_error.is_none();
        self.profile.provider = AgentProviderView {
            phase: if ready {
                AgentProviderPhase::Ready
            } else if self.view.connecting || checking_configuration {
                AgentProviderPhase::Checking
            } else {
                AgentProviderPhase::Unavailable
            },
            name: "Codex App Server",
            detail: if ready {
                "Codex · local chats · managed permissions"
            } else if self.view.connecting {
                "Connecting to Codex…"
            } else if !self.view.connected {
                "Connect Codex in AI accounts"
            } else if self.view.refreshing {
                "Checking the saved ChatGPT sign-in and Codex configuration…"
            } else if let Some(policy) = self
                .view
                .account
                .as_ref()
                .and_then(|account| account.policy.as_deref())
            {
                policy
            } else if !authenticated {
                "Sign in with ChatGPT in AI accounts"
            } else {
                "Wait for Codex to finish checking the account, or review the error in AI accounts"
            }
            .into(),
            authenticated,
            version: self.view.version.clone(),
        };
        // Initial selection only. A removed model must not silently become a
        // different model for a queued or previously configured request.
        if self.profile.selection.model.is_empty()
            && let Some(selection) =
                provider_types::normalize_selection(&self.view.models, &self.profile.selection)
        {
            self.profile.selection = selection;
        }
    }
}

impl BrowserApp {
    pub(in crate::browser) fn configure_app_server(
        &mut self,
        selection: AgentSelection,
    ) -> Result<(), String> {
        provider_types::validate_selection(&self.app_server.view.models, &selection)?;
        if selection.context_window.is_some() {
            return Err(
                "A native context-window override is not implemented; use the model default".into(),
            );
        }
        let bytes = serde_json::to_vec_pretty(&selection).map_err(|e| e.to_string())?;
        write_file_atomically::<AgentSelection>(
            &self.data_dir.join("app-server-profile.json"),
            &bytes,
        )
        .map_err(|e| e.to_string())?;
        self.app_server.profile.selection = selection;
        self.app_server.view.errors.remove("profile");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn connected() -> State {
        let mut state = State::default();
        state.view.connected = true;
        state.view.requirements_loaded = true;
        state.view.account = Some(Account {
            kind: "chatgpt".into(),
            email: None,
            plan_type: None,
            supported: true,
            policy: None,
        });
        state.view.models.push(AgentModelOption {
            model: "fixture-model".into(),
            id: "fixture".into(),
            is_default: true,
            ..AgentModelOption::default()
        });
        state
    }
    #[test]
    fn usage_refresh_preserves_readiness_but_configuration_refresh_does_not() {
        let mut state = connected();
        state.view.refreshing = true;
        state.reads.insert("rate-limits");
        state.sync_profile();
        assert!(state.configuration().0.can_run());
        state.reads.insert("account");
        state.sync_profile();
        assert!(!state.configuration().0.can_run());
        state.reads.remove("account");
        state.view.requirements_loaded = false;
        state.sync_profile();
        assert!(!state.configuration().0.can_run());
    }

    #[test]
    fn subscription_profile_needs_native_account_catalog_and_requirements() {
        let mut state = connected();
        state.sync_profile();
        assert!(state.configuration().0.can_run());
        state.view.account.as_mut().unwrap().kind = "apiKey".into();
        state.view.account.as_mut().unwrap().supported = false;
        state.sync_profile();
        assert!(!state.configuration().0.can_run());
        state.view.account.as_mut().unwrap().kind = "chatgpt".into();
        state.view.account.as_mut().unwrap().supported = true;
        state.view.requirements_loaded = false;
        state.sync_profile();
        assert!(!state.configuration().0.can_run());
        state.view.requirements_loaded = true;
        state.view.refreshing = true;
        state.sync_profile();
        assert!(!state.configuration().0.can_run());
    }
    #[test]
    fn catalog_changes_do_not_silently_replace_an_existing_profile() {
        let mut state = connected();
        state.profile.selection.model = "previous-model".into();
        state.sync_profile();
        assert_eq!(state.configuration().2.model, "previous-model");
        assert!(
            provider_types::validate_selection(state.configuration().1, state.configuration().2)
                .is_err()
        );
    }
    #[test]
    fn p2_controller_state_reaches_the_aggregated_settings_view() {
        let mut state = connected();
        state.p2.sync_scope(Some(Path::new("C:\\p2-project")));
        state.sync_profile();
        let view = serde_json::to_value(&state.view).unwrap();
        assert_eq!(view["p2"]["scope"]["project"], "C:\\p2-project");
        assert!(view["p2"]["scope"]["id"].as_str().is_some());
    }
    #[test]
    fn saved_model_profile_loads_without_native_account_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = serde_json::to_vec(&AgentSelection {
            model: "chosen".into(),
            effort: "high".into(),
            ..AgentSelection::default()
        })
        .unwrap();
        fs::write(dir.path().join("app-server-profile.json"), bytes).unwrap();
        let state = State::load(dir.path());
        assert_eq!(state.configuration().2.model, "chosen");
        assert!(!state.configuration().0.can_run());
    }
}
