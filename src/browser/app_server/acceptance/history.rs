//! Native history UI acceptance with an isolated desktop profile. Two opted-in
//! one-word prompts seed owned native history; every list query uses its UUID prefix.
use super::*;
use delivery::{input, script};

const ROOT: &str = "CENTRAL_AGENT_HISTORY_TEST_ROOT";
const TOKEN: &str = "CENTRAL_AGENT_HISTORY_TEST_TOKEN";
const DRAFT: &str = "Native history import must retain this unsent draft";
const MARKER: &str = "NATIVE_HISTORY_FIXTURE";
fn title() -> String {
    format!(
        "{} archived",
        std::env::var(TOKEN).expect("Validated fixture token")
    )
}
fn active_title() -> String {
    format!(
        "{} active",
        std::env::var(TOKEN).expect("Validated fixture token")
    )
}
fn seed_turn(client: &Client, id: &str, directory: &Path) -> anyhow::Result<()> {
    api::start_turn(
        id,
        &Uuid::new_v4().to_string(),
        vec![api::text_input(&format!(
            "Reply with exactly {MARKER}. Do not use tools, read files or change anything."
        ))],
        directory.to_str().unwrap(),
        &api::Profile::default(),
        api::Access::ReadOnly,
    )
    .send(client)?
    .wait()?;
    Ok(())
}

pub(super) fn context() -> anyhow::Result<Option<PathBuf>> {
    let Some(root) = std::env::var_os(ROOT) else {
        return Ok(None);
    };
    let root = fs::canonicalize(root)?;
    let token = std::env::var(TOKEN)?;
    anyhow::ensure!(
        root.parent() == Some(fs::canonicalize(std::env::temp_dir())?.as_path())
            && root.file_name().is_some_and(|name| name
                .to_string_lossy()
                .starts_with("central-native-history-"))
            && Uuid::parse_str(&token).is_ok()
            && fs::read_to_string(root.join("owner-token"))? == token,
        "Invalid owned history fixture context"
    );
    Ok(Some(root))
}

pub(super) fn coordinate(graph: bool, fork: bool) -> anyhow::Result<()> {
    let root = tempfile::Builder::new()
        .prefix("central-native-history-")
        .tempdir()?;
    let token = Uuid::new_v4().to_string();
    fs::write(root.path().join("owner-token"), &token)?;
    let mut child = std::process::Command::new(std::env::current_exe()?);
    child
        .arg("--check-native-history")
        .arg("--allow-test-inference")
        .env(ROOT, root.path())
        .env(TOKEN, token)
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY");
    if graph {
        child.arg("--graph");
    }
    if fork {
        child.arg("--fork-history");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        child.creation_flags(0x08000000);
    }
    let status = child.status()?;
    cleanup_profile(root)?;
    anyhow::ensure!(
        status.success(),
        "Native history acceptance child failed: {status}"
    );
    println!(
        "History coordinator: disposable desktop profile removed; only exact owned native histories were deleted."
    );
    Ok(())
}

fn cleanup_profile(root: tempfile::TempDir) -> anyhow::Result<()> {
    let path = fs::canonicalize(root.path())?;
    anyhow::ensure!(
        path.parent() == Some(fs::canonicalize(std::env::temp_dir())?.as_path())
            && path.file_name().is_some_and(|name| name
                .to_string_lossy()
                .starts_with("central-native-history-")),
        "Unexpected history cleanup scope"
    );
    let _ = root.keep();
    for attempt in 0..50 {
        match fs::remove_dir_all(&path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) if matches!(error.raw_os_error(), Some(32 | 33 | 145)) && attempt < 49 => {
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(error) => anyhow::bail!("Owned fixture retained at {}: {error}", path.display()),
        }
    }
    unreachable!()
}

pub(super) fn sample(owner: &str) -> String {
    format!(
        r#"(()=>{{{}const state={};const d=root?.querySelector('dialog.native-history');
      state.history={{secure:window.isSecureContext,uuid:typeof crypto.randomUUID,random:typeof crypto.getRandomValues,open:d?.open,loading:!!d?.querySelector('[role=status]'),error:d?.querySelector('[role=alert]')?.textContent,
        titles:[...d?.querySelectorAll('.results article h3')||[]].map(n=>n.textContent),
        confirmation:d?.querySelector('.confirmation')?.textContent,
        archive:d?.querySelector('input[type=checkbox]')?.checked,
        search:d?.querySelector('input:not([type=checkbox])')?.value}};
      return state;}})()"#,
        delivery::root_script(owner),
        delivery::sample(owner)
    )
}

fn search(app: &BrowserApp, owner: &str, query: &str, archived: bool) -> anyhow::Result<()> {
    println!("History host: search {query}, archived={archived}.");
    script(
        app,
        owner,
        &format!(
            r#"
      const dialog=root.querySelector('dialog.native-history[open]');if(!dialog)throw new Error('Native history dialog missing');
      const input=dialog.querySelector('input:not([type=checkbox])');input.value={};input.dispatchEvent(new Event('input',{{bubbles:true}}));
      const checkbox=dialog.querySelector('input[type=checkbox]');if(checkbox.checked!=={archived})checkbox.click();
      queueMicrotask(()=>dialog.querySelector('form').requestSubmit());
    "#,
            json!(query)
        ),
    )
}

pub(super) struct History {
    phase: u8,
    fork: bool,
    opened: bool,
    owned: Vec<String>,
    seed_done: bool,
    active_done: bool,
    error: Option<String>,
    imported: bool,
    choice_cancelled: bool,
    active_choice: bool,
    last_history: Value,
}
impl History {
    pub(super) fn new(fork: bool) -> Self {
        Self {
            phase: 0,
            fork,
            opened: false,
            owned: vec![],
            seed_done: false,
            active_done: false,
            error: None,
            imported: false,
            choice_cancelled: false,
            active_choice: false,
            last_history: Value::Null,
        }
    }

    pub(super) fn observe(&mut self, owner: &str, event: &BrowserEvent) {
        match event {
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::Notification { method, params },
                ..
            }) if method == "turn/completed"
                && self.owned.iter().any(|id| params["threadId"] == *id) =>
            {
                if params["turn"]["status"] == "completed" {
                    if params["threadId"] == self.owned[0] {
                        self.seed_done = true;
                    } else {
                        self.active_done = true;
                    }
                } else {
                    self.error = Some(format!(
                        "Native fixture turn failed: {}",
                        params["turn"]["error"]
                    ));
                }
            }
            BrowserEvent::AppServer(Event::ConversationReply {
                request, result, ..
            }) if request.owner == owner => {
                if request.call.method == "turn/start" {
                    self.error = Some("History import unexpectedly submitted inference".into());
                }
                if let Ok(value) = result {
                    if request.call.method == "thread/fork" {
                        if let Some(id) = value["thread"]["id"].as_str() {
                            self.owned.push(id.into());
                        }
                        self.imported = true;
                    } else if request.call.method == "thread/read" {
                        self.imported = true;
                    }
                } else if let Err(error) = result {
                    self.error = Some(format!(
                        "Native history {} failed: {error}",
                        request.call.method
                    ));
                }
            }
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::ServerRequest { .. },
                ..
            }) => {
                self.error =
                    Some("Unexpected approval; history fixture cannot grant permissions".into());
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
        if !app.app_server.view.connected || sample.is_null() {
            return Ok(false);
        }
        if app.app_server.view.account.is_none() {
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
        let client = app
            .app_server
            .client
            .as_ref()
            .context("Missing native client")?;
        let directory = graph
            .and_then(|graph| graph.directory(owner))
            .or_else(|| app.workspace.root())
            .context("Missing fixture cwd")?
            .to_path_buf();
        if self.phase == 0 {
            println!("History host capabilities: {}", sample["history"]);
            let value = api::start_thread(
                directory.to_str().unwrap(),
                &api::Profile::default(),
                api::Access::ReadOnly,
            )
            .send(client)?
            .wait()?;
            let id = value["thread"]["id"]
                .as_str()
                .context("Missing fixture thread id")?
                .to_owned();
            self.owned.push(id.clone());
            seed_turn(client, &id, &directory)?;
            input(app, owner, DRAFT, false)?;
            self.phase = 1;
            println!("History host: first of two opted-in fixture prompts submitted.");
            return Ok(false);
        }
        if sample["draft"] != DRAFT {
            return Ok(false);
        }
        if self.phase == 1 && self.seed_done {
            let id = &self.owned[0];
            let read = api::read_thread(id).send(client)?.wait()?;
            anyhow::ensure!(
                read["thread"]["turns"]
                    .as_array()
                    .is_some_and(|turns| turns.len() == 1)
                    && read.to_string().contains(MARKER),
                "Native fixture history was not persisted"
            );
            api::rename_thread(id, &title()).send(client)?.wait()?;
            let active = api::start_thread(
                directory.to_str().unwrap(),
                &api::Profile::default(),
                api::Access::ReadOnly,
            )
            .send(client)?
            .wait()?;
            let active = active["thread"]["id"]
                .as_str()
                .context("Missing active fixture")?
                .to_owned();
            self.owned.push(active.clone());
            seed_turn(client, &active, &directory)?;
            println!("History host: second of two opted-in fixture prompts submitted.");
            self.phase = 11;
            return Ok(false);
        }
        if self.phase == 11 && self.active_done {
            api::rename_thread(&self.owned[1], &active_title())
                .send(client)?
                .wait()?;
            api::archive_thread(&self.owned[0]).send(client)?.wait()?;
            let prefix = std::env::var(TOKEN)?;
            let listed = api::search_threads(None, false, Some(&prefix))
                .send(client)?
                .wait()?;
            anyhow::ensure!(
                listed["data"]
                    .as_array()
                    .is_some_and(|items| items.iter().any(|item| item["id"] == self.owned[1])),
                "Native fixture is not discoverable by its unique title: {listed}"
            );
            script(
                app,
                owner,
                &format!(
                    "const field=root.querySelector('.native-history input:not([type=checkbox])');field.value={};field.dispatchEvent(new Event('input',{{bubbles:true}}));const button=[...root.querySelectorAll('button')].find(b=>b.textContent==='Browse Codex history…');if(!button||button.disabled)throw new Error('History browser unavailable');queueMicrotask(()=>button.click());",
                    json!(prefix)
                ),
            )?;
            self.phase = 2;
            return Ok(false);
        }
        let state = &sample["history"];
        if *state != self.last_history {
            println!("History host phase {}: {}", self.phase, state);
            self.last_history = state.clone();
        }
        anyhow::ensure!(
            state["error"].as_str().is_none_or(str::is_empty),
            "History UI error: {}",
            state["error"]
        );
        if state["loading"] == true {
            return Ok(false);
        }
        let titles = state["titles"].as_array();
        let selected_title = if self.active_choice {
            active_title()
        } else {
            title()
        };
        match self.phase {
            2 if state["open"] == true
                && titles.is_some_and(|items| items.len() == 1 && items[0] == active_title()) =>
            {
                search(app, owner, &format!("{} missing", title()), false)?;
                self.phase = 3;
            }
            3 if state["search"] == format!("{} missing", title())
                && titles.is_some_and(Vec::is_empty) =>
            {
                search(app, owner, &title(), false)?;
                self.phase = 4;
            }
            4 if state["search"] == title() && titles.is_some_and(Vec::is_empty) => {
                search(app, owner, &title(), true)?;
                self.phase = 5;
            }
            5 if state["archive"] == !self.active_choice
                && titles.is_some_and(|items| items.len() == 1 && items[0] == selected_title) =>
            {
                anyhow::ensure!(
                    app.app_server.conversations.saved().bindings.is_empty(),
                    "Browsing implicitly imported history"
                );
                if self.fork && !self.active_choice {
                    script(
                        app,
                        owner,
                        "const button=[...root.querySelectorAll('.native-history .results button')].find(b=>b.textContent==='Fork into this chat…');if(!button?.disabled)throw new Error('Archived fork must be unavailable');button.click();",
                    )?;
                    search(app, owner, &active_title(), false)?;
                    self.active_choice = true;
                    self.phase = 12;
                    return Ok(false);
                }
                let label = if self.fork {
                    "Fork into this chat…"
                } else {
                    "Link to this chat…"
                };
                script(
                    app,
                    owner,
                    &format!(
                        "const article=[...root.querySelectorAll('.native-history .results article')].find(a=>a.querySelector('h3')?.textContent==={});const button=[...article?.querySelectorAll('button')||[]].find(b=>b.textContent==={});if(!button||button.disabled)throw new Error('Import choice unavailable');button.click();",
                        json!(selected_title),
                        json!(label)
                    ),
                )?;
                self.phase = 6;
            }
            6 if !self.choice_cancelled
                && state["confirmation"]
                    .as_str()
                    .is_some_and(|text| text.contains(&selected_title)) =>
            {
                anyhow::ensure!(
                    app.app_server.conversations.saved().bindings.is_empty(),
                    "Selecting history bypassed confirmation"
                );
                script(
                    app,
                    owner,
                    "root.querySelector('.native-history .confirmation button').click();",
                )?;
                self.phase = 7;
            }
            7 if state["confirmation"].is_null() => {
                anyhow::ensure!(
                    app.app_server.conversations.saved().bindings.is_empty(),
                    "Cancel choice imported history"
                );
                self.phase = 5;
                self.choice_cancelled = true;
            }
            12 if state["archive"] == false
                && titles.is_some_and(|items| items.len() == 1 && items[0] == active_title()) =>
            {
                anyhow::ensure!(
                    app.app_server.conversations.saved().bindings.is_empty(),
                    "Disabled archived fork imported history"
                );
                self.phase = 5;
            }
            _ => {}
        }
        // On the second selection, confirm. The first selection/cancellation above
        // proves that neither browsing nor clicking a row mutates the local binding.
        if self.phase == 6
            && self.choice_cancelled
            && state["confirmation"]
                .as_str()
                .is_some_and(|text| text.contains(&selected_title))
        {
            let label = if self.fork {
                "Create fork"
            } else {
                "Link conversation"
            };
            script(
                app,
                owner,
                &format!(
                    "const button=root.querySelector('.native-history .confirmation button[aria-label={} ]');if(!button||button.disabled)throw new Error('Import confirmation unavailable');button.click();",
                    json!(label)
                ),
            )?;
            self.imported = false;
            self.phase = 8;
        }
        if self.phase == 8 && self.imported && state["open"] == false {
            let binding = app
                .app_server
                .conversations
                .binding(owner)
                .context("No imported native binding")?;
            anyhow::ensure!(
                app.app_server.conversations.saved().bindings.len() == 1
                    && (if self.fork {
                        binding.thread_id != self.owned[1] && !binding.archived
                    } else {
                        binding.thread_id == self.owned[0] && binding.archived
                    }),
                "Import identity/archive state incorrect"
            );
            let native = api::read_thread(&binding.thread_id).send(client)?.wait()?;
            anyhow::ensure!(
                native["thread"]["turns"]
                    .as_array()
                    .is_some_and(|turns| turns.len() == 1)
                    && native.to_string().contains(MARKER),
                "Imported native history changed"
            );
            anyhow::ensure!(
                app.project_chats
                    .iter()
                    .all(|chat| chat.messages.is_empty()),
                "Import copied history into another provider"
            );
            println!(
                "History host: active/archive filtering, empty search, cancelled selection and explicit import passed; draft/native history preserved."
            );
            return Ok(true);
        }
        Ok(false)
    }

    pub(super) fn cleanup(&mut self, app: &BrowserApp) -> anyhow::Result<()> {
        if let Some(client) = &app.app_server.client {
            // Child forks first: source deletion may be refused while referenced.
            for id in self.owned.iter().rev() {
                api::delete_thread(id).send(client)?.wait()?;
                println!("History host: deleted exact owned fixture {id}.");
            }
            client.shutdown();
        }
        Ok(())
    }
}
