//! Opt-in native review through the real main/graph confirmation and IPC.
//! One seed prompt and one review of an in-prompt snippet or owned Git fixture.
use super::*;
use central_agent_codex_runtime::wire::ServerRequestKey;
use delivery::{input, script};
mod git_fixture;
mod history;

const SEED: &str =
    "Reply exactly NATIVE_REVIEW_READY. Do not use tools, read files or change anything.";
const DRAFT: &str = "Unsent review-owner draft";
const SIBLING: &str = "Unsent independent graph review draft";
const INSTRUCTIONS: &str = "Review only this self-contained Rust snippet: `fn is_positive(n: i32) -> bool { n < 0 }`. Its specification is to return true exactly for positive integers. Report the correctness issue concisely using the normal review format. Do not use tools, inspect files, or change anything; all necessary code is in this instruction.";
const APPROVAL_INSTRUCTIONS: &str = "Review this self-contained Rust snippet: `fn is_positive(n: i32) -> bool { n < 0 }`, whose specification is true exactly for positive integers. Before reporting the issue, invoke exec_command exactly once with cmd=[string]::Concat('NATIVE_REVIEW_APPROVAL'), login=false, sandbox_permissions=require_escalated and justification=Confirm this print-only native review test. Use the current working directory. This test will interrupt instead of approving the request. Do not add commands, read or change files, or change configuration. If the runtime rejects the request, report that rejection without retrying. The source to review is entirely in this instruction.";
const TARGETS: [&str; 4] = ["uncommittedChanges", "baseBranch", "commit", "custom"];
const COMMAND_INSTRUCTIONS: &str = "Review this self-contained Rust snippet: `fn is_positive(n: i32) -> bool { n < 0 }`, whose specification is true exactly for positive integers. Before reporting the issue, use exec_command exactly once with this exact PowerShell command: Write-Output 'NATIVE_REVIEW_RUNNING'; Start-Sleep -Seconds 20; Write-Output 'NATIVE_REVIEW_FINISHED'. Use the current working directory, login=false and yield_time_ms=1000. This is an interruption test: do not add commands, read or modify files, request extra permissions, or change configuration. If interrupted, do not retry. All source to review is in this instruction.";

fn native_review_interrupted(turns: &[Value], review: &str, stopped: &str) -> bool {
    !review.is_empty()
        && !stopped.is_empty()
        && [review, stopped].into_iter().all(|id| {
            turns
                .iter()
                .any(|turn| turn["id"] == id && turn["status"] == "interrupted")
        })
}

pub(super) fn sample(owner: &str) -> String {
    format!(
        r#"(()=>{{const state={};{}const d=root?.querySelector('dialog[open]');
      state.reviewDialog=d?{{title:d.querySelector('h2')?.textContent,text:d.textContent,
        kind:d.querySelector('select')?.value,options:[...d.querySelector('select')?.options||[]].map(o=>o.value),
        value:d.querySelector('textarea')?.value,disabled:d.querySelector('button[type=submit]')?.disabled}}:null;
      state.reviewApprovalCount=root?.querySelectorAll('.native-requests article').length;
      state.reviewApprovalVisible=[...root?.querySelectorAll('.native-requests article')||[]].some(card=>card.getClientRects().length>0&&card.textContent.includes('NATIVE_REVIEW_APPROVAL'));
      state.reviewRendered=window.__nativeReviewRendered===true;state.reviewMatches=window.__nativeReviewMatches;state.reviewPromptMatches=window.__nativeReviewPromptMatches;return state;}})()"#,
        lifecycle::sample(owner),
        delivery::root_script(owner)
    )
}

fn dialog_script(app: &BrowserApp, owner: &str, body: &str) -> anyhow::Result<()> {
    script(
        app,
        owner,
        &format!(
            "const dialog=root.querySelector('dialog[open]');if(dialog?.querySelector('h2')?.textContent!=='Review code with Codex')throw new Error('Missing exact review confirmation');{body}"
        ),
    )
}

fn completed_review(
    thread: &central_agent_codex_runtime::mirror::Thread,
    turn_id: &str,
) -> anyhow::Result<Option<String>> {
    let Some(turn) = thread.turns.iter().find(|turn| turn.id == turn_id) else {
        return Ok(None);
    };
    if turn.active() {
        return Ok(None);
    }
    anyhow::ensure!(
        turn.status == "completed" && turn.error.is_none(),
        "Native review did not complete successfully"
    );
    for kind in ["enteredReviewMode", "exitedReviewMode"] {
        anyhow::ensure!(
            turn.items.iter().any(|item| item.completed
                && item.value["type"] == kind
                && item.value["review"]
                    .as_str()
                    .is_some_and(|text| !text.trim().is_empty())),
            "Native history lacks completed {kind}"
        );
    }
    Ok(turn
        .items
        .iter()
        .find(|item| item.value["type"] == "exitedReviewMode")
        .and_then(|item| item.value["review"].as_str())
        .map(str::to_owned))
}

#[derive(Default)]
pub(super) struct Review {
    command_stop: bool,
    running_command: Option<(String, String)>,
    running_output: String,
    observed_items: history::Evidence,
    approval_stop: bool,
    approval: Option<(ServerRequestKey, Value)>,
    approval_resolved: bool,
    prepare_stop: bool,
    preparation_held: bool,
    held_preparation: Option<BrowserEvent>,
    git_mode: Option<String>,
    fixture: Option<git_fixture::Fixture>,
    read_commands: usize,
    inspected_bug: bool,
    stop: bool,
    stop_turn: Option<String>,
    interrupt_ack: bool,
    interrupted_event: bool,
    phase: u8,
    graph_open: bool,
    cancelled: usize,
    seed_count: usize,
    confirmed: usize,
    review_turn: Option<String>,
    source: Option<String>,
    seed_record: Option<Value>,
    entered: bool,
    exited: bool,
    error: Option<String>,
}

impl Review {
    pub(super) fn new(
        stop: bool,
        prepare_stop: bool,
        approval_stop: bool,
        command_stop: bool,
        git_mode: Option<String>,
    ) -> Self {
        Self {
            stop: stop || approval_stop || command_stop,
            prepare_stop,
            approval_stop,
            command_stop,
            git_mode,
            ..Self::default()
        }
    }

    /// Hold only the actual preparation reply at the worker-to-host boundary.
    /// Its contents and all native events are preserved. This hook exists solely
    /// in the explicitly opted-in acceptance host, not the production transport.
    pub(super) fn defer_preparation(
        &mut self,
        owner: &str,
        event: BrowserEvent,
    ) -> Option<BrowserEvent> {
        if self.prepare_stop
            && self.phase == 9
            && !self.preparation_held
            && matches!(&event, BrowserEvent::AppServer(Event::ConversationReply {
                request, result: Ok(_), ..
            }) if request.owner == owner && request.call.method == "thread/resume"
                && request.call.params["approvalsReviewer"] == "user")
        {
            self.preparation_held = true;
            self.held_preparation = Some(event);
            println!(
                "Review host: held the actual preparation reply until the real Stop button cancels the unsent review."
            );
            None
        } else {
            Some(event)
        }
    }

    fn target(&self) -> Value {
        self.fixture
            .as_ref()
            .map(|f| f.target.clone())
            .unwrap_or_else(|| json!({"type":"custom","instructions":if self.command_stop { COMMAND_INSTRUCTIONS } else if self.approval_stop { APPROVAL_INSTRUCTIONS } else { INSTRUCTIONS }}))
    }

    pub(super) fn observe(&mut self, app: &BrowserApp, owner: &str, event: &BrowserEvent) {
        if self.command_stop {
            self.observed_items
                .observe(owner, self.source.as_deref(), event);
        }
        let result = (|| -> anyhow::Result<()> {
            match event {
                BrowserEvent::ScopedAgentPanel {
                    message:
                        AgentPanelMessage::AppServerConversation {
                            owner: inner,
                            action:
                                ConversationAction::StartReview {
                                    expected_thread_id,
                                    target,
                                    ..
                                },
                        },
                    ..
                }
                | BrowserEvent::AgentPanel(AgentPanelMessage::AppServerConversation {
                    owner: inner,
                    action:
                        ConversationAction::StartReview {
                            expected_thread_id,
                            target,
                            ..
                        },
                }) if inner == owner => {
                    if let BrowserEvent::ScopedAgentPanel { owner: routed, .. } = event {
                        anyhow::ensure!(
                            routed == owner,
                            "Review IPC route differs from its immutable owner"
                        );
                    }
                    anyhow::ensure!(
                        inner == owner
                            && self.phase == 9
                            && self.confirmed == 0
                            && Some(expected_thread_id) == self.source.as_ref()
                            && serde_json::to_value(target)? == self.target(),
                        "Review confirmation changed owner, thread or target"
                    );
                    self.confirmed += 1;
                }
                BrowserEvent::AppServer(Event::ConversationReply {
                    request, result, ..
                }) if request.owner == owner => {
                    let value = result.as_ref().map_err(|e| {
                        anyhow::anyhow!("Native {} failed: {e}", request.call.method)
                    })?;
                    if request.call.method == "thread/resume" && self.phase == 9 {
                        println!(
                            "Review preparation native scope: {}",
                            json!({
                                "requestedCwd":request.call.params["cwd"], "cwd":value["cwd"],
                                "sandboxType":value["sandbox"]["type"], "networkAccess":value["sandbox"]["networkAccess"],
                                "approvalPolicy":value["approvalPolicy"], "approvalsReviewer":value["approvalsReviewer"]
                            })
                        );
                    }
                    if request.call.method == "turn/start" {
                        anyhow::ensure!(
                            self.seed_count == 0
                                && request.call.params["input"].as_array().is_some_and(
                                    |items| items.len() == 1 && items[0]["text"] == SEED
                                ),
                            "Unexpected review seed inference"
                        );
                        self.seed_count += 1;
                    } else if request.call.method == "review/start" {
                        anyhow::ensure!(
                            !self.prepare_stop,
                            "Cancelled preparation unexpectedly dispatched native review/start"
                        );
                        anyhow::ensure!(
                            self.confirmed == 1
                                && self.review_turn.is_none()
                                && request.call.params["delivery"] == "inline"
                                && request.call.params["threadId"]
                                    == self.source.as_deref().unwrap_or("")
                                && request.call.params["target"] == self.target()
                                && value["reviewThreadId"] == self.source.as_deref().unwrap_or(""),
                            "Native review request or acknowledgement changed scope"
                        );
                        self.review_turn = Some(
                            value["turn"]["id"]
                                .as_str()
                                .filter(|id| !id.is_empty())
                                .context("Missing native review turn ID")?
                                .to_owned(),
                        );
                    } else if request.call.method == "turn/interrupt" {
                        anyhow::ensure!(
                            self.stop
                                && self.phase == 11
                                && !self.interrupt_ack
                                && self.stop_turn.is_some()
                                && request.call.params["threadId"]
                                    == self.source.as_deref().unwrap_or("")
                                && request.call.params["turnId"]
                                    == self.stop_turn.as_deref().unwrap()
                                && value == &json!({}),
                            "Review Stop did not acknowledge exactly the displayed native turn"
                        );
                        self.interrupt_ack = true;
                    }
                }
                BrowserEvent::AppServer(Event::Transport {
                    event:
                        TransportEvent::ServerRequest {
                            key,
                            method,
                            params,
                        },
                    ..
                }) => {
                    anyhow::ensure!(
                        self.approval_stop
                            && self.phase == 9
                            && self.approval.is_none()
                            && method == "item/commandExecution/requestApproval"
                            && params["threadId"] == self.source.as_deref().unwrap_or("")
                            && params["turnId"].as_str().is_some_and(|id| !id.is_empty())
                            && params["itemId"].as_str().is_some_and(|id| !id.is_empty())
                            && params["networkApprovalContext"].is_null()
                            && params["additionalPermissions"].is_null()
                            && params["command"]
                                .as_str()
                                .is_some_and(|s| s.contains("NATIVE_REVIEW_APPROVAL")),
                        "Unexpected native review permission request; no approval will be sent"
                    );
                    let root = app
                        .app_server
                        .roots
                        .get(owner)
                        .context("Missing review root")?;
                    anyhow::ensure!(
                        params["cwd"]
                            .as_str()
                            .and_then(|p| fs::canonicalize(p).ok())
                            == Some(fs::canonicalize(root)?),
                        "Review approval targets another directory"
                    );
                    self.approval = Some((key.clone(), params.clone()));
                }
                BrowserEvent::AppServer(Event::Transport {
                    event: TransportEvent::Notification { method, params },
                    ..
                }) if self.approval_stop && method == "serverRequest/resolved" => {
                    anyhow::ensure!(
                        self.approval
                            .as_ref()
                            .is_some_and(|(key, _)| json!(key.id) == params["requestId"])
                            && params["threadId"] == self.source.as_deref().unwrap_or("")
                            && self.phase == 11
                            && !self.approval_resolved,
                        "Review request resolved before Stop or in another conversation"
                    );
                    self.approval_resolved = true;
                }
                BrowserEvent::AgentPanel(AgentPanelMessage::AppServerRequest { .. })
                | BrowserEvent::ScopedAgentPanel {
                    message: AgentPanelMessage::AppServerRequest { .. },
                    ..
                }
                | BrowserEvent::AppServer(Event::Answered { .. })
                    if self.approval_stop =>
                {
                    anyhow::bail!("Review Stop must not answer or approve the pending command")
                }
                BrowserEvent::AppServer(Event::Transport {
                    event: TransportEvent::Notification { method, params },
                    ..
                }) if self.command_stop && method == "item/commandExecution/outputDelta" => {
                    anyhow::ensure!(
                        params["threadId"] == self.source.as_deref().unwrap_or("")
                            && self
                                .running_command
                                .as_ref()
                                .is_some_and(|(turn, item)| params["turnId"] == *turn
                                    && params["itemId"] == *item),
                        "Command output arrived for another review turn/item"
                    );
                    self.running_output
                        .push_str(params["delta"].as_str().unwrap_or(""));
                    anyhow::ensure!(
                        !self.running_output.contains("NATIVE_REVIEW_FINISHED"),
                        "Review command completed before interruption"
                    );
                }
                BrowserEvent::AppServer(Event::Transport {
                    event: TransportEvent::Notification { method, params },
                    ..
                }) if self.stop_turn.is_some()
                    && method == "turn/completed"
                    && params["threadId"] == self.source.as_deref().unwrap_or("") =>
                {
                    let id = params["turn"]["id"]
                        .as_str()
                        .context("Missing native completion ID")?;
                    println!(
                        "Review host: native terminal event {id}, status {} (Stop targeted {}).",
                        params["turn"]["status"],
                        self.stop_turn.as_deref().unwrap()
                    );
                    anyhow::ensure!(
                        [
                            self.stop_turn.as_deref(),
                            self.review_turn.as_deref(),
                            self.running_command.as_ref().map(|(id, _)| id.as_str())
                        ]
                        .contains(&Some(id))
                            && params["turn"]["status"] == "interrupted",
                        "Native completion was unrelated or not interrupted; this is not an interruption pass"
                    );
                    self.interrupted_event = true;
                }
                BrowserEvent::AppServer(Event::Transport {
                    event: TransportEvent::Notification { method, params },
                    ..
                }) if app
                    .app_server
                    .conversations
                    .binding(owner)
                    .is_some_and(|b| params["threadId"] == b.thread_id)
                    && matches!(method.as_str(), "item/started" | "item/completed") =>
                {
                    let kind = params["item"]["type"].as_str().unwrap_or("");
                    anyhow::ensure!(
                        (kind == "commandExecution"
                            && (self.fixture.is_some() || self.approval_stop || self.command_stop)
                            && self.phase >= 9)
                            || matches!(
                                kind,
                                "userMessage"
                                    | "agentMessage"
                                    | "reasoning"
                                    | "plan"
                                    | "enteredReviewMode"
                                    | "exitedReviewMode"
                            ),
                        "Review probe emitted unexpected activity: {kind}"
                    );
                    if kind == "commandExecution" && self.command_stop {
                        if method == "item/started" {
                            anyhow::ensure!(
                                self.running_command.is_none()
                                    && params["item"]["command"]
                                        .as_str()
                                        .is_some_and(|s| s.contains("NATIVE_REVIEW_RUNNING")
                                            && s.contains("Start-Sleep")),
                                "Unexpected or repeated review command"
                            );
                            self.running_command = Some((
                                params["turnId"]
                                    .as_str()
                                    .context("Missing command turn")?
                                    .into(),
                                params["item"]["id"]
                                    .as_str()
                                    .context("Missing command item")?
                                    .into(),
                            ));
                        } else {
                            anyhow::ensure!(
                                params["item"]["status"] != "completed"
                                    && params["item"]["exitCode"] != 0,
                                "Review command finished instead of being interrupted"
                            );
                        }
                    } else if kind == "commandExecution"
                        && method == "item/completed"
                        && self.approval_stop
                    {
                        println!(
                            "Review approval probe: native command outcome {}",
                            params["item"]
                        );
                        anyhow::ensure!(
                            params["item"]["status"] != "completed"
                                && params["item"]["exitCode"] != 0,
                            "Review command executed without approval"
                        );
                    } else if kind == "commandExecution" && method == "item/completed" {
                        let item = &params["item"];
                        anyhow::ensure!(
                            self.fixture.as_ref().is_some_and(|f| item["cwd"]
                                .as_str()
                                .is_some_and(|cwd| f.owns(cwd))),
                            "Review command reported a different working directory"
                        );
                        if item["status"] == "completed" && item["exitCode"] == 0 {
                            self.read_commands += 1;
                            let output = item["aggregatedOutput"].as_str().unwrap_or("");
                            self.inspected_bug |=
                                output.contains("is_positive") && output.contains("n < 0");
                        }
                    }
                    if kind == "enteredReviewMode" {
                        self.entered = true;
                    }
                    if kind == "exitedReviewMode" && method == "item/completed" {
                        self.exited = true;
                    }
                }
                _ => {}
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.error = Some(error.to_string());
        }
    }

    pub(super) fn tick(
        &mut self,
        app: &mut BrowserApp,
        owner: &str,
        sample: &Value,
        graph: Option<&graph::Graph>,
    ) -> anyhow::Result<bool> {
        if let Some(error) = &self.error {
            anyhow::bail!("{error}")
        }
        if sample.is_null() {
            return Ok(false);
        }
        if self.phase == 0 {
            if !app.app_server.configuration().0.can_run() {
                return Ok(false);
            }
            if let Some(graph) = graph {
                if !self.graph_open {
                    graph.open_cards(app)?;
                    self.graph_open = true;
                    return Ok(false);
                }
                if sample["ready"] != true {
                    return Ok(false);
                }
                script(
                    app,
                    owner,
                    "root.addEventListener('central-agent:graph-composer-state',event=>root.__nativeDeliveryState=event.detail);",
                )?;
            }
            if let Some(mode) = &self.git_mode {
                let root = graph
                    .and_then(|g| g.directory(owner))
                    .or_else(|| app.workspace.root())
                    .context("Missing review fixture workspace")?;
                self.fixture = Some(git_fixture::Fixture::create(root, mode)?);
            }
            input(app, owner, SEED, true)?;
            self.phase = 1;
            println!("Review host: one seed prompt sent through the real composer.");
            return Ok(false);
        }
        let Some(binding) = app.app_server.conversations.binding(owner).cloned() else {
            return Ok(false);
        };
        if let Some(source) = &self.source {
            anyhow::ensure!(
                *source == binding.thread_id,
                "Review changed its native thread"
            );
        } else {
            self.source = Some(binding.thread_id.clone());
        }
        let Some(thread) = app
            .app_server
            .conversations
            .mirror
            .thread(&binding.thread_id)
        else {
            return Ok(false);
        };
        if self.phase == 1 {
            if app.app_server.busy(owner)
                || thread.turns.len() != 1
                || thread.turns[0].status != "completed"
                || !sample["text"]
                    .as_str()
                    .unwrap_or("")
                    .contains("NATIVE_REVIEW_READY")
            {
                return Ok(false);
            }
            input(app, owner, DRAFT, false)?;
            let native = api::read_thread(&binding.thread_id)
                .send(
                    app.app_server
                        .client
                        .as_ref()
                        .context("Disconnected seed")?,
                )?
                .wait()?;
            let turns = native["thread"]["turns"]
                .as_array()
                .context("Missing native seed history")?;
            anyhow::ensure!(
                turns.len() == 1 && turns[0]["status"] == "completed",
                "Seed did not persist as one completed native turn"
            );
            self.seed_record = Some(turns[0].clone());
            if let Some(graph) = graph {
                input(app, &graph.owners()[1], SIBLING, false)?;
            }
            self.phase = 2;
            return Ok(false);
        }
        if sample["draft"] != DRAFT {
            return Ok(false);
        }
        if let Some(graph) = graph {
            let sibling = &graph.owners()[1];
            anyhow::ensure!(
                app.app_server.conversations.binding(sibling).is_none()
                    && !app.app_server.busy(sibling)
                    && app
                        .app_server
                        .conversations
                        .binding(&app.conversation_key(None))
                        .is_none(),
                "Review touched another graph/main conversation"
            );
            if !sample["siblings"]
                .as_array()
                .is_some_and(|rows| rows.len() == 1 && rows[0]["draft"] == SIBLING)
            {
                return Ok(false);
            }
        }
        anyhow::ensure!(
            sample["lifecycleError"].as_str().is_none_or(str::is_empty),
            "Review UI error: {}",
            sample["lifecycleError"]
        );
        let dialog = &sample["reviewDialog"];
        match self.phase {
            2 => {
                lifecycle::button(app, owner, "Review code…")?;
                self.phase = 3;
            }
            3 => {
                if dialog.is_null() {
                    return Ok(false);
                }
                anyhow::ensure!(
                    dialog["options"] == json!(TARGETS),
                    "Review target choices changed"
                );
                dialog_script(
                    app,
                    owner,
                    &format!(
                        "const select=dialog.querySelector('select');select.value={};select.dispatchEvent(new Event('change',{{bubbles:true}}));",
                        json!(TARGETS[self.cancelled])
                    ),
                )?;
                self.phase = 4;
            }
            4 => {
                if dialog["kind"] != TARGETS[self.cancelled] {
                    return Ok(false);
                }
                anyhow::ensure!(
                    dialog["disabled"] == (self.cancelled != 0),
                    "Empty review target validation failed"
                );
                dialog_script(
                    app,
                    owner,
                    "const button=[...dialog.querySelectorAll('button')].find(b=>b.textContent==='Cancel');if(!button)throw new Error('Missing review Cancel');button.click();",
                )?;
                self.phase = 5;
            }
            5 => {
                if !dialog.is_null() {
                    return Ok(false);
                }
                anyhow::ensure!(
                    self.confirmed == 0
                        && self.review_turn.is_none()
                        && self.seed_count == 1
                        && thread.turns.len() == 1
                        && !app.app_server.busy(owner),
                    "Cancelled review sent native work"
                );
                self.cancelled += 1;
                lifecycle::button(app, owner, "Review code…")?;
                self.phase = if self.cancelled == TARGETS.len() {
                    6
                } else {
                    3
                };
            }
            6 => {
                if dialog.is_null() {
                    return Ok(false);
                }
                if let Some(fixture) = &self.fixture {
                    dialog_script(
                        app,
                        owner,
                        &format!(
                            "const select=dialog.querySelector('select');select.value={};select.dispatchEvent(new Event('change',{{bubbles:true}}));",
                            fixture.target["type"]
                        ),
                    )?;
                    self.phase = 8;
                    return Ok(false);
                }
                anyhow::ensure!(
                    dialog["kind"] == "custom",
                    "Last cancelled target was not retained"
                );
                dialog_script(
                    app,
                    owner,
                    &format!(
                        "const field=dialog.querySelector('textarea');field.value={};field.dispatchEvent(new Event('input',{{bubbles:true}}));",
                        self.target()["instructions"]
                    ),
                )?;
                self.phase = 7;
            }
            8 => {
                let target = self.target();
                if dialog["kind"] != target["type"] {
                    return Ok(false);
                }
                if let Some(value) = target["branch"].as_str().or_else(|| target["sha"].as_str()) {
                    dialog_script(
                        app,
                        owner,
                        &format!(
                            "const field=dialog.querySelector('textarea');field.value={};field.dispatchEvent(new Event('input',{{bubbles:true}}));",
                            json!(value)
                        ),
                    )?;
                }
                self.phase = 7;
            }
            7 => {
                if !app.app_server.configuration().0.can_run() {
                    return Ok(false);
                }
                let target = self.target();
                let value = target["instructions"]
                    .as_str()
                    .or_else(|| target["branch"].as_str())
                    .or_else(|| target["sha"].as_str());
                if dialog["kind"] != target["type"]
                    || value.is_some_and(|value| dialog["value"] != value)
                    || dialog["disabled"] != false
                {
                    return Ok(false);
                }
                let directory = graph
                    .and_then(|g| g.directory(owner))
                    .or_else(|| app.workspace.root())
                    .context("Missing review directory")?;
                let text = dialog["text"].as_str().unwrap_or("");
                anyhow::ensure!(
                    text.contains(&display_path(directory))
                        && text.contains("read-only")
                        && text.contains("account allowance"),
                    "Review confirmation omitted scope/usage: expected directory {}, dialog {}",
                    directory.display(),
                    text
                );
                dialog_script(
                    app,
                    owner,
                    "dialog.querySelector('button[aria-label=\"Start review\"]').click();",
                )?;
                println!(
                    "Review host readiness: selected={}, ready={}",
                    app.native_access_selected(owner),
                    app.app_server.configuration().0.can_run()
                );
                self.phase = 9;
                println!(
                    "Review host: four cancelled targets sent no work; confirmed one inline native review."
                );
            }
            9 => {
                if self.prepare_stop {
                    if self.held_preparation.is_none()
                        || sample["stop"] != true
                        || sample["active"] != "true"
                    {
                        return Ok(false);
                    }
                    let pending = app
                        .app_server
                        .reviews
                        .get(owner)
                        .context("Held preparation has no pending review")?;
                    anyhow::ensure!(
                        !pending.sending
                            && !pending.cancelled
                            && self.review_turn.is_none()
                            && !self.entered
                            && !self.exited
                            && app.app_server.conversations.active_turn(owner).is_none(),
                        "Review started before preparation cancellation"
                    );
                    delivery::click(app, owner, "stop-agent")?;
                    self.phase = 12;
                    return Ok(false);
                }
                let Some(turn) = &self.review_turn else {
                    return Ok(false);
                };
                if self.stop {
                    anyhow::ensure!(!self.exited, "Review already exited before the Stop test");
                    if !self.entered || sample["stop"] != true || sample["active"] != "true" {
                        return Ok(false);
                    }
                    let active = app
                        .app_server
                        .conversations
                        .active_turn(owner)
                        .context("Stop displayed without a native active review turn")?;
                    if self.approval_stop {
                        let Some((_, params)) = &self.approval else {
                            return Ok(false);
                        };
                        let views = app.app_server.requests.views(owner);
                        anyhow::ensure!(
                            views.len() == 1
                                && views[0].params == *params
                                && params["turnId"] == active
                                && !views[0].responding,
                            "Pending review approval does not own the active native turn"
                        );
                        if sample["reviewApprovalVisible"] != true {
                            return Ok(false);
                        }
                        println!(
                            "Review host: scoped command approval is visible; interrupting without answering it."
                        );
                    }
                    if self.command_stop {
                        if self.running_command.is_none()
                            || !self.running_output.contains("NATIVE_REVIEW_RUNNING")
                        {
                            return Ok(false);
                        }
                        anyhow::ensure!(
                            self.running_command
                                .as_ref()
                                .is_some_and(|(worker, _)| thread
                                    .turns
                                    .iter()
                                    .any(|t| t.id == *worker && t.active())),
                            "Running command has no active worker in the owned native thread"
                        );
                        println!(
                            "Review host: native command streamed its running marker; stopping native turn {active}, worker {}.",
                            self.running_command.as_ref().unwrap().0
                        );
                    }
                    if sample["turn"] != active {
                        return Ok(false);
                    }
                    self.stop_turn = Some(active.into());
                    delivery::click(app, owner, "stop-agent")?;
                    self.phase = 11;
                    println!(
                        "Review host: clicked actual Stop for active native review turn {active} (review ACK {turn})."
                    );
                    return Ok(false);
                }
                if app.app_server.busy(owner) || !self.entered || !self.exited {
                    return Ok(false);
                }
                let Some(text) = completed_review(thread, turn)? else {
                    return Ok(false);
                };
                if self.fixture.is_some() {
                    anyhow::ensure!(
                        self.read_commands > 0 && self.inspected_bug && text.contains("lib.rs"),
                        "Git review did not prove successful native inspection and a finding on the changed source"
                    );
                }
                println!("Review host: native result candidates {}", json!(thread.turns.iter().flat_map(|t|
                    t.items.iter().map(|i|json!({"turn":t.id,"id":i.id,"type":i.value["type"],"phase":i.value["phase"],
                        "matchesResult":i.value["text"].as_str().or_else(||i.value["review"].as_str()).is_some_and(|body|body.trim()==text.trim())}))).collect::<Vec<_>>()));
                anyhow::ensure!(
                    self.seed_count == 1
                        && self.confirmed == 1
                        && app.app_server.conversations.saved().unresolved.is_empty(),
                    "Review lifecycle or native history did not settle: seed={}, confirmations={}, unresolved={}, projected={}",
                    self.seed_count, self.confirmed, app.app_server.conversations.saved().unresolved.len(),
                    json!(thread.turns.iter().map(|turn| json!({"id":turn.id,"status":turn.status,
                        "items":turn.items.iter().map(|item|item.value["type"].clone()).collect::<Vec<_>>()})).collect::<Vec<_>>())
                );
                // Compare the actual chat body to the native review's Markdown text.
                // The detached element is an oracle only; never append fixture HTML to chat.
                script(
                    app,
                    owner,
                    &format!(
                        "const expected=document.createElement('div');expected.innerHTML={};const prompt=document.createElement('div');prompt.innerHTML={};const norm=s=>s.replace(/\\s+/g,' ').trim();window.__nativeReviewMatches=[...root.querySelectorAll('.chat-message.assistant:not(.reasoning):not(.activity) .chat-text')].filter(row=>norm(row.textContent)===norm(expected.textContent)).length;window.__nativeReviewPromptMatches=[...root.querySelectorAll('.chat-message.user .chat-text')].filter(row=>norm(row.textContent)===norm(prompt.textContent)).length;window.__nativeReviewRendered=window.__nativeReviewMatches>0;",
                        json!(crate::markdown::render_safe_markdown(&text)),
                        json!(crate::markdown::render_safe_markdown(INSTRUCTIONS))
                    ),
                )?;
                app.app_server_conversation(owner.into(), ConversationAction::Read {});
                self.phase = 10;
            }
            11 => {
                if !self.interrupt_ack
                    || !self.interrupted_event
                    || app.app_server.busy(owner)
                    || sample["active"] != "false"
                    || sample["stop"] != false
                    || self.approval_stop
                        && (!self.approval_resolved
                            || sample["reviewApprovalCount"] != 0
                            || !app.app_server.requests.views(owner).is_empty())
                {
                    return Ok(false);
                }
                app.app_server_conversation(owner.into(), ConversationAction::Read {});
                self.phase = 10;
            }
            12 => {
                let pending = app
                    .app_server
                    .reviews
                    .get(owner)
                    .context("Preparation disappeared before its real reply was released")?;
                if !pending.cancelled {
                    return Ok(false);
                }
                anyhow::ensure!(!pending.sending, "Stop failed to cancel the unsent review");
                app.proxy
                    .send_event(
                        self.held_preparation
                            .take()
                            .context("Missing exact held native preparation reply")?,
                    )
                    .map_err(|_| anyhow::anyhow!("Could not release preparation reply"))?;
                self.phase = 13;
                println!(
                    "Review host: actual Stop marked cancellation; released the unchanged native preparation reply."
                );
            }
            13 => {
                if app.app_server.busy(owner)
                    || sample["active"] != "false"
                    || sample["stop"] != false
                {
                    return Ok(false);
                }
                anyhow::ensure!(
                    self.preparation_held
                        && self.held_preparation.is_none()
                        && self.seed_count == 1
                        && self.confirmed == 1
                        && self.review_turn.is_none()
                        && !self.entered
                        && !self.exited
                        && !self.interrupt_ack
                        && app.app_server.conversations.saved().unresolved.is_empty()
                        && !app.app_server.reviews.contains_key(owner),
                    "Preparation cancellation left pending work, a receipt or native review activity"
                );
                let native = api::read_thread(&binding.thread_id)
                    .send(
                        app.app_server
                            .client
                            .as_ref()
                            .context("Disconnected cancelled review")?,
                    )?
                    .wait()?;
                let seed = self.seed_record.as_ref().context("Missing original seed")?;
                anyhow::ensure!(
                    unchanged_seed(&native, &binding.thread_id, seed)
                        && thread.turns.len() == 1
                        && thread.turns[0].id == seed["id"]
                        && thread.turns[0].status == "completed",
                    "Cancelled preparation changed native or projected conversation history"
                );
                let directory = graph
                    .and_then(|g| g.directory(owner))
                    .or_else(|| app.workspace.root())
                    .context("Missing original review workspace")?;
                anyhow::ensure!(
                    fs::read_dir(directory)?.next().is_none(),
                    "Cancelled review changed its workspace"
                );
                println!(
                    "Review host: native history remains exactly the original seed; no review/start, no turn/interrupt, no workspace changes; owner/sibling drafts preserved."
                );
                return Ok(true);
            }
            10 => {
                if (!self.stop && sample["reviewRendered"] != true) || app.app_server.busy(owner) {
                    return Ok(false);
                }
                anyhow::ensure!(
                    self.stop || sample["reviewMatches"] == 1,
                    "Native review result rendered {} times instead of once",
                    sample["reviewMatches"]
                );
                anyhow::ensure!(
                    self.stop || self.fixture.is_some() || sample["reviewPromptMatches"] == 1,
                    "Native review instruction rendered {} times instead of once",
                    sample["reviewPromptMatches"]
                );
                let native = api::read_thread(&binding.thread_id)
                    .send(
                        app.app_server
                            .client
                            .as_ref()
                            .context("Disconnected review")?,
                    )?
                    .wait()?;
                let turns = native["thread"]["turns"]
                    .as_array()
                    .context("Missing persisted review history")?;
                let reviewed = turns
                    .iter()
                    .find(|turn| turn["id"] == self.review_turn.as_deref().unwrap())
                    .context("Acknowledged review missing from native history")?;
                let seed = self
                    .seed_record
                    .as_ref()
                    .context("Missing native seed snapshot")?;
                anyhow::ensure!(
                    native["thread"]["id"] == binding.thread_id
                        && turns.iter().find(|turn| turn["id"] == seed["id"]) == Some(seed),
                    "Review changed the original native seed or thread"
                );
                if self.stop {
                    anyhow::ensure!(
                        self.interrupt_ack
                            && self.interrupted_event
                            && native_review_interrupted(
                                turns,
                                self.review_turn.as_deref().unwrap(),
                                self.stop_turn.as_deref().unwrap()
                            )
                            && self.seed_count == 1
                            && self.confirmed == 1
                            && app.app_server.conversations.saved().unresolved.is_empty(),
                        "Native history does not confirm the interrupted review and its targeted worker"
                    );
                    if self.approval_stop {
                        anyhow::ensure!(
                            self.approval_resolved
                                && self.approval.is_some()
                                && turns
                                    .iter()
                                    .flat_map(|t| t["items"].as_array().into_iter().flatten())
                                    .filter(|item| item["type"] == "commandExecution")
                                    .all(|item| item["status"] != "completed"
                                        && item["exitCode"] != 0),
                            "Native review history contains a command executed without consent"
                        );
                        println!(
                            "Review host: native request resolution and cleared card verified; command never approved or executed."
                        );
                    }
                } else {
                    anyhow::ensure!(
                        reviewed["status"] == "completed"
                            && reviewed["items"]
                                .as_array()
                                .is_some_and(|items| items.iter().any(|i| i["type"]
                                    == "exitedReviewMode"
                                    && i["review"].as_str().is_some_and(|s| !s.trim().is_empty()))),
                        "Persisted native review differs from rendered completion"
                    );
                }
                if self.command_stop {
                    let (command_turn, command_item) = self
                        .running_command
                        .as_ref()
                        .context("Missing observed command")?;
                    let item = turns
                        .iter()
                        .find(|t| t["id"] == *command_turn)
                        .and_then(|t| t["items"].as_array())
                        .and_then(|items| items.iter().find(|i| i["id"] == *command_item));
                    anyhow::ensure!(
                        self.running_output.contains("NATIVE_REVIEW_RUNNING")
                            && native_review_interrupted(
                                turns,
                                command_turn,
                                self.stop_turn.as_ref().unwrap()
                            )
                            && item.is_none_or(|item| item["type"] == "commandExecution"
                                && item["status"] != "completed"
                                && !item["aggregatedOutput"]
                                    .as_str()
                                    .unwrap_or("")
                                    .contains("NATIVE_REVIEW_FINISHED")),
                        "Native history does not confirm interruption of the observed review worker"
                    );
                    for native_turn in turns {
                        let projected_turn = thread
                            .turns
                            .iter()
                            .find(|turn| turn.id == native_turn["id"])
                            .context("Native history turn missing from projection")?;
                        anyhow::ensure!(
                            self.observed_items.matches(projected_turn, native_turn),
                            "Review Stop projection omitted native history or contains an item without native evidence in {}",
                            native_turn["id"]
                        );
                    }
                    let rows = app.app_server_messages(owner);
                    let command = rows
                        .iter()
                        .find(|row| {
                            row["id"]
                                == format!(
                                    "native:{}:{command_turn}:{command_item}",
                                    binding.thread_id
                                )
                        })
                        .context("Streamed command missing from the displayed timeline")?;
                    anyhow::ensure!(
                        command["activityStatus"] == "stopped"
                            && !rows.iter().any(|row| row["activityStatus"] == "running"),
                        "Interrupted native review left an activity displayed as running"
                    );
                    println!(
                        "Review host: native history confirms interrupted review/worker; streamed command retained by native history: {}. Persisted items match in order; extra display items have native event/response evidence; no activity is displayed as running.",
                        item.is_some()
                    );
                }
                // Do not assume a fixed count for native worker turn records.
                // Require the display projection to match authoritative history.
                let projected: BTreeMap<_, _> = thread
                    .turns
                    .iter()
                    .map(|turn| (turn.id.clone(), turn.status.clone()))
                    .collect();
                let persisted: BTreeMap<_, _> = turns
                    .iter()
                    .map(|turn| {
                        Ok((
                            turn["id"]
                                .as_str()
                                .context("Missing native turn ID")?
                                .to_owned(),
                            turn["status"]
                                .as_str()
                                .context("Missing native turn status")?
                                .to_owned(),
                        ))
                    })
                    .collect::<anyhow::Result<_>>()?;
                anyhow::ensure!(
                    projected == persisted,
                    "Review display projection differs from native history: projected={projected:?}, persisted={persisted:?}"
                );
                println!(
                    "Review host: native thread/read confirms {} turn records, unchanged seed and acknowledged review status {}.",
                    turns.len(),
                    reviewed["status"]
                );
                let directory = graph
                    .and_then(|g| g.directory(owner))
                    .or_else(|| app.workspace.root())
                    .unwrap();
                if let Some(fixture) = &self.fixture {
                    fixture.verify()?;
                    println!(
                        "Review host: {} successful native commands; source inspection and unchanged Git fixture verified.",
                        self.read_commands
                    );
                } else {
                    anyhow::ensure!(
                        fs::read_dir(directory)?.next().is_none(),
                        "No-tool review changed its empty workspace"
                    );
                }
                println!(
                    "Review host: native history, unchanged workspace and owner/sibling drafts verified."
                );
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }
}

fn unchanged_seed(native: &Value, thread_id: &str, seed: &Value) -> bool {
    native["thread"]["id"] == thread_id
        && native["thread"]["turns"]
            .as_array()
            .is_some_and(|turns| turns.len() == 1 && &turns[0] == seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preparation_stop_oracle_rejects_added_turns_changed_seed_and_wrong_thread() {
        let seed = json!({"id":"seed", "status":"completed", "items":[]});
        let mut native = json!({"thread":{"id":"owned", "turns":[seed.clone()]}});
        assert!(unchanged_seed(&native, "owned", &seed));
        assert!(!unchanged_seed(&native, "other", &seed));
        native["thread"]["turns"][0]["status"] = json!("interrupted");
        assert!(!unchanged_seed(&native, "owned", &seed));
        native["thread"]["turns"] = json!([seed.clone(), {"id":"review", "status":"interrupted"}]);
        assert!(!unchanged_seed(&native, "owned", &seed));
        native["thread"]["turns"] = json!([]);
        assert!(!unchanged_seed(&native, "owned", &seed));
    }
    #[test]
    fn stop_oracle_requires_native_interruption_of_both_review_and_targeted_worker() {
        let mut turns = vec![
            json!({"id":"review","status":"interrupted"}),
            json!({"id":"worker","status":"interrupted"}),
        ];
        assert!(native_review_interrupted(&turns, "review", "review"));
        assert!(native_review_interrupted(&turns, "review", "worker"));
        assert!(!native_review_interrupted(&turns, "review", "missing"));
        assert!(!native_review_interrupted(&turns, "", "worker"));
        for status in ["completed", "failed", "inProgress"] {
            turns[1]["status"] = json!(status);
            assert!(!native_review_interrupted(&turns, "review", "worker"));
        }
        turns[1]["status"] = json!("interrupted");
        turns[0]["status"] = json!("completed");
        assert!(!native_review_interrupted(&turns, "review", "worker"));
    }

    fn fixture(status: &str, with_exit: bool) -> central_agent_codex_runtime::mirror::Mirror {
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        let mut items = vec![json!({"id":"entry","type":"enteredReviewMode","review":"fixture"})];
        if with_exit {
            items.push(
                json!({"id":"exit","type":"exitedReviewMode","review":"Native review result"}),
            );
        }
        mirror
            .hydrate(
                &json!({"id":"thread","status":{"type":"idle"},"turns":[{
                    "id":"review","status":status,"items":items,"error":null
                }]}),
                0,
            )
            .unwrap();
        mirror
    }
    #[test]
    fn review_oracle_requires_exact_native_turn_and_successful_entry_exit() {
        let mirror = fixture("completed", true);
        let thread = mirror.thread("thread").unwrap();
        assert_eq!(
            completed_review(thread, "review").unwrap().as_deref(),
            Some("Native review result")
        );
        assert!(completed_review(thread, "different").unwrap().is_none());
        for status in ["failed", "interrupted"] {
            let mirror = fixture(status, true);
            assert!(completed_review(mirror.thread("thread").unwrap(), "review").is_err());
        }
        let mirror = fixture("inProgress", true);
        assert!(
            completed_review(mirror.thread("thread").unwrap(), "review")
                .unwrap()
                .is_none()
        );
        let mirror = fixture("completed", false);
        assert!(completed_review(mirror.thread("thread").unwrap(), "review").is_err());
    }
}
