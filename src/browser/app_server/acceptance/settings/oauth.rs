//! Actual Settings OAuth intent and native completion. The OS browser launch
//! boundary is replaced only in this opt-in harness by owned loopback consent.
use super::*;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Stdio};

const NAME: &str = "oauth_fixture";
struct Fixture(Child);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
pub(super) struct Check {
    fixture: Fixture,
    lines: mpsc::Receiver<String>,
    origin: String,
    trace: PathBuf,
    phase: u8,
    starts: usize,
    opens: usize,
    authorized: bool,
    completed: bool,
    previous_epoch: u64,
    scoped: bool,
    directory: Option<PathBuf>,
    thread: Option<String>,
    owner: Option<String>,
    graph_opened: bool,
    scoped_ready: bool,
    error: Option<String>,
}
pub(super) fn sample(owner: &str) -> String {
    format!(
        r#"(()=>{{try{{{}const state={};
      const mcp=root?.querySelector({});
      const row=[...mcp?.querySelectorAll('.server')||[]].find(row=>row.querySelector('summary')?.textContent==={});
      const button=(label)=>[...mcp?.querySelectorAll('button')||[]].find(b=>b.textContent===label&&!b.disabled);
      state.oauth={{phase:window.__oauthHostPhase||0,ready:!!mcp,row:row?.textContent||'',
        refresh:!!button('Refresh servers'),authorize:!!row&&[...row.querySelectorAll('button')].some(b=>b.textContent==='Authorize OAuth…'&&!b.disabled),
        login:!!mcp?.querySelector('.login'),open:!!button('Open authorization page'),
        instructions:mcp?.querySelector('.login')?.textContent||'',error:mcp?.querySelector('[role=alert]')?.textContent||'',
        privateExposed:['fixture-access','fixture-code','code_challenge=','state='].some(s=>document.body.textContent.includes(s))}};return state;
      }}catch(error){{return {{errors:['OAuth sample failed: '+String(error)]}};}}}})()"#,
        delivery::root_script(owner),
        delivery::sample(owner),
        json!(selector()),
        json!(NAME)
    )
}
fn selector() -> &'static str {
    if std::env::args_os().any(|arg| arg == "--conversation-oauth") {
        ".native-mcp:not([data-central-agent-svelte='app-server-account'] .native-mcp)"
    } else {
        "[data-central-agent-svelte='app-server-account'] .native-mcp"
    }
}
impl Check {
    pub(super) fn new() -> anyhow::Result<Self> {
        let root = context()?.context("Missing owned OAuth profile")?;
        let script = root.join("oauth-mcp.mjs");
        fs::write(
            &script,
            include_str!("../../../../../crates/central-agent-codex-runtime/tests/oauth-mcp.mjs"),
        )?;
        let trace = root.join("oauth-methods.txt");
        let mut command = std::process::Command::new("node");
        let script_text = script.to_string_lossy();
        let trace_text = trace.to_string_lossy();
        command
            .arg(script_text.strip_prefix(r"\\?\").unwrap_or(&script_text))
            .arg(trace_text.strip_prefix(r"\\?\").unwrap_or(&trace_text))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut fixture = Fixture(command.spawn()?);
        let output = fixture.0.stdout.take().unwrap();
        let (send, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                match line {
                    Ok(line) => {
                        if send.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        let origin = lines
            .recv_timeout(Duration::from_secs(15))
            .context("Owned OAuth fixture did not start")?;
        let parsed = url::Url::parse(&origin)?;
        anyhow::ensure!(
            parsed.scheme() == "http"
                && parsed.host_str() == Some("127.0.0.1")
                && parsed.port().is_some()
                && parsed.path() == "/"
                && parsed.query().is_none(),
            "Unexpected OAuth fixture origin"
        );
        let config = root.join("native/config.toml");
        let old = fs::read_to_string(&config)?;
        // Synthetic credentials stay inside this newly created profile, never keyring.
        let mut trust = String::new();
        for path in [
            root.join("workspace"),
            root.join("graph-projects/A"),
            root.join("graph-projects/B"),
        ] {
            let text = path.to_string_lossy();
            trust.push_str(&format!(
                "[projects.{}]\ntrust_level = \"trusted\"\n",
                json!(text.strip_prefix(r"\\?\").unwrap_or(&text))
            ));
        }
        fs::write(
            config,
            format!(
                "mcp_oauth_credentials_store = \"file\"\n{old}\n[mcp_servers.{NAME}]\nurl = \"{origin}/mcp\"\n{trust}"
            ),
        )?;
        Ok(Self {
            fixture,
            lines,
            origin,
            trace,
            phase: 0,
            starts: 0,
            opens: 0,
            authorized: false,
            completed: false,
            previous_epoch: 0,
            scoped: std::env::args_os().any(|arg| arg == "--conversation-oauth"),
            directory: None,
            thread: None,
            owner: None,
            graph_opened: false,
            scoped_ready: false,
            error: None,
        })
    }
    pub(super) fn owned_thread_start(&self, params: &Value) -> bool {
        self.scoped
            && self.directory.as_ref().is_some_and(|path| {
                params["thread"]["ephemeral"] == true
                    && params["thread"]["cwd"]
                        .as_str()
                        .and_then(|cwd| fs::canonicalize(cwd).ok())
                        == fs::canonicalize(path).ok()
            })
    }
    pub(super) fn observe(&mut self, event: &BrowserEvent) {
        match event {
            BrowserEvent::AppServer(Event::McpReply { result, .. }) => match result {
                Ok(value) if value.get("authorizationUrl").is_some() => self.starts += 1,
                Err(error) => self.error = Some(format!("Native OAuth request failed: {error}")),
                _ => {}
            },
            BrowserEvent::AppServer(Event::ThreadMcpReply {
                owner,
                thread: thread_id,
                result,
                ..
            }) => {
                if self.owner.as_deref() != Some(owner) || self.thread.as_deref() != Some(thread_id)
                {
                    self.error = Some("OAuth reply reached a different conversation".into());
                } else {
                    match result {
                        Ok(value) if value.get("authorizationUrl").is_some() => self.starts += 1,
                        Ok(value) if value.get("data").is_some() => {
                            let statuses: Vec<_> = value["data"].as_array().into_iter().flatten()
                                .map(|s| json!({"name":s["name"],"auth":s["authStatus"],"runtime":s["runtimeStatus"]})).collect();
                            println!(
                                "Scoped OAuth inventory phase {}: {}",
                                self.phase,
                                json!(statuses)
                            );
                        }
                        Err(error) => {
                            self.error = Some(format!("Scoped OAuth request failed: {error}"))
                        }
                        _ => {}
                    }
                }
            }
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::Notification { method, params },
                ..
            }) if method == "mcpServer/startupStatus/updated" && params["name"] == NAME => {
                if self.scoped && params["threadId"] == json!(self.thread) {
                    self.scoped_ready = params["status"] == "ready";
                }
                println!(
                    "OAuth startup phase {}: status={}, ownedThread={}",
                    self.phase,
                    params["status"],
                    params["threadId"] == json!(self.thread)
                );
            }
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::Notification { method, params },
                ..
            }) if method == "thread/started" && self.owned_thread_start(params) => {
                self.thread = params["thread"]["id"].as_str().map(str::to_owned);
            }
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::Notification { method, params },
                ..
            }) if method == "mcpServer/oauthLogin/completed" => {
                if params["name"] == NAME
                    && params["threadId"] == json!(self.thread)
                    && params["success"] == true
                {
                    self.completed = true;
                } else {
                    self.error =
                        Some("Wrong native OAuth completion scope or failed authorization".into());
                }
            }
            _ => {}
        }
    }
    pub(super) fn intercept(
        &mut self,
        app: &BrowserApp,
        owner: &str,
        event: &BrowserEvent,
    ) -> anyhow::Result<bool> {
        let message = match event {
            BrowserEvent::ScopedAgentPanel {
                owner: destination,
                message,
            } => {
                if destination != owner {
                    return Ok(false);
                }
                message
            }
            BrowserEvent::AgentPanel(message) if owner.starts_with("graph:") => message,
            _ => return Ok(false),
        };
        let (attempt_id, thread) = match message {
            AgentPanelMessage::AppServerMcp {
                action: McpAction::OpenLogin { attempt_id },
            } if !self.scoped => (attempt_id, None),
            AgentPanelMessage::AppServerConversation {
                owner: destination,
                action:
                    ConversationAction::McpControl {
                        expected_thread_id,
                        action: McpAction::OpenLogin { attempt_id },
                    },
            } if self.scoped => {
                anyhow::ensure!(
                    destination == owner
                        && self.thread.as_deref() == Some(expected_thread_id)
                        && app
                            .app_server
                            .conversations
                            .binding(owner)
                            .is_some_and(|b| b.thread_id == *expected_thread_id
                                && !b.deleted
                                && !b.archived)
                        && app.app_server.conversations.is_thread_loaded(owner),
                    "Foreign or unloaded OAuth thread intent"
                );
                (attempt_id, Some(expected_thread_id))
            }
            _ => return Ok(false),
        };
        anyhow::ensure!(
            self.phase == 3 && self.opens == 0 && app.app_server.view.connected,
            "Unexpected OAuth browser intent"
        );
        // The same production attempt/URL validator is used before substituting
        // only the external browser side effect with consent at our own fixture.
        let url = if let Some(thread) = thread {
            app.app_server
                .thread_mcp
                .login_url(owner, thread, attempt_id)
        } else {
            app.app_server.mcp.login_url(attempt_id)
        }
        .map_err(anyhow::Error::msg)?;
        anyhow::ensure!(
            url.origin().ascii_serialization() == self.origin && url.path() == "/authorize",
            "Foreign OAuth URL rejected by test fixture"
        );
        writeln!(
            self.fixture.0.stdin.as_mut().unwrap(),
            "{}",
            json!({"authorize":url.as_str()})
        )?;
        self.opens += 1;
        Ok(true)
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
        while let Ok(line) = self.lines.try_recv() {
            anyhow::ensure!(
                serde_json::from_str::<Value>(&line)?["authorized"] == true,
                "Owned OAuth callback failed"
            );
            self.authorized = true;
        }
        if self.scoped {
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
            if self.directory.is_none() {
                let directory = graph
                    .and_then(|g| g.directory(owner))
                    .or_else(|| app.workspace.root())
                    .context("Missing scoped OAuth directory")?
                    .to_path_buf();
                self.directory = Some(directory.clone());
                self.owner = Some(owner.into());
                let mut request = app
                    .app_server
                    .conversations
                    .open(
                        owner,
                        &directory,
                        &api::Profile::default(),
                        api::Access::ReadOnly,
                    )
                    .map_err(anyhow::Error::msg)?;
                request.call.params["ephemeral"] = json!(true);
                app.dispatch_app_server_thread(request);
                if let Some(graph) = graph {
                    for sibling in graph.owners().into_iter().filter(|s| s != owner) {
                        input(app, &sibling, "Keep the sibling OAuth draft", false)?;
                    }
                }
                println!("OAuth host: requested one owned ephemeral native thread, no model turn");
                return Ok(false);
            }
            if !app.app_server.conversations.is_thread_loaded(owner) {
                return Ok(false);
            }
            let binding = app
                .app_server
                .conversations
                .binding(owner)
                .context("Missing owned OAuth binding")?;
            anyhow::ensure!(
                self.thread.as_deref() == Some(binding.thread_id.as_str())
                    && app
                        .app_server
                        .conversations
                        .mirror
                        .thread(&binding.thread_id)
                        .is_some_and(|t| t.turns.is_empty()),
                "OAuth changed native history"
            );
            if let Some(graph) = graph {
                for sibling in graph.owners().into_iter().filter(|s| s != owner) {
                    anyhow::ensure!(
                        app.app_server.conversations.binding(&sibling).is_none(),
                        "OAuth created a sibling binding"
                    );
                }
                anyhow::ensure!(
                    app.app_server
                        .conversations
                        .binding(&app.conversation_key(None))
                        .is_none(),
                    "Graph OAuth changed main chat"
                );
                if let Some(siblings) = state["siblings"].as_array() {
                    anyhow::ensure!(
                        siblings
                            .iter()
                            .all(|s| s["draft"] == "Keep the sibling OAuth draft"),
                        "OAuth changed a sibling draft"
                    );
                }
            }
        }
        let o = &state["oauth"];
        anyhow::ensure!(
            o["privateExposed"] != true,
            "OAuth credentials/query values entered the DOM"
        );
        anyhow::ensure!(
            o["error"].as_str().is_none_or(str::is_empty),
            "OAuth UI failed: {}",
            o["error"]
        );
        if o["phase"].as_u64() != Some(u64::from(self.phase))
            || self.phase > 0 && state["draft"] != DRAFT
        {
            return Ok(false);
        }
        let previous = self.phase;
        match self.phase {
            0 if o["ready"] == true => {
                input(app, owner, DRAFT, false)?;
                if !self.scoped {
                    app.open_settings();
                    script(
                        app,
                        owner,
                        "root.querySelector('[data-settings-target=\"settings-ai\"]').click();",
                    )?;
                }
                script(
                    app,
                    owner,
                    &format!("root.querySelector({}).open=true;", json!(selector())),
                )?;
                click(app, owner, selector(), "Refresh servers")?;
                self.phase = 1;
            }
            1 if o["authorize"] == true && o["refresh"] == true => {
                anyhow::ensure!(
                    self.starts == 0 && self.opens == 0 && !self.completed,
                    "OAuth started without intent"
                );
                script(
                    app,
                    owner,
                    &format!(
                        "const host=root.querySelector({});const row=[...host.querySelectorAll('.server')].find(row=>row.querySelector('summary')?.textContent==={});row.open=true;const button=[...row.querySelectorAll('button')].find(b=>b.textContent==='Authorize OAuth…');button.click();",
                        json!(selector()),
                        json!(NAME)
                    ),
                )?;
                self.phase = 2;
            }
            2 if o["login"] == true && o["open"] == true => {
                anyhow::ensure!(
                    self.starts == 1
                        && self.opens == 0
                        && !self.completed
                        && o["instructions"]
                            .as_str()
                            .is_some_and(|s| s.contains(&self.origin)),
                    "Incorrect native OAuth presentation"
                );
                let trace = fs::read_to_string(&self.trace)?;
                anyhow::ensure!(
                    !trace
                        .lines()
                        .any(|s| s == "GET /authorize" || s == "POST /token"),
                    "OAuth consent happened before opening the URL"
                );
                self.phase = 3;
                click(
                    app,
                    owner,
                    &format!("{} .login", selector()),
                    "Open authorization page",
                )?;
            }
            3 if self.completed
                && self.authorized
                // OAuth completion precedes the loaded server's restart. The
                // production client correctly invalidates reads crossing that
                // restart; exercise Refresh after this fixture is ready, not
                // a timing-dependent stale snapshot or an automatic retry.
                && (!self.scoped || self.scoped_ready)
                && o["login"] == false
                && o["refresh"] == true =>
            {
                anyhow::ensure!(
                    self.starts == 1 && self.opens == 1,
                    "OAuth was automatically replayed"
                );
                click(app, owner, selector(), "Refresh servers")?;
                self.phase = 4;
            }
            4 | 6
                if o["refresh"] == true
                    && o["row"]
                        .as_str()
                        .is_some_and(|s| s.contains("OAuth authorized")) =>
            {
                let inventory = self
                    .thread
                    .as_deref()
                    .map_or_else(
                        || api::mcp_status(None),
                        |thread| api::thread_mcp_status(thread, None),
                    )
                    .send(app.app_server.client.as_ref().unwrap())?
                    .wait()?;
                anyhow::ensure!(
                    inventory["data"].as_array().is_some_and(|servers| servers
                        .iter()
                        .any(|s| s["name"] == NAME && s["authStatus"] == "oAuth")),
                    "Native auth inventory did not confirm OAuth"
                );
                let trace = fs::read_to_string(&self.trace)?;
                anyhow::ensure!(
                    trace.lines().filter(|s| *s == "POST /token").count() == 1
                        && self.starts == 1
                        && self.opens == 1
                        && !trace
                            .lines()
                            .any(|s| s == "MCP tools/call" || s == "MCP resources/read"),
                    "OAuth did not stay authentication/inventory only"
                );
                anyhow::ensure!(
                    app.app_server.conversations.saved().bindings.len() == usize::from(self.scoped)
                        && app.project_chats.iter().all(|c| c.messages.is_empty()),
                    "OAuth created conversation history"
                );
                if self.scoped {
                    anyhow::ensure!(
                        app.app_server.mcp.view.login.is_none(),
                        "Scoped OAuth leaked a global login"
                    );
                    println!(
                        "Conversation OAuth host: exact native owner/thread, explicit URL intent, PKCE exchange, scoped completion/auth readback, unchanged target/sibling drafts verified; no model turns/tool calls. OS browser launch substituted by owned loopback consent."
                    );
                    return Ok(true);
                } else if self.phase == 4 {
                    self.previous_epoch = app.app_server.epoch;
                    click(
                        app,
                        owner,
                        "[data-central-agent-svelte='app-server-account']",
                        "Reconnect",
                    )?;
                    self.phase = 5;
                } else {
                    println!(
                        "OAuth Settings host: explicit service/login URL intent, native completion, PKCE fixture exchange, auth readback after native reconnect and draft/privacy verified. OS browser launching substituted by owned loopback consent; zero inference/tool calls."
                    );
                    return Ok(true);
                }
            }
            5 if app.app_server.epoch > self.previous_epoch && o["refresh"] == true => {
                click(app, owner, selector(), "Refresh servers")?;
                self.phase = 6;
            }
            _ => {}
        }
        if previous != self.phase {
            script(
                app,
                owner,
                &format!("window.__oauthHostPhase={};", self.phase),
            )?;
            println!("OAuth Settings host phase {}", self.phase);
        }
        Ok(false)
    }
}
