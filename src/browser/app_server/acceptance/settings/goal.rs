//! Actual compiled Goal controls and native autonomous turn, no client scheduler.
use super::*;
const OBJECTIVE: &str = "Native goal UI acceptance: reply READY without tools or file access.";
const SIBLING: &str = "Keep this unrelated graph goal draft";
pub(super) fn enabled() -> bool {
    std::env::args_os().any(|a| a == "--native-goal")
}
pub(super) fn output(request: &Value, index: usize) -> Result<Value, String> {
    // Test endpoint latency keeps the native turn observable to both hosts.
    // This is not a production timeout, artificial chat event or tool loop.
    std::thread::sleep(Duration::from_secs(5));
    reload_responses_fixture::ready_output(request, index)
}
pub(super) fn sample(owner: &str) -> String {
    format!(
        r#"(()=>{{try{{{}const state={};const dialog=root?.querySelector('.native-goal');
      const enabled=(label)=>[...dialog?.querySelectorAll('button')||[]].some(b=>b.textContent.trim()===label&&!b.disabled);
      state.goal={{available:[...root?.querySelectorAll('button')||[]].some(b=>b.textContent.trim()==='Goal…'&&!b.disabled),open:dialog?.open,error:dialog?.querySelector('[role=alert]')?.textContent||'',
      state:dialog?.querySelector('h3')?.textContent||'',objective:dialog?.querySelector('.objective')?.textContent||'',
      confirm:!!dialog?.querySelector('[aria-label="Confirm native goal change"]'),
      ready:enabled('Refresh goal'),create:enabled('Create paused goal…'),activate:enabled('Set active…'),pause:enabled('Pause goal…'),remove:enabled('Remove goal…'),
      absent:dialog?.textContent.includes('No native goal in this conversation.')}};return state;
      }}catch(error){{return {{errors:['Goal sample failed: '+String(error)]}};}}}})()"#,
        delivery::root_script(owner),
        delivery::sample(owner)
    )
}
#[derive(Default)]
pub(super) struct Check {
    phase: u8,
    opened: bool,
    directory: Option<PathBuf>,
    thread: Option<String>,
    started: std::collections::BTreeSet<String>,
    completed: std::collections::BTreeSet<String>,
    readback: Option<Value>,
    error: Option<String>,
}
impl Check {
    pub(super) fn observe(&mut self, owner: &str, event: &BrowserEvent) {
        match event {
            BrowserEvent::AppServer(Event::ConversationReply {
                request, result, ..
            }) if request.owner == owner => match result {
                Err(error) => {
                    self.error = Some(format!(
                        "Native Goal host {} failed: {error}",
                        request.call.method
                    ))
                }
                Ok(value) => match request.call.method {
                    "thread/start" => {
                        self.thread = value["thread"]["id"].as_str().map(str::to_owned)
                    }
                    "thread/read" => self.readback = Some(value.clone()),
                    "turn/start" | "turn/steer" => {
                        self.error =
                            Some("Client submitted a prompt during native Goal acceptance".into())
                    }
                    _ => {}
                },
            },
            BrowserEvent::AppServer(Event::GoalReply {
                owner: destination,
                result: Err(error),
                ..
            }) if destination == owner => self.error = Some(format!("Native goal failed: {error}")),
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::ServerRequest { .. },
                ..
            }) => {
                self.error = Some("Unexpected execution authority request; no approval sent".into())
            }
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::Notification { method, params },
                ..
            }) if method == "turn/started" || method == "turn/completed" => {
                if params["threadId"] != json!(self.thread) || self.phase < 6 {
                    self.error = Some("Goal started outside the confirmed native owner".into());
                    return;
                }
                let Some(id) = params["turn"]["id"].as_str() else {
                    self.error = Some("Native turn ID missing".into());
                    return;
                };
                if method == "turn/started" {
                    self.started.insert(id.into());
                } else if params["turn"]["status"] == "completed" {
                    self.completed.insert(id.into());
                } else {
                    self.error = Some(format!(
                        "Native fixture turn failed: {}",
                        params["turn"]["status"]
                    ));
                }
            }
            _ => {}
        }
    }
    pub(super) fn tick(
        &mut self,
        app: &mut BrowserApp,
        owner: &str,
        state: &Value,
        graph: Option<&graph::Graph>,
    ) -> anyhow::Result<bool> {
        if let Some(error) = &self.error {
            anyhow::bail!("{error}");
        }
        if let Some(graph) = graph {
            if !self.opened {
                graph.open_cards(app)?;
                self.opened = true;
                return Ok(false);
            }
            if state["ready"] != true {
                return Ok(false);
            }
        }
        if self.directory.is_none() {
            if graph.is_some() {
                script(
                    app,
                    owner,
                    "root.addEventListener('central-agent:graph-composer-state',event=>root.__nativeDeliveryState=event.detail);",
                )?;
            }
            let directory = graph
                .and_then(|g| g.directory(owner))
                .or_else(|| app.workspace.root())
                .context("Missing Goal test directory")?
                .to_path_buf();
            let request = app
                .app_server
                .conversations
                .open(
                    owner,
                    &directory,
                    &api::Profile::default(),
                    api::Access::ReadOnly,
                )
                .map_err(anyhow::Error::msg)?;
            self.directory = Some(directory);
            app.dispatch_app_server_thread(request);
            input(app, owner, DRAFT, false)?;
            if let Some(graph) = graph {
                anyhow::ensure!(
                    app.app_server
                        .conversations
                        .binding(&app.conversation_key(None))
                        .is_none(),
                    "Graph goal bound the main chat"
                );
                for sibling in graph.owners().into_iter().filter(|s| s != owner) {
                    input(app, &sibling, SIBLING, false)?;
                }
            }
            return Ok(false);
        }
        if !app.app_server.conversations.is_thread_loaded(owner) {
            return Ok(false);
        }
        let binding = app
            .app_server
            .conversations
            .binding(owner)
            .context("Missing Goal binding")?;
        anyhow::ensure!(
            Some(binding.thread_id.as_str()) == self.thread.as_deref(),
            "Goal binding changed"
        );
        if self.phase > 0 {
            anyhow::ensure!(state["draft"] == DRAFT, "Goal changed the unsent draft");
            if let Some(graph) = graph {
                for sibling in graph.owners().into_iter().filter(|s| s != owner) {
                    anyhow::ensure!(
                        app.app_server.conversations.binding(&sibling).is_none(),
                        "Goal bound a sibling"
                    );
                }
                anyhow::ensure!(
                    state["siblings"].as_array().is_some_and(|s| !s.is_empty()
                        && s.iter()
                            .all(|s| s["draft"] == SIBLING && s["active"] != "true")),
                    "Goal changed graph siblings"
                );
            }
        }
        let g = &state["goal"];
        anyhow::ensure!(
            g["error"].as_str().is_none_or(str::is_empty),
            "Goal dialog error: {}",
            g["error"]
        );
        let previous = self.phase;
        match self.phase {
            0 if g["available"] == true => {
                script(
                    app,
                    owner,
                    "const b=[...root.querySelectorAll('button')].find(b=>b.textContent.trim()==='Goal…'&&!b.disabled);if(!b)throw new Error('Goal control unavailable');b.click();",
                )?;
                self.phase = 1;
            }
            1 if g["open"] == true && g["create"] == true => {
                script(
                    app,
                    owner,
                    &format!(
                        "const d=root.querySelector('.native-goal');d.querySelector('details').open=true;const t=d.querySelector('textarea');t.value={};t.dispatchEvent(new Event('input',{{bubbles:true}}));",
                        json!(OBJECTIVE)
                    ),
                )?;
                self.phase = 2;
            }
            2 => {
                click(app, owner, ".native-goal", "Create paused goal…")?;
                self.phase = 3;
            }
            3 if g["confirm"] == true => {
                click(app, owner, ".native-goal", "Confirm change")?;
                self.phase = 4;
            }
            4 if g["state"] == "Paused" && g["activate"] == true => {
                anyhow::ensure!(
                    self.started.is_empty() && g["objective"] == OBJECTIVE,
                    "Paused creation changed objective or started work"
                );
                click(app, owner, ".native-goal", "Set active…")?;
                self.phase = 5;
            }
            5 if g["confirm"] == true => {
                click(app, owner, ".native-goal", "Confirm change")?;
                self.phase = 6;
            }
            6 if g["state"] == "Active"
                && g["pause"] == true
                && state["active"] == "true"
                && state["stop"] == true =>
            {
                anyhow::ensure!(!self.started.is_empty(), "UI active without native turn");
                click(app, owner, ".native-goal", "Pause goal…")?;
                self.phase = 7;
            }
            7 if g["confirm"] == true => {
                click(app, owner, ".native-goal", "Confirm change")?;
                self.phase = 8;
            }
            8 if g["state"] == "Paused"
                && g["ready"] == true
                && state["active"] == "false"
                && state["text"].as_str().is_some_and(|t| t.contains("READY")) =>
            {
                anyhow::ensure!(
                    !self.started.is_empty() && self.started == self.completed,
                    "Native turns still pending"
                );
                click(app, owner, ".native-goal", "Remove goal…")?;
                self.phase = 9;
            }
            9 if g["confirm"] == true => {
                click(app, owner, ".native-goal", "Confirm change")?;
                self.phase = 10;
            }
            10 if g["absent"] == true && g["ready"] == true => {
                click(app, owner, ".native-goal", "Close")?;
                script(
                    app,
                    owner,
                    "const b=root.querySelector('.native-conversation button[aria-label=\"Load or refresh native Codex history\"]');if(!b||b.disabled)throw new Error('History read unavailable');b.click();",
                )?;
                self.phase = 11;
            }
            11 if self.readback.is_some() && g["open"] == false && state["active"] == "false" => {
                let history = self.readback.as_ref().unwrap();
                let turns = history["thread"]["turns"]
                    .as_array()
                    .context("Missing persisted Goal turns")?;
                let turn_ids = turns
                    .iter()
                    .filter_map(|turn| turn["id"].as_str())
                    .collect::<Vec<_>>();
                // App Server 0.153.4 can defer writing an autonomous goal turn's
                // rollout until shutdown. While the session stays loaded, an
                // immediate thread/read may therefore be empty. The production
                // mirror must retain its newer live event; the runtime restart
                // probe separately proves that the rollout is persisted.
                let persisted_history_is_compatible = turns.len() <= self.completed.len()
                    && turns.iter().all(|turn| {
                        turn["id"]
                            .as_str()
                            .is_some_and(|id| self.completed.contains(id))
                    });
                anyhow::ensure!(
                    history["thread"]["id"] == json!(self.thread)
                        && persisted_history_is_compatible
                        && state["text"]
                            .as_str()
                            .is_some_and(|text| text.contains("READY")),
                    "Goal history refresh lost the live turn or invented native turns: thread={}, persisted={turn_ids:?}, completed={:?}",
                    history["thread"]["id"],
                    self.completed
                );
                anyhow::ensure!(
                    fs::read_dir(self.directory.as_ref().unwrap())?
                        .next()
                        .is_none(),
                    "Goal fixture changed workspace files"
                );
                println!(
                    "PASS: {} native Goal create paused/activate/autonomous chat turn/pause/clear/live history; compiled controls, retained drafts and owner isolation; persisted history is covered by the restart probe; no client prompt or scheduler.",
                    if graph.is_some() { "graph" } else { "main" }
                );
                return Ok(true);
            }
            _ => {}
        }
        if previous != self.phase {
            println!("Goal host phase {}", self.phase);
        }
        Ok(false)
    }
}
