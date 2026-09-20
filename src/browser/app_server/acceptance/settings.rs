//! No-inference settings acceptance through the compiled main/graph dialogs.
//! Native configuration and skills exist only in this owned temporary profile.
use super::*;
use delivery::{input, script};
mod drafts;
mod goal;
mod mcp;
mod oauth;
mod options;

pub(super) fn drafts_restoring() -> anyhow::Result<bool> {
    Ok(drafts::enabled() && drafts::stage()? > 1)
}

const ROOT: &str = "CENTRAL_AGENT_SETTINGS_TEST_ROOT";
const TOKEN: &str = "CENTRAL_AGENT_SETTINGS_TEST_TOKEN";
const SKILL: &str = "native-settings-fixture";
const PRIVATE: &str = "NATIVE_SETTINGS_PRIVATE_SENTINEL";
const DRAFT: &str = "Keep this unsent draft while inspecting native settings";

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
                .starts_with("central-native-settings-"))
            && Uuid::parse_str(&token).is_ok()
            && fs::read_to_string(root.join("owner-token"))? == token
            && fs::canonicalize(
                std::env::var_os("CODEX_HOME").context("Missing isolated native home")?
            )? == root.join("native"),
        "Invalid owned settings profile"
    );
    Ok(Some(root))
}

pub(super) fn coordinate(graph: bool, mcp: bool, oauth: bool) -> anyhow::Result<()> {
    let root = tempfile::Builder::new()
        .prefix("central-native-settings-")
        .tempdir()?;
    let token = Uuid::new_v4().to_string();
    fs::write(root.path().join("owner-token"), &token)?;
    let home = root.path().join("native");
    let skill = home.join("skills").join(SKILL);
    fs::create_dir_all(&skill)?;
    fs::write(
        skill.join("SKILL.md"),
        format!(
            "---\nname: {SKILL}\ndescription: Disposable native settings acceptance fixture.\n---\n{PRIVATE}\n"
        ),
    )?;
    fs::write(
        home.join("config.toml"),
        format!(
            "model_verbosity = \"low\"\n[mcp_servers.disabled_fixture]\nenabled = false\ncommand = \"never-start-this-fixture\"\n[mcp_servers.disabled_fixture.env]\nFIXTURE_VALUE = \"{PRIVATE}\"\n{}",
            if options::enabled() {
                format!(
                    "[mcp_servers.options_http_fixture]\nenabled = false\nurl = \"http://127.0.0.1:9/mcp\"\n[mcp_servers.options_http_fixture.http_headers]\nX-Private-Fixture = \"{PRIVATE}\"\n[mcp_servers.options_http_fixture.env_http_headers]\nX-Old-Fixture = \"CENTRAL_AGENT_OLD_OPTION_TEST_HEADER\"\n"
                )
            } else {
                String::new()
            }
        ),
    )?;
    let mut goal_fixture = if goal::enabled() {
        let fixture = reload_responses_fixture::Fixture::with_output(goal::output)?;
        fs::write(
            home.join("config.toml"),
            format!(
                "model = \"gpt-5.6-luna\"\nmodel_provider = \"goal_fixture\"\n[model_providers.goal_fixture]\nname = \"Owned goal fixture\"\nbase_url = \"http://{}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = false\nsupports_websockets = false\nrequest_max_retries = 0\nstream_max_retries = 0\n",
                fixture.address
            ),
        )?;
        Some(fixture)
    } else {
        None
    };
    if mcp {
        fs::write(
            root.path().join("inventory-mcp.mjs"),
            include_str!("../../../../crates/central-agent-codex-runtime/tests/inventory-mcp.mjs"),
        )?;
    }
    let mut child = std::process::Command::new(std::env::current_exe()?);
    child
        .arg("--check-native-settings")
        .env(ROOT, root.path())
        .env(TOKEN, token)
        .env("CODEX_HOME", home)
        .env_remove("CODEX_SQLITE_HOME")
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY");
    if graph {
        child.arg("--graph");
    }
    if goal::enabled() {
        child.arg("--native-goal");
    }
    if mcp {
        child.arg("--mcp-settings");
    }
    if options::enabled() {
        child.arg("--mcp-options");
    }
    if oauth {
        child.arg("--mcp-oauth");
    }
    if std::env::args_os().any(|arg| arg == "--conversation-oauth") {
        child.arg("--conversation-oauth");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        child.creation_flags(0x08000000);
    }
    let mut status = if drafts::enabled() {
        anyhow::ensure!(
            !mcp && !oauth,
            "Draft restart is a separate settings acceptance mode"
        );
        child
            .arg("--draft-persistence")
            .env("CENTRAL_AGENT_DRAFT_TEST_STAGE", "1")
            .status()?
    } else {
        child.status()?
    };
    if drafts::enabled() {
        for stage in [2, 3] {
            if !status.success() {
                break;
            }
            status = child
                .env("CENTRAL_AGENT_DRAFT_TEST_STAGE", stage.to_string())
                .status()?;
        }
    }
    let path = fs::canonicalize(root.path())?;
    if let Some(fixture) = &mut goal_fixture {
        fixture.finish().map_err(anyhow::Error::msg)?;
        anyhow::ensure!(
            !fixture
                .requests
                .lock()
                .map_err(|_| anyhow::anyhow!("Poisoned goal fixture"))?
                .is_empty(),
            "Goal never reached the local model endpoint"
        );
    }
    anyhow::ensure!(
        path.parent() == Some(fs::canonicalize(std::env::temp_dir())?.as_path())
            && path.file_name().is_some_and(|name| name
                .to_string_lossy()
                .starts_with("central-native-settings-")),
        "Unexpected cleanup scope"
    );
    let _ = root.keep();
    for attempt in 0..50 {
        match fs::remove_dir_all(&path) {
            Ok(()) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) if matches!(error.raw_os_error(), Some(32 | 33 | 145)) && attempt < 49 => {
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(error) => anyhow::bail!(
                "Owned settings fixture retained at {}: {error}",
                path.display()
            ),
        }
    }
    anyhow::ensure!(
        status.success(),
        "Settings acceptance child failed: {status}"
    );
    println!(
        "Settings coordinator: owned profile removed; no personal configuration or account-backed inference used."
    );
    Ok(())
}

pub(super) fn sample(owner: &str) -> String {
    if goal::enabled() {
        return goal::sample(owner);
    }
    if options::enabled() {
        return options::sample(owner);
    }
    if drafts::enabled() {
        return drafts::sample(owner);
    }
    if std::env::args_os().any(|arg| arg == "--mcp-oauth") {
        return oauth::sample(owner);
    }
    if std::env::args_os().any(|arg| arg == "--mcp-settings") {
        return mcp::sample(owner);
    }
    format!(
        r#"(()=>{{try{{{}const state={};const prefs=root?.querySelector('.native-preferences'),skills=root?.querySelector('.native-skills');
      const preference=[...prefs?.querySelectorAll('article')||[]].find(row=>[...row.querySelectorAll('details p')].some(p=>p.textContent==='model_verbosity'));
      const skill=[...skills?.querySelectorAll('article')||[]].find(row=>row.querySelector('h3')?.textContent==={});
      state.preferences={{open:prefs?.open,error:prefs?.querySelector('[role=alert]')?.textContent,
        value:preference?.querySelector('p span')?.textContent,editPresent:[...preference?.querySelectorAll('button')||[]].some(b=>b.textContent==='Edit…'),edit:[...preference?.querySelectorAll('button')||[]].some(b=>b.textContent==='Edit…'&&!b.disabled),readOnly:prefs?.textContent?.includes('These preferences are read-only.')||false,stale:prefs?.textContent?.includes('Last observed settings.')||false,loading:prefs?.textContent?.includes('Loading native configuration…')||false,writing:prefs?.textContent?.includes('Waiting for the shared configuration write.')||false,form:!!preference?.querySelector('form'),confirmation:!!prefs?.querySelector('[aria-label="Confirm shared Codex preference"]'),
        clearable:[...preference?.querySelectorAll('button')||[]].some(b=>b.textContent==='Clear saved value…'&&!b.disabled),
        refresh:[...prefs?.querySelectorAll('button')||[]].some(b=>b.textContent==='Refresh from Codex'&&!b.disabled)}};
      state.skills={{open:skills?.open,error:skills?.querySelector('[role=alert]')?.textContent,
        disabled:!!skill&&[...skill.querySelectorAll('button')].some(b=>b.textContent==='Enable…'),ready:!!skill,
        confirmation:!!skills?.querySelector('[aria-label="Confirm shared skill configuration"]'),refresh:skills?.querySelector('button[type=submit]')?.disabled===false}};
      state.selectedSkills=root?.querySelectorAll('.selected-skills .actions').length||0;
      state.privateExposed=(root===document?document.body:root)?.textContent?.includes({});return state;}}catch(error){{return {{errors:['Settings sample failed: '+String(error)]}};}}}})()"#,
        delivery::root_script(owner),
        delivery::sample(owner),
        json!(SKILL),
        json!(PRIVATE)
    )
}

fn click(app: &BrowserApp, owner: &str, selector: &str, label: &str) -> anyhow::Result<()> {
    script(
        app,
        owner,
        &format!(
            "const host=root.querySelector({})||root;const button=[...host.querySelectorAll('button')].find(b=>b.textContent.trim()==={});if(!button||button.disabled)throw new Error('Unavailable settings control: '+{});button.click();",
            json!(selector),
            json!(label),
            json!(label)
        ),
    )
}
fn preference(app: &BrowserApp, owner: &str, body: &str) -> anyhow::Result<()> {
    script(
        app,
        owner,
        &format!(
            "const row=[...root.querySelectorAll('.native-preferences article')].find(row=>[...row.querySelectorAll('details p')].some(p=>p.textContent==='model_verbosity'));if(!row)throw new Error('Verbosity preference missing');{body}"
        ),
    )
}
fn skill_button(app: &BrowserApp, owner: &str, label: &str) -> anyhow::Result<()> {
    script(
        app,
        owner,
        &format!(
            "const row=[...root.querySelectorAll('.native-skills article')].find(row=>row.querySelector('h3')?.textContent==={});const button=[...row?.querySelectorAll('button')||[]].find(b=>b.textContent==={});if(!button||button.disabled)throw new Error('Fixture skill control unavailable');button.click();",
            json!(SKILL),
            json!(label)
        ),
    )
}

#[derive(Default)]
pub(super) struct Settings {
    goal: Option<goal::Check>,
    options: Option<options::Check>,
    drafts: Option<drafts::Check>,
    mcp: Option<mcp::Check>,
    oauth: Option<oauth::Check>,
    phase: u8,
    graph_opened: bool,
    preference_saved: bool,
    skill_saved: bool,
    preference_writes: usize,
    skill_writes: usize,
    phase_one_diagnosed: bool,
    error: Option<String>,
}
impl Settings {
    pub(super) fn new(mcp: bool, oauth: bool) -> anyhow::Result<Self> {
        Ok(Self {
            goal: goal::enabled().then(goal::Check::default),
            options: options::enabled().then(options::Check::default),
            drafts: drafts::enabled().then(drafts::Check::default),
            mcp: mcp.then(mcp::Check::default),
            oauth: oauth.then(oauth::Check::new).transpose()?,
            ..Self::default()
        })
    }
    pub(super) fn intercept(
        &mut self,
        app: &BrowserApp,
        owner: &str,
        event: &BrowserEvent,
    ) -> anyhow::Result<bool> {
        self.oauth
            .as_mut()
            .map_or(Ok(false), |oauth| oauth.intercept(app, owner, event))
    }
    pub(super) fn observe(&mut self, owner: &str, event: &BrowserEvent) {
        if let Some(goal) = &mut self.goal {
            goal.observe(owner, event);
            return;
        }
        if let Some(options) = &mut self.options {
            options.observe(event);
        }
        if let Some(mcp) = &mut self.mcp {
            mcp.observe(event);
        }
        if let Some(oauth) = &mut self.oauth {
            oauth.observe(event);
        }
        match event {
            BrowserEvent::AppServer(Event::PreferencesReply {
                owner: destination,
                result,
                ..
            }) if destination == owner => match result {
                Ok(value) if value.get("filePath").is_some() => {
                    self.preference_saved = true;
                    self.preference_writes += 1;
                }
                Err(error) => self.error = Some(format!("Native preferences failed: {error}")),
                _ => {}
            },
            BrowserEvent::AppServer(Event::SkillsReply {
                owner: destination,
                result,
                ..
            }) if destination == owner => match result {
                Ok(value) if value.get("effectiveEnabled").is_some() => {
                    self.skill_writes += 1;
                    if value["effectiveEnabled"] == (self.phase >= 21) {
                        self.skill_saved = true;
                    } else {
                        self.error = Some(
                            "Native skill enablement did not match the confirmed action".into(),
                        );
                    }
                }
                Err(error) => self.error = Some(format!("Native skills failed: {error}")),
                _ => {}
            },
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::Notification { method, params },
                ..
            }) if (method == "thread/started" || method == "turn/started")
                && (method != "thread/started"
                    || !self
                        .oauth
                        .as_ref()
                        .is_some_and(|oauth| oauth.owned_thread_start(params))) =>
            {
                self.error =
                    Some("Settings unexpectedly opened a native thread or model turn".into());
            }
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::ServerRequest { .. },
                ..
            }) => self.error = Some("Settings unexpectedly requested execution authority".into()),
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
            anyhow::bail!("{error}")
        }
        if !app.app_server.view.connected || state.is_null() {
            return Ok(false);
        }
        anyhow::ensure!(
            app.app_server.view.account.is_none(),
            "Settings test must remain unauthenticated"
        );
        if let Some(goal) = &mut self.goal {
            return goal.tick(app, owner, state, graph);
        }
        if let Some(mcp) = &mut self.mcp {
            return mcp.tick(app, owner, state);
        }
        if let Some(options) = &mut self.options {
            return options.tick(app, owner, state);
        }
        if let Some(drafts) = &mut self.drafts {
            return drafts.tick(app, owner, state, graph);
        }
        if let Some(oauth) = &mut self.oauth {
            return oauth.tick(app, owner, state, graph);
        }
        if let Some(graph) = graph {
            if !self.graph_opened {
                graph.open_cards(app)?;
                self.graph_opened = true;
                return Ok(false);
            }
            if state["ready"] != true {
                return Ok(false);
            }
        }
        anyhow::ensure!(
            state["privateExposed"] != true,
            "Private configuration or skill instructions entered the UI"
        );
        for key in ["preferences", "skills"] {
            anyhow::ensure!(
                state[key]["error"].as_str().is_none_or(str::is_empty),
                "Settings UI failed: {}",
                state[key]["error"]
            );
        }
        if self.phase > 0 && state["draft"] != DRAFT {
            return Ok(false);
        }
        let previous = self.phase;
        if self.phase == 1
            && state["preferences"]["open"] == true
            && state["preferences"]["value"] == "low"
            && state["preferences"]["edit"] != true
            && !self.phase_one_diagnosed
        {
            self.phase_one_diagnosed = true;
            println!(
                "Settings host edit diagnostic: native_busy={}, conversation_busy={}, view={}",
                app.app_server.busy(owner),
                app.conversation_busy(owner),
                app.app_server_conversation_view(owner)
            );
        }
        match self.phase {
            0 => {
                input(app, owner, DRAFT, false)?;
                click(app, owner, ".native-conversation", "Codex defaults…")?;
                self.phase = 1;
            }
            1 if state["preferences"]["open"] == true
                && state["preferences"]["value"] == "low"
                && state["preferences"]["edit"] == true
                && state["preferences"]["refresh"] == true =>
            {
                preference(
                    app,
                    owner,
                    "const edit=[...row.querySelectorAll('button')].find(b=>b.textContent==='Edit…'&&!b.disabled);if(!edit)throw new Error('Editable preference action missing');edit.click();",
                )?;
                self.phase = 2;
            }
            2 if state["preferences"]["form"] == true => {
                preference(
                    app,
                    owner,
                    "const field=row.querySelector('form select');field.value='high';field.dispatchEvent(new Event('change',{bubbles:true}));queueMicrotask(()=>row.querySelector('form').requestSubmit());",
                )?;
                self.phase = 3;
            }
            3 if state["preferences"]["confirmation"] == true => {
                anyhow::ensure!(
                    !self.preference_saved,
                    "Preference changed before confirmation"
                );
                click(
                    app,
                    owner,
                    ".native-preferences [aria-label='Confirm shared Codex preference']",
                    "Back",
                )?;
                self.phase = 4;
            }
            4 if state["preferences"]["form"] == true
                && state["preferences"]["confirmation"] == false =>
            {
                anyhow::ensure!(
                    !self.preference_saved,
                    "Cancelling changed native configuration"
                );
                preference(app, owner, "row.querySelector('form').requestSubmit();")?;
                self.phase = 5;
            }
            5 if state["preferences"]["confirmation"] == true => {
                click(app, owner, ".native-preferences", "Save default")?;
                self.phase = 6;
            }
            6 if self.preference_saved && state["preferences"]["refresh"] == true => {
                let cwd = graph
                    .and_then(|g| g.directory(owner))
                    .or_else(|| app.workspace.root())
                    .unwrap();
                let config = api::config_read(cwd.to_str().unwrap())
                    .send(app.app_server.client.as_ref().unwrap())?
                    .wait()?;
                anyhow::ensure!(
                    config["config"]["model_verbosity"] == "high",
                    "Saved preference was not persisted by native Codex"
                );
                click(app, owner, ".native-preferences", "Refresh from Codex")?;
                self.phase = 7;
            }
            7 if state["preferences"]["value"] == "high"
                && state["preferences"]["refresh"] == true =>
            {
                click(app, owner, ".native-preferences header", "Close")?;
                self.phase = 8;
            }
            8 if state["preferences"]["open"] == false => {
                click(app, owner, ".native-conversation", "Codex skills…")?;
                self.phase = 9;
            }
            9 if state["skills"]["open"] == true
                && state["skills"]["ready"] == true
                && state["skills"]["refresh"] == true =>
            {
                skill_button(app, owner, "Disable…")?;
                self.phase = 10;
            }
            10 if state["skills"]["confirmation"] == true => {
                anyhow::ensure!(!self.skill_saved, "Skill changed before confirmation");
                click(
                    app,
                    owner,
                    ".native-skills [aria-label='Confirm shared skill configuration']",
                    "Cancel",
                )?;
                self.phase = 11;
            }
            11 if state["skills"]["confirmation"] == false => {
                anyhow::ensure!(!self.skill_saved, "Cancelling changed the skill");
                skill_button(app, owner, "Disable…")?;
                self.phase = 12;
            }
            12 if state["skills"]["confirmation"] == true => {
                click(app, owner, ".native-skills", "Confirm disable")?;
                self.phase = 13;
            }
            13 if self.skill_saved && state["skills"]["refresh"] == true => {
                click(app, owner, ".native-skills", "Refresh from Codex")?;
                self.phase = 14;
            }
            14 if state["skills"]["disabled"] == true && state["skills"]["refresh"] == true => {
                let cwd = graph
                    .and_then(|g| g.directory(owner))
                    .or_else(|| app.workspace.root())
                    .unwrap();
                let skills = api::skills_list(cwd.to_str().unwrap())
                    .send(app.app_server.client.as_ref().unwrap())?
                    .wait()?;
                let row = skills["data"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|group| group["skills"].as_array().into_iter().flatten())
                    .find(|skill| skill["name"] == SKILL)
                    .context("Native fixture skill disappeared")?;
                anyhow::ensure!(
                    row["enabled"] == false,
                    "Native skill readback was not disabled"
                );
                click(app, owner, ".native-skills header", "Close")?;
                self.phase = 15;
            }
            15 if state["skills"]["open"] == false => {
                click(app, owner, ".native-conversation", "Codex defaults…")?;
                self.phase = 16;
            }
            16 if state["preferences"]["open"] == true
                && state["preferences"]["clearable"] == true =>
            {
                preference(
                    app,
                    owner,
                    "[...row.querySelectorAll('button')].find(b=>b.textContent==='Clear saved value…').click();",
                )?;
                self.phase = 17;
            }
            17 if state["preferences"]["confirmation"] == true => {
                anyhow::ensure!(
                    self.preference_writes == 1,
                    "Clear wrote before confirmation"
                );
                click(app, owner, ".native-preferences", "Back")?;
                self.phase = 18;
            }
            18 if state["preferences"]["confirmation"] == false => {
                anyhow::ensure!(
                    self.preference_writes == 1,
                    "Cancelling clear changed configuration"
                );
                preference(
                    app,
                    owner,
                    "[...row.querySelectorAll('button')].find(b=>b.textContent==='Clear saved value…').click();",
                )?;
                self.phase = 19;
            }
            19 if state["preferences"]["confirmation"] == true => {
                click(app, owner, ".native-preferences", "Clear saved value")?;
                self.phase = 20;
            }
            20 if self.preference_writes == 2 && state["preferences"]["refresh"] == true => {
                let cwd = graph
                    .and_then(|g| g.directory(owner))
                    .or_else(|| app.workspace.root())
                    .unwrap();
                let config = api::config_read(cwd.to_str().unwrap())
                    .send(app.app_server.client.as_ref().unwrap())?
                    .wait()?;
                let snapshot = central_agent_codex_runtime::configuration::Snapshot::read(&config)
                    .map_err(anyhow::Error::msg)?;
                anyhow::ensure!(
                    snapshot
                        .preferences
                        .iter()
                        .any(|p| p.key == "model_verbosity" && p.user_value.is_none()),
                    "Native clear retained the saved override"
                );
                anyhow::ensure!(
                    config["config"]["mcp_servers"]["disabled_fixture"]["env"]["FIXTURE_VALUE"]
                        == PRIVATE
                        && config["config"]["mcp_servers"]["disabled_fixture"]["enabled"] == false,
                    "Preference writes changed unrelated private configuration"
                );
                click(app, owner, ".native-preferences header", "Close")?;
                self.phase = 21;
            }
            21 if state["preferences"]["open"] == false => {
                self.skill_saved = false;
                click(app, owner, ".native-conversation", "Codex skills…")?;
                self.phase = 22;
            }
            22 if state["skills"]["open"] == true
                && state["skills"]["disabled"] == true
                && state["skills"]["refresh"] == true =>
            {
                skill_button(app, owner, "Enable…")?;
                self.phase = 23;
            }
            23 if state["skills"]["confirmation"] == true => {
                anyhow::ensure!(self.skill_writes == 1, "Enable wrote before confirmation");
                click(app, owner, ".native-skills", "Confirm enable")?;
                self.phase = 24;
            }
            24 if self.skill_saved && state["skills"]["refresh"] == true => {
                click(app, owner, ".native-skills", "Refresh from Codex")?;
                self.phase = 25;
            }
            25 if state["skills"]["ready"] == true
                && state["skills"]["disabled"] == false
                && state["skills"]["refresh"] == true =>
            {
                skill_button(app, owner, "Use with next prompt")?;
                self.phase = 26;
            }
            26 if state["selectedSkills"] == 1 => {
                click(app, owner, ".native-skills header", "Close")?;
                self.phase = 27;
            }
            27 if state["skills"]["open"] == false && state["selectedSkills"] == 1 => {
                click(app, owner, ".selected-skills", "Remove")?;
                self.phase = 28;
            }
            28 if state["selectedSkills"] == 0 => {
                anyhow::ensure!(
                    self.preference_writes == 2 && self.skill_writes == 2,
                    "Settings produced unexpected native writes"
                );
                anyhow::ensure!(
                    app.app_server.conversations.saved().bindings.is_empty()
                        && app
                            .project_chats
                            .iter()
                            .all(|chat| chat.messages.is_empty()),
                    "Settings created conversation history"
                );
                println!(
                    "Settings host: preference save/clear, skill disable/enable, selection/removal, confirmations, cancellation, refresh, privacy and unchanged draft verified; no inference."
                );
                return Ok(true);
            }
            _ => {}
        }
        if previous != self.phase {
            println!("Settings host phase {}", self.phase);
        }
        Ok(false)
    }
    pub(super) fn cleanup(&self, app: &BrowserApp) {
        if let Some(client) = &app.app_server.client {
            client.shutdown();
        }
    }
}
