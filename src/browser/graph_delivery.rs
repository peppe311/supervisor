//! Graph follow-ups inherit the accepted execution identity, not today's main
//! workspace or selected shell. No SSH discovery or model call is needed here.
use super::*;

impl BrowserApp {
    pub(super) fn prepare_graph_delivery(
        &mut self,
        owner: &str,
        message: String,
        file_ids: Vec<String>,
        tab_ids: Vec<u64>,
        terminal_session_ids: Vec<u64>,
    ) -> Result<PendingAgentSubmission, String> {
        let node = owner.strip_prefix("graph:").ok_or("Invalid graph owner.")?;
        let binding = self
            .agent_graph_bindings
            .iter()
            .find(|binding| binding.node_key() == node)
            .ok_or("This graph agent is no longer assigned.")?;

        if message.trim().is_empty()
            && file_ids.is_empty()
            && tab_ids.is_empty()
            && terminal_session_ids.is_empty()
        {
            return Err("Write a follow-up before choosing Send now or Queue.".into());
        }
        let submission = if binding.provider == AgentProviderKind::CodexAppServer {
            let context =
                self.agent_graph_node_context(&binding.record_type, &binding.record_id)?;
            if context.ssh_profile_id.is_some() {
                return Err("Remote tools are deferred for native Codex".into());
            }
            let directory = context
                .project_directory
                .ok_or("Select a local graph directory")?;
            PendingAgentSubmission {
                delivery_trace: None,
                native_skills: vec![],
                native_apps: vec![],
                scope: Some(submission_scope::SubmissionScope {
                    native_access: self.app_server.access(),
                    native_summary: self.app_server.summary(owner),
                    owner: owner.into(),
                    workspace_root: Some(fs::canonicalize(&directory).map_err(|e| e.to_string())?),
                    native_targets: Default::default(),
                }),
                snapshots: SubmissionSnapshots::default(),
                provider: binding.provider,
                message: String::new(),
                tab_ids: vec![],
                terminal_session_ids: vec![],
                file_ids: vec![],
                selection_override: Some(binding.selection.clone()),
                agent_graph_launch: Some(AgentGraphLaunch {
                    node_key: node.into(),
                    agent_name: binding.name.clone(),
                    project_directory: directory,
                    ssh_profile_id: None,
                    user_request: String::new(),
                }),
                card_draft: false,
                supervision_review: None,
            }
        } else if let Some(pending) = self
            .agent_submission_queue
            .iter()
            .chain(self.pending_agent_submissions.iter())
            .find(|entry| entry.owner() == Some(owner))
        {
            pending.clone()
        } else {
            let run = self.agent_graph_runs.get(node).ok_or(
                "Resolve this node's saved pending input or unfinished checkpoint before sending another request.")?;
            let context = self
                .run_execution_contexts
                .get(&run.run_id)
                .filter(|context| context.conversation_key == owner)
                .ok_or(
                    "The active graph run has no saved execution scope. Your draft was retained.",
                )?;
            PendingAgentSubmission {
                delivery_trace: None,
                native_skills: vec![],
                native_apps: vec![],
                scope: Some(submission_scope::SubmissionScope {
                    native_access: Default::default(),
                    native_summary: None,
                    owner: owner.into(),
                    workspace_root: context.workspace_root.clone(),
                    native_targets: context.native_targets.clone(),
                }),

                snapshots: SubmissionSnapshots::default(),
                provider: run.provider,
                message: String::new(),
                tab_ids: vec![],
                terminal_session_ids: vec![],
                file_ids: vec![],
                selection_override: Some(run.selection.clone()),
                agent_graph_launch: Some(AgentGraphLaunch {
                    node_key: node.into(),
                    agent_name: run.agent_name.clone(),
                    project_directory: run.project_directory.clone(),
                    ssh_profile_id: context.ssh_profile_id.clone(),
                    user_request: String::new(),
                }),
                card_draft: false,
                supervision_review: None,
            }
        };
        let mut submission = renew_graph_submission(submission, message)?;
        if submission.provider != AgentProviderKind::CodexAppServer
            && let Some(context) = self.supervised_chat_context(node)
        {
            submission
                .message
                .push_str(&format!("\n\nObserved worker state:\n{context}"));
        }
        submission.snapshots =
            self.freeze_graph_contexts(owner, &tab_ids, &terminal_session_ids)?;
        submission.snapshots.files = self.graph_files.capture(owner, &file_ids)?;
        if submission.provider == AgentProviderKind::CodexAppServer {
            // The UI node's original spelling, not another selected project or
            // the canonicalized execution root, owns the selection.
            submission.native_skills = self.app_server.capture_skills_for_prompt(
                owner,
                submission
                    .agent_graph_launch
                    .as_ref()
                    .map(|l| Path::new(&l.project_directory)),
                &submission.message,
            )?;
            let thread = self
                .app_server
                .conversations
                .binding(owner)
                .map(|binding| binding.thread_id.clone());
            submission.native_apps = self.app_server.capture_apps(owner, thread.as_deref())?;
        }
        submission.file_ids = file_ids;
        submission.tab_ids = tab_ids;
        submission.terminal_session_ids = terminal_session_ids;
        self.validate_initial_submission(&submission)?;
        self.validate_submission_scope(&submission)?;
        Ok(submission)
    }

    pub(super) fn validate_graph_delivery_scope(
        &self,
        submission: &PendingAgentSubmission,
    ) -> Result<(), String> {
        let launch = submission
            .agent_graph_launch
            .as_ref()
            .ok_or("Missing graph launch.")?;
        let binding = self
            .agent_graph_bindings
            .iter()
            .find(|binding| binding.node_key() == launch.node_key)
            .ok_or("The request's graph agent was removed. It was not sent elsewhere.")?;
        let context = self.agent_graph_node_context(&binding.record_type, &binding.record_id)?;
        if binding.provider != submission.provider
            || context.ssh_profile_id != launch.ssh_profile_id
            || context
                .project_directory
                .as_deref()
                .is_some_and(|directory| directory != launch.project_directory)
        {
            return Err("The graph node's provider, directory or access changed. Review the saved request before sending it again.".into());
        }

        Ok(())
    }

    pub(super) fn clear_agent_graph_queue(
        &mut self,
        record_type: &str,
        id: &str,
        conversation_id: Option<Uuid>,
    ) {
        let key = agent_graph_conversation_key(record_type, id, conversation_id);
        if !self
            .agent_graph_bindings
            .iter()
            .any(|binding| binding.matches_target(record_type, id, conversation_id))
        {
            return;
        }
        let owner = format!("graph:{key}");
        if let Err(error) = self.cancel_submissions_for_owner(&owner, true, true) {
            self.conversation_event(
                "central-agent:conversation-error",
                json!({"owner":owner,"error":error}),
            );
        }
        self.render_agent_graph_surface();
    }
}

fn renew_graph_submission(
    mut submission: PendingAgentSubmission,
    message: String,
) -> Result<PendingAgentSubmission, String> {
    // New input, never a replay of the previous operation, receipt or attachments.
    submission
        .agent_graph_launch
        .as_mut()
        .ok_or("Missing graph launch scope.")?
        .user_request = message.clone();
    submission.message = message;
    submission.snapshots = SubmissionSnapshots::default();
    submission.tab_ids.clear();
    submission.terminal_session_ids.clear();
    submission.file_ids.clear();
    submission.native_skills.clear();
    submission.native_apps.clear();

    Ok(submission)
}
