//! Explicit compiled-host acceptance using the runtime probe's print-only model
//! fixture. Codex executes commands and stores rules; this drives its real UI
//! request card inside the coordinator-owned profile, never ordinary startup.
use super::*;
use central_agent_codex_runtime::{requests::Decision, transport::Ticket, wire::ServerRequestKey};
mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/crates/central-agent-codex-runtime/src/transport/native_exec_policy_fixture.rs"
    ));
}
pub(super) fn model_output(body: &Value, index: usize) -> Result<Value, String> {
    fixture::model_output(body, index)
}
const DRAFT: &str = "Unsent command-policy draft";
const SIBLING: &str = "Independent command-policy sibling draft";
const BUTTON: &str = "Accept proposed command policy";

pub(super) fn sample(owner: &str) -> String {
    format!(
        "(()=>{{const sample={}; {}sample.buttons=[...root?.querySelectorAll('.native-requests article button')||[]].map(b=>({{text:b.textContent,disabled:b.disabled}}));sample.policyOpen=[...root?.querySelectorAll('.native-requests details')||[]].some(d=>d.querySelector('summary')?.textContent==='Session and policy choices'&&d.open);return sample;}})()",
        approvals::sample(owner),
        approvals::root_script(owner)
    )
}

#[derive(Default)]
pub(super) struct Check {
    graph_open: bool,
    opened: bool,
    phase: u8,
    index: usize,
    directory: Option<PathBuf>,
    turn: Option<String>,
    start: Option<Ticket>,
    request: Option<ServerRequestKey>,
    request_item: Option<String>,
    ui_ticket: Option<String>,
    selected: bool,
    answered: bool,
    resolved: bool,
    completed: bool,
    item: Option<Value>,
    error: Option<String>,
}
impl Check {
    fn identify_turn(&mut self, id: &Value) -> anyhow::Result<()> {
        let id = id
            .as_str()
            .filter(|s| !s.is_empty())
            .context("Missing native policy turn")?;
        anyhow::ensure!(
            self.turn.as_deref().is_none_or(|existing| existing == id),
            "Policy event belongs to another turn"
        );
        self.turn = Some(id.into());
        Ok(())
    }
    pub(super) fn observe(&mut self, app: &BrowserApp, owner: &str, event: &BrowserEvent) {
        if let Err(e) = self.observe_result(app, owner, event) {
            self.error = Some(e.to_string());
        }
    }
    fn observe_result(
        &mut self,
        app: &BrowserApp,
        owner: &str,
        event: &BrowserEvent,
    ) -> anyhow::Result<()> {
        let same = |params: &Value| {
            app.app_server
                .conversations
                .binding(owner)
                .is_some_and(|b| params["threadId"] == b.thread_id)
        };
        match event {
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
                    self.index == 0
                        && self.request.is_none()
                        && same(params)
                        && method == "item/commandExecution/requestApproval"
                        && fixture::owned_policy(params)
                        && params["cwd"]
                            .as_str()
                            .and_then(|p| fs::canonicalize(p).ok())
                            == self
                                .directory
                                .as_ref()
                                .and_then(|p| fs::canonicalize(p).ok()),
                    "Unexpected native command/policy/scope; no approval authorized"
                );
                self.identify_turn(&params["turnId"])?;
                self.request_item = Some(
                    params["itemId"]
                        .as_str()
                        .context("Missing native item ID")?
                        .into(),
                );
                self.request = Some(key.clone());
            }
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::Notification { method, params },
                ..
            }) => {
                if method == "turn/started" {
                    anyhow::ensure!(same(params), "Unexpected sibling policy turn");
                    self.identify_turn(&params["turn"]["id"])?;
                }
                if method == "serverRequest/resolved" {
                    anyhow::ensure!(
                        same(params)
                            && !self.resolved
                            && self
                                .request
                                .as_ref()
                                .is_some_and(|key| json!(key.id) == params["requestId"]),
                        "Wrong native policy resolution"
                    );
                    self.resolved = true;
                }
                if method == "item/started" {
                    anyhow::ensure!(
                        same(params)
                            && matches!(
                                params["item"]["type"].as_str(),
                                Some(
                                    "userMessage"
                                        | "agentMessage"
                                        | "reasoning"
                                        | "commandExecution"
                                )
                            ),
                        "Unexpected work in print-only fixture"
                    );
                }
                if method == "item/completed" && params["item"]["type"] == "commandExecution" {
                    anyhow::ensure!(
                        same(params)
                            && params["turnId"].as_str() == self.turn.as_deref()
                            && self.item.is_none(),
                        "Wrong or repeated native command item"
                    );
                    let item = &params["item"];
                    anyhow::ensure!(
                        item["status"] == "completed"
                            && item["exitCode"] == 0
                            && item["aggregatedOutput"]
                                .as_str()
                                .is_some_and(|s| s.trim() == fixture::MARKER)
                            && (self.index != 0
                                || item["id"].as_str() == self.request_item.as_deref()),
                        "Native print-only command failed: {item}"
                    );
                    self.item = Some(item.clone());
                }
                if method == "turn/completed" {
                    anyhow::ensure!(
                        same(params)
                            && params["turn"]["id"].as_str() == self.turn.as_deref()
                            && params["turn"]["status"] == "completed",
                        "Native policy turn did not complete"
                    );
                    self.completed = true;
                }
            }
            BrowserEvent::AgentPanel(AgentPanelMessage::AppServerRequest {
                owner: reply_owner,
                action: requests::RequestAction::Answer { ticket, decision },
            })
            | BrowserEvent::ScopedAgentPanel {
                message:
                    AgentPanelMessage::AppServerRequest {
                        owner: reply_owner,
                        action: requests::RequestAction::Answer { ticket, decision },
                    },
                ..
            } => {
                anyhow::ensure!(
                    reply_owner == owner
                        && self.ui_ticket.as_ref() == Some(ticket)
                        && matches!(decision, Decision::ExecPolicy {})
                        && !self.selected,
                    "Real UI selected another policy/owner or repeated its answer"
                );
                self.selected = true;
            }
            BrowserEvent::AppServer(Event::Answered {
                owner: reply_owner,
                key,
                result,
                ..
            }) => {
                anyhow::ensure!(
                    reply_owner == owner
                        && self.request.as_ref() == Some(key)
                        && result.is_ok()
                        && self.selected
                        && !self.answered,
                    "Native policy answer failed or changed identity"
                );
                self.answered = true;
            }
            BrowserEvent::AppServer(Event::ConversationReply {
                request, result, ..
            }) if request.owner == owner && result.is_err() => {
                anyhow::bail!("Native policy thread request failed")
            }
            _ => {}
        }
        Ok(())
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
        if !app.app_server.view.connected {
            return Ok(false);
        }
        anyhow::ensure!(
            app.app_server.view.account.is_none(),
            "Policy fixture must not use an account"
        );
        if let Some(graph) = graph
            && !self.graph_open
        {
            if !graph::profile_ready(&app.app_server) {
                return Ok(false);
            }
            graph.open_cards(app)?;
            self.graph_open = true;
            return Ok(false);
        }
        if sample["ready"] != true {
            return Ok(false);
        }
        let directory = graph
            .and_then(|g| g.directory(owner))
            .or_else(|| app.workspace.root())
            .context("Missing policy fixture directory")?
            .to_owned();
        if !self.opened {
            self.directory = Some(directory.clone());
            let request = app
                .app_server
                .conversations
                .open(
                    owner,
                    &directory,
                    &api::Profile::default(),
                    api::Access::WorkspaceWrite,
                )
                .map_err(anyhow::Error::msg)?;
            app.dispatch_app_server_thread(request);
            approvals::script(
                app,
                owner,
                &format!(
                    "input.value={};input.dispatchEvent(new Event('input',{{bubbles:true}}));",
                    json!(DRAFT)
                ),
            )?;
            if let Some(graph) = graph {
                for sibling in graph.owners().iter().filter(|o| *o != owner) {
                    approvals::script(
                        app,
                        sibling,
                        &format!(
                            "input.value={};input.dispatchEvent(new Event('input',{{bubbles:true}}));",
                            json!(SIBLING)
                        ),
                    )?;
                }
            }
            self.opened = true;
            return Ok(false);
        }
        let Some(binding) = app.app_server.conversations.binding(owner) else {
            return Ok(false);
        };
        let thread = binding.thread_id.clone();
        if sample["draft"] != DRAFT {
            return Ok(false);
        }
        if let Some(graph) = graph {
            for sibling in graph.owners().iter().filter(|o| *o != owner) {
                anyhow::ensure!(
                    app.app_server.conversations.binding(sibling).is_none()
                        && app.app_server.requests.views(sibling).is_empty(),
                    "Policy leaked to sibling"
                );
            }
            anyhow::ensure!(
                app.app_server
                    .conversations
                    .binding(&app.conversation_key(None))
                    .is_none(),
                "Graph policy leaked to main chat"
            );
            let siblings = sample["siblings"]
                .as_array()
                .context("Missing sibling sample")?;
            if siblings.is_empty() || siblings.iter().any(|s| s["draft"] != SIBLING) {
                return Ok(false);
            }
            anyhow::ensure!(
                siblings.iter().all(|s| s["requests"] == 0),
                "Sibling shows policy request"
            );
        }
        if self.phase == 0 {
            if app.app_server.conversations.busy(owner) {
                return Ok(false);
            }
            self.start = Some(
                api::start_turn(
                    &thread,
                    &format!("host-policy-{}", self.index),
                    vec![api::text_input(fixture::INPUT)],
                    directory.to_str().context("Invalid fixture cwd")?,
                    &api::Profile::default(),
                    api::Access::WorkspaceWrite,
                )
                .send(app.app_server.client.as_ref().unwrap())?,
            );
            self.phase = if self.index == 0 { 1 } else { 3 };
            println!(
                "Desktop command policy: started native turn {}",
                self.index + 1
            );
            return Ok(false);
        }
        if let Some(ticket) = &self.start {
            match ticket.try_result() {
                Ok(result) => {
                    self.identify_turn(&result?["turn"]["id"])?;
                    self.start = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(error) => anyhow::bail!("Native policy ACK lost: {error}"),
            }
        }
        let cards = sample["cards"]
            .as_array()
            .context("Missing policy card sample")?;
        if self.phase == 1 && self.request.is_some() && cards.len() == 1 {
            let view = app
                .app_server
                .requests
                .views(owner)
                .pop()
                .context("Missing native policy view")?;
            let choices = view.choices.as_ref().context("Missing native choices")?;
            anyhow::ensure!(
                choices.accept
                    && choices.cancel
                    && choices.exec_policy
                    && !choices.decline
                    && !choices.session
                    && choices.network_policies.is_empty(),
                "Native policy options changed"
            );
            let buttons = sample["buttons"]
                .as_array()
                .context("Missing policy buttons")?;
            anyhow::ensure!(
                exact_buttons(buttons),
                "Compiled card shows missing/extra/disabled policy buttons"
            );
            anyhow::ensure!(
                cards[0]["text"]
                    .as_str()
                    .is_some_and(|s| s.contains(fixture::MARKER)),
                "Policy card hides its literal command"
            );
            self.ui_ticket = Some(view.ticket);
            approvals::script(
                app,
                owner,
                "const summary=[...root.querySelectorAll('.native-requests summary')].find(s=>s.textContent==='Session and policy choices');if(!summary)throw new Error('Missing policy disclosure');summary.click();",
            )?;
            self.phase = 2;
            return Ok(false);
        }
        if self.phase == 2 && sample["policyOpen"] == true {
            approvals::script(
                app,
                owner,
                &format!(
                    "const button=[...root.querySelectorAll('.native-requests article button')].find(b=>b.textContent==={});if(!button||button.disabled)throw new Error('Missing policy button');button.click();",
                    json!(BUTTON)
                ),
            )?;
            self.phase = 3;
            return Ok(false);
        }
        if self.phase == 3
            && self.completed
            && self.start.is_none()
            && cards.is_empty()
            && app.app_server.requests.views(owner).is_empty()
        {
            if self.index == 0 && (!self.selected || !self.answered || !self.resolved) {
                return Ok(false);
            }
            if sample["text"]
                .as_str()
                .unwrap_or("")
                .match_indices("READY")
                .count()
                < self.index + 1
            {
                return Ok(false);
            }
            let item = self
                .item
                .as_ref()
                .context("Missing native command completion")?;
            let history = api::read_thread(&thread)
                .send(app.app_server.client.as_ref().unwrap())?
                .wait()?;
            let turns = history["thread"]["turns"]
                .as_array()
                .context("Missing native history")?;
            anyhow::ensure!(
                turns.len() == self.index + 1
                    && turns
                        .last()
                        .is_some_and(|turn| turn["id"].as_str() == self.turn.as_deref()
                            && turn["items"]
                                .as_array()
                                .is_some_and(|items| items.contains(item)
                                    && items.iter().any(
                                        |i| i["type"] == "agentMessage" && i["text"] == "READY"
                                    ))),
                "Native command/history/continuation did not persist"
            );
            println!(
                "Desktop command policy: native turn {} and actual rendered continuation verified",
                self.index + 1
            );
            self.index += 1;
            if self.index == 2 {
                anyhow::ensure!(
                    fs::read_dir(directory)?.next().is_none(),
                    "Print-only fixture changed workspace"
                );
                return Ok(true);
            }
            self.phase = 0;
            self.turn = None;
            self.item = None;
            self.completed = false;
        }
        Ok(false)
    }
}

fn exact_buttons(buttons: &[Value]) -> bool {
    buttons.len() == 3
        && ["Allow once", "Cancel request", BUTTON]
            .iter()
            .all(|expected| {
                buttons
                    .iter()
                    .filter(|b| b["text"] == *expected && b["disabled"] == false)
                    .count()
                    == 1
            })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_card_oracle_rejects_extra_or_missing_native_decisions() {
        let mut buttons = vec![
            json!({"text":"Allow once","disabled":false}),
            json!({"text":"Cancel request","disabled":false}),
            json!({"text":BUTTON,"disabled":false}),
        ];
        assert!(exact_buttons(&buttons));
        buttons.push(json!({"text":"Allow for session","disabled":false}));
        assert!(!exact_buttons(&buttons));
        buttons.pop();
        buttons[0]["disabled"] = json!(true);
        assert!(!exact_buttons(&buttons));
        buttons.pop();
        assert!(!exact_buttons(&buttons));
    }
    #[test]
    fn policy_turn_ack_and_events_require_the_same_native_id() {
        let mut state = Check::default();
        assert!(state.identify_turn(&Value::Null).is_err());
        state.identify_turn(&json!("turn-a")).unwrap();
        state.identify_turn(&json!("turn-a")).unwrap();
        assert!(state.identify_turn(&json!("turn-b")).is_err());
        assert_eq!(state.turn.as_deref(), Some("turn-a"));
    }
}
