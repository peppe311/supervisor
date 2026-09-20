//! Explicit real-account test of native delegation through production controls.
//! The fixture never grants permission, executes a model itself, or deletes
//! native history. Only the opted-in disposable workspace is inspected.
use super::*;
use central_agent_codex_runtime::requests::Decision;
use delivery::{input, script};

const PROMPT: &str = "Use update_plan for two steps: delegate, then verify. Spawn exactly ONE native subagent, explicitly requesting model gpt-5.6-luna and reasoning_effort low, Standard/default speed. Ask it to read ONLY fixture.txt in the current directory using a read-only tool and reply with the exact file content. Do not tell it the expected value. Do not create or change files. If an approval is declined, use an available read-only method without asking again. Wait for the child and compare its result with SUPERVISOR_DELEGATE_OK. Only if they match, complete the plan and reply exactly VERIFIED SUPERVISOR_DELEGATE_OK. Do not spawn any other agent.";
const STOP: &str = "This is a native approval Stop test. Call exec_command exactly once with cmd=[string]::Concat('SUPERVISOR_CHILD_APPROVAL'), login=false, sandbox_permissions=require_escalated and justification=Print-only child approval test. Use the current working directory. Wait for that request. The test will interrupt you rather than approve it. Do not add commands, read or modify files, retry or delegate.";
const CONTINUE: &str = "Reply exactly SUPERVISOR_CHILD_CONTINUED. Do not use tools, read or change files, or delegate.";

#[derive(Default)]
pub(super) struct Check {
    phase: u8,
    graph_open: bool,
    child: Option<String>,
    child_thread: Option<String>,
    parent_thread: Option<String>,
    stop_turn: Option<String>,
    initial_declines: usize,
}

impl Check {
    pub(super) fn sample(&self, parent: &str) -> String {
        let owner = if self.phase >= 2 {
            self.child.as_deref().unwrap_or(parent)
        } else {
            parent
        };
        format!(
            r#"(()=>{{const state={};{}state.delegates=[...root?.querySelectorAll('[data-delegate-owner]')||[]].map(b=>b.dataset.delegateOwner);state.logThread=root?.querySelector('.native-event-log')?.dataset.threadId;state.logRows=root?.querySelectorAll('.native-event-log li').length||0;state.requestCards=root?.querySelectorAll('.native-requests article').length||0;return state;}})()"#,
            delivery::sample(owner),
            delivery::root_script(owner)
        )
    }

    pub(super) fn tick(
        &mut self,
        app: &mut BrowserApp,
        parent: &str,
        sample: &Value,
        graph: Option<&graph::Graph>,
    ) -> anyhow::Result<bool> {
        if sample.is_null() {
            return Ok(false);
        }
        if self.phase == 0 {
            if !app.app_server.configuration().0.can_run() {
                return Ok(false);
            }
            let selection = &mut app.app_server.profile.selection;
            selection.model = "gpt-5.6-luna".into();
            selection.effort = "low".into();
            // The production Standard picker is None, serialized as explicit
            // serviceTier:null by turn/start (clears a previous fast override).
            selection.service_tier = None;
            provider_types::validate_selection(&app.app_server.view.models, selection)
                .map_err(anyhow::Error::msg)?;
            if let Some(graph) = graph {
                if !self.graph_open {
                    graph.open_cards(app)?;
                    self.graph_open = true;
                    return Ok(false);
                }
                if sample["ready"] != true {
                    return Ok(false);
                }
            }
            let root = graph
                .and_then(|g| g.directory(parent))
                .or_else(|| app.workspace.root())
                .context("Missing fixture directory")?;
            fs::write(root.join("fixture.txt"), "SUPERVISOR_DELEGATE_OK")?;
            app.render_agent_panel();
            input(app, parent, PROMPT, true)?;
            self.phase = 1;
            println!("Delegation host: parent prompt submitted with Luna, low, Standard.");
            return Ok(false);
        }
        let Some(parent_binding) = app.app_server.conversations.binding(parent).cloned() else {
            return Ok(false);
        };
        self.parent_thread
            .get_or_insert(parent_binding.thread_id.clone());
        anyhow::ensure!(
            self.parent_thread.as_deref() == Some(&parent_binding.thread_id),
            "Parent thread was replaced"
        );
        if self.phase == 1 {
            let children = app
                .app_server
                .conversations
                .saved()
                .delegations
                .iter()
                .filter(|(_, p)| **p == parent_binding.thread_id)
                .map(|(c, _)| c.clone())
                .collect::<Vec<_>>();
            anyhow::ensure!(
                children.len() <= 1,
                "The native model created more than one child"
            );
            let owners = app
                .app_server
                .conversations
                .saved()
                .bindings
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            for owner in owners {
                let requests = app.app_server.requests.views(&owner);
                if let Some(request) = requests.first() {
                    anyhow::ensure!(
                        self.initial_declines < 4,
                        "Repeated unexpected native approvals"
                    );
                    let ticket = request.ticket.clone();
                    app.app_server_request(
                        owner,
                        requests::RequestAction::Answer {
                            ticket,
                            decision: Decision::Decline {},
                        },
                    );
                    self.initial_declines += 1;
                }
            }
            let Some(child_thread) = children.first() else {
                return Ok(false);
            };
            let child = app
                .app_server
                .conversations
                .owner_for_thread(child_thread)
                .context("Child has no exact local owner")?
                .to_owned();
            if app.app_server.busy(parent) || app.app_server.busy(&child) {
                return Ok(false);
            }
            let parent_messages = app.app_server_messages(parent);
            if !parent_messages.iter().any(|m| {
                m["messagePhase"] == "final_answer"
                    && m["text"]
                        .as_str()
                        .is_some_and(|t| t.trim() == "VERIFIED SUPERVISOR_DELEGATE_OK")
            }) {
                return Ok(false);
            }
            anyhow::ensure!(
                app.app_server_messages(&child).iter().any(|m| m["text"]
                    .as_str()
                    .is_some_and(|t| t.trim() == "SUPERVISOR_DELEGATE_OK")),
                "The child did not return the observed fixture value"
            );
            anyhow::ensure!(
                !app.app_server
                    .conversations
                    .saved()
                    .lineage
                    .contains_key(child_thread),
                "Native child was misclassified as a fork"
            );
            let saved: Saved =
                serde_json::from_slice(&fs::read(app.data_dir.join("app-server-threads.json"))?)?;
            anyhow::ensure!(
                saved
                    .bindings
                    .get(&child)
                    .is_some_and(|b| &b.thread_id == child_thread)
                    && saved.delegations.get(child_thread) == Some(&parent_binding.thread_id),
                "Native child linkage was not durably saved"
            );
            if !sample["delegates"]
                .as_array()
                .is_some_and(|v| v.contains(&json!(child)))
            {
                return Ok(false);
            }
            script(
                app,
                parent,
                &format!(
                    "const button=[...root.querySelectorAll('[data-delegate-owner]')].find(b=>b.dataset.delegateOwner==={});if(!button)throw new Error('Missing native child control');button.click();",
                    json!(child)
                ),
            )?;
            self.child = Some(child);
            self.child_thread = Some(child_thread.clone());
            self.phase = 2;
            println!(
                "Delegation host: native result verified; child linkage saved and opened through compiled control."
            );
            return Ok(false);
        }
        let child = self.child.as_deref().context("Missing child owner")?;
        let id = self
            .child_thread
            .as_deref()
            .context("Missing child thread")?;
        anyhow::ensure!(
            app.app_server
                .conversations
                .binding(child)
                .is_some_and(|b| b.thread_id == id),
            "Child thread was replaced"
        );
        match self.phase {
            2 => {
                if sample["historyEnabled"] != true || sample["logThread"] != id {
                    return Ok(false);
                }
                script(
                    app,
                    child,
                    "root.querySelector('button[aria-label=\"Load or refresh native Codex history\"]').click();const details=root.querySelector('.native-event-log');if(!details)throw new Error('Missing event diagnostics');details.open=true;",
                )?;
                self.phase = 3;
                println!(
                    "Delegation host: exact child controls mounted; history and diagnostic reads requested."
                );
            }
            3 => {
                if app.app_server.history_pages.busy(child)
                    || sample["logRows"].as_u64().unwrap_or(0) == 0
                    || !app.app_server.conversations.ready_for_turn(child)
                {
                    return Ok(false);
                }
                input(app, child, STOP, true)?;
                self.phase = 4;
            }
            4 => {
                if app.app_server.requests.views(child).is_empty()
                    || sample["requestCards"].as_u64().unwrap_or(0) == 0
                {
                    return Ok(false);
                }
                anyhow::ensure!(
                    app.app_server.requests.views(parent).is_empty(),
                    "Child approval leaked to supervisor"
                );
                self.stop_turn = app
                    .app_server
                    .conversations
                    .active_turn(child)
                    .map(str::to_owned);
                anyhow::ensure!(
                    self.stop_turn.is_some(),
                    "Approval has no active child turn"
                );
                delivery::click(app, child, "stop-agent")?;
                self.phase = 5;
                println!(
                    "Delegation host: native child approval rendered on its own card; Stop clicked without permission grant."
                );
            }
            5 => {
                if app.app_server.busy(child) || !app.app_server.conversations.ready_for_turn(child)
                {
                    return Ok(false);
                }
                let turn = app
                    .app_server
                    .conversations
                    .mirror
                    .thread(id)
                    .and_then(|t| {
                        t.turns
                            .iter()
                            .find(|t| Some(t.id.as_str()) == self.stop_turn.as_deref())
                    })
                    .context("Missing interrupted child turn")?;
                anyhow::ensure!(
                    turn.status == "interrupted",
                    "Child Stop did not interrupt the selected native turn"
                );
                anyhow::ensure!(
                    app.app_server.requests.views(child).is_empty(),
                    "Stopped child retained an approval"
                );
                input(app, child, CONTINUE, true)?;
                self.phase = 6;
            }
            6 => {
                if app.app_server.busy(child) {
                    return Ok(false);
                }
                let messages = app.app_server_messages(child);
                if !messages.iter().any(|m| {
                    m["text"]
                        .as_str()
                        .is_some_and(|t| t.trim() == "SUPERVISOR_CHILD_CONTINUED")
                }) {
                    return Ok(false);
                }
                let root = app
                    .app_server
                    .roots
                    .get(parent)
                    .context("Missing original fixture root")?;
                anyhow::ensure!(
                    fs::read_to_string(root.join("fixture.txt"))? == "SUPERVISOR_DELEGATE_OK",
                    "Fixture was modified"
                );
                let restored = event_log::Journal::load(&app.data_dir);
                let log = restored.view(child, Some(id));
                anyhow::ensure!(
                    log["persistent"] == true
                        && log["entries"].as_array().is_some_and(|entries| entries
                            .iter()
                            .any(|entry| entry["method"] == "turn/interrupt")),
                    "Persisted journal lost the child's stop correlation"
                );
                if let Some(directory) = std::env::var_os("SUPERVISOR_DELEGATION_REPORT_DIR") {
                    let path = PathBuf::from(directory);
                    fs::create_dir_all(&path)?;
                    fs::write(
                        path.join(if graph.is_some() {
                            "graph.json"
                        } else {
                            "main.json"
                        }),
                        serde_json::to_vec_pretty(
                            &json!({"parentOwner":parent,"parentThread":self.parent_thread,"childOwner":child,"childThread":id,"stopTurn":self.stop_turn,"model":"gpt-5.6-luna","effort":"low","serviceTier":"default","initialDeclines":self.initial_declines,"parentMessages":app.app_server_messages(parent),"childMessages":messages,"childEventLog":log,"bindingStore":saved_ids(app)}),
                        )?,
                    )?;
                }
                println!(
                    "Delegation host: child native history, diagnostic UI, Stop, same-thread continuation and persisted metadata verified."
                );
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }
}

fn saved_ids(app: &BrowserApp) -> Value {
    json!(app.app_server.conversations.saved())
}
