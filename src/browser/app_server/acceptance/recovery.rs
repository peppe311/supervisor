//! Opt-in real process failure, never a fabricated transport or model event.
use super::*;
use delivery::{input, script};
const FIRST: &str = "Without tools, file reads or changes, print integers from 1 through 400, one per line without skipping. This is an interrupted-stream test.";
const NEXT: &str =
    "Reply with exactly NATIVE_CRASH_CONTINUED. Do not use tools, read files or change anything.";
const DRAFT: &str = "Keep this unsent draft through a real server crash";
const SIBLING: &str = "Graph B is independent of A recovery";

pub(super) fn sample(owner: &str) -> String {
    format!(
        "(()=>{{{}const state={};state.receipt=[...root?.querySelectorAll('.native-requests p')||[]].some(p=>p.textContent.includes('Codex has not confirmed'));state.checkHistory=[...root?.querySelectorAll('.native-requests button')||[]].some(button=>button.textContent==='Check native history'&&!button.disabled);state.reviewEnabled=[...root?.querySelectorAll('.native-requests button')||[]].some(button=>button.textContent==='I reviewed the history · dismiss delivery warning'&&!button.disabled);return state;}})()",
        delivery::root_script(owner),
        delivery::sample(owner)
    )
}

// The process handle targets only the child owned by this disposable host. No
// process-name enumeration, broad taskkill or graceful client shutdown shortcut.
#[cfg(windows)]
fn crash_owned_child(client: &Client) -> anyhow::Result<()> {
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};
    let id = client
        .process_id()
        .context("Owned native child is not running")?;
    anyhow::ensure!(
        id != std::process::id(),
        "Cannot terminate the acceptance host"
    );
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, false, id)?;
        let _owned = OwnedHandle::from_raw_handle(handle.0);
        anyhow::ensure!(
            client.process_id() == Some(id),
            "Native child identity changed"
        );
        TerminateProcess(handle, 97)?;
    }
    println!(
        "Recovery host: terminated only its owned native server PID {id} during real streaming."
    );
    Ok(())
}
#[cfg(not(windows))]
fn crash_owned_child(_: &Client) -> anyhow::Result<()> {
    anyhow::bail!("This process-crash acceptance currently requires Windows")
}

fn has_input(thread: &central_agent_codex_runtime::mirror::Thread, text: &str) -> bool {
    thread
        .turns
        .iter()
        .flat_map(|turn| &turn.items)
        .any(|item| {
            item.value["type"] == "userMessage"
                && item.value["content"]
                    .as_array()
                    .is_some_and(|items| items.iter().any(|value| value["text"] == text))
        })
}

#[derive(Default)]
pub(super) struct Recovery {
    phase: u8,
    opened: bool,
    hooked: bool,
    streamed: bool,
    closed: bool,
    starts: usize,
    reads: usize,
    resumed: bool,
    thread: Option<String>,
    first_turn: Option<String>,
    directory: Option<PathBuf>,
    epoch: u64,
    error: Option<String>,
    lost_ack: bool,
    held_ack: Option<BrowserEvent>,
    receipt: Option<String>,
    stale_sent: bool,
    stale_seen: bool,
    review_sent: bool,
}
impl Recovery {
    pub(super) fn diagnostics(&self, app: &BrowserApp, owner: &str) -> Value {
        let book = &app.app_server.conversations;
        let thread = book
            .binding(owner)
            .and_then(|b| book.mirror.thread(&b.thread_id));
        json!({"phase":self.phase,"reads":self.reads,"starts":self.starts,"resumed":self.resumed,
            "providerReady":app.app_server.configuration().0.can_run(),"hostBusy":app.app_server.busy(owner),
            "pagesBusy":app.app_server.history_pages.busy(owner),"nativeBusy":book.busy(owner),"readyForTurn":book.ready_for_turn(owner),
            "status":thread.and_then(|t|t.status.as_ref()),"turns":thread.map(|t|t.turns.iter().map(|turn|json!({"id":turn.id,"status":turn.status})).collect::<Vec<_>>())})
    }

    pub(super) fn new(lost_ack: bool) -> Self {
        Self {
            lost_ack,
            ..Self::default()
        }
    }

    /// Delay an actual successful worker reply, not its wire payload or model
    /// events. Deliver this exact old-generation event only after reconnect.
    pub(super) fn defer_reply(&mut self, event: BrowserEvent) -> Option<BrowserEvent> {
        if self.lost_ack
            && self.phase == 1
            && self.held_ack.is_none()
            && matches!(&event, BrowserEvent::AppServer(Event::ConversationReply{request,result:Ok(_),..}) if request.call.method=="turn/start")
        {
            self.held_ack = Some(event);
            println!(
                "Recovery host: delayed the real turn/start worker ACK before production host delivery."
            );
            None
        } else {
            Some(event)
        }
    }

    pub(super) fn observe(&mut self, app: &BrowserApp, owner: &str, event: &BrowserEvent) {
        if matches!(event,BrowserEvent::AppServer(Event::ConversationReply{epoch,request,..}) if request.owner==owner && *epoch!=app.app_server.epoch)
        {
            self.stale_seen = true;
            return;
        }
        match event {
            BrowserEvent::AppServer(Event::ConversationReply {
                request, result, ..
            }) if request.owner == owner => match result {
                Err(error) => {
                    self.error = Some(format!("Native {} failed: {error}", request.call.method))
                }
                Ok(value) => match request.call.method {
                    "turn/start" => self.starts += 1,
                    "thread/read" => {
                        self.reads += 1;
                        if value["thread"]["id"].as_str() != self.thread.as_deref()
                            || self
                                .directory
                                .as_ref()
                                .and_then(|p| fs::canonicalize(p).ok())
                                .zip(
                                    value["thread"]["cwd"]
                                        .as_str()
                                        .and_then(|p| fs::canonicalize(p).ok()),
                                )
                                .is_none_or(|(a, b)| a != b)
                        {
                            self.error = Some(
                                "Recovery readback changed native identity or directory".into(),
                            );
                        }
                    }
                    "thread/resume" if self.phase >= 5 => {
                        self.resumed = true;
                        println!(
                            "Recovery host: resume acknowledged in phase {}.",
                            self.phase
                        );
                    }
                    _ => {}
                },
            },
            BrowserEvent::AppServer(Event::Transport {
                epoch,
                event: TransportEvent::Closed { .. },
                ..
            }) if *epoch == self.epoch && self.phase >= 3 => self.closed = true,
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::Notification { method, params },
                ..
            }) if method == "item/agentMessage/delta"
                && app
                    .app_server
                    .conversations
                    .binding(owner)
                    .is_some_and(|binding| params["threadId"] == binding.thread_id) =>
            {
                self.streamed = true
            }
            _ => {}
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
            anyhow::bail!("{error}");
        }
        if sample.is_null() {
            return Ok(false);
        }
        anyhow::ensure!(
            sample["renderer"] == "function",
            "Recovery renderer missing"
        );
        if self.phase == 0 {
            if !app.app_server.configuration().0.can_run() {
                return Ok(false);
            }
            if let Some(graph) = graph {
                if !self.opened {
                    graph.open_cards(app)?;
                    self.opened = true;
                    return Ok(false);
                }
                if sample["ready"] != true
                    || sample["siblings"].as_array().is_none_or(|s| s.len() != 1)
                {
                    return Ok(false);
                }
                if !self.hooked {
                    script(
                        app,
                        owner,
                        &format!(
                            "root.addEventListener('central-agent:graph-composer-state',event=>root.__nativeDeliveryState=event.detail);for(const card of document.querySelectorAll('.agent-console[data-node-key]')){{if(card!==root){{const input=card.querySelector('textarea[data-role=\"message\"]');input.value={};input.dispatchEvent(new Event('input',{{bubbles:true}}));}}}}",
                            json!(SIBLING)
                        ),
                    )?;
                    self.hooked = true;
                    app.render_agent_graph_surface();
                    return Ok(false);
                }
                if sample["stateReady"] != true || sample["siblings"][0]["draft"] != SIBLING {
                    return Ok(false);
                }
            }
            self.directory = graph
                .and_then(|g| g.directory(owner))
                .or_else(|| app.workspace.root())
                .map(Path::to_path_buf);
            anyhow::ensure!(self.directory.is_some(), "No recovery fixture workspace");
            input(app, owner, FIRST, true)?;
            self.phase = 1;
            return Ok(false);
        }
        if graph.is_some() {
            let siblings = sample["siblings"]
                .as_array()
                .context("No sibling observation")?;
            anyhow::ensure!(
                siblings.len() == 1
                    && siblings[0]["draft"] == SIBLING
                    && siblings[0]["active"] != "true"
                    && siblings[0]["requests"] == 0,
                "Recovery affected graph B"
            );
        }
        let book = &app.app_server.conversations;
        anyhow::ensure!(
            book.saved().bindings.keys().all(|key| key == owner),
            "Recovery created another native binding"
        );
        let Some(binding) = book.binding(owner) else {
            return Ok(false);
        };
        if let Some(id) = &self.thread {
            anyhow::ensure!(*id == binding.thread_id, "Recovery replaced native thread");
        } else {
            self.thread = Some(binding.thread_id.clone());
        }
        let Some(thread) = book.mirror.thread(&binding.thread_id) else {
            return Ok(false);
        };
        anyhow::ensure!(
            thread.turns.iter().flat_map(|t| &t.items).all(|i| matches!(
                i.value["type"].as_str(),
                Some("userMessage" | "agentMessage" | "reasoning" | "plan")
            )),
            "Unexpected recovery tool use"
        );
        match self.phase {
            1 if self.starts == 1
                && self.streamed
                && book.active_turn(owner).is_some()
                && sample["draft"] == if self.lost_ack { FIRST } else { "" } =>
            {
                self.first_turn = book.active_turn(owner).map(str::to_owned);
                if self.lost_ack {
                    anyhow::ensure!(self.held_ack.is_some(), "Missing delayed native ACK");
                    self.receipt = Some(
                        book.saved()
                            .unresolved
                            .get(owner)
                            .context("No write-ahead receipt before host ACK")?
                            .message_id
                            .clone(),
                    );
                }
                input(app, owner, DRAFT, false)?;
                self.phase = 2;
            }
            2 if sample["draft"] == DRAFT => {
                anyhow::ensure!(
                    book.active_turn(owner) == self.first_turn.as_deref(),
                    "Turn ended before real crash"
                );
                self.epoch = app.app_server.epoch;
                crash_owned_child(
                    app.app_server
                        .client
                        .as_ref()
                        .context("Missing owned client")?,
                )?;
                self.phase = 3;
            }
            3 if self.closed
                && !app.app_server.view.connected
                && app.app_server.client.is_none() =>
            {
                anyhow::ensure!(
                    self.starts == 1 && !book.observed(owner) && !book.is_thread_loaded(owner),
                    "Disconnected native state remained loaded or resent input"
                );
                if self.lost_ack {
                    anyhow::ensure!(
                        book.saved()
                            .unresolved
                            .get(owner)
                            .map(|r| r.message_id.as_str())
                            == self.receipt.as_deref(),
                        "Lost ACK discarded its unresolved receipt"
                    );
                    let stored = read_bindings(&app.data_dir.join("app-server-threads.json"))
                        .map_err(anyhow::Error::msg)?;
                    anyhow::ensure!(
                        stored
                            .saved()
                            .unresolved
                            .get(owner)
                            .map(|r| r.message_id.as_str())
                            == self.receipt.as_deref(),
                        "Lost ACK receipt did not survive on disk"
                    );
                    if sample["receipt"] != true {
                        return Ok(false);
                    }
                    anyhow::ensure!(
                        sample["reviewEnabled"] == false,
                        "Uninspected delivery could be dismissed"
                    );
                } else {
                    anyhow::ensure!(
                        book.saved().unresolved.is_empty(),
                        "Accepted input became ambiguously undelivered"
                    );
                }
                if sample["draft"] != DRAFT || !self.lost_ack && sample["active"] == "true" {
                    return Ok(false);
                }
                app.app_server_account(AccountAction::Connect);
                self.phase = 4;
            }
            4 if app.app_server.configuration().0.can_run()
                && (sample["historyEnabled"] == true
                    || self.lost_ack && sample["checkHistory"] == true) =>
            {
                anyhow::ensure!(
                    app.app_server.epoch > self.epoch,
                    "Reconnect reused crashed connection"
                );
                if self.lost_ack {
                    if !self.stale_sent {
                        let event = self.held_ack.take().context("Missing actual delayed ACK")?;
                        app.proxy.send_event(event).map_err(|_| {
                            anyhow::anyhow!("Could not deliver delayed native callback")
                        })?;
                        self.stale_sent = true;
                        return Ok(false);
                    }
                    if !self.stale_seen {
                        return Ok(false);
                    }
                    anyhow::ensure!(
                        self.starts == 1
                            && book
                                .saved()
                                .unresolved
                                .get(owner)
                                .map(|r| r.message_id.as_str())
                                == self.receipt.as_deref(),
                        "Late ACK mutated new-generation state"
                    );
                    if sample["draft"] != DRAFT || sample["receipt"] != true {
                        return Ok(false);
                    }
                    anyhow::ensure!(
                        sample["reviewEnabled"] == false,
                        "Late ACK falsely reviewed history"
                    );
                }
                script(
                    app,
                    owner,
                    if self.lost_ack {
                        "[...root.querySelectorAll('.native-requests button')].find(button=>button.textContent==='Check native history').click();"
                    } else {
                        "const panel=root.querySelector('.native-conversation');panel.open=true;panel.querySelector('button[aria-label=\"Load or refresh native Codex history\"]').click();"
                    },
                )?;
                self.phase = 5;
            }
            5 if self.reads == 1
                && !app.app_server.history_pages.busy(owner)
                && (self.lost_ack || sample["resumeEnabled"] == true) =>
            {
                anyhow::ensure!(
                    thread.turns.len() == 1
                        && thread.turns[0].id == self.first_turn.as_deref().unwrap()
                        && has_input(thread, FIRST),
                    "Persisted crash input/turn was lost or duplicated"
                );
                if self.lost_ack {
                    if book.saved().unresolved.contains_key(owner) {
                        if sample["reviewEnabled"] != true || self.review_sent {
                            return Ok(false);
                        }
                        script(
                            app,
                            owner,
                            "[...root.querySelectorAll('.native-requests button')].find(button=>button.textContent==='I reviewed the history · dismiss delivery warning').click();",
                        )?;
                        self.review_sent = true;
                        return Ok(false);
                    }
                    if !self.review_sent {
                        anyhow::ensure!(
                            thread
                                .turns
                                .iter()
                                .flat_map(|turn| &turn.items)
                                .any(|item| item.value["type"] == "userMessage"
                                    && item.value["clientId"].as_str() == self.receipt.as_deref()),
                            "Receipt cleared without exact native clientId or explicit history review"
                        );
                    }
                    if sample["receipt"] != false {
                        return Ok(false);
                    }
                    let stored = read_bindings(&app.data_dir.join("app-server-threads.json"))
                        .map_err(anyhow::Error::msg)?;
                    anyhow::ensure!(
                        !stored.saved().unresolved.contains_key(owner),
                        "Paginated history cleared the receipt only in memory; restart would restore it"
                    );
                    println!(
                        "Recovery host: delayed old-generation ACK ignored; persisted uncertainty resolved on disk only after authoritative history inspection."
                    );
                }
                if sample["resumeEnabled"] != true {
                    return Ok(false);
                }
                script(
                    app,
                    owner,
                    "const panel=root.querySelector('.native-conversation');panel.open=true;const button=[...panel.querySelectorAll('button')].find(button=>button.textContent==='Resume connection');if(!button||button.disabled)throw new Error('Resume unavailable after receipt resolution');button.click();",
                )?;
                self.phase = 6;
            }
            6 if self.resumed
                && book.ready_for_turn(owner)
                && !app.app_server.busy(owner)
                && app.app_server.configuration().0.can_run()
                && sample["active"] == "false"
                && sample["draft"] == DRAFT =>
            {
                anyhow::ensure!(
                    self.starts == 1 && thread.turns.len() == 1 && has_input(thread, FIRST),
                    "Resume replayed the interrupted input"
                );
                println!(
                    "Recovery host: real EOF, retained draft, native history and explicit resume verified. Continuing the same thread."
                );
                input(app, owner, NEXT, true)?;
                self.phase = 7;
            }
            7 if self.starts == 2 && sample["draft"] == "" => {
                input(app, owner, DRAFT, false)?;
                self.phase = 8;
            }
            8 if thread.turns.len() == 2
                && thread.turns[1].status == "completed"
                && !app.app_server.busy(owner) =>
            {
                if sample["draft"] != DRAFT
                    || !sample["text"]
                        .as_str()
                        .unwrap_or_default()
                        .contains("NATIVE_CRASH_CONTINUED")
                {
                    return Ok(false);
                }
                app.app_server_conversation(owner.into(), ConversationAction::Read {});
                self.phase = 9;
            }
            9 if self.reads == 2 && !app.app_server.history_pages.busy(owner) => {
                anyhow::ensure!(
                    self.starts == 2
                        && thread.turns.len() == 2
                        && has_input(thread, FIRST)
                        && has_input(thread, NEXT)
                        && thread.turns[1].status == "completed",
                    "Final crash history not authoritative or duplicated"
                );
                anyhow::ensure!(
                    book.saved().unresolved.is_empty()
                        && app.agent_submission_queue.is_empty()
                        && app.project_chats.iter().all(|c| c.messages.is_empty()),
                    "Crash recovery retained pending input or copied provider history"
                );
                if sample["draft"] != DRAFT || sample["active"] == "true" {
                    return Ok(false);
                }
                println!(
                    "Recovery host: same native thread/cwd, two exact inputs, successful continuation and new draft verified after readback."
                );
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_sampling_keeps_receipt_controls_scoped_to_the_original_owner() {
        let graph = sample("graph:entity:fixture");
        assert!(graph.contains("card.dataset.nodeKey===\"entity:fixture\""));
        assert!(graph.contains("root?.querySelectorAll('.native-requests button')"));
        assert!(graph.contains("Check native history"));
        assert!(graph.contains("state.reviewEnabled"));
        assert!(!graph.contains("document.querySelectorAll('.native-requests"));
        assert!(sample("chat:fixture").contains("const root=document;"));
        assert!(!Recovery::new(false).lost_ack);
        assert!(Recovery::new(true).lost_ack);
    }
    #[test]
    fn recovery_oracle_requires_exact_native_user_content_not_assistant_echoes() {
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        let item = |kind: &str, text: &str| json!({"id":"fixture-item","type":kind,"text":text,"content":[{"type":"text","text":text}]});
        let value = |item: Value| json!({"id":"fixture-thread","status":{"type":"idle"},"turns":[{"id":"fixture-turn","status":"completed","items":[item]}]});
        for (kind, text, expected) in [
            ("agentMessage", FIRST, false),
            ("userMessage", "other input", false),
            ("userMessage", FIRST, true),
        ] {
            mirror
                .hydrate(&value(item(kind, text)), mirror.revision())
                .unwrap();
            assert_eq!(
                has_input(mirror.thread("fixture-thread").unwrap(), FIRST),
                expected
            );
        }
    }
}
