//! Real graph host acceptance, sharing the hidden-window/native transport harness.
use super::*;

pub(super) const SAMPLE: &str = r#"({renderer:typeof window.renderAgentGraphSurfaceState,
  navigation:{focus:graph.folderFocusKey,open:[...graph.openAgentKeys],
    nodes:graph.allNodes.map(node=>({key:node.key,category:node.category,path:node.filesystemPath})),
    visible:graph.nodes.map(node=>node.key)},
  cards:[...document.querySelectorAll('.agent-console[data-node-key]')].map(card=>({
    owner:'graph:'+card.dataset.nodeKey,
    draft:card.querySelector('textarea[data-role="message"]')?.value,
    disabled:card.querySelector('[data-action="submit"]')?.disabled,
    active:card.dataset.assignmentBusy,
    text:[...card.querySelectorAll('.chat-message.assistant:not(.reasoning):not(.activity)')].map(row=>row.textContent).join('\n')
  })),errors:window.__nativeAcceptanceErrors || []})"#;
const MARKERS: [&str; 2] = ["NATIVE_GRAPH_A", "NATIVE_GRAPH_B"];
const NEXT: &str = "NATIVE_GRAPH_A_CONTINUED";
const DRAFT: &str = "Unsent draft belongs only to graph B";

// The owned loopback fixtures have no account. Wait for the real startup
// catalog/selection, not the subscription-only can_run() or just connected.
pub(super) fn profile_ready(state: &State) -> bool {
    let (_, models, selection) = state.configuration();
    state.view.connected
        && !state.view.refreshing
        && state.view.requirements_loaded
        && !selection.model.is_empty()
        && !selection.effort.is_empty()
        && provider_types::validate_selection(models, selection).is_ok()
}

struct Node {
    key: String,
    owner: String,
    path: PathBuf,
    thread: Option<String>,
    submissions: usize,
    deltas: usize,
    read: bool,
}

pub(super) struct Graph {
    nodes: Vec<Node>,
    phase: u8,
    overlap: bool,
    error: Option<String>,
}

fn script(app: &BrowserApp, script: &str) -> anyhow::Result<()> {
    app.agent_graph_surface
        .as_ref()
        .context("Missing graph test surface")?
        .evaluate_script(&format!(
            "try {{ {script} }} catch(error) {{ (window.__nativeAcceptanceErrors ||= []).push(String(error)); }}"
        ))
        .map_err(Into::into)
}

fn send(app: &BrowserApp, key: &str, marker: &str) -> anyhow::Result<()> {
    let prompt =
        format!("Reply with exactly {marker}. Do not use tools, read files, or change anything.");
    script(
        app,
        &format!(
            r#"(() => {{
        const card=[...document.querySelectorAll('.agent-console[data-node-key]')].find(card=>card.dataset.nodeKey==={});
        const input=card?.querySelector('textarea[data-role="message"]');
        if(!input || card.querySelector('[data-action="submit"]').disabled) throw new Error('Graph composer not ready');
        input.value={};input.dispatchEvent(new Event('input',{{bubbles:true}}));
        card.querySelector('form.agent-console-compose').requestSubmit();
    }})()"#,
            json!(key),
            json!(prompt)
        ),
    )
}

impl Graph {
    pub(super) fn prepare(app: &mut BrowserApp, root: &Path) -> anyhow::Result<Self> {
        let parent = root.join("graph-projects");
        fs::create_dir(&parent)?;
        let mut project_roots = Vec::new();
        let mut nodes = Vec::new();
        for label in ["A", "B"] {
            let path = parent.join(label);
            fs::create_dir(&path)?;
            let path = fs::canonicalize(path)?;
            let id = stable_graph_node_id(&path.display().to_string());
            let binding = AgentGraphBinding {
                record_type: "entity".into(),
                record_id: id,
                name: format!("Native test {label}"),
                mission: "Disposable graph acceptance fixture; not native model instructions"
                    .into(),
                provider: AgentProviderKind::CodexAppServer,
                project_directory: Some(path.display().to_string()),
                ssh_profile_id: None,
                ..AgentGraphBinding::default()
            };
            let key = binding.node_key();
            nodes.push(Node {
                owner: format!("graph:{key}"),
                key,
                path: path.clone(),
                thread: None,
                submissions: 0,
                deltas: 0,
                read: false,
            });
            project_roots.push(path);
            app.agent_graph_bindings.push(binding);
        }
        app.workspace = WorkspaceRuntime::restore_projects(
            project_roots.clone(),
            Vec::new(),
            project_roots.first().cloned(),
        );
        Ok(Self {
            nodes,
            phase: 0,
            overlap: false,
            error: None,
        })
    }

    pub(super) fn owners(&self) -> Vec<String> {
        self.nodes.iter().map(|node| node.owner.clone()).collect()
    }

    pub(super) fn restart_fixture(&self) -> Value {
        json!({"nodes":self.nodes.iter().map(|node|
            json!({"key":node.key,"owner":node.owner,"path":node.path})).collect::<Vec<_>>()})
    }

    // Restore only fixture addresses. All application bindings/history must have
    // come from BrowserApp's production loader, not this acceptance manifest.
    pub(super) fn restore_fixture(
        app: &BrowserApp,
        root: &Path,
        value: &Value,
    ) -> anyhow::Result<Self> {
        let values = value["nodes"]
            .as_array()
            .context("Missing restart graph nodes")?;
        anyhow::ensure!(values.len() == 2, "Expected two restart graph nodes");
        let mut nodes = Vec::new();
        for (value, label) in values.iter().zip(["A", "B"]) {
            let key = value["key"]
                .as_str()
                .context("Missing graph key")?
                .to_owned();
            let owner = value["owner"]
                .as_str()
                .context("Missing graph owner")?
                .to_owned();
            let path = PathBuf::from(value["path"].as_str().context("Missing graph path")?);
            anyhow::ensure!(
                owner == format!("graph:{key}")
                    && fs::canonicalize(&path)?
                        == fs::canonicalize(root.join("graph-projects").join(label))?,
                "Graph fixture identity changed"
            );
            anyhow::ensure!(
                app.agent_graph_bindings
                    .iter()
                    .any(|binding| binding.node_key() == key
                        && binding.provider == AgentProviderKind::CodexAppServer),
                "Production loader lost graph binding"
            );
            nodes.push(Node {
                key,
                owner,
                path,
                thread: None,
                submissions: 0,
                deltas: 0,
                read: false,
            });
        }
        Ok(Self {
            nodes,
            phase: 0,
            overlap: false,
            error: None,
        })
    }

    pub(super) fn directory(&self, owner: &str) -> Option<&Path> {
        self.nodes
            .iter()
            .find(|node| node.owner == owner)
            .map(|node| node.path.as_path())
    }

    pub(super) fn open_cards(&self, app: &mut BrowserApp) -> anyhow::Result<()> {
        let selection = app.app_server.configuration().2.clone();
        for binding in &mut app.agent_graph_bindings {
            binding.selection = selection.clone();
            anyhow::ensure!(
                normalize_loaded_agent_graph_binding(binding.clone()).is_some(),
                "Graph fixture must survive production persistence validation"
            );
        }
        app.render_agent_graph_surface();
        // Use production card opening, not reconstructed HTML.
        for node in &self.nodes {
            script(
                app,
                &format!(
                    "window.dispatchEvent(new CustomEvent('central-agent:graph-open-conversation',{{detail:{{owner:{},nodeKey:{}}}}}));",
                    json!(node.owner),
                    json!(node.key)
                ),
            )?;
        }
        Ok(())
    }

    pub(super) fn observe(&mut self, app: &BrowserApp, event: &BrowserEvent) {
        for node in &mut self.nodes {
            match event {
                BrowserEvent::AppServer(Event::Transport {
                    event: TransportEvent::Notification { method, params },
                    ..
                }) if method == "item/agentMessage/delta"
                    && app
                        .app_server
                        .conversations
                        .binding(&node.owner)
                        .is_some_and(|binding| params["threadId"] == binding.thread_id) =>
                {
                    node.deltas += 1
                }
                BrowserEvent::AgentPanel(AgentPanelMessage::ContinueAgentGraphAgent {
                    record_type,
                    id,
                    conversation_id,
                    ..
                }) if agent_graph_conversation_key(record_type, id, *conversation_id)
                    == node.key =>
                {
                    node.submissions += 1
                }
                BrowserEvent::AppServer(Event::ConversationReply {
                    request,
                    result: Ok(value),
                    ..
                }) if request.owner == node.owner && request.call.method == "thread/read" => {
                    let cwd = value["thread"]["cwd"]
                        .as_str()
                        .and_then(|cwd| fs::canonicalize(cwd).ok());
                    if cwd != fs::canonicalize(&node.path).ok() {
                        self.error = Some("Graph native readback used the wrong directory".into());
                    }
                    node.read = true;
                }
                _ => {}
            }
        }
    }

    pub(super) fn tick(&mut self, app: &mut BrowserApp, sample: &Value) -> anyhow::Result<bool> {
        anyhow::ensure!(
            self.error.is_none(),
            "{}",
            self.error.as_deref().unwrap_or_default()
        );
        if sample.is_null() {
            return Ok(false);
        }
        anyhow::ensure!(
            sample["renderer"] == "function",
            "Graph renderer missing before first interaction"
        );
        let cards = sample["cards"]
            .as_array()
            .context("Missing graph DOM sample")?;
        if self.phase == 0 {
            if !app.app_server.configuration().0.can_run() {
                return Ok(false);
            }
            self.open_cards(app)?;
            self.phase = 1;
            println!("Graph host: selected fixture parent and requested both cards.");
            return Ok(false);
        }
        if self.phase == 1 {
            if self.nodes.iter().any(|node| {
                cards
                    .iter()
                    .find(|card| card["owner"] == node.owner)
                    .is_none_or(|card| card["disabled"] != false)
            }) {
                return Ok(false);
            }
            for (node, marker) in self.nodes.iter().zip(MARKERS) {
                send(app, &node.key, marker)?;
            }
            println!("Graph host: submitted two independent card forms concurrently.");
            self.phase = 2;
            return Ok(false);
        }
        self.overlap |= self.nodes.iter().all(|node| {
            app.app_server
                .conversations
                .active_turn(&node.owner)
                .is_some()
        });
        let mut complete = true;
        for (index, node) in self.nodes.iter_mut().enumerate() {
            let Some(binding) = app.app_server.conversations.binding(&node.owner) else {
                complete = false;
                continue;
            };
            if let Some(id) = &node.thread {
                anyhow::ensure!(
                    *id == binding.thread_id,
                    "Graph continuation replaced its native thread"
                );
            } else {
                node.thread = Some(binding.thread_id.clone());
            }
            let Some(thread) = app
                .app_server
                .conversations
                .mirror
                .thread(&binding.thread_id)
            else {
                complete = false;
                continue;
            };
            anyhow::ensure!(
                thread.turns.iter().all(|turn| turn.status != "failed"),
                "Graph native turn failed: {:?}",
                thread
                    .turns
                    .iter()
                    .filter_map(|turn| turn.error.as_ref())
                    .collect::<Vec<_>>()
            );
            let count = if self.phase >= 3 && index == 0 { 2 } else { 1 };
            if thread.turns.len() != count
                || thread.turns.iter().any(|turn| turn.status != "completed")
                || app.app_server.busy(&node.owner)
            {
                complete = false;
                continue;
            }
            for (turn, marker) in thread.turns.iter().zip([MARKERS[index], NEXT]) {
                anyhow::ensure!(
                    turn.items.iter().all(|item| matches!(
                        item.value["type"].as_str(),
                        Some("userMessage" | "agentMessage" | "reasoning" | "plan")
                    )),
                    "Unexpected tool use in no-tool graph acceptance"
                );
                anyhow::ensure!(
                    turn.items
                        .iter()
                        .any(|item| item.value["type"] == "agentMessage"
                            && item.value["text"]
                                .as_str()
                                .is_some_and(|text| text.trim() == marker)),
                    "Wrong graph native answer"
                );
            }
            let Some(card) = cards.iter().find(|card| card["owner"] == node.owner) else {
                complete = false;
                continue;
            };
            let text = card["text"].as_str().unwrap_or_default();
            anyhow::ensure!(
                !text.contains(MARKERS[1 - index]),
                "A graph answer reached the sibling card"
            );
            let expected_draft = if self.phase >= 3 && index == 1 {
                DRAFT
            } else {
                ""
            };
            if card["draft"] != expected_draft
                || card["active"] != "false"
                || !text.contains(MARKERS[index])
                || count == 2 && !text.contains(NEXT)
            {
                complete = false;
            }
        }
        if !complete {
            return Ok(false);
        }
        anyhow::ensure!(
            self.nodes[0].thread != self.nodes[1].thread,
            "Graph nodes share a native thread"
        );
        match self.phase {
            2 => {
                anyhow::ensure!(
                    self.overlap,
                    "Concurrent graph turns were not observed; do not claim concurrency acceptance"
                );
                script(
                    app,
                    &format!(
                        r#"(() => {{ const input=document.getElementById({});input.value={};input.dispatchEvent(new Event('input',{{bubbles:true}})); }})()"#,
                        json!(format!("graph-input-{}", self.nodes[1].owner)),
                        json!(DRAFT)
                    ),
                )?;
                send(app, &self.nodes[0].key, NEXT)?;
                println!(
                    "Graph host: both native turns completed; continue A while retaining B's unsent draft."
                );
                self.phase = 3;
            }
            3 => {
                for (index, node) in self.nodes.iter_mut().enumerate() {
                    anyhow::ensure!(
                        node.submissions == if index == 0 { 2 } else { 1 } && node.deltas > 0,
                        "Graph native submission/delta evidence missing"
                    );
                    node.read = false;
                    app.app_server_conversation(node.owner.clone(), ConversationAction::Read {});
                }
                self.phase = 4;
            }
            4 if self.nodes.iter().all(|node| node.read) => {
                anyhow::ensure!(
                    app.app_server.conversations.saved().unresolved.is_empty(),
                    "Graph left unresolved accepted input"
                );
                anyhow::ensure!(
                    app.agent_graph_sessions.is_empty(),
                    "Native graph workflow created an other-provider session"
                );
                anyhow::ensure!(
                    app.app_server
                        .conversations
                        .binding(&app.conversation_key(None))
                        .is_none(),
                    "Graph test touched main native conversation"
                );
                println!(
                    "Graph host: isolated native IDs/directories, overlap, assistant DOM rows, A continuation, B draft and both native readbacks verified."
                );
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }
}

#[cfg(test)]
mod readiness_tests {
    use super::*;

    #[test]
    fn graph_fixture_waits_for_native_catalog_without_requiring_an_account() {
        let mut state = State::default();
        assert!(!profile_ready(&state));
        state.view.connected = true;
        assert!(!profile_ready(&state));
        state.view.requirements_loaded = true;
        state.view.models = serde_json::from_value(json!([{
            "id":"fixture", "model":"fixture-model", "isDefault":true,
            "defaultReasoningEffort":"high",
            "supportedReasoningEfforts":[{"reasoningEffort":"high"}]
        }]))
        .unwrap();
        assert!(!profile_ready(&state));
        state.sync_profile();
        assert!(profile_ready(&state));
        assert!(state.view.account.is_none());
        assert!(!state.configuration().0.can_run());

        state.view.refreshing = true;
        assert!(!profile_ready(&state));
        state.view.refreshing = false;
        state.profile.selection.effort = "unsupported".into();
        assert!(!profile_ready(&state));
        state.profile.selection.effort = "high".into();
        state.view.models.clear();
        assert!(!profile_ready(&state));
    }
}
