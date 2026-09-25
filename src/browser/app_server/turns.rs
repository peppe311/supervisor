//! Native input dispatch. No agent loop, synthetic history or checkpoint hooks.
use super::*;

pub(super) struct Submission {
    input: PendingAgentSubmission,
    cwd: PathBuf,
    profile: api::Profile,
    access: api::Access,
    message_id: String,
    sending: bool,
    cancelled: bool,
    acknowledge: bool,
}

impl State {
    pub(in crate::browser) fn project_activity(
        &self,
        owner: &str,
        root: &str,
        other_run_started: Option<u128>,
    ) -> Option<crate::browser::project_activity::NativeActivity> {
        let pending = self.submissions.contains_key(owner) || self.reviews.contains_key(owner);
        let thread = self
            .conversations
            .binding(owner)
            .filter(|binding| !binding.deleted && !binding.archived)
            .and_then(|binding| self.conversations.mirror.thread(&binding.thread_id));
        if pending && thread.is_none_or(|thread| thread.active_turn().is_none()) {
            return Some((
                if self.view.connected {
                    "working"
                } else {
                    "waiting"
                },
                Vec::new(),
                false,
            ));
        }
        let thread = thread?;
        let turn = thread.active_turn().or_else(|| thread.turns.last())?;
        if !turn.active()
            && turn
                .started_at_ms
                .or(turn.completed_at_ms)
                .zip(other_run_started)
                .is_some_and(|(native, other)| u128::from(native) < other)
        {
            return None;
        }
        let connected = self.view.connected;
        let requests = self.requests.views(owner);
        let waiting_items = requests
            .iter()
            .filter(|request| request.turn_id.as_deref() == Some(turn.id.as_str()))
            .filter_map(|request| request.params["itemId"].as_str().map(str::to_owned))
            .collect();
        let state = match turn.status.as_str() {
            "failed" => "failed",
            "completed" => "done",
            "inProgress" if connected && !requests.is_empty() => "waiting",
            "inProgress" if connected => "working",
            _ => "stopped",
        };
        let cwd = self
            .roots
            .get(owner)
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.into());
        let (files, truncated) = crate::browser::project_activity::native_files(
            turn,
            root,
            &cwd,
            connected,
            &waiting_items,
        );
        Some((state, files, truncated))
    }

    pub(in crate::browser) fn busy(&self, owner: &str) -> bool {
        self.submissions.contains_key(owner)
            || self.reviews.contains_key(owner)
            || self.history_pages.busy(owner)
            || (self.conversations.busy(owner)
                && (!self.preparation.pending(owner)
                    || self.conversations.active_turn(owner).is_some()))
            || self.goals.writing(owner)
    }
    pub(in crate::browser) fn any_busy(&self) -> bool {
        !self.submissions.is_empty()
            || !self.reviews.is_empty()
            || self.history_pages.any_busy()
            || self.conversations.any_busy()
            || self.goals.any_writing()
            || self.native_config_pending()
            || self.p2.busy()
    }
    pub(in crate::browser) fn stoppable_owners(&self) -> Vec<String> {
        let mut owners = std::collections::BTreeSet::new();
        owners.extend(self.conversations.saved().bindings.keys().cloned());
        owners.extend(self.conversations.saved().fork_origins.keys().cloned());
        owners.extend(self.submissions.keys().cloned());
        owners.extend(self.reviews.keys().cloned());
        owners.extend(self.steering.keys().cloned());
        owners.extend(self.roots.keys().cloned());
        owners
            .into_iter()
            .filter(|owner| {
                self.submissions.contains_key(owner)
                    || self.reviews.contains_key(owner)
                    || self.steering.contains_key(owner)
                    || self.conversations.active_turn(owner).is_some()
                    || self.conversations.compaction_pending(owner)
            })
            .collect()
    }
    pub(in crate::browser) fn workspace_busy(&self, root: &str) -> bool {
        let root = fs::canonicalize(root).unwrap_or_else(|_| PathBuf::from(root));
        self.roots.iter().any(|(owner, directory)| {
            self.busy(owner) && (directory.starts_with(&root) || root.starts_with(directory))
        })
    }
    pub(in crate::browser) fn delivery_warning(
        &self,
        owner: &str,
    ) -> Option<&central_agent_codex_runtime::conversations::Receipt> {
        if self.submissions.contains_key(owner)
            || self.steering.contains_key(owner)
            || self.reviews.contains_key(owner)
            || self.conversations.compaction_pending(owner)
        {
            None
        } else {
            self.conversations.saved().unresolved.get(owner)
        }
    }
}

impl BrowserApp {
    pub(in crate::browser) fn submit_app_server(
        &mut self,
        mut input: PendingAgentSubmission,
        acknowledge: bool,
    ) {
        if let Err(error) = native_input(&input) {
            self.push_submission_message(&input, ChatRole::System, error);
            self.render_agent_panel();
            return;
        }
        if !self.accept_submission(&mut input) {
            return;
        }
        let owner = input.owner().expect("accepted owner").to_owned();
        if self.owner_has_running_work(&owner) {
            self.enqueue_agent_submission(input);
            return;
        }
        if let Err(error) = self.prepare_app_server_submission(input.clone(), acknowledge) {
            self.push_submission_message(&input, ChatRole::System, error);
        }
        self.render_agent_panel();
    }

    fn prepare_app_server_submission(
        &mut self,
        mut input: PendingAgentSubmission,
        acknowledge: bool,
    ) -> Result<(), String> {
        if self.app_server.native_config_pending() {
            return Err(
                "Wait for the shared Codex configuration change before submitting a prompt".into(),
            );
        }
        attachments::validate_inputs(
            native_input(&input)?,
            &input.tab_ids,
            &input.snapshots.tabs,
            &input.terminal_session_ids,
            &input.snapshots.terminals,
            &input.file_ids,
            &input.snapshots.files,
        )?;
        if !self.app_server.configuration().0.can_run() {
            return Err("Connect and sign in to Codex in AI accounts first".into());
        }
        let owner = input
            .owner()
            .ok_or("The request has no conversation owner")?
            .to_owned();
        if self.app_server.history_pages.busy(&owner) {
            return Err("Wait for native Codex history to finish loading".into());
        }
        let cwd = match input.workspace_root() {
            Some(root) => root.to_owned(),
            None if !owner.starts_with("graph:") => {
                let root = self.data_dir.join("codex-workspace");
                fs::create_dir_all(&root).map_err(|e| e.to_string())?;
                root
            }
            None => return Err("Select a graph node with a local working directory".into()),
        };
        let selection = input
            .selection_override
            .as_ref()
            .ok_or("Select a native model profile")?;
        provider_types::validate_selection(&self.app_server.view.models, selection)?;
        if selection.context_window.is_some() {
            return Err("Use the native model's default context window".into());
        }
        let profile = api::Profile {
            model: Some(selection.model.clone()),
            effort: Some(selection.effort.clone()),
            service_tier: selection.service_tier.clone(),
            personality: selection
                .personality
                .as_deref()
                .map(|personality| match personality {
                    "friendly" => api::Personality::Friendly,
                    "pragmatic" => api::Personality::Pragmatic,
                    _ => api::Personality::None,
                }),
            summary: input.scope.as_ref().and_then(|scope| scope.native_summary),
        };
        let access = input
            .scope
            .as_ref()
            .ok_or("Missing native submission scope")?
            .native_access;
        access.validate_requirements(self.app_server.requirements.as_ref())?;
        // Frozen user intent; never inherit permissions from another adapter.
        let preparing = self.app_server.preparation.pending(&owner);
        let request = if preparing || self.app_server.conversations.ready_for_turn(&owner) {
            None
        } else {
            Some(
                self.app_server
                    .conversations
                    .open(&owner, &cwd, &profile, access)?,
            )
        };
        self.app_server.roots.insert(owner.clone(), cwd.clone());
        let message_id = Uuid::new_v4().to_string();
        let trace = input
            .delivery_trace
            .clone()
            .unwrap_or_else(|| delivery::Trace::new(None));
        input.delivery_trace = Some(trace.clone());
        let mode = if preparing {
            "preparing"
        } else if request.is_none() {
            "ready"
        } else if self.app_server.conversations.binding(&owner).is_some() {
            "resume"
        } else {
            "new"
        };
        self.app_server.timings.register(
            &owner,
            &message_id,
            self.app_server
                .conversations
                .binding(&owner)
                .map(|b| b.thread_id.as_str()),
            trace,
            mode,
        );
        self.app_server.pending_inputs.begin(
            &owner,
            &message_id,
            native_input(&input)?,
            &input.snapshots,
            None,
            None,
        );
        self.app_server.submissions.insert(
            owner.clone(),
            Submission {
                input,
                cwd,
                profile,
                access,
                message_id: message_id.clone(),
                sending: false,
                cancelled: false,
                acknowledge,
            },
        );
        if let Some(request) = request {
            self.dispatch_app_server_thread(request);
        } else if !preparing && let Err(error) = self.start_opened_app_server_turn(&owner) {
            if let Some(pending) = self.app_server.submissions.remove(&owner)
                && let Some(trace) = &pending.input.delivery_trace
            {
                trace.finish("failed");
            }
            self.app_server.pending_inputs.remove(&owner, &message_id);
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn start_opened_app_server_turn(&mut self, owner: &str) -> Result<(), String> {
        let supervision_review = self
            .app_server
            .submissions
            .get(owner)
            .and_then(|pending| pending.input.supervision_review.clone());
        let supervision_context = owner
            .strip_prefix("graph:")
            .and_then(|node_key| self.supervised_chat_context(node_key));
        if !supervision_review
            .as_ref()
            .is_some_and(|review| review.automatic)
            && let Some(thread) = self
                .app_server
                .conversations
                .binding(owner)
                .map(|binding| binding.thread_id.clone())
        {
            self.app_server.promote_apps(owner, &thread);
        }
        let Some(pending) = self.app_server.submissions.get(owner) else {
            return Ok(());
        };
        if pending.cancelled {
            if let Some(trace) = &pending.input.delivery_trace {
                trace.finish("cancelled");
            }
            self.app_server
                .pending_inputs
                .remove(owner, &pending.message_id);
            self.app_server.submissions.remove(owner);
            return Ok(());
        }
        if !self.app_server.configuration().0.can_run() {
            return Err("Codex configuration changed while opening the chat. Review AI accounts before resubmitting.".into());
        }
        // Revalidate the frozen destination, never recapture the selected project.
        self.validate_submission_scope(&pending.input)?;
        provider_types::validate_selection(
            &self.app_server.view.models,
            pending
                .input
                .selection_override
                .as_ref()
                .ok_or("Missing native model profile")?,
        )?;
        let mut input = attachments::native_inputs(
            native_input(&pending.input)?,
            &pending.input.tab_ids,
            &pending.input.snapshots.tabs,
            &pending.input.terminal_session_ids,
            &pending.input.snapshots.terminals,
            &pending.input.file_ids,
            &pending.input.snapshots.files,
        )?;
        if let Some(review) = supervision_review.as_ref() {
            input.push(api::text_input(&supervision::review_instructions(review)));
        }
        if let Some(context) = supervision_context {
            input.push(api::text_input(&context));
        }
        let skill_directory = pending
            .input
            .agent_graph_launch
            .as_ref()
            .map(|l| Path::new(&l.project_directory))
            .or_else(|| pending.input.workspace_root())
            .unwrap_or(&pending.cwd);
        skills::append_skill_inputs(&mut input, &pending.input.native_skills, skill_directory)?;
        apps::append_app_inputs(&mut input, &pending.input.native_apps)?;
        pending
            .access
            .validate_requirements(self.app_server.requirements.as_ref())?;
        if let Some(trace) = &pending.input.delivery_trace {
            trace.prepared();
        }
        let turn_options = api::TurnOptions {
            output_schema: supervision_review
                .as_ref()
                .map(|_| supervision::review_output_schema()),
            ..api::TurnOptions::default()
        };
        let request = self.app_server.conversations.start_turn_with_options(
            central_agent_codex_runtime::conversations::TurnStart {
                owner,
                message_id: &pending.message_id,
                input,
                cwd: &pending.cwd,
                profile: &pending.profile,
                access: pending.access,
                options: &turn_options,
            },
        )?;
        // Persist the delivery receipt BEFORE any byte of turn/start is sent.
        if let Err(error) = self.save_app_server_bindings() {
            let _ = self
                .app_server
                .conversations
                .complete(&request, Err(CallError::Rejected(error.clone())));
            return Err(error);
        }
        self.app_server.submissions.get_mut(owner).unwrap().sending = true;
        self.dispatch_app_server_thread(request);
        Ok(())
    }

    pub(super) fn accept_app_server_submission(
        &mut self,
        owner: &str,
        message_id: &str,
        turn_id: &str,
    ) {
        self.app_server
            .pending_inputs
            .accept(owner, message_id, turn_id);
        if self.accept_app_server_steering(owner, message_id) {
            return;
        }
        if self
            .app_server
            .submissions
            .get(owner)
            .is_none_or(|s| s.message_id != message_id)
        {
            return;
        }
        let pending = self.app_server.submissions.remove(owner).unwrap();
        if let Some(review) = pending.input.supervision_review.clone() {
            self.accept_supervisor_review(owner, turn_id, review);
        }
        // Queue admission already cleared that draft. A second acknowledgement
        // could erase new text the user wrote while the queued turn was waiting.
        if pending.acknowledge {
            self.acknowledge_submission(&pending.input);
        }
        if pending.cancelled {
            self.stop_app_server(owner);
        } else {
            self.begin_automatic_supervision(owner, turn_id);
        }
    }

    pub(super) fn fail_app_server_submission(&mut self, owner: &str, error: String) {
        let steering_receipt_owner = self.app_server.steering.remove(owner).map(|pending| {
            self.app_server
                .pending_inputs
                .remove(owner, &pending.message_id);
            pending.receipt_owner
        });
        if let Some(pending) = self.app_server.submissions.remove(owner) {
            if let Some(review) = pending.input.supervision_review.as_ref() {
                self.fail_supervisor_review(review);
            }
            if let Some(trace) = &pending.input.delivery_trace {
                trace.finish("failed");
            }
            self.app_server
                .pending_inputs
                .remove(owner, &pending.message_id);
            self.push_submission_message(&pending.input, ChatRole::System, error);
        } else {
            self.conversation_event(
                "central-agent:conversation-error",
                json!({"owner":steering_receipt_owner.as_deref().unwrap_or(owner),"error":error}),
            );
        }
    }

    pub(in crate::browser) fn stop_app_server(&mut self, owner: &str) -> bool {
        if !self.app_server.busy(owner) {
            return false;
        }
        if self.app_server.conversations.request_compaction_stop(owner)
            && !self.app_server.conversations.take_compaction_stop(owner)
        {
            self.emit_app_server_conversation(owner, "compaction_stop_requested");
            return true;
        }
        if let Some(pending) = self.app_server.reviews.get_mut(owner)
            && (pending.request_stop()
                || self.app_server.conversations.active_turn(owner).is_none())
        {
            return true;
        }
        if let Some(pending) = self.app_server.submissions.get_mut(owner) {
            pending.cancelled = true;
            // Keep cancellation across a racing turn/started/ACK pair.
            if !pending.sending {
                if let Some(trace) = &pending.input.delivery_trace {
                    trace.finish("cancelled");
                }
                self.app_server
                    .pending_inputs
                    .remove(owner, &pending.message_id);
                return true;
            }
        }
        match self
            .app_server
            .conversations
            .action(owner, ThreadAction::Interrupt)
        {
            Ok(request) => self.dispatch_app_server_thread(request),
            Err(error) if !self.app_server.submissions.contains_key(owner) => self
                .conversation_event(
                    "central-agent:conversation-error",
                    json!({"owner":owner,"error":error}),
                ),
            Err(_) => {}
        }
        self.render_agent_panel();
        true
    }

    pub(super) fn dispatch_app_server_thread(&mut self, request: ThreadRequest) {
        if matches!(
            request.call.method,
            "thread/fork" | "thread/compact/start" | "thread/revert"
        ) && let Err(error) = self.save_app_server_bindings()
        {
            let _ = self
                .app_server
                .conversations
                .complete(&request, Err(CallError::Rejected(error.clone())));
            self.conversation_event(
                "central-agent:app-server-conversation",
                json!({"owner":request.owner,"error":error}),
            );
            return;
        }
        let Some(client) = self.app_server.client.clone() else {
            let result = Err(CallError::Rejected(
                "App Server disconnected; request was not sent".into(),
            ));
            let _ = self
                .proxy
                .send_event(BrowserEvent::AppServer(Event::ConversationReply {
                    epoch: self.app_server.epoch,
                    request,
                    result,
                }));
            return;
        };
        let epoch = self.app_server.epoch;
        self.record_native_event(
            Some(&request.owner),
            "dispatch",
            request.call.method,
            &request.call.params,
            Some(request.diagnostic_id()),
        );
        let proxy = self.proxy.clone();
        let timing = self
            .app_server
            .timings
            .dispatch(&request.owner, &request.call.params);
        std::thread::spawn(move || {
            let result = request.call.clone().send(&client).and_then(|ticket| {
                if let Some(trace) = &timing {
                    trace.written();
                }
                ticket.wait()
            });
            if let Some(trace) = &timing {
                trace.reply(&result);
            }
            let _ = proxy.send_event(BrowserEvent::AppServer(Event::ConversationReply {
                epoch,
                request,
                result,
            }));
        });
    }
}

fn native_input(input: &PendingAgentSubmission) -> Result<&str, String> {
    if let Some(launch) = &input.agent_graph_launch {
        if launch.ssh_profile_id.is_some() {
            return Err(
                "Remote node tools are deferred. Select a local directory for native Codex.".into(),
            );
        }
        // Never use the legacy graph launcher's synthetic node/context prompt.
        Ok(&launch.user_request)
    } else {
        Ok(&input.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queued_native_skill_references_survive_snapshot_roundtrip_without_skill_content() {
        let temp = tempfile::tempdir().unwrap();
        let mut pending = input();
        pending.native_skills.push(SelectedSkill {
            id: "selected-a".into(),
            name: "fixture-skill".into(),
            path: temp
                .path()
                .join(".agents/skills/fixture/SKILL.md")
                .display()
                .to_string(),
            directory: temp.path().into(),
            current: true,
        });
        let encoded = serde_json::to_string(&pending).unwrap();
        let queued: PendingAgentSubmission = serde_json::from_str(&encoded).unwrap();
        pending.native_skills.clear();
        assert_eq!(queued.native_skills.len(), 1);
        let mut inputs = attachments::native_inputs(
            native_input(&queued).unwrap(),
            &queued.tab_ids,
            &queued.snapshots.tabs,
            &queued.terminal_session_ids,
            &queued.snapshots.terminals,
            &queued.file_ids,
            &queued.snapshots.files,
        )
        .unwrap();
        skills::append_skill_inputs(&mut inputs, &queued.native_skills, temp.path()).unwrap();
        for call in [
            api::start_turn(
                "thread",
                "receipt",
                inputs.clone(),
                temp.path().to_str().unwrap(),
                &api::Profile::default(),
                api::Access::ReadOnly,
            ),
            api::steer_turn("thread", "turn", "followup", inputs.clone()),
        ] {
            assert_eq!(call.params["input"][0]["text"], "Inspect only");
            assert_eq!(call.params["input"][1]["text"], "$fixture-skill");
            assert_eq!(call.params["input"][2]["type"], "skill");
            assert!(call.params["input"][2].get("instructions").is_none());
        }
    }
    #[test]
    fn queued_native_contexts_survive_snapshot_roundtrip_without_live_recapture() {
        let tab = TabContextSnapshot::from_value(
            7,
            11,
            "Page",
            "https://example.com",
            1,
            100,
            json!({"title":"Frozen tab","url":"https://example.com/docs","text":"Original page bytes"}),
        )
        .unwrap();
        let terminal = DraftTerminalContext {
            snapshot: TerminalContextSnapshot {
                session_id: 9,
                label: "Frozen terminal".into(),
                kind: TerminalKind::Local,
                profile_id: None,
                remote_target: None,
                phase: TerminalPhase::Running,
                busy: false,
                shell: "PowerShell".into(),
                cwd: "C:\\project".into(),
                status: "Ready".into(),
                output: "Original terminal bytes".into(),
                source_output_char_count: 23,
                output_revision: 1,
                last_exit_code: Some(0),
                captured_at_ms: 100,
                estimated_token_count: 54,
                redaction_count: 0,
                truncated: false,
            },
            follow_live: true,
            last_live_render_ms: 100,
            live_update_scheduled: false,
        };
        let mut pending = input();
        pending.message.clear();
        pending.tab_ids = vec![7];
        pending.terminal_session_ids = vec![9];
        pending.snapshots.tabs.push(tab);
        pending.snapshots.terminals.push(terminal);
        let queued: PendingAgentSubmission =
            serde_json::from_str(&serde_json::to_string(&pending).unwrap()).unwrap();
        let inputs = attachments::native_inputs(
            native_input(&queued).unwrap(),
            &queued.tab_ids,
            &queued.snapshots.tabs,
            &queued.terminal_session_ids,
            &queued.snapshots.terminals,
            &queued.file_ids,
            &queued.snapshots.files,
        )
        .unwrap();
        let (tabs, terminals) = attachments::native_context_views(&Value::Array(inputs));
        assert_eq!(tabs[0]["title"], "Frozen tab");
        assert_eq!(terminals[0]["outputPreview"], "Original terminal bytes");
        assert_eq!(terminals[0]["followedLive"], true);
    }
    fn input() -> PendingAgentSubmission {
        PendingAgentSubmission {
            delivery_trace: None,
            native_skills: vec![],
            native_apps: vec![],
            scope: None,
            snapshots: SubmissionSnapshots::default(),
            provider: AgentProviderKind::CodexAppServer,
            message: "Inspect only".into(),
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
    fn native_graph_input_never_injects_synthetic_history_or_custom_tools() {
        let mut input = input();
        assert_eq!(native_input(&input).unwrap(), "Inspect only");
        input.message = "Legacy prompt, supplemental instructions and imported history".into();
        input.agent_graph_launch = Some(AgentGraphLaunch {
            node_key: "node".into(),
            agent_name: "Name".into(),
            project_directory: "C:/project".into(),
            ssh_profile_id: None,
            user_request: "The user's actual instruction".into(),
        });
        assert_eq!(
            native_input(&input).unwrap(),
            "The user's actual instruction"
        );
        input.agent_graph_launch.as_mut().unwrap().ssh_profile_id = Some("remote".into());
        assert!(native_input(&input).is_err());
        input.agent_graph_launch = None;
        input.tab_ids.push(1);
        assert_eq!(
            native_input(&input).unwrap(),
            "Legacy prompt, supplemental instructions and imported history"
        );
        input.tab_ids.clear();
        input.file_ids.push("file".into());
        assert!(
            attachments::native_file_inputs(
                native_input(&input).unwrap(),
                &input.file_ids,
                &input.snapshots.files
            )
            .is_err()
        );
    }
    #[test]
    fn pending_open_is_owned_busy_work_before_a_native_thread_exists() {
        let temp = tempfile::tempdir().unwrap();
        let directory = fs::canonicalize(temp.path()).unwrap();
        let mut state = State::default();
        state.roots.insert("graph:n".into(), directory.clone());
        state.submissions.insert(
            "graph:n".into(),
            Submission {
                input: input(),
                cwd: directory,
                profile: api::Profile::default(),
                access: api::Access::ReadOnly,
                message_id: "receipt".into(),
                sending: false,
                cancelled: false,
                acknowledge: false,
            },
        );
        assert!(state.busy("graph:n"));
        assert!(state.any_busy());
        assert!(state.workspace_busy(&temp.path().display().to_string()));
        assert!(!state.busy("chat:another"));
        assert!(state.delivery_warning("graph:n").is_none());
        state.submissions.clear();
        assert!(!state.any_busy());
    }
}
