//! Opt-in two-process desktop restart. The parent owns a fresh disposable profile;
//! children use production loaders, UI handlers and native calls, never replay.
use super::*;
use anyhow::Context as _;
use delivery::{input, script};
mod wire;
pub(super) use wire::Mode as WireMode;

const ROOT_ENV: &str = "CENTRAL_AGENT_RESTART_TEST_ROOT";
const TOKEN_ENV: &str = "CENTRAL_AGENT_RESTART_TEST_TOKEN";
const STAGE_ENV: &str = "CENTRAL_AGENT_RESTART_TEST_STAGE";
const FIRST: &str =
    "Reply with exactly RESTART_FIRST. Do not use tools, read files or change anything.";
const SECOND: &str =
    "Reply with exactly RESTART_SECOND. Do not use tools, read files or change anything.";
const WIRE_LOST: &str =
    "Reply with exactly RESTART_WIRE_LOST. Do not use tools, read files or change anything.";
const DRAFT: &str = "New draft survives native history load after app restart";
const SIBLING_DRAFT: &str = "Sibling draft survives another card's uncertain restart";
const ACTIVE_FIRST: &str = "Without tools, file reads or changes, print integers from 1 through 400, one per line without skipping. This is a desktop-close-during-stream test.";
fn first_prompt(active: bool) -> &'static str {
    if active { ACTIVE_FIRST } else { FIRST }
}
pub(super) fn sample(owner: &str) -> String {
    format!(
        "(()=>{{{}const state={};state.userText=[...root?.querySelectorAll('.chat-message.user')||[]].map(row=>row.textContent).join('\\n');state.draftStatus=root?.querySelector('textarea[data-role=\"message\"],#chat-input')?.dataset.draftStatus;state.siblingDrafts=[...document.querySelectorAll('.agent-console[data-node-key]')].filter(card=>card!==root).map(card=>({{text:card.querySelector('textarea[data-role=\"message\"]')?.value,status:card.querySelector('textarea[data-role=\"message\"]')?.dataset.draftStatus}}));return state;}})()",
        delivery::root_script(owner),
        recovery::sample(owner)
    )
}

pub(super) struct Context {
    pub root: PathBuf,
    pub second: bool,
}
impl Context {
    pub(super) fn from_environment() -> anyhow::Result<Option<Self>> {
        let Some(root) = std::env::var_os(ROOT_ENV) else {
            return Ok(None);
        };
        let token = std::env::var(TOKEN_ENV).context("Missing restart test token")?;
        let stage = std::env::var(STAGE_ENV).context("Missing restart stage")?;
        Self::validate(PathBuf::from(root), &token, &stage).map(Some)
    }

    fn validate(root: PathBuf, token: &str, stage: &str) -> anyhow::Result<Self> {
        let root = fs::canonicalize(root)?;
        let temp = fs::canonicalize(std::env::temp_dir())?;
        anyhow::ensure!(
            root.parent() == Some(temp.as_path())
                && root.file_name().is_some_and(|name| name
                    .to_string_lossy()
                    .starts_with("central-native-restart-")),
            "Restart profile must be a dedicated temporary directory"
        );
        anyhow::ensure!(
            Uuid::parse_str(token).is_ok()
                && fs::read_to_string(root.join("owner-token"))? == token,
            "Restart test profile ownership mismatch"
        );
        let second = match stage {
            "first" => false,
            "second" => true,
            _ => anyhow::bail!("Invalid restart stage"),
        };
        Ok(Self { root, second })
    }
}

#[cfg(test)]
#[test]
fn restart_profile_requires_owned_temp_scope_token_and_stage() {
    let profile = tempfile::Builder::new()
        .prefix("central-native-restart-")
        .tempdir()
        .unwrap();
    let token = Uuid::new_v4().to_string();
    fs::write(profile.path().join("owner-token"), &token).unwrap();
    assert!(
        !Context::validate(profile.path().into(), &token, "first")
            .unwrap()
            .second
    );
    assert!(
        Context::validate(profile.path().into(), &token, "second")
            .unwrap()
            .second
    );
    assert!(
        Context::validate(profile.path().into(), &Uuid::new_v4().to_string(), "second").is_err()
    );
    assert!(Context::validate(profile.path().into(), &token, "unknown").is_err());
    let nested = profile.path().join("central-native-restart-nested");
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join("owner-token"), &token).unwrap();
    assert!(Context::validate(nested, &token, "first").is_err());
}

fn cleanup(root: &Path) -> anyhow::Result<()> {
    let data = root.join("data");
    let book = read_bindings(&data.join("app-server-threads.json")).map_err(anyhow::Error::msg)?;
    if book.saved().bindings.is_empty() {
        return Ok(());
    }
    let home = std::env::var_os("CODEX_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| data.join("codex"));
    let runtime = Runtime::discover(&data)
        .and_then(|runtime| runtime.with_home(&home))
        .map_err(anyhow::Error::msg)?;
    let (sender, events) = mpsc::channel();
    let client = Client::spawn(&runtime, env!("CARGO_PKG_VERSION"), move |event| {
        let _ = sender.send(event);
    })
    .map_err(anyhow::Error::msg)?;
    let result = (|| -> anyhow::Result<()> {
        anyhow::ensure!(
            matches!(
                events.recv_timeout(Duration::from_secs(30)),
                Ok(TransportEvent::Ready { .. })
            ),
            "Restart cleanup handshake failed"
        );
        for binding in book.saved().bindings.values() {
            // The directory was created empty by this parent; never list personal threads.
            let state = api::read_thread(&binding.thread_id).send(&client)?.wait()?;
            if let Some(turns) = state["thread"]["turns"].as_array() {
                for turn in turns.iter().filter(|turn| turn["status"] == "inProgress") {
                    api::interrupt_turn(
                        &binding.thread_id,
                        turn["id"].as_str().context("Missing active turn")?,
                    )
                    .send(&client)?
                    .wait()?;
                }
            }
            if retain_test_history() {
                println!(
                    "Restart parent: retained owned test history {} by explicit --retain-test-history.",
                    binding.thread_id
                );
            } else {
                api::delete_thread(&binding.thread_id)
                    .send(&client)?
                    .wait()?;
                println!(
                    "Restart parent: deleted its owned native history {}.",
                    binding.thread_id
                );
            }
        }
        Ok(())
    })();
    client.shutdown();
    result
}

fn crash_proof(proof: &Value, child_id: u32) -> anyhow::Result<u32> {
    let native = proof["nativePid"]
        .as_u64()
        .and_then(|id| u32::try_from(id).ok())
        .context("Missing owned native process")?;
    anyhow::ensure!(
        proof["pid"] == child_id
            && proof["abrupt"] == true
            && (proof["activeClose"] == true || proof["lostAck"] == true)
            && proof["thread"].as_str().is_some_and(|id| !id.is_empty())
            && proof["turn"].as_str().is_some_and(|id| !id.is_empty())
            && native != 0
            && native != child_id
            && native != std::process::id(),
        "Crash proof does not identify the owned active child"
    );
    Ok(native)
}

#[cfg(windows)]
fn crash_first(
    command: &mut std::process::Command,
    root: &Path,
    token: &str,
) -> anyhow::Result<()> {
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use windows::Win32::{
        Foundation::WAIT_OBJECT_0,
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    };
    let mut child = command.spawn()?;
    let result = (|| -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(180);
        loop {
            anyhow::ensure!(
                child.try_wait()?.is_none(),
                "Owned crash child exited before the parent could terminate it"
            );
            if root.join("crash-ready").is_file() {
                break;
            }
            anyhow::ensure!(
                Instant::now() < deadline,
                "Owned crash child did not reach streaming readiness"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        anyhow::ensure!(
            fs::read_to_string(root.join("crash-ready"))? == token,
            "Crash readiness token mismatch"
        );
        let proof: Value = serde_json::from_slice(&fs::read(root.join("first-result.json"))?)?;
        let native_id = crash_proof(&proof, child.id())?;
        // Observe the still-owned native descendant before killing the host.
        // No native termination rights: production kill-on-close job ownership
        // must clean it up when Windows closes the crashed application's handles.
        let native_handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, native_id)? };
        let _native = unsafe { OwnedHandle::from_raw_handle(native_handle.0) };
        let wire_native = if let Some(pid) = proof["wireNativePid"].as_u64() {
            let pid = u32::try_from(pid)?;
            anyhow::ensure!(
                pid != 0 && pid != child.id() && pid != std::process::id() && pid != native_id,
                "Wire evidence did not identify a separate native descendant"
            );
            let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid)? };
            let owned = unsafe { OwnedHandle::from_raw_handle(handle.0) };
            anyhow::ensure!(
                unsafe { WaitForSingleObject(handle, 0) } != WAIT_OBJECT_0,
                "Actual native descendant exited before app crash"
            );
            Some((handle, owned))
        } else {
            None
        };
        anyhow::ensure!(
            unsafe { WaitForSingleObject(native_handle, 0) } != WAIT_OBJECT_0,
            "Native server already exited before app crash"
        );
        child.kill()?; // Exact std::process::Child handle, never a PID/name lookup.
        let status = child.wait()?;
        anyhow::ensure!(
            !status.success(),
            "Owned child did not exit by forced termination"
        );
        anyhow::ensure!(
            unsafe { WaitForSingleObject(native_handle, 30_000) } == WAIT_OBJECT_0,
            "Production native process ownership did not clean up after app crash"
        );
        if let Some((handle, _)) = wire_native {
            anyhow::ensure!(
                unsafe { WaitForSingleObject(handle, 30_000) } == WAIT_OBJECT_0,
                "Actual official server survived its owning app crash"
            );
            println!(
                "Restart parent: the actual official server behind the test relay exited through production process ownership."
            );
        }
        println!(
            "Restart parent: forcibly terminated its owned app PID {}; its native server also exited through production process ownership.",
            child.id()
        );
        Ok(())
    })();
    if child.try_wait()?.is_none() {
        child.kill()?;
        child.wait()?;
    }
    result
}
#[cfg(not(windows))]
fn crash_first(_: &mut std::process::Command, _: &Path, _: &str) -> anyhow::Result<()> {
    anyhow::bail!("Owned app-crash acceptance currently requires Windows")
}

#[cfg(test)]
#[test]
fn restart_crash_proof_rejects_foreign_pids_or_unready_records() {
    let child = u32::MAX - 1;
    let native = u32::MAX - 2;
    let valid = json!({"pid":child,"nativePid":native,"abrupt":true,"activeClose":true,"thread":"fixture","turn":"turn"});
    assert_eq!(crash_proof(&valid, child).unwrap(), native);
    for (field, value) in [
        ("pid", json!(child - 1)),
        ("nativePid", json!(child)),
        ("nativePid", json!(0)),
        ("nativePid", json!(std::process::id())),
        ("abrupt", json!(false)),
        ("activeClose", json!(false)),
        ("turn", Value::Null),
        ("thread", json!("")),
    ] {
        let mut bad = valid.clone();
        bad[field] = value;
        assert!(crash_proof(&bad, child).is_err(), "{field}");
    }
}

pub(super) fn coordinate(
    graph: bool,
    active_close: bool,
    abrupt: bool,
    lost_ack: bool,
    wire_mode: Option<WireMode>,
) -> anyhow::Result<()> {
    let profile = tempfile::Builder::new()
        .prefix("central-native-restart-")
        .tempdir()?;
    let token = Uuid::new_v4().to_string();
    fs::write(profile.path().join("owner-token"), &token)?;
    let wire_fixture = wire_mode
        .map(|_| wire::Fixture::create(profile.path()))
        .transpose()?;
    let result = (|| -> anyhow::Result<()> {
        for stage in ["first", "second"] {
            let mut command = std::process::Command::new(std::env::current_exe()?);
            command
                .args([
                    "--check-native-conversation",
                    "--restart",
                    "--allow-test-inference",
                ])
                .env(ROOT_ENV, profile.path())
                .env(TOKEN_ENV, &token)
                .env(STAGE_ENV, stage);
            if graph {
                command.arg("--graph");
            }
            if active_close {
                command.arg("--restart-active");
            }
            if abrupt {
                command.arg("--restart-crash");
            }
            if let Some(mode) = wire_mode {
                command.arg(format!("--restart-wire-loss={}", mode.name()));
                if stage == "first" {
                    wire_fixture.as_ref().unwrap().configure(&mut command, mode);
                }
            } else if lost_ack {
                command.arg("--restart-lost-ack");
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000);
            }
            if abrupt && stage == "first" {
                crash_first(&mut command, profile.path(), &token)?;
                continue;
            }
            let status = command.status()?;
            anyhow::ensure!(
                status.success(),
                "Restart {stage} application process failed: {status}"
            );
            println!("Restart parent: {stage} application process exited successfully.");
        }
        Ok(())
    })();
    if let Err(error) = cleanup(profile.path()) {
        let preserved = profile.keep();
        anyhow::bail!(
            "Restart cleanup failed: {error}; owned test profile retained at {}",
            preserved.display()
        );
    }
    result?;
    profile
        .close()
        .context("Owned restart test profile could not be removed after process exit")?;
    println!(
        "PASS: {} desktop restart in two separate processes; close during active streaming: {active_close}; abrupt owned app crash: {abrupt}; pending receipt/drafts: {lost_ack}; wire fault: {wire_mode:?}; production persisted binding load, explicit native read/resume, same-thread continuation and independent history. No visual test; host-ACK and wire-loss variants are distinct.",
        if graph { "graph" } else { "main" }
    );
    Ok(())
}

pub(super) struct Restart {
    active_close: bool,
    abrupt: bool,
    streamed: bool,
    context: Context,
    phase: u8,
    opened: bool,
    starts: usize,
    reads: usize,
    resumed: bool,
    thread: Option<String>,
    first_turn: Option<String>,
    directory: Option<PathBuf>,
    error: Option<String>,
    lost_ack: bool,
    held_ack: Option<BrowserEvent>,
    receipt: Option<String>,
    draft_written: bool,
    reviewed: bool,
    wire_mode: Option<WireMode>,
    wire_native_pid: Option<u32>,
}
impl Restart {
    pub(super) fn new(
        context: Context,
        app: &BrowserApp,
        owner: &str,
        active_close: bool,
        abrupt: bool,
        lost_ack: bool,
        wire_mode: Option<WireMode>,
    ) -> anyhow::Result<Self> {
        let mut test = Self {
            active_close,
            abrupt,
            streamed: false,
            context,
            phase: 0,
            opened: false,
            starts: 0,
            reads: 0,
            resumed: false,
            thread: None,
            first_turn: None,
            directory: None,
            error: None,
            lost_ack,
            held_ack: None,
            receipt: None,
            draft_written: false,
            reviewed: false,
            wire_mode,
            wire_native_pid: None,
        };
        if test.context.second {
            let proof: Value =
                serde_json::from_slice(&fs::read(test.context.root.join("first-result.json"))?)?;
            let id = proof["thread"]
                .as_str()
                .context("Missing first native thread")?;
            anyhow::ensure!(
                proof["pid"] != std::process::id()
                    && proof["owner"] == owner
                    && proof["session"] != app.native_session_id
                    && proof["activeClose"] == active_close
                    && proof["abrupt"] == abrupt
                    && proof["lostAck"].as_bool().unwrap_or(false) == lost_ack
                    && proof["wireMode"] == json!(wire_mode.map(WireMode::name)),
                "Restart did not create a distinct application session/owner"
            );
            anyhow::ensure!(
                app.app_server.binding_store_error.is_none()
                    && app
                        .app_server
                        .conversations
                        .binding(owner)
                        .is_some_and(|binding| binding.thread_id == id),
                "Production startup did not load the saved native binding"
            );
            anyhow::ensure!(
                app.app_server.conversations.mirror.thread(id).is_none()
                    && !app.app_server.conversations.is_thread_loaded(owner)
                    && !app.app_server.conversations.observed(owner),
                "Restart reconstructed native state before reading server"
            );
            anyhow::ensure!(
                app.app_server.access() == api::Access::ReadOnly,
                "An untouched permissions preference did not retain read only"
            );
            test.thread = Some(id.into());
            test.first_turn = Some(proof["turn"].as_str().context("Missing first turn")?.into());
            test.directory = Some(PathBuf::from(
                proof["directory"]
                    .as_str()
                    .context("Missing first directory")?,
            ));
            if lost_ack {
                let receipt = proof["receipt"]
                    .as_str()
                    .context("Missing pre-crash receipt")?;
                anyhow::ensure!(
                    app.app_server
                        .conversations
                        .saved()
                        .unresolved
                        .get(owner)
                        .is_some_and(|saved| saved.message_id == receipt && saved.thread_id == id),
                    "Production restart lost or replaced the unacknowledged receipt"
                );
                test.receipt = Some(receipt.into());
            }
        }
        Ok(test)
    }
    /// Lose the actual worker ACK only at the host boundary. This does not
    /// simulate wire loss, alter native messages or fabricate model output.
    pub(super) fn defer_reply(&mut self, owner: &str, event: BrowserEvent) -> Option<BrowserEvent> {
        if self.lost_ack
            && self.wire_mode.is_none()
            && !self.context.second
            && self.held_ack.is_none()
            && matches!(&event, BrowserEvent::AppServer(Event::ConversationReply { request, result: Ok(_), .. }) if request.owner == owner && request.call.method == "turn/start")
        {
            self.held_ack = Some(event);
            println!("Restart child: native worker ACK held before production host delivery.");
            None
        } else {
            Some(event)
        }
    }
    pub(super) fn observe(&mut self, owner: &str, event: &BrowserEvent) {
        if let BrowserEvent::AppServer(Event::Transport {
            event: TransportEvent::Notification { method, params },
            ..
        }) = event
            && method == "item/agentMessage/delta"
            && self.thread.as_deref() == params["threadId"].as_str()
        {
            self.streamed = true;
        }
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
            match request.call.method {
                "thread/start" if self.context.second => {
                    anyhow::bail!("Restart created a replacement native thread")
                }
                "turn/start" => {
                    let expected = if self.context.second {
                        SECOND
                    } else {
                        first_prompt(self.active_close)
                    };
                    let items = request.call.params["input"]
                        .as_array()
                        .context("Missing restart input")?;
                    anyhow::ensure!(
                        self.starts == 0 && items.len() == 1 && items[0]["text"] == expected,
                        "Restart replayed or duplicated prompt input"
                    );
                    self.starts += 1;
                }
                "thread/resume" => {
                    anyhow::ensure!(
                        request.call.params.get("history").is_none(),
                        "Restart supplied reconstructed history"
                    );
                    self.resumed = true;
                }
                "thread/read" => {
                    anyhow::ensure!(
                        self.thread.as_deref() == value["thread"]["id"].as_str(),
                        "Restart read wrong native identity"
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
                        "Restart changed workspace directory"
                    );
                    self.reads += 1;
                }
                _ => {}
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
            }
            if self.context.second {
                if self.lost_ack {
                    if sample["checkHistory"] != true
                        || sample["receipt"] != true
                        || sample["draft"] != DRAFT
                        || sample["draftStatus"] != "saved"
                    {
                        return Ok(false);
                    }
                    if graph.is_some() && !siblings_saved(sample) {
                        return Ok(false);
                    }
                    anyhow::ensure!(
                        self.starts == 0
                            && self.reads == 0
                            && !self.resumed
                            && sample["reviewEnabled"] == false,
                        "Restart replayed input or allowed dismissal before native inspection"
                    );
                    script(
                        app,
                        owner,
                        "[...root.querySelectorAll('.native-requests button')].find(button=>button.textContent==='Check native history'&&!button.disabled).click();",
                    )?;
                    self.phase = 3;
                    println!(
                        "Restart child: production startup restored exact unsent draft and pending receipt before any native history read."
                    );
                    return Ok(false);
                }
                if sample["historyEnabled"] != true {
                    return Ok(false);
                }
                input(app, owner, DRAFT, false)?;
                script(
                    app,
                    owner,
                    "const panel=root.querySelector('.native-conversation');panel.open=true;panel.querySelector('button[aria-label=\"Load or refresh native Codex history\"]').click();",
                )?;
                self.phase = 3;
            } else {
                self.directory = Some(
                    graph
                        .and_then(|graph| graph.directory(owner))
                        .or_else(|| app.workspace.root())
                        .context("Missing restart workspace")?
                        .to_path_buf(),
                );
                input(app, owner, first_prompt(self.active_close), true)?;
                self.phase = 1;
            }
            return Ok(false);
        }
        let book = &app.app_server.conversations;
        anyhow::ensure!(
            book.saved().bindings.keys().all(|key| key == owner),
            "Restart created a foreign binding"
        );
        let Some(binding) = book.binding(owner) else {
            return Ok(false);
        };
        if let Some(id) = &self.thread {
            anyhow::ensure!(*id == binding.thread_id, "Restart replaced native thread");
        } else {
            self.thread = Some(binding.thread_id.clone());
        }
        let Some(thread) = book.mirror.thread(&binding.thread_id) else {
            return Ok(false);
        };
        anyhow::ensure!(
            thread.turns.iter().all(|turn| turn.status != "failed"),
            "Restart turn failed"
        );
        if graph.is_some() {
            let siblings = sample["siblings"]
                .as_array()
                .context("Missing restart sibling")?;
            anyhow::ensure!(
                siblings.len() == 1
                    && siblings[0]["active"] != "true"
                    && siblings[0]["requests"] == 0,
                "Restart activated sibling graph card"
            );
            if self.lost_ack && self.context.second {
                anyhow::ensure!(
                    siblings_saved(sample),
                    "Uncertain restart changed the sibling's persisted draft"
                );
            }
        }
        if self.wire_mode.is_some()
            && !self.context.second
            && self.phase == 1
            && self.starts == 1
            && !app.app_server.busy(owner)
            && thread.turns.len() == 1
            && thread.turns[0].status == "completed"
        {
            self.first_turn = Some(thread.turns[0].id.clone());
            input(app, owner, WIRE_LOST, true)?;
            self.phase = 8;
            println!(
                "Restart child: completed seed; submitting one prompt whose request/ACK will be lost at stdio."
            );
            return Ok(false);
        }
        if let Some(mode) = self.wire_mode
            && !self.context.second
            && self.phase == 8
        {
            self.wire_native_pid = mode.marker(&self.context.root)?;
        }
        if self.lost_ack
            && !self.context.second
            && ((self.wire_mode.is_none() && self.phase == 1 && self.held_ack.is_some())
                || (self.wire_mode.is_some() && self.phase == 8 && self.wire_native_pid.is_some()))
            && self.starts == 1
            && thread.turns.len() == 1
            && thread.turns[0].status == "completed"
        {
            let receipt = book
                .saved()
                .unresolved
                .get(owner)
                .context("Missing unacknowledged receipt")?;
            self.receipt = Some(receipt.message_id.clone());
            self.first_turn = Some(thread.turns[0].id.clone());
            if !self.draft_written {
                input(app, owner, DRAFT, false)?;
                if let Some(graph) = graph {
                    for sibling in graph.owners().into_iter().filter(|s| s != owner) {
                        input(app, &sibling, SIBLING_DRAFT, false)?;
                    }
                }
                self.draft_written = true;
                return Ok(false);
            }
            if sample["draft"] != DRAFT
                || sample["draftStatus"] != "saved"
                || graph.is_some() && !siblings_saved(sample)
            {
                return Ok(false);
            }
            let stored = read_bindings(&app.data_dir.join("app-server-threads.json"))
                .map_err(anyhow::Error::msg)?;
            anyhow::ensure!(
                fs::read_dir(self.directory.as_ref().context("Missing owned workspace")?)?
                    .next()
                    .is_none(),
                "No-tool restart fixture changed its workspace"
            );
            anyhow::ensure!(
                stored
                    .saved()
                    .unresolved
                    .get(owner)
                    .is_some_and(
                        |r| r.message_id == receipt.message_id && r.thread_id == binding.thread_id
                    ),
                "Uncertain receipt was not durably saved before crash"
            );
            self.write_proof(app, owner, &binding.thread_id)?;
            fs::write(
                self.context.root.join("crash-ready"),
                fs::read(self.context.root.join("owner-token"))?,
            )?;
            self.phase = 7;
            println!(
                "Restart child: receipt and newer main/sibling drafts persisted through production IPC; parent may crash this app without any close-handler flush."
            );
            return Ok(false);
        }
        if self.lost_ack && self.context.second && self.phase == 3 && self.reads == 1 {
            if book.saved().unresolved.contains_key(owner) {
                if sample["reviewEnabled"] == true && !self.reviewed {
                    script(
                        app,
                        owner,
                        "[...root.querySelectorAll('.native-requests button')].find(button=>button.textContent==='I reviewed the history · dismiss delivery warning'&&!button.disabled).click();",
                    )?;
                    self.reviewed = true;
                }
                return Ok(false);
            }
            anyhow::ensure!(
                self.reviewed
                    || thread
                        .turns
                        .iter()
                        .flat_map(|t| &t.items)
                        .any(|i| i.value["type"] == "userMessage"
                            && i.value["clientId"].as_str() == self.receipt.as_deref()),
                "Receipt cleared without exact native correlation or explicit history review"
            );
        }
        match self.phase {
            1 if !self.lost_ack
                && self.active_close
                && self.starts == 1
                && self.streamed
                && book.active_turn(owner).is_some()
                && thread.turns.len() == 1
                && thread.turns[0].status == "inProgress" =>
            {
                self.first_turn = Some(thread.turns[0].id.clone());
                anyhow::ensure!(
                    book.active_turn(owner) == self.first_turn.as_deref(),
                    "Closing did not target the streamed turn"
                );
                let stored = read_bindings(&app.data_dir.join("app-server-threads.json"))
                    .map_err(anyhow::Error::msg)?;
                anyhow::ensure!(
                    stored
                        .binding(owner)
                        .is_some_and(|saved| saved.thread_id == binding.thread_id)
                        && stored.saved().unresolved.is_empty(),
                    "Active binding was not accepted and persisted before close"
                );
                self.write_proof(app, owner, &binding.thread_id)?;
                if self.abrupt {
                    fs::write(
                        self.context.root.join("crash-ready"),
                        fs::read(self.context.root.join("owner-token"))?,
                    )?;
                    self.phase = 7;
                    println!(
                        "Restart child: acknowledged native response is streaming; parent may crash this owned app now. No close handler or pre-crash state flush."
                    );
                    return Ok(false);
                }
                println!(
                    "Restart child: acknowledged native turn is streaming; requesting actual normal application close without a test-side interrupt."
                );
                return Ok(true);
            }
            1 if !self.lost_ack
                && self.starts == 1
                && !app.app_server.busy(owner)
                && thread
                    .turns
                    .first()
                    .is_some_and(|turn| turn.status == "completed") =>
            {
                if !sample["text"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("RESTART_FIRST")
                {
                    return Ok(false);
                }
                self.first_turn = Some(thread.turns[0].id.clone());
                app.app_server_conversation(owner.into(), ConversationAction::Read {});
                self.phase = 2;
            }
            2 if self.reads == 1 && !app.app_server.busy(owner) => {
                anyhow::ensure!(
                    thread.turns.len() == 1 && book.saved().unresolved.is_empty(),
                    "First restart session was not settled"
                );
                anyhow::ensure!(
                    !self.active_close,
                    "Turn completed before active-close acceptance"
                );
                self.write_proof(app, owner, &binding.thread_id)?;
                println!(
                    "Restart child: first completed native turn verified; requesting normal application close."
                );
                return Ok(true);
            }
            3 if self.reads == 1
                && !app.app_server.busy(owner)
                && sample["resumeEnabled"] == true =>
            {
                anyhow::ensure!(
                    self.starts == 0
                        && thread.turns.len() == self.prior_turns()
                        && Some(thread.turns[0].id.as_str()) == self.first_turn.as_deref(),
                    "Reload changed native history: starts={}, turns={}, expected first={:?}, actual={:?}",
                    self.starts,
                    thread.turns.len(),
                    self.first_turn,
                    thread.turns.first().map(|turn| &turn.id)
                );
                // The native read ACK can beat the next asynchronous WebView
                // sample. Wait for actual rendered history and the retained
                // draft; the overall acceptance deadline catches a real loss.
                if sample["draft"] != DRAFT
                    || !(if self.active_close {
                        &sample["userText"]
                    } else {
                        &sample["text"]
                    })
                    .as_str()
                    .unwrap_or_default()
                    .contains(if self.active_close {
                        ACTIVE_FIRST
                    } else {
                        "RESTART_FIRST"
                    })
                {
                    return Ok(false);
                }
                script(
                    app,
                    owner,
                    "[...root.querySelectorAll('.native-conversation button')].find(button=>button.textContent==='Resume connection'&&!button.disabled).click();",
                )?;
                self.phase = 4;
            }
            4 if self.resumed && book.is_thread_loaded(owner) && !app.app_server.busy(owner) => {
                anyhow::ensure!(
                    self.starts == 0
                        && thread.turns.len() == self.prior_turns()
                        && sample["draft"] == DRAFT,
                    "Native resume replayed history or cleared draft"
                );
                input(app, owner, SECOND, true)?;
                self.phase = 5;
            }
            5 if self.starts == 1
                && !app.app_server.busy(owner)
                && thread.turns.len() == self.prior_turns() + 1
                && thread
                    .turns
                    .last()
                    .is_some_and(|turn| turn.status == "completed") =>
            {
                if !sample["text"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("RESTART_SECOND")
                {
                    return Ok(false);
                }
                app.app_server_conversation(owner.into(), ConversationAction::Read {});
                self.phase = 6;
            }
            6 if self.reads == 2 && !app.app_server.busy(owner) => {
                anyhow::ensure!(
                    fs::read_dir(
                        self.directory
                            .as_ref()
                            .context("Missing restored workspace")?
                    )?
                    .next()
                    .is_none()
                        && thread
                            .turns
                            .iter()
                            .flat_map(|turn| &turn.items)
                            .all(|item| matches!(
                                item.value["type"].as_str(),
                                Some("userMessage" | "agentMessage" | "reasoning")
                            )),
                    "No-tool restart fixture changed files or native history contains unexpected tool work"
                );
                let inputs = thread
                    .turns
                    .iter()
                    .flat_map(|turn| &turn.items)
                    .filter(|item| item.value["type"] == "userMessage")
                    .collect::<Vec<_>>();
                anyhow::ensure!(
                    inputs.len() == self.prior_turns() + 1
                        && thread.turns.len() == self.prior_turns() + 1
                        && inputs[0].value["content"]
                            .as_array()
                            .is_some_and(|items| items
                                .iter()
                                .any(|item| item["text"] == first_prompt(self.active_close)))
                        && inputs[self.prior_turns()].value["content"]
                            .as_array()
                            .is_some_and(|items| items.iter().any(|item| item["text"] == SECOND))
                        && Some(thread.turns[0].id.as_str()) == self.first_turn.as_deref()
                        && book.saved().unresolved.is_empty(),
                    "Restart readback lost or duplicated native turns"
                );
                if self.wire_mode == Some(WireMode::After) {
                    anyhow::ensure!(
                        inputs[1].value["content"]
                            .as_array()
                            .is_some_and(|items| items.len() == 1 && items[0]["text"] == WIRE_LOST)
                            && thread.turns[1].status == "completed",
                        "Native accepted lost input is missing, changed or duplicated"
                    );
                }
                anyhow::ensure!(
                    app.project_chat_history_snapshot()
                        .chats
                        .iter()
                        .all(|chat| chat.messages.is_empty())
                        && app.agent_graph_sessions.is_empty(),
                    "Restart copied native transcript into legacy history"
                );
                println!(
                    "Restart child: production disk load, new session, exact native history/resume and continuation verified."
                );
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }
    fn prior_turns(&self) -> usize {
        if self.wire_mode == Some(WireMode::After) {
            2
        } else {
            1
        }
    }
    fn write_proof(&self, app: &BrowserApp, owner: &str, thread: &str) -> anyhow::Result<()> {
        fs::write(
            self.context.root.join("first-result.json"),
            serde_json::to_vec(
                &json!({"pid":std::process::id(),"session":app.native_session_id,"owner":owner,"thread":thread,"turn":self.first_turn,"directory":self.directory,"activeClose":self.active_close,"abrupt":self.abrupt,"lostAck":self.lost_ack,"receipt":self.receipt,"wireMode":self.wire_mode.map(WireMode::name),"wireNativePid":self.wire_native_pid,"nativePid":app.app_server.client.as_ref().and_then(Client::process_id)}),
            )?,
        )?;
        Ok(())
    }
}

fn siblings_saved(sample: &Value) -> bool {
    sample["siblingDrafts"].as_array().is_some_and(|siblings| {
        siblings.len() == 1
            && siblings[0]["text"] == SIBLING_DRAFT
            && siblings[0]["status"] == "saved"
    })
}

#[cfg(test)]
#[test]
fn uncertain_restart_oracle_requires_exact_saved_sibling_and_owned_crash_identity() {
    assert!(siblings_saved(
        &json!({"siblingDrafts":[{"text":SIBLING_DRAFT,"status":"saved"}]})
    ));
    for value in [
        json!({"siblingDrafts":[]}),
        json!({"siblingDrafts":[{"text":SIBLING_DRAFT,"status":"saving"}]}),
        json!({"siblingDrafts":[{"text":"different","status":"saved"}]}),
        json!({"siblingDrafts":[{"text":SIBLING_DRAFT,"status":"saved"},{"text":SIBLING_DRAFT,"status":"saved"}]}),
    ] {
        assert!(!siblings_saved(&value));
    }
    let child = u32::MAX - 1;
    let proof = json!({"pid":child,"nativePid":u32::MAX - 2,"abrupt":true,"activeClose":false,"lostAck":true,"thread":"owned","turn":"native-turn"});
    assert!(crash_proof(&proof, child).is_ok());
    assert!(crash_proof(&proof, child - 1).is_err());
    let script = sample("graph:entity:fixture");
    assert!(script.contains("card.dataset.nodeKey===\"entity:fixture\""));
    assert!(script.contains("state.draftStatus"));
    assert!(script.contains("Check native history"));
}
