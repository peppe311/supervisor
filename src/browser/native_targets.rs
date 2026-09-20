//! Ephemeral native handles are scoped to one process lifetime and accepted input.
use super::*;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct NativeTargets {
    pub app_session: String,
    pub browser_tab_id: Option<u64>,
    pub browser_tabs: Vec<AgentBrowserTab>,
    pub remote_desktop_generation: Option<u64>,
    #[serde(default)]
    pub terminal_cwd: Option<PathBuf>,
}

impl NativeTargets {
    pub(super) fn in_session(&self, session: &str) -> Self {
        if self.app_session == session {
            self.clone()
        } else {
            Self {
                terminal_cwd: self.terminal_cwd.clone(),
                ..Self::default()
            }
        }
    }

    pub(super) fn permits_remote(&self, session: &str, generation: u64) -> bool {
        self.app_session == session && self.remote_desktop_generation == Some(generation)
    }
}

impl BrowserApp {
    pub(super) fn capture_native_targets(&self) -> NativeTargets {
        NativeTargets {
            app_session: self.native_session_id.clone(),
            browser_tab_id: self.active_tab_id,
            browser_tabs: self
                .tabs
                .iter()
                .map(|tab| AgentBrowserTab {
                    id: tab.id,
                    title: sanitize_context_title(&tab.title),
                    url: sanitize_context_url(&tab.url),
                    active: self.active_tab_id == Some(tab.id),
                })
                .collect(),
            remote_desktop_generation: (self.remote_desktop.agent_control_enabled()
                && self.remote_desktop.is_connected())
            .then(|| self.remote_desktop.generation()),
            terminal_cwd: self
                .terminal
                .default_working_directory()
                .canonicalize()
                .ok(),
        }
    }

    pub(super) fn browser_target_for_run(&self, run_id: Option<u64>) -> Result<u64, String> {
        let target = match run_id {
            Some(id) => {
                self.run_execution_contexts
                    .get(&id)
                    .ok_or("The run has no native execution context.")?
                    .native_targets
                    .browser_tab_id
            }
            None => self.active_tab_id,
        };
        target.filter(|id| self.tabs.iter().any(|tab| tab.id == *id))
            .ok_or_else(|| "The run's browser tab is unavailable. Read the tab inventory and explicitly select a tab; the action was not redirected.".to_owned())
    }

    pub(super) fn set_run_browser_target(&mut self, run_id: Option<u64>, tab_id: u64) {
        if let Some(context) = run_id.and_then(|id| self.run_execution_contexts.get_mut(&id)) {
            context.native_targets.browser_tab_id = Some(tab_id);
        }
    }

    pub(super) fn validate_remote_target(&self, run_id: Option<u64>) -> Result<(), String> {
        if let Some(id) = run_id {
            let context = self
                .run_execution_contexts
                .get(&id)
                .ok_or("The run has no native execution context.")?;
            if !context
                .native_targets
                .permits_remote(&self.native_session_id, self.remote_desktop.generation())
            {
                return Err("This run is not bound to the current remote desktop session. Review the connection and start a new request.".to_owned());
            }
            if let Some(profile) = context.ssh_profile_id.as_deref()
                && self.remote_desktop.view().profile_id != Some(profile)
            {
                return Err("This run belongs to a different remote server.".to_owned());
            }
        }
        Ok(())
    }

    pub(super) fn local_terminal_for_run(&mut self, run_id: u64) -> Result<(u64, PathBuf), String> {
        let context = self
            .run_execution_contexts
            .get(&run_id)
            .ok_or("The run has no execution context.")?;
        let owner = context.conversation_key.clone();
        let cwd = context.local_terminal_directory()?;
        // Once assigned, closing a run's session is a stop, not permission to choose another.
        if let Some(session_id) = context.terminal_session_id {
            if !self.terminal.is_local_session(session_id) {
                return Err(
                    "This run's shell was closed. Start a new request to open another shell."
                        .to_owned(),
                );
            }
            return Ok((session_id, cwd));
        }
        let session_id = match self
            .local_agent_terminals
            .get(&owner)
            .copied()
            .filter(|id| self.terminal.is_local_session(*id))
        {
            Some(id) => id,
            None => {
                let callback = self.terminal_callback();
                let id = self.terminal.open_in_directory(&cwd, callback)?;
                self.local_agent_terminals.insert(owner, id);
                id
            }
        };
        if let Some(context) = self.run_execution_contexts.get_mut(&run_id) {
            context.terminal_session_id = Some(session_id);
        }
        Ok((session_id, cwd))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_ids_are_not_reused_after_application_restart() {
        let targets = NativeTargets {
            app_session: "before".into(),
            browser_tab_id: Some(1),
            remote_desktop_generation: Some(4),
            ..Default::default()
        };
        let restored: NativeTargets =
            serde_json::from_str(&serde_json::to_string(&targets).unwrap()).unwrap();
        assert_eq!(restored.in_session("before").browser_tab_id, Some(1));
        assert_eq!(restored.in_session("after").browser_tab_id, None);
        assert!(!restored.permits_remote("after", 4));
        assert!(!restored.permits_remote("before", 5));
        assert!(restored.permits_remote("before", 4));
    }
}
