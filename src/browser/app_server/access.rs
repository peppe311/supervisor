//! Native preset intent and Windows setup; never an alternate permission engine.
use super::*;
mod preference;

const ERROR_KEY: &str = "permissions";

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum AccessAction {
    ReadOnly {},
    WorkspaceWrite {},
    ConfirmFullAccess {},
    SetSummary {
        value: Option<api::ReasoningSummary>,
    },
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Setup {
    mode: Option<api::WindowsSandboxMode>,
    phase: &'static str,
    detail: String,
    #[serde(skip)]
    completed_success: Option<bool>,
}
impl Setup {
    pub(super) fn busy(&self) -> bool {
        matches!(self.phase, "starting" | "running" | "finishing")
    }
    pub(super) fn begin(&mut self, mode: api::WindowsSandboxMode) {
        self.mode = Some(mode);
        self.phase = "starting";
        self.completed_success = None;
        self.detail = "Starting native Windows sandbox setup…".into();
    }
    pub(super) fn replied(
        &mut self,
        mode: api::WindowsSandboxMode,
        result: &Result<Value, CallError>,
    ) {
        // Completion can precede its request acknowledgement.
        if self.mode != Some(mode) {
            return;
        }
        if self.phase == "finishing" {
            self.phase = if self.completed_success == Some(true) {
                "ready"
            } else {
                "error"
            };
            return;
        }
        if self.phase != "starting" {
            return;
        }
        match result {
            Ok(v) if v["started"] == true => {
                self.phase = "running";
                self.detail = "Waiting for native sandbox setup to finish…".into();
            }
            Ok(_) => {
                self.phase = "error";
                self.detail =
                    "Codex did not start sandbox setup. Check native configuration and retry."
                        .into();
            }
            Err(e) => {
                self.phase = "error";
                self.detail = e.to_string();
            }
        }
    }
    pub(super) fn completed(&mut self, params: &Value) {
        let Ok(mode) = serde_json::from_value::<api::WindowsSandboxMode>(params["mode"].clone())
        else {
            return;
        };
        if self.mode != Some(mode) || !matches!(self.phase, "starting" | "running") {
            return;
        }
        let awaiting_ack = self.phase == "starting";
        let success = params["success"] == true;
        self.completed_success = Some(success);
        self.phase = if awaiting_ack {
            "finishing"
        } else if success {
            "ready"
        } else {
            "error"
        };
        self.detail = if success {
            "Native Windows sandbox setup completed.".into()
        } else {
            params["error"]
                .as_str()
                .unwrap_or("Native sandbox setup failed")
                .into()
        };
    }
    pub(super) fn disconnected(&mut self) {
        if self.busy() {
            self.phase = "unknown";
            self.detail="Connection lost during setup. Its outcome is unknown; no automatic retry was performed.".into();
        }
    }
}
impl State {
    pub(in crate::browser) fn summary(&self, owner: &str) -> Option<api::ReasoningSummary> {
        self.summaries
            .get(owner)
            .copied()
            .unwrap_or(Some(api::ReasoningSummary::Auto))
    }
    pub(in crate::browser) fn inherit_summary(
        &mut self,
        owner: String,
        summary: Option<api::ReasoningSummary>,
    ) {
        self.summaries.entry(owner).or_insert(summary);
    }
    pub(in crate::browser) fn access(&self) -> api::Access {
        self.access
    }
    pub(super) fn load_access_preference(&mut self, data_dir: &Path) {
        match preference::load(data_dir) {
            Ok(access) => self.access = access,
            Err(error) => {
                self.access = api::Access::ReadOnly;
                self.view.errors.insert(ERROR_KEY, format!("Saved Codex permissions could not be loaded: {error}. Read only is active. Select permissions again to save a new choice."));
            }
        }
    }
    fn save_access_preference(
        &mut self,
        data_dir: &Path,
        access: api::Access,
    ) -> Result<(), String> {
        if self.binding_lock.is_none() || self.binding_store_error.is_some() {
            return Err(
                "Codex storage is unavailable or owned by another Supervisor instance".into(),
            );
        }
        access.validate_requirements(self.requirements.as_ref())?;
        preference::save(data_dir, access).map_err(|error| {
            format!(
                "Codex permissions could not be saved. The previous choice remains active: {error}"
            )
        })?;
        self.access = access;
        self.view.errors.remove(ERROR_KEY);
        Ok(())
    }
}
impl BrowserApp {
    fn app_server_permissions_busy(&self) -> bool {
        self.app_server.any_busy()
            || self.app_server.view.sandbox_setup.busy()
            || self
                .pending_agent_submissions
                .iter()
                .chain(&self.agent_submission_queue)
                .any(|input| input.provider == AgentProviderKind::CodexAppServer)
    }
    pub(in crate::browser) fn native_access_selected(&self, owner: &str) -> bool {
        if let Some(node) = owner.strip_prefix("graph:") {
            self.agent_graph_bindings
                .iter()
                .any(|b| b.node_key() == node && b.provider == AgentProviderKind::CodexAppServer)
        } else if let Some(chat_id) = owner.strip_prefix("chat:")
            && let Some(profile) = self.project_chat_card_profiles.get(chat_id)
            && self.is_project_chat_card_owner(owner)
        {
            profile.provider == AgentProviderKind::CodexAppServer
        } else if self.is_supervised_project_chat_owner(owner) {
            self.app_server.conversations.binding(owner).is_some()
        } else {
            self.agent_provider == AgentProviderKind::CodexAppServer
                && owner == self.composer_owner()
        }
    }
    pub(in crate::browser) fn app_server_access_view(&self, owner: &str) -> Value {
        let options: Vec<_> = [
            (api::Access::ReadOnly, "Read only"),
            (api::Access::WorkspaceWrite, "Project access"),
            (api::Access::FullAccess, "Full access"),
        ]
        .into_iter()
        .map(|(access, label)| {
            let error = access
                .validate_requirements(self.app_server.requirements.as_ref())
                .err();
            json!({"value":access,"label":label,"allowed":error.is_none(),"reason":error})
        })
        .collect();
        json!({"visible":self.native_access_selected(owner),"selected":self.app_server.access(),"summary":self.app_server.summary(owner),
            "disabled":!self.app_server.configuration().0.can_run() || self.conversation_busy(owner),
            "permissionsDisabled":!self.app_server.configuration().0.can_run() || self.app_server_permissions_busy(),
            "permissionsBusy":self.app_server_permissions_busy(),"error":self.app_server.view.errors.get(ERROR_KEY),"options":options})
    }
    pub(in crate::browser) fn app_server_access(&mut self, owner: String, action: AccessAction) {
        let result = (|| -> Result<(), String> {
            if !self.native_access_selected(&owner) {
                return Err("Select Codex in this conversation first".into());
            }
            if self.conversation_target(&owner)?.remote {
                return Err("Native access profiles require a local directory".into());
            }
            if !self.app_server.configuration().0.can_run() {
                return Err("Connect Codex and load its configuration first".into());
            }
            let access = match action {
                AccessAction::ReadOnly {} => api::Access::ReadOnly,
                AccessAction::WorkspaceWrite {} => api::Access::WorkspaceWrite,
                AccessAction::ConfirmFullAccess {} => api::Access::FullAccess,
                AccessAction::SetSummary { value } => {
                    if self.conversation_busy(&owner) {
                        return Err(
                            "Finish or clear pending input before changing its summaries".into(),
                        );
                    }
                    self.app_server.summaries.insert(owner.clone(), value);
                    return Ok(());
                }
            };
            if self.app_server_permissions_busy() {
                return Err("Finish or clear active and queued Codex work before changing permissions for all agents".into());
            }
            self.app_server
                .save_access_preference(&self.data_dir, access)
        })();
        self.conversation_event("central-agent:app-server-access",json!({"owner":owner,"error":result.err(),"nativeAccess":self.app_server_access_view(&owner)}));
        self.render_agent_panel();
    }
    pub(super) fn start_native_sandbox_setup(&mut self, mode: api::WindowsSandboxMode) {
        let error = if !cfg!(windows) {
            Some("Native Windows sandbox setup is only available on Windows".to_owned())
        } else if self.app_server.any_busy() || self.app_server.view.sandbox_setup.busy() {
            Some("Finish active Codex work or sandbox setup before starting setup again".into())
        } else if !self.app_server.view.requirements_loaded {
            Some("Load native configuration requirements first".into())
        } else {
            self.app_server
                .requirements
                .as_ref()
                .and_then(|r| r.get("allowedWindowsSandboxImplementations"))
                .filter(|v| !v.is_null())
                .and_then(|allowed| {
                    (!allowed
                        .as_array()
                        .is_some_and(|items| items.contains(&json!(mode))))
                    .then(|| {
                        "This sandbox implementation is not permitted by managed configuration"
                            .into()
                    })
                })
        };
        if let Some(error) = error {
            self.app_server.view.errors.insert("windows-sandbox", error);
            return;
        }
        self.app_server.view.errors.remove("windows-sandbox");
        self.app_server.view.sandbox_setup.begin(mode);
        self.app_server_call(Query::SandboxSetup(mode), api::setup_windows_sandbox(mode));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn summaries_are_owner_scoped_with_explicit_off_distinct_from_inheritance() {
        let mut state = State::default();
        assert_eq!(state.summary("chat:a"), Some(api::ReasoningSummary::Auto));
        state.inherit_summary("chat:a".into(), Some(api::ReasoningSummary::None));
        state.inherit_summary("graph:b".into(), None);
        state.inherit_summary("chat:a".into(), Some(api::ReasoningSummary::Detailed));
        assert_eq!(state.summary("chat:a"), Some(api::ReasoningSummary::None));
        assert_eq!(state.summary("graph:b"), None);
        assert_eq!(state.summary("chat:new"), Some(api::ReasoningSummary::Auto));
        assert!(
            serde_json::from_value::<AccessAction>(
                json!({"kind":"set_summary","value":"detailed"})
            )
            .is_ok()
        );
        assert!(
            serde_json::from_value::<AccessAction>(json!({"kind":"set_summary","value":"private"}))
                .is_err()
        );
    }
    #[test]
    fn shared_permissions_survive_restart_and_reach_every_native_owner() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = State::load(dir.path());
        assert_eq!(state.access(), api::Access::ReadOnly);
        for (access, sandbox, approval) in [
            (api::Access::WorkspaceWrite, "workspace-write", "on-request"),
            (api::Access::FullAccess, "danger-full-access", "never"),
            (api::Access::ReadOnly, "read-only", "untrusted"),
        ] {
            state.requirements = Some(json!({}));
            state.save_access_preference(dir.path(), access).unwrap();
            drop(state);
            state = State::load(dir.path());
            assert_eq!(state.access(), access);
            assert!(!state.configuration().0.can_run());
            for owner in [
                "chat:main",
                "chat:another",
                "graph:node",
                "chat:fork",
                "chat:delegate",
            ] {
                let selected = state.access();
                let request = state
                    .conversations
                    .open(owner, dir.path(), &api::Profile::default(), selected)
                    .unwrap();
                assert_eq!(request.call.params["sandbox"], sandbox);
                assert_eq!(request.call.params["approvalPolicy"], approval);
            }
            // No preference is embedded in native bindings or account data.
            assert!(
                !serde_json::to_string(state.conversations.saved())
                    .unwrap()
                    .contains("fullAccess")
            );
        }
        assert!(
            fs::metadata(dir.path().join(preference::FILE_NAME))
                .unwrap()
                .len()
                < 256
        );
    }
    #[test]
    fn stored_permissions_do_not_override_new_managed_restrictions() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = State::load(dir.path());
        state.requirements = Some(json!({}));
        state
            .save_access_preference(dir.path(), api::Access::FullAccess)
            .unwrap();
        drop(state);
        let mut state = State::load(dir.path());
        let before = fs::read(dir.path().join(preference::FILE_NAME)).unwrap();
        assert!(
            state
                .access()
                .validate_requirements(state.requirements.as_ref())
                .is_err()
        );
        for requirements in [
            json!({"allowedSandboxModes":["read-only"],"allowedApprovalPolicies":["untrusted"]}),
            json!({"allowedPermissionProfiles":["managed"]}),
        ] {
            state.requirements = Some(requirements);
            assert!(
                state
                    .save_access_preference(dir.path(), api::Access::FullAccess)
                    .is_err()
            );
            assert_eq!(state.access(), api::Access::FullAccess);
            assert_eq!(
                fs::read(dir.path().join(preference::FILE_NAME)).unwrap(),
                before
            );
        }
    }
    #[test]
    fn invalid_or_missing_permissions_never_restore_a_broader_backup() {
        for bytes in [
            b"broken".to_vec(),
            br#"{"version":2,"access":"fullAccess"}"#.to_vec(),
            br#"{"version":1,"access":"unknown"}"#.to_vec(),
            br#"{"version":1,"access":"fullAccess","extra":true}"#.to_vec(),
            vec![b' '; 4097],
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join(preference::FILE_NAME);
            fs::write(&path, &bytes).unwrap();
            fs::write(
                backup_path_for(&path),
                br#"{"version":1,"access":"fullAccess"}"#,
            )
            .unwrap();
            let state = State::load(dir.path());
            assert_eq!(state.access(), api::Access::ReadOnly);
            assert!(state.view.errors.contains_key(ERROR_KEY));
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(preference::FILE_NAME);
        fs::write(
            backup_path_for(&path),
            br#"{"version":1,"access":"fullAccess"}"#,
        )
        .unwrap();
        let state = State::load(dir.path());
        assert_eq!(state.access(), api::Access::ReadOnly);
        assert!(state.view.errors.contains_key(ERROR_KEY));
        assert!(!path.exists());
    }
    #[test]
    fn failed_or_unowned_save_keeps_the_previous_shared_choice() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = State::load(dir.path());
        state.requirements = Some(json!({}));
        state
            .save_access_preference(dir.path(), api::Access::WorkspaceWrite)
            .unwrap();
        let blocked = dir.path().join("blocked");
        fs::write(&blocked, b"not a directory").unwrap();
        assert!(
            state
                .save_access_preference(&blocked, api::Access::FullAccess)
                .is_err()
        );
        assert_eq!(state.access(), api::Access::WorkspaceWrite);
        let mut second = State::load(dir.path());
        second.requirements = Some(json!({}));
        assert!(
            second
                .save_access_preference(dir.path(), api::Access::FullAccess)
                .is_err()
        );
        drop(state);
        assert_eq!(
            State::load(dir.path()).access(),
            api::Access::WorkspaceWrite
        );
    }
    #[test]
    fn full_access_requires_the_explicit_semantic_confirmation() {
        assert!(
            serde_json::from_value::<AccessAction>(json!({"kind":"confirm_full_access"})).is_ok()
        );
        for value in [
            json!({"kind":"full_access"}),
            json!({"kind":"workspace_write","approvalPolicy":"never"}),
            json!({"kind":"confirm_full_access","cwd":"C:/"}),
        ] {
            assert!(serde_json::from_value::<AccessAction>(value).is_err());
        }
    }
    #[test]
    fn windows_setup_waits_for_both_completion_and_acknowledgement() {
        let mut setup = Setup::default();
        setup.begin(api::WindowsSandboxMode::Elevated);
        setup.completed(&json!({"mode":"unelevated","success":true}));
        assert_eq!(setup.phase, "starting");
        setup.completed(&json!({"mode":"elevated","success":true}));
        assert_eq!(setup.phase, "finishing");
        assert!(setup.busy());
        setup.replied(
            api::WindowsSandboxMode::Elevated,
            &Ok(json!({"started":true})),
        );
        assert_eq!(setup.phase, "ready");
        assert!(!setup.busy());
        setup.completed(&json!({"mode":"elevated","success":false,"error":"late duplicate"}));
        assert_eq!(setup.phase, "ready");
    }
    #[test]
    fn windows_setup_failure_and_disconnect_do_not_create_a_success_or_retry() {
        let mut setup = Setup::default();
        setup.begin(api::WindowsSandboxMode::Unelevated);
        setup.replied(
            api::WindowsSandboxMode::Unelevated,
            &Ok(json!({"started":false})),
        );
        assert_eq!(setup.phase, "error");
        setup.begin(api::WindowsSandboxMode::Unelevated);
        setup.replied(
            api::WindowsSandboxMode::Unelevated,
            &Ok(json!({"started":true})),
        );
        assert_eq!(setup.phase, "running");
        setup.disconnected();
        assert_eq!(setup.phase, "unknown");
        assert!(!setup.busy());
        setup.completed(&json!({"mode":"unelevated","success":true}));
        assert_eq!(setup.phase, "unknown");
    }
}
