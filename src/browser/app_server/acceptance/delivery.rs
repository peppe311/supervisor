//! Opt-in real desktop delivery acceptance. No manufactured RPC/ACK/model state.
use super::*;

const LONG: &str = "Without tools, file reads or changes, print the integers from 1 through 400, one per line, without skipping any. This is a streaming test.";
const STEER: &str = "Continue the requested count without tools. Add NATIVE_STEERING_APPLIED after the last number.";
const QUEUE: &str =
    "Reply with exactly NATIVE_QUEUE_COMPLETED. Do not use tools, read files or change anything.";
const DRAFT: &str = "Unsent draft must survive Stop and native history readback";
const MAIN_SAMPLE: &str = r#"({renderer:typeof window.renderAgentPanelState,
  draft:document.getElementById('chat-input')?.value,
  active:document.getElementById('composer')?.dataset.agentActive,
  turn:document.getElementById('composer')?.dataset.nativeTurnId,
  steer:document.getElementById('composer')?.dataset.supportsSteer,
  choices:document.getElementById('agent-delivery-options')?.hidden === false,
  stop:document.getElementById('stop-agent')?.hidden === false,
  historyEnabled:document.querySelector('.native-conversation button[aria-label="Load or refresh native Codex history"]')?.disabled === false,
  resumeEnabled:[...document.querySelectorAll('.native-conversation button')].some(button=>button.textContent==='Resume connection' && !button.disabled),
  text:[...document.querySelectorAll('#chat-messages .chat-message.assistant:not(.reasoning):not(.activity)')].map(row=>row.textContent).join('\n'),
  errors:window.__nativeAcceptanceErrors || []})"#;

const SIBLING_DRAFT: &str = "Unsent graph B draft must survive A delivery and reconnect";
fn same_native_directory(expected: Option<&Path>, response: &Value) -> bool {
    expected
        .and_then(|path| fs::canonicalize(path).ok())
        .zip(
            response["thread"]["cwd"]
                .as_str()
                .and_then(|path| fs::canonicalize(path).ok()),
        )
        .is_some_and(|(expected, actual)| expected == actual)
}
pub(super) fn root_script(owner: &str) -> String {
    match owner.strip_prefix("graph:") {
        Some(key) => format!(
            "const root=[...document.querySelectorAll('.agent-console[data-node-key]')].find(card=>card.dataset.nodeKey==={});",
            json!(key)
        ),
        None => "const root=document;".into(),
    }
}
pub(super) fn sample(owner: &str) -> String {
    if !owner.starts_with("graph:") {
        return MAIN_SAMPLE.into();
    }
    format!(
        r#"(() => {{ {} const state=root?.__nativeDeliveryState;return {{
      renderer:typeof window.renderAgentGraphSurfaceState,ready:!!root?.querySelector('textarea[data-role="message"]'),stateReady:!!state,
      draft:root?.querySelector('textarea[data-role="message"]')?.value,active:String(state?.active===true),turn:state?.nativeTurnId,steer:String(state?.supportsSteer===true),
      choices:!!root?.querySelector('.graph-delivery[role="group"]'),stop:root?.querySelector('[data-action="stop"]')?.hidden===false,
      historyEnabled:root?.querySelector('.native-conversation button[aria-label="Load or refresh native Codex history"]')?.disabled===false,
      resumeEnabled:[...root?.querySelectorAll('.native-conversation button')||[]].some(button=>button.textContent==='Resume connection' && !button.disabled),
      text:[...root?.querySelectorAll('.chat-message.assistant:not(.reasoning):not(.activity)')||[]].map(row=>row.textContent).join('\n'),
      siblings:[...document.querySelectorAll('.agent-console[data-node-key]')].filter(card=>card!==root).map(card=>({{owner:'graph:'+card.dataset.nodeKey,draft:card.querySelector('textarea[data-role="message"]')?.value,active:card.dataset.assignmentBusy,requests:card.querySelectorAll('.native-requests article').length}})),
      errors:window.__nativeAcceptanceErrors||[]}}; }})()"#,
        root_script(owner)
    )
}

#[derive(Default)]
pub(super) struct Delivery {
    phase: u8,
    thread: Option<String>,
    first_turn: Option<String>,
    starts: usize,
    steered: bool,
    interrupted: bool,
    read: bool,
    streamed_turns: std::collections::BTreeSet<String>,
    error: Option<String>,
    reconnect_epoch: u64,
    resumed: bool,
    graph_opened: bool,
    graph_observed: bool,
    sibling_ready: bool,
    directory: Option<PathBuf>,
}

pub(super) fn script(app: &BrowserApp, owner: &str, body: &str) -> anyhow::Result<()> {
    let panel = if owner.starts_with("graph:") {
        &app.agent_graph_surface
    } else {
        &app.agent_panel
    };
    panel.as_ref().context("Missing delivery test panel")?
        .evaluate_script(&format!("try {{ {} {body} }} catch(error) {{ (window.__nativeAcceptanceErrors ||= []).push(String(error)); }}",root_script(owner)))?;
    Ok(())
}
pub(super) fn input(app: &BrowserApp, owner: &str, text: &str, enter: bool) -> anyhow::Result<()> {
    script(
        app,
        owner,
        &format!(
            r#"(() => {{
      const input=root.querySelector('textarea[data-role="message"],#chat-input');
      if(!input || input.disabled) throw new Error('Composer not usable');
      input.value={}; input.dispatchEvent(new Event('input',{{bubbles:true}}));
      if({enter}) input.dispatchEvent(new KeyboardEvent('keydown',{{key:'Enter',bubbles:true,cancelable:true}}));
    }})()"#,
            json!(text)
        ),
    )
}
pub(super) fn click(app: &BrowserApp, owner: &str, id: &str) -> anyhow::Result<()> {
    let selector = if owner.starts_with("graph:") {
        match id {
            "stop-agent"=>"root.querySelector('[data-action=\"stop\"]')".into(),
            "agent-delivery-now"=>"[...root.querySelectorAll('.graph-delivery[role=\"group\"] button')].find(button=>button.textContent==='Send now')".into(),
            "agent-delivery-queue"=>"[...root.querySelectorAll('.graph-delivery[role=\"group\"] button')].find(button=>button.textContent==='Queue')".into(),
            _=>anyhow::bail!("Unknown graph delivery control"),
        }
    } else {
        format!("document.getElementById({})", json!(id))
    };
    script(
        app,
        owner,
        &format!(
            "const button={selector}; if(!button || button.hidden || button.disabled) throw new Error('Delivery button unavailable'); button.click();"
        ),
    )
}

impl Delivery {
    pub(super) fn observe(&mut self, app: &BrowserApp, owner: &str, event: &BrowserEvent) {
        match event {
            BrowserEvent::AppServer(Event::ConversationReply {
                request, result, ..
            }) if request.owner == owner => match result {
                Err(error) => {
                    self.error = Some(format!("Native {} failed: {error}", request.call.method))
                }
                Ok(value) => match request.call.method {
                    "turn/start" => self.starts += 1,
                    "turn/steer" => {
                        if self.first_turn.as_deref() != value["turnId"].as_str() {
                            self.error =
                                Some("Steering ACK did not target the observed first turn".into());
                        }
                        self.steered = true;
                    }
                    "turn/interrupt" => self.interrupted = true,
                    "thread/read" => {
                        self.read = true;
                        if !same_native_directory(self.directory.as_deref(), value) {
                            self.error =
                                Some("Native delivery history has a different directory".into());
                        }
                    }
                    "thread/resume" if self.phase >= 11 => self.resumed = true,
                    _ => {}
                },
            },
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
                if let Some(turn) = params["turnId"].as_str() {
                    self.streamed_turns.insert(turn.into());
                }
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
            "Delivery renderer missing"
        );
        if self.phase == 0 {
            if !app.app_server.configuration().0.can_run() {
                return Ok(false);
            }
            self.directory = graph
                .and_then(|g| g.directory(owner))
                .or_else(|| app.workspace.root())
                .map(Path::to_path_buf);
            anyhow::ensure!(
                self.directory.is_some(),
                "Delivery test requires an explicit existing local directory"
            );
            if let Some(graph) = graph {
                if !self.graph_opened {
                    graph.open_cards(app)?;
                    self.graph_opened = true;
                    return Ok(false);
                }
                if sample["ready"] != true
                    || sample["siblings"]
                        .as_array()
                        .is_none_or(|cards| cards.len() != 1)
                {
                    return Ok(false);
                }
                if !self.graph_observed {
                    script(
                        app,
                        owner,
                        &format!(
                            "root.addEventListener('central-agent:graph-composer-state',event=>root.__nativeDeliveryState=event.detail);for(const card of document.querySelectorAll('.agent-console[data-node-key]')){{if(card!==root){{const input=card.querySelector('textarea[data-role=\"message\"]');input.value={};input.dispatchEvent(new Event('input',{{bubbles:true}}));}}}}",
                            json!(SIBLING_DRAFT)
                        ),
                    )?;
                    self.graph_observed = true;
                    app.render_agent_graph_surface();
                    return Ok(false);
                }
                if sample["stateReady"] != true {
                    return Ok(false);
                }
            } else {
                app.render_agent_panel();
            }
            input(app, owner, LONG, true)?;
            self.phase = 1;
            println!("Delivery host: started read-only streaming turn.");
            return Ok(false);
        }
        if graph.is_some() {
            let siblings = sample["siblings"]
                .as_array()
                .context("Missing graph sibling observation")?;
            if !self.sibling_ready && (siblings.len() != 1 || siblings[0]["draft"] != SIBLING_DRAFT)
            {
                return Ok(false);
            }
            self.sibling_ready = true;
            anyhow::ensure!(
                siblings.len() == 1
                    && siblings[0]["draft"] == SIBLING_DRAFT
                    && siblings[0]["active"] != "true"
                    && siblings[0]["requests"] == 0,
                "Delivery changed the sibling graph card"
            );
            anyhow::ensure!(
                app.app_server
                    .conversations
                    .saved()
                    .bindings
                    .keys()
                    .all(|key| key == owner),
                "Delivery created another card or main native conversation"
            );
        }
        let Some(binding) = app.app_server.conversations.binding(owner) else {
            return Ok(false);
        };
        if let Some(thread) = &self.thread {
            anyhow::ensure!(
                thread == &binding.thread_id,
                "Delivery replaced native history"
            );
        } else {
            self.thread = Some(binding.thread_id.clone());
        }
        let Some(thread) = app
            .app_server
            .conversations
            .mirror
            .thread(&binding.thread_id)
        else {
            return Ok(false);
        };
        anyhow::ensure!(
            thread.turns.iter().all(|turn| turn.status != "failed"),
            "Native delivery turn failed"
        );
        anyhow::ensure!(
            thread
                .turns
                .iter()
                .flat_map(|turn| &turn.items)
                .all(|item| matches!(
                    item.value["type"].as_str(),
                    Some("userMessage" | "agentMessage" | "reasoning" | "plan")
                )),
            "Unexpected tool use in delivery acceptance"
        );
        let active = app.app_server.conversations.active_turn(owner);
        match self.phase {
            1 => {
                if let Some(turn) = active.filter(|turn| {
                    sample["turn"] == *turn && sample["steer"] == "true" && sample["draft"] == ""
                }) {
                    anyhow::ensure!(
                        sample["choices"] == false,
                        "Delivery choices appeared before a follow-up"
                    );
                    self.first_turn = Some(turn.into());
                    input(app, owner, STEER, true)?;
                    self.phase = 2;
                }
            }
            2 if sample["choices"] == true => {
                click(app, owner, "agent-delivery-now")?;
                self.phase = 3;
            }
            3 if self.steered && sample["draft"] == "" => {
                anyhow::ensure!(
                    active == self.first_turn.as_deref(),
                    "First turn ended before queue acceptance could be exercised"
                );
                input(app, owner, QUEUE, true)?;
                self.phase = 4;
            }
            4 if sample["choices"] == true => {
                click(app, owner, "agent-delivery-queue")?;
                self.phase = 5;
            }
            5 if sample["draft"] == ""
                && app
                    .agent_submission_queue
                    .iter()
                    .any(|entry| entry.owner() == Some(owner) && entry.message == QUEUE) =>
            {
                anyhow::ensure!(
                    self.starts == 1 && active == self.first_turn.as_deref(),
                    "Queue did not remain queued behind the active turn"
                );
                input(app, owner, DRAFT, false)?;
                self.phase = 6;
                println!(
                    "Delivery host: native steering ACK and held queue verified; retain a newer draft during dequeue."
                );
            }
            6 if thread.turns.len() == 2
                && thread.turns.iter().all(|turn| turn.status == "completed")
                && !app.app_server.busy(owner) =>
            {
                anyhow::ensure!(self.starts == 2, "Queue started more than one native turn");
                if sample["draft"] != DRAFT
                    || !sample["text"]
                        .as_str()
                        .unwrap_or_default()
                        .contains("NATIVE_QUEUE_COMPLETED")
                {
                    return Ok(false);
                }
                anyhow::ensure!(
                    thread.turns[0]
                        .items
                        .iter()
                        .any(|item| item.value["type"] == "userMessage"
                            && item.value.to_string().contains("NATIVE_STEERING_APPLIED")),
                    "Native history did not retain steering input"
                );
                anyhow::ensure!(
                    app.agent_submission_queue.is_empty(),
                    "Completed queue retained an input"
                );
                input(app, owner, LONG, true)?;
                self.phase = 7;
                println!(
                    "Delivery host: queued reply rendered once; newer draft survived dequeue. Starting Stop test."
                );
            }
            7 if thread.turns.len() == 3
                && sample["draft"] == ""
                && sample["stop"] == true
                && active.is_some_and(|turn| self.streamed_turns.contains(turn)) =>
            {
                input(app, owner, DRAFT, false)?;
                click(app, owner, "stop-agent")?;
                self.phase = 8;
            }
            8 if self.interrupted
                && thread.turns.len() == 3
                && thread.turns[2].status == "interrupted"
                && !app.app_server.busy(owner) =>
            {
                if sample["draft"] != DRAFT || sample["active"] == "true" {
                    return Ok(false);
                }
                app.app_server_conversation(owner.into(), ConversationAction::Read {});
                self.phase = 9;
            }
            9 if self.read && !app.app_server.any_busy() => {
                anyhow::ensure!(
                    self.starts == 3
                        && thread.turns.len() == 3
                        && thread.turns[2].status == "interrupted",
                    "Native readback lost turn identity or interruption"
                );
                anyhow::ensure!(
                    app.app_server.conversations.saved().unresolved.is_empty()
                        && app.agent_submission_queue.is_empty(),
                    "Delivery left pending input"
                );
                anyhow::ensure!(
                    app.project_chats
                        .iter()
                        .all(|chat| chat.messages.is_empty()),
                    "Native delivery copied an other-provider transcript"
                );
                if sample["draft"] != DRAFT
                    || sample["active"] == "true"
                    || sample["choices"] == true
                {
                    return Ok(false);
                }
                println!(
                    "Delivery host: native steering, queue, Stop, three-turn readback and draft preservation verified."
                );
                self.read = false;
                self.reconnect_epoch = app.app_server.epoch;
                app.app_server_account(AccountAction::Connect);
                self.phase = 10;
            }
            10 if app.app_server.configuration().0.can_run()
                && sample["historyEnabled"] == true =>
            {
                anyhow::ensure!(
                    app.app_server.epoch > self.reconnect_epoch,
                    "Reconnect did not replace the native connection"
                );
                script(
                    app,
                    owner,
                    "const panel=root.querySelector('.native-conversation'); panel.open=true; panel.querySelector('button[aria-label=\"Load or refresh native Codex history\"]').click();",
                )?;
                self.phase = 11;
            }
            11 if self.read && !app.app_server.any_busy() && sample["resumeEnabled"] == true => {
                anyhow::ensure!(
                    app.app_server.conversations.observed(owner),
                    "New connection did not observe native history"
                );
                script(
                    app,
                    owner,
                    "const panel=root.querySelector('.native-conversation'); panel.open=true; [...panel.querySelectorAll('button')].find(button=>button.textContent==='Resume connection').click();",
                )?;
                self.phase = 12;
            }
            12 if self.resumed && app.app_server.conversations.ready_for_turn(owner) => {
                anyhow::ensure!(
                    self.starts == 3
                        && thread.turns.len() == 3
                        && thread.turns[2].status == "interrupted",
                    "Reconnect replayed input or changed native history"
                );
                anyhow::ensure!(
                    app.app_server.conversations.saved().unresolved.is_empty()
                        && app.agent_submission_queue.is_empty(),
                    "Reconnect introduced pending input"
                );
                if sample["draft"] != DRAFT
                    || sample["active"] == "true"
                    || !sample["text"]
                        .as_str()
                        .unwrap_or_default()
                        .contains("NATIVE_QUEUE_COMPLETED")
                {
                    return Ok(false);
                }
                println!(
                    "Delivery host: new connection, actual Load history/Resume controls, same native thread, three unchanged turns and retained draft verified without replay."
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
    fn readback_requires_existing_exact_directory_not_two_missing_values() {
        let root = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        assert!(same_native_directory(
            Some(root.path()),
            &json!({"thread":{"cwd":root.path()}})
        ));
        assert!(!same_native_directory(
            Some(root.path()),
            &json!({"thread":{"cwd":other.path()}})
        ));
        assert!(!same_native_directory(None, &json!({})));
        assert!(!same_native_directory(
            Some(root.path()),
            &json!({"thread":{}})
        ));
    }
    #[test]
    fn graph_sampling_is_scoped_and_observes_production_composer_state() {
        let code = sample("graph:entity:fixture");
        assert!(code.contains("card.dataset.nodeKey===\"entity:fixture\""));
        assert!(code.contains("root?.__nativeDeliveryState"));
        assert!(code.contains("filter(card=>card!==root)"));
        assert!(!code.contains("root=document"));
        assert_eq!(sample("chat:fixture"), MAIN_SAMPLE);
        let unusual = "entity:quote\";throw new Error('not code')";
        assert!(root_script(&format!("graph:{unusual}")).contains(&json!(unusual).to_string()));
    }
}
