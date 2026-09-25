//! Navigation for the project board. Existing chat/card owners stay canonical.
use super::*;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Action {
    Work {
        request_id: String,
        action: work_results::Action,
    },
    CreateWorktree {
        root: String,
        name: String,
    },
    Select {
        source: String,
        id: String,
    },
    PreviewChat {
        chat_id: String,
    },
    CloseChatPreview {
        chat_id: String,
    },
    SetChatProfile {
        chat_id: String,
        provider: AgentProviderKind,
        model: String,
        effort: String,
        #[serde(default)]
        service_tier: Option<String>,
        #[serde(default)]
        context_window: Option<u64>,
    },
    SubmitChat {
        chat_id: String,
        message: String,
        #[serde(default)]
        delivery: AgentSubmissionDelivery,
        #[serde(default)]
        resume: bool,
        #[serde(default)]
        file_ids: Vec<String>,
        #[serde(default)]
        tab_ids: Vec<u64>,
        #[serde(default)]
        terminal_session_ids: Vec<u64>,
        #[serde(default)]
        timing: Option<app_server::delivery::ClientTiming>,
    },
    SteerSupervised {
        node_key: String,
        expected_turn_id: String,
        message: String,
        #[serde(default)]
        timing: Option<app_server::delivery::ClientTiming>,
    },
    StopChat {
        chat_id: String,
    },
    ClearChatQueue {
        chat_id: String,
    },
    NewChat {
        root: String,
    },
    OpenFile {
        root: String,
        path: String,
    },
    NewAgent {
        node_key: String,
        #[serde(default)]
        chat_id: Option<String>,
    },
    LinkChat {
        node_key: String,
        #[serde(default)]
        chat_id: Option<String>,
    },
    RenameAgent {
        node_key: String,
        name: String,
    },
    DeleteAgent {
        node_key: String,
    },
}

pub(super) fn chat_in_project(
    chats: &[ProjectChat],
    id: &str,
    directory: Option<&str>,
    remote: bool,
) -> bool {
    !remote
        && chats.iter().any(|chat| {
            chat.id == id
                && !chat.archived
                && directory.is_some_and(|root| Path::new(root) == Path::new(&chat.project_root))
        })
}

impl BrowserApp {
    pub(super) fn handle_project_board(&mut self, action: Action) {
        if let Action::Work { request_id, action } = action {
            self.handle_work_results(&request_id, action);
            return;
        }
        let card_owner = match &action {
            Action::SetChatProfile { chat_id, .. }
            | Action::SubmitChat { chat_id, .. }
            | Action::StopChat { chat_id }
            | Action::ClearChatQueue { chat_id } => Some(format!("chat:{chat_id}")),
            Action::SteerSupervised { node_key, .. } => Some(format!("graph:{node_key}")),
            _ => None,
        };
        let result = self.apply_project_board(action);
        if let Err(error) = result {
            if let Some(owner) = card_owner {
                self.conversation_event(
                    "central-agent:conversation-error",
                    json!({"owner":owner,"error":error.clone()}),
                );
            }
            self.conversation_event(
                "central-agent:project-board-error",
                json!({"owner":"graph:project-board","error":error}),
            );
        }
        self.render_agent_graph_surface();
    }

    fn apply_project_board(&mut self, action: Action) -> Result<(), String> {
        match action {
            Action::Work { .. } => unreachable!("work actions are handled separately"),
            Action::CreateWorktree { root, name } => self.create_board_worktree(&root, &name)?,
            Action::Select { source, id } => {
                if source == "local" {
                    if !self
                        .workspace
                        .project_roots()
                        .iter()
                        .any(|root| root == Path::new(&id))
                    {
                        return Err("Reconnect this project before opening it.".into());
                    }
                    self.activate_workspace(&id);
                } else if source == "ssh" {
                    let project = self
                        .remote_projects
                        .iter_mut()
                        .find(|project| project.id == id)
                        .ok_or("This SSH project is no longer connected.")?;
                    project.last_opened_at_ms = project_timestamp_ms();
                    self.save_session();
                } else {
                    return Err("Unknown project source.".into());
                }
                self.project_board_open_chat_ids.clear();
            }
            Action::PreviewChat { chat_id } => {
                let chat = self
                    .project_chats
                    .iter()
                    .find(|chat| chat.id == chat_id && !chat.archived)
                    .cloned()
                    .ok_or("This conversation is no longer available.")?;
                if !self
                    .workspace
                    .project_roots()
                    .iter()
                    .any(|root| root == Path::new(&chat.project_root))
                {
                    return Err("Reconnect this project before opening its conversation.".into());
                }
                self.chat_ownership.load(&chat, &mut self.chat_messages);
                if !self.project_board_open_chat_ids.contains(&chat_id) {
                    self.project_board_open_chat_ids.push(chat_id.clone());
                }
                if !self.project_chat_card_profiles.contains_key(&chat_id) {
                    let provider = self.agent_provider;
                    let selection = self.provider_configuration(provider).2.clone();
                    self.project_chat_card_profiles.insert(
                        chat_id,
                        ProjectChatCardProfile {
                            provider,
                            selection,
                        },
                    );
                }
            }
            Action::CloseChatPreview { chat_id } => {
                self.project_board_open_chat_ids.retain(|id| id != &chat_id);
            }
            Action::SetChatProfile {
                chat_id,
                provider,
                model,
                effort,
                service_tier,
                context_window,
            } => {
                let owner = self.project_chat_card_owner(&chat_id)?;
                if self.conversation_busy(&owner) {
                    return Err(
                        "Stop this conversation before changing its provider or model.".into(),
                    );
                }
                let selection = AgentSelection {
                    model: model.trim().to_owned(),
                    effort: effort.trim().to_owned(),
                    service_tier: service_tier
                        .map(|tier| tier.trim().to_owned())
                        .filter(|tier| !tier.is_empty()),
                    context_window,
                    personality: None,
                };
                provider_types::validate_selection(
                    self.provider_configuration(provider).1,
                    &selection,
                )?;
                self.project_chat_card_profiles.insert(
                    chat_id,
                    ProjectChatCardProfile {
                        provider,
                        selection,
                    },
                );
            }
            Action::SubmitChat {
                chat_id,
                message,
                delivery,
                resume,
                file_ids,
                tab_ids,
                terminal_session_ids,
                timing,
            } => {
                self.submit_project_chat_card(
                    &chat_id,
                    message,
                    delivery,
                    resume,
                    file_ids,
                    tab_ids,
                    terminal_session_ids,
                    timing,
                )?;
            }
            Action::SteerSupervised {
                node_key,
                expected_turn_id,
                message,
                timing,
            } => {
                self.steer_supervised_chat(&node_key, expected_turn_id, message, timing)?;
            }
            Action::StopChat { chat_id } => {
                self.stop_project_chat_card(&chat_id)?;
            }
            Action::ClearChatQueue { chat_id } => {
                let owner = self.project_chat_card_owner(&chat_id)?;
                self.cancel_submissions_for_owner(&owner, true, false)?;
            }
            Action::NewChat { root } => {
                if !self
                    .workspace
                    .project_roots()
                    .iter()
                    .any(|path| path == Path::new(&root))
                {
                    return Err("Select a connected local project first.".into());
                }
                self.create_project_chat(&root);
                if let Some(chat_id) = self.active_project_chat_id.clone() {
                    self.apply_project_board(Action::PreviewChat { chat_id })?;
                }
            }
            Action::OpenFile { root, path } => {
                let workspace = self.editor_workspace(&root)?;
                self.file_editor.open(&workspace, &path)?;
                self.activate_workspace(&root);
                self.render_file_editor();
                self.close_agent_graph();
            }
            Action::NewAgent { node_key, chat_id } => {
                if self.agent_graph_bindings.len() >= MAX_AGENT_GRAPH_BINDINGS {
                    return Err("The project agent limit has been reached.".into());
                }
                let project = self
                    .project_registry_view()
                    .projects
                    .into_iter()
                    .find(|project| project.node_key == node_key)
                    .ok_or("Select a connected project first.")?;
                let (record_type, record_id) = node_key
                    .split_once(':')
                    .ok_or("Invalid project identity.")?;
                let context = self.agent_graph_node_context(record_type, record_id)?;
                if chat_id.as_ref().is_some_and(|id| {
                    !chat_in_project(
                        &self.project_chats,
                        id,
                        context.project_directory.as_deref(),
                        context.ssh_profile_id.is_some(),
                    )
                }) {
                    return Err("An agent can be associated only with a conversation in its own local project.".into());
                }
                let provider = if context.ssh_profile_id.is_some() {
                    AgentProviderKind::ClaudeCode
                } else {
                    self.agent_provider
                };
                let (_, _, selection) = self.provider_configuration(provider);
                if selection.model.is_empty() || selection.effort.is_empty() {
                    return Err("Connect a provider in Settings before adding a supervisor.".into());
                }
                let binding = AgentGraphBinding {
                    record_type: record_type.into(),
                    record_id: record_id.into(),
                    conversation_id: Some(Uuid::new_v4()),
                    project_chat_id: chat_id,
                    project_directory: context.project_directory,
                    ssh_profile_id: context.ssh_profile_id,
                    name: format!("Supervisor {}", project.agent_count + 1),
                    mission: "Coordinate, verify and complete work in this project.".into(),
                    provider,
                    selection: selection.clone(),
                };
                let key = binding.node_key();
                self.agent_graph_bindings.push(binding);
                self.save_session();
                self.render_agent_graph_surface();
                self.conversation_event(
                    "central-agent:project-board-open-agent",
                    json!({"owner":"graph:project-board","nodeKey":key}),
                );
            }
            Action::LinkChat { node_key, chat_id } => {
                let binding = self
                    .agent_graph_bindings
                    .iter()
                    .find(|binding| binding.node_key() == node_key)
                    .ok_or("This agent is no longer available.")?;
                if chat_id.as_ref().is_some_and(|id| {
                    !chat_in_project(
                        &self.project_chats,
                        id,
                        binding.project_directory.as_deref(),
                        binding.ssh_profile_id.is_some(),
                    )
                }) {
                    return Err("Choose a conversation in the same project as this agent.".into());
                }
                self.reset_supervision(&node_key);
                self.agent_graph_bindings
                    .iter_mut()
                    .find(|binding| binding.node_key() == node_key)
                    .expect("binding validated above")
                    .project_chat_id = chat_id;
                self.save_session();
            }
            Action::RenameAgent { node_key, name } => {
                self.rename_agent_graph_agent(&node_key, &name)?;
            }
            Action::DeleteAgent { node_key } => {
                self.remove_agent_graph_agent_by_node_key(&node_key)?;
            }
        }
        Ok(())
    }

    pub(super) fn is_project_chat_card_owner(&self, owner: &str) -> bool {
        owner.strip_prefix("chat:").is_some_and(|chat_id| {
            self.project_board_open_chat_ids
                .iter()
                .any(|id| id == chat_id)
                && self
                    .project_chats
                    .iter()
                    .any(|chat| chat.id == chat_id && !chat.archived)
        })
    }

    pub(super) fn is_supervised_project_chat_owner(&self, owner: &str) -> bool {
        owner.strip_prefix("chat:").is_some_and(|chat_id| {
            self.agent_graph_bindings.iter().any(|binding| {
                binding.project_chat_id.as_deref() == Some(chat_id)
                    && chat_in_project(
                        &self.project_chats,
                        chat_id,
                        binding.project_directory.as_deref(),
                        binding.ssh_profile_id.is_some(),
                    )
            })
        })
    }

    fn project_chat_card_owner(&self, chat_id: &str) -> Result<String, String> {
        let owner = format!("chat:{chat_id}");
        if !self.is_project_chat_card_owner(&owner) {
            return Err("Reopen this project conversation before using its agent card.".into());
        }
        Ok(owner)
    }

    #[allow(clippy::too_many_arguments)]
    fn submit_project_chat_card(
        &mut self,
        chat_id: &str,
        mut message: String,
        delivery: AgentSubmissionDelivery,
        resume: bool,
        file_ids: Vec<String>,
        tab_ids: Vec<u64>,
        terminal_session_ids: Vec<u64>,
        timing: Option<app_server::delivery::ClientTiming>,
    ) -> Result<(), String> {
        let owner = self.project_chat_card_owner(chat_id)?;
        let chat = self
            .project_chats
            .iter()
            .find(|chat| chat.id == chat_id && !chat.archived)
            .cloned()
            .ok_or("This conversation is no longer available.")?;
        let profile = self
            .project_chat_card_profiles
            .get(chat_id)
            .cloned()
            .ok_or("This conversation card has no agent profile.")?;
        if resume {
            if delivery != AgentSubmissionDelivery::Start
                || !file_ids.is_empty()
                || !tab_ids.is_empty()
                || !terminal_session_ids.is_empty()
                || !self.agent_can_resume_for_owner(&owner, profile.provider)
            {
                return Err(
                    "This response is no longer stopped. Your draft and attachments were retained."
                        .into(),
                );
            }
            message = RESUME_INTERRUPTED_PROMPT.to_owned();
        }
        if profile.provider == AgentProviderKind::CodexAppServer
            && delivery == AgentSubmissionDelivery::Steer
        {
            return Err("Native Send now requires the displayed turn ID. Reopen the delivery choices; this input was not queued or started.".into());
        }
        let mut snapshots = self.freeze_graph_contexts(&owner, &tab_ids, &terminal_session_ids)?;
        snapshots.files = self.graph_files.capture(&owner, &file_ids)?;
        let root = (!chat.project_root.is_empty()).then(|| PathBuf::from(&chat.project_root));
        let native_summary = self.app_server.summary(&owner);
        let mut submission = PendingAgentSubmission {
            delivery_trace: Some(app_server::delivery::Trace::new(timing)),
            native_skills: vec![],
            native_apps: vec![],
            scope: Some(submission_scope::SubmissionScope {
                native_access: self.app_server.access(),
                native_summary,
                owner: owner.clone(),
                workspace_root: root.clone(),
                native_targets: self.capture_native_targets(),
            }),
            snapshots,
            provider: profile.provider,
            message,
            tab_ids,
            terminal_session_ids,
            file_ids,
            selection_override: Some(profile.selection),
            agent_graph_launch: None,
            card_draft: true,
            supervision_review: None,
        };
        if submission.provider == AgentProviderKind::CodexAppServer {
            let fallback = self.data_dir.join("codex-workspace");
            let directory = root.as_deref().unwrap_or(&fallback);
            submission.native_skills = self.app_server.capture_skills_for_prompt(
                &owner,
                Some(directory),
                &submission.message,
            )?;
            let thread = self
                .app_server
                .conversations
                .binding(&owner)
                .map(|binding| binding.thread_id.clone());
            submission.native_apps = self.app_server.capture_apps(&owner, thread.as_deref())?;
        }
        self.validate_initial_submission(&submission)?;
        self.validate_submission_scope(&submission)?;
        if self.owner_has_running_work(&owner) {
            self.enqueue_agent_submission(submission);
        } else if !self.agent_submission_queue.is_empty() {
            self.enqueue_agent_submission(submission);
            self.start_next_agent_submission_if_idle();
        } else {
            self.submit_agent_submission(submission);
        }
        Ok(())
    }

    fn stop_project_chat_card(&mut self, chat_id: &str) -> Result<(), String> {
        let owner = self.project_chat_card_owner(chat_id)?;
        if self.stop_app_server(&owner) {
            return Ok(());
        }
        if let Some(run_id) = self
            .main_runs
            .latest(&owner)
            .filter(|run| run.is_active())
            .map(|run| run.id)
        {
            self.stop_agent_run_by_id(run_id);
        }
        Ok(())
    }

    fn steer_supervised_chat(
        &mut self,
        node_key: &str,
        expected_turn_id: String,
        message: String,
        timing: Option<app_server::delivery::ClientTiming>,
    ) -> Result<(), String> {
        if message.trim().is_empty() || message.chars().count() > 10_000 || message.contains('\0') {
            return Err(
                "Write a correction of at most 10,000 characters. Your draft was retained.".into(),
            );
        }
        let binding = self
            .agent_graph_bindings
            .iter()
            .find(|binding| binding.node_key() == node_key)
            .ok_or("This supervisor is no longer available.")?;
        let chat_id = binding
            .project_chat_id
            .as_deref()
            .ok_or("Link this supervisor to a project conversation first.")?;
        if !chat_in_project(
            &self.project_chats,
            chat_id,
            binding.project_directory.as_deref(),
            binding.ssh_profile_id.is_some(),
        ) {
            return Err("The supervised conversation no longer belongs to this project.".into());
        }
        let target_owner = format!("chat:{chat_id}");
        if self.app_server.steer_target(&target_owner) != Some(expected_turn_id.as_str()) {
            return Err("The observed Codex turn is no longer active. The correction was not started or queued, and your draft was retained.".into());
        }
        let input = app_server::SteerInput::supervised(expected_turn_id, message, timing);
        self.app_server_steer_for(target_owner, input, format!("graph:{node_key}"));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_card_identity_and_optional_chat_link_survive_reload() {
        let owner = Uuid::new_v4();
        let chat = Uuid::new_v4().to_string();
        let old = json!({"recordType":"entity", "recordId":Uuid::new_v4().to_string(),
            "conversationId":owner,"projectDirectory":"C:/work/a","name":"Verifier",
            "mission":"Verify the project", "selection":{"model":"fixture","effort":"low"}});
        let legacy: AgentGraphBinding = serde_json::from_value(old).unwrap();
        assert!(legacy.project_chat_id.is_none());
        let linked = AgentGraphBinding {
            project_chat_id: Some(chat.clone()),
            ..legacy
        };
        let reloaded: AgentGraphBinding =
            serde_json::from_slice(&serde_json::to_vec(&linked).unwrap()).unwrap();
        let validated = agent_graph::normalize_loaded_agent_graph_binding(reloaded).unwrap();
        assert_eq!(validated.conversation_id, Some(owner));
        assert_eq!(validated.project_chat_id, Some(chat));
        assert_eq!(validated.mission, "Verify the project");
    }

    #[test]
    fn associations_reject_cross_project_archived_and_remote_chats() {
        let chat: ProjectChat =
            serde_json::from_value(json!({"id":"chat", "projectRoot":"C:/work/a",
            "title":"Original", "pinned":false,"archived":false,
            "createdAtMs":1,"updatedAtMs":1,"messages":[]}))
            .unwrap();
        assert!(chat_in_project(
            std::slice::from_ref(&chat),
            "chat",
            Some("C:/work/a"),
            false
        ));
        assert!(!chat_in_project(
            std::slice::from_ref(&chat),
            "chat",
            Some("C:/work/b"),
            false
        ));
        assert!(!chat_in_project(
            std::slice::from_ref(&chat),
            "chat",
            Some("C:/work/a"),
            true
        ));
        let archived = ProjectChat {
            archived: true,
            ..chat
        };
        assert!(!chat_in_project(
            &[archived],
            "chat",
            Some("C:/work/a"),
            false
        ));
    }

    #[test]
    fn supervisor_card_actions_keep_the_exact_saved_identity() {
        let rename: Action = serde_json::from_value(json!({
            "type":"rename_agent",
            "node_key":"conversation:7e9efef3-1496-47cd-898e-40f0a49c8a1d",
            "name":"Release coordinator"
        }))
        .unwrap();
        assert!(matches!(rename, Action::RenameAgent { node_key, name }
            if node_key == "conversation:7e9efef3-1496-47cd-898e-40f0a49c8a1d"
                && name == "Release coordinator"));

        let delete: Action = serde_json::from_value(json!({
            "type":"delete_agent",
            "node_key":"conversation:7e9efef3-1496-47cd-898e-40f0a49c8a1d"
        }))
        .unwrap();
        assert!(matches!(delete, Action::DeleteAgent { node_key }
            if node_key == "conversation:7e9efef3-1496-47cd-898e-40f0a49c8a1d"));

        let steer: Action = serde_json::from_value(json!({
            "type":"steer_supervised",
            "node_key":"conversation:7e9efef3-1496-47cd-898e-40f0a49c8a1d",
            "expected_turn_id":"turn-active",
            "message":"Run the failing test before editing anything else."
        }))
        .unwrap();
        assert!(
            matches!(steer, Action::SteerSupervised { node_key, expected_turn_id, message, .. }
            if node_key == "conversation:7e9efef3-1496-47cd-898e-40f0a49c8a1d"
                && expected_turn_id == "turn-active"
                && message.starts_with("Run the failing test"))
        );
    }
}
