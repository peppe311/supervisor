//! Accepted input retains its destination while queued or awaiting recovery.
use super::*;
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(super) struct SubmissionSnapshots {
    pub tabs: Vec<TabContextSnapshot>,
    pub terminals: Vec<DraftTerminalContext>,
    pub files: Vec<PendingFileAttachment>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SubmissionScope {
    #[serde(default)]
    pub native_access: central_agent_codex_runtime::api::Access,
    #[serde(default)]
    pub native_summary: Option<central_agent_codex_runtime::api::ReasoningSummary>,
    pub owner: String,
    pub workspace_root: Option<PathBuf>,
    #[serde(default)]
    pub native_targets: native_targets::NativeTargets,
}

impl PendingAgentSubmission {
    pub(super) fn has_input(&self) -> bool {
        !self.message.trim().is_empty()
            || !self.tab_ids.is_empty()
            || !self.terminal_session_ids.is_empty()
            || !self.file_ids.is_empty()
    }
    pub(super) fn owner(&self) -> Option<&str> {
        self.scope.as_ref().map(|scope| scope.owner.as_str())
    }
    pub(super) fn workspace_root(&self) -> Option<&Path> {
        self.scope
            .as_ref()
            .and_then(|scope| scope.workspace_root.as_deref())
    }
}

pub(super) fn next_ready_index(
    queue: &VecDeque<PendingAgentSubmission>,
    mut owner_ready: impl FnMut(&str) -> bool,
) -> Option<usize> {
    queue
        .iter()
        .position(|submission| submission.owner().is_some_and(&mut owner_ready))
}

impl BrowserApp {
    pub(super) fn accept_submission(&mut self, submission: &mut PendingAgentSubmission) -> bool {
        match self
            .freeze_submission_scope(submission)
            .and_then(|_| self.validate_submission_scope(submission))
        {
            Ok(()) => true,
            Err(error) => {
                self.push_submission_message(submission, ChatRole::System, error);
                self.render_agent_panel();
                false
            }
        }
    }
    pub(super) fn validate_initial_submission(
        &self,
        submission: &PendingAgentSubmission,
    ) -> Result<(), String> {
        let submission_root = submission
            .scope
            .as_ref()
            .and_then(|scope| scope.workspace_root.as_deref())
            .or_else(|| self.workspace.root());
        if submission.agent_graph_launch.is_none()
            && submission_root.is_some_and(|root| {
                self.pending_workspace_ejections
                    .contains(&root.display().to_string())
            })
        {
            return Err(
                "This project is being removed. Cancel removal before starting new work."
                    .to_owned(),
            );
        }
        if !submission.has_input() {
            return Err("The request cannot be empty.".to_owned());
        }
        if submission.provider != AgentProviderKind::CodexAppServer
            && !submission.file_ids.is_empty()
        {
            return Err("File attachments are not supported by the remaining provider integrations in this build. Open the file in the workspace instead.".to_owned());
        }
        let (provider, models, selection) = self.provider_configuration(submission.provider);
        if !provider.can_run() {
            return Err(format!(
                "{} is not ready: {} Open Settings → AI provider to reconnect or retry.",
                submission.provider.label(),
                provider.detail
            ));
        }
        let model = provider_types::validate_selection(
            models,
            submission.selection_override.as_ref().unwrap_or(selection),
        )?;
        if submission.tab_ids.len() > MAX_CHAT_ATTACHMENTS
            || submission.terminal_session_ids.len() > MAX_TERMINAL_CHAT_ATTACHMENTS
            || submission.file_ids.len() > MAX_FILE_ATTACHMENTS
        {
            return Err(
                "The request has too many attachments. Remove an attachment before sending."
                    .to_owned(),
            );
        }
        for id in &submission.tab_ids {
            let context = submission
                .snapshots
                .tabs
                .iter()
                .find(|tab| tab.tab_id == *id)
                .ok_or("An attached tab is unavailable in this draft.")?;
            if !context.is_ready() {
                return Err("Wait for the tab capture to finish, or refresh/remove the failed capture before sending.".to_owned());
            }
            let revision = self
                .tabs
                .iter()
                .find(|tab| tab.id == *id)
                .map(|tab| tab.content_revision);
            if context.is_stale(revision, unix_time_ms()) {
                return Err(
                    "A snapshot expired or its page changed. Refresh it before sending.".to_owned(),
                );
            }
        }
        for id in &submission.terminal_session_ids {
            if !submission
                .snapshots
                .terminals
                .iter()
                .any(|terminal| terminal.snapshot.session_id == *id)
            {
                return Err("An attached shell is unavailable in this draft.".to_owned());
            }
        }
        let mut file_ids = HashSet::new();
        for id in &submission.file_ids {
            if !file_ids.insert(id) {
                return Err(
                    "A file attachment was selected twice. Review this draft before sending."
                        .into(),
                );
            }
            let file = submission
                .snapshots
                .files
                .iter()
                .find(|file| file.id() == id)
                .ok_or("An attached file is unavailable in this draft.")?;

            if file.kind() == FileAttachmentKind::Image && !model.supports_image_input() {
                return Err("This model does not accept images. Select an image-capable model or remove the image.".to_owned());
            }
            if file.kind() == FileAttachmentKind::Audio && !model.supports_audio_input() {
                return Err("This model does not accept audio. Select an audio-capable model or remove the audio file.".to_owned());
            }
        }
        Ok(())
    }

    pub(super) fn freeze_submission_scope(
        &mut self,
        submission: &mut PendingAgentSubmission,
    ) -> Result<(), String> {
        if submission.scope.is_some() {
            return Ok(());
        }
        if submission.scope.is_none() {
            if let Some(launch) = &submission.agent_graph_launch {
                let owner = format!("graph:{}", launch.node_key);
                submission.snapshots = self.freeze_graph_contexts(
                    &owner,
                    &submission.tab_ids,
                    &submission.terminal_session_ids,
                )?;
                submission.snapshots.files =
                    self.graph_files.capture(&owner, &submission.file_ids)?;
            } else {
                let owner = self.composer_owner();
                let contexts = self.freeze_native_contexts(
                    &owner,
                    &submission.tab_ids,
                    &submission.terminal_session_ids,
                )?;
                submission.snapshots = SubmissionSnapshots {
                    tabs: contexts.tabs,
                    terminals: contexts.terminals,
                    files: self
                        .draft_file_attachments
                        .iter()
                        .filter(|file| submission.file_ids.iter().any(|id| id == file.id()))
                        .cloned()
                        .collect(),
                };
            }
        }
        if submission.selection_override.is_none() {
            submission.selection_override =
                Some(self.provider_configuration(submission.provider).2.clone());
        }
        if submission.scope.is_none() {
            self.validate_initial_submission(submission)?;
        }
        let automatic_review = submission
            .supervision_review
            .as_ref()
            .is_some_and(|review| review.automatic);
        let native_access = supervision::review_access(
            submission.supervision_review.as_ref(),
            self.app_server.access(),
        );
        let skill_owner = if let Some(launch) = &submission.agent_graph_launch {
            format!("graph:{}", launch.node_key)
        } else {
            self.composer_owner()
        };
        let native_summary = self.app_server.summary(&skill_owner);
        if submission.provider == AgentProviderKind::CodexAppServer && !automatic_review {
            let directory = submission_checkpoint_root(
                self.workspace.root(),
                submission.agent_graph_launch.as_ref(),
            );
            let fallback = self.data_dir.join("codex-workspace");
            submission.native_skills = self.app_server.capture_skills_for_prompt(
                &skill_owner,
                Some(directory.as_deref().unwrap_or(&fallback)),
                &submission.message,
            )?;
            let thread = self
                .app_server
                .conversations
                .binding(&skill_owner)
                .map(|binding| binding.thread_id.clone());
            submission.native_apps = self
                .app_server
                .capture_apps(&skill_owner, thread.as_deref())?;
        }
        if submission.agent_graph_launch.is_none() {
            self.ensure_active_project_chat();
        }
        let owner = self.conversation_key(
            submission
                .agent_graph_launch
                .as_ref()
                .map(|launch| launch.node_key.as_str()),
        );

        submission.scope = Some(SubmissionScope {
            native_access,
            native_summary,
            native_targets: self.capture_native_targets(),

            owner,
            workspace_root: submission_checkpoint_root(
                self.workspace.root(),
                submission.agent_graph_launch.as_ref(),
            ),
        });
        if submission.provider == AgentProviderKind::CodexAppServer {
            if !automatic_review {
                self.app_server
                    .promote_skills(&skill_owner, submission.owner().unwrap());
                self.emit_app_server_skills(submission.owner().unwrap(), None);
            }
            self.app_server
                .inherit_summary(submission.owner().unwrap().into(), native_summary);
        }
        Ok(())
    }

    fn capture_main_contexts(
        &self,
        owner: &str,
        tab_ids: &[u64],
        terminal_ids: &[u64],
    ) -> Result<SubmissionSnapshots, String> {
        if owner != self.main_draft_owner || owner != self.composer_owner() {
            return Err(
                "The selected conversation changed. Reopen the intended draft before sending."
                    .into(),
            );
        }
        if tab_ids.len() > MAX_CHAT_ATTACHMENTS
            || terminal_ids.len() > MAX_TERMINAL_CHAT_ATTACHMENTS
        {
            return Err("Too many context attachments for one request.".into());
        }
        let mut snapshots = SubmissionSnapshots::default();
        let mut seen_tabs = HashSet::new();
        for id in tab_ids {
            if !seen_tabs.insert(*id) {
                return Err("A browser tab snapshot was selected twice.".into());
            }
            snapshots.tabs.push(
                self.draft_contexts
                    .iter()
                    .find(|context| context.tab_id == *id)
                    .ok_or("This browser tab no longer belongs to the current draft. Reattach it.")?
                    .clone(),
            );
        }
        let mut seen_terminals = HashSet::new();
        for id in terminal_ids {
            if !seen_terminals.insert(*id) {
                return Err("A terminal output snapshot was selected twice.".into());
            }
            snapshots.terminals.push(
                self.draft_terminal_contexts
                    .iter()
                    .find(|context| context.snapshot.session_id == *id)
                    .ok_or(
                        "This terminal output no longer belongs to the current draft. Reattach it.",
                    )?
                    .clone(),
            );
        }
        Ok(snapshots)
    }

    pub(super) fn freeze_native_contexts(
        &mut self,
        owner: &str,
        tab_ids: &[u64],
        terminal_ids: &[u64],
    ) -> Result<SubmissionSnapshots, String> {
        self.conversation_target(owner)?;
        let snapshots = if owner.starts_with("graph:") || self.is_project_chat_card_owner(owner) {
            self.freeze_graph_contexts(owner, tab_ids, terminal_ids)?
        } else {
            // Validate ownership before refreshing any follow-live terminal. The
            // second capture freezes the exact bytes used by start, queue or steer.
            self.capture_main_contexts(owner, tab_ids, terminal_ids)?;
            for id in terminal_ids {
                self.refresh_following_terminal_context(*id, true);
            }
            self.capture_main_contexts(owner, tab_ids, terminal_ids)?
        };
        for snapshot in &snapshots.tabs {
            if !snapshot.is_ready() {
                return Err(
                    "Wait for the browser tab snapshot to finish, or remove it before sending."
                        .into(),
                );
            }
            let revision = self
                .tabs
                .iter()
                .find(|tab| tab.id == snapshot.tab_id)
                .map(|tab| tab.content_revision);
            if snapshot.is_stale(revision, unix_time_ms()) {
                return Err(
                    "A browser tab snapshot expired or its page changed. Refresh it before sending."
                        .into(),
                );
            }
        }
        Ok(snapshots)
    }

    pub(super) fn validate_submission_scope(
        &self,
        submission: &PendingAgentSubmission,
    ) -> Result<(), String> {
        let scope = submission
            .scope
            .as_ref()
            .ok_or("The request has no conversation owner.")?;
        let expected_access = supervision::review_access(
            submission.supervision_review.as_ref(),
            self.app_server.access(),
        );
        if submission.provider == AgentProviderKind::CodexAppServer
            && scope.native_access != expected_access
        {
            return Err("Shared Codex permissions changed. Review this prompt and its access profile before resubmitting.".into());
        }
        if let Some(launch) = &submission.agent_graph_launch {
            if scope.owner != format!("graph:{}", launch.node_key) {
                return Err("The queued request belongs to a different graph node.".to_owned());
            }
            self.validate_graph_delivery_scope(submission)?;
        } else {
            let chat_id = scope
                .owner
                .strip_prefix("chat:")
                .ok_or("The request has an invalid chat owner.")?;
            let chat = self
                .project_chats
                .iter()
                .find(|chat| chat.id == chat_id && !chat.archived)
                .ok_or(
                    "The request's chat is unavailable or archived. It was not sent elsewhere.",
                )?;
            let current_root =
                (!chat.project_root.is_empty()).then(|| PathBuf::from(&chat.project_root));
            if self
                .pending_workspace_ejections
                .contains(&chat.project_root)
            {
                return Err("This project is being removed. Cancel removal or choose another project before sending a request.".to_owned());
            }
            if scope.workspace_root != current_root {
                return Err("The queued request's project changed. Review its original scope before sending it again.".to_owned());
            }
        }
        Ok(())
    }

    pub(super) fn run_for_owner(&self, owner: &str) -> Option<&AgentRun> {
        self.main_runs.latest(owner).or_else(|| {
            owner
                .strip_prefix("graph:")
                .and_then(|node| self.agent_graph_runs.get(node))
                .map(|run| &run.runtime)
        })
    }

    pub(super) fn owner_has_running_work(&self, owner: &str) -> bool {
        self.app_server.busy(owner)
            || self.run_for_owner(owner).is_some_and(|run| {
                run.is_active()
                    || self.run_has_provider_job(run.id)
                    || run
                        .active_request_id
                        .as_deref()
                        .is_some_and(|id| self.pending_commands.contains_key(id))
            })
    }

    pub(super) fn owner_is_waiting_for_recovery(&self, owner: &str) -> bool {
        self.pending_agent_submissions
            .iter()
            .any(|submission| submission.owner() == Some(owner))
    }

    pub(super) fn active_submission_queue_count(&self) -> usize {
        let owner = self.conversation_key(None);
        self.agent_submission_queue
            .iter()
            .filter(|submission| submission.owner() == Some(owner.as_str()))
            .count()
    }

    pub(super) fn active_pending_submission(&self) -> Option<&PendingAgentSubmission> {
        let owner = self.conversation_key(None);
        self.pending_agent_submissions
            .iter()
            .find(|submission| submission.owner() == Some(owner.as_str()))
    }

    pub(super) fn submission_recovery_blocked(&self, submission: &PendingAgentSubmission) -> bool {
        submission.workspace_root().is_some_and(|root| {
            self.time_machine_run_roots.iter().any(|(id, active)| {
                checkpoint_roots_overlap(active, root)
                    && (self.retryable_time_machine_finalizations.contains(id)
                        || self.time_machine_finalizations_in_flight.contains(id))
            })
        })
    }

    pub(super) fn push_submission_message(
        &mut self,
        submission: &PendingAgentSubmission,
        role: ChatRole,
        text: String,
    ) {
        if let Some(launch) = &submission.agent_graph_launch {
            self.conversation_event(
                "central-agent:conversation-error",
                json!({"owner":format!("graph:{}", launch.node_key),"error":text}),
            );
            return;
        }
        self.push_chat_message(role, text, vec![]);
        if let Some(chat_id) = submission
            .owner()
            .and_then(|owner| owner.strip_prefix("chat:"))
            && let Some(row) = self.chat_messages.back()
        {
            self.chat_ownership.claim(row.id, chat_id);
        }
    }

    pub(super) fn cancel_submissions_for_owner(
        &mut self,
        owner: &str,
        ready: bool,
        recovery: bool,
    ) -> Result<(), String> {
        if ready {
            self.agent_submission_queue
                .retain(|submission| submission.owner() != Some(owner));
        }
        if recovery {
            self.pending_agent_submissions
                .retain(|submission| submission.owner() != Some(owner));
        }
        Ok(())
    }

    pub(super) fn cancel_submissions_for_workspace(&mut self, root: &str) -> Result<(), String> {
        let owners = self
            .project_chats
            .iter()
            .filter(|chat| chat.project_root == root)
            .map(|chat| format!("chat:{}", chat.id))
            .collect::<Vec<_>>();
        for owner in owners {
            self.cancel_submissions_for_owner(&owner, true, true)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_summary_selection_survives_snapshot_roundtrip_without_cross_owner_changes() {
        let mut first = queued("chat:a", "first");
        first.scope.as_mut().unwrap().native_summary =
            Some(central_agent_codex_runtime::api::ReasoningSummary::Detailed);
        let sibling = queued("graph:b", "second");
        let frozen: SubmissionScope =
            serde_json::from_value(serde_json::to_value(first.scope.as_ref().unwrap()).unwrap())
                .unwrap();
        assert_eq!(
            frozen.native_summary,
            Some(central_agent_codex_runtime::api::ReasoningSummary::Detailed)
        );
        assert_eq!(sibling.scope.as_ref().unwrap().native_summary, None);
        assert_eq!(frozen.owner, "chat:a");
    }
    fn queued(owner: &str, text: &str) -> PendingAgentSubmission {
        PendingAgentSubmission {
            delivery_trace: None,
            native_skills: vec![],
            native_apps: vec![],
            scope: Some(SubmissionScope {
                native_access: Default::default(),
                native_summary: None,
                owner: owner.to_owned(),
                workspace_root: None,
                native_targets: native_targets::NativeTargets::default(),
            }),

            snapshots: SubmissionSnapshots::default(),
            provider: AgentProviderKind::ClaudeCode,
            message: text.to_owned(),
            tab_ids: vec![],
            terminal_session_ids: vec![],
            file_ids: vec![],
            selection_override: None,
            agent_graph_launch: None,
            card_draft: false,
            supervision_review: None,
        }
    }
    #[test]
    fn a_busy_chat_does_not_block_another_chat_and_each_owner_remains_fifo() {
        let mut queue = VecDeque::from([
            queued("chat:a", "a1"),
            queued("chat:b", "b1"),
            queued("chat:a", "a2"),
            queued("chat:b", "b2"),
        ]);
        let index = next_ready_index(&queue, |owner| owner != "chat:a").unwrap();
        assert_eq!(queue.remove(index).unwrap().message, "b1");
        let index = next_ready_index(&queue, |owner| owner == "chat:a").unwrap();
        assert_eq!(queue.remove(index).unwrap().message, "a1");
        assert_eq!(
            queue
                .iter()
                .map(|item| item.message.as_str())
                .collect::<Vec<_>>(),
            ["a2", "b2"]
        );
    }

    #[test]
    fn attachment_only_input_is_not_confused_with_an_empty_prompt() {
        let empty = queued("chat:a", "  ");
        assert!(!empty.has_input());
        let mut tab = empty.clone();
        tab.tab_ids.push(1);
        assert!(tab.has_input());
        let mut terminal = empty.clone();
        terminal.terminal_session_ids.push(2);
        assert!(terminal.has_input());
        let mut file = empty;
        file.file_ids.push("file".into());
        assert!(file.has_input());
        // has_input is not attachment validation: the normal scope gate must
        // still resolve each ID to a ready snapshot owned by this draft.
        assert!(file.snapshots.files.is_empty());
    }
    #[test]
    fn graph_queue_is_fifo_and_independent_of_main_and_other_nodes() {
        let mut queue = VecDeque::from([
            queued("graph:entity:a", "a1"),
            queued("chat:main", "main"),
            queued("graph:entity:b", "b1"),
            queued("graph:entity:a", "a2"),
        ]);
        let index = next_ready_index(&queue, |owner| owner == "graph:entity:b").unwrap();
        assert_eq!(queue.remove(index).unwrap().message, "b1");
        assert!(next_ready_index(&queue, |_| false).is_none());
        let index = next_ready_index(&queue, |owner| owner == "graph:entity:a").unwrap();
        assert_eq!(queue.remove(index).unwrap().message, "a1");
        queue.retain(|entry| entry.owner() != Some("graph:entity:a"));
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].owner(), Some("chat:main"));
    }
    #[test]
    fn queued_scope_survives_serialization_without_rebinding() {
        let mut submission = queued("chat:a", "Continue");
        submission.scope.as_mut().unwrap().workspace_root = Some(PathBuf::from("C:/project-a"));
        let json = serde_json::to_string(&submission).unwrap();
        let restored: PendingAgentSubmission = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.owner(), Some("chat:a"));
        assert_eq!(restored.workspace_root(), Some(Path::new("C:/project-a")));
    }

    #[test]
    fn unowned_legacy_requests_are_not_automatically_scheduled() {
        let mut submission = queued("chat:a", "Legacy");
        submission.scope = None;
        assert!(next_ready_index(&VecDeque::from([submission]), |_| true).is_none());
    }

    #[test]
    fn queued_attachments_are_a_snapshot_not_an_alias_to_another_draft() {
        let mut queued = queued("chat:a", "Read my attachment");
        queued.file_ids.push("file-a".to_owned());
        let mut later_draft = queued.clone();
        later_draft.file_ids.clear();
        later_draft.message = "Different request".to_owned();
        assert_eq!(queued.file_ids, ["file-a"]);
        assert_eq!(queued.message, "Read my attachment");
    }
}
