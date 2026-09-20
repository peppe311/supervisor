//! Explicit, non-visual desktop-host acceptance. Unlike the VM and transport
//! tests this drives the compiled composer, production IPC/host, and real Codex.
//! Never run by ordinary startup/build checks: inference requires CLI consent.
use super::*;
use std::sync::mpsc;
mod approvals;
mod command_policy;
// Shared deterministic model fixture; instantiated only by explicit isolated
// acceptance coordinators, never by ordinary application startup.
#[allow(dead_code)]
mod reload_responses_fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/central-agent-codex-runtime/src/transport/reload_responses_fixture.rs"
    ));
}
mod delegation;
mod delivery;
mod elicitation;
mod file_changes;
mod graph;
mod history;
mod lifecycle;
mod media;
mod providers;
mod recovery;
mod restart;
mod review_host;
mod settings;
mod usage;

const FIRST: &str = "CENTRAL_NATIVE_FIRST";
const SECOND: &str = "CENTRAL_NATIVE_SECOND";

fn retain_test_history() -> bool {
    std::env::args_os().any(|argument| argument == "--retain-test-history")
}

#[derive(Debug, PartialEq, Eq)]
enum Stage {
    Connecting,
    First,
    Second,
    Read,
    Usage,
    Done,
}

struct Check {
    app: BrowserApp,
    // Drop after BrowserApp: WebView and native-runtime handles must release first.
    _profile: Option<tempfile::TempDir>,
    stage: Stage,
    owner: String,
    native_id: Option<String>,
    samples: mpsc::Receiver<String>,
    sample_sender: mpsc::Sender<String>,
    sample_pending: bool,
    sample: Value,
    deltas: usize,
    submitted: usize,
    read_completed: bool,
    error: Option<anyhow::Error>,
    deadline: Instant,
    graph: Option<graph::Graph>,
    delivery: Option<delivery::Delivery>,
    delegation: Option<delegation::Check>,
    approvals: Option<approvals::Approvals>,
    command_policy: Option<command_policy::Check>,
    media: Option<media::Media>,
    recovery: Option<recovery::Recovery>,
    providers: Option<providers::Providers>,
    restart: Option<restart::Restart>,
    elicitation: Option<elicitation::Elicitation>,
    lifecycle: Option<lifecycle::Lifecycle>,
    review: Option<review_host::Review>,
    history: Option<history::History>,
    settings: Option<settings::Settings>,
    usage: Option<usage::Usage>,
}

impl Check {
    fn script(&self, script: &str) -> anyhow::Result<()> {
        self.app
            .agent_panel
            .as_ref()
            .context("Missing test panel")?
            .evaluate_script(script)
            .map_err(Into::into)
    }

    fn send_prompt(&mut self, marker: &str) -> anyhow::Result<()> {
        let prompt = format!(
            "Reply with exactly {marker}. Do not use tools, read files, or change anything."
        );
        // Real Svelte input + Enter handlers; no fabricated submission or ACK.
        self.script(&format!(r#"(() => {{
            const input = document.getElementById('chat-input');
            if (!input || input.disabled) throw new Error('Native composer is not usable');
            input.value = {};
            input.dispatchEvent(new Event('input', {{bubbles:true}}));
            input.dispatchEvent(new KeyboardEvent('keydown', {{key:'Enter',bubbles:true,cancelable:true}}));
        }})()"#, json!(prompt)))?;
        self.sample = Value::Null;
        println!("Desktop host: submitted {marker} through composer Enter.");
        Ok(())
    }

    fn tick(&mut self) -> anyhow::Result<()> {
        if self.stage == Stage::Done {
            return Ok(());
        }
        while let Ok(raw) = self.samples.try_recv() {
            self.sample_pending = false;
            self.sample =
                serde_json::from_str(&raw).context("Invalid WebView acceptance sample")?;
        }
        if !self.app.agent_panel_ready
            || self.graph.is_some() && !self.app.agent_graph_surface_ready
        {
            return Ok(());
        }
        if !self.sample_pending {
            let sender = self.sample_sender.clone();
            let panel = if self.graph.is_some() {
                &self.app.agent_graph_surface
            } else {
                &self.app.agent_panel
            };
            let script = if let Some(delegation) = &self.delegation {
                delegation.sample(&self.owner)
            } else if self.settings.is_some() {
                settings::sample(&self.owner)
            } else if self.review.is_some() {
                review_host::sample(&self.owner)
            } else if self.history.is_some() {
                history::sample(&self.owner)
            } else if let Some(lifecycle) = &self.lifecycle {
                lifecycle.sample(&self.owner)
            } else if self.command_policy.is_some() {
                command_policy::sample(&self.owner)
            } else if self.elicitation.is_some() {
                elicitation::sample(&self.owner)
            } else if self.restart.is_some() {
                restart::sample(&self.owner)
            } else if self.providers.is_some() {
                providers::sample(&self.owner)
            } else if self.recovery.is_some() {
                recovery::sample(&self.owner)
            } else if self.media.is_some() {
                media::sample(&self.owner)
            } else if self.approvals.is_some() {
                approvals::sample(&self.owner)
            } else if self.delivery.is_some() {
                delivery::sample(&self.owner)
            } else if self.graph.is_some() {
                graph::SAMPLE.to_owned()
            } else {
                r#"({renderer:typeof window.renderAgentPanelState,
                    draft:document.getElementById('chat-input')?.value,
                    text:[...document.querySelectorAll('#chat-messages .chat-message.assistant:not(.reasoning):not(.activity)')].map(row=>row.textContent).join('\n'),
                    active:document.getElementById('composer')?.dataset.agentActive,
                    errors:window.__nativeAcceptanceErrors || []})"#.to_owned()
            };
            let script = if self.usage.is_some() {
                usage::sample(&script, &self.owner, self.graph.is_some())
            } else {
                script
            };
            panel
                .as_ref()
                .unwrap()
                .evaluate_script_with_callback(&script, move |raw| {
                    let _ = sender.send(raw);
                })?;
            self.sample_pending = true;
        }
        anyhow::ensure!(
            self.sample["errors"].as_array().is_none_or(Vec::is_empty),
            "Desktop JavaScript failed: {}",
            self.sample["errors"]
        );
        if self.stage == Stage::Usage {
            if self
                .usage
                .as_mut()
                .context("Missing usage acceptance")?
                .tick(&mut self.app, &self.sample, self.graph.is_some())?
            {
                self.stage = Stage::Done;
            }
            return Ok(());
        }
        if let Some(settings) = &mut self.settings {
            if settings.tick(
                &mut self.app,
                &self.owner,
                &self.sample,
                self.graph.as_ref(),
            )? {
                self.stage = Stage::Done;
            }
            return Ok(());
        }
        if let Some(review) = &mut self.review {
            if review.tick(
                &mut self.app,
                &self.owner,
                &self.sample,
                self.graph.as_ref(),
            )? {
                self.stage = Stage::Done;
            }
            return Ok(());
        }
        if let Some(delegation) = &mut self.delegation {
            if delegation.tick(
                &mut self.app,
                &self.owner,
                &self.sample,
                self.graph.as_ref(),
            )? {
                self.stage = Stage::Done;
            }
            return Ok(());
        }
        if let Some(history) = &mut self.history {
            if history.tick(
                &mut self.app,
                &self.owner,
                &self.sample,
                self.graph.as_ref(),
            )? {
                self.stage = Stage::Done;
            }
            return Ok(());
        }
        if let Some(lifecycle) = &mut self.lifecycle {
            if lifecycle.tick(
                &mut self.app,
                &self.owner,
                &self.sample,
                self.graph.as_ref(),
            )? {
                self.stage = Stage::Done;
            }
            return Ok(());
        }
        if let Some(policy) = &mut self.command_policy {
            if policy.tick(
                &mut self.app,
                &self.owner,
                &self.sample,
                self.graph.as_ref(),
            )? {
                self.stage = Stage::Done;
            }
            return Ok(());
        }
        if let Some(elicitation) = &mut self.elicitation {
            if elicitation.tick(
                &mut self.app,
                &self.owner,
                &self.sample,
                self.graph.as_ref(),
            )? {
                self.stage = Stage::Done;
            }
            return Ok(());
        }
        if let Some(restart) = &mut self.restart {
            if restart.tick(
                &mut self.app,
                &self.owner,
                &self.sample,
                self.graph.as_ref(),
            )? {
                self.stage = Stage::Done;
            }
            return Ok(());
        }
        if let Some(providers) = &mut self.providers {
            if providers.tick(
                &mut self.app,
                &self.owner,
                &self.sample,
                self.graph.as_ref(),
            )? {
                self.stage = Stage::Done;
            }
            return Ok(());
        }
        if let Some(recovery) = &mut self.recovery {
            if recovery.tick(
                &mut self.app,
                &self.owner,
                &self.sample,
                self.graph.as_ref(),
            )? {
                self.stage = Stage::Done;
            }
            return Ok(());
        }
        if let Some(media) = &mut self.media {
            if media.tick(
                &mut self.app,
                &self.owner,
                &self.sample,
                self.graph.as_ref(),
            )? {
                self.stage = Stage::Done;
            }
            return Ok(());
        }
        if let Some(approvals) = &mut self.approvals {
            if approvals.tick(
                &mut self.app,
                &self.owner,
                &self.sample,
                self.graph.as_ref(),
            )? {
                self.stage = Stage::Done;
            }
            return Ok(());
        }
        if let Some(delivery) = &mut self.delivery {
            if delivery.tick(
                &mut self.app,
                &self.owner,
                &self.sample,
                self.graph.as_ref(),
            )? {
                self.stage = Stage::Done;
            }
            return Ok(());
        }
        if let Some(graph) = &mut self.graph {
            if graph.tick(&mut self.app, &self.sample)? {
                self.stage = if self.usage.is_some() {
                    Stage::Usage
                } else {
                    Stage::Done
                };
            }
            return Ok(());
        }
        if let Some(binding) = self.app.app_server.conversations.binding(&self.owner) {
            match &self.native_id {
                Some(id) => anyhow::ensure!(
                    *id == binding.thread_id,
                    "Continuation changed the native thread"
                ),
                None => self.native_id = Some(binding.thread_id.clone()),
            }
        }
        if self.stage == Stage::Connecting {
            if self.sample.is_null() {
                return Ok(());
            }
            anyhow::ensure!(
                self.sample["renderer"] == "function",
                "Pristine panel has no state renderer; no inference was submitted"
            );
            if !self.app.app_server.configuration().0.can_run() {
                return Ok(());
            }
            self.app.render_agent_panel();
            self.send_prompt(FIRST)?;
            self.stage = Stage::First;
            return Ok(());
        }
        let Some(thread) = self
            .native_id
            .as_deref()
            .and_then(|id| self.app.app_server.conversations.mirror.thread(id))
        else {
            return Ok(());
        };
        anyhow::ensure!(
            thread.turns.iter().all(|turn| turn.status != "failed"),
            "Native test turn failed: {:?}",
            thread
                .turns
                .iter()
                .filter_map(|turn| turn.error.as_ref())
                .collect::<Vec<_>>()
        );
        for item in thread.turns.iter().flat_map(|turn| &turn.items) {
            anyhow::ensure!(
                matches!(
                    item.value["type"].as_str(),
                    Some("userMessage" | "agentMessage" | "reasoning" | "plan")
                ),
                "No-tool acceptance received unexpected native item {}",
                item.value["type"]
            );
        }
        if self.app.app_server.busy(&self.owner) {
            return Ok(());
        }
        let count = if self.stage == Stage::First { 1 } else { 2 };
        if thread.turns.len() != count || thread.turns.iter().any(|turn| turn.status != "completed")
        {
            return Ok(());
        }
        for (turn, marker) in thread.turns.iter().zip([FIRST, SECOND]) {
            anyhow::ensure!(
                turn.items
                    .iter()
                    .any(|item| item.value["type"] == "agentMessage"
                        && item.value["text"]
                            .as_str()
                            .is_some_and(|text| text.trim() == marker)),
                "Native final answer did not match the acceptance marker"
            );
        }
        let visible = self.sample["text"].as_str().unwrap_or_default();
        if self.sample["draft"] != ""
            || self.sample["active"] == "true"
            || !visible.contains(FIRST)
            || (count == 2 && !visible.contains(SECOND))
        {
            return Ok(());
        }
        match self.stage {
            Stage::First => {
                println!(
                    "Desktop host: native first turn completed; draft cleared by ACK and answer rendered."
                );
                self.send_prompt(SECOND)?;
                self.stage = Stage::Second;
            }
            Stage::Second => {
                anyhow::ensure!(
                    self.submitted == 2 && self.deltas > 0,
                    "Missing actual native submissions/stream deltas"
                );
                self.read_completed = false;
                self.app
                    .app_server_conversation(self.owner.clone(), ConversationAction::Read {});
                self.stage = Stage::Read;
            }
            Stage::Read if self.read_completed => {
                anyhow::ensure!(
                    self.app
                        .app_server
                        .conversations
                        .saved()
                        .unresolved
                        .is_empty(),
                    "Accepted turns retained uncertain receipts"
                );
                anyhow::ensure!(
                    self.app
                        .project_chats
                        .iter()
                        .all(|chat| chat.messages.is_empty()),
                    "Codex transcript was copied into another provider's history"
                );
                println!(
                    "Desktop host: same native thread, two completed turns, streamed delta, native readback and rendered answers verified."
                );
                self.stage = if self.usage.is_some() {
                    Stage::Usage
                } else {
                    Stage::Done
                };
            }
            _ => {}
        }
        Ok(())
    }

    fn cleanup(&mut self) -> anyhow::Result<()> {
        let has_owned_history = !self
            .app
            .app_server
            .conversations
            .saved()
            .bindings
            .is_empty();
        if !has_owned_history && self.app.app_server.client.is_none() {
            return Ok(());
        }
        let connected = self.app.app_server.view.connected && self.app.app_server.client.is_some();
        let client = if connected {
            self.app.app_server.client.as_ref().unwrap().clone()
        } else {
            // A failed crash test must still remove only its exact disposable
            // history. A new native handshake is not history replay or inference.
            let home = self
                .app
                .app_server
                .acceptance_home
                .clone()
                .unwrap_or_else(|| self.app.data_dir.join("codex"));
            let runtime = Runtime::discover(&self.app.data_dir)
                .and_then(|runtime| runtime.with_home(&home))
                .map_err(anyhow::Error::msg)?;
            let (sender, events) = mpsc::channel();
            let client = Client::spawn(&runtime, env!("CARGO_PKG_VERSION"), move |event| {
                let _ = sender.send(event);
            })
            .map_err(anyhow::Error::msg)?;
            match events.recv_timeout(Duration::from_secs(30)) {
                Ok(TransportEvent::Ready { .. }) => client,
                result => {
                    client.shutdown();
                    anyhow::bail!("Owned-history cleanup handshake failed: {result:?}");
                }
            }
        };
        // IDs come only from this empty local test store; no list/search of any
        // personal history. On failure interrupt this owned turn before delete.
        let mut owners: Vec<_> = self
            .app
            .app_server
            .conversations
            .saved()
            .bindings
            .keys()
            .cloned()
            .collect();
        if self.lifecycle.is_some() {
            // The new native fork references its source's persisted history.
            // On test failure remove the exact owned fork before its source.
            owners.sort_by_key(|owner| owner == &self.owner);
        }
        let mut failures = Vec::new();
        for owner in owners {
            let result: anyhow::Result<()> = (|| {
                if let Some(binding) = self.app.app_server.conversations.binding(&owner) {
                    if binding.deleted {
                        return Ok(());
                    }
                    let id = &binding.thread_id;
                    if connected
                        && let Some(turn) = self
                            .app
                            .app_server
                            .conversations
                            .mirror
                            .thread(id)
                            .and_then(|thread| thread.active_turn())
                    {
                        api::interrupt_turn(id, &turn.id).send(&client)?.wait()?;
                    }
                    if retain_test_history() {
                        println!(
                            "Desktop host: retained owned test history {id} by explicit --retain-test-history."
                        );
                    } else {
                        api::delete_thread(id).send(&client)?.wait()?;
                        println!("Desktop host: deleted its owned native test thread {id}.");
                    }
                }
                Ok(())
            })();
            if let Err(error) = result {
                failures.push(format!("{owner}: {error}"));
            }
        }
        client.shutdown();
        anyhow::ensure!(failures.is_empty(), "{}", failures.join("; "));
        Ok(())
    }
}

impl ApplicationHandler<BrowserEvent> for Check {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.app.window.is_some() {
            return;
        }
        let result = (|| -> anyhow::Result<()> {
            self.app.window = Some(
                event_loop.create_window(
                    Window::default_attributes()
                        .with_visible(false)
                        .with_title("Native conversation acceptance")
                        .with_inner_size(LogicalSize::new(1280.0, 900.0)),
                )?,
            );
            self.app.ui_context = Some(WebContext::new(Some(self.app.data_dir.join("webview"))));
            self.app.build_agent_panel()?;
            if self.graph.is_some() {
                self.app.agent_graph_open = true;
                self.app.build_agent_graph_surface()?;
            }
            self.app.app_server_account(AccountAction::Connect);
            Ok(())
        })();
        if let Err(error) = result {
            self.error = Some(error);
            event_loop.exit();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: BrowserEvent) {
        let event = match self.review.as_mut() {
            Some(review) => match review.defer_preparation(&self.owner, event) {
                Some(event) => event,
                None => return,
            },
            None => event,
        };
        if let Some(review) = &mut self.review {
            review.observe(&self.app, &self.owner, &event);
        }
        if let Some(usage) = &mut self.usage {
            usage.observe(&self.app, &event);
        }
        if let Some(settings) = &mut self.settings {
            settings.observe(&self.owner, &event);
            match settings.intercept(&self.app, &self.owner, &event) {
                Ok(true) => return,
                Ok(false) => {}
                Err(error) => {
                    self.error = Some(error);
                    event_loop.exit();
                    return;
                }
            }
        }
        if let Some(history) = &mut self.history {
            history.observe(&self.owner, &event);
        }
        if let Some(lifecycle) = &mut self.lifecycle {
            lifecycle.observe(&self.owner, &event);
        }
        if let Some(elicitation) = &mut self.elicitation {
            elicitation.observe(&self.app, &self.owner, &event);
        }
        if let Some(restart) = &mut self.restart {
            restart.observe(&self.owner, &event);
        }
        if let Some(providers) = &mut self.providers {
            providers.observe(&self.owner, &event);
        }
        if let Some(recovery) = &mut self.recovery {
            recovery.observe(&self.app, &self.owner, &event);
        }
        let event = match self.recovery.as_mut() {
            Some(recovery) => match recovery.defer_reply(event) {
                Some(event) => event,
                None => return,
            },
            None => event,
        };
        let event = match self.restart.as_mut() {
            Some(restart) => match restart.defer_reply(&self.owner, event) {
                Some(event) => event,
                None => return,
            },
            None => event,
        };
        if let Some(media) = &mut self.media {
            media.observe(&mut self.app, &self.owner, &event);
        }
        if let Some(graph) = &mut self.graph {
            graph.observe(&self.app, &event);
        }
        if let Some(delivery) = &mut self.delivery {
            delivery.observe(&self.app, &self.owner, &event);
        }
        if let Some(approvals) = &mut self.approvals {
            approvals.observe(&self.app, &self.owner, &event);
        }
        if let Some(policy) = &mut self.command_policy {
            policy.observe(&self.app, &self.owner, &event);
        }
        match &event {
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::Notification { method, params },
                ..
            }) if method == "item/agentMessage/delta"
                && self
                    .app
                    .app_server
                    .conversations
                    .binding(&self.owner)
                    .is_some_and(|binding| params["threadId"] == binding.thread_id) =>
            {
                self.deltas += 1
            }
            BrowserEvent::AppServer(Event::ConversationReply {
                request, result, ..
            }) if request.call.method == "thread/read"
                && result.is_ok()
                && request.owner == self.owner =>
            {
                self.read_completed = true
            }
            BrowserEvent::ScopedAgentPanel {
                owner,
                message: AgentPanelMessage::SubmitChat { .. },
                ..
            } if *owner == self.owner => self.submitted += 1,
            _ => {}
        }
        match event {
            BrowserEvent::AgentGraphSurfaceReady if self.graph.is_some() => {
                self.app.agent_graph_surface_ready = true;
                let result = self.app.agent_graph_surface.as_ref().unwrap().evaluate_script("window.__nativeAcceptanceErrors=[];window.addEventListener('error',event=>window.__nativeAcceptanceErrors.push(event.message));");
                if let Err(error) = result {
                    self.error = Some(error.into());
                    event_loop.exit();
                    return;
                }
                self.app.render_agent_graph_surface();
            }
            event @ BrowserEvent::AgentPanel(_) if self.graph.is_some() => {
                self.app.user_event(event_loop, event)
            }
            BrowserEvent::AgentPanelReady => {
                self.app.agent_panel_ready = true;
                if let Err(error) = self.script("window.__nativeAcceptanceErrors=[];window.addEventListener('error',event=>window.__nativeAcceptanceErrors.push(event.message));") {
                    self.error = Some(error); event_loop.exit(); return;
                }
                self.app.render_agent_panel();
            }
            // Same production dispatch and immutable-owner checks. Do not run
            // normal resumed/about_to_wait: no browser tabs, SSH maps or providers.
            event @ (BrowserEvent::AppServer(_) | BrowserEvent::ScopedAgentPanel { .. }) => {
                self.app.user_event(event_loop, event)
            }
            _ => {}
        }
    }
    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let result = self.tick().and_then(|()| {
            anyhow::ensure!(Instant::now() < self.deadline,
                "Native acceptance timed out at {:?}; submissions={}, deltas={}, UI={}, account errors={:?}, recovery={}",
                self.stage, self.submitted, self.deltas, self.sample, self.app.app_server.view.errors,
                self.recovery.as_ref().map(|recovery|recovery.diagnostics(&self.app,&self.owner)).unwrap_or(Value::Null));
            Ok(())
        });
        if let Err(error) = result {
            self.error = Some(error);
            event_loop.exit();
        } else if self.stage == Stage::Done {
            if self.restart.is_some() {
                let window = self.app.window.as_ref().unwrap().id();
                self.app
                    .window_event(event_loop, window, WindowEvent::CloseRequested);
            } else {
                event_loop.exit();
            }
        } else {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(50),
            ));
        }
    }
}

pub(crate) fn run() -> anyhow::Result<()> {
    let delegation_mode = std::env::args_os().any(|arg| arg == "--delegation");
    if delegation_mode {
        anyhow::ensure!(
            std::env::args_os().any(|arg| arg == "--allow-test-inference") && retain_test_history(),
            "Delegation acceptance requires --allow-test-inference and --retain-test-history"
        );
    }
    let review_mode = std::env::args_os().any(|arg| arg == "--review");
    let review_stop = std::env::args_os().any(|arg| arg == "--review-stop");
    let review_prepare_stop = std::env::args_os().any(|arg| arg == "--review-prepare-stop");
    let review_approval_stop = std::env::args_os().any(|arg| arg == "--review-approval-stop");
    let review_command_stop = std::env::args_os().any(|arg| arg == "--review-command-stop");
    anyhow::ensure!(
        !review_command_stop
            || (review_mode && !review_stop && !review_prepare_stop && !review_approval_stop),
        "--review-command-stop requires --review without other Stop modes"
    );
    anyhow::ensure!(
        !review_approval_stop || (review_mode && !review_stop && !review_prepare_stop),
        "--review-approval-stop requires --review and cannot combine with another Stop mode"
    );
    anyhow::ensure!(
        !review_prepare_stop || (review_mode && !review_stop),
        "--review-prepare-stop requires --review and cannot combine with --review-stop"
    );
    let review_git =
        std::env::args().find_map(|arg| arg.strip_prefix("--review-git=").map(str::to_owned));
    anyhow::ensure!(
        review_git.as_deref().is_none_or(|mode| review_mode
            && !review_stop
            && !review_prepare_stop
            && !review_approval_stop
            && !review_command_stop
            && ["uncommitted", "branch", "commit"].contains(&mode)),
        "--review-git=uncommitted|branch|commit requires --review without a Stop test"
    );
    anyhow::ensure!(
        !review_stop || review_mode,
        "--review-stop requires --review"
    );
    if review_mode {
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        anyhow::ensure!(
            args.iter()
                .filter(|a| a.to_str().is_some_and(|s| s.starts_with("--review-git=")))
                .count()
                <= 1,
            "Choose exactly one Git review target per acceptance run"
        );
        anyhow::ensure!(
            args.iter().any(|a| a == "--check-native-conversation")
                && args.iter().any(|a| a == "--allow-test-inference")
                && args.iter().all(|a| a == "--check-native-conversation"
                    || a == "--allow-test-inference"
                    || a == "--retain-test-history"
                    || a == "--review"
                    || a == "--review-stop"
                    || a == "--review-prepare-stop"
                    || a == "--review-approval-stop"
                    || a == "--review-command-stop"
                    || a.to_str().is_some_and(|a| matches!(
                        a,
                        "--review-git=uncommitted" | "--review-git=branch" | "--review-git=commit"
                    ))
                    || a == "--graph"),
            "--review requires --check-native-conversation --allow-test-inference; optional --graph and one of --review-stop, --review-prepare-stop, --review-command-stop, --review-approval-stop or --review-git=uncommitted|branch|commit; one seed prompt uses account allowance, plus one native review unless preparation is cancelled"
        );
    }
    let command_policy_mode = std::env::args_os().any(|arg| arg == "--check-native-command-policy");
    if command_policy_mode {
        anyhow::ensure!(
            cfg!(windows),
            "Command policy desktop fixture currently requires Windows"
        );
        anyhow::ensure!(
            std::env::args_os()
                .skip(1)
                .all(|a| a == "--check-native-command-policy" || a == "--graph"),
            "--check-native-command-policy accepts only optional --graph; no account-inference mode may be mixed in"
        );
    }
    let graph_mode = std::env::args_os().any(|arg| arg == "--graph");
    let usage_mode = std::env::args_os().any(|arg| arg == "--usage");
    anyhow::ensure!(
        !usage_mode || std::env::args_os().any(|arg| arg == "--check-native-conversation"),
        "--usage requires --check-native-conversation"
    );
    let settings_mode = std::env::args_os().any(|arg| arg == "--check-native-settings");
    if std::env::args_os().any(|arg| arg == "--native-goal") {
        anyhow::ensure!(
            settings_mode
                && std::env::args_os()
                    .skip(1)
                    .all(|arg| arg == "--check-native-settings"
                        || arg == "--native-goal"
                        || arg == "--graph"),
            "--native-goal accepts only --check-native-settings and optional --graph"
        );
    }
    let mcp_settings = std::env::args_os().any(|arg| arg == "--mcp-settings");
    let mcp_oauth = std::env::args_os().any(|arg| arg == "--mcp-oauth");
    let scoped_oauth = std::env::args_os().any(|arg| arg == "--conversation-oauth");
    let mcp_options = std::env::args_os().any(|arg| arg == "--mcp-options");
    anyhow::ensure!(
        !mcp_options
            || settings_mode
                && !graph_mode
                && !mcp_settings
                && !mcp_oauth
                && !std::env::args_os().any(|a| a == "--draft-persistence"),
        "--mcp-options requires --check-native-settings without graph, OAuth, MCP CRUD or draft modes"
    );
    anyhow::ensure!(
        !scoped_oauth || mcp_oauth,
        "--conversation-oauth requires --mcp-oauth"
    );
    anyhow::ensure!(
        !mcp_oauth || settings_mode && (!graph_mode || scoped_oauth) && !mcp_settings,
        "--mcp-oauth requires --check-native-settings, no --mcp-settings, and --conversation-oauth for --graph"
    );
    anyhow::ensure!(
        !mcp_settings || settings_mode && !graph_mode,
        "--mcp-settings requires --check-native-settings without --graph (shared Settings surface)"
    );
    let settings_root = if settings_mode {
        settings::context()?
    } else {
        None
    };
    if settings_mode && settings_root.is_none() {
        return settings::coordinate(graph_mode, mcp_settings, mcp_oauth);
    }
    let history_mode = std::env::args_os().any(|arg| arg == "--check-native-history");
    let history_fork = std::env::args_os().any(|arg| arg == "--fork-history");
    anyhow::ensure!(
        !history_mode || std::env::args_os().any(|arg| arg == "--allow-test-inference"),
        "--check-native-history uses two short native prompts; add --allow-test-inference explicitly"
    );
    anyhow::ensure!(
        !history_fork || history_mode,
        "--fork-history requires --check-native-history"
    );
    let history_root = if history_mode {
        history::context()?
    } else {
        None
    };
    if history_mode && history_root.is_none() {
        return history::coordinate(graph_mode, history_fork);
    }
    let elicitation_mode = std::env::args_os().any(|arg| arg == "--check-native-mcp-requests");
    anyhow::ensure!(
        !elicitation::in_turn_enabled() || elicitation_mode,
        "--in-turn requires --check-native-mcp-requests"
    );
    let elicitation_root = if elicitation_mode || command_policy_mode {
        elicitation::context()?
    } else {
        None
    };
    if (elicitation_mode || command_policy_mode) && elicitation_root.is_none() {
        return elicitation::coordinate(graph_mode, command_policy_mode);
    }
    let delivery_mode = std::env::args_os().any(|arg| arg == "--delivery");
    let approval_mode = std::env::args_os().any(|arg| arg == "--approvals");
    let file_change = std::env::args_os().any(|arg| arg == "--file-change");
    anyhow::ensure!(
        !file_change || approval_mode,
        "--file-change requires --approvals"
    );
    let media_mode = std::env::args_os().any(|arg| arg == "--files");
    let (media_format, media_delivery) = media::options(
        media_mode,
        std::env::args_os().any(|arg| arg == "--files-png"),
        std::env::args_os().any(|arg| arg == "--files-queue"),
        std::env::args_os().any(|arg| arg == "--files-steer"),
    )?;
    let recovery_mode = std::env::args_os().any(|arg| arg == "--crash");
    let provider_mode = std::env::args_os().any(|arg| arg == "--providers");
    let profile_mode = std::env::args_os().any(|arg| arg == "--profiles");
    anyhow::ensure!(
        !profile_mode || provider_mode,
        "--profiles requires --providers"
    );
    let lifecycle_mode = std::env::args_os().any(|arg| arg == "--lifecycle");
    let restart_mode = std::env::args_os().any(|arg| arg == "--restart");
    let restart_wire = restart::WireMode::parse(std::env::args())?;
    let restart_host_ack = std::env::args_os().any(|arg| arg == "--restart-lost-ack");
    anyhow::ensure!(
        restart_wire.is_none() || !restart_host_ack,
        "Wire loss cannot combine with a held host ACK"
    );
    let restart_lost_ack = restart_host_ack || restart_wire.is_some();
    let restart_crash = std::env::args_os().any(|arg| arg == "--restart-crash");
    let restart_active = restart_crash && !restart_lost_ack
        || std::env::args_os().any(|arg| arg == "--restart-active");
    anyhow::ensure!(
        !restart_lost_ack || restart_mode && restart_crash && !restart_active,
        "--restart-lost-ack requires --restart --restart-crash without --restart-active"
    );
    anyhow::ensure!(
        !restart_active || restart_mode,
        "--restart-active/--restart-crash require --restart"
    );
    let lost_ack = std::env::args_os().any(|arg| arg == "--lost-ack");
    anyhow::ensure!(!lost_ack || recovery_mode, "--lost-ack requires --crash");
    let choice = approvals::Choice::parse(
        approval_mode,
        std::env::args_os().any(|arg| arg == "--decline"),
        std::env::args_os().any(|arg| arg == "--cancel"),
        std::env::args_os().any(|arg| arg == "--session"),
    )?;
    anyhow::ensure!(
        [
            delivery_mode,
            delegation_mode,
            approval_mode,
            media_mode,
            recovery_mode,
            provider_mode,
            lifecycle_mode,
            review_mode,
            restart_mode,
            elicitation_mode,
            command_policy_mode,
            history_mode,
            settings_mode,
            usage_mode
        ]
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
            <= 1,
        "Choose only one of --delivery, --approvals, --files, --crash, --providers, --lifecycle or --restart, optionally with --graph"
    );
    let restart_context = if restart_mode {
        restart::Context::from_environment()?
    } else {
        None
    };
    if restart_mode && restart_context.is_none() {
        return restart::coordinate(
            graph_mode,
            restart_active,
            restart_crash,
            restart_lost_ack,
            restart_wire,
        );
    }
    let restoring = restart_context
        .as_ref()
        .is_some_and(|context| context.second)
        || settings_mode && settings::drafts_restoring()?;
    let profile = if restart_context.is_none()
        && !elicitation_mode
        && !command_policy_mode
        && !history_mode
        && !settings_mode
    {
        Some(
            tempfile::Builder::new()
                .prefix("central-native-host-")
                .tempdir()?,
        )
    } else {
        None
    };
    let profile_root = restart_context
        .as_ref()
        .map(|context| context.root.clone())
        .or(elicitation_root)
        .or(history_root)
        .or(settings_root)
        .or_else(|| profile.as_ref().map(|profile| profile.path().to_path_buf()))
        .unwrap();
    let event_loop = winit::event_loop::EventLoop::<BrowserEvent>::with_user_event().build()?;
    let mut app =
        BrowserApp::with_data_dir(event_loop.create_proxy(), profile_root.join("data"), false)?;
    let workspace = profile_root.join("workspace");
    if !restoring {
        fs::create_dir(&workspace)?;
        app.workspace
            .connect(workspace)
            .map_err(anyhow::Error::msg)?;
        app.ensure_active_project_chat();
        app.agent_provider = AgentProviderKind::CodexAppServer;
    } else {
        anyhow::ensure!(
            app.workspace
                .root()
                .and_then(|path| fs::canonicalize(path).ok())
                == Some(fs::canonicalize(workspace)?)
                && app.agent_provider == AgentProviderKind::CodexAppServer,
            "Production loader lost workspace/provider selection"
        );
    }
    let owner = app.conversation_key(None);
    anyhow::ensure!(
        app.project_chats.len() == 1
            && (app.app_server.conversations.saved().unresolved.is_empty()
                || restoring
                    && restart_lost_ack
                    && app.app_server.conversations.saved().unresolved.len() == 1),
        "Acceptance requires a fresh store or the exact pending-receipt restart fixture"
    );
    let (sample_sender, samples) = mpsc::channel();
    let graph = if graph_mode {
        if restoring {
            let fixture =
                serde_json::from_slice(&fs::read(profile_root.join("restart-graph.json"))?)?;
            Some(graph::Graph::restore_fixture(
                &app,
                &profile_root,
                &fixture,
            )?)
        } else {
            let graph = graph::Graph::prepare(&mut app, &profile_root)?;
            if restart_mode
                || settings_mode && std::env::args_os().any(|a| a == "--draft-persistence")
            {
                fs::write(
                    profile_root.join("restart-graph.json"),
                    serde_json::to_vec(&graph.restart_fixture())?,
                )?;
            }
            Some(graph)
        }
    } else {
        None
    };
    let owner = if approval_mode
        || delivery_mode
        || delegation_mode
        || media_mode
        || recovery_mode
        || provider_mode
        || lifecycle_mode
        || review_mode
        || restart_mode
        || elicitation_mode
        || command_policy_mode
        || history_mode
        || settings_mode
    {
        graph
            .as_ref()
            .map_or_else(|| owner.clone(), |graph| graph.owners()[0].clone())
    } else {
        owner
    };
    let restart = restart_context
        .map(|context| {
            restart::Restart::new(
                context,
                &app,
                &owner,
                restart_active,
                restart_crash,
                restart_lost_ack,
                restart_wire,
            )
        })
        .transpose()?;
    if restart_mode && !restoring {
        // Fixture setup represents an already saved user project. Nothing is
        // flushed by the test once the native prompt starts or before a crash.
        app.save_session();
    }
    let mut check = Check {
        app,
        _profile: profile,
        owner,
        stage: Stage::Connecting,
        native_id: None,
        samples,
        sample_sender,
        sample_pending: false,
        sample: Value::Null,
        deltas: 0,
        submitted: 0,
        read_completed: false,
        error: None,
        // Harness deadline only; no timeout was added to actual agent commands.
        deadline: Instant::now() + Duration::from_secs(180),
        graph,
        delivery: delivery_mode.then(delivery::Delivery::default),
        delegation: delegation_mode.then(delegation::Check::default),
        approvals: approval_mode.then(|| approvals::Approvals::new(choice, file_change)),
        command_policy: command_policy_mode.then(command_policy::Check::default),
        media: media_mode.then(|| media::Media::new(media_format, media_delivery)),
        recovery: recovery_mode.then(|| recovery::Recovery::new(lost_ack)),
        providers: provider_mode.then(|| providers::Providers::new(profile_mode)),
        restart,
        elicitation: elicitation_mode.then(elicitation::Elicitation::new),
        lifecycle: lifecycle_mode.then(lifecycle::Lifecycle::default),
        review: review_mode.then(|| {
            review_host::Review::new(
                review_stop,
                review_prepare_stop,
                review_approval_stop,
                review_command_stop,
                review_git.clone(),
            )
        }),
        history: history_mode.then(|| history::History::new(history_fork)),
        usage: usage_mode.then(usage::Usage::default),
        settings: settings_mode
            .then(|| settings::Settings::new(mcp_settings, mcp_oauth))
            .transpose()?,
    };
    let run_result = event_loop.run_app(&mut check);
    if let Some(error) = &check.error {
        eprintln!("Native acceptance failure before cleanup: {error:#}");
    }
    let cleanup = if let Some(settings) = &check.settings {
        settings.cleanup(&check.app);
        Ok(())
    } else if let Some(history) = &mut check.history {
        history.cleanup(&check.app)
    } else if check.restart.is_some()
        || check.elicitation.is_some()
        || check.command_policy.is_some()
    {
        // The parent owns deletion only after both application processes exit.
        if let Some(client) = &check.app.app_server.client {
            client.shutdown();
        }
        Ok(())
    } else {
        check.cleanup()
    };
    run_result?;
    cleanup.context("Native acceptance cleanup failed; do not claim a clean test run")?;
    if let Some(error) = check.error.take() {
        return Err(error);
    }
    anyhow::ensure!(
        check.stage == Stage::Done,
        "Native host acceptance was incomplete"
    );
    if delegation_mode {
        println!(
            "PASS: native delegation through compiled main/graph composer, distinct local child, exact ownership and durable ancestry, child Stop at approval and same-thread continuation, parent verification, metadata journal; Luna low Standard. Nonvisual acceptance; native test history retained."
        );
    } else if let Some(mode) = &review_git {
        println!(
            "PASS: {} native {mode} Git review through actual target dialog and IPC, successful native source inspection, one rendered result and persisted history; fixture files/HEAD/index and drafts unchanged; one seed and one opted-in review, no visual tests.",
            if graph_mode { "graph" } else { "main" }
        );
    } else if review_command_stop {
        println!(
            "PASS: {} actual Stop during a running native review command; scoped native interrupt, streamed running marker, persisted interrupted review/worker and native-evidenced display items, no running activity, unchanged workspace/drafts; one seed and one opted-in review, no visual tests.",
            if graph_mode { "graph" } else { "main" }
        );
    } else if review_approval_stop {
        println!(
            "PASS: {} actual Stop during native review command approval; exact scoped turn/interrupt, native request resolution, cleared card, persisted interrupted history, unchanged workspace/drafts; no command consent granted, no visual tests.",
            if graph_mode { "graph" } else { "main" }
        );
    } else if review_prepare_stop {
        println!(
            "PASS: {} actual Stop during held native review preparation; exact reply released without review/start or turn/interrupt, unchanged native seed/workspace/drafts; one opted-in seed prompt, no visual tests.",
            if graph_mode { "graph" } else { "main" }
        );
    } else if review_stop {
        println!(
            "PASS: {} actual review Stop button, scoped native turn/interrupt, native terminal event and persisted interrupted history, unchanged seed and owner/sibling drafts; one seed prompt and one opted-in review, no visual tests.",
            if graph_mode { "graph" } else { "main" }
        );
    } else if check.review.is_some() {
        println!(
            "PASS: {} actual native review dialog cancellation/confirmation, inline review/start, native entry/exit, persisted result, rendered output and draft isolation; one seed prompt and one opted-in review, no visual tests.",
            if graph_mode { "graph" } else { "main" }
        );
    } else if check.settings.is_some() {
        println!(
            "PASS: native settings through the real desktop host, isolated native configuration, no account-backed inference or visual tests."
        );
    } else if check.history.is_some() {
        println!(
            "PASS: {} native history browsing/search/archive filter, explicit {} confirmation, exact native readback and preserved draft; two opted-in seed prompts, no inference during import, no visual tests.",
            if graph_mode { "graph" } else { "main" },
            if history_fork { "fork" } else { "link" }
        );
    } else if check.lifecycle.is_some() {
        println!(
            "PASS: {} host native rename/archive/unarchive/fork/delete controls, confirmations, original draft, independent fork readback and owned-history cleanup; no visual tests.",
            if graph_mode { "graph" } else { "main" }
        );
    } else if check.command_policy.is_some() {
        println!(
            "PASS: {} actual desktop command-policy button, exact native approval/resolution, two persisted print-only turns, native rule reuse, draft and sibling isolation; no account inference, personal rules or visual tests.",
            if graph_mode { "graph" } else { "main" }
        );
    } else if check.elicitation.is_some() {
        println!(
            "PASS: {} actual desktop MCP form/URL accept/decline/cancel cards, native results and draft isolation; isolated profile, no account inference, personal configuration or visual tests.",
            if check.graph.is_some() {
                "graph"
            } else {
                "main"
            }
        );
    } else if check.restart.is_some() {
        println!(
            "Restart application stage completed; owned history retained for parent verification/cleanup."
        );
    } else if check.providers.is_some() {
        println!(
            "PASS: {} host actual provider selector round trip, draft/legacy-history preservation and same-native-thread continuation/readback. Claude used a local fixture, not inference; full app restart remains a separate gate.",
            if graph_mode { "graph" } else { "main" }
        );
    } else if check.recovery.is_some() {
        println!(
            "PASS: {} host owned-server crash during streaming, real EOF, disconnected draft preservation, explicit reconnect/history/resume and same-thread continuation without replay. Delayed host ACK mode: {lost_ack}. Whole-app restart, wire-level response loss and provider switching remain separate gates.",
            if graph_mode { "graph" } else { "main" }
        );
    } else if check.media.is_some() {
        println!(
            "PASS: {} host native UTF-8/{media_format:?} input ({media_delivery:?}), selected snapshot fidelity, model recognition, ACK-scoped attachment cleanup, newer draft/file preservation and history readback. No visual test; OS picker interaction and generated artifacts remain separate gates.",
            if graph_mode { "graph" } else { "main" }
        );
    } else if check.approvals.is_some() {
        println!(
            "PASS: {} host native {} {:?} decision, expected execution/rejection, request removal and history readback (no visual tests). Other request kinds remain separate gates.",
            if graph_mode { "graph" } else { "main" },
            if file_change {
                "single-file change and rendered diff"
            } else {
                "command"
            },
            choice
        );
    } else if check.delivery.is_some() {
        println!(
            "PASS: {} desktop host native steering, queue, interruption, explicit reconnect, draft preservation and history readback (no visual tests). Approvals, crash/uncertain recovery and media remain separate gates.",
            if graph_mode { "graph" } else { "main" }
        );
    } else if check.graph.is_some() {
        println!(
            "PASS: two graph-card desktop hosts, concurrent native threads and isolated continuation/readback (no visual tests). Queue/Steer, approvals and media remain separate gates."
        );
    } else {
        println!(
            "PASS: main-chat desktop host native acceptance (no visual tests). Graph, Queue/Steer, approvals and media have separate acceptance gates."
        );
    }
    Ok(())
}
