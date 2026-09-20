//! Real native continuation across UI provider changes. Claude history/catalog
//! are explicitly local fixtures; this does not test or launch Claude inference.
use super::*;
use delivery::{input, script};
mod profiles;

const LEGACY: &str = "CLAUDE_FIXTURE_ONLY_NEVER_NATIVE_INPUT";
const DRAFT: &str = "Unsent draft survives provider selection";
const SIBLING: &str = "Sibling draft is independent of provider selection";
const PROMPTS: [&str; 2] = [
    "Reply with exactly PROVIDER_FIRST. Do not use tools, read files or change anything.",
    "Reply with exactly PROVIDER_SECOND. Do not use tools, read files or change anything.",
];

fn exact_prompt(params: &Value, expected: &str) -> bool {
    params["input"].as_array().is_some_and(|items| {
        items.len() == 1 && items[0]["type"] == "text" && items[0]["text"] == expected
    }) && !params.to_string().contains(LEGACY)
}

pub(super) fn sample(owner: &str) -> String {
    format!(
        "(()=>{{{}const state={};state.provider=root?.querySelector('#provider-select,[data-role=provider]')?.value;state.legacy=[...root?.querySelectorAll('.chat-message.assistant')||[]].filter(row=>row.textContent.includes({})).length;state.profile=Object.fromEntries(['model','effort','speed'].map(field=>[field,root?.querySelector('#'+field+'-select,[data-role='+field+']')?.value]));state.access=root?.querySelector('.native-access select')?.value;state.confirming=root?.querySelector('dialog[aria-label=\"Allow full Codex access?\"]')?.open===true;return state;}})()",
        delivery::root_script(owner),
        delivery::sample(owner),
        json!(LEGACY)
    )
}

fn select(app: &BrowserApp, owner: &str, provider: &str) -> anyhow::Result<()> {
    if owner.starts_with("graph:") {
        script(
            app,
            owner,
            &format!(
                r#"
            const select=root.querySelector('[data-role="provider"]');
            const save=root.querySelector('[data-action="save"]');
            if(!select||select.disabled||!save||save.disabled) throw new Error('Graph provider unavailable');
            select.value={};select.dispatchEvent(new Event('change',{{bubbles:true}}));save.click();
        "#,
                json!(provider)
            ),
        )
    } else {
        script(
            app,
            owner,
            &format!(
                r#"
            const trigger=document.getElementById('tetra-config-button');
            if(document.getElementById('tetra-config-menu').hidden) trigger.click();
            document.getElementById('provider-picker-button').click();
            const option=[...document.querySelectorAll('#provider-picker-menu button')].find(button=>button.dataset.value==={});
            if(!option||option.disabled) throw new Error('Main provider option unavailable');
            option.click();
            if(!document.getElementById('tetra-config-menu').hidden) trigger.click();
        "#,
                json!(provider)
            ),
        )
    }
}

fn legacy_history(app: &BrowserApp, owner: &str) -> anyhow::Result<Value> {
    if let Some(key) = owner.strip_prefix("graph:") {
        let session = app
            .agent_graph_sessions
            .iter()
            .find(|session| session.node_key == key)
            .context("Legacy graph fixture disappeared")?;
        Ok(serde_json::to_value(&session.turns)?)
    } else {
        Ok(serde_json::to_value(
            app.main_chat_messages().collect::<Vec<_>>(),
        )?)
    }
}

fn seed(app: &mut BrowserApp, owner: &str, directory: &Path) -> anyhow::Result<Value> {
    app.claude_models = vec![AgentModelOption {
        id: "fixture-claude".into(),
        model: "fixture-claude".into(),
        display_name: "Local fixture (no inference)".into(),
        supported_reasoning_efforts: vec![crate::provider_types::AgentReasoningEffortOption {
            reasoning_effort: "low".into(),
            description: "Fixture".into(),
        }],
        default_reasoning_effort: "low".into(),
        is_default: true,
        ..Default::default()
    }];
    app.claude_selection = AgentSelection {
        model: "fixture-claude".into(),
        effort: "low".into(),
        ..Default::default()
    };
    if let Some(key) = owner.strip_prefix("graph:") {
        let binding = app
            .agent_graph_bindings
            .iter()
            .find(|binding| binding.node_key() == key)
            .context("Graph fixture binding missing")?;
        app.agent_graph_sessions.push(serde_json::from_value(json!({
            "nodeKey":key,"recordType":binding.record_type,"recordId":binding.record_id,
            "agentName":binding.name,"projectDirectory":directory,"provider":"claude_code",
            "selection":app.claude_selection,"updatedAtMs":1,
            "turns":[{"runId":990001,"request":"Local fixture request","provider":"claude_code",
                "selection":app.claude_selection,"startedAtMs":1,"finishedAtMs":2,"phase":"completed","status":"Fixture only",
                "messages":[{"id":990001,"role":"assistant","kind":"message","text":LEGACY,"timestampMs":2}],"steps":[]}]
        }))?);
        app.save_agent_graph_sessions();
    } else {
        app.push_chat_message(ChatRole::Assistant, LEGACY.into(), Vec::new());
        app.chat_messages
            .back_mut()
            .context("Fixture missing")?
            .provider = Some(AgentProviderKind::ClaudeCode);
    }
    app.save_session();
    app.render_agent_panel();
    legacy_history(app, owner)
}

#[derive(Default)]
pub(super) struct Providers {
    profiles: Option<profiles::Profiles>,
    phase: u8,
    opened: bool,
    starts: usize,
    read: bool,
    thread: Option<String>,
    directory: Option<PathBuf>,
    legacy: Value,
    error: Option<String>,
}
impl Providers {
    pub(super) fn new(profiles: bool) -> Self {
        Self {
            profiles: profiles.then(profiles::Profiles::default),
            ..Self::default()
        }
    }
    pub(super) fn observe(&mut self, owner: &str, event: &BrowserEvent) {
        let result = (|| -> anyhow::Result<()> {
            let BrowserEvent::AppServer(Event::ConversationReply {
                request, result, ..
            }) = event
            else {
                return Ok(());
            };
            if request.owner != owner {
                return Ok(());
            }
            let value = result.as_ref().map_err(|error| {
                anyhow::anyhow!("Native {} failed: {error}", request.call.method)
            })?;
            anyhow::ensure!(
                !request.call.params.to_string().contains(LEGACY),
                "Legacy transcript entered native request"
            );
            if request.call.method == "turn/start" {
                if let Some(profiles) = &self.profiles {
                    profiles.verify_call(&request.call.params, self.starts)?;
                }
                let prompt = PROMPTS
                    .get(self.starts)
                    .context("Unexpected extra native start")?;
                anyhow::ensure!(
                    exact_prompt(&request.call.params, prompt),
                    "Native input replayed or changed the prompt"
                );
                self.starts += 1;
            }
            if request.call.method == "thread/read" {
                anyhow::ensure!(
                    self.thread.as_deref() == value["thread"]["id"].as_str(),
                    "Readback changed native identity"
                );
                let actual = value["thread"]["cwd"]
                    .as_str()
                    .and_then(|path| fs::canonicalize(path).ok());
                let expected = self
                    .directory
                    .as_ref()
                    .and_then(|path| fs::canonicalize(path).ok());
                anyhow::ensure!(
                    expected.is_some() && expected == actual,
                    "Readback changed native directory"
                );
                self.read = true;
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
            anyhow::bail!("{error}");
        }
        if sample.is_null() {
            return Ok(false);
        }
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
                    || sample["siblings"].as_array().is_none_or(|v| v.len() != 1)
                {
                    return Ok(false);
                }
                script(
                    app,
                    owner,
                    &format!(
                        "for(const card of document.querySelectorAll('.agent-console[data-node-key]')){{if(card!==root){{const input=card.querySelector('textarea[data-role=message]');input.value={};input.dispatchEvent(new Event('input',{{bubbles:true}}));}}}}",
                        json!(SIBLING)
                    ),
                )?;
            }
            self.directory = Some(
                graph
                    .and_then(|graph| graph.directory(owner))
                    .or_else(|| app.workspace.root())
                    .context("No fixture directory")?
                    .to_path_buf(),
            );
            self.legacy = seed(app, owner, self.directory.as_ref().unwrap())?;
            self.phase = 1;
            return Ok(false);
        }
        anyhow::ensure!(
            legacy_history(app, owner)? == self.legacy,
            "Provider change mutated other-provider transcript"
        );
        if self.phase == 1 {
            if sample["legacy"] != 1 {
                return Ok(false);
            }
            if let Some(profiles) = &mut self.profiles
                && !profiles.tick(app, owner, sample, 0)?
            {
                return Ok(false);
            }
            input(app, owner, PROMPTS[0], true)?;
            self.phase = 2;
            println!(
                "Provider host: local Claude history fixture shown; first real Codex prompt submitted."
            );
            return Ok(false);
        }
        if graph.is_some() {
            let siblings = sample["siblings"].as_array().context("Missing sibling")?;
            anyhow::ensure!(
                siblings.len() == 1
                    && siblings[0]["draft"] == SIBLING
                    && siblings[0]["active"] != "true"
                    && siblings[0]["requests"] == 0,
                "Provider change affected sibling graph card"
            );
        }
        anyhow::ensure!(
            app.app_server
                .conversations
                .saved()
                .bindings
                .keys()
                .all(|key| key == owner),
            "Provider selection created another native binding"
        );
        let Some(binding) = app.app_server.conversations.binding(owner) else {
            return Ok(false);
        };
        if let Some(id) = &self.thread {
            anyhow::ensure!(
                *id == binding.thread_id,
                "Provider switch replaced native thread"
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
            thread.turns.len() <= 2 && thread.turns.iter().all(|turn| turn.status != "failed"),
            "Native turn failed or duplicated"
        );
        if app.app_server.busy(owner)
            || thread
                .turns
                .last()
                .is_none_or(|turn| turn.status != "completed")
        {
            return Ok(false);
        }
        match self.phase {
            2 => {
                if self.starts != 1
                    || !sample["text"]
                        .as_str()
                        .unwrap_or_default()
                        .contains("PROVIDER_FIRST")
                {
                    return Ok(false);
                }
                input(app, owner, DRAFT, false)?;
                select(app, owner, "claude_code")?;
                self.phase = 3;
            }
            3 | 4 => {
                let expected = if self.phase == 3 {
                    AgentProviderKind::ClaudeCode
                } else {
                    AgentProviderKind::CodexAppServer
                };
                let actual = if let Some(key) = owner.strip_prefix("graph:") {
                    app.agent_graph_bindings
                        .iter()
                        .find(|b| b.node_key() == key)
                        .map(|b| b.provider)
                } else {
                    Some(app.agent_provider)
                };
                let id = if self.phase == 3 {
                    "claude_code"
                } else {
                    "codex_app_server"
                };
                if actual != Some(expected) || sample["provider"] != id {
                    return Ok(false);
                }
                anyhow::ensure!(
                    sample["draft"] == DRAFT && sample["legacy"] == 1,
                    "Provider selection lost draft or legacy row"
                );
                if self.phase == 3 {
                    select(app, owner, "codex_app_server")?;
                    self.phase = 4;
                } else {
                    if let Some(profiles) = &mut self.profiles
                        && !profiles.tick(app, owner, sample, 1)?
                    {
                        return Ok(false);
                    }
                    input(app, owner, PROMPTS[1], true)?;
                    self.phase = 5;
                    println!(
                        "Provider host: actual selector round trip preserved draft and native thread; continued with Codex."
                    );
                }
            }
            5 => {
                if self.starts != 2
                    || !sample["text"]
                        .as_str()
                        .unwrap_or_default()
                        .contains("PROVIDER_SECOND")
                {
                    return Ok(false);
                }
                app.app_server_conversation(owner.into(), ConversationAction::Read {});
                self.phase = 6;
            }
            6 if self.read => {
                anyhow::ensure!(
                    thread.turns.len() == 2
                        && app.app_server.conversations.saved().unresolved.is_empty(),
                    "Native continuation incomplete"
                );
                let inputs = thread
                    .turns
                    .iter()
                    .flat_map(|turn| &turn.items)
                    .filter(|item| item.value["type"] == "userMessage")
                    .collect::<Vec<_>>();
                anyhow::ensure!(
                    inputs.len() == 2
                        && inputs
                            .iter()
                            .all(|item| !item.value.to_string().contains(LEGACY)),
                    "Native history imported legacy messages"
                );
                app.save_session();
                let path = app.data_dir.join(if graph.is_some() {
                    AGENT_GRAPH_HISTORY_FILE
                } else {
                    PROJECT_CHAT_HISTORY_FILE
                });
                let disk: Value = serde_json::from_slice(&fs::read(path)?)?;
                anyhow::ensure!(
                    disk.to_string().contains(LEGACY)
                        && !disk.to_string().contains("PROVIDER_SECOND"),
                    "Local history lost Claude fixture or copied native transcript"
                );
                println!(
                    "Provider host: two native turns read back on same thread/cwd; other-provider transcript preserved separately on disk."
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
    fn provider_acceptance_rejects_transcript_replay_and_changed_input() {
        let mut params = json!({"input":[{"type":"text","text":PROMPTS[0],"text_elements":[]}]});
        assert!(exact_prompt(&params, PROMPTS[0]));
        assert!(!exact_prompt(&params, PROMPTS[1]));
        params["input"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type":"text","text":LEGACY}));
        assert!(!exact_prompt(&params, PROMPTS[0]));
        params["input"].as_array_mut().unwrap().pop();
        params["baseInstructions"] = json!(LEGACY);
        assert!(!exact_prompt(&params, PROMPTS[0]));
        assert!(!exact_prompt(&json!({}), PROMPTS[0]));
    }

    #[test]
    fn provider_acceptance_sample_is_scoped_to_exact_graph_owner() {
        let script = sample("graph:entity:fixture:conversation");
        assert!(script.contains("card.dataset.nodeKey===\"entity:fixture:conversation\""));
        assert!(script.contains("state.provider=root?.querySelector"));
        assert!(script.contains("state.legacy=[...root?.querySelectorAll"));
        assert!(sample("main:fixture").contains("const root=document;"));
    }
}
