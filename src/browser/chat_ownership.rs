//! Message ownership survives selection changes. Persisted ProjectChat containers
//! remain the file format; the in-memory ledger may contain several loaded chats.
use super::*;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ChatLineageView {
    kind: &'static str,
    parent_chat_id: Option<String>,
    parent_title: Option<String>,
    parent_available: bool,
    child_count: usize,
    delegated_count: usize,
}

/// Project only existing local identities and display names. Never infer a
/// relationship from a title, directory, recency, or a truncated native ID.
pub(super) fn project_chat_lineage(
    chats: &mut [ProjectChatSummaryView],
    saved: &central_agent_codex_runtime::conversations::Saved,
) {
    let local: HashMap<_, _> = chats
        .iter()
        .map(|chat| {
            (
                format!("chat:{}", chat.id),
                (chat.id.clone(), chat.title.clone(), !chat.archived),
            )
        })
        .collect();
    let by_native: HashMap<_, _> = saved
        .bindings
        .iter()
        .filter_map(|(owner, binding)| {
            local
                .get(owner)
                .map(|chat| (binding.thread_id.as_str(), chat))
        })
        .collect();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut delegated_counts: HashMap<&str, usize> = HashMap::new();
    for (child, parent) in &saved.delegations {
        if by_native.contains_key(child.as_str()) && by_native.contains_key(parent.as_str()) {
            *delegated_counts.entry(parent).or_default() += 1;
        }
    }
    for (child, parent) in &saved.lineage {
        if by_native.contains_key(child.as_str()) && by_native.contains_key(parent.as_str()) {
            *counts.entry(parent).or_default() += 1;
        }
    }
    for chat in chats {
        let owner = format!("chat:{}", chat.id);
        let binding = saved.bindings.get(&owner);
        let pending = saved.fork_origins.get(&owner);
        let delegation = binding.and_then(|binding| saved.delegations.get(&binding.thread_id));
        let parent = binding
            .and_then(|binding| saved.lineage.get(&binding.thread_id))
            .or(pending);
        let parent = delegation.or(parent);
        let parent_chat = parent.and_then(|id| by_native.get(id.as_str()));
        let child_count = binding
            .and_then(|binding| counts.get(binding.thread_id.as_str()))
            .copied()
            .unwrap_or(0);
        let delegated_count = binding
            .and_then(|binding| delegated_counts.get(binding.thread_id.as_str()))
            .copied()
            .unwrap_or(0);
        chat.lineage = if parent.is_some() || child_count > 0 || delegated_count > 0 {
            Some(ChatLineageView {
                kind: if delegation.is_some() {
                    "delegated"
                } else if pending.is_some() {
                    "pending"
                } else if parent.is_some() {
                    "fork"
                } else {
                    "source"
                },
                parent_chat_id: parent_chat.map(|chat| chat.0.clone()),
                parent_title: parent_chat.map(|chat| chat.1.clone()),
                parent_available: parent_chat.is_some_and(|chat| chat.2),
                child_count,
                delegated_count,
            })
        } else {
            None
        };
    }
}

#[derive(Default)]
pub(super) struct ChatOwnership {
    loaded: HashSet<String>,
    messages: HashMap<u64, String>,
}

impl ChatOwnership {
    pub fn restored(chat_id: Option<&str>, rows: &VecDeque<ChatMessage>) -> Self {
        let mut ownership = Self::default();
        if let Some(chat_id) = chat_id {
            ownership.loaded.insert(chat_id.to_owned());
            for row in rows {
                ownership.claim(row.id, chat_id);
            }
        }
        ownership
    }

    pub fn claim(&mut self, message_id: u64, chat_id: &str) {
        self.messages.insert(message_id, chat_id.to_owned());
    }

    pub fn is_loaded(&self, chat_id: &str) -> bool {
        self.loaded.contains(chat_id)
    }

    pub fn belongs_to(
        &self,
        row: &ChatMessage,
        chat_id: Option<&str>,
        contexts: &HashMap<u64, run_context::RunExecutionContext>,
    ) -> bool {
        // Run identity wins over UI selection, including a message queued just
        // before navigation and graph messages initially built by a shared helper.
        if let Some(context) = row.run_id.and_then(|id| contexts.get(&id)) {
            return context
                .conversation_key
                .strip_prefix("chat:")
                .is_some_and(|owner| Some(owner) == chat_id);
        }
        self.messages.get(&row.id).map(String::as_str) == chat_id
    }

    pub fn load(&mut self, chat: &ProjectChat, rows: &mut VecDeque<ChatMessage>) {
        if self.loaded.insert(chat.id.clone()) {
            for row in &chat.messages {
                self.claim(row.id, &chat.id);
                rows.push_back(row.clone());
            }
        }
    }

    pub fn forget(
        &mut self,
        chat_id: &str,
        rows: &mut VecDeque<ChatMessage>,
        contexts: &HashMap<u64, run_context::RunExecutionContext>,
    ) {
        rows.retain(|row| !self.belongs_to(row, Some(chat_id), contexts));
        self.messages.retain(|_, owner| owner != chat_id);
        self.loaded.remove(chat_id);
    }
}

impl BrowserApp {
    pub(super) fn chat_messages_for<'a>(
        &'a self,
        chat_id: Option<&'a str>,
    ) -> impl Iterator<Item = &'a ChatMessage> {
        self.chat_messages.iter().filter(move |message| {
            self.chat_ownership
                .belongs_to(message, chat_id, &self.run_execution_contexts)
        })
    }

    pub(super) fn message_in_active_chat(&self, message: &ChatMessage) -> bool {
        self.chat_ownership.belongs_to(
            message,
            self.active_project_chat_id.as_deref(),
            &self.run_execution_contexts,
        )
    }

    pub(super) fn chat_history_snapshot(&self, chat: &ProjectChat) -> ProjectChat {
        let mut chat = chat.clone();
        if self.chat_ownership.is_loaded(&chat.id) {
            chat.messages = self.chat_messages_for(Some(&chat.id)).cloned().collect();
            if !chat.title_custom {
                chat.title = project_chat_title(&chat.messages);
            }
            chat.updated_at_ms = chat.messages.last().map_or(chat.updated_at_ms, |message| {
                message.timestamp_ms.max(chat.updated_at_ms)
            });
        }
        if self.active_project_chat_id.as_deref() == Some(chat.id.as_str()) {
            chat.claude_session_id = self.claude_session_id.clone();
            chat.claude_session_cwd = self.claude_session_cwd.clone();
        }
        chat
    }

    pub(super) fn chat_summary(&self, chat: &ProjectChat) -> ProjectChatSummaryView {
        // Navigation is repainted during streaming. Do not clone transcripts,
        // images and diffs from every loaded conversation just to render a title.
        let (title, message_count, updated_at_ms) = if self.chat_ownership.is_loaded(&chat.id) {
            let title = if chat.title_custom {
                chat.title.clone()
            } else {
                project_chat_title(self.chat_messages_for(Some(&chat.id)))
            };
            let (count, updated) = self
                .chat_messages_for(Some(&chat.id))
                .fold((0, chat.updated_at_ms), |(count, updated), row| {
                    (count + 1, updated.max(row.timestamp_ms))
                });
            (title, count, updated)
        } else {
            (chat.title.clone(), chat.messages.len(), chat.updated_at_ms)
        };
        ProjectChatSummaryView {
            lineage: None,
            id: chat.id.clone(),
            project_root: chat.project_root.clone(),
            title,
            pinned: chat.pinned,
            archived: chat.archived,
            active: self.active_project_chat_id.as_deref() == Some(chat.id.as_str()),
            agent_active: self.owner_has_running_work(&format!("chat:{}", chat.id)),
            needs_attention: self
                .main_runs
                .latest(&format!("chat:{}", chat.id))
                .is_some_and(|run| self.pending_approval_for_run(run.id).is_some()),
            mutation_locked: self.chat_has_unfinished_work(&chat.id),
            verification: self.supervision_delivery_for_chat(&chat.id),
            message_count,
            updated_at_ms,
        }
    }

    pub(super) fn project_chat_preview(&self, chat_id: &str) -> Option<ProjectChatPreviewView<'_>> {
        let chat = self
            .project_chats
            .iter()
            .find(|chat| chat.id == chat_id && !chat.archived)?;
        let loaded = self.chat_ownership.is_loaded(&chat.id);
        let title = if loaded && !chat.title_custom {
            project_chat_title(self.chat_messages_for(Some(&chat.id)))
        } else {
            chat.title.clone()
        };
        let messages = if loaded {
            self.chat_messages_for(Some(&chat.id))
                .map(|message| RenderedChatMessage::new(message, &self.artifact_store))
                .collect()
        } else {
            chat.messages
                .iter()
                .map(|message| RenderedChatMessage::new(message, &self.artifact_store))
                .collect()
        };
        let owner = format!("chat:{}", chat.id);
        let profile = self
            .project_chat_card_profiles
            .get(&chat.id)
            .cloned()
            .unwrap_or_else(|| {
                let provider = if self.app_server.conversations.binding(&owner).is_some() {
                    AgentProviderKind::CodexAppServer
                } else {
                    self.agent_provider
                };
                ProjectChatCardProfile {
                    provider,
                    selection: self.provider_configuration(provider).2.clone(),
                }
            });
        let mut agent = if profile.provider == AgentProviderKind::CodexAppServer {
            self.app_server_run_view(&owner)
                .unwrap_or_else(AgentRuntimeView::idle)
        } else {
            self.main_runs
                .latest(&owner)
                .map(AgentRun::view)
                .unwrap_or_else(AgentRuntimeView::idle)
        };
        agent.active = self.owner_has_running_work(&owner);
        let pending_approval = self
            .main_runs
            .latest(&owner)
            .and_then(|run| self.pending_approval_for_run(run.id));
        Some(ProjectChatPreviewView {
            id: &chat.id,
            title,
            provider: profile.provider,
            selection: profile.selection,
            agent,
            can_resume: self.agent_can_resume_for_owner(&owner, profile.provider),
            pending_approval,
            conversation_state: self.conversation_state(&owner),
            messages,
        })
    }

    pub(super) fn project_chat_previews(&self) -> Vec<ProjectChatPreviewView<'_>> {
        self.project_board_open_chat_ids
            .iter()
            .filter_map(|chat_id| self.project_chat_preview(chat_id))
            .collect()
    }

    pub(super) fn forget_chat_messages(&mut self, chat_id: &str) {
        self.main_drafts.remove(&format!("chat:{chat_id}"));
        self.chat_ownership.forget(
            chat_id,
            &mut self.chat_messages,
            &self.run_execution_contexts,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_ancestry_uses_bound_ids_and_current_titles_not_similar_names() {
        fn row(id: &str) -> ProjectChatSummaryView {
            ProjectChatSummaryView {
                lineage: None,
                agent_active: false,
                needs_attention: false,
                mutation_locked: false,
                verification: None,
                id: id.into(),
                project_root: "project".into(),
                title: "Same title".into(),
                pinned: false,
                archived: false,
                active: false,
                message_count: 0,
                updated_at_ms: 0,
            }
        }
        let saved = serde_json::from_value(json!({"version":1,"unresolved":{},"bindings":{
            "chat:source":{"threadId":"s","sessionId":null,"archived":false},
            "chat:child":{"threadId":"c","sessionId":null,"archived":false},
            "chat:nested":{"threadId":"n","sessionId":null,"archived":false},
            "chat:orphan":{"threadId":"o","sessionId":null,"archived":false}
        },"lineage":{"c":"s","n":"c","o":"external"},"forkOrigins":{"chat:pending":"s"}}))
        .unwrap();
        let mut rows = vec![
            row("source"),
            row("child"),
            row("nested"),
            row("orphan"),
            row("pending"),
            row("unrelated"),
        ];
        project_chat_lineage(&mut rows, &saved);
        assert_eq!(rows[0].lineage.as_ref().unwrap().kind, "source");
        assert_eq!(rows[0].lineage.as_ref().unwrap().child_count, 1);
        assert_eq!(rows[1].lineage.as_ref().unwrap().kind, "fork");
        assert_eq!(rows[1].lineage.as_ref().unwrap().child_count, 1);
        assert_eq!(
            rows[2].lineage.as_ref().unwrap().parent_chat_id.as_deref(),
            Some("child")
        );
        assert!(!rows[3].lineage.as_ref().unwrap().parent_available);
        assert_eq!(rows[4].lineage.as_ref().unwrap().kind, "pending");
        assert!(rows[5].lineage.is_none());
        rows[0].title = "Renamed source".into();
        rows[0].archived = true;
        project_chat_lineage(&mut rows, &saved);
        assert_eq!(
            rows[1].lineage.as_ref().unwrap().parent_title.as_deref(),
            Some("Renamed source")
        );
        assert!(!rows[1].lineage.as_ref().unwrap().parent_available);
        assert!(
            !serde_json::to_string(&rows[1].lineage)
                .unwrap()
                .contains("threadId")
        );
    }

    fn row(id: u64, run_id: u64) -> ChatMessage {
        ChatMessage::reasoning(id, run_id, format!("item-{id}"), 0, "Progress")
    }

    fn contexts() -> HashMap<u64, run_context::RunExecutionContext> {
        [(1, "chat:a"), (2, "chat:b"), (3, "graph:node")]
            .into_iter()
            .map(|(id, owner)| {
                (
                    id,
                    run_context::RunExecutionContext {
                        conversation_key: owner.to_owned(),
                        workspace_root: None,
                        ssh_profile_id: None,
                        provider: AgentProviderKind::ClaudeCode,

                        native_targets: native_targets::NativeTargets::default(),
                        terminal_session_id: None,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn background_stream_is_not_rendered_in_selected_chat() {
        let contexts = contexts();
        let mut ownership = ChatOwnership::default();
        let message = row(10, 1);
        // Simulates a shared helper called while B is selected. Run A wins.
        ownership.claim(message.id, "b");
        assert!(ownership.belongs_to(&message, Some("a"), &contexts));
        assert!(!ownership.belongs_to(&message, Some("b"), &contexts));
        assert!(!ownership.belongs_to(&row(11, 3), None, &contexts));
        assert!(!ownership.belongs_to(&row(11, 3), Some("a"), &contexts));
    }

    #[test]
    fn legacy_history_keeps_ownership_without_a_live_run_context() {
        let rows = VecDeque::from([row(1, 40), row(2, 41)]);
        let ownership = ChatOwnership::restored(Some("old-chat"), &rows);
        assert!(ownership.is_loaded("old-chat"));
        for message in &rows {
            assert!(ownership.belongs_to(message, Some("old-chat"), &HashMap::new()));
            assert!(!ownership.belongs_to(message, Some("new-chat"), &HashMap::new()));
        }
    }

    #[test]
    fn removing_a_chat_forgets_only_its_ledger_rows() {
        let contexts = contexts();
        let mut rows = VecDeque::from([row(10, 1), row(20, 2), row(30, 3)]);
        let mut ownership = ChatOwnership::restored(Some("a"), &VecDeque::from([row(10, 1)]));
        ownership.forget("a", &mut rows, &contexts);
        assert!(!ownership.is_loaded("a"));
        assert_eq!(rows.iter().map(|row| row.id).collect::<Vec<_>>(), [20, 30]);
    }

    #[test]
    fn loading_a_chat_twice_does_not_duplicate_or_drop_existing_messages() {
        let chat = ProjectChat {
            id: "b".to_owned(),
            project_root: String::new(),
            title: "B".to_owned(),
            title_custom: false,
            pinned: false,
            archived: false,
            created_at_ms: 1,
            updated_at_ms: 1,
            messages: vec![row(20, 2)],
            claude_session_id: None,
            claude_session_cwd: None,
        };
        let mut rows = VecDeque::from([row(10, 1), row(30, 3)]);
        let mut ownership = ChatOwnership::default();
        ownership.load(&chat, &mut rows);
        ownership.load(&chat, &mut rows);
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            [10, 30, 20]
        );
    }
}
