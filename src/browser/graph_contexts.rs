//! Graph-owned page/shell drafts. Capture callbacks use globally unique IDs;
//! submitted snapshots are separate immutable copies, never live subscriptions.
use super::*;

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Action {
    Read {},
    Tab { id: u64 },
    RemoveTab { id: u64 },
    Shell { id: u64 },
    RemoveShell { id: u64 },
    Follow { id: u64, enabled: bool },
}

#[derive(Default)]
pub(super) struct State {
    pub drafts: HashMap<String, SubmissionSnapshots>,
}
impl State {
    pub(super) fn capture(
        &self,
        owner: &str,
        tabs: &[u64],
        shells: &[u64],
    ) -> Result<SubmissionSnapshots, String> {
        if tabs.len() > MAX_CHAT_ATTACHMENTS || shells.len() > MAX_TERMINAL_CHAT_ATTACHMENTS {
            return Err("Too many context attachments for one request.".into());
        }
        let empty = SubmissionSnapshots::default();
        let draft = self.drafts.get(owner).unwrap_or(&empty);
        let mut result = SubmissionSnapshots::default();
        for id in tabs {
            if result.tabs.iter().any(|tab| tab.tab_id == *id) {
                return Err("A tab was submitted twice.".into());
            }
            result.tabs.push(
                draft
                    .tabs
                    .iter()
                    .find(|tab| tab.tab_id == *id)
                    .ok_or("This tab no longer belongs to this graph draft. Reattach it.")?
                    .clone(),
            );
        }
        for id in shells {
            if result
                .terminals
                .iter()
                .any(|shell| shell.snapshot.session_id == *id)
            {
                return Err("A shell was submitted twice.".into());
            }
            result.terminals.push(
                draft
                    .terminals
                    .iter()
                    .find(|shell| shell.snapshot.session_id == *id)
                    .ok_or("This shell no longer belongs to this graph draft. Reattach it.")?
                    .clone(),
            );
        }
        Ok(result)
    }
    pub(super) fn consume(&mut self, owner: &str, accepted: &SubmissionSnapshots) {
        if let Some(draft) = self.drafts.get_mut(owner) {
            main_drafts::remove_accepted_attachments(draft, accepted);
        }
    }
}

impl BrowserApp {
    pub(super) fn graph_context_state(&self, owner: &str) -> Value {
        let empty = SubmissionSnapshots::default();
        let draft = self.graph_contexts.drafts.get(owner).unwrap_or(&empty);
        let now = unix_time_ms();
        json!({"owner":owner,
            "tabs":draft.tabs.iter().map(|tab| tab.view(self.tabs.iter().find(|t| t.id == tab.tab_id).map(|t| t.content_revision),now)).collect::<Vec<_>>(),
            "shells":draft.terminals.iter().map(|shell| shell.view(self.terminal.has_session(shell.snapshot.session_id))).collect::<Vec<_>>()})
    }
    pub(super) fn emit_graph_contexts(&self) {
        for owner in self.graph_contexts.drafts.keys() {
            self.conversation_event(
                "central-agent:graph-contexts-result",
                self.graph_context_state(owner),
            );
        }
    }
    pub(super) fn manage_graph_contexts(&mut self, owner: &str, action: Action) {
        let result = (|| -> Result<(), String> {
            if !owner.starts_with("graph:") && !self.is_project_chat_card_owner(owner) {
                return Err("Reopen this project conversation before attaching context.".into());
            }
            if matches!(
                action,
                Action::RemoveTab { .. } | Action::RemoveShell { .. }
            ) {
                if owner.starts_with("graph:")
                    && !self
                        .agent_graph_bindings
                        .iter()
                        .any(|b| format!("graph:{}", b.node_key()) == owner)
                {
                    return Err("This graph conversation no longer exists.".into());
                }
            } else {
                self.conversation_target(owner)?;
            }
            match action {
                Action::Read {} => {}
                Action::Tab { id } => {
                    let tab = self
                        .tabs
                        .iter()
                        .find(|tab| tab.id == id)
                        .ok_or("This tab was closed.")?;
                    let draft = self.graph_contexts.drafts.entry(owner.into()).or_default();
                    if !draft.tabs.iter().any(|tab| tab.tab_id == id)
                        && draft.tabs.len() >= MAX_CHAT_ATTACHMENTS
                    {
                        return Err(format!(
                            "You can attach at most {MAX_CHAT_ATTACHMENTS} tabs."
                        ));
                    }
                    let capture_id = self.next_context_capture_id;
                    self.next_context_capture_id += 1;
                    let snapshot = TabContextSnapshot::capturing(
                        id,
                        capture_id,
                        &tab.title,
                        &tab.url,
                        tab.content_revision,
                    );
                    draft.tabs.retain(|tab| tab.tab_id != id);
                    draft.tabs.push(snapshot);
                    self.request_tab_context_capture(id, capture_id);
                }
                Action::RemoveTab { id } => {
                    if let Some(draft) = self.graph_contexts.drafts.get_mut(owner) {
                        draft.tabs.retain(|tab| tab.tab_id != id);
                    }
                }
                Action::Shell { id } => {
                    let draft = self.graph_contexts.drafts.entry(owner.into()).or_default();
                    if !draft.terminals.iter().any(|s| s.snapshot.session_id == id)
                        && draft.terminals.len() >= MAX_TERMINAL_CHAT_ATTACHMENTS
                    {
                        return Err(format!(
                            "You can attach at most {MAX_TERMINAL_CHAT_ATTACHMENTS} shells."
                        ));
                    }
                    let now = unix_time_ms();
                    let snapshot = self.terminal.context_snapshot(id, now)?;
                    let follow = draft
                        .terminals
                        .iter()
                        .find(|s| s.snapshot.session_id == id)
                        .is_none_or(|s| s.follow_live);
                    draft.terminals.retain(|s| s.snapshot.session_id != id);
                    let mut context = DraftTerminalContext::attached(snapshot, now);
                    context.follow_live = follow;
                    draft.terminals.push(context);
                }
                Action::RemoveShell { id } => {
                    if let Some(draft) = self.graph_contexts.drafts.get_mut(owner) {
                        draft.terminals.retain(|s| s.snapshot.session_id != id);
                    }
                }
                Action::Follow { id, enabled } => {
                    let context = self
                        .graph_contexts
                        .drafts
                        .get_mut(owner)
                        .and_then(|d| d.terminals.iter_mut().find(|s| s.snapshot.session_id == id))
                        .ok_or("This shell is not attached to this conversation.")?;
                    if enabled {
                        context.snapshot = self.terminal.context_snapshot(id, unix_time_ms())?;
                    }
                    context.follow_live = enabled;
                    context.live_update_scheduled = false;
                }
            }
            Ok(())
        })();
        let mut state = self.graph_context_state(owner);
        state["error"] = json!(result.err());
        if matches!(action, Action::Read {}) && state["error"].is_null() {
            // Enumeration carries only safe labels/IDs, not output or page content.
            state["availableTabs"] = json!(self.tabs.iter().map(|t| json!({"id":t.id,"label":crate::tab_context::sanitize_context_url(&t.url)})).collect::<Vec<_>>());
            state["availableShells"] = json!(
                self.terminal
                    .view()
                    .sessions
                    .iter()
                    .map(|s| json!({"id":s.id,"label":s.label}))
                    .collect::<Vec<_>>()
            );
        }
        self.conversation_event("central-agent:graph-contexts-result", state);
    }

    pub(super) fn refresh_graph_shells(&mut self, session_id: u64, force: bool) {
        let now = unix_time_ms();
        let mut snapshot = None;
        let mut changed = Vec::new();
        let mut delay = None;
        for (owner, draft) in &mut self.graph_contexts.drafts {
            for context in &mut draft.terminals {
                if context.snapshot.session_id != session_id || !context.follow_live {
                    continue;
                }
                let elapsed = now.saturating_sub(context.last_live_render_ms);
                if !force && elapsed < TERMINAL_LIVE_UI_INTERVAL_MS {
                    if !context.live_update_scheduled {
                        context.live_update_scheduled = true;
                        delay = Some((TERMINAL_LIVE_UI_INTERVAL_MS - elapsed) as u64);
                    }
                    continue;
                }
                context.live_update_scheduled = false;
                match snapshot
                    .get_or_insert_with(|| self.terminal.context_snapshot(session_id, now))
                {
                    Ok(value) => {
                        context.snapshot = value.clone();
                        context.last_live_render_ms = now;
                    }
                    Err(_) => context.follow_live = false,
                }
                changed.push(owner.clone());
            }
        }
        if let Some(delay) = delay {
            let proxy = self.proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(delay));
                let _ = proxy.send_event(BrowserEvent::TerminalContextRefresh { session_id });
            });
        }
        for owner in changed {
            self.conversation_event(
                "central-agent:graph-contexts-result",
                self.graph_context_state(&owner),
            );
        }
    }
    pub(super) fn freeze_graph_contexts(
        &mut self,
        owner: &str,
        tabs: &[u64],
        shells: &[u64],
    ) -> Result<SubmissionSnapshots, String> {
        // Validate ownership before refreshing any live shell.
        self.graph_contexts.capture(owner, tabs, shells)?;
        for id in shells {
            self.refresh_graph_shells(*id, true);
        }
        self.graph_contexts.capture(owner, tabs, shells)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(capture: u64, output: &str) -> SubmissionSnapshots {
        SubmissionSnapshots {
            tabs: vec![TabContextSnapshot::from_value(7,capture,"Page","https://example.com",1,100,
                json!({"text":output,"links":[{"href":"https://example.com/path","text":"Link"}]})).unwrap()],
            terminals: vec![DraftTerminalContext::attached(TerminalContextSnapshot {
                session_id:9,label:"Shell".into(),kind:TerminalKind::Local,profile_id:None,remote_target:None,
                phase:TerminalPhase::Running,busy:false,shell:"pwsh".into(),cwd:"C:/fixture".into(),status:"Ready".into(),
                output:output.into(),source_output_char_count:output.len(),output_revision:capture,last_exit_code:None,
                captured_at_ms:capture as u128,estimated_token_count:4,redaction_count:0,truncated:false,
            },100)],files:vec![],
        }
    }
    #[test]
    fn graph_snapshot_ownership_and_serialized_recovery_preserve_exact_accepted_content() {
        let mut state = State {
            drafts: HashMap::from([
                ("graph:a".into(), fixture(1, "first")),
                ("graph:b".into(), fixture(2, "second")),
            ]),
        };
        assert!(state.capture("chat:main", &[7], &[9]).is_err());
        assert!(state.capture("graph:unknown", &[7], &[]).is_err());
        assert!(state.capture("graph:a", &[7, 7], &[]).is_err());
        assert!(state.capture("graph:a", &[], &[9, 9]).is_err());
        let accepted = state.capture("graph:a", &[7], &[9]).unwrap();
        let bytes = serde_json::to_vec(&accepted).unwrap();
        state
            .drafts
            .insert("graph:a".into(), fixture(3, "manually changed"));
        let recovered: SubmissionSnapshots = serde_json::from_slice(&bytes).unwrap();
        state.consume("graph:a", &recovered);
        assert_eq!(state.drafts["graph:a"].tabs[0].capture_id, 3);
        assert_eq!(
            state.drafts["graph:a"].terminals[0].snapshot.output,
            "manually changed"
        );
        assert_eq!(state.drafts["graph:b"].tabs[0].text, "second");
        assert_eq!(snapshot_delivery::tabs(&recovered.tabs)[0].text, "first");
        assert_eq!(recovered.terminals[0].snapshot.output, "first");
        let current = state.capture("graph:a", &[7], &[9]).unwrap();
        state.consume("graph:a", &current);
        assert!(state.drafts["graph:a"].tabs.is_empty());
        assert!(state.drafts["graph:a"].terminals.is_empty());
        assert_eq!(state.drafts["graph:b"].terminals.len(), 1);
    }
    #[test]
    fn context_ipc_accepts_only_existing_session_ids_not_content_or_native_authority() {
        for value in [
            json!({"kind":"tab","id":7}),
            json!({"kind":"shell","id":9}),
            json!({"kind":"follow","id":9,"enabled":true}),
        ] {
            assert!(serde_json::from_value::<Action>(value.clone()).is_ok());
            for field in ["text", "url", "cwd", "profile_id", "owner", "command"] {
                let mut forged = value.clone();
                forged[field] = json!("injected");
                assert!(serde_json::from_value::<Action>(forged).is_err());
            }
        }
        let parsed:AgentPanelMessage=serde_json::from_value(json!({"message_type":"continue_knowledge_agent","record_type":"entity","id":"fixture","message":"","tab_ids":[7],"terminal_session_ids":[9],"delivery":"steer"})).unwrap();
        assert!(
            matches!(parsed,AgentPanelMessage::ContinueAgentGraphAgent {tab_ids,terminal_session_ids,delivery:AgentSubmissionDelivery::Steer,..} if tab_ids==[7] && terminal_session_ids==[9])
        );
    }
}
