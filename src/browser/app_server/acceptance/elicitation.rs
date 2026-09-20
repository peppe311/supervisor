//! No-inference native MCP callback acceptance through actual desktop request cards.
use super::*;
use central_agent_codex_runtime::{transport::Ticket, wire::ServerRequestKey};
mod in_turn;
use super::reload_responses_fixture as responses;

pub(super) fn in_turn_enabled() -> bool {
    std::env::args_os().any(|arg| arg == "--in-turn")
}

const ROOT: &str = "CENTRAL_AGENT_ELICITATION_TEST_ROOT";
const TOKEN: &str = "CENTRAL_AGENT_ELICITATION_TEST_TOKEN";
const DRAFT: &str = "Unsent draft survives native MCP questions";
const CASES: [(&str, &str); 6] = [
    ("form", "accept"),
    ("form", "decline"),
    ("form", "cancel"),
    ("url", "accept"),
    ("url", "decline"),
    ("url", "cancel"),
];

pub(super) fn context() -> anyhow::Result<Option<PathBuf>> {
    let Some(root) = std::env::var_os(ROOT) else {
        return Ok(None);
    };
    let token = std::env::var(TOKEN).context("Missing fixture ownership token")?;
    validate_context(
        Path::new(&root),
        &token,
        Path::new(&std::env::var_os("CODEX_HOME").context("Missing isolated native home")?),
    )
    .map(Some)
}

fn validate_context(root: &Path, token: &str, home: &Path) -> anyhow::Result<PathBuf> {
    let root = fs::canonicalize(root)?;
    anyhow::ensure!(
        root.parent() == Some(fs::canonicalize(std::env::temp_dir())?.as_path())
            && root.file_name().is_some_and(|name| name
                .to_string_lossy()
                .starts_with("central-native-elicitation-"))
            && Uuid::parse_str(token).is_ok()
            && fs::read_to_string(root.join("owner-token"))? == token,
        "Invalid owned elicitation fixture profile"
    );
    anyhow::ensure!(
        fs::canonicalize(home)? == root.join("native"),
        "Native fixture must not use personal Codex configuration"
    );
    Ok(root)
}

pub(super) fn coordinate(graph: bool, command_policy: bool) -> anyhow::Result<()> {
    let mut model = if command_policy || in_turn_enabled() {
        Some(responses::Fixture::with_output(if command_policy {
            command_policy::model_output
        } else {
            responses::mcp_output
        })?)
    } else {
        None
    };
    let root = tempfile::Builder::new()
        .prefix("central-native-elicitation-")
        .tempdir()?;
    let token = Uuid::new_v4().to_string();
    fs::write(root.path().join("owner-token"), &token)?;
    let home = root.path().join("native");
    fs::create_dir(&home)?;
    let fixture = root.path().join("elicitation-mcp.mjs");
    if !command_policy {
        fs::write(
            &fixture,
            include_str!(
                "../../../../crates/central-agent-codex-runtime/tests/elicitation-mcp.mjs"
            ),
        )?;
    }
    let mut config = if command_policy {
        String::new()
    } else {
        format!(
            "[mcp_servers.elicitation_fixture]\ncommand = \"node\"\nargs = {}\n",
            json!([fixture])
        )
    };
    if let Some(model) = &model {
        config = format!(
            "model = \"gpt-5.6-luna\"\nmodel_provider = \"mcp_fixture\"\n[model_providers.mcp_fixture]\nname = \"Owned host MCP fixture\"\nbase_url = \"http://{}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = false\nsupports_websockets = false\nrequest_max_retries = 0\nstream_max_retries = 0\n{config}",
            model.address
        );
    }
    for path in [
        root.path().join("workspace"),
        root.path().join("graph-projects/A"),
        root.path().join("graph-projects/B"),
    ] {
        config.push_str(&format!(
            "[projects.{}]\ntrust_level = \"trusted\"\n",
            json!(path)
        ));
    }
    fs::write(home.join("config.toml"), config)?;
    let mut child = std::process::Command::new(std::env::current_exe()?);
    child
        .arg(if command_policy {
            "--check-native-command-policy"
        } else {
            "--check-native-mcp-requests"
        })
        .env(ROOT, root.path())
        .env(TOKEN, token)
        .env("CODEX_HOME", &home)
        .env_remove("CODEX_SQLITE_HOME")
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY");
    if graph {
        child.arg("--graph");
    }
    if in_turn_enabled() {
        child.arg("--in-turn");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        child.creation_flags(0x08000000);
    }
    let status = child.status()?;
    let model_result = if let Some(model) = &mut model {
        model.finish().map_err(anyhow::Error::msg).and_then(|()| {
            anyhow::ensure!(
                model
                    .requests
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Poisoned model fixture"))?
                    .len()
                    == if command_policy { 4 } else { 12 },
                "Unexpected local model request count"
            );
            Ok(())
        })
    } else {
        Ok(())
    };
    cleanup_profile(root)
        .context("Native request test profile could not be removed after child exit")?;
    anyhow::ensure!(
        status.success(),
        "Native request desktop child failed: {status}"
    );
    model_result?;
    println!("Desktop request coordinator: isolated profile removal verified");
    Ok(())
}

fn cleanup_profile(root: tempfile::TempDir) -> anyhow::Result<()> {
    let path = fs::canonicalize(root.path())?;
    anyhow::ensure!(
        path.parent() == Some(fs::canonicalize(std::env::temp_dir())?.as_path())
            && path.file_name().is_some_and(|name| name
                .to_string_lossy()
                .starts_with("central-native-elicitation-")),
        "Unexpected temporary cleanup scope"
    );
    // WebView child processes can briefly retain cache handles or finish cache
    // writes after the test app exits. Retry sharing violations and directory-
    // not-empty only on this newly owned TempDir; never revisit retained roots,
    // kill processes, change permissions or impose an agent-command timeout.
    // TempDir::close wraps and hides the raw Windows error code. Retain its
    // already validated exact path and use std cleanup so retry is code-based.
    let _ = root.keep();
    let first = fs::remove_dir_all(&path);
    if first.is_ok() {
        return Ok(());
    }
    let mut error = first.unwrap_err();
    for _ in 0..50 {
        if !retryable_profile_cleanup_error(&error) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
        match fs::remove_dir_all(&path) {
            Ok(()) => return Ok(()),
            Err(next) if next.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(next) => error = next,
        }
    }
    Err(anyhow::anyhow!(
        "Temporary fixture retained at {}: {error}",
        path.display()
    ))
}

fn retryable_profile_cleanup_error(error: &std::io::Error) -> bool {
    cfg!(windows) && matches!(error.raw_os_error(), Some(32 | 33 | 145))
}

pub(super) fn sample(owner: &str) -> String {
    format!(
        "(() => {{const sample={}; {} sample.formLabel=root?.querySelector('.native-requests input[name=label]')?.value;return sample;}})()",
        approvals::sample(owner),
        approvals::root_script(owner)
    )
}

#[derive(Default)]
pub(super) struct Elicitation {
    turn: Option<in_turn::Turn>,
    opened_graph: bool,
    opened: bool,
    ready_thread: Option<String>,
    index: usize,
    phase: u8,
    key: Option<ServerRequestKey>,
    answered: bool,
    resolved: bool,
    ticket: Option<Ticket>,
    reply: Option<Value>,
    error: Option<String>,
}
impl Elicitation {
    pub(super) fn new() -> Self {
        Self {
            turn: in_turn_enabled().then(in_turn::Turn::default),
            ..Self::default()
        }
    }
    pub(super) fn observe(&mut self, app: &BrowserApp, owner: &str, event: &BrowserEvent) {
        if let Some(turn) = &mut self.turn {
            match turn.observe(app, owner, event, &mut self.reply) {
                Ok(true) => return,
                Ok(false) => {}
                Err(error) => {
                    self.error = Some(error.to_string());
                    return;
                }
            }
        }
        match event {
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::Notification { method, params },
                ..
            }) => {
                if method == "turn/started" && self.turn.is_none() {
                    self.error =
                        Some("No inference turn may start during MCP fixture acceptance".into());
                }
                let same = app
                    .app_server
                    .conversations
                    .binding(owner)
                    .is_some_and(|binding| params["threadId"] == binding.thread_id);
                if method == "mcpServer/startupStatus/updated"
                    && params["name"] == "elicitation_fixture"
                    && params["threadId"].is_string()
                {
                    if params["status"] == "ready" {
                        // Native startup can complete before thread/start's
                        // response installs its binding. Retain the exact ID.
                        self.ready_thread = params["threadId"].as_str().map(str::to_owned);
                    } else if params["status"] != "starting" {
                        self.error = Some("Fixture MCP did not start".into());
                    }
                }
                if method == "serverRequest/resolved" {
                    if same
                        && self
                            .key
                            .as_ref()
                            .is_some_and(|key| json!(key.id) == params["requestId"])
                    {
                        self.resolved = true;
                    } else {
                        self.error = Some("MCP resolution targeted an unexpected request".into());
                    }
                }
            }
            BrowserEvent::AppServer(Event::Transport {
                event:
                    TransportEvent::ServerRequest {
                        key,
                        method,
                        params,
                    },
                ..
            }) => {
                let expected = CASES.get(self.index).map(|case| case.0);
                if self.key.is_some()
                    || method != "mcpServer/elicitation/request"
                    || params["serverName"] != "elicitation_fixture"
                    || params["mode"].as_str() != expected
                    || !self
                        .turn
                        .as_ref()
                        .map_or_else(|| params["turnId"].is_null(), |t| t.matches(params))
                    || !app
                        .app_server
                        .conversations
                        .binding(owner)
                        .is_some_and(|binding| params["threadId"] == binding.thread_id)
                {
                    self.error = Some("Unexpected native MCP request; no answer authorized".into());
                } else {
                    self.key = Some(key.clone());
                }
            }
            BrowserEvent::AppServer(Event::Answered {
                owner: reply_owner,
                key,
                result,
                ..
            }) if reply_owner == owner => {
                if self.key.as_ref() != Some(key) || result.is_err() {
                    self.error = Some("Actual native MCP answer failed or changed identity".into());
                } else {
                    self.answered = true;
                }
            }
            BrowserEvent::AppServer(Event::ConversationReply {
                request, result, ..
            }) if request.owner == owner && result.is_err() => {
                self.error = Some("Native fixture thread did not open".into())
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
        if !app.app_server.view.connected {
            return Ok(false);
        }
        anyhow::ensure!(
            app.app_server.view.account.is_none(),
            "Isolated MCP fixture unexpectedly authenticated"
        );
        if let Some(graph) = graph
            && !self.opened_graph
        {
            if !graph::profile_ready(&app.app_server) {
                return Ok(false);
            }
            graph.open_cards(app)?;
            self.opened_graph = true;
            return Ok(false);
        }
        if sample["ready"] != true {
            return Ok(false);
        }
        let directory = graph
            .and_then(|graph| graph.directory(owner))
            .or_else(|| app.workspace.root())
            .context("No fixture directory")?
            .to_path_buf();
        if !self.opened {
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
            if self.turn.is_none() {
                request.call.params["ephemeral"] = json!(true);
            }
            app.dispatch_app_server_thread(request);
            self.opened = true;
            println!(
                "Desktop MCP fixture: native isolated thread requested; in-turn mode: {}",
                self.turn.is_some()
            );
            approvals::script(
                app,
                owner,
                &format!(
                    "input.value={};input.dispatchEvent(new Event('input',{{bubbles:true}}));",
                    json!(DRAFT)
                ),
            )?;
            return Ok(false);
        }
        let Some(binding) = app.app_server.conversations.binding(owner) else {
            return Ok(false);
        };
        let thread = binding.thread_id.clone();
        if self.ready_thread.as_deref() != Some(thread.as_str())
            || (self.phase == 0 && app.app_server.conversations.busy(owner))
            || sample["draft"] != DRAFT
        {
            return Ok(false);
        }
        anyhow::ensure!(
            self.turn.is_some()
                || app
                    .app_server
                    .conversations
                    .mirror
                    .thread(&thread)
                    .is_some_and(|thread| thread.turns.is_empty()),
            "MCP fixture created a model turn"
        );
        if let Some(graph) = graph {
            for sibling in graph
                .owners()
                .into_iter()
                .filter(|sibling| sibling != owner)
            {
                anyhow::ensure!(
                    app.app_server.conversations.binding(&sibling).is_none()
                        && app.app_server.requests.views(&sibling).is_empty(),
                    "MCP request escaped to sibling"
                );
            }
            anyhow::ensure!(
                app.app_server
                    .conversations
                    .binding(&app.conversation_key(None))
                    .is_none(),
                "MCP request escaped into main conversation"
            );
        }
        if self.index == CASES.len() {
            anyhow::ensure!(
                fs::read_dir(directory)?.next().is_none()
                    && app
                        .project_chats
                        .iter()
                        .all(|chat| chat.messages.is_empty())
                    && app.app_server.conversations.saved().unresolved.is_empty(),
                "MCP test persisted work or changed files"
            );
            return Ok(true);
        }
        let (mode, action) = CASES[self.index];
        if self.phase == 0 {
            self.ticket = Some(if self.turn.is_some() {
                api::start_turn(
                    &thread,
                    &format!("host-mcp-{}", self.index),
                    vec![api::text_input(if mode == "form" {
                        "MCP_FIXTURE_FORM"
                    } else {
                        "MCP_FIXTURE_URL"
                    })],
                    directory.to_str().context("Invalid fixture cwd")?,
                    &api::Profile::default(),
                    api::Access::ReadOnly,
                )
                .send(app.app_server.client.as_ref().unwrap())?
            } else {
                app.app_server.client.as_ref().unwrap().request("mcpServer/tool/call",json!({"threadId":thread,"server":"elicitation_fixture","tool":"fixture_question","arguments":{"mode":mode}}))?
            });
            self.phase = 1;
            println!(
                "Desktop MCP fixture: requested {mode}/{action}; in-turn: {}",
                self.turn.is_some()
            );
            return Ok(false);
        }
        if let Some(ticket) = &self.ticket {
            match ticket.try_result() {
                Ok(result) => {
                    if let Some(turn) = &mut self.turn {
                        turn.acknowledged(result?)?;
                    } else {
                        self.reply = Some(result?);
                    }
                    self.ticket = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(error) => anyhow::bail!("Native fixture reply lost: {error}"),
            }
        }
        let cards = sample["cards"]
            .as_array()
            .context("Missing real MCP request card sample")?;
        if let Some(turn) = &mut self.turn
            && turn.permission(app, owner, cards)?
        {
            return Ok(false);
        }
        if self.phase == 1 && self.key.is_some() && cards.len() == 1 {
            anyhow::ensure!(
                cards[0]["text"]
                    .as_str()
                    .is_some_and(|text| text.contains("elicitation_fixture")),
                "MCP card hides requesting server"
            );
            if mode == "form" && action == "accept" {
                approvals::script(
                    app,
                    owner,
                    "const form=root.querySelector('.native-requests form');form.elements.namedItem('label').value='fixture';form.elements.namedItem('enabled').value='false';form.elements.namedItem('count').value='3';",
                )?;
                if graph.is_some() {
                    app.render_agent_graph_surface();
                } else {
                    app.render_agent_panel();
                }
                self.phase = 2;
                return Ok(false);
            }
            self.phase = 2;
        }
        if self.phase == 2 && cards.len() == 1 {
            if mode == "form" && action == "accept" {
                if sample["formLabel"] != "fixture" {
                    return Ok(false);
                }
                approvals::script(
                    app,
                    owner,
                    "root.querySelector('.native-requests form').requestSubmit();",
                )?;
            } else {
                let label = match action {
                    "accept" => "I have completed the external step",
                    "decline" => "Decline",
                    _ => "Cancel request",
                };
                approvals::script(
                    app,
                    owner,
                    &format!(
                        "const button=[...root.querySelectorAll('.native-requests button')].find(button=>button.textContent==={});if(!button||button.disabled)throw new Error('MCP decision unavailable');button.click();",
                        json!(label)
                    ),
                )?;
            }
            self.phase = 3;
        }
        if self.phase == 3
            && self.answered
            && self.resolved
            && self.reply.is_some()
            && cards.is_empty()
            && self.turn.as_ref().is_none_or(|t| t.done)
        {
            let reply = self.reply.as_ref().unwrap();
            anyhow::ensure!(
                (self.turn.is_some() || reply["isError"] == false)
                    && reply["structuredContent"]["action"] == action,
                "Fixture did not receive the actual selected decision"
            );
            let expected = if mode == "form" && action == "accept" {
                json!({"label":"fixture","enabled":false,"count":3})
            } else if action == "accept" {
                json!({})
            } else {
                Value::Null
            };
            anyhow::ensure!(
                reply["structuredContent"]["content"] == expected,
                "MCP form values changed in transit"
            );
            anyhow::ensure!(
                app.app_server.requests.views(owner).is_empty(),
                "Resolved MCP card remained pending"
            );
            println!(
                "Desktop MCP {mode}/{action}: real card, answer, resolution, exact result and draft verified"
            );
            if let Some(turn) = &mut self.turn {
                turn.verify_history(app, &thread, self.index + 1)?;
                *turn = in_turn::Turn::default();
            }
            self.index += 1;
            self.phase = 0;
            self.key = None;
            self.answered = false;
            self.resolved = false;
            self.ticket = None;
            self.reply = None;
        }
        Ok(false)
    }
}

#[cfg(test)]
#[test]
fn elicitation_profile_requires_owned_root_token_and_isolated_native_home() {
    let root = tempfile::Builder::new()
        .prefix("central-native-elicitation-")
        .tempdir()
        .unwrap();
    let token = Uuid::new_v4().to_string();
    fs::write(root.path().join("owner-token"), &token).unwrap();
    let home = root.path().join("native");
    fs::create_dir(&home).unwrap();
    assert!(validate_context(root.path(), &token, &home).is_ok());
    assert!(validate_context(root.path(), "wrong", &home).is_err());
    assert!(validate_context(root.path(), &token, root.path()).is_err());
    let nested = root.path().join("central-native-elicitation-nested");
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join("owner-token"), &token).unwrap();
    assert!(validate_context(&nested, &token, &home).is_err());
}

#[cfg(all(test, windows))]
#[test]
fn elicitation_cleanup_waits_for_owned_windows_file_handle_release() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = tempfile::Builder::new()
        .prefix("central-native-elicitation-")
        .tempdir()
        .unwrap();
    let path = root.path().to_path_buf();
    let file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .share_mode(0)
        .open(path.join("cache-lock"))
        .unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        drop(file);
    });
    cleanup_profile(root).unwrap();
    release.join().unwrap();
    assert!(!path.exists());
}

#[cfg(all(test, windows))]
#[test]
fn elicitation_cleanup_retries_cache_races_not_access_denied() {
    for code in [32, 33, 145] {
        assert!(retryable_profile_cleanup_error(
            &std::io::Error::from_raw_os_error(code)
        ));
    }
    for code in [2, 3, 5, 13, 123] {
        assert!(!retryable_profile_cleanup_error(
            &std::io::Error::from_raw_os_error(code)
        ));
    }
    assert!(!retryable_profile_cleanup_error(&std::io::Error::other(
        "unknown failure"
    )));
}
