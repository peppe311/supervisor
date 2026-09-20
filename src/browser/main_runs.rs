//! Main run lifetime is independent from the currently displayed chat.
use super::*;

pub(super) fn parse_control(raw: &str) -> Result<(String, AgentPanelMessage), String> {
    let mut envelope: Value = serde_json::from_str(raw).map_err(|error| error.to_string())?;
    let object = envelope
        .as_object_mut()
        .ok_or("Agent control must be an object.")?;
    let owner = match object.remove("ui_owner") {
        Some(Value::String(owner)) => owner,
        None => String::new(),
        _ => return Err("Invalid agent control owner.".to_owned()),
    };
    // The owner belongs to the trusted envelope, not the deny-unknown-fields
    // action payload. Preserve strict validation of every remaining field.
    let message = serde_json::from_value(envelope).map_err(|error| error.to_string())?;
    Ok((owner, message))
}

pub(super) fn requires_chat_owner(message: &AgentPanelMessage) -> bool {
    matches!(
        message,
        AgentPanelMessage::SubmitChat { .. }
            | AgentPanelMessage::StopAgent
            | AgentPanelMessage::CancelPendingAgentSubmission
            | AgentPanelMessage::ClearAgentSubmissionQueue
            | AgentPanelMessage::SelectChatFiles
            | AgentPanelMessage::RemoveChatFile { .. }
            | AgentPanelMessage::CaptureTabContext { .. }
            | AgentPanelMessage::RefreshTabContext { .. }
            | AgentPanelMessage::RemoveTabContext { .. }
            | AgentPanelMessage::CaptureTerminalContext { .. }
            | AgentPanelMessage::RefreshTerminalContext { .. }
            | AgentPanelMessage::RemoveTerminalContext { .. }
            | AgentPanelMessage::SetTerminalContextFollow { .. }
            | AgentPanelMessage::SetAgentProvider { .. }
            | AgentPanelMessage::SetAgentConfiguration { .. }
            | AgentPanelMessage::ResetClaudeSession
    )
}

#[derive(Default)]
pub(super) struct MainRuns {
    latest: HashMap<String, u64>,
    runs: HashMap<u64, AgentRun>,
}

impl MainRuns {
    pub fn insert(&mut self, owner: String, run: AgentRun) {
        self.latest.insert(owner, run.id);
        self.runs.insert(run.id, run);
    }

    pub fn latest(&self, owner: &str) -> Option<&AgentRun> {
        self.latest.get(owner).and_then(|id| self.runs.get(id))
    }

    pub fn get(&self, id: u64) -> Option<&AgentRun> {
        self.runs.get(&id)
    }
    pub fn get_mut(&mut self, id: u64) -> Option<&mut AgentRun> {
        self.runs.get_mut(&id)
    }
    pub fn values(&self) -> impl Iterator<Item = &AgentRun> {
        self.runs.values()
    }
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut AgentRun> {
        self.runs.values_mut()
    }
}

impl BrowserApp {
    pub(super) fn active_main_busy(&self) -> bool {
        self.owner_has_running_work(&self.conversation_key(None))
    }

    pub(super) fn chat_has_unfinished_work(&self, chat_id: &str) -> bool {
        self.owner_has_unfinished_work(&format!("chat:{chat_id}"))
    }

    pub(super) fn owner_has_unfinished_work(&self, owner: &str) -> bool {
        self.owner_has_running_work(owner)
            || self.run_execution_contexts.iter().any(|(id, context)| {
                context.conversation_key == owner
                    && (self.time_machine_run_roots.contains_key(id)
                        || self.pending_run_finalizations.contains(id)
                        || self.processes.has_active_owner(*id))
            })
    }

    pub(super) fn chat_mutation_available(&mut self, chat_id: &str) -> bool {
        if !self.chat_has_unfinished_work(chat_id) {
            return true;
        }
        self.push_chat_message(ChatRole::System,
            "This chat still owns active work or an unfinished checkpoint. Open it and stop the run before moving, archiving or deleting it.".to_owned(), vec![]);
        self.render_agent_panel();
        false
    }

    pub(super) fn project_mutation_available(&mut self, root: &str) -> bool {
        if !self.workspace_has_active_agent(root)
            && self.workspace_checkpoint_run_ids(root).is_empty()
        {
            return true;
        }
        self.push_chat_message(ChatRole::System,
            "This project still owns active work or an unfinished checkpoint. Stop its agents before archiving it or its chats.".to_owned(), vec![]);
        self.render_agent_panel();
        false
    }

    pub(super) fn active_main_run(&self) -> Option<&AgentRun> {
        self.main_runs.latest(&self.conversation_key(None))
    }

    pub(super) fn active_main_run_mut(&mut self) -> Option<&mut AgentRun> {
        let id = self.active_main_run()?.id;
        self.main_runs.get_mut(id)
    }

    pub(super) fn main_agent_can_resume(&self) -> bool {
        let owner = self.conversation_key(None);
        self.agent_can_resume_for_owner(&owner, self.agent_provider)
    }

    pub(super) fn agent_can_resume_for_owner(
        &self,
        owner: &str,
        provider: AgentProviderKind,
    ) -> bool {
        if self.owner_has_running_work(owner)
            || self
                .pending_agent_submissions
                .iter()
                .any(|entry| entry.owner() == Some(owner))
            || self
                .agent_submission_queue
                .iter()
                .any(|entry| entry.owner() == Some(owner))
        {
            return false;
        }
        if provider == AgentProviderKind::CodexAppServer {
            let Some(binding) = self.app_server.conversations.binding(owner) else {
                return false;
            };
            let interrupted = self
                .app_server
                .conversations
                .mirror
                .thread(&binding.thread_id)
                .and_then(|thread| thread.turns.last())
                .is_some_and(|turn| turn.status == "interrupted");
            return interrupted
                && !binding.archived
                && !binding.deleted
                && self.app_server.is_connected()
                && self.app_server.delivery_warning(owner).is_none()
                && !self.app_server.busy(owner);
        }
        self.main_runs.latest(owner).is_some_and(|run| {
            run.phase == AgentPhase::Stopped
                && self
                    .run_execution_contexts
                    .get(&run.id)
                    .is_some_and(|context| context.provider == provider)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(id: u64) -> AgentRun {
        AgentRun::planning_for_provider(id, format!("Request {id}"), "Agent")
    }

    #[test]
    fn selecting_other_chats_does_not_drop_the_running_turn() {
        let mut runs = MainRuns::default();
        runs.insert("chat:a".to_owned(), run(1));
        runs.insert("chat:b".to_owned(), run(2));
        assert_eq!(runs.latest("chat:b").unwrap().id, 2);
        assert!(runs.get(1).unwrap().is_active());
        assert_eq!(runs.latest("chat:a").unwrap().id, 1);
        assert!(runs.latest("chat:empty").is_none());
    }

    #[test]
    fn targeted_stop_and_late_updates_cannot_modify_another_run() {
        let mut runs = MainRuns::default();
        runs.insert("chat:a".to_owned(), run(1));
        runs.insert("chat:b".to_owned(), run(2));
        runs.get_mut(1).unwrap().phase = AgentPhase::Stopped;
        runs.get_mut(1).unwrap().status = "Cancellation requested".to_owned();
        assert!(runs.get(2).unwrap().is_active());
        assert!(!runs.get(1).unwrap().is_active());
        runs.insert("chat:a".to_owned(), run(3));
        runs.get_mut(1).unwrap().status = "Previous turn ended".to_owned();
        assert_eq!(runs.latest("chat:a").unwrap().id, 3);
        assert!(runs.latest("chat:a").unwrap().is_active());
    }

    #[test]
    fn stale_main_controls_require_their_view_owner_but_explicit_navigation_does_not() {
        for action in [
            "stop_agent",
            "cancel_pending_agent_submission",
            "clear_agent_submission_queue",
            "select_chat_files",
        ] {
            let message: AgentPanelMessage =
                serde_json::from_value(json!({"message_type": action})).unwrap();
            assert!(requires_chat_owner(&message), "{action}");
        }
        let navigation: AgentPanelMessage = serde_json::from_value(
            json!({"message_type": "activate_project_chat", "chat_id": "b"}),
        )
        .unwrap();
        assert!(!requires_chat_owner(&navigation));
        let response: AgentPanelMessage = serde_json::from_value(
            json!({"message_type": "approve_action", "request_id": "a-request"}),
        )
        .unwrap();
        assert!(!requires_chat_owner(&response));
    }

    #[test]
    fn actual_ui_envelope_preserves_owner_without_breaking_strict_action_parsing() {
        let (owner, action) =
            parse_control(r#"{"message_type":"stop_agent","ui_owner":"chat:a"}"#).unwrap();
        assert_eq!(owner, "chat:a");
        assert!(requires_chat_owner(&action));
        assert!(
            parse_control(
                r#"{"message_type":"codex_history","ui_owner":"chat:a","owner":"chat:a"}"#
            )
            .is_err()
        );
        assert!(
            parse_control(r#"{"message_type":"codex_mode","ui_owner":"chat:a","owner":"chat:a","mode":"default","elevate":true}"#)
                .is_err()
        );
        assert!(parse_control(r#"{"message_type":"stop_agent","ui_owner":42}"#).is_err());
    }
}
