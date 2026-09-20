//! Immutable execution identity. UI selection and checkpoint lifetime must never
//! decide the workspace or process owner of an already accepted run.
use super::*;

#[derive(Clone, Debug)]
pub(super) struct RunExecutionContext {
    pub conversation_key: String,
    pub workspace_root: Option<PathBuf>,
    pub ssh_profile_id: Option<String>,
    pub provider: AgentProviderKind,

    pub native_targets: native_targets::NativeTargets,
    pub terminal_session_id: Option<u64>,
}

impl RunExecutionContext {
    pub(super) fn local_terminal_directory(&self) -> Result<PathBuf, String> {
        if self.workspace_root.is_some() || self.ssh_profile_id.is_some() {
            return self.local_workspace()?.resolve_directory(None);
        }
        let root = self.native_targets.terminal_cwd.as_deref()
            .ok_or("This request has no saved local shell directory. Connect a project and start a new request.")?;
        let runtime = WorkspaceRuntime::scoped(root)?;
        if runtime.root() != Some(root) {
            return Err(
                "The saved shell directory changed. Review it before starting a new request."
                    .to_owned(),
            );
        }
        runtime.resolve_directory(None)
    }

    fn local_workspace(&self) -> Result<WorkspaceRuntime, String> {
        if self.ssh_profile_id.is_some() {
            return Err(
                "Remote Agent Graph agents must use the SSH tools for remote files and commands"
                    .to_owned(),
            );
        }
        let root = self.workspace_root.as_deref().ok_or_else(|| {
            "This run was started without a local workspace. Connect a project and start a new request.".to_owned()
        })?;
        let runtime = WorkspaceRuntime::scoped(root)?;
        if runtime.root() != Some(root) {
            return Err("The run's workspace target changed. Start a new request after reviewing the project location.".to_owned());
        }
        Ok(runtime)
    }

    fn permits_ssh_profile(&self, profile_id: &str) -> bool {
        self.ssh_profile_id
            .as_deref()
            .is_none_or(|bound| bound == profile_id)
    }
}

fn execution_context(
    contexts: &HashMap<u64, RunExecutionContext>,
    run_id: u64,
) -> Result<&RunExecutionContext, String> {
    contexts.get(&run_id).ok_or_else(|| {
        format!("Run {run_id} has no execution context. The action was not redirected to the selected project.")
    })
}

impl BrowserApp {
    pub(super) fn workspace_for_run(&self, run_id: u64) -> Result<WorkspaceRuntime, String> {
        execution_context(&self.run_execution_contexts, run_id)?.local_workspace()
    }

    pub(super) fn validate_run_ssh_profile(
        &self,
        run_id: u64,
        profile_id: &str,
    ) -> Result<(), String> {
        if execution_context(&self.run_execution_contexts, run_id)?.permits_ssh_profile(profile_id)
        {
            Ok(())
        } else {
            Err("This graph run belongs to a different SSH profile. Start a request on that server's node instead.".to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(owner: &str, root: Option<PathBuf>) -> RunExecutionContext {
        RunExecutionContext {
            conversation_key: owner.to_owned(),
            workspace_root: root,
            ssh_profile_id: None,
            provider: AgentProviderKind::ClaudeCode,

            native_targets: native_targets::NativeTargets::default(),
            terminal_session_id: None,
        }
    }

    #[test]
    fn read_and_command_cwd_stay_in_original_project_before_any_checkpoint() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        fs::write(first.path().join("owned.txt"), "first chat").unwrap();
        fs::write(second.path().join("owned.txt"), "second chat").unwrap();
        let mut selected_workspace = WorkspaceRuntime::default();
        selected_workspace
            .connect(first.path().to_path_buf())
            .unwrap();
        let run = context("chat:first", selected_workspace.root().map(PathBuf::from));
        selected_workspace
            .connect(second.path().to_path_buf())
            .unwrap();

        let owned = run.local_workspace().unwrap();
        assert_eq!(
            owned.root(),
            Some(first.path().canonicalize().unwrap().as_path())
        );
        assert_eq!(
            owned.resolve_directory(None).unwrap(),
            first.path().canonicalize().unwrap()
        );
        let command = WorkspaceCommand::Read {
            path: "owned.txt".to_owned(),
            start_line: 1,
            line_count: 10,
        };
        let result = owned.execute(&command).unwrap();
        assert!(result.to_string().contains("first chat"));
        assert!(!result.to_string().contains("second chat"));
        assert_eq!(
            selected_workspace.root(),
            Some(second.path().canonicalize().unwrap().as_path())
        );
    }

    #[test]
    fn missing_or_unbound_run_never_inherits_the_selected_workspace() {
        let contexts = HashMap::new();
        assert!(
            execution_context(&contexts, 99)
                .unwrap_err()
                .contains("not redirected")
        );
        assert!(context("chat:unbound", None).local_workspace().is_err());
    }

    #[test]
    fn removed_workspace_fails_instead_of_falling_back_to_another_project() {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("removed");
        let run = context("chat:removed", Some(root));
        assert!(run.local_workspace().is_err());
    }

    #[test]
    fn remote_run_cannot_enter_local_workspace_or_another_server() {
        let root = tempfile::tempdir().unwrap();
        let mut run = context("graph:remote-node", Some(root.path().to_path_buf()));
        run.ssh_profile_id = Some("server-a".to_owned());
        assert!(run.local_workspace().is_err());
        assert!(run.permits_ssh_profile("server-a"));
        assert!(!run.permits_ssh_profile("server-b"));
    }

    #[test]
    fn sibling_chats_have_distinct_process_owners_even_in_the_same_project() {
        let first = context("chat:first", None);
        let second = context("chat:second", None);
        assert_ne!(first.conversation_key, second.conversation_key);
    }

    #[test]
    fn shell_without_a_project_retains_its_accepted_directory() {
        let original = tempfile::tempdir().unwrap();
        let mut run = context("chat:unbound-project", None);
        run.native_targets.terminal_cwd = Some(original.path().canonicalize().unwrap());
        assert_eq!(
            run.local_terminal_directory().unwrap(),
            original.path().canonicalize().unwrap()
        );
        run.ssh_profile_id = Some("remote-only".to_owned());
        assert!(run.local_terminal_directory().is_err());
    }
}
