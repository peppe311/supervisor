//! Persistent, full-duplex stdio with a bounded inbound JSONL frame. Calls return
//! tickets without waiting on model work. No hidden retry or inference loop lives
//! here; turn lifetime remains owned by App Server and explicit user cancellation.
use crate::{
    runtime::Runtime,
    wire::{self, Incoming, Phase, RequestId, RpcError, ServerRequestKey, Session},
};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
// Full-history thread/read and thread/resume responses are single JSONL frames.
// Real long-running Codex conversations can exceed 16 MiB before the client can
// switch to incremental notifications, so keep a larger but still hard bound.
const MAX_JSONL_FRAME_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub enum CallError {
    Rejected(String),
    Rpc(RpcError),
    /// The server may have accepted the call. Reconcile, never resend blindly.
    Disconnected {
        reason: String,
        delivery_unknown: bool,
    },
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(reason) => f.write_str(reason),
            Self::Rpc(error) => error.fmt(f),
            Self::Disconnected {
                reason,
                delivery_unknown,
            } => write!(
                f,
                "{reason}{}",
                if *delivery_unknown {
                    "; request acceptance is unknown"
                } else {
                    ""
                }
            ),
        }
    }
}
impl std::error::Error for CallError {}

#[derive(Clone, Debug)]
pub enum Event {
    Ready {
        initialization: Value,
    },
    Notification {
        method: String,
        params: Value,
    },
    ServerRequest {
        key: ServerRequestKey,
        method: String,
        params: Value,
    },
    Closed {
        reason: String,
    },
}

pub struct Ticket {
    pub id: RequestId,
    reply: mpsc::Receiver<Result<Value, CallError>>,
}
impl Ticket {
    /// Use on a worker, never the UI thread. EOF/shutdown resolves all tickets.
    pub fn wait(self) -> Result<Value, CallError> {
        self.reply.recv().unwrap_or_else(|_| {
            Err(CallError::Disconnected {
                reason: "App Server reply channel closed".into(),
                delivery_unknown: true,
            })
        })
    }
    pub fn try_result(&self) -> Result<Result<Value, CallError>, mpsc::TryRecvError> {
        self.reply.try_recv()
    }
}

struct State {
    session: Session,
    input: Option<BufWriter<Box<dyn Write + Send>>>,
    waiters: HashMap<RequestId, mpsc::Sender<Result<Value, CallError>>>,
}

impl State {
    fn write(&mut self, frame: &Value) -> Result<(), String> {
        let input = self.input.as_mut().ok_or("App Server input is closed")?;
        serde_json::to_writer(&mut *input, frame)
            .map_err(|e| format!("App Server write failed: {e}"))?;
        input
            .write_all(b"\n")
            .and_then(|()| input.flush())
            .map_err(|e| format!("App Server write failed: {e}"))
    }
}

struct Core {
    state: Mutex<State>,
    emit: Arc<dyn Fn(Event) + Send + Sync>,
}

struct ProcessGuard {
    child: Mutex<Option<OwnedChild>>,
    core: Arc<Core>,
    #[cfg(windows)]
    job: Mutex<Option<std::os::windows::io::OwnedHandle>>,
}

enum OwnedChild {
    Standard(Child),
    #[cfg(windows)]
    Hidden(crate::windows_process::HiddenChild),
}

impl OwnedChild {
    fn id(&self) -> u32 {
        match self {
            Self::Standard(child) => child.id(),
            #[cfg(windows)]
            Self::Hidden(child) => child.id(),
        }
    }

    fn try_wait(&mut self) -> io::Result<Option<()>> {
        match self {
            Self::Standard(child) => child.try_wait().map(|status| status.map(|_| ())),
            #[cfg(windows)]
            Self::Hidden(child) => child.try_wait().map(|status| status.map(|_| ())),
        }
    }

    fn kill(&mut self) -> io::Result<()> {
        match self {
            Self::Standard(child) => child.kill(),
            #[cfg(windows)]
            Self::Hidden(child) => child.kill(),
        }
    }

    fn wait(&mut self) -> io::Result<()> {
        match self {
            Self::Standard(child) => child.wait().map(|_| ()),
            #[cfg(windows)]
            Self::Hidden(child) => child.wait().map(|_| ()),
        }
    }
}

struct SpawnedProcess {
    child: OwnedChild,
    input: Box<dyn Write + Send>,
    output: Box<dyn Read + Send>,
    errors: Box<dyn Read + Send>,
    #[cfg(windows)]
    job: std::os::windows::io::OwnedHandle,
}

impl ProcessGuard {
    fn stop(&self) {
        // Close stdin first so the owned App Server can flush its rollout and
        // configuration state. A bounded fallback still terminates the whole
        // Windows job when the process does not exit on EOF.
        fail(&self.core, "App Server connection was closed".into());
        let Some(mut process) = self.child.lock().ok().and_then(|mut slot| slot.take()) else {
            return;
        };
        let mut exited = false;
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match process.try_wait() {
                Ok(Some(_)) => {
                    exited = true;
                    break;
                }
                Ok(None) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(10));
                }
                Ok(None) | Err(_) => break,
            }
        }
        #[cfg(windows)]
        if let Ok(mut job) = self.job.lock() {
            // Explicit shutdown must release the job even while client clones
            // exist. If EOF did not stop the root, closing it kills the tree.
            job.take();
        }
        if !exited {
            let _ = process.kill();
        }
        let _ = process.wait();
    }
}
impl Drop for ProcessGuard {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Clone)]
pub struct Client {
    core: Arc<Core>,
    process: Arc<ProcessGuard>,
}

impl Client {
    /// Spawns one private official server; neither uses nor changes a daemon
    /// belonging to Codex Desktop, another client, or another user.
    pub fn spawn(
        runtime: &Runtime,
        client_version: &str,
        on_event: impl Fn(Event) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        let (command, prepare_hidden_console) = runtime.server_command();
        Self::spawn_command_with_console(command, client_version, on_event, prepare_hidden_console)
    }

    #[cfg(test)]
    fn spawn_command(
        command: Command,
        client_version: &str,
        on_event: impl Fn(Event) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        Self::spawn_command_with_console(command, client_version, on_event, false)
    }

    fn spawn_command_with_console(
        command: Command,
        client_version: &str,
        on_event: impl Fn(Event) + Send + Sync + 'static,
        prepare_hidden_console: bool,
    ) -> Result<Self, String> {
        let spawned = spawn_process(command, prepare_hidden_console)?;
        let SpawnedProcess {
            child,
            input,
            output,
            mut errors,
            #[cfg(windows)]
            job,
        } = spawned;
        let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
        let mut state = State {
            session: Session::new(generation),
            input: Some(BufWriter::new(input)),
            waiters: HashMap::new(),
        };
        let initialize = state.session.initialize(client_version)?;
        let core = Arc::new(Core {
            state: Mutex::new(state),
            emit: Arc::new(on_event),
        });
        let process = Arc::new(ProcessGuard {
            child: Mutex::new(Some(child)),
            core: core.clone(),
            #[cfg(windows)]
            job: Mutex::new(Some(job)),
        });
        let client = Self {
            core: core.clone(),
            process,
        };
        core.state
            .lock()
            .map_err(|_| "App Server state is unavailable")?
            .write(&initialize)?;
        thread::Builder::new()
            .name("codex-stderr".into())
            .spawn(move || {
                // Always drain stderr. Do not forward raw diagnostic output to chat or
                // logs: it may contain credential material or private configuration.
                let mut buffer = [0u8; 8192];
                loop {
                    match errors.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
            })
            .map_err(|e| format!("Could not start App Server stderr reader: {e}"))?;
        thread::Builder::new()
            .name("codex-jsonl".into())
            .spawn(move || {
                let mut reader = BufReader::new(output);
                let mut frame = Vec::new();
                loop {
                    match read_jsonl_frame(&mut reader, &mut frame, MAX_JSONL_FRAME_BYTES) {
                        Ok(0) => {
                            fail(&core, "Codex App Server exited or closed stdout".into());
                            break;
                        }
                        Err(error) => {
                            fail(&core, format!("App Server read failed: {error}"));
                            break;
                        }
                        Ok(_) => {
                            let line = match std::str::from_utf8(&frame) {
                                Ok(line) => line,
                                Err(error) => {
                                    fail(
                                        &core,
                                        format!("App Server frame is not valid UTF-8: {error}"),
                                    );
                                    break;
                                }
                            };
                            if line.trim().is_empty() {
                                continue;
                            }
                            match wire::decode(line).and_then(|frame| receive(&core, frame)) {
                                Ok(()) => {}
                                Err(error) => {
                                    fail(&core, error);
                                    break;
                                }
                            }
                        }
                    }
                }
            })
            .map_err(|e| format!("Could not start App Server event reader: {e}"))?;
        Ok(client)
    }

    pub fn ready(&self) -> bool {
        self.core
            .state
            .lock()
            .is_ok_and(|s| s.session.phase() == Phase::Ready)
    }

    pub fn request(&self, method: &str, params: Value) -> Result<Ticket, CallError> {
        check_product_method(method)?;
        let (tx, rx) = mpsc::channel();
        let result = {
            let mut state = self
                .core
                .state
                .lock()
                .map_err(|_| CallError::Rejected("App Server state is unavailable".into()))?;
            let (id, frame) = state
                .session
                .begin(method, params)
                .map_err(CallError::Rejected)?;
            state.waiters.insert(id.clone(), tx);
            match state.write(&frame) {
                Ok(()) => Ok(Ticket { id, reply: rx }),
                Err(reason) => Err(reason),
            }
        };
        result.map_err(|reason| {
            fail(&self.core, reason.clone());
            CallError::Disconnected {
                reason,
                delivery_unknown: true,
            }
        })
    }

    pub fn answer(
        &self,
        key: &ServerRequestKey,
        result: Result<Value, RpcError>,
    ) -> Result<(), CallError> {
        let result = {
            let mut state = self
                .core
                .state
                .lock()
                .map_err(|_| CallError::Rejected("App Server state is unavailable".into()))?;
            let frame = state
                .session
                .answer(key, result)
                .map_err(CallError::Rejected)?;
            state.write(&frame)
        };
        result.map_err(|reason| {
            fail(&self.core, reason.clone());
            CallError::Disconnected {
                reason,
                delivery_unknown: true,
            }
        })
    }

    pub fn shutdown(&self) {
        self.process.stop();
    }

    /// Diagnostic identity of this client's owned child, not a process-name lookup.
    /// Returns no PID after it has exited or ownership has been released.
    pub fn process_id(&self) -> Option<u32> {
        let mut child = self.process.child.lock().ok()?;
        let child = child.as_mut()?;
        match child.try_wait() {
            Ok(None) => Some(child.id()),
            _ => None,
        }
    }
}

/// Enforce the product gate at the wire boundary, including direct requests.
/// Item pagination is prepared for contract testing but has no enable switch.
fn check_product_method(method: &str) -> Result<(), CallError> {
    if method == "thread/items/list" {
        return Err(CallError::Rejected(
            "Thread item pagination is disabled in this client".into(),
        ));
    }
    Ok(())
}

fn read_jsonl_frame<R: BufRead>(
    reader: &mut R,
    frame: &mut Vec<u8>,
    max_bytes: usize,
) -> io::Result<usize> {
    frame.clear();
    let read = reader
        .take(max_bytes.saturating_add(1) as u64)
        .read_until(b'\n', frame)?;
    if frame.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("App Server JSONL frame exceeds the {max_bytes}-byte limit"),
        ));
    }
    Ok(read)
}

#[cfg(test)]
mod bounded_frame_tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    #[test]
    fn jsonl_reader_accepts_a_complete_bounded_frame() {
        let mut reader = BufReader::new(Cursor::new(b"{\"id\":1}\nnext"));
        let mut frame = Vec::new();
        assert_eq!(read_jsonl_frame(&mut reader, &mut frame, 16).unwrap(), 9);
        assert_eq!(frame, b"{\"id\":1}\n");
    }

    #[test]
    fn jsonl_reader_rejects_a_frame_before_it_can_grow_past_the_limit() {
        let mut reader = BufReader::new(Cursor::new(b"123456789\n"));
        let mut frame = Vec::new();
        let error = read_jsonl_frame(&mut reader, &mut frame, 8).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(frame.len(), 9);
    }

    #[test]
    fn production_frame_limit_remains_explicit_and_bounded() {
        assert_eq!(MAX_JSONL_FRAME_BYTES, 64 * 1024 * 1024);
    }
}

fn receive(core: &Core, incoming: Incoming) -> Result<(), String> {
    let event = {
        let mut state = core
            .state
            .lock()
            .map_err(|_| "App Server state is unavailable")?;
        if state.session.phase() == Phase::Closed {
            return Ok(());
        }
        match incoming {
            Incoming::Response { id, result } => {
                let (method, initialized) = state.session.resolve(&id, result.is_ok())?;
                if let Some(frame) = initialized {
                    state.write(&frame)?;
                }
                if method == "initialize" {
                    match result {
                        Ok(initialization) => Some(Event::Ready { initialization }),
                        Err(error) => {
                            state.input.take();
                            Some(Event::Closed {
                                reason: format!("Codex initialization failed: {error}"),
                            })
                        }
                    }
                } else {
                    if let Some(reply) = state.waiters.remove(&id) {
                        let _ = reply.send(result.map_err(CallError::Rpc));
                    }
                    None
                }
            }
            Incoming::Request { id, method, params } => {
                let key = state.session.receive_request(id)?;
                Some(Event::ServerRequest {
                    key,
                    method,
                    params,
                })
            }
            Incoming::Notification { method, params } => {
                if method == "serverRequest/resolved"
                    && let Some(id) = params.get("requestId")
                {
                    let id = serde_json::from_value(id.clone())
                        .map_err(|_| "Invalid resolved server request ID")?;
                    state.session.dismiss(&id);
                }
                Some(Event::Notification { method, params })
            }
        }
    };
    if let Some(event) = event {
        (core.emit)(event);
    }
    Ok(())
}

fn fail(core: &Core, reason: String) {
    let changed = if let Ok(mut state) = core.state.lock() {
        let changed = state.session.phase() != Phase::Closed;
        state.session.close();
        state.input.take();
        for (_, waiter) in state.waiters.drain() {
            let _ = waiter.send(Err(CallError::Disconnected {
                reason: reason.clone(),
                delivery_unknown: true,
            }));
        }
        changed
    } else {
        false
    };
    if changed {
        (core.emit)(Event::Closed { reason });
    }
}

fn spawn_process(
    mut command: Command,
    prepare_hidden_console: bool,
) -> Result<SpawnedProcess, String> {
    #[cfg(windows)]
    if prepare_hidden_console {
        let mut spawned = crate::windows_process::spawn_hidden(&command)?;
        let job = match own_process_tree(&spawned.child) {
            Ok(job) => job,
            Err(error) => {
                let _ = spawned.child.kill();
                let _ = spawned.child.wait();
                return Err(error);
            }
        };
        if let Err(error) = spawned.child.resume() {
            drop(job);
            let _ = spawned.child.kill();
            let _ = spawned.child.wait();
            return Err(error.to_string());
        }
        return Ok(SpawnedProcess {
            child: OwnedChild::Hidden(spawned.child),
            input: Box::new(spawned.input),
            output: Box::new(spawned.output),
            errors: Box::new(spawned.errors),
            job,
        });
    }

    #[cfg(not(windows))]
    let _ = prepare_hidden_console;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not start Codex App Server: {error}"))?;
    #[cfg(windows)]
    let job = match own_process_tree(&child) {
        Ok(job) => job,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let input = child.stdin.take().expect("piped stdin");
    let output = child.stdout.take().expect("piped stdout");
    let errors = child.stderr.take().expect("piped stderr");
    Ok(SpawnedProcess {
        child: OwnedChild::Standard(child),
        input: Box::new(input),
        output: Box::new(output),
        errors: Box::new(errors),
        #[cfg(windows)]
        job,
    })
}

#[cfg(windows)]
fn own_process_tree(
    child: &impl std::os::windows::io::AsRawHandle,
) -> Result<std::os::windows::io::OwnedHandle, String> {
    use std::{
        mem::size_of,
        os::windows::io::{FromRawHandle, OwnedHandle},
    };
    use windows::Win32::{
        Foundation::HANDLE,
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        },
    };
    // The owned kernel handle is Send+Sync. Closing it terminates this server's
    // children too; no process-name scanning or account-wide process termination.
    unsafe {
        let raw = CreateJobObjectW(None, None)
            .map_err(|e| format!("Could not create Codex process ownership: {e}"))?;
        let job = OwnedHandle::from_raw_handle(raw.0);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            raw,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as _,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .and_then(|()| AssignProcessToJobObject(raw, HANDLE(child.as_raw_handle())))
        .map_err(|e| format!("Could not contain Codex child processes: {e}"))?;
        Ok(job)
    }
}

#[cfg(all(test, windows))]
mod windowless_process_tests {
    use super::*;
    use std::os::windows::process::CommandExt;

    const CHILD_PROBE: &str = "CENTRAL_AGENT_HIDDEN_CONSOLE_TEST_CHILD";
    const TEST_NAME: &str =
        "transport::windowless_process_tests::app_server_process_tree_stays_windowless";

    #[test]
    fn app_server_process_tree_stays_windowless() {
        if std::env::var_os(CHILD_PROBE).is_none() {
            let mut probe = Command::new(std::env::current_exe().unwrap());
            probe
                .args(["--exact", TEST_NAME, "--nocapture"])
                .env(CHILD_PROBE, "1")
                .creation_flags(0x0000_0008);
            let output = probe.output().unwrap();
            assert!(
                output.status.success(),
                "windowless subprocess failed:\n{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let powershell = std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let mut command = Command::new(powershell);
        assert!(
            crate::runtime::prepare_process_tree_window(&mut command),
            "the detached probe unexpectedly inherited a console"
        );
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                r#"
                    Add-Type 'using System; using System.Runtime.InteropServices; public static class SupervisorConsoleProbe { [DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow(); [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window); }';
                    $window = [SupervisorConsoleProbe]::GetConsoleWindow();
                    $childScript = @'
Add-Type 'using System; using System.Runtime.InteropServices; public static class SupervisorChildConsoleProbe { [DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow(); [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window); }';
$childWindow = [SupervisorChildConsoleProbe]::GetConsoleWindow();
[Console]::Out.Write("{0}:{1}", $childWindow.ToInt64(), [SupervisorChildConsoleProbe]::IsWindowVisible($childWindow));
'@;
                    $encodedChild = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($childScript));
                    $child = & "$env:WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -EncodedCommand $encodedChild;
                    [Console]::Out.Write("{0}:{1}|{2}|{3}", $window.ToInt64(), [SupervisorConsoleProbe]::IsWindowVisible($window), $child, $env:SUPERVISOR_HIDDEN_PROBE);
                "#,
            ])
            .env("SUPERVISOR_HIDDEN_PROBE", "profile with space");
        let mut spawned = crate::windows_process::spawn_hidden(&command).unwrap();
        let job = own_process_tree(&spawned.child).unwrap();
        spawned.child.resume().unwrap();
        drop(spawned.input);
        let code = spawned.child.wait().unwrap();
        let mut stdout = String::new();
        let mut stderr = String::new();
        spawned.output.read_to_string(&mut stdout).unwrap();
        spawned.errors.read_to_string(&mut stderr).unwrap();
        drop(job);
        assert_eq!(code, 0, "hidden PowerShell probe failed: {stderr}");
        let parts = stdout.trim().split('|').collect::<Vec<_>>();
        assert_eq!(parts.len(), 3, "unexpected console probe output: {stdout}");
        let (server_handle, server_visible) = parts[0]
            .split_once(':')
            .unwrap_or_else(|| panic!("invalid server console state: {stdout:?}"));
        let (child_handle, child_visible) = parts[1]
            .split_once(':')
            .unwrap_or_else(|| panic!("invalid child console state: {stdout:?}; {stderr:?}"));
        assert_eq!(server_handle, "0", "the server must not own a console");
        assert_eq!(
            child_handle, "0",
            "child tools must remain without a console"
        );
        assert_eq!(server_visible, "False", "the server must remain windowless");
        assert_eq!(child_visible, "False", "child tools must remain windowless");
        assert_eq!(
            parts[2], "profile with space",
            "the child environment changed"
        );
    }
}

#[cfg(test)]
mod native_config_reload_tests;
#[cfg(test)]
mod native_elicitation_tests;
#[cfg(all(test, windows))]
mod native_exec_policy_fixture;
#[cfg(all(test, windows))]
mod native_exec_policy_tests;
#[cfg(test)]
mod native_goal_tests;
#[cfg(test)]
mod native_in_turn_mcp_tests;
#[cfg(test)]
mod native_mcp_configuration_tests;
#[cfg(test)]
mod native_mcp_tests;
#[cfg(all(test, windows))]
mod native_network_policy_tests;
#[cfg(test)]
mod native_oauth_tests;
#[cfg(test)]
mod native_permission_profile_tests;
#[cfg(test)]
mod native_profile_tests;
#[cfg(all(test, windows))]
mod native_review_policy_tests;
#[cfg(test)]
mod native_summary_tests;
#[cfg(test)]
mod native_thread_management_tests;
#[cfg(test)]
mod native_wire_loss_tests;
#[cfg(test)]
mod reload_responses_fixture;
#[cfg(test)]
mod tests;
