//! Local destinations for native forks. Preparing a chat/card does not copy
//! messages, start another provider, grant permissions or submit a model turn.
use super::*;

#[derive(Clone, Debug, Serialize)]
pub(super) struct BranchTarget {
    pub(super) owner: String,
    pub(super) name: String,
}

fn project_branch(root: &str, title: &str) -> ProjectChat {
    let now = unix_time_ms();
    ProjectChat {
        id: Uuid::new_v4().to_string(),
        project_root: root.into(),
        title: title.into(),
        title_custom: true,
        pinned: false,
        archived: false,
        created_at_ms: now,
        updated_at_ms: now,
        messages: vec![],
        claude_session_id: None,
        claude_session_cwd: None,
    }
}
fn graph_branch(source: &AgentGraphBinding, title: &str) -> AgentGraphBinding {
    AgentGraphBinding {
        conversation_id: Some(Uuid::new_v4()),
        name: title.into(),
        provider: AgentProviderKind::CodexAppServer,
        ..source.clone()
    }
}

impl BrowserApp {
    /// Allocate and durably save the local owner before a native operation can
    /// create a thread for it. The destination contains no copied transcript,
    /// provider session or permission consent.
    pub(super) fn create_native_destination(
        &mut self,
        owner: &str,
        title: &str,
        root: &Path,
    ) -> Result<BranchTarget, String> {
        let title = normalized_user_title(title, MAX_PROJECT_LABEL_CHARS)
            .ok_or("Name the new conversation")?;
        if self
            .pending_workspace_ejections
            .contains(&root.display().to_string())
        {
            return Err("This project is being removed".into());
        }
        let destination = if let Some(id) = owner.strip_prefix("chat:") {
            let project_root = self
                .project_chats
                .iter()
                .find(|chat| chat.id == id && !chat.archived)
                .filter(|chat| Path::new(&chat.project_root) == root)
                .map(|chat| chat.project_root.clone())
                .ok_or("The source project changed")?;
            let chat = project_branch(&project_root, &title);
            let destination = format!("chat:{}", chat.id);
            self.project_chats.push(chat);
            let result = serde_json::to_vec_pretty(&self.project_chat_history_snapshot())
                .map_err(|error| error.to_string())
                .and_then(|bytes| {
                    write_file_atomically::<ProjectChatHistory>(
                        &self.data_dir.join(PROJECT_CHAT_HISTORY_FILE),
                        &bytes,
                    )
                    .map_err(|error| error.to_string())
                });
            if let Err(error) = result {
                self.project_chats.pop();
                return Err(format!(
                    "The new chat could not be saved. No native operation was sent: {error}"
                ));
            }
            destination
        } else if let Some(key) = owner.strip_prefix("graph:") {
            let source = self
                .agent_graph_bindings
                .iter()
                .find(|binding| binding.node_key() == key)
                .cloned()
                .ok_or("The graph agent is no longer assigned")?;
            let binding = graph_branch(&source, &title);
            let destination = format!("graph:{}", binding.node_key());
            self.agent_graph_bindings.push(binding);
            let result = serde_json::to_vec_pretty(&self.session_snapshot())
                .map_err(|error| error.to_string())
                .and_then(|bytes| {
                    write_file_atomically::<BrowserSession>(
                        &self.data_dir.join("session.json"),
                        &bytes,
                    )
                    .map_err(|error| error.to_string())
                });
            if let Err(error) = result {
                self.agent_graph_bindings.pop();
                return Err(format!(
                    "The new graph conversation could not be saved. No native operation was sent: {error}"
                ));
            }
            destination
        } else {
            return Err("Create a project chat before creating another native conversation".into());
        };
        Ok(BranchTarget {
            owner: destination,
            name: title,
        })
    }

    pub(super) fn create_native_branch(
        &mut self,
        owner: &str,
        title: &str,
        expected_directory: &str,
        last_turn_id: Option<&str>,
    ) -> Result<(), String> {
        if !self.native_access_selected(owner) {
            return Err("Select Codex before creating a native branch".into());
        }
        self.app_server.conversations.validate_fork_source(owner)?;
        let target = self.conversation_target(owner)?;
        let root = target
            .root
            .filter(|p| !target.remote && p.is_absolute() && p.is_dir())
            .ok_or("Choose an existing local project directory for this fork")?;
        if !same_local_path(&root, Path::new(expected_directory)) {
            return Err("The project directory changed. Confirm the fork again.".into());
        }
        let selection = self.app_server.configuration().2.clone();
        provider_types::validate_selection(&self.app_server.view.models, &selection)?;
        let profile = api::Profile {
            model: Some(selection.model),
            effort: Some(selection.effort),
            service_tier: selection.service_tier,
            personality: selection
                .personality
                .as_deref()
                .map(|personality| match personality {
                    "friendly" => api::Personality::Friendly,
                    "pragmatic" => api::Personality::Pragmatic,
                    _ => api::Personality::None,
                }),
            summary: self.app_server.summary(owner),
        };
        let access = self.app_server.access();
        access.validate_requirements(self.app_server.requirements.as_ref())?;
        let options = api::ThreadForkOptions {
            last_turn_id: last_turn_id.map(str::to_owned),
            cwd: Some(expected_directory.into()),
            profile: Some(profile),
            access: Some(access),
            ..api::ThreadForkOptions::default()
        };
        let branch = self.create_native_destination(owner, title, &root)?;
        let destination = branch.owner.clone();
        self.app_server.roots.insert(destination.clone(), root);
        // New destinations use the same saved Supervisor preset. Native
        // request-specific grants are never copied from their source.
        self.app_server.last_branches.insert(owner.into(), branch);
        let request = self.app_server.conversations.action(
            owner,
            ThreadAction::ForkWithOptions {
                destination,
                options: Box::new(options),
            },
        )?;
        self.dispatch_app_server_thread(request);
        Ok(())
    }

    pub(super) fn open_native_branch(
        &mut self,
        owner: &str,
        destination: &str,
    ) -> Result<(), String> {
        self.conversation_target(owner)?;
        if self
            .app_server
            .last_branches
            .get(owner)
            .is_none_or(|b| b.owner != destination)
        {
            return Err("This branch shortcut is no longer current. Use Projects or the graph to open the conversation.".into());
        }
        self.open_native_destination(owner, destination)?;
        self.emit_app_server_conversation(owner, "branch_opened");
        Ok(())
    }

    pub(super) fn open_native_destination(
        &mut self,
        owner: &str,
        destination: &str,
    ) -> Result<(), String> {
        if let Some(id) = destination.strip_prefix("chat:") {
            if !self
                .project_chats
                .iter()
                .any(|chat| chat.id == id && !chat.archived)
            {
                return Err("The branch chat is no longer available".into());
            }
            self.activate_project_chat(id);
        } else if let Some(key) = destination.strip_prefix("graph:") {
            if !self
                .agent_graph_bindings
                .iter()
                .any(|binding| binding.node_key() == key)
            {
                return Err("The graph branch is no longer available".into());
            }
            self.render_agent_panel();
            self.conversation_event(
                "central-agent:graph-open-conversation",
                json!({"owner":owner,"nodeKey":key,"replaceCurrent":false}),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn branch_destinations_have_independent_ids_and_no_other_provider_history() {
        let a = project_branch("C:\\fixture", "Branch");
        let b = project_branch("C:\\fixture", "Branch");
        assert_ne!(a.id, b.id);
        assert!(a.messages.is_empty());
        assert!(a.claude_session_id.is_none());
        assert!(a.title_custom);
        let source = AgentGraphBinding {
            record_type: "entity".into(),
            record_id: "node".into(),
            name: "Source".into(),
            ..Default::default()
        };
        let first = graph_branch(&source, "First");
        let second = graph_branch(&first, "Second");
        assert_ne!(source.node_key(), first.node_key());
        assert_ne!(first.node_key(), second.node_key());
        assert_eq!(source.record_id, second.record_id);
        assert_eq!(source.record_type, second.record_type);
        assert_eq!(second.provider, AgentProviderKind::CodexAppServer);
    }
}
