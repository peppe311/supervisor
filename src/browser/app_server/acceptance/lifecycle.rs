//! Actual native history controls through the compiled main/graph component.
//! Two opt-in prompts seed owned history and verify independent fork continuation.
use super::*;
use delivery::{input, script};

const PROMPT: &str =
    "Reply with exactly NATIVE_LIFECYCLE_READY. Do not use tools, read files or change anything.";
const DRAFT: &str = "Unsent draft retained through native history management";
const NAME: &str = "Native lifecycle acceptance renamed";
const BRANCH: &str = "Native lifecycle independent fork";
const BRANCH_PROMPT: &str =
    "Reply with exactly NATIVE_FORK_CONTINUED. Do not use tools, read files or change anything.";
const BRANCH_DRAFT: &str = "Unsent draft belongs only to the opened native fork";

pub(super) fn sample(owner: &str) -> String {
    format!(
        r#"(()=>{{{}const state={};
      state.name=root?.querySelector('.native-conversation .conversation-name')?.textContent;
      state.lifecycleText=root?.querySelector('.native-conversation')?.textContent;
      state.lifecycleError=root?.querySelector('.native-conversation [role=alert]')?.textContent;
      state.owner=root?.dataset?.nodeKey ? 'graph:'+root.dataset.nodeKey : (typeof currentConversationOwner==='string'?currentConversationOwner:null);
      state.dialogs=[...root?.querySelectorAll('dialog[open]')||[]].map(d=>({{title:d.querySelector('h2')?.textContent,submit:d.querySelector('button[type=submit]')?.getAttribute('aria-label')}}));
      return state;}})()"#,
        delivery::root_script(owner),
        delivery::sample(owner)
    )
}

pub(super) fn button(app: &BrowserApp, owner: &str, label: &str) -> anyhow::Result<()> {
    println!("Lifecycle host: activate {label}.");
    script(
        app,
        owner,
        &format!(
            r#"
      const details=root.querySelector('.native-conversation');if(!details)throw new Error('Missing lifecycle controls');details.open=true;
      const button=[...details.querySelectorAll('button')].find(b=>b.textContent.trim()==={});
      if(!button||button.disabled)throw new Error('Lifecycle control unavailable: '+{});button.click();
    "#,
            json!(label),
            json!(label)
        ),
    )
}

fn confirm(
    app: &BrowserApp,
    owner: &str,
    expected: &str,
    name: Option<&str>,
) -> anyhow::Result<()> {
    script(
        app,
        owner,
        &format!(
            r#"
      const dialog=[...root.querySelectorAll('dialog[open]')].find(d=>d.querySelector('button[type=submit]')?.getAttribute('aria-label')==={});
      if(!dialog)throw new Error('Missing exact native confirmation');
      const name={};if(name!==null){{const field=dialog.querySelector('input');if(!field)throw new Error('Missing name field');field.value=name;field.dispatchEvent(new Event('input',{{bubbles:true}}));}}
      queueMicrotask(()=>{{const submit=dialog.querySelector('button[type=submit]');if(submit.disabled){{window.__nativeAcceptanceErrors.push('Confirmation disabled');return;}}submit.click();}});
    "#,
            json!(expected),
            json!(name)
        ),
    )
}

#[derive(Default)]
pub(super) struct Lifecycle {
    phase: u8,
    opened: bool,
    starts: usize,
    source: Option<String>,
    branch_owner: Option<String>,
    replies: Vec<(String, Value)>,
    error: Option<String>,
    delete_rejected: bool,
    branch_verified: bool,
}

impl Lifecycle {
    pub(super) fn sample(&self, owner: &str) -> String {
        let branch = self
            .branch_owner
            .as_deref()
            .map(sample)
            .unwrap_or_else(|| "null".into());
        format!(
            "(()=>{{const state={};state.branch={branch};return state;}})()",
            sample(owner)
        )
    }
    pub(super) fn observe(&mut self, owner: &str, event: &BrowserEvent) {
        let result = (|| -> anyhow::Result<()> {
            let BrowserEvent::AppServer(Event::ConversationReply {
                request, result, ..
            }) = event
            else {
                return Ok(());
            };
            if request.owner != owner && Some(&request.owner) != self.branch_owner.as_ref() {
                return Ok(());
            }
            if request.call.method == "thread/delete"
                && request.owner == owner
                && !self.delete_rejected
                && let Err(error) = result
            {
                anyhow::ensure!(
                    error
                        .to_string()
                        .contains("forked history still references it"),
                    "Unexpected native delete rejection: {error}"
                );
                self.delete_rejected = true;
                println!(
                    "Lifecycle host: native fork dependency refused source deletion as expected."
                );
                return Ok(());
            }
            let value = result.as_ref().map_err(|error| {
                anyhow::anyhow!("Native {} failed: {error}", request.call.method)
            })?;
            if request.call.method == "turn/start" {
                let (expected_owner, expected_prompt) = match self.starts {
                    0 => (owner, PROMPT),
                    1 => (
                        self.branch_owner
                            .as_deref()
                            .context("Fork continuation has no owner")?,
                        BRANCH_PROMPT,
                    ),
                    _ => anyhow::bail!("Unexpected lifecycle inference"),
                };
                anyhow::ensure!(
                    request.owner == expected_owner
                        && request.call.params["input"]
                            .as_array()
                            .is_some_and(
                                |items| items.len() == 1 && items[0]["text"] == expected_prompt
                            ),
                    "Lifecycle controls submitted an unexpected model turn"
                );
                self.starts += 1;
            }
            self.replies
                .push((request.call.method.into(), value.clone()));
            Ok(())
        })();
        if let Err(error) = result {
            self.error = Some(error.to_string());
        }
    }

    fn replied(&self, method: &str) -> bool {
        self.replies.iter().any(|(m, _)| m == method)
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
                if sample["ready"] != true {
                    return Ok(false);
                }
            }
            input(app, owner, PROMPT, true)?;
            self.phase = 1;
            println!("Lifecycle host: one seed prompt submitted through the actual composer.");
            return Ok(false);
        }
        let Some(binding) = app.app_server.conversations.binding(owner).cloned() else {
            return Ok(false);
        };
        if let Some(source) = &self.source {
            anyhow::ensure!(
                *source == binding.thread_id,
                "Lifecycle changed source identity"
            );
        } else {
            self.source = Some(binding.thread_id.clone());
        }
        if (19..=23).contains(&self.phase) {
            return self.continue_branch(app, owner, sample);
        }
        if self.phase == 1 {
            let ready = app
                .app_server
                .conversations
                .mirror
                .thread(&binding.thread_id)
                .is_some_and(|thread| {
                    thread.turns.len() == 1 && thread.turns[0].status == "completed"
                });
            if !ready
                || app.app_server.busy(owner)
                || !sample["text"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("NATIVE_LIFECYCLE_READY")
            {
                return Ok(false);
            }
            input(app, owner, DRAFT, false)?;
            self.phase = 2;
            return Ok(false);
        }
        // Every mutation is submitted by the actual button and its actual dialog.
        // Sampling can lag one native event; wait for both authoritative and DOM state.
        if sample["draft"] != DRAFT {
            return Ok(false);
        }
        anyhow::ensure!(
            self.starts == if self.branch_verified { 2 } else { 1 },
            "Unexpected lifecycle inference"
        );
        if app.app_server.busy(owner) {
            return Ok(false);
        }
        let dialog = sample["dialogs"]
            .as_array()
            .and_then(|v| v.first())
            .and_then(|d| d["submit"].as_str());
        match self.phase {
            2 if sample["historyEnabled"] == true => {
                button(app, owner, "Rename")?;
                self.phase = 3;
            }
            3 if dialog == Some("Save name") => {
                confirm(app, owner, "Save name", Some(NAME))?;
                self.phase = 4;
            }
            4 if self.replied("thread/name/set") && sample["name"] == NAME => {
                button(app, owner, "Archive")?;
                self.phase = 5;
            }
            5 if dialog == Some("Archive") => {
                confirm(app, owner, "Archive", None)?;
                self.phase = 6;
            }
            6 if self.replied("thread/archive")
                && binding.archived
                && sample["lifecycleText"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("Archived in Codex") =>
            {
                button(app, owner, "Restore conversation")?;
                self.phase = 7;
            }
            7 if dialog == Some("Restore conversation") => {
                confirm(app, owner, "Restore conversation", None)?;
                self.phase = 8;
            }
            8 if self.replied("thread/unarchive")
                && !binding.archived
                && sample["historyEnabled"] == true =>
            {
                self.replies.retain(|(method, _)| method != "thread/read");
                script(
                    app,
                    owner,
                    "root.querySelector('.native-conversation button[aria-label=\"Load or refresh native Codex history\"]').click();",
                )?;
                self.phase = 9;
            }
            9 if self.replied("thread/read") && sample["historyEnabled"] == true => {
                button(app, owner, "Fork into a new conversation…")?;
                self.phase = 10;
            }
            10 if dialog == Some("Create fork") => {
                confirm(app, owner, "Create fork", Some(BRANCH))?;
                self.phase = 11;
            }
            11 if self.replied("thread/fork") => {
                let branch = app
                    .app_server
                    .last_branches
                    .get(owner)
                    .context("Missing local fork destination")?;
                let fork = app
                    .app_server
                    .conversations
                    .binding(&branch.owner)
                    .context("Missing native fork binding")?;
                anyhow::ensure!(
                    fork.thread_id != binding.thread_id,
                    "Fork reused source identity"
                );
                anyhow::ensure!(
                    app.app_server.conversations.saved().bindings.len() == 2,
                    "Fork touched unrelated conversations"
                );
                self.branch_owner = Some(branch.owner.clone());
                self.replies.retain(|(method, _)| method != "thread/read");
                self.read_fork(app)?;
                self.phase = 12;
            }
            12 if self.replied("thread/read") && sample["historyEnabled"] == true => {
                self.check_fork(app)?;
                if !self.branch_verified {
                    button(app, owner, "Open branch")?;
                    self.phase = 19;
                } else {
                    button(app, owner, "Delete conversation")?;
                    self.phase = 13;
                }
            }
            13 if dialog == Some("Permanently delete") => {
                confirm(app, owner, "Permanently delete", None)?;
                self.phase = 14;
            }
            14 if self.delete_rejected
                && !binding.deleted
                && sample["lifecycleError"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("forked history still references it") =>
            {
                self.replies.retain(|(method, _)| method != "thread/read");
                self.read_fork(app)?;
                self.phase = 15;
            }
            15 if self.replied("thread/read") => {
                self.check_fork(app)?;
                let branch_owner = self.branch_owner.as_ref().unwrap().clone();
                let thread = app
                    .app_server
                    .conversations
                    .binding(&branch_owner)
                    .unwrap()
                    .thread_id
                    .clone();
                // Only the explicit test driver removes its own new fork. Production
                // rejection never deletes dependencies or sends an automatic retry.
                api::delete_thread(&thread)
                    .send(app.app_server.client.as_ref().unwrap())?
                    .wait()?;
                self.phase = 16;
            }
            16 if app
                .app_server
                .conversations
                .binding(self.branch_owner.as_ref().unwrap())
                .is_some_and(|b| b.deleted)
                && sample["historyEnabled"] == true =>
            {
                button(app, owner, "Delete conversation")?;
                self.phase = 17;
            }
            17 if dialog == Some("Permanently delete") => {
                confirm(app, owner, "Permanently delete", None)?;
                self.phase = 18;
            }
            18 if binding.deleted
                && sample["lifecycleText"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("This Codex conversation was deleted") =>
            {
                anyhow::ensure!(
                    app.app_server.conversations.saved().unresolved.is_empty(),
                    "Lifecycle left delivery receipts"
                );
                anyhow::ensure!(
                    app.project_chats
                        .iter()
                        .all(|chat| chat.messages.is_empty()),
                    "Native history copied into another provider"
                );
                println!(
                    "Lifecycle host: dialogs/RPC replies passed; fork readback and native delete refusal preserved history/draft; explicit owned-fork removal then source deletion succeeded."
                );
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }

    fn read_fork(&mut self, app: &BrowserApp) -> anyhow::Result<()> {
        // Independent verification of the owned native result. Main host controls
        // deliberately reject actions on a non-selected local chat; do not weaken
        // that boundary to inspect the fixture's unopened fork.
        let owner = self.branch_owner.as_ref().context("Missing fork owner")?;
        let id = &app
            .app_server
            .conversations
            .binding(owner)
            .context("Missing owned fork")?
            .thread_id;
        let value = api::read_thread(id)
            .send(app.app_server.client.as_ref().unwrap())?
            .wait()?;
        self.replies.push(("thread/read".into(), value));
        Ok(())
    }

    fn continue_branch(
        &mut self,
        app: &mut BrowserApp,
        source: &str,
        sample: &Value,
    ) -> anyhow::Result<bool> {
        let owner = self
            .branch_owner
            .as_deref()
            .context("Missing branch owner")?;
        let state = &sample["branch"];
        let binding = app
            .app_server
            .conversations
            .binding(owner)
            .context("Missing branch binding")?;
        let thread = app
            .app_server
            .conversations
            .mirror
            .thread(&binding.thread_id)
            .context("Missing branch history")?;
        if source.starts_with("graph:") {
            anyhow::ensure!(
                sample["draft"] == DRAFT,
                "Opening graph branch lost source draft"
            );
        } else if self.phase < 23 && (app.composer_owner() != owner || state["owner"] != owner) {
            return Ok(false);
        }
        match self.phase {
            19 if state["historyEnabled"] == true
                && state["text"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("NATIVE_LIFECYCLE_READY") =>
            {
                anyhow::ensure!(
                    state["draft"] == "",
                    "Fork inherited the source's unsent draft"
                );
                anyhow::ensure!(
                    app.app_server.access() == api::Access::ReadOnly,
                    "Fork changed the shared permissions preference"
                );
                input(app, owner, BRANCH_DRAFT, false)?;
                self.phase = 20;
            }
            20 if state["draft"] == BRANCH_DRAFT => {
                input(app, owner, BRANCH_PROMPT, true)?;
                self.phase = 21;
                println!(
                    "Lifecycle host: Open branch showed native history; continued through its actual composer."
                );
            }
            21 if self.starts == 2
                && !app.app_server.busy(owner)
                && thread.turns.len() == 2
                && thread.turns[1].status == "completed"
                && state["text"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("NATIVE_FORK_CONTINUED") =>
            {
                input(app, owner, BRANCH_DRAFT, false)?;
                self.phase = 22;
            }
            22 if state["draft"] == BRANCH_DRAFT => {
                let source_id = &app
                    .app_server
                    .conversations
                    .binding(source)
                    .unwrap()
                    .thread_id;
                let original = api::read_thread(source_id)
                    .send(app.app_server.client.as_ref().unwrap())?
                    .wait()?;
                anyhow::ensure!(
                    original["thread"]["turns"]
                        .as_array()
                        .is_some_and(|turns| turns.len() == 1),
                    "Fork continuation changed source history"
                );
                self.branch_verified = true;
                self.replies.retain(|(method, _)| method != "thread/read");
                self.read_fork(app)?;
                self.check_fork(app)?;
                if let Some(id) = source.strip_prefix("chat:") {
                    script(
                        app,
                        source,
                        &format!(
                            "const row=[...document.querySelectorAll('.workspace-chat-row')].find(row=>row.dataset.chatId==={});const button=row?.querySelector('.workspace-chat-open');if(!button||button.disabled)throw new Error('Source project chat not selectable');button.click();",
                            json!(id)
                        ),
                    )?;
                }
                self.phase = 23;
            }
            23 if (source.starts_with("graph:") || app.composer_owner() == source)
                && sample["owner"] == source
                && sample["draft"] == DRAFT
                && sample["name"] == NAME =>
            {
                println!(
                    "Lifecycle host: original chat/draft restored; native source remains one turn and fork has two."
                );
                self.phase = 12;
            }
            _ => {}
        }
        Ok(false)
    }

    fn check_fork(&self, app: &BrowserApp) -> anyhow::Result<()> {
        let owner = self
            .branch_owner
            .as_deref()
            .context("Missing branch owner")?;
        let binding = app
            .app_server
            .conversations
            .binding(owner)
            .context("Missing branch binding")?;
        let (_, response) = self
            .replies
            .iter()
            .rev()
            .find(|(method, value)| {
                method == "thread/read" && value["thread"]["id"] == binding.thread_id
            })
            .context("Missing independent native fork readback")?;
        anyhow::ensure!(
            !binding.deleted && !binding.archived,
            "Source lifecycle affected fork"
        );
        let thread = &response["thread"];
        let turns = thread["turns"].as_array().context("Missing fork history")?;
        anyhow::ensure!(
            turns.len() == if self.branch_verified { 2 } else { 1 }
                && turns.iter().all(|turn| turn["status"] == "completed"),
            "Fork lost or duplicated a turn"
        );
        let items = turns[0]["items"].as_array().context("Missing fork items")?;
        let users = items
            .iter()
            .filter(|item| item["type"] == "userMessage")
            .collect::<Vec<_>>();
        anyhow::ensure!(
            users.len() == 1 && users[0]["content"][0]["text"] == PROMPT,
            "Fork did not preserve exact input"
        );
        if self.branch_verified {
            let inputs = turns[1]["items"]
                .as_array()
                .context("Missing continued fork items")?
                .iter()
                .filter(|item| item["type"] == "userMessage")
                .collect::<Vec<_>>();
            anyhow::ensure!(
                inputs.len() == 1 && inputs[0]["content"][0]["text"] == BRANCH_PROMPT,
                "Fork continuation changed/replayed input"
            );
        }
        let root = app
            .app_server
            .roots
            .get(owner)
            .context("Missing branch project")?;
        anyhow::ensure!(
            fs::canonicalize(root).ok().is_some_and(|root| thread["cwd"]
                .as_str()
                .and_then(|p| fs::canonicalize(p).ok())
                == Some(root)),
            "Fork changed project directory"
        );
        Ok(())
    }
}
