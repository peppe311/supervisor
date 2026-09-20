use std::{
    env,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child as ProcessChild, Command, Stdio},
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use directories::UserDirs;
use portable_pty::{Child as PtyChild, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    ssh_runtime::SshLaunchSpec,
    tab_context::{sanitize_terminal_snapshot_text, sanitize_ui_text},
};

const DEFAULT_ROWS: u16 = 24;
const DEFAULT_COLS: u16 = 100;
const SHELL_INTERRUPT_RECOVERY_DELAY: Duration = Duration::from_millis(650);
const MAX_TERMINAL_SESSIONS: usize = 8;
const SSH_CAPABILITY_BEGIN: &str = "__CENTRAL_AGENT_SSH_CAPABILITIES_V2__";
const SSH_CAPABILITY_END: &str = "__CENTRAL_AGENT_SSH_CAPABILITIES_END__";
pub(crate) const SSH_RUNTIME_DISCOVERY_ACTION: &str = "ssh_runtime_discovery";
const SSH_CAPABILITY_TOOLS: &[&str] = &[
    "command", "printf", "test", "pwd", "cd", "ls", "cat", "find", "stat", "grep", "sed", "awk",
    "head", "tail", "cut", "sort", "uniq", "xargs", "readlink", "dirname", "basename", "du", "wc",
    "base64", "tr", "git", "file", "identify", "python3", "python", "perl", "tar", "gzip", "unzip",
    "curl", "wget", "make", "cargo", "rustc", "node", "npm", "docker", "podman", "jq", "tree",
];
#[derive(Debug)]
pub(crate) enum TerminalEvent {
    Output {
        session_id: u64,
        chunk: String,
    },
    CommandFinished {
        session_id: u64,
        request_id: String,
        action: String,
        exit_code: i32,
        output: String,
        cwd: Option<String>,
    },
    CommandFailed {
        session_id: u64,
        request_id: String,
        action: String,
        message: String,
    },
    Exited {
        session_id: u64,
        message: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TerminalPhase {
    Running,
    Busy,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TerminalKind {
    Local,
    Ssh,
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SshCapabilities {
    pub shell: String,
    #[serde(skip_serializing)]
    tool_bits: String,
    pub available_tool_count: usize,
    pub navigation_ready: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalContextSnapshot {
    pub session_id: u64,
    pub label: String,
    pub kind: TerminalKind,
    pub profile_id: Option<String>,
    pub remote_target: Option<String>,
    pub phase: TerminalPhase,
    pub busy: bool,
    pub shell: String,
    pub cwd: String,
    pub status: String,
    pub output: String,
    pub source_output_char_count: usize,
    pub output_revision: u64,
    pub last_exit_code: Option<i32>,
    pub captured_at_ms: u128,
    pub estimated_token_count: usize,
    pub redaction_count: usize,
    pub truncated: bool,
}

impl TerminalContextSnapshot {
    pub(crate) fn output_preview(&self) -> String {
        self.output.clone()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalView<'a> {
    pub sessions: Vec<TerminalSessionView<'a>>,
    pub active_session_id: Option<u64>,
    pub can_open: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalSessionView<'a> {
    pub id: u64,
    pub label: &'a str,
    pub kind: TerminalKind,
    pub profile_id: Option<&'a str>,
    pub remote_target: Option<&'a str>,
    pub agent_allowed: bool,
    pub agent_ready: bool,
    pub ssh_capabilities: Option<&'a SshCapabilities>,
    pub phase: &'a TerminalPhase,
    pub busy: bool,
    pub shell: &'a str,
    pub cwd: String,
    pub status: &'a str,
    pub output: &'a str,
    pub output_revision: u64,
}

struct PendingCommand {
    request_id: String,
    action: String,
    marker: String,
    capture: String,
}

struct PendingLocalAgentCommand {
    request_id: String,
    control: Arc<LocalAgentProcessControl>,
}

struct LocalAgentProcessControl {
    child: Mutex<ProcessChild>,
    #[cfg(windows)]
    job: TerminalWindowsJob,
}

impl LocalAgentProcessControl {
    fn terminate(&self) -> Result<(), String> {
        #[cfg(windows)]
        if self.job.terminate(0xCA11_0001).is_ok() {
            return Ok(());
        }

        self.child
            .lock()
            .map_err(|_| "Local agent process state is unavailable".to_owned())?
            .kill()
            .map_err(|error| format!("Could not interrupt the local agent process: {error}"))
    }
}

type TerminalCallback = dyn Fn(TerminalEvent) + Send + Sync + 'static;

struct TerminalStartupState {
    ready: bool,
    exited: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalCommandProtocol {
    PowerShell,
    Posix,
}

pub(crate) struct TerminalSession {
    id: u64,
    master: Box<dyn MasterPty + Send>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    child: Arc<Mutex<Box<dyn PtyChild + Send + Sync>>>,
    pending: Arc<Mutex<Option<PendingCommand>>>,
    local_pending: Arc<Mutex<Option<PendingLocalAgentCommand>>>,
    startup: Arc<(Mutex<TerminalStartupState>, Condvar)>,
    callback: Arc<TerminalCallback>,
    #[cfg(windows)]
    _job: TerminalWindowsJob,
}

#[cfg(windows)]
struct TerminalWindowsJob(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
unsafe impl Send for TerminalWindowsJob {}

#[cfg(windows)]
unsafe impl Sync for TerminalWindowsJob {}

#[cfg(windows)]
impl TerminalWindowsJob {
    fn attach(process_handle: std::os::windows::io::RawHandle) -> Result<Self, String> {
        use windows::Win32::{
            Foundation::HANDLE,
            System::JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject,
            },
        };

        let handle = unsafe { CreateJobObjectW(None, None) }
            .map_err(|error| format!("Could not create the terminal cleanup job: {error}"))?;
        let job = Self(handle);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        }
        .map_err(|error| format!("Could not configure terminal crash cleanup: {error}"))?;
        unsafe { AssignProcessToJobObject(job.0, HANDLE(process_handle)) }
            .map_err(|error| format!("Could not attach the shell to crash cleanup: {error}"))?;
        Ok(job)
    }

    fn terminate(&self, exit_code: u32) -> windows::core::Result<()> {
        unsafe { windows::Win32::System::JobObjects::TerminateJobObject(self.0, exit_code) }
    }
}

#[cfg(windows)]
impl Drop for TerminalWindowsJob {
    fn drop(&mut self) {
        let _ = unsafe { windows::Win32::Foundation::CloseHandle(self.0) };
    }
}

impl TerminalSession {
    pub(crate) fn start<F>(id: u64, cwd: &Path, callback: F) -> Result<Self, String>
    where
        F: Fn(TerminalEvent) + Send + Sync + 'static,
    {
        let shell = shell_program();
        let arguments = shell_arguments();
        Self::start_program(id, cwd, Path::new(&shell), &arguments, false, callback)
    }

    pub(crate) fn start_ssh<F>(
        id: u64,
        cwd: &Path,
        launch: &SshLaunchSpec,
        callback: F,
    ) -> Result<Self, String>
    where
        F: Fn(TerminalEvent) + Send + Sync + 'static,
    {
        Self::start_program(id, cwd, &launch.program, &launch.arguments, true, callback)
    }

    fn start_program<F>(
        id: u64,
        cwd: &Path,
        program: &Path,
        arguments: &[String],
        ready_immediately: bool,
        callback: F,
    ) -> Result<Self, String>
    where
        F: Fn(TerminalEvent) + Send + Sync + 'static,
    {
        let cwd = validated_cwd(cwd)?;
        let program_label = program.display().to_string();
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: DEFAULT_ROWS,
                cols: DEFAULT_COLS,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("Could not create the local terminal: {error}"))?;

        let mut command = CommandBuilder::new(program);
        for argument in arguments {
            command.arg(argument);
        }
        command.cwd(cwd);
        if let Ok(path) = env::var("PATH") {
            command.env("PATH", path);
        }
        command.env("TERM", "xterm-256color");

        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("Could not start {program_label}: {error}"))?;
        drop(pair.slave);

        #[cfg(windows)]
        let job = match child.as_raw_handle() {
            Some(handle) => match TerminalWindowsJob::attach(handle) {
                Ok(job) => job,
                Err(error) => {
                    let _ = child.kill();
                    return Err(error);
                }
            },
            None => {
                let _ = child.kill();
                return Err(
                    "The shell did not expose a process handle for crash cleanup".to_owned(),
                );
            }
        };

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("Could not read the local terminal: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("Could not write to the local terminal: {error}"))?;

        let writer = Arc::new(Mutex::new(writer));
        let child = Arc::new(Mutex::new(child));
        let pending = Arc::new(Mutex::new(None::<PendingCommand>));
        let local_pending = Arc::new(Mutex::new(None::<PendingLocalAgentCommand>));
        let startup = Arc::new((
            Mutex::new(TerminalStartupState {
                ready: ready_immediately || !cfg!(windows),
                exited: false,
            }),
            Condvar::new(),
        ));
        let reader_pending = Arc::clone(&pending);
        let reader_startup = Arc::clone(&startup);
        let callback: Arc<TerminalCallback> = Arc::new(callback);
        let reader_callback = Arc::clone(&callback);
        let reader_child = Arc::clone(&child);
        let reader_writer = Arc::clone(&writer);

        thread::spawn(move || {
            let mut bytes = [0_u8; 8 * 1024];
            let mut query_tail = String::new();
            let mut startup_ready = ready_immediately || !cfg!(windows);
            loop {
                match reader.read(&mut bytes) {
                    Ok(0) => break,
                    Ok(read) => {
                        let raw = String::from_utf8_lossy(&bytes[..read]);
                        let answered_query =
                            answer_terminal_queries(&raw, &mut query_tail, &reader_writer);
                        if !startup_ready && answered_query {
                            mark_terminal_ready(&reader_startup);
                            startup_ready = true;
                        }
                        let chunk = raw.into_owned();
                        reader_callback(TerminalEvent::Output {
                            session_id: id,
                            chunk: chunk.clone(),
                        });
                        let normalized = normalize_terminal_text(&chunk);
                        if !normalized.is_empty() {
                            inspect_pending_command(
                                id,
                                &normalized,
                                &reader_pending,
                                &reader_callback,
                            );
                        }
                    }
                    Err(error) => {
                        fail_pending_command(
                            id,
                            format!("Terminal output failed: {error}"),
                            &reader_pending,
                            &reader_callback,
                        );
                        break;
                    }
                }
            }

            fail_pending_command(
                id,
                "The shell exited before the command completed".to_owned(),
                &reader_pending,
                &reader_callback,
            );
            mark_terminal_exited(&reader_startup);
            let message = reader_child
                .lock()
                .ok()
                .and_then(|mut child| child.try_wait().ok().flatten())
                .map(|status| format!("Shell exited with {status}"))
                .unwrap_or_else(|| "Shell stopped".to_owned());
            reader_callback(TerminalEvent::Exited {
                session_id: id,
                message,
            });
        });

        Ok(Self {
            id,
            master: pair.master,
            writer,
            child,
            pending,
            local_pending,
            startup,
            callback,
            #[cfg(windows)]
            _job: job,
        })
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn send_data(&self, data: &str) -> Result<(), String> {
        if data.contains('\0') {
            return Err("Terminal input cannot contain a null byte".to_owned());
        }
        self.wait_until_ready()?;
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| "The terminal input stream is unavailable".to_owned())?;
        writer
            .write_all(data.as_bytes())
            .and_then(|_| writer.flush())
            .map_err(|error| format!("Could not write to the terminal: {error}"))
    }

    pub(crate) fn send_control_c(&self) -> Result<(), String> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| "The terminal input stream is unavailable".to_owned())?;
        writer
            .write_all(&[3])
            .and_then(|_| writer.flush())
            .map_err(|error| format!("Could not interrupt the terminal: {error}"))
    }

    pub(crate) fn cancel_agent_command(&self, request_id: &str) -> Result<bool, String> {
        let local_command = {
            let mut pending = self
                .local_pending
                .lock()
                .map_err(|_| "Local agent command state is unavailable".to_owned())?;
            if pending
                .as_ref()
                .is_some_and(|pending| pending.request_id == request_id)
            {
                pending.take()
            } else {
                None
            }
        };
        if let Some(local_command) = local_command {
            if let Err(error) = local_command.control.terminate() {
                if let Ok(mut pending) = self.local_pending.lock() {
                    *pending = Some(local_command);
                }
                return Err(error);
            }
            return Ok(true);
        }

        let cancelled = {
            let mut pending = self
                .pending
                .lock()
                .map_err(|_| "Terminal command state is unavailable".to_owned())?;
            if pending
                .as_ref()
                .is_some_and(|pending| pending.request_id == request_id)
            {
                pending.take()
            } else {
                None
            }
        };
        let Some(cancelled) = cancelled else {
            return Ok(false);
        };
        if let Err(error) = self.send_control_c() {
            if let Ok(mut pending) = self.pending.lock() {
                *pending = Some(cancelled);
            }
            return Err(error);
        }
        // ConPTY and remote interactive shells acknowledge Ctrl+C asynchronously. Give the
        // terminal input loop a brief chance to restore its prompt before a queued fallback or
        // follow-up command is written; otherwise PowerShell can discard its first input line.
        thread::sleep(SHELL_INTERRUPT_RECOVERY_DELAY);
        Ok(true)
    }

    pub(crate) fn interrupt_active_command(&self) -> Result<Option<String>, String> {
        let local_command = self
            .local_pending
            .lock()
            .map_err(|_| "Local agent command state is unavailable".to_owned())?
            .take();
        if let Some(local_command) = local_command {
            let request_id = local_command.request_id.clone();
            if let Err(error) = local_command.control.terminate() {
                if let Ok(mut pending) = self.local_pending.lock() {
                    *pending = Some(local_command);
                }
                return Err(error);
            }
            return Ok(Some(request_id));
        }

        let pending_command = self
            .pending
            .lock()
            .map_err(|_| "Terminal command state is unavailable".to_owned())?
            .take();
        if let Err(error) = self.send_control_c() {
            if let Ok(mut pending) = self.pending.lock() {
                *pending = pending_command;
            }
            return Err(error);
        }
        Ok(pending_command.map(|pending| pending.request_id))
    }

    fn run_local_agent_command(
        &self,
        request_id: String,
        action: &str,
        command: &str,
        cwd: &Path,
    ) -> Result<(), String> {
        self.wait_until_ready()?;
        if command.contains('\0') {
            return Err("Terminal command cannot contain a null byte".to_owned());
        }
        if self
            .pending
            .lock()
            .map_err(|_| "Terminal command state is unavailable".to_owned())?
            .is_some()
            || self
                .local_pending
                .lock()
                .map_err(|_| "Local agent command state is unavailable".to_owned())?
                .is_some()
        {
            return Err("Another terminal command is still running".to_owned());
        }

        let cwd = validated_cwd(cwd)?;
        let cwd_record = env::temp_dir().join(format!(
            "central-agent-terminal-cwd-{}-{}.txt",
            self.id,
            Uuid::new_v4().simple()
        ));
        let mut child = local_agent_process_command(command, &cwd, &cwd_record)
            .spawn()
            .map_err(|error| format!("Could not start the local agent process: {error}"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Local agent process stdout was not captured".to_owned())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Local agent process stderr was not captured".to_owned())?;

        #[cfg(windows)]
        let job = {
            use std::os::windows::io::AsRawHandle;
            let job = match TerminalWindowsJob::attach(AsRawHandle::as_raw_handle(&child)) {
                Ok(job) => job,
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error);
                }
            };
            if let Err(error) = resume_terminal_process_threads(child.id()) {
                let _ = job.terminate(0xCA11_0002);
                let _ = child.wait();
                return Err(error);
            }
            job
        };

        let control = Arc::new(LocalAgentProcessControl {
            child: Mutex::new(child),
            #[cfg(windows)]
            job,
        });
        {
            let mut pending = self
                .local_pending
                .lock()
                .map_err(|_| "Local agent command state is unavailable".to_owned())?;
            *pending = Some(PendingLocalAgentCommand {
                request_id: request_id.clone(),
                control: Arc::clone(&control),
            });
        }

        let visible_command = command.replace("\r\n", "\n").replace('\r', "\n");
        (self.callback)(TerminalEvent::Output {
            session_id: self.id,
            chunk: format!(
                "\r\nPS {}> {}\r\n",
                cwd.display(),
                visible_command.replace('\n', "\r\n")
            ),
        });

        let capture = Arc::new(Mutex::new(String::new()));
        let stdout_reader = spawn_local_agent_output_reader(
            self.id,
            stdout,
            Arc::clone(&capture),
            Arc::clone(&self.callback),
        );
        let stderr_reader = spawn_local_agent_output_reader(
            self.id,
            stderr,
            Arc::clone(&capture),
            Arc::clone(&self.callback),
        );
        let session_id = self.id;
        let action = action.to_owned();
        let pending = Arc::clone(&self.local_pending);
        let callback = Arc::clone(&self.callback);
        thread::spawn(move || {
            let status = wait_for_local_agent_process(&control);
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            let output = capture
                .lock()
                .map(|capture| clean_local_agent_output(&capture))
                .unwrap_or_else(|_| "Local command output became unavailable".to_owned());
            let cwd = std::fs::read_to_string(&cwd_record)
                .ok()
                .map(|cwd| cwd.trim().to_owned())
                .filter(|cwd| !cwd.is_empty());
            let _ = std::fs::remove_file(&cwd_record);
            let should_report = pending
                .lock()
                .ok()
                .and_then(|mut pending| {
                    if pending
                        .as_ref()
                        .is_some_and(|pending| pending.request_id == request_id)
                    {
                        pending.take()
                    } else {
                        None
                    }
                })
                .is_some();
            if !should_report {
                return;
            }
            match status {
                Ok(exit_code) => callback(TerminalEvent::CommandFinished {
                    session_id,
                    request_id,
                    action,
                    exit_code,
                    output,
                    cwd,
                }),
                Err(message) => callback(TerminalEvent::CommandFailed {
                    session_id,
                    request_id,
                    action,
                    message,
                }),
            }
        });
        Ok(())
    }

    fn run_agent_command(
        &self,
        request_id: String,
        action: &str,
        command: &str,
        nonce: &str,
        protocol: TerminalCommandProtocol,
    ) -> Result<(), String> {
        self.wait_until_ready()?;
        let marker = format!("__CENTRAL_AGENT_DONE_{nonce}__");
        {
            let mut pending = self
                .pending
                .lock()
                .map_err(|_| "Terminal command state is unavailable".to_owned())?;
            if pending.is_some() {
                return Err("Another terminal command is still running".to_owned());
            }
            *pending = Some(PendingCommand {
                request_id,
                action: action.to_owned(),
                marker: marker.clone(),
                capture: String::new(),
            });
        }

        let payload = agent_command_payload(protocol, command, nonce);

        let write_result = (|| {
            let mut writer = self
                .writer
                .lock()
                .map_err(|_| "The terminal input stream is unavailable".to_owned())?;
            writer
                .write_all(payload.as_bytes())
                .and_then(|_| writer.flush())
                .map_err(|error| format!("Could not run the terminal command: {error}"))
        })();

        if write_result.is_err()
            && let Ok(mut pending) = self.pending.lock()
        {
            *pending = None;
        }
        write_result
    }

    pub(crate) fn resize(&self, rows: u16, cols: u16) -> Result<(), String> {
        self.master
            .resize(PtySize {
                rows: rows.clamp(4, 200),
                cols: cols.clamp(20, 400),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("Could not resize the terminal: {error}"))
    }

    fn wait_until_ready(&self) -> Result<(), String> {
        let (state, ready) = &*self.startup;
        let state = state
            .lock()
            .map_err(|_| "The terminal startup state is unavailable".to_owned())?;
        let state = ready
            .wait_while(state, |state| !state.ready && !state.exited)
            .map_err(|_| "The terminal startup state is unavailable".to_owned())?;
        if state.exited {
            Err("The shell exited before it was ready".to_owned())
        } else {
            Ok(())
        }
    }

    pub(crate) fn stop(&mut self) -> Result<(), String> {
        if let Some(command) = self
            .local_pending
            .lock()
            .ok()
            .and_then(|mut pending| pending.take())
        {
            let _ = command.control.terminate();
        }
        self.child
            .lock()
            .map_err(|_| "The terminal process is unavailable".to_owned())?
            .kill()
            .map_err(|error| format!("Could not stop the shell: {error}"))
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if let Some(command) = self
            .local_pending
            .lock()
            .ok()
            .and_then(|mut pending| pending.take())
        {
            let _ = command.control.terminate();
        }
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum TerminalParserMode {
    #[default]
    Normal,
    Escape,
    EscapeIntermediate,
    Csi,
    StringControl,
}

#[derive(Debug)]
struct TerminalTextScreen {
    rows: usize,
    cols: usize,
    lines: Vec<Vec<char>>,
    line: usize,
    col: usize,
    saved_line: usize,
    saved_col: usize,
    mode: TerminalParserMode,
    csi: String,
    string_control_escape: bool,
}

impl Default for TerminalTextScreen {
    fn default() -> Self {
        Self {
            rows: usize::from(DEFAULT_ROWS),
            cols: usize::from(DEFAULT_COLS),
            lines: vec![Vec::new()],
            line: 0,
            col: 0,
            saved_line: 0,
            saved_col: 0,
            mode: TerminalParserMode::Normal,
            csi: String::new(),
            string_control_escape: false,
        }
    }
}

impl TerminalTextScreen {
    fn consume(&mut self, value: &str) {
        for character in value.chars() {
            match self.mode {
                TerminalParserMode::Normal => self.consume_normal(character),
                TerminalParserMode::Escape => self.consume_escape(character),
                TerminalParserMode::EscapeIntermediate => {
                    if ('\u{30}'..='\u{7e}').contains(&character) {
                        self.mode = TerminalParserMode::Normal;
                    } else if !('\u{20}'..='\u{2f}').contains(&character) {
                        self.mode = TerminalParserMode::Normal;
                        self.consume_normal(character);
                    }
                }
                TerminalParserMode::Csi => {
                    if ('@'..='~').contains(&character) {
                        let sequence = std::mem::take(&mut self.csi);
                        self.mode = TerminalParserMode::Normal;
                        self.apply_csi(&sequence, character);
                    } else if self.csi.len() < 128 {
                        self.csi.push(character);
                    } else {
                        self.csi.clear();
                        self.mode = TerminalParserMode::Normal;
                    }
                }
                TerminalParserMode::StringControl => {
                    if character == '\u{7}'
                        || (self.string_control_escape && character == '\\')
                        || character == '\u{9c}'
                    {
                        self.mode = TerminalParserMode::Normal;
                        self.string_control_escape = false;
                    } else {
                        self.string_control_escape = character == '\u{1b}';
                    }
                }
            }
        }
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        self.rows = usize::from(rows.clamp(4, 200));
        self.cols = usize::from(cols.clamp(20, 400));
        self.col = self.col.min(self.cols.saturating_sub(1));
    }

    fn text(&self) -> String {
        let mut rows = self
            .lines
            .iter()
            .map(|line| line.iter().collect::<String>().trim_end().to_owned())
            .collect::<Vec<_>>();
        while rows.last().is_some_and(String::is_empty) {
            rows.pop();
        }
        rows.join("\n")
    }

    fn consume_normal(&mut self, character: char) {
        match character {
            '\u{1b}' => self.mode = TerminalParserMode::Escape,
            '\u{9b}' => {
                self.csi.clear();
                self.mode = TerminalParserMode::Csi;
            }
            '\u{90}' | '\u{9d}' | '\u{9e}' | '\u{9f}' => {
                self.string_control_escape = false;
                self.mode = TerminalParserMode::StringControl;
            }
            '\r' => self.col = 0,
            '\n' => self.new_line(),
            '\u{8}' => self.col = self.col.saturating_sub(1),
            '\t' => {
                let next_stop = ((self.col / 8) + 1) * 8;
                self.col = next_stop.min(self.cols.saturating_sub(1));
            }
            control if control.is_control() => {}
            _ => self.put(character),
        }
    }

    fn consume_escape(&mut self, character: char) {
        match character {
            '[' => {
                self.csi.clear();
                self.mode = TerminalParserMode::Csi;
            }
            ']' | 'P' | '^' | '_' => {
                self.string_control_escape = false;
                self.mode = TerminalParserMode::StringControl;
            }
            '7' => {
                self.save_cursor();
                self.mode = TerminalParserMode::Normal;
            }
            '8' => {
                self.restore_cursor();
                self.mode = TerminalParserMode::Normal;
            }
            'D' | 'E' => {
                self.new_line();
                self.mode = TerminalParserMode::Normal;
            }
            'M' => {
                self.line = self.line.saturating_sub(1);
                self.ensure_line();
                self.mode = TerminalParserMode::Normal;
            }
            'c' => self.reset(),
            intermediate if ('\u{20}'..='\u{2f}').contains(&intermediate) => {
                self.mode = TerminalParserMode::EscapeIntermediate;
            }
            '\u{1b}' => {}
            _ => self.mode = TerminalParserMode::Normal,
        }
    }

    fn apply_csi(&mut self, sequence: &str, final_character: char) {
        let parameters = sequence.trim_start_matches(['?', '>', '<', '=', '!']);
        let parameters = parameters
            .split_once(|character: char| ('\u{20}'..='\u{2f}').contains(&character))
            .map_or(parameters, |(parameters, _)| parameters);
        let values = parameters
            .split(';')
            .map(|value| value.parse::<usize>().unwrap_or(0))
            .collect::<Vec<_>>();
        let first = values.first().copied().unwrap_or(0);
        let amount = first.max(1);

        match final_character {
            'A' => self.line = self.line.saturating_sub(amount),
            'B' | 'e' => {
                self.line = self.line.saturating_add(amount);
                self.ensure_line();
            }
            'C' | 'a' => {
                self.col = self
                    .col
                    .saturating_add(amount)
                    .min(self.cols.saturating_sub(1));
            }
            'D' => self.col = self.col.saturating_sub(amount),
            'E' => {
                self.line = self.line.saturating_add(amount);
                self.col = 0;
                self.ensure_line();
            }
            'F' => {
                self.line = self.line.saturating_sub(amount);
                self.col = 0;
                self.ensure_line();
            }
            'G' | '`' => self.col = amount.saturating_sub(1).min(self.cols - 1),
            'H' | 'f' => {
                let base = self.lines.len().saturating_sub(self.rows);
                self.line = base.saturating_add(first.max(1) - 1);
                self.col = values
                    .get(1)
                    .copied()
                    .unwrap_or(1)
                    .max(1)
                    .saturating_sub(1)
                    .min(self.cols - 1);
                self.ensure_line();
            }
            'd' => {
                let base = self.lines.len().saturating_sub(self.rows);
                self.line = base.saturating_add(amount - 1);
                self.ensure_line();
            }
            'J' => self.erase_display(first),
            'K' => self.erase_line(first),
            's' => self.save_cursor(),
            'u' => self.restore_cursor(),
            'P' => self.delete_characters(amount),
            'X' => self.erase_characters(amount),
            '@' => self.insert_spaces(amount),
            _ => {}
        }
    }

    fn put(&mut self, character: char) {
        let col = self.col;
        let line = self.ensure_line();
        if line.len() < col {
            line.resize(col, ' ');
        }
        if col < line.len() {
            line[col] = character;
        } else {
            line.push(character);
        }
        self.col = self.col.saturating_add(1);
        if self.col >= self.cols {
            self.new_line();
        }
    }

    fn new_line(&mut self) {
        self.line = self.line.saturating_add(1);
        self.col = 0;
        self.ensure_line();
    }

    fn ensure_line(&mut self) -> &mut Vec<char> {
        while self.lines.len() <= self.line {
            self.lines.push(Vec::new());
        }
        &mut self.lines[self.line]
    }

    fn erase_line(&mut self, mode: usize) {
        let col = self.col;
        let line = self.ensure_line();
        match mode {
            1 => {
                if line.len() <= col {
                    line.resize(col + 1, ' ');
                }
                line.iter_mut()
                    .take(col.saturating_add(1))
                    .for_each(|character| *character = ' ');
            }
            2 => line.clear(),
            _ => line.truncate(col),
        }
    }

    fn erase_display(&mut self, mode: usize) {
        match mode {
            2 | 3 => self.clear_screen(),
            1 => {
                let removed = self.line;
                if removed > 0 {
                    self.lines.drain(..removed);
                    self.line = 0;
                    self.saved_line = self.saved_line.saturating_sub(removed);
                }
                self.erase_line(1);
            }
            _ => {
                self.erase_line(0);
                self.lines.truncate(self.line.saturating_add(1));
            }
        }
    }

    fn delete_characters(&mut self, amount: usize) {
        let col = self.col;
        let line = self.ensure_line();
        if col < line.len() {
            let end = col.saturating_add(amount).min(line.len());
            line.drain(col..end);
        }
    }

    fn erase_characters(&mut self, amount: usize) {
        let col = self.col;
        let line = self.ensure_line();
        if line.len() < col.saturating_add(amount) {
            line.resize(col.saturating_add(amount), ' ');
        }
        line.iter_mut()
            .skip(col)
            .take(amount)
            .for_each(|character| *character = ' ');
    }

    fn insert_spaces(&mut self, amount: usize) {
        let col = self.col;
        let cols = self.cols;
        let line = self.ensure_line();
        if line.len() < col {
            line.resize(col, ' ');
        }
        line.splice(col..col, std::iter::repeat_n(' ', amount));
        line.truncate(cols);
    }

    fn save_cursor(&mut self) {
        self.saved_line = self.line;
        self.saved_col = self.col;
    }

    fn restore_cursor(&mut self) {
        self.line = self.saved_line;
        self.col = self.saved_col.min(self.cols.saturating_sub(1));
        self.ensure_line();
    }

    fn reset(&mut self) {
        self.mode = TerminalParserMode::Normal;
        self.csi.clear();
        self.string_control_escape = false;
        self.clear_screen();
    }

    fn clear_screen(&mut self) {
        self.lines.clear();
        self.lines.push(Vec::new());
        self.line = 0;
        self.col = 0;
        self.saved_line = 0;
        self.saved_col = 0;
    }
}

struct ManagedTerminalSession {
    session: TerminalSession,
    label: String,
    kind: TerminalKind,
    profile_id: Option<String>,
    remote_target: Option<String>,
    agent_allowed: bool,
    agent_ready: bool,
    ssh_capabilities: Option<SshCapabilities>,
    command_protocol: TerminalCommandProtocol,
    phase: TerminalPhase,
    shell: String,
    cwd: PathBuf,
    status: String,
    transcript: String,
    context_screen: TerminalTextScreen,
    suppress_context_output: bool,
    busy: bool,
    output_revision: u64,
    last_exit_code: Option<i32>,
}

pub(crate) struct TerminalRuntime {
    sessions: Vec<ManagedTerminalSession>,
    active_session_id: Option<u64>,
    default_cwd: PathBuf,
    next_session_id: u64,
}

impl Default for TerminalRuntime {
    fn default() -> Self {
        Self {
            sessions: Vec::new(),
            active_session_id: None,
            default_cwd: default_cwd(),
            next_session_id: 1,
        }
    }
}

impl TerminalRuntime {
    pub(crate) fn reset_default_cwd(&mut self) {
        self.default_cwd = default_cwd();
    }

    pub(crate) fn is_running(&self) -> bool {
        !self.sessions.is_empty()
    }

    pub(crate) fn is_busy(&self) -> bool {
        self.sessions.iter().any(|session| session.busy)
    }

    pub(crate) fn view(&self) -> TerminalView<'_> {
        TerminalView {
            sessions: self
                .sessions
                .iter()
                .map(|managed| TerminalSessionView {
                    id: managed.session.id(),
                    label: &managed.label,
                    kind: managed.kind,
                    profile_id: managed.profile_id.as_deref(),
                    remote_target: managed.remote_target.as_deref(),
                    agent_allowed: managed.agent_allowed,
                    agent_ready: managed.agent_ready,
                    ssh_capabilities: managed.ssh_capabilities.as_ref(),
                    phase: &managed.phase,
                    busy: managed.busy,
                    shell: &managed.shell,
                    cwd: managed.cwd.display().to_string(),
                    status: &managed.status,
                    output: &managed.transcript,
                    output_revision: managed.output_revision,
                })
                .collect(),
            active_session_id: self.active_session_id,
            can_open: self.sessions.len() < MAX_TERMINAL_SESSIONS,
        }
    }

    pub(crate) fn has_session(&self, session_id: u64) -> bool {
        self.sessions
            .iter()
            .any(|managed| managed.session.id() == session_id)
    }

    pub(crate) fn has_local_session(&self) -> bool {
        self.sessions
            .iter()
            .any(|managed| managed.kind == TerminalKind::Local)
    }

    pub(crate) fn has_ssh_profile_session(&self, profile_id: &str) -> bool {
        self.sessions.iter().any(|managed| {
            managed.kind == TerminalKind::Ssh && managed.profile_id.as_deref() == Some(profile_id)
        })
    }

    pub(crate) fn ssh_profile_session_id(&self, profile_id: &str) -> Option<u64> {
        self.sessions
            .iter()
            .find(|managed| {
                managed.kind == TerminalKind::Ssh
                    && managed.profile_id.as_deref() == Some(profile_id)
            })
            .map(|managed| managed.session.id())
    }

    pub(crate) fn has_agent_ready_ssh_profile(&self, profile_id: &str) -> bool {
        self.sessions.iter().any(|managed| {
            managed.kind == TerminalKind::Ssh
                && managed.profile_id.as_deref() == Some(profile_id)
                && managed.agent_allowed
                && managed.agent_ready
        })
    }

    pub(crate) fn has_agent_ready_ssh_session(&self) -> bool {
        self.sessions.iter().any(|managed| {
            managed.kind == TerminalKind::Ssh && managed.agent_allowed && managed.agent_ready
        })
    }

    pub(crate) fn set_ssh_capabilities(
        &mut self,
        session_id: u64,
        capabilities: SshCapabilities,
    ) -> Result<(), String> {
        let managed = self
            .sessions
            .iter_mut()
            .find(|managed| managed.session.id() == session_id)
            .ok_or_else(|| "The selected shell is not running".to_owned())?;
        if managed.kind != TerminalKind::Ssh {
            return Err("Capabilities can be attached only to an SSH session".to_owned());
        }
        if capabilities.tool_bits.len() != SSH_CAPABILITY_TOOLS.len() {
            return Err("The local SSH capability bitset is incomplete".to_owned());
        }
        let tool_count = capabilities.available_tool_count;
        managed.ssh_capabilities = Some(capabilities);
        managed.status =
            format!("SSH connected; Agent access enabled · {tool_count} tools detected");
        Ok(())
    }

    pub(crate) fn command_action(&self, session_id: u64) -> String {
        let Some(managed) = self
            .sessions
            .iter()
            .find(|managed| managed.session.id() == session_id)
        else {
            return "run_command".to_owned();
        };
        managed
            .session
            .pending
            .lock()
            .ok()
            .and_then(|pending| pending.as_ref().map(|pending| pending.action.clone()))
            .unwrap_or_else(|| match managed.kind {
                TerminalKind::Local => "run_command".to_owned(),
                TerminalKind::Ssh => "ssh_run".to_owned(),
            })
    }

    pub(crate) fn remote_target(&self, session_id: u64) -> Option<&str> {
        self.sessions
            .iter()
            .find(|managed| managed.session.id() == session_id)
            .and_then(|managed| managed.remote_target.as_deref())
    }

    pub(crate) fn session_is_busy(&self, session_id: u64) -> bool {
        self.sessions
            .iter()
            .find(|managed| managed.session.id() == session_id)
            .is_some_and(|managed| managed.busy)
    }

    pub(crate) fn context_output_is_suppressed(&self, session_id: u64) -> bool {
        self.sessions
            .iter()
            .find(|managed| managed.session.id() == session_id)
            .is_some_and(|managed| managed.suppress_context_output)
    }

    pub(crate) fn append_sanitized_context_output(
        &mut self,
        session_id: u64,
        output: &str,
    ) -> Result<(), String> {
        let managed = self
            .sessions
            .iter_mut()
            .find(|managed| managed.session.id() == session_id)
            .ok_or_else(|| "The selected shell is not running".to_owned())?;
        let output = strip_ssh_capability_probe(output);
        if !output.is_empty() && output != "Command completed without output" {
            managed.context_screen.consume(&format!("\r\n{output}\r\n"));
            managed.output_revision = managed.output_revision.saturating_add(1);
        }
        Ok(())
    }

    pub(crate) fn context_snapshot(
        &self,
        session_id: u64,
        captured_at_ms: u128,
    ) -> Result<TerminalContextSnapshot, String> {
        let managed = self
            .sessions
            .iter()
            .find(|managed| managed.session.id() == session_id)
            .ok_or_else(|| "The selected shell is not running".to_owned())?;
        let normalized = clean_terminal_transcript(&managed.context_screen.text());
        let (output, source_output_char_count, output_redactions, output_truncated) =
            sanitize_terminal_snapshot_text(&normalized);
        let cwd_source = managed.cwd.display().to_string();
        let (cwd, cwd_redactions) = sanitize_ui_text(&cwd_source, 512);
        let (label, label_redactions) = sanitize_ui_text(&managed.label, 96);
        let (shell, shell_redactions) = sanitize_ui_text(&managed.shell, 260);
        let (status, status_redactions) = sanitize_ui_text(&managed.status, 180);
        let (remote_target, remote_target_redactions) = match &managed.remote_target {
            Some(target) => {
                let (target, redactions) = sanitize_ui_text(target, 320);
                (Some(target), redactions)
            }
            None => (None, 0),
        };
        let redaction_count = output_redactions
            + cwd_redactions
            + label_redactions
            + shell_redactions
            + status_redactions
            + remote_target_redactions;
        let estimated_token_count = estimate_terminal_context_tokens(&[
            &label,
            &shell,
            &cwd,
            &status,
            remote_target.as_deref().unwrap_or_default(),
            &output,
        ]);
        Ok(TerminalContextSnapshot {
            session_id,
            label,
            kind: managed.kind,
            profile_id: managed.profile_id.clone(),
            remote_target,
            phase: managed.phase,
            busy: managed.busy,
            shell,
            cwd,
            status,
            output,
            source_output_char_count,
            output_revision: managed.output_revision,
            last_exit_code: managed.last_exit_code,
            captured_at_ms,
            estimated_token_count,
            redaction_count,
            truncated: output_truncated,
        })
    }

    pub(crate) fn open<F>(&mut self, callback: F) -> Result<u64, String>
    where
        F: Fn(TerminalEvent) + Send + Sync + 'static,
    {
        self.open_in_directory(&self.default_cwd.clone(), callback)
    }

    pub(crate) fn is_local_session(&self, session_id: u64) -> bool {
        self.sessions.iter().any(|managed| {
            managed.session.id() == session_id && managed.kind == TerminalKind::Local
        })
    }

    pub(crate) fn default_working_directory(&self) -> &Path {
        &self.default_cwd
    }

    pub(crate) fn open_in_directory<F>(
        &mut self,
        directory: &Path,
        callback: F,
    ) -> Result<u64, String>
    where
        F: Fn(TerminalEvent) + Send + Sync + 'static,
    {
        if self.sessions.len() >= MAX_TERMINAL_SESSIONS {
            return Err(format!(
                "At most {MAX_TERMINAL_SESSIONS} terminal sessions can be open"
            ));
        }

        let cwd = validated_cwd(directory)?;
        let session_id = self.next_session_id;
        self.next_session_id = self.next_session_id.saturating_add(1);
        let session = TerminalSession::start(session_id, &cwd, callback)?;
        self.sessions.push(ManagedTerminalSession {
            session,
            label: format!("Shell {session_id}"),
            kind: TerminalKind::Local,
            profile_id: None,
            remote_target: None,
            agent_allowed: false,
            agent_ready: false,
            ssh_capabilities: None,
            command_protocol: local_command_protocol(),
            phase: TerminalPhase::Running,
            shell: shell_program(),
            cwd,
            status: "Interactive shell is running".to_owned(),
            transcript: String::new(),
            context_screen: TerminalTextScreen::default(),
            suppress_context_output: false,
            busy: false,
            output_revision: 0,
            last_exit_code: None,
        });
        self.active_session_id = Some(session_id);
        Ok(session_id)
    }

    pub(crate) fn open_ssh<F>(&mut self, launch: SshLaunchSpec, callback: F) -> Result<u64, String>
    where
        F: Fn(TerminalEvent) + Send + Sync + 'static,
    {
        if self.sessions.len() >= MAX_TERMINAL_SESSIONS {
            return Err(format!(
                "At most {MAX_TERMINAL_SESSIONS} terminal sessions can be open"
            ));
        }

        let cwd = validated_cwd(&self.default_cwd)?;
        let session_id = self.next_session_id;
        self.next_session_id = self.next_session_id.saturating_add(1);
        let session = TerminalSession::start_ssh(session_id, &cwd, &launch, callback)?;
        let shell = format!("OpenSSH · {}", launch.target);
        self.sessions.push(ManagedTerminalSession {
            session,
            label: format!("SSH · {}", launch.profile_name),
            kind: TerminalKind::Ssh,
            profile_id: Some(launch.profile_id),
            remote_target: Some(launch.target),
            agent_allowed: launch.agent_allowed,
            agent_ready: false,
            ssh_capabilities: None,
            command_protocol: TerminalCommandProtocol::Posix,
            phase: TerminalPhase::Running,
            shell,
            cwd: PathBuf::from("Remote"),
            status: "SSH is connecting; authenticate in this visible terminal".to_owned(),
            transcript: String::new(),
            context_screen: TerminalTextScreen::default(),
            suppress_context_output: false,
            busy: false,
            output_revision: 0,
            last_exit_code: None,
        });
        self.active_session_id = Some(session_id);
        Ok(session_id)
    }

    pub(crate) fn ensure_started<F>(&mut self, callback: F) -> Result<u64, String>
    where
        F: Fn(TerminalEvent) + Send + Sync + 'static,
    {
        if let Some(session_id) = self.active_session_id.filter(|id| {
            self.sessions
                .iter()
                .any(|managed| managed.session.id() == *id && managed.kind == TerminalKind::Local)
        }) {
            Ok(session_id)
        } else if let Some(session_id) = self
            .sessions
            .iter()
            .find(|managed| managed.kind == TerminalKind::Local)
            .map(|managed| managed.session.id())
        {
            self.active_session_id = Some(session_id);
            Ok(session_id)
        } else {
            self.open(callback)
        }
    }

    pub(crate) fn activate(&mut self, session_id: u64) -> bool {
        if self
            .sessions
            .iter()
            .any(|managed| managed.session.id() == session_id)
        {
            self.active_session_id = Some(session_id);
            true
        } else {
            false
        }
    }

    pub(crate) fn set_ssh_agent_ready(
        &mut self,
        session_id: u64,
        ready: bool,
    ) -> Result<(), String> {
        let managed = self
            .sessions
            .iter_mut()
            .find(|managed| managed.session.id() == session_id)
            .ok_or_else(|| "The selected shell is not running".to_owned())?;
        if managed.kind != TerminalKind::Ssh {
            return Err("Only SSH sessions can be enabled for remote agent commands".to_owned());
        }
        if ready && !managed.agent_allowed {
            return Err("Enable Agent access on this server profile first".to_owned());
        }
        if managed.busy {
            return Err("Wait for the current remote command to finish".to_owned());
        }
        managed.agent_ready = ready;
        managed.status = if ready {
            managed.ssh_capabilities.as_ref().map_or_else(
                || "SSH connected; Agent access is enabled for this session".to_owned(),
                |capabilities| {
                    format!(
                        "SSH connected; Agent access enabled · {} tools detected",
                        capabilities.available_tool_count
                    )
                },
            )
        } else {
            "Interactive SSH is available; Agent access is disabled".to_owned()
        };
        Ok(())
    }

    pub(crate) fn reorder(&mut self, session_id: u64, before_session_id: Option<u64>) -> bool {
        let Some(source_index) = self
            .sessions
            .iter()
            .position(|managed| managed.session.id() == session_id)
        else {
            return false;
        };

        if before_session_id == Some(session_id) {
            return false;
        }

        let session = self.sessions.remove(source_index);
        let target_index = before_session_id
            .and_then(|target_id| {
                self.sessions
                    .iter()
                    .position(|managed| managed.session.id() == target_id)
            })
            .unwrap_or(self.sessions.len());
        self.sessions.insert(target_index, session);
        true
    }

    pub(crate) fn write(&mut self, session_id: u64, data: &str) -> Result<Option<String>, String> {
        let managed = self
            .sessions
            .iter_mut()
            .find(|managed| managed.session.id() == session_id)
            .ok_or_else(|| "The selected shell is not running".to_owned())?;

        if data.contains('\u{3}') && managed.busy {
            let request_id = managed.session.interrupt_active_command()?;
            managed.busy = false;
            managed.phase = TerminalPhase::Running;
            managed.status = "Agent command interrupted".to_owned();
            return Ok(request_id);
        }

        managed.session.send_data(data)?;
        if matches!(managed.phase, TerminalPhase::Error) {
            managed.phase = TerminalPhase::Running;
        }
        managed.status = match managed.kind {
            TerminalKind::Local => "Interactive shell is running".to_owned(),
            TerminalKind::Ssh if managed.agent_ready => {
                "Interactive SSH is running; Agent access is enabled".to_owned()
            }
            TerminalKind::Ssh => "Interactive SSH is running".to_owned(),
        };
        Ok(None)
    }

    pub(crate) fn resize(&mut self, session_id: u64, rows: u16, cols: u16) -> Result<(), String> {
        let managed = self
            .sessions
            .iter_mut()
            .find(|managed| managed.session.id() == session_id)
            .ok_or_else(|| "The selected shell is not running".to_owned())?;
        managed.session.resize(rows, cols)?;
        managed.context_screen.resize(rows, cols);
        Ok(())
    }

    pub(crate) fn cancel_agent_command(&mut self, request_id: &str) -> Result<bool, String> {
        for managed in &mut self.sessions {
            let cancelled = managed.session.cancel_agent_command(request_id)?;
            if cancelled {
                managed.busy = false;
                managed.phase = TerminalPhase::Running;
                managed.status = "Agent command interrupted".to_owned();
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(crate) fn interrupt_all_agent_commands(&mut self) -> Vec<String> {
        let mut interrupted = Vec::new();
        for managed in &mut self.sessions {
            if !managed.busy {
                continue;
            }
            match managed.session.interrupt_active_command() {
                Ok(Some(request_id)) => interrupted.push(request_id),
                Ok(None) => {}
                Err(_) => continue,
            }
            managed.busy = false;
            managed.phase = TerminalPhase::Running;
            managed.status = "Agent command interrupted by the emergency stop".to_owned();
        }
        interrupted
    }

    pub(crate) fn close(&mut self, session_id: u64) -> Result<Option<String>, String> {
        let index = self
            .sessions
            .iter()
            .position(|managed| managed.session.id() == session_id)
            .ok_or_else(|| "The selected shell is not running".to_owned())?;
        let mut managed = self.sessions.remove(index);
        let interrupted = if managed.busy {
            managed.session.interrupt_active_command()?
        } else {
            None
        };
        let stop_result = managed.session.stop();
        if self.active_session_id == Some(session_id) {
            self.active_session_id = self
                .sessions
                .get(index.min(self.sessions.len().saturating_sub(1)))
                .or_else(|| self.sessions.last())
                .map(|managed| managed.session.id());
        }
        stop_result?;
        Ok(interrupted)
    }

    pub(crate) fn run_agent_command(
        &mut self,
        request_id: String,
        command: &str,
    ) -> Result<u64, String> {
        let session_id = self
            .active_session_id
            .filter(|session_id| {
                self.sessions.iter().any(|managed| {
                    managed.session.id() == *session_id && managed.kind == TerminalKind::Local
                })
            })
            .or_else(|| {
                self.sessions
                    .iter()
                    .find(|managed| managed.kind == TerminalKind::Local)
                    .map(|managed| managed.session.id())
            })
            .ok_or_else(|| "The shell is not running".to_owned())?;
        let cwd = self
            .sessions
            .iter()
            .find(|managed| managed.session.id() == session_id)
            .ok_or("The shell is not running")?
            .cwd
            .clone();
        self.run_agent_command_in_session(session_id, &cwd, request_id, command)
    }

    pub(crate) fn run_agent_command_in_session(
        &mut self,
        session_id: u64,
        cwd: &Path,
        request_id: String,
        command: &str,
    ) -> Result<u64, String> {
        let cwd = validated_cwd(cwd)?;
        let managed = self
            .sessions
            .iter_mut()
            .find(|managed| {
                managed.session.id() == session_id && managed.kind == TerminalKind::Local
            })
            .ok_or_else(|| "The shell is not running".to_owned())?;
        if managed.busy {
            return Err("The active shell is already running an agent command".to_owned());
        }
        managed
            .session
            .run_local_agent_command(request_id, "run_command", command, &cwd)?;
        managed.busy = true;
        managed.phase = TerminalPhase::Busy;
        managed.status = "Agent command is running".to_owned();
        self.active_session_id = Some(session_id);
        Ok(session_id)
    }

    pub(crate) fn run_ssh_agent_command(
        &mut self,
        profile_id: &str,
        request_id: String,
        command: &str,
    ) -> Result<u64, String> {
        let session_id = self
            .active_session_id
            .filter(|session_id| {
                self.sessions.iter().any(|managed| {
                    managed.session.id() == *session_id
                        && managed.kind == TerminalKind::Ssh
                        && managed.profile_id.as_deref() == Some(profile_id)
                        && managed.agent_allowed
                        && managed.agent_ready
                })
            })
            .or_else(|| {
                self.sessions
                    .iter()
                    .find(|managed| {
                        managed.kind == TerminalKind::Ssh
                            && managed.profile_id.as_deref() == Some(profile_id)
                            && managed.agent_allowed
                            && managed.agent_ready
                    })
                    .map(|managed| managed.session.id())
            })
            .ok_or_else(|| {
                "Open and authenticate this SSH profile, then enable agent access for its live session"
                    .to_owned()
            })?;
        let managed = self
            .sessions
            .iter_mut()
            .find(|managed| managed.session.id() == session_id)
            .ok_or_else(|| "The SSH session is not running".to_owned())?;
        if managed.busy {
            return Err("The selected SSH session is already running an agent command".to_owned());
        }
        let nonce = Uuid::new_v4().simple().to_string();
        let include_runtime_discovery = managed.ssh_capabilities.is_none();
        let command = if include_runtime_discovery {
            ssh_command_with_runtime_discovery(command)
        } else {
            command.to_owned()
        };
        managed.session.run_agent_command(
            request_id,
            "ssh_run",
            &command,
            &nonce,
            managed.command_protocol,
        )?;
        managed.suppress_context_output = include_runtime_discovery;
        managed.busy = true;
        managed.phase = TerminalPhase::Busy;
        managed.status = if managed.ssh_capabilities.is_some() {
            "Agent remote command is running".to_owned()
        } else {
            "Preparing the SSH runtime and running the Agent command".to_owned()
        };
        self.active_session_id = Some(session_id);
        Ok(session_id)
    }

    pub(crate) fn start_ssh_runtime_discovery(
        &mut self,
        session_id: u64,
        request_id: String,
    ) -> Result<bool, String> {
        let managed = self
            .sessions
            .iter_mut()
            .find(|managed| managed.session.id() == session_id)
            .ok_or_else(|| "The SSH session is not running".to_owned())?;
        if managed.kind != TerminalKind::Ssh || !managed.agent_allowed || !managed.agent_ready {
            return Err("Enable agent access for this authenticated SSH session first".to_owned());
        }
        if managed.ssh_capabilities.is_some() || managed.busy {
            return Ok(false);
        }
        let nonce = Uuid::new_v4().simple().to_string();
        let command = ssh_capability_probe_command();
        managed.session.run_agent_command(
            request_id,
            SSH_RUNTIME_DISCOVERY_ACTION,
            &command,
            &nonce,
            managed.command_protocol,
        )?;
        managed.suppress_context_output = true;
        managed.busy = true;
        managed.phase = TerminalPhase::Busy;
        managed.status = "Preparing the SSH runtime locally".to_owned();
        self.active_session_id = Some(session_id);
        Ok(true)
    }

    pub(crate) fn handle_output(&mut self, session_id: u64, chunk: &str) -> Option<u64> {
        let managed = self
            .sessions
            .iter_mut()
            .find(|managed| managed.session.id() == session_id)?;
        managed.transcript.push_str(chunk);
        if !managed.suppress_context_output {
            managed.context_screen.consume(chunk);
        }
        managed.output_revision = managed.output_revision.saturating_add(1);
        Some(managed.output_revision)
    }

    pub(crate) fn handle_command_finished(
        &mut self,
        session_id: u64,
        cwd: Option<&str>,
        success: bool,
        exit_code: Option<i32>,
    ) -> bool {
        let Some(managed) = self
            .sessions
            .iter_mut()
            .find(|managed| managed.session.id() == session_id)
        else {
            return false;
        };
        if let Some(cwd) = cwd.filter(|cwd| !cwd.trim().is_empty()) {
            managed.cwd = PathBuf::from(cwd);
        }
        managed.busy = false;
        managed.suppress_context_output = false;
        managed.last_exit_code = exit_code;
        managed.phase = if success {
            TerminalPhase::Running
        } else {
            TerminalPhase::Error
        };
        managed.status = if success && managed.kind == TerminalKind::Ssh {
            "Remote command completed".to_owned()
        } else if success {
            "Command completed".to_owned()
        } else if managed.kind == TerminalKind::Ssh {
            "Remote command failed".to_owned()
        } else {
            "Command failed".to_owned()
        };
        true
    }

    pub(crate) fn handle_exit(&mut self, session_id: u64, message: String) -> bool {
        let Some(index) = self
            .sessions
            .iter()
            .position(|managed| managed.session.id() == session_id)
        else {
            return false;
        };
        let _ = message;
        self.sessions.remove(index);
        if self.active_session_id == Some(session_id) {
            self.active_session_id = self
                .sessions
                .get(index.min(self.sessions.len().saturating_sub(1)))
                .or_else(|| self.sessions.last())
                .map(|managed| managed.session.id());
        }
        true
    }
}

fn inspect_pending_command<F>(
    session_id: u64,
    chunk: &str,
    pending: &Arc<Mutex<Option<PendingCommand>>>,
    callback: &Arc<F>,
) where
    F: Fn(TerminalEvent) + Send + Sync + 'static + ?Sized,
{
    let completion = {
        let Ok(mut guard) = pending.lock() else {
            return;
        };
        let Some(command) = guard.as_mut() else {
            return;
        };
        command.capture.push_str(chunk);
        parse_completion(command).map(|(exit_code, output, cwd)| {
            let request_id = command.request_id.clone();
            let action = command.action.clone();
            (request_id, action, exit_code, output, cwd)
        })
    };

    if let Some((request_id, action, exit_code, output, cwd)) = completion {
        if let Ok(mut guard) = pending.lock() {
            *guard = None;
        }
        callback(TerminalEvent::CommandFinished {
            session_id,
            request_id,
            action,
            exit_code,
            output,
            cwd,
        });
    }
}

fn parse_completion(command: &PendingCommand) -> Option<(i32, String, Option<String>)> {
    let marker_start = command.capture.find(&command.marker)?;
    let marker_tail = &command.capture[marker_start + command.marker.len()..];
    let marker_line = marker_tail
        .trim_start_matches(':')
        .split(['\r', '\n'])
        .next()?;
    let mut fields = marker_line.splitn(2, ':');
    let exit_code = fields.next()?.trim().parse::<i32>().ok()?;
    let cwd = fields
        .next()
        .and_then(|encoded| BASE64.decode(encoded.trim()).ok())
        .and_then(|bytes| String::from_utf8(bytes).ok());
    let output = clean_command_output(&command.capture[..marker_start]);
    Some((exit_code, output, cwd))
}

pub(crate) fn parse_ssh_capability_probe(output: &str) -> Result<SshCapabilities, String> {
    let begin = output.rfind(SSH_CAPABILITY_BEGIN).ok_or_else(|| {
        "The remote capability response did not include its start marker".to_owned()
    })?;
    let frame = &output[begin + SSH_CAPABILITY_BEGIN.len()..];
    let end = frame.find(SSH_CAPABILITY_END).ok_or_else(|| {
        "The remote capability response did not include its end marker".to_owned()
    })?;
    let frame = &frame[..end];
    let bits_start = frame
        .rfind("bits|")
        .ok_or_else(|| "The remote capability response did not include its tool map".to_owned())?
        + "bits|".len();
    let bits = frame[bits_start..]
        .chars()
        .take_while(|character| matches!(character, '0' | '1'))
        .collect::<Vec<_>>();
    if bits.len() != SSH_CAPABILITY_TOOLS.len() {
        return Err(format!(
            "The remote capability response contained {} of {} tool states",
            bits.len(),
            SSH_CAPABILITY_TOOLS.len()
        ));
    }
    let tool_bits = bits.iter().collect::<String>();
    let available_tool_count = bits.iter().filter(|available| **available == '1').count();
    let has = |tool: &str| {
        SSH_CAPABILITY_TOOLS
            .iter()
            .position(|candidate| *candidate == tool)
            .and_then(|index| bits.get(index))
            .is_some_and(|available| *available == '1')
    };
    let navigation_ready = ["command", "printf", "test", "pwd", "cd"]
        .iter()
        .all(|tool| has(tool))
        && (has("ls") || has("find"));
    Ok(SshCapabilities {
        shell: "posix".to_owned(),
        tool_bits,
        available_tool_count,
        navigation_ready,
    })
}

pub(crate) fn extract_ssh_capability_probe(
    output: &str,
) -> Result<Option<(SshCapabilities, String)>, String> {
    if !output.contains(SSH_CAPABILITY_BEGIN) {
        return Ok(None);
    }
    let capabilities = parse_ssh_capability_probe(output)?;
    Ok(Some((capabilities, strip_ssh_capability_probe(output))))
}

pub(crate) fn strip_ssh_capability_probe(output: &str) -> String {
    output
        .lines()
        .filter(|line| {
            let line = line.trim();
            line != SSH_CAPABILITY_BEGIN
                && line != SSH_CAPABILITY_END
                && !line.strip_prefix("bits|").is_some_and(|bits| {
                    bits.len() == SSH_CAPABILITY_TOOLS.len()
                        && bits.bytes().all(|bit| matches!(bit, b'0' | b'1'))
                })
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

fn ssh_capability_probe_command() -> String {
    format!(
        "printf '%s\\n' '{SSH_CAPABILITY_BEGIN}'; __ca_caps=''; for __ca_tool in {}; do if command -v \"$__ca_tool\" >/dev/null 2>&1; then __ca_caps=\"${{__ca_caps}}1\"; else __ca_caps=\"${{__ca_caps}}0\"; fi; done; printf 'bits|%s\\n' \"$__ca_caps\"; printf '%s\\n' '{SSH_CAPABILITY_END}'",
        SSH_CAPABILITY_TOOLS.join(" ")
    )
}

fn ssh_command_with_runtime_discovery(command: &str) -> String {
    format!("{}\n{command}", ssh_capability_probe_command())
}

fn fail_pending_command<F>(
    session_id: u64,
    message: String,
    pending: &Arc<Mutex<Option<PendingCommand>>>,
    callback: &Arc<F>,
) where
    F: Fn(TerminalEvent) + Send + Sync + 'static + ?Sized,
{
    let pending_command = pending.lock().ok().and_then(|mut pending| pending.take());
    if let Some(pending_command) = pending_command {
        callback(TerminalEvent::CommandFailed {
            session_id,
            request_id: pending_command.request_id,
            action: pending_command.action,
            message,
        });
    }
}

fn clean_command_output(value: &str) -> String {
    let normalized = normalize_terminal_text(value);
    let output = filter_terminal_protocol_lines(&normalized);
    if output.is_empty() {
        "Command completed without output".to_owned()
    } else {
        output
    }
}

fn clean_local_agent_output(value: &str) -> String {
    let output = normalize_terminal_text(value).trim().to_owned();
    if output.is_empty() {
        "Command completed without output".to_owned()
    } else {
        output
    }
}

fn spawn_local_agent_output_reader<R: Read + Send + 'static>(
    session_id: u64,
    mut reader: R,
    capture: Arc<Mutex<String>>,
    callback: Arc<TerminalCallback>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut bytes = [0_u8; 16 * 1024];
        loop {
            let read = match reader.read(&mut bytes) {
                Ok(0) => break,
                Ok(read) => read,
                Err(_) => break,
            };
            let chunk = String::from_utf8_lossy(&bytes[..read]).into_owned();
            if let Ok(mut output) = capture.lock() {
                output.push_str(&chunk);
            }
            callback(TerminalEvent::Output { session_id, chunk });
        }
    })
}

fn wait_for_local_agent_process(control: &LocalAgentProcessControl) -> Result<i32, String> {
    loop {
        let status = control
            .child
            .lock()
            .map_err(|_| "Local agent process state is unavailable".to_owned())?
            .try_wait()
            .map_err(|error| format!("Could not read local agent process status: {error}"))?;
        if let Some(status) = status {
            return Ok(status.code().unwrap_or(-1));
        }
        thread::sleep(Duration::from_millis(40));
    }
}

fn local_agent_process_command(command: &str, cwd: &Path, cwd_record: &Path) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_SUSPENDED: u32 = 0x0000_0004;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let encoded_command = BASE64.encode(command.as_bytes());
        let encoded_cwd_record = BASE64.encode(cwd_record.to_string_lossy().as_bytes());
        let script = format!(
            "$__ca_source=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{encoded_command}')); $__ca_code=0; try {{ $global:LASTEXITCODE=0; & ([ScriptBlock]::Create($__ca_source)); $__ca_ok=$?; $__ca_native=[int]$LASTEXITCODE; $__ca_code=if($__ca_native -ne 0){{$__ca_native}}elseif($__ca_ok){{0}}else{{1}} }} catch {{ [Console]::Error.WriteLine($_.ToString()); $__ca_code=1 }} finally {{ try {{ $__ca_path=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{encoded_cwd_record}')); [IO.File]::WriteAllText($__ca_path,(Get-Location).Path,(New-Object Text.UTF8Encoding($false))) }} catch {{}} }}; exit $__ca_code"
        );
        let mut process = Command::new("powershell.exe");
        process
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        process
    }
    #[cfg(not(windows))]
    {
        let script = "trap '__ca_code=$?; pwd > \"$CENTRAL_AGENT_CWD_RECORD\"; trap - EXIT; exit $__ca_code' EXIT; eval \"$CENTRAL_AGENT_COMMAND\"";
        let mut process = Command::new("sh");
        process
            .args(["-lc", script])
            .env("CENTRAL_AGENT_COMMAND", command)
            .env("CENTRAL_AGENT_CWD_RECORD", cwd_record)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        process
    }
}

#[cfg(windows)]
fn resume_terminal_process_threads(pid: u32) -> Result<(), String> {
    use windows::Win32::{
        Foundation::CloseHandle,
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First,
                Thread32Next,
            },
            Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
        },
    };

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }
        .map_err(|error| format!("Could not inspect the suspended agent process: {error}"))?;
    let result = (|| {
        let mut entry = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        unsafe { Thread32First(snapshot, &mut entry) }
            .map_err(|error| format!("Could not find the suspended agent thread: {error}"))?;
        let mut resumed = 0usize;
        loop {
            if entry.th32OwnerProcessID == pid {
                let thread =
                    unsafe { OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID) }
                        .map_err(|error| {
                            format!("Could not open the suspended agent thread: {error}")
                        })?;
                let resume_result = unsafe { ResumeThread(thread) };
                let _ = unsafe { CloseHandle(thread) };
                if resume_result == u32::MAX {
                    return Err("Could not resume the local agent process thread".to_owned());
                }
                resumed = resumed.saturating_add(1);
            }
            if unsafe { Thread32Next(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
        if resumed == 0 {
            return Err("The local agent process had no resumable thread".to_owned());
        }
        Ok(())
    })();
    let _ = unsafe { CloseHandle(snapshot) };
    result
}

fn clean_terminal_transcript(value: &str) -> String {
    let normalized = normalize_terminal_text(value);
    strip_ssh_capability_probe(&filter_terminal_protocol_lines(&normalized))
}

fn filter_terminal_protocol_lines(value: &str) -> String {
    let mut output = String::new();
    for line in value.lines() {
        if line.contains("$global:LASTEXITCODE=0")
            || line.contains("$__ca_")
            || line.contains("__ca_code=")
            || line.contains("__ca_pwd=")
            || line.contains("__ca_marker=")
            || (line.contains("__CENTRAL_") && line.contains("AGENT_DONE_"))
        {
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    output.trim().to_owned()
}

fn normalize_terminal_text(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            match chars.peek().copied() {
                Some('[') => {
                    chars.next();
                    for next in chars.by_ref() {
                        if ('@'..='~').contains(&next) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    let mut escaped = false;
                    for next in chars.by_ref() {
                        if next == '\u{7}' || (escaped && next == '\\') {
                            break;
                        }
                        escaped = next == '\u{1b}';
                    }
                }
                Some(_) => {
                    chars.next();
                }
                None => {}
            }
            continue;
        }
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    continue;
                }
                output.push('\n');
            }
            '\u{8}' => {
                output.pop();
            }
            '\n' | '\t' => output.push(ch),
            control if control.is_control() => {}
            _ => output.push(ch),
        }
    }
    output
}

fn answer_terminal_queries(
    chunk: &str,
    tail: &mut String,
    writer: &Arc<Mutex<Box<dyn Write + Send>>>,
) -> bool {
    let combined = format!("{tail}{chunk}");
    let old_tail_len = tail.len();
    let query_count = ["\u{1b}[6n", "\u{1b}[?6n"]
        .into_iter()
        .map(|query| {
            combined
                .match_indices(query)
                .filter(|(start, _)| start + query.len() > old_tail_len)
                .count()
        })
        .sum::<usize>();
    let answered = query_count > 0
        && writer.lock().is_ok_and(|mut writer| {
            (0..query_count)
                .try_for_each(|_| writer.write_all(b"\x1b[1;1R"))
                .and_then(|_| writer.flush())
                .is_ok()
        });
    const QUERY_TAIL_CHARS: usize = 4;
    *tail = combined
        .chars()
        .rev()
        .take(QUERY_TAIL_CHARS)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    answered
}

fn mark_terminal_ready(startup: &Arc<(Mutex<TerminalStartupState>, Condvar)>) {
    let (state, ready) = &**startup;
    if let Ok(mut state) = state.lock() {
        state.ready = true;
        ready.notify_all();
    }
}

fn mark_terminal_exited(startup: &Arc<(Mutex<TerminalStartupState>, Condvar)>) {
    let (state, ready) = &**startup;
    if let Ok(mut state) = state.lock() {
        state.exited = true;
        ready.notify_all();
    }
}

fn estimate_terminal_context_tokens(parts: &[&str]) -> usize {
    let character_count = parts.iter().map(|part| part.chars().count()).sum::<usize>();
    48 + character_count.div_ceil(4)
}

fn validated_cwd(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map_err(|error| format!("Current directory is unavailable: {error}"))?
            .join(path)
    };
    if !absolute.is_dir() {
        return Err(format!(
            "Terminal directory does not exist: {}",
            absolute.display()
        ));
    }
    Ok(absolute)
}

fn default_cwd() -> PathBuf {
    UserDirs::new()
        .and_then(|dirs| dirs.document_dir().map(Path::to_path_buf))
        .filter(|path| path.is_dir())
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn shell_program() -> String {
    if cfg!(windows) {
        "powershell.exe".to_owned()
    } else {
        env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned())
    }
}

fn shell_arguments() -> Vec<String> {
    if cfg!(windows) {
        vec![
            "-NoLogo".to_owned(),
            "-NoProfile".to_owned(),
            "-NoExit".to_owned(),
        ]
    } else {
        vec!["-i".to_owned()]
    }
}

fn local_command_protocol() -> TerminalCommandProtocol {
    if cfg!(windows) {
        TerminalCommandProtocol::PowerShell
    } else {
        TerminalCommandProtocol::Posix
    }
}

fn agent_command_payload(protocol: TerminalCommandProtocol, command: &str, nonce: &str) -> String {
    let command = command.replace("\r\n", "\n").replace('\r', "\n");
    match protocol {
        TerminalCommandProtocol::PowerShell => {
            let command = command.replace('\n', "\r");
            let marker_command = format!(
                "$__ca_ok=$?; $__ca_code=if($LASTEXITCODE -ne 0){{[int]$LASTEXITCODE}}elseif($__ca_ok){{0}}else{{1}}; $__ca_pwd=[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes((Get-Location).Path)); $__ca_marker='__CENTRAL_'+'AGENT_DONE_'+'{nonce}'+'__'; [Console]::Out.WriteLine($__ca_marker+':'+$__ca_code+':'+$__ca_pwd)"
            );
            format!("$global:LASTEXITCODE=0\r{command}\r{marker_command}\r")
        }
        TerminalCommandProtocol::Posix => {
            let marker_command = format!(
                "__ca_code=$?; __ca_pwd=$(printf '%s' \"$PWD\" | base64 | tr -d '\\r\\n'); __ca_marker='__CENTRAL_'\"AGENT_DONE_\"'{nonce}'\"__\"; printf '\\n%s:%s:%s\\n' \"$__ca_marker\" \"$__ca_code\" \"$__ca_pwd\""
            );
            format!("{command}\n{marker_command}\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    use std::{
        sync::{OnceLock, mpsc},
        time::{Duration, Instant},
    };

    #[cfg(windows)]
    fn windows_pty_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
        GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[derive(Clone)]
    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn strips_terminal_control_sequences() {
        assert_eq!(
            normalize_terminal_text("\u{1b}[31mred\u{1b}[0m\r\nok"),
            "red\nok"
        );
    }

    #[test]
    fn terminal_screen_applies_backspace_edits() {
        let mut screen = TerminalTextScreen::default();
        screen.consume("PS C:\\work> Get-Locatiom");
        screen.consume("\u{8} \u{8}n");

        assert_eq!(screen.text(), "PS C:\\work> Get-Location");
    }

    #[test]
    fn terminal_screen_clears_the_remainder_of_a_redrawn_line() {
        let mut screen = TerminalTextScreen::default();
        screen.consume("PS C:\\work> cargo --version");
        screen.consume("\rPS C:\\work> cargo --versio\u{1b}[K");

        assert_eq!(screen.text(), "PS C:\\work> cargo --versio");
    }

    #[test]
    fn terminal_screen_applies_delete_and_split_cursor_sequences() {
        let mut screen = TerminalTextScreen::default();
        screen.consume("PS> abcd\u{1b}[");
        screen.consume("2D\u{1b}[");
        screen.consume("P");

        assert_eq!(screen.text(), "PS> abd");
    }

    #[test]
    fn terminal_screen_ignores_split_osc_titles() {
        let mut screen = TerminalTextScreen::default();
        screen.consume("before\u{1b}]0;private window");
        screen.consume(" title\u{1b}\\after");

        assert_eq!(screen.text(), "beforeafter");
    }

    #[test]
    fn parses_completion_marker_and_working_directory() {
        let cwd = BASE64.encode("C:\\work");
        let command = PendingCommand {
            request_id: "req-1".to_owned(),
            action: "run_command".to_owned(),
            marker: "__CENTRAL_AGENT_DONE_7__".to_owned(),
            capture: format!("echo hi\r\nhi\r\n__CENTRAL_AGENT_DONE_7__:0:{cwd}\r\n"),
        };
        let (code, output, parsed_cwd) = parse_completion(&command).unwrap();
        assert_eq!(code, 0);
        assert!(output.contains("hi"));
        assert_eq!(parsed_cwd.as_deref(), Some("C:\\work"));
    }

    #[test]
    fn command_capture_preserves_output_larger_than_the_old_limit() {
        let cwd = BASE64.encode("/home/central-agent");
        let payload = format!("BEGIN-CAPTURE\n{}\nEND-CAPTURE", "x".repeat(150_000));
        let command = PendingCommand {
            request_id: "req-large".to_owned(),
            action: "run_command".to_owned(),
            marker: "__CENTRAL_AGENT_DONE_LARGE__".to_owned(),
            capture: format!("{payload}\n__CENTRAL_AGENT_DONE_LARGE__:0:{cwd}\n"),
        };

        let (code, output, parsed_cwd) = parse_completion(&command).unwrap();
        assert_eq!(code, 0);
        assert!(output.starts_with("BEGIN-CAPTURE"));
        assert!(output.ends_with("END-CAPTURE"));
        assert!(output.chars().count() > 150_000);
        assert_eq!(parsed_cwd.as_deref(), Some("/home/central-agent"));
    }

    #[test]
    fn validates_existing_directory() {
        let cwd = env::current_dir().unwrap();
        assert_eq!(validated_cwd(&cwd).unwrap(), cwd);
    }

    #[test]
    fn protocol_lines_are_not_returned_to_the_model() {
        let output = clean_command_output(
            "PS> Get-Location\r\nC:\\work\r\nPS> $__ca_marker='__CENTRAL_'+'AGENT_DONE_'\r\n",
        );
        assert!(output.contains("C:\\work"));
        assert!(!output.contains("$__ca_marker"));
    }

    #[test]
    fn posix_agent_payload_does_not_echo_the_complete_marker() {
        let payload =
            agent_command_payload(TerminalCommandProtocol::Posix, "printf 'hello'", "abc123");

        assert!(payload.starts_with("printf 'hello'\n"));
        assert!(payload.contains("__ca_code=$?"));
        assert!(payload.contains("base64"));
        assert!(!payload.contains("__CENTRAL_AGENT_DONE_abc123__"));
    }

    #[test]
    fn parses_a_bounded_ssh_capability_profile() {
        let bits = SSH_CAPABILITY_TOOLS
            .iter()
            .map(|tool| if *tool == "file" { '0' } else { '1' })
            .collect::<String>();
        let output = format!(
            "wrapped prompt noise {SSH_CAPABILITY_BEGIN} ignored\n{SSH_CAPABILITY_BEGIN}\nbits|{bits}\n{SSH_CAPABILITY_END}\nshell prompt"
        );

        let capabilities = parse_ssh_capability_probe(&output).unwrap();

        assert!(capabilities.navigation_ready);
        assert_eq!(capabilities.tool_bits, bits);
        assert_eq!(
            capabilities.available_tool_count,
            SSH_CAPABILITY_TOOLS.len() - 1
        );
        let find_index = SSH_CAPABILITY_TOOLS
            .iter()
            .position(|tool| *tool == "find")
            .unwrap();
        let file_index = SSH_CAPABILITY_TOOLS
            .iter()
            .position(|tool| *tool == "file")
            .unwrap();
        assert_eq!(capabilities.tool_bits.as_bytes()[find_index], b'1');
        assert_eq!(capabilities.tool_bits.as_bytes()[file_index], b'0');
    }

    #[test]
    fn extracts_the_local_bitset_without_returning_it_with_command_output() {
        let bits = "1".repeat(SSH_CAPABILITY_TOOLS.len());
        let output =
            format!("{SSH_CAPABILITY_BEGIN}\nbits|{bits}\n{SSH_CAPABILITY_END}\nproject-file.txt");

        let (capabilities, cleaned) = extract_ssh_capability_probe(&output).unwrap().unwrap();

        assert_eq!(
            capabilities.available_tool_count,
            SSH_CAPABILITY_TOOLS.len()
        );
        assert_eq!(cleaned, "project-file.txt");
        assert!(!cleaned.contains(&bits));
        assert!(!cleaned.contains(SSH_CAPABILITY_BEGIN));
    }

    #[test]
    fn capability_bitset_is_not_serialized_outside_the_runtime() {
        let bits = "1".repeat(SSH_CAPABILITY_TOOLS.len());
        let capabilities = parse_ssh_capability_probe(&format!(
            "{SSH_CAPABILITY_BEGIN}\nbits|{bits}\n{SSH_CAPABILITY_END}"
        ))
        .unwrap();

        let serialized = serde_json::to_value(capabilities).unwrap();

        assert_eq!(serialized["availableToolCount"], SSH_CAPABILITY_TOOLS.len());
        assert!(serialized.get("toolBits").is_none());
        assert!(serialized.get("availableTools").is_none());
        assert!(serialized.get("missingTools").is_none());
        assert!(!serialized.to_string().contains(&bits));
    }

    #[test]
    fn first_remote_command_gets_a_runtime_owned_probe() {
        let command = ssh_command_with_runtime_discovery("pwd && ls");

        assert!(command.contains(SSH_CAPABILITY_BEGIN));
        assert!(command.ends_with("pwd && ls"));
        assert!(!command.contains("sudo"));
        assert!(!command.contains("apt"));
    }

    #[test]
    fn rejects_an_incomplete_ssh_capability_profile() {
        let output = format!("{SSH_CAPABILITY_BEGIN}\nbits|101\n{SSH_CAPABILITY_END}");

        let error = parse_ssh_capability_probe(&output).unwrap_err();
        assert!(error.contains("3 of"));
        let command = ssh_capability_probe_command();
        assert!(command.contains("bits|%s"));
        assert!(!command.contains("sudo"));
        assert!(!command.contains("apt"));
    }

    #[test]
    fn terminal_context_preserves_complete_output_and_redacts_secrets() {
        let source = format!(
            "{}\napi_key=sk-private-token-value\nlatest-line",
            "old-output".repeat(20_000)
        );
        let cleaned = clean_terminal_transcript(&source);
        let (output, source_chars, redactions, truncated) =
            sanitize_terminal_snapshot_text(&cleaned);

        assert!(!truncated);
        assert_eq!(source_chars, output.chars().count());
        assert!(redactions >= 1);
        assert!(output.starts_with("old-output"));
        assert!(output.contains("latest-line"));
        assert!(!output.contains("sk-private-token-value"));
    }

    #[test]
    fn answers_each_cursor_query_only_once() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let writer: Arc<Mutex<Box<dyn Write + Send>>> =
            Arc::new(Mutex::new(Box::new(SharedWriter(Arc::clone(&captured)))));
        let mut tail = String::new();

        assert!(answer_terminal_queries("\u{1b}[6n", &mut tail, &writer));
        assert!(!answer_terminal_queries(
            "PS C:\\work> ",
            &mut tail,
            &writer
        ));
        assert_eq!(captured.lock().unwrap().as_slice(), b"\x1b[1;1R");
    }

    #[test]
    fn answers_a_cursor_query_split_across_chunks() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let writer: Arc<Mutex<Box<dyn Write + Send>>> =
            Arc::new(Mutex::new(Box::new(SharedWriter(Arc::clone(&captured)))));
        let mut tail = String::new();

        assert!(!answer_terminal_queries("\u{1b}[?", &mut tail, &writer));
        assert!(answer_terminal_queries("6n", &mut tail, &writer));
        assert_eq!(captured.lock().unwrap().as_slice(), b"\x1b[1;1R");
    }

    #[cfg(windows)]
    #[test]
    fn windows_owned_shell_commands_ignore_the_selected_session_and_default_directory() {
        let _guard = windows_pty_test_guard();
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        std::fs::write(a.path().join("owner.txt"), "owned-by-a").unwrap();
        std::fs::write(b.path().join("owner.txt"), "owned-by-b").unwrap();
        let (sender, receiver) = mpsc::channel();
        let mut terminals = TerminalRuntime::default();
        let callback_a = sender.clone();
        let session_a = terminals
            .open_in_directory(a.path(), move |event| {
                let _ = callback_a.send(event);
            })
            .unwrap();
        let session_b = terminals
            .open_in_directory(b.path(), move |event| {
                let _ = sender.send(event);
            })
            .unwrap();
        terminals.default_cwd = validated_cwd(b.path()).unwrap();
        assert_eq!(terminals.active_session_id, Some(session_b));
        terminals
            .run_agent_command_in_session(
                session_a,
                a.path(),
                "request-a".to_owned(),
                "Get-Content -LiteralPath 'owner.txt'",
            )
            .unwrap();
        terminals
            .run_agent_command_in_session(
                session_b,
                b.path(),
                "request-b".to_owned(),
                "Get-Content -LiteralPath 'owner.txt'",
            )
            .unwrap();
        let mut completed = std::collections::HashMap::new();
        let deadline = Instant::now() + Duration::from_secs(15);
        while completed.len() < 2 && Instant::now() < deadline {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(TerminalEvent::CommandFinished {
                    request_id,
                    exit_code,
                    output,
                    ..
                }) => {
                    assert_eq!(exit_code, 0);
                    completed.insert(request_id, output);
                }
                Ok(TerminalEvent::CommandFailed { message, .. }) => panic!("{message}"),
                _ => {}
            }
        }
        for session in &mut terminals.sessions {
            session.busy = false;
        }
        terminals.close(session_a).unwrap();
        assert!(
            terminals
                .run_agent_command_in_session(
                    session_a,
                    a.path(),
                    "closed".to_owned(),
                    "Get-Location"
                )
                .is_err()
        );
        terminals.close(session_b).unwrap();
        assert!(
            completed
                .get("request-a")
                .is_some_and(|text| text.contains("owned-by-a") && !text.contains("owned-by-b"))
        );
        assert!(
            completed
                .get("request-b")
                .is_some_and(|text| text.contains("owned-by-b") && !text.contains("owned-by-a"))
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_dedicated_agent_process_streams_and_completes_a_command() {
        let _guard = windows_pty_test_guard();
        let (sender, receiver) = mpsc::channel();
        let cwd = env::current_dir().unwrap();
        let mut session = TerminalSession::start(91, &cwd, move |event| {
            let _ = sender.send(event);
        })
        .unwrap();
        session
            .run_local_agent_command(
                "terminal-test".to_owned(),
                "run_command",
                "Write-Output 'central-agent-terminal-ok'",
                &cwd,
            )
            .unwrap();

        let mut completion = None;
        let mut transcript = String::new();
        for _ in 0..40 {
            match receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(TerminalEvent::Output { chunk, .. }) => transcript.push_str(&chunk),
                Ok(TerminalEvent::CommandFinished {
                    request_id,
                    exit_code,
                    output,
                    ..
                }) => {
                    completion = Some((request_id, exit_code, output));
                    break;
                }
                Ok(TerminalEvent::CommandFailed { message, .. }) => panic!("{message}"),
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        let (request_id, exit_code, output) = completion
            .unwrap_or_else(|| panic!("terminal command completion; transcript: {transcript:?}"));
        assert_eq!(request_id, "terminal-test");
        assert_eq!(exit_code, 0);
        assert!(output.contains("central-agent-terminal-ok"));
        assert!(!transcript.contains("__CENTRAL_AGENT_DONE_"));
        session.stop().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_dedicated_agent_process_interrupts_a_stuck_command_and_recovers() {
        let _guard = windows_pty_test_guard();
        let (sender, receiver) = mpsc::channel();
        let cwd = env::current_dir().unwrap();
        let mut session = TerminalSession::start(93, &cwd, move |event| {
            let _ = sender.send(event);
        })
        .unwrap();
        session
            .run_local_agent_command(
                "timeout-test".to_owned(),
                "run_command",
                "Start-Sleep -Seconds 30",
                &cwd,
            )
            .unwrap();

        let mut command_started = false;
        let mut transcript = String::new();
        for _ in 0..40 {
            match receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(TerminalEvent::Output { chunk, .. }) => {
                    transcript.push_str(&chunk);
                    if normalize_terminal_text(&transcript).contains("Start-Sleep -Seconds 30") {
                        command_started = true;
                        break;
                    }
                }
                Ok(TerminalEvent::CommandFailed { message, .. }) => panic!("{message}"),
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        assert!(
            command_started,
            "sleep command did not reach the PTY; transcript: {transcript:?}"
        );

        assert!(session.cancel_agent_command("timeout-test").unwrap());
        assert!(!session.cancel_agent_command("timeout-test").unwrap());
        session
            .run_local_agent_command(
                "recovery-test".to_owned(),
                "run_command",
                "Write-Output 'central-agent-recovered'",
                &cwd,
            )
            .unwrap();

        let mut recovered = false;
        for _ in 0..40 {
            match receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(TerminalEvent::CommandFinished {
                    request_id, output, ..
                }) if request_id == "recovery-test" => {
                    recovered = output.contains("central-agent-recovered");
                    break;
                }
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        assert!(recovered);
        session.stop().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_dedicated_agent_process_completes_paged_file_reads() {
        let _guard = windows_pty_test_guard();
        let directory = tempfile::tempdir().unwrap();
        let readme = (0..702)
            .map(|line| format!("README line {line}"))
            .collect::<Vec<_>>()
            .join("\r\n");
        std::fs::write(directory.path().join("README.md"), readme).unwrap();
        let (sender, receiver) = mpsc::channel();
        let mut session = TerminalSession::start(94, directory.path(), move |event| {
            let _ = sender.send(event);
        })
        .unwrap();
        session
            .run_local_agent_command(
                "paged-read-test".to_owned(),
                "run_command",
                "$p=(Get-Location).Path; Get-Content -Path (Join-Path $p 'README.md') | Select-Object -Skip 240 -First 320",
                directory.path(),
            )
            .unwrap();

        let mut completion = None;
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            match receiver.recv_timeout(Duration::from_millis(125)) {
                Ok(TerminalEvent::CommandFinished {
                    request_id,
                    exit_code,
                    output,
                    cwd,
                    ..
                }) => {
                    completion = Some((request_id, exit_code, output, cwd));
                    break;
                }
                Ok(TerminalEvent::CommandFailed { message, .. }) => panic!("{message}"),
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        let (request_id, exit_code, output, cwd) =
            completion.expect("paged file read should complete through the process exit event");
        assert_eq!(request_id, "paged-read-test");
        assert_eq!(exit_code, 0);
        assert!(output.contains("README line 240"));
        assert!(output.contains("README line 559"));
        assert_eq!(
            cwd.as_deref(),
            Some(directory.path().to_string_lossy().as_ref())
        );
        session.stop().unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_pty_preserves_the_first_character_of_immediate_manual_input() {
        let _guard = windows_pty_test_guard();
        let (sender, receiver) = mpsc::channel();
        let cwd = env::current_dir().unwrap();
        let mut session = TerminalSession::start(92, &cwd, move |event| {
            let _ = sender.send(event);
        })
        .unwrap();
        session
            .send_data("Get-Location | ForEach-Object { Write-Output 'central-agent-manual-ok' }\r")
            .unwrap();

        let mut transcript = String::new();
        for _ in 0..40 {
            match receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(TerminalEvent::Output { chunk, .. }) => {
                    transcript.push_str(&chunk);
                    if transcript.contains("central-agent-manual-ok") {
                        break;
                    }
                }
                Ok(TerminalEvent::Exited { message, .. }) => panic!("{message}"),
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        assert!(
            transcript.contains("central-agent-manual-ok"),
            "manual command did not run; transcript: {transcript:?}"
        );
        assert!(
            !transcript.contains("et-Location :"),
            "the first character was lost; transcript: {transcript:?}"
        );
        session.stop().unwrap();
    }
}
