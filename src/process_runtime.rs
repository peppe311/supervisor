use std::{
    collections::VecDeque,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU8, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use regex::Regex;
use serde::Serialize;
use serde_json::{Value, json};
use url::{Host, Url};

use crate::tab_context::{redact_sensitive, redaction_safe_prefix_len};

const INITIAL_PROCESS_TREE_CAPACITY: usize = 64;
const MAX_PREVIEW_URLS: usize = 6;
const MAX_PREVIEW_SCAN_TAIL_CHARS: usize = 8_192;
const MAX_LIVE_OUTPUT_TAIL_CHARS: usize = 64 * 1_024;
const PROCESS_STATUS_CHUNK_BYTES: usize = 256 * 1_024;
const STREAM_REDACTION_LOOKBEHIND_BYTES: usize = 4 * 1_024;
const MAX_MANAGED_PROCESS_RUNTIME: Duration = Duration::from_secs(30 * 60);
const MAX_MANAGED_PROCESS_OUTPUT_BYTES: u64 = 64 * 1_024 * 1_024;
const MAX_MANAGED_PROCESS_STDIN_BYTES: u64 = 8 * 1_024 * 1_024;
const STOP_NONE: u8 = 0;
const STOP_CANCELLED: u8 = 1;
const STOP_TIMEOUT: u8 = 2;
const STOP_OUTPUT_LIMIT: u8 = 3;
const STOP_SHUTDOWN: u8 = 4;

#[derive(Clone, Copy, Debug)]
pub(crate) enum ProcessStream {
    Stdout,
    Stderr,
}

#[derive(Debug)]
pub(crate) enum ManagedProcessEvent {
    Output {
        process_id: u64,
        stream: ProcessStream,
        chunk: String,
    },
    Exited {
        process_id: u64,
        exit_code: i32,
        stop_reason: u8,
        error: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ManagedProcessPhase {
    Running,
    Stopping,
    Completed,
    Failed,
    Cancelled,
}

impl ManagedProcessPhase {
    fn is_active(self) -> bool {
        matches!(self, Self::Running | Self::Stopping)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManagedProcessLimitsView {
    automatic_timeout: bool,
    automatic_timeout_ms: u64,
    output_cap: bool,
    output_cap_bytes: u64,
    stdin_cap_bytes: u64,
    job_objects: bool,
}

#[derive(Clone, Copy)]
struct ManagedProcessLimits {
    max_runtime: Duration,
    max_output_bytes: u64,
    max_stdin_bytes: u64,
}

impl Default for ManagedProcessLimits {
    fn default() -> Self {
        Self {
            max_runtime: MAX_MANAGED_PROCESS_RUNTIME,
            max_output_bytes: MAX_MANAGED_PROCESS_OUTPUT_BYTES,
            max_stdin_bytes: MAX_MANAGED_PROCESS_STDIN_BYTES,
        }
    }
}

struct ManagedProcess {
    id: u64,
    pid: u32,
    owner_run_id: Option<u64>,
    owner_scope: Option<String>,
    label: String,
    command: String,
    phase: ManagedProcessPhase,
    status: String,
    started_at_ms: u128,
    finished_at_ms: Option<u128>,
    exit_code: Option<i32>,
    stdout: ProcessOutput,
    stderr: ProcessOutput,
    preview_urls: Vec<String>,
    preview_scan_tail: String,
    stdin: Option<ChildStdin>,
    stdin_bytes: u64,
    output_limit_reached: bool,
    limits: ManagedProcessLimits,
    control: Arc<ProcessControl>,
}

struct ProcessOutput {
    live_tail: String,
    live_tail_truncated: bool,
    total_bytes: u64,
    source_bytes: u64,
    redaction_count: usize,
    redaction_pending: String,
    redaction_mode: Option<StreamingRedactionMode>,
    redaction_deferred_prefix: String,
    redacting_token_started: bool,
    spool: tempfile::NamedTempFile,
    spool_error: Option<String>,
}

impl ProcessOutput {
    fn create(stream_name: &str) -> Result<Self, String> {
        let spool = tempfile::Builder::new()
            .prefix("central-agent-process-")
            .suffix(&format!("-{stream_name}.log"))
            .tempfile()
            .map_err(|error| {
                format!("Could not create the managed process output spool: {error}")
            })?;
        Ok(Self {
            live_tail: String::new(),
            live_tail_truncated: false,
            total_bytes: 0,
            source_bytes: 0,
            redaction_count: 0,
            redaction_pending: String::new(),
            redaction_mode: None,
            redaction_deferred_prefix: String::new(),
            redacting_token_started: false,
            spool,
            spool_error: None,
        })
    }

    fn append(&mut self, chunk: &str) {
        self.source_bytes = self.source_bytes.saturating_add(chunk.len() as u64);
        self.redaction_pending.push_str(chunk);
        self.drain_redaction_pending(false);
    }

    fn finalize(&mut self) {
        self.drain_redaction_pending(true);
        self.redaction_mode = None;
        self.redaction_deferred_prefix.clear();
        self.redacting_token_started = false;
    }

    fn drain_redaction_pending(&mut self, final_chunk: bool) {
        loop {
            if let Some(mode) = self.redaction_mode {
                if matches!(mode, StreamingRedactionMode::PossibleUrlUserinfo) {
                    let delimiter = self
                        .redaction_pending
                        .char_indices()
                        .find(|(_, character)| {
                            *character == '@'
                                || *character == '/'
                                || *character == '?'
                                || *character == '#'
                                || character.is_whitespace()
                        })
                        .map(|(index, character)| (index, index + character.len_utf8(), character));

                    if let Some((_delimiter_start, delimiter_end, delimiter)) = delimiter {
                        let segment = self.redaction_pending[..delimiter_end].to_owned();
                        self.redaction_pending.drain(..delimiter_end);
                        if delimiter == '@' {
                            self.append_safe("[REDACTED URL CREDENTIAL]@");
                            self.redaction_count = self.redaction_count.saturating_add(1);
                        } else {
                            let mut visible = std::mem::take(&mut self.redaction_deferred_prefix);
                            visible.push_str(&segment);
                            self.append_redacted(&visible);
                        }
                        self.redaction_deferred_prefix.clear();
                        self.redaction_mode = None;
                        self.redacting_token_started = false;
                        continue;
                    }

                    if final_chunk {
                        let mut visible = std::mem::take(&mut self.redaction_deferred_prefix);
                        visible.push_str(&std::mem::take(&mut self.redaction_pending));
                        self.append_redacted(&visible);
                        self.redaction_mode = None;
                        return;
                    }

                    if self.redaction_pending.len() > STREAM_REDACTION_LOOKBEHIND_BYTES {
                        self.redaction_pending.clear();
                        self.redaction_deferred_prefix.clear();
                        self.append_safe("[REDACTED URL CREDENTIAL]");
                        self.redaction_count = self.redaction_count.saturating_add(1);
                        self.redaction_mode = Some(StreamingRedactionMode::UrlPassword);
                        continue;
                    }
                    return;
                }

                let mut token_started = self.redacting_token_started;
                let mut delimiter = None;
                for (index, character) in self.redaction_pending.char_indices() {
                    let delimiter_found = match mode {
                        StreamingRedactionMode::Line => character == '\n' || character == '\r',
                        StreamingRedactionMode::UrlPassword => {
                            character == '@' || character.is_whitespace()
                        }
                        StreamingRedactionMode::Token if !token_started => {
                            if character == '\n' || character == '\r' {
                                true
                            } else if character.is_whitespace() {
                                false
                            } else {
                                token_started = true;
                                false
                            }
                        }
                        StreamingRedactionMode::Token => character.is_whitespace(),
                        StreamingRedactionMode::PossibleUrlUserinfo => {
                            unreachable!("possible URL userinfo is handled before token delimiters")
                        }
                    };
                    if delimiter_found {
                        delimiter = Some((index, index + character.len_utf8()));
                        break;
                    }
                }

                if let Some((delimiter_start, delimiter_end)) = delimiter {
                    let delimiter =
                        self.redaction_pending[delimiter_start..delimiter_end].to_owned();
                    self.redaction_pending.drain(..delimiter_end);
                    self.append_safe(&delimiter);
                    self.redaction_mode = None;
                    self.redacting_token_started = false;
                    continue;
                }

                self.redacting_token_started = token_started;
                self.redaction_pending.clear();
                return;
            }

            if let Some(prefix) = streaming_sensitive_prefix(&self.redaction_pending) {
                let before = self.redaction_pending[..prefix.start].to_owned();
                let matched = self.redaction_pending[prefix.start..prefix.end].to_owned();
                self.redaction_pending.drain(..prefix.end);
                self.append_redacted(&before);
                if matches!(prefix.mode, StreamingRedactionMode::PossibleUrlUserinfo) {
                    self.redaction_deferred_prefix = matched;
                } else {
                    self.append_safe(prefix.replacement);
                    self.redaction_count = self.redaction_count.saturating_add(1);
                }
                self.redaction_mode = Some(prefix.mode);
                self.redacting_token_started = prefix.token_started;
                continue;
            }

            if final_chunk {
                let pending = std::mem::take(&mut self.redaction_pending);
                self.append_redacted(&pending);
                return;
            }

            if let Some(line_end) = self
                .redaction_pending
                .char_indices()
                .rev()
                .find(|(_, character)| *character == '\n' || *character == '\r')
                .map(|(index, character)| index + character.len_utf8())
            {
                let tail = self.redaction_pending.split_off(line_end);
                let stable = std::mem::replace(&mut self.redaction_pending, tail);
                self.append_redacted(&stable);
                continue;
            }

            if self.redaction_pending.len() <= STREAM_REDACTION_LOOKBEHIND_BYTES {
                return;
            }

            let mut desired = self
                .redaction_pending
                .len()
                .saturating_sub(STREAM_REDACTION_LOOKBEHIND_BYTES);
            while desired > 0 && !self.redaction_pending.is_char_boundary(desired) {
                desired -= 1;
            }
            let safe_prefix = redaction_safe_prefix_len(&self.redaction_pending, desired);
            if safe_prefix == 0 {
                return;
            }
            let tail = self.redaction_pending.split_off(safe_prefix);
            let stable = std::mem::replace(&mut self.redaction_pending, tail);
            self.append_redacted(&stable);
        }
    }

    fn append_redacted(&mut self, chunk: &str) {
        let (chunk, redactions) = redact_sensitive(chunk);
        self.redaction_count = self.redaction_count.saturating_add(redactions);
        self.append_safe(&chunk);
    }

    fn append_safe(&mut self, chunk: &str) {
        self.total_bytes = self.total_bytes.saturating_add(chunk.len() as u64);
        if self.spool_error.is_none()
            && let Err(error) = self.spool.write_all(chunk.as_bytes())
        {
            self.spool_error = Some(format!(
                "Could not preserve the complete managed process output: {error}"
            ));
        }

        self.live_tail.push_str(chunk);
        self.live_tail_truncated |=
            trim_front_chars(&mut self.live_tail, MAX_LIVE_OUTPUT_TAIL_CHARS);
    }

    fn chunk_at(&mut self, offset: u64) -> Result<ProcessOutputChunk, String> {
        let result = (|| {
            self.spool.flush()?;
            let mut file = self.spool.reopen()?;
            let available_bytes = file.metadata()?.len();
            if offset > available_bytes {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "requested offset {offset} is beyond the {available_bytes} preserved bytes"
                    ),
                ));
            }
            file.seek(SeekFrom::Start(offset))?;
            let remaining = available_bytes.saturating_sub(offset);
            let read_limit = remaining.min(PROCESS_STATUS_CHUNK_BYTES as u64) as usize;
            let mut bytes = Vec::with_capacity(read_limit);
            file.take(read_limit as u64).read_to_end(&mut bytes)?;

            let valid_length = match std::str::from_utf8(&bytes) {
                Ok(_) => bytes.len(),
                Err(error) if error.error_len().is_none() => error.valid_up_to(),
                Err(error) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("process output spool is not valid UTF-8: {error}"),
                    ));
                }
            };
            bytes.truncate(valid_length);
            let content = String::from_utf8(bytes)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
            let next_offset = offset.saturating_add(valid_length as u64);
            Ok((content, available_bytes, next_offset))
        })();

        match result {
            Ok((content, available_bytes, next_offset)) => Ok(ProcessOutputChunk {
                content,
                offset,
                next_offset,
                has_more: next_offset < available_bytes,
                truncated: self.spool_error.is_some(),
                error: self.spool_error.clone(),
            }),
            Err(error) => Err(format!(
                "Could not read the preserved managed process output at offset {offset}: {error}"
            )),
        }
    }
}

struct ProcessOutputChunk {
    content: String,
    offset: u64,
    next_offset: u64,
    has_more: bool,
    truncated: bool,
    error: Option<String>,
}

struct StreamingSensitivePrefix {
    start: usize,
    end: usize,
    replacement: &'static str,
    mode: StreamingRedactionMode,
    token_started: bool,
}

#[derive(Clone, Copy)]
enum StreamingRedactionMode {
    Token,
    Line,
    PossibleUrlUserinfo,
    UrlPassword,
}

fn streaming_sensitive_prefix(value: &str) -> Option<StreamingSensitivePrefix> {
    static PATTERNS: OnceLock<Vec<(Regex, &'static str, StreamingRedactionMode, bool)>> =
        OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        [
            (
                r"(?i)\b(?:password|passcode|otp|pin|secret|api[_ -]?key|token|access[_ -]?token|auth[_ -]?token|refresh[_ -]?token|session[_ -]?token|gh[_ -]?token|client[_ -]?secret|private[_ -]?key|aws[_ -]?secret[_ -]?access[_ -]?key|authorization|credential)\b[ \t]*[:=][ \t]*",
                "[REDACTED CREDENTIAL]",
                StreamingRedactionMode::Line,
                false,
            ),
            (
                r"(?i)\bbearer[ \t]+",
                "Bearer [REDACTED TOKEN]",
                StreamingRedactionMode::Token,
                false,
            ),
            (
                r"(?i)\b(?:sk|pk)-",
                "[REDACTED TOKEN]",
                StreamingRedactionMode::Token,
                true,
            ),
            (
                r"(?i)\b(?:ghp_|github_pat_|xox[baprs]-|AKIA)",
                "[REDACTED TOKEN]",
                StreamingRedactionMode::Token,
                true,
            ),
            (
                r"\beyJ[A-Za-z0-9_-]{2,}",
                "[REDACTED TOKEN]",
                StreamingRedactionMode::Token,
                true,
            ),
            (
                r"(?i)\b[a-z][a-z0-9+.-]{1,15}://[^/\s:@]{1,256}:",
                "[REDACTED URL CREDENTIAL]",
                StreamingRedactionMode::PossibleUrlUserinfo,
                false,
            ),
        ]
        .into_iter()
        .map(|(pattern, replacement, mode, token_started)| {
            (
                Regex::new(pattern).expect("streaming credential prefix regex must compile"),
                replacement,
                mode,
                token_started,
            )
        })
        .collect()
    });

    patterns
        .iter()
        .filter_map(|(pattern, replacement, mode, token_started)| {
            pattern.find(value).map(|found| StreamingSensitivePrefix {
                start: found.start(),
                end: found.end(),
                replacement,
                mode: *mode,
                token_started: *token_started,
            })
        })
        .min_by_key(|prefix| prefix.start)
}

struct ProcessControl {
    child: Mutex<Child>,
    stop_reason: AtomicU8,
    output_bytes: AtomicU64,
    #[cfg(windows)]
    job: WindowsJob,
}

impl ProcessControl {
    fn terminate(&self, reason: u8) -> Result<(), String> {
        let effective_reason = self
            .stop_reason
            .compare_exchange(STOP_NONE, reason, Ordering::SeqCst, Ordering::SeqCst)
            .unwrap_or_else(|existing| existing);

        #[cfg(windows)]
        if self
            .job
            .terminate(0xCA11_0000 | u32::from(effective_reason))
            .is_ok()
        {
            return Ok(());
        }

        self.child
            .lock()
            .map_err(|_| "Managed process state is unavailable".to_owned())?
            .kill()
            .map_err(|error| format!("Could not terminate the managed process: {error}"))
    }

    fn owned_pids(&self, root_pid: u32) -> Vec<u32> {
        #[cfg(windows)]
        {
            let mut pids = self.job.pids().unwrap_or_default();
            if !pids.contains(&root_pid) {
                pids.push(root_pid);
            }
            pids
        }

        #[cfg(not(windows))]
        {
            vec![root_pid]
        }
    }
}

#[cfg(windows)]
struct WindowsJob(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
unsafe impl Send for WindowsJob {}
#[cfg(windows)]
unsafe impl Sync for WindowsJob {}

#[cfg(windows)]
impl WindowsJob {
    fn create_for(child: &mut Child) -> Result<Self, String> {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::{
            Foundation::HANDLE,
            System::JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject,
            },
        };

        let handle = unsafe { CreateJobObjectW(None, None) }
            .map_err(|error| format!("Could not create the Windows process job: {error}"))?;
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
        .map_err(|error| format!("Could not configure Windows process ownership: {error}"))?;

        let process_handle = HANDLE(child.as_raw_handle());
        unsafe { AssignProcessToJobObject(job.0, process_handle) }
            .map_err(|error| format!("Could not assign the process to its Windows job: {error}"))?;
        resume_process_threads(child.id())?;
        Ok(job)
    }

    fn terminate(&self, exit_code: u32) -> windows::core::Result<()> {
        unsafe { windows::Win32::System::JobObjects::TerminateJobObject(self.0, exit_code) }
    }

    fn pids(&self) -> Result<Vec<u32>, String> {
        use windows::Win32::System::JobObjects::{
            JOBOBJECT_BASIC_PROCESS_ID_LIST, JobObjectBasicProcessIdList, QueryInformationJobObject,
        };

        let header_bytes = std::mem::offset_of!(JOBOBJECT_BASIC_PROCESS_ID_LIST, ProcessIdList);
        let mut capacity = INITIAL_PROCESS_TREE_CAPACITY;
        loop {
            let id_bytes = capacity
                .checked_mul(std::mem::size_of::<usize>())
                .ok_or_else(|| "The managed process tree is too large to inspect".to_owned())?;
            let buffer_bytes = header_bytes
                .checked_add(id_bytes)
                .filter(|bytes| *bytes <= u32::MAX as usize)
                .ok_or_else(|| "The managed process tree is too large to inspect".to_owned())?;
            let mut buffer = vec![0_u8; buffer_bytes];
            let query = unsafe {
                QueryInformationJobObject(
                    Some(self.0),
                    JobObjectBasicProcessIdList,
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    None,
                )
            };

            let list = unsafe { &*(buffer.as_ptr().cast::<JOBOBJECT_BASIC_PROCESS_ID_LIST>()) };
            let assigned = list.NumberOfAssignedProcesses as usize;
            let listed = list.NumberOfProcessIdsInList as usize;
            if assigned > capacity || listed > capacity {
                capacity = assigned
                    .max(listed)
                    .max(capacity.checked_mul(2).ok_or_else(|| {
                        "The managed process tree is too large to inspect".to_owned()
                    })?);
                continue;
            }

            query
                .map_err(|error| format!("Could not inspect the managed process tree: {error}"))?;
            let ids = unsafe {
                std::slice::from_raw_parts(
                    buffer.as_ptr().add(header_bytes).cast::<usize>(),
                    listed,
                )
            };
            return Ok(ids
                .iter()
                .copied()
                .filter_map(|pid| u32::try_from(pid).ok())
                .filter(|pid| *pid != 0)
                .collect());
        }
    }
}

#[cfg(windows)]
impl Drop for WindowsJob {
    fn drop(&mut self) {
        let _ = unsafe { windows::Win32::Foundation::CloseHandle(self.0) };
    }
}

#[cfg(windows)]
fn resume_process_threads(pid: u32) -> Result<(), String> {
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
        .map_err(|error| format!("Could not inspect the suspended process: {error}"))?;
    let result = (|| {
        let mut entry = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        unsafe { Thread32First(snapshot, &mut entry) }
            .map_err(|error| format!("Could not find the suspended process thread: {error}"))?;
        let mut resumed = 0usize;
        loop {
            if entry.th32OwnerProcessID == pid {
                let thread =
                    unsafe { OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID) }
                        .map_err(|error| {
                            format!("Could not open the suspended process thread: {error}")
                        })?;
                let resume_result = unsafe { ResumeThread(thread) };
                let _ = unsafe { CloseHandle(thread) };
                if resume_result == u32::MAX {
                    return Err("Could not resume the managed process thread".to_owned());
                }
                resumed = resumed.saturating_add(1);
            }
            if unsafe { Thread32Next(snapshot, &mut entry) }.is_err() {
                break;
            }
        }
        if resumed == 0 {
            return Err("The managed process had no resumable thread".to_owned());
        }
        Ok(())
    })();
    let _ = unsafe { CloseHandle(snapshot) };
    result
}

pub(crate) struct ManagedProcessRuntime {
    processes: VecDeque<ManagedProcess>,
    next_process_id: u64,
}

impl Default for ManagedProcessRuntime {
    fn default() -> Self {
        Self {
            processes: VecDeque::new(),
            next_process_id: 1,
        }
    }
}

impl ManagedProcessRuntime {
    pub(crate) fn start<F>(
        &mut self,
        command: String,
        cwd: PathBuf,
        owner_run_id: Option<u64>,
        owner_scope: Option<String>,
        callback: F,
    ) -> Result<Value, String>
    where
        F: Fn(ManagedProcessEvent) + Send + Sync + 'static,
    {
        self.start_with_limits(
            command,
            cwd,
            owner_run_id,
            owner_scope,
            ManagedProcessLimits::default(),
            callback,
        )
    }

    fn start_with_limits<F>(
        &mut self,
        command: String,
        cwd: PathBuf,
        owner_run_id: Option<u64>,
        owner_scope: Option<String>,
        limits: ManagedProcessLimits,
        callback: F,
    ) -> Result<Value, String>
    where
        F: Fn(ManagedProcessEvent) + Send + Sync + 'static,
    {
        if limits.max_runtime.is_zero()
            || limits.max_output_bytes == 0
            || limits.max_stdin_bytes == 0
        {
            return Err("Managed process resource limits must be positive".to_owned());
        }
        let cwd = canonical_directory(&cwd)?;
        let process_id = self.next_process_id;
        self.next_process_id = self.next_process_id.saturating_add(1);

        let stdout_output = ProcessOutput::create("stdout")?;
        let stderr_output = ProcessOutput::create("stderr")?;

        let mut child = managed_command(&command, &cwd)
            .spawn()
            .map_err(|error| format!("Could not start the managed process: {error}"))?;
        let pid = child.id();
        let stdin = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Managed process stdout was not captured".to_owned())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "Managed process stderr was not captured".to_owned())?;

        #[cfg(windows)]
        let job = match WindowsJob::create_for(&mut child) {
            Ok(job) => job,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };

        let control = Arc::new(ProcessControl {
            child: Mutex::new(child),
            stop_reason: AtomicU8::new(STOP_NONE),
            output_bytes: AtomicU64::new(0),
            #[cfg(windows)]
            job,
        });
        let callback: Arc<dyn Fn(ManagedProcessEvent) + Send + Sync> = Arc::new(callback);
        let stdout_reader = spawn_output_reader(
            process_id,
            ProcessStream::Stdout,
            stdout,
            Arc::clone(&control),
            limits.max_output_bytes,
            Arc::clone(&callback),
        );
        let stderr_reader = spawn_output_reader(
            process_id,
            ProcessStream::Stderr,
            stderr,
            Arc::clone(&control),
            limits.max_output_bytes,
            Arc::clone(&callback),
        );
        spawn_waiter(
            process_id,
            Arc::clone(&control),
            [stdout_reader, stderr_reader],
            limits.max_runtime,
            callback,
        );

        let command_summary = command;
        self.processes.push_back(ManagedProcess {
            id: process_id,
            pid,
            owner_run_id,
            owner_scope,
            label: format!("Task {process_id}"),
            command: command_summary.clone(),
            phase: ManagedProcessPhase::Running,
            status: "Managed process is running".to_owned(),
            started_at_ms: unix_time_ms(),
            finished_at_ms: None,
            exit_code: None,
            stdout: stdout_output,
            stderr: stderr_output,
            preview_urls: Vec::new(),
            preview_scan_tail: String::new(),
            stdin,
            stdin_bytes: 0,
            output_limit_reached: false,
            limits,
            control,
        });
        Ok(json!({
            "processId": process_id,
            "pid": pid,
            "phase": "running",
            "command": command_summary,
            "cwd": cwd.display().to_string(),
        }))
    }

    pub(crate) fn list_value_for_owner(&self, owner_scope: &str) -> Value {
        let visible = self
            .processes
            .iter()
            .filter(|process| process.owner_scope.as_deref() == Some(owner_scope))
            .collect::<Vec<_>>();
        json!({
            "processes": visible.iter().rev().map(|process| json!({
                "processId": process.id,
                "pid": process.pid,
                "ownerRunId": process.owner_run_id,
                "label": process.label,
                "phase": process.phase,
                "running": process.phase.is_active(),
                "status": process.status,
                "exitCode": process.exit_code,
                "previewUrls": process.preview_urls,
            })).collect::<Vec<_>>(),
            "runningCount": visible.iter().filter(|process| process.phase.is_active()).count(),
            "limits": limits_view(),
        })
    }

    pub(crate) fn status_value_for_owner(
        &mut self,
        process_id: u64,
        stdout_offset: u64,
        stderr_offset: u64,
        owner_scope: &str,
    ) -> Result<Value, String> {
        self.ensure_owner(process_id, owner_scope)?;
        self.status_value(process_id, stdout_offset, stderr_offset)
    }

    pub(crate) fn status_value(
        &mut self,
        process_id: u64,
        stdout_offset: u64,
        stderr_offset: u64,
    ) -> Result<Value, String> {
        let process = self.find_mut(process_id)?;
        let stdout = process.stdout.chunk_at(stdout_offset)?;
        let stderr = process.stderr.chunk_at(stderr_offset)?;
        let has_more = stdout.has_more || stderr.has_more;
        let output_truncated = stdout.truncated || stderr.truncated || process.output_limit_reached;
        let output_capture_errors = [stdout.error.clone(), stderr.error.clone()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let output_capture_error =
            (!output_capture_errors.is_empty()).then(|| output_capture_errors.join("; "));
        Ok(json!({
            "processId": process.id,
            "pid": process.pid,
            "ownerRunId": process.owner_run_id,
            "phase": process.phase,
            "running": process.phase.is_active(),
            "status": process.status,
            "exitCode": process.exit_code,
            "stdout": stdout.content,
            "stderr": stderr.content,
            "stdoutBytes": process.stdout.total_bytes,
            "stderrBytes": process.stderr.total_bytes,
            "stdoutSourceBytes": process.stdout.source_bytes,
            "stderrSourceBytes": process.stderr.source_bytes,
            "stdoutRedactionCount": process.stdout.redaction_count,
            "stderrRedactionCount": process.stderr.redaction_count,
            "providerRedacted": true,
            "stdoutOffset": stdout.offset,
            "stdoutNextOffset": stdout.next_offset,
            "stdoutHasMore": stdout.has_more,
            "stderrOffset": stderr.offset,
            "stderrNextOffset": stderr.next_offset,
            "stderrHasMore": stderr.has_more,
            "hasMore": has_more,
            "outputComplete": !process.phase.is_active() && !has_more && !output_truncated,
            "outputTruncated": output_truncated,
            "outputCaptureError": output_capture_error,
            "resourceLimits": limits_view_for(process.limits),
            "stdinBytes": process.stdin_bytes,
            "startedAtMs": process.started_at_ms,
            "finishedAtMs": process.finished_at_ms,
            "previewUrls": process.preview_urls,
        }))
    }

    pub(crate) fn write_stdin(
        &mut self,
        process_id: u64,
        data: &str,
        close: bool,
    ) -> Result<Value, String> {
        let process = self.find_mut(process_id)?;
        if !process.phase.is_active() {
            return Err("The managed process is no longer running".to_owned());
        }
        if !data.is_empty() {
            let next_stdin_bytes = process
                .stdin_bytes
                .checked_add(data.len() as u64)
                .ok_or_else(|| "Managed process stdin byte count overflowed".to_owned())?;
            if next_stdin_bytes > process.limits.max_stdin_bytes {
                return Err(format!(
                    "Managed process stdin exceeds the {}-byte lifetime limit",
                    process.limits.max_stdin_bytes
                ));
            }
            let stdin = process
                .stdin
                .as_mut()
                .ok_or_else(|| "Managed process stdin is closed".to_owned())?;
            stdin
                .write_all(data.as_bytes())
                .and_then(|_| stdin.flush())
                .map_err(|error| format!("Could not write to managed process stdin: {error}"))?;
            process.stdin_bytes = next_stdin_bytes;
        }
        if close {
            process.stdin.take();
        }
        Ok(json!({
            "processId": process_id,
            "writtenBytes": data.len(),
            "stdinClosed": close,
        }))
    }

    pub(crate) fn write_stdin_for_owner(
        &mut self,
        process_id: u64,
        data: &str,
        close: bool,
        owner_scope: &str,
    ) -> Result<Value, String> {
        self.ensure_owner(process_id, owner_scope)?;
        self.write_stdin(process_id, data, close)
    }

    pub(crate) fn cancel(&mut self, process_id: u64) -> Result<Value, String> {
        let process = self.find_mut(process_id)?;
        if !process.phase.is_active() {
            return Ok(json!({
                "processId": process_id,
                "alreadyFinished": true,
                "phase": process.phase,
            }));
        }
        process.control.terminate(STOP_CANCELLED)?;
        process.phase = ManagedProcessPhase::Stopping;
        process.status = "Cancellation requested".to_owned();
        process.stdin.take();
        Ok(json!({
            "processId": process_id,
            "cancellationRequested": true,
        }))
    }

    pub(crate) fn cancel_for_owner(
        &mut self,
        process_id: u64,
        owner_scope: &str,
    ) -> Result<Value, String> {
        self.ensure_owner(process_id, owner_scope)?;
        self.cancel(process_id)
    }

    pub(crate) fn cancel_owner(&mut self, owner_run_id: u64) -> usize {
        let mut count = 0usize;
        for process in &mut self.processes {
            if process.owner_run_id == Some(owner_run_id) && process.phase.is_active() {
                let _ = process.control.terminate(STOP_CANCELLED);
                process.phase = ManagedProcessPhase::Stopping;
                process.status = "Cancelled with the owning agent run".to_owned();
                process.stdin.take();
                count = count.saturating_add(1);
            }
        }
        count
    }

    pub(crate) fn has_active_owner(&self, owner_run_id: u64) -> bool {
        self.processes
            .iter()
            .any(|process| process.owner_run_id == Some(owner_run_id) && process.phase.is_active())
    }

    pub(crate) fn owner_run_id(&self, process_id: u64) -> Option<u64> {
        self.find(process_id)
            .ok()
            .and_then(|process| process.owner_run_id)
    }

    pub(crate) fn cancel_all(&mut self, status: &str) -> usize {
        let mut count = 0usize;
        for process in &mut self.processes {
            if !process.phase.is_active() {
                continue;
            }
            let _ = process.control.terminate(STOP_CANCELLED);
            process.phase = ManagedProcessPhase::Stopping;
            process.status = status.to_owned();
            process.stdin.take();
            count = count.saturating_add(1);
        }
        count
    }

    pub(crate) fn handle_event(&mut self, event: ManagedProcessEvent) -> Option<u64> {
        match event {
            ManagedProcessEvent::Output {
                process_id,
                stream,
                chunk,
            } => {
                let process = self.find_mut(process_id).ok()?;
                process.preview_scan_tail.push_str(&chunk);
                trim_front_chars(&mut process.preview_scan_tail, MAX_PREVIEW_SCAN_TAIL_CHARS);
                for url in extract_local_preview_urls(&process.preview_scan_tail) {
                    if process.preview_urls.len() >= MAX_PREVIEW_URLS {
                        break;
                    }
                    if !process.preview_urls.contains(&url) {
                        process.preview_urls.push(url);
                    }
                }
                let target = match stream {
                    ProcessStream::Stdout => &mut process.stdout,
                    ProcessStream::Stderr => &mut process.stderr,
                };
                target.append(&chunk);
                Some(process_id)
            }
            ManagedProcessEvent::Exited {
                process_id,
                exit_code,
                stop_reason,
                error,
            } => {
                let process = self.find_mut(process_id).ok()?;
                process.stdin.take();
                process.stdout.finalize();
                process.stderr.finalize();
                process.exit_code = Some(exit_code);
                process.finished_at_ms = Some(unix_time_ms());
                let (phase, status) = match stop_reason {
                    STOP_CANCELLED | STOP_SHUTDOWN => (
                        ManagedProcessPhase::Cancelled,
                        "Managed process was cancelled".to_owned(),
                    ),
                    STOP_TIMEOUT => (
                        ManagedProcessPhase::Failed,
                        format!(
                            "Managed process exceeded the {} ms runtime limit",
                            process.limits.max_runtime.as_millis()
                        ),
                    ),
                    STOP_OUTPUT_LIMIT => {
                        process.output_limit_reached = true;
                        (
                            ManagedProcessPhase::Failed,
                            format!(
                                "Managed process exceeded the {}-byte combined output limit",
                                process.limits.max_output_bytes
                            ),
                        )
                    }
                    _ if exit_code == 0 && error.is_none() => (
                        ManagedProcessPhase::Completed,
                        "Managed process completed".to_owned(),
                    ),
                    _ => (
                        ManagedProcessPhase::Failed,
                        error.unwrap_or_else(|| {
                            format!("Managed process exited with code {exit_code}")
                        }),
                    ),
                };
                process.phase = phase;
                process.status = status;
                Some(process_id)
            }
        }
    }

    pub(crate) fn owned_pids(&self) -> Vec<u32> {
        let mut pids = self
            .processes
            .iter()
            .filter(|process| process.phase.is_active())
            .flat_map(|process| process.control.owned_pids(process.pid))
            .collect::<Vec<_>>();
        pids.sort_unstable();
        pids.dedup();
        pids
    }

    pub(crate) fn validated_preview_url(
        &self,
        process_id: u64,
        candidate: &str,
    ) -> Result<String, String> {
        let process = self.find(process_id)?;
        if !process.phase.is_active() {
            return Err("The managed process that owned this preview has stopped".to_owned());
        }
        let normalized = normalize_local_preview_url(candidate)
            .ok_or_else(|| "Only detected HTTP loopback previews can be opened".to_owned())?;
        process
            .preview_urls
            .iter()
            .any(|url| url == &normalized)
            .then_some(normalized)
            .ok_or_else(|| {
                "The preview URL does not belong to the selected managed process".to_owned()
            })
    }

    pub(crate) fn process_id_for_pid(&self, pid: u32) -> Option<u64> {
        self.processes
            .iter()
            .rev()
            .filter(|process| process.phase.is_active())
            .find_map(|process| {
                process
                    .control
                    .owned_pids(process.pid)
                    .contains(&pid)
                    .then_some(process.id)
            })
    }

    pub(crate) fn process_looks_like_game(&self, process_id: u64) -> bool {
        self.find(process_id)
            .is_ok_and(|process| looks_like_game_command(&process.command))
    }

    fn find(&self, process_id: u64) -> Result<&ManagedProcess, String> {
        self.processes
            .iter()
            .find(|process| process.id == process_id)
            .ok_or_else(|| "Unknown or expired managed process reference".to_owned())
    }

    fn find_mut(&mut self, process_id: u64) -> Result<&mut ManagedProcess, String> {
        self.processes
            .iter_mut()
            .find(|process| process.id == process_id)
            .ok_or_else(|| "Unknown or expired managed process reference".to_owned())
    }

    fn ensure_owner(&self, process_id: u64, owner_scope: &str) -> Result<(), String> {
        let process = self.find(process_id)?;
        if process.owner_scope.as_deref() != Some(owner_scope) {
            return Err("The process belongs to a different agent conversation".to_owned());
        }
        Ok(())
    }
}

impl Drop for ManagedProcessRuntime {
    fn drop(&mut self) {
        for process in &mut self.processes {
            if process.phase.is_active() {
                let _ = process.control.terminate(STOP_SHUTDOWN);
            }
        }
    }
}

fn local_preview_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?i)https?://(?:localhost|127(?:\.[0-9]{1,3}){3}|\[::1\]):[0-9]{1,5}(?:/[^\s<>\"'`]*)?"#,
        )
        .expect("local preview URL regex must compile")
    })
}

fn extract_local_preview_urls(value: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for capture in local_preview_regex().find_iter(value) {
        let candidate = capture
            .as_str()
            .trim_end_matches(['.', ',', ';', '!', '?', ')', ']', '}']);
        let Some(url) = normalize_local_preview_url(candidate) else {
            continue;
        };
        if !urls.contains(&url) {
            urls.push(url);
        }
        if urls.len() >= MAX_PREVIEW_URLS {
            break;
        }
    }
    urls
}

fn normalize_local_preview_url(value: &str) -> Option<String> {
    let mut parsed = Url::parse(value).ok()?;
    if parsed.scheme() != "http"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_none()
    {
        return None;
    }
    let is_loopback = match parsed.host()? {
        Host::Domain(domain) => domain.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(address) => address.is_loopback(),
        Host::Ipv6(address) => address.is_loopback(),
    };
    if !is_loopback {
        return None;
    }
    parsed.set_fragment(None);
    Some(parsed.to_string())
}

fn looks_like_game_command(command: &str) -> bool {
    let command = command.to_ascii_lowercase();
    [
        "bevy",
        "macroquad",
        "raylib",
        "ggez",
        "sdl",
        "godot",
        "game.exe",
        "game ",
    ]
    .iter()
    .any(|marker| command.contains(marker))
}

fn limits_view() -> ManagedProcessLimitsView {
    limits_view_for(ManagedProcessLimits::default())
}

fn limits_view_for(limits: ManagedProcessLimits) -> ManagedProcessLimitsView {
    ManagedProcessLimitsView {
        automatic_timeout: true,
        automatic_timeout_ms: u64::try_from(limits.max_runtime.as_millis()).unwrap_or(u64::MAX),
        output_cap: true,
        output_cap_bytes: limits.max_output_bytes,
        stdin_cap_bytes: limits.max_stdin_bytes,
        job_objects: cfg!(windows),
    }
}

fn spawn_output_reader<R: Read + Send + 'static>(
    process_id: u64,
    stream: ProcessStream,
    mut reader: R,
    control: Arc<ProcessControl>,
    max_output_bytes: u64,
    callback: Arc<dyn Fn(ManagedProcessEvent) + Send + Sync>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut bytes = [0_u8; 16 * 1024];
        let mut pending = Vec::new();
        loop {
            let read = match reader.read(&mut bytes) {
                Ok(0) => break,
                Ok(read) => read,
                Err(_) => break,
            };
            let accepted = reserve_output_bytes(&control.output_bytes, read, max_output_bytes);
            pending.extend_from_slice(&bytes[..accepted]);
            emit_complete_utf8(&mut pending, process_id, stream, &callback);
            if accepted < read || control.output_bytes.load(Ordering::Acquire) >= max_output_bytes {
                let _ = control.terminate(STOP_OUTPUT_LIMIT);
                break;
            }
        }
        if !pending.is_empty() {
            callback(ManagedProcessEvent::Output {
                process_id,
                stream,
                chunk: String::from_utf8_lossy(&pending).into_owned(),
            });
        }
    })
}

fn reserve_output_bytes(counter: &AtomicU64, requested: usize, maximum: u64) -> usize {
    let requested = requested as u64;
    loop {
        let current = counter.load(Ordering::Acquire);
        let accepted = requested.min(maximum.saturating_sub(current));
        if accepted == 0 {
            return 0;
        }
        if counter
            .compare_exchange_weak(
                current,
                current.saturating_add(accepted),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            return accepted as usize;
        }
    }
}

fn emit_complete_utf8(
    pending: &mut Vec<u8>,
    process_id: u64,
    stream: ProcessStream,
    callback: &Arc<dyn Fn(ManagedProcessEvent) + Send + Sync>,
) {
    loop {
        match std::str::from_utf8(pending) {
            Ok("") => return,
            Ok(value) => {
                callback(ManagedProcessEvent::Output {
                    process_id,
                    stream,
                    chunk: value.to_owned(),
                });
                pending.clear();
                return;
            }
            Err(error) => {
                let valid_length = error.valid_up_to();
                if valid_length > 0 {
                    let chunk = String::from_utf8(pending[..valid_length].to_vec())
                        .expect("validated UTF-8 prefix");
                    callback(ManagedProcessEvent::Output {
                        process_id,
                        stream,
                        chunk,
                    });
                    pending.drain(..valid_length);
                    continue;
                }
                let Some(invalid_length) = error.error_len() else {
                    return;
                };
                callback(ManagedProcessEvent::Output {
                    process_id,
                    stream,
                    chunk: String::from_utf8_lossy(&pending[..invalid_length]).into_owned(),
                });
                pending.drain(..invalid_length);
            }
        }
    }
}

fn spawn_waiter(
    process_id: u64,
    control: Arc<ProcessControl>,
    output_readers: [thread::JoinHandle<()>; 2],
    max_runtime: Duration,
    callback: Arc<dyn Fn(ManagedProcessEvent) + Send + Sync>,
) {
    thread::spawn(move || {
        let started = Instant::now();
        let mut timeout_requested = false;
        let (exit_code, error) = loop {
            if !timeout_requested && started.elapsed() >= max_runtime {
                timeout_requested = true;
                if let Err(error) = control.terminate(STOP_TIMEOUT) {
                    break (
                        -1,
                        Some(format!(
                            "Could not enforce managed process timeout: {error}"
                        )),
                    );
                }
            }
            let status = match control.child.lock() {
                Ok(mut child) => child.try_wait(),
                Err(_) => {
                    break (
                        -1,
                        Some("Managed process state became unavailable".to_owned()),
                    );
                }
            };
            match status {
                Ok(Some(status)) => {
                    break (status.code().unwrap_or(-1), None);
                }
                Ok(None) => {}
                Err(error) => {
                    break (
                        -1,
                        Some(format!("Could not read managed process status: {error}")),
                    );
                }
            }
            thread::sleep(Duration::from_millis(40));
        };
        for reader in output_readers {
            let _ = reader.join();
        }
        callback(ManagedProcessEvent::Exited {
            process_id,
            exit_code,
            stop_reason: control.stop_reason.load(Ordering::SeqCst),
            error,
        });
    });
}

fn managed_command(command: &str, cwd: &Path) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_SUSPENDED: u32 = 0x0000_0004;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut process = Command::new("powershell.exe");
        process
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                command,
            ])
            .creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW);
        configure_command(&mut process, cwd);
        process
    }
    #[cfg(not(windows))]
    {
        let mut process = Command::new("sh");
        process.args(["-lc", command]);
        configure_command(&mut process, cwd);
        process
    }
}

fn configure_command(command: &mut Command, cwd: &Path) {
    command
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
}

fn canonical_directory(path: &Path) -> Result<PathBuf, String> {
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("Managed process working directory is unavailable: {error}"))?;
    if !canonical.is_dir() {
        return Err("Managed process working directory is not a folder".to_owned());
    }
    Ok(canonical)
}

fn trim_front_chars(value: &mut String, max_chars: usize) -> bool {
    let count = value.chars().count();
    if count <= max_chars {
        return false;
    }
    let remove = count - max_chars;
    let byte_index = value
        .char_indices()
        .nth(remove)
        .map(|(index, _)| index)
        .unwrap_or(value.len());
    value.drain(..byte_index);
    true
}

fn unix_time_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Instant};

    fn collect_until_exit(
        runtime: &mut ManagedProcessRuntime,
        receiver: &mpsc::Receiver<ManagedProcessEvent>,
    ) {
        collect_until_exit_with_timeout(runtime, receiver, Duration::from_secs(10));
    }

    fn collect_until_exit_with_timeout(
        runtime: &mut ManagedProcessRuntime,
        receiver: &mpsc::Receiver<ManagedProcessEvent>,
        timeout: Duration,
    ) {
        let deadline = Instant::now() + timeout;
        let mut output_bytes = 0;
        let mut output_events = 0;
        while Instant::now() < deadline {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => {
                    if let ManagedProcessEvent::Output { chunk, .. } = &event {
                        output_bytes += chunk.len();
                        output_events += 1;
                    }
                    let exited = matches!(event, ManagedProcessEvent::Exited { .. });
                    runtime.handle_event(event);
                    if exited {
                        let mut idle_polls = 0;
                        while idle_polls < 5 {
                            match receiver.recv_timeout(Duration::from_millis(50)) {
                                Ok(event) => {
                                    runtime.handle_event(event);
                                    idle_polls = 0;
                                }
                                Err(mpsc::RecvTimeoutError::Timeout) => idle_polls += 1,
                                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            }
                        }
                        return;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        panic!(
            "managed process did not exit within {timeout:?}; received {output_bytes} bytes in {output_events} output events"
        );
    }

    #[test]
    fn captures_a_complete_managed_process_result() {
        let directory = tempfile::tempdir().unwrap();
        let (sender, receiver) = mpsc::channel();
        let mut runtime = ManagedProcessRuntime::default();
        let result = runtime
            .start(
                if cfg!(windows) {
                    "Write-Output 'central-agent-process-ok'".to_owned()
                } else {
                    "printf central-agent-process-ok".to_owned()
                },
                directory.path().to_path_buf(),
                Some(7),
                Some("test-conversation".to_owned()),
                move |event| {
                    let _ = sender.send(event);
                },
            )
            .unwrap();
        let process_id = result.get("processId").and_then(Value::as_u64).unwrap();
        collect_until_exit(&mut runtime, &receiver);
        let status = runtime.status_value(process_id, 0, 0).unwrap();
        assert_eq!(
            status.get("phase").and_then(Value::as_str),
            Some("completed")
        );
        assert!(
            status
                .get("stdout")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .contains("central-agent-process-ok")
        );
        assert_eq!(status.get("ownerRunId").and_then(Value::as_u64), Some(7));
        assert_eq!(
            runtime.list_value_for_owner("test-conversation")["processes"]
                .as_array()
                .map(Vec::len),
            Some(1)
        );
        assert!(
            runtime.list_value_for_owner("another-conversation")["processes"]
                .as_array()
                .is_some_and(Vec::is_empty)
        );
        assert!(
            runtime
                .status_value_for_owner(process_id, 0, 0, "another-conversation")
                .is_err()
        );
        assert!(
            runtime
                .write_stdin_for_owner(process_id, "", false, "another-conversation")
                .is_err()
        );
        assert!(
            runtime
                .cancel_for_owner(process_id, "another-conversation")
                .is_err()
        );
    }

    #[test]
    fn enforces_the_combined_output_limit_and_reports_truncation() {
        let directory = tempfile::tempdir().unwrap();
        let (sender, receiver) = mpsc::channel();
        let mut runtime = ManagedProcessRuntime::default();
        let result = runtime
            .start_with_limits(
                if cfg!(windows) {
                    "[Console]::Out.Write(('x' * 200000)); Start-Sleep -Seconds 30".to_owned()
                } else {
                    "head -c 200000 /dev/zero | tr '\\0' x; sleep 30".to_owned()
                },
                directory.path().to_path_buf(),
                Some(12),
                Some("output-limit-conversation".to_owned()),
                ManagedProcessLimits {
                    max_runtime: Duration::from_secs(5),
                    max_output_bytes: 4 * 1024,
                    max_stdin_bytes: 1024,
                },
                move |event| {
                    let _ = sender.send(event);
                },
            )
            .unwrap();
        let process_id = result.get("processId").and_then(Value::as_u64).unwrap();
        collect_until_exit(&mut runtime, &receiver);

        let status = runtime.status_value(process_id, 0, 0).unwrap();
        assert_eq!(status["phase"], "failed");
        assert_eq!(status["outputTruncated"], true);
        assert_eq!(status["stdoutSourceBytes"], 4 * 1024);
        assert!(
            status["status"]
                .as_str()
                .is_some_and(|status| status.contains("combined output limit"))
        );
    }

    #[test]
    fn enforces_the_managed_process_runtime_limit() {
        let directory = tempfile::tempdir().unwrap();
        let (sender, receiver) = mpsc::channel();
        let mut runtime = ManagedProcessRuntime::default();
        let result = runtime
            .start_with_limits(
                if cfg!(windows) {
                    "Start-Sleep -Seconds 30".to_owned()
                } else {
                    "sleep 30".to_owned()
                },
                directory.path().to_path_buf(),
                Some(13),
                Some("timeout-conversation".to_owned()),
                ManagedProcessLimits {
                    max_runtime: Duration::from_millis(120),
                    max_output_bytes: 1024,
                    max_stdin_bytes: 1024,
                },
                move |event| {
                    let _ = sender.send(event);
                },
            )
            .unwrap();
        let process_id = result.get("processId").and_then(Value::as_u64).unwrap();
        collect_until_exit(&mut runtime, &receiver);

        let status = runtime.status_value(process_id, 0, 0).unwrap();
        assert_eq!(status["phase"], "failed");
        assert!(
            status["status"]
                .as_str()
                .is_some_and(|status| status.contains("runtime limit"))
        );
    }

    #[test]
    fn enforces_the_managed_process_lifetime_stdin_limit() {
        let directory = tempfile::tempdir().unwrap();
        let (sender, receiver) = mpsc::channel();
        let mut runtime = ManagedProcessRuntime::default();
        let result = runtime
            .start_with_limits(
                if cfg!(windows) {
                    "Start-Sleep -Seconds 30".to_owned()
                } else {
                    "sleep 30".to_owned()
                },
                directory.path().to_path_buf(),
                Some(14),
                Some("stdin-limit-conversation".to_owned()),
                ManagedProcessLimits {
                    max_runtime: Duration::from_secs(5),
                    max_output_bytes: 1024,
                    max_stdin_bytes: 8,
                },
                move |event| {
                    let _ = sender.send(event);
                },
            )
            .unwrap();
        let process_id = result.get("processId").and_then(Value::as_u64).unwrap();
        assert!(runtime.write_stdin(process_id, "12345678", false).is_ok());
        let error = runtime.write_stdin(process_id, "9", false).unwrap_err();
        assert!(error.contains("8-byte lifetime limit"));
        assert_eq!(
            runtime.status_value(process_id, 0, 0).unwrap()["stdinBytes"],
            8
        );
        runtime.cancel(process_id).unwrap();
        collect_until_exit(&mut runtime, &receiver);
    }

    #[test]
    fn captures_output_larger_than_the_old_volume_cutoff() {
        let directory = tempfile::tempdir().unwrap();
        let (sender, receiver) = mpsc::channel();
        let mut runtime = ManagedProcessRuntime::default();
        let result = runtime
            .start(
                if cfg!(windows) {
                    "[Console]::Out.Write(('x' * 600000)); [Console]::Error.Write(('y' * 300000))"
                        .to_owned()
                } else {
                    "head -c 600000 /dev/zero | tr '\\0' x; head -c 300000 /dev/zero | tr '\\0' y >&2"
                        .to_owned()
                },
                directory.path().to_path_buf(),
                Some(11),
                Some("large-output-conversation".to_owned()),
                move |event| {
                    let _ = sender.send(event);
                },
            )
            .unwrap();
        let process_id = result.get("processId").and_then(Value::as_u64).unwrap();

        // This is a capture-integrity test, not a throughput benchmark. Hosted
        // Windows runners also drain and redact these 900 KB in an unoptimized
        // build while other process tests run. Keep the timeout/cancellation
        // tests on their shorter deadlines, but allow this fixture to drain.
        collect_until_exit_with_timeout(&mut runtime, &receiver, Duration::from_secs(60));
        let process = runtime.find(process_id).unwrap();
        assert_eq!(
            process.stdout.live_tail.chars().count(),
            MAX_LIVE_OUTPUT_TAIL_CHARS
        );
        assert!(process.stdout.live_tail_truncated);

        let mut stdout = String::new();
        let mut stderr = String::new();
        let mut expected_stdout_offset = 0;
        let mut expected_stderr_offset = 0;
        let mut status_calls = 0;
        let status = loop {
            let status = runtime
                .status_value(process_id, expected_stdout_offset, expected_stderr_offset)
                .unwrap();
            status_calls += 1;
            assert_eq!(
                status.get("stdoutOffset").and_then(Value::as_u64),
                Some(expected_stdout_offset)
            );
            assert_eq!(
                status.get("stderrOffset").and_then(Value::as_u64),
                Some(expected_stderr_offset)
            );
            stdout.push_str(status.get("stdout").and_then(Value::as_str).unwrap());
            stderr.push_str(status.get("stderr").and_then(Value::as_str).unwrap());
            expected_stdout_offset = status
                .get("stdoutNextOffset")
                .and_then(Value::as_u64)
                .unwrap();
            expected_stderr_offset = status
                .get("stderrNextOffset")
                .and_then(Value::as_u64)
                .unwrap();
            if status.get("hasMore").and_then(Value::as_bool) == Some(false) {
                break status;
            }
        };
        assert_eq!(
            status.get("phase").and_then(Value::as_str),
            Some("completed")
        );
        assert_eq!(stdout.chars().count(), 600_000);
        assert_eq!(stderr.chars().count(), 300_000);
        assert!(status_calls >= 3);
        assert_eq!(expected_stdout_offset, 600_000);
        assert_eq!(expected_stderr_offset, 300_000);
        assert_eq!(
            status.get("outputComplete").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            status.get("outputTruncated").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            status.get("stdoutBytes").and_then(Value::as_u64),
            Some(600_000)
        );
        assert_eq!(
            status.get("stderrBytes").and_then(Value::as_u64),
            Some(300_000)
        );
        assert!(status.get("outputCaptureError").is_some_and(Value::is_null));

        let drained = runtime
            .status_value(process_id, expected_stdout_offset, expected_stderr_offset)
            .unwrap();
        assert_eq!(drained.get("stdout").and_then(Value::as_str), Some(""));
        assert_eq!(
            drained.get("stdoutOffset").and_then(Value::as_u64),
            Some(600_000)
        );
        assert_eq!(
            drained.get("stdoutNextOffset").and_then(Value::as_u64),
            Some(600_000)
        );
        assert_eq!(drained.get("hasMore").and_then(Value::as_bool), Some(false));
    }

    #[test]
    fn bounds_multibyte_live_output_without_truncating_the_spool() {
        let mut output = ProcessOutput::create("test").unwrap();
        let complete = format!(
            "{}è{}",
            "x".repeat(PROCESS_STATUS_CHUNK_BYTES - 1),
            "y".repeat(MAX_LIVE_OUTPUT_TAIL_CHARS + 37)
        );
        output.append(&complete);
        output.finalize();

        assert_eq!(output.live_tail.chars().count(), MAX_LIVE_OUTPUT_TAIL_CHARS);
        assert!(output.live_tail_truncated);
        assert_eq!(output.total_bytes, complete.len() as u64);

        let mut captured = String::new();
        let mut expected_offset = 0;
        let first = output.chunk_at(0).unwrap();
        let retried = output.chunk_at(0).unwrap();
        assert_eq!(first.content, retried.content);
        assert_eq!(first.next_offset, retried.next_offset);
        let (truncated, error) = loop {
            let chunk = output.chunk_at(expected_offset).unwrap();
            assert_eq!(chunk.offset, expected_offset);
            expected_offset = chunk.next_offset;
            captured.push_str(&chunk.content);
            if !chunk.has_more {
                break (chunk.truncated, chunk.error);
            }
        };
        assert_eq!(captured, complete);
        assert!(!truncated);
        assert!(error.is_none());

        let drained = output.chunk_at(expected_offset).unwrap();
        assert!(drained.content.is_empty());
        assert_eq!(drained.offset, complete.len() as u64);
        assert_eq!(drained.next_offset, complete.len() as u64);
        assert!(!drained.has_more);
        assert!(output.chunk_at(complete.len() as u64 + 1).is_err());
    }

    #[test]
    fn redacts_a_secret_split_at_the_provider_page_boundary() {
        let mut output = ProcessOutput::create("boundary-redaction").unwrap();
        let marker = "api_key=";
        let first = format!(
            "{} {}",
            "x".repeat(PROCESS_STATUS_CHUNK_BYTES - marker.len() - 1),
            marker
        );
        assert_eq!(first.len(), PROCESS_STATUS_CHUNK_BYTES);

        output.append(&first);
        let pending = output.chunk_at(0).unwrap();
        assert!(!pending.content.is_empty());
        assert!(!pending.content.contains("sk-private-token-value"));
        assert_eq!(output.source_bytes, PROCESS_STATUS_CHUNK_BYTES as u64);

        output.append("sk-private-token-value\nvisible tail");
        output.finalize();

        let mut captured = String::new();
        let mut offset = 0;
        loop {
            let chunk = output.chunk_at(offset).unwrap();
            captured.push_str(&chunk.content);
            offset = chunk.next_offset;
            if !chunk.has_more {
                break;
            }
        }
        assert!(!captured.contains("sk-private-token-value"));
        assert!(captured.contains("[REDACTED CREDENTIAL]"));
        assert!(captured.ends_with("visible tail"));
        assert_eq!(output.redaction_count, 1);
    }

    #[test]
    fn redacts_quoted_assignment_across_stream_chunks() {
        let mut output = ProcessOutput::create("quoted-redaction").unwrap();
        output.append("PASSWORD=\"correct horse");
        output.append(" battery staple\"\nvisible tail");
        output.finalize();

        let captured = output.chunk_at(0).unwrap().content;
        assert!(!captured.contains("correct"));
        assert!(!captured.contains("horse"));
        assert!(!captured.contains("battery"));
        assert!(!captured.contains("staple"));
        assert!(captured.contains("[REDACTED CREDENTIAL]"));
        assert!(captured.ends_with("visible tail"));
        assert_eq!(output.redaction_count, 1);
    }

    #[test]
    fn redacts_long_url_password_without_buffering_it() {
        let mut output = ProcessOutput::create("url-redaction").unwrap();
        output.append("https://private-user:");
        for _ in 0..8 {
            output.append(&"s".repeat(STREAM_REDACTION_LOOKBEHIND_BYTES));
            assert!(output.redaction_pending.len() <= STREAM_REDACTION_LOOKBEHIND_BYTES);
        }
        output.append("@example.test/path\nvisible");
        output.finalize();

        let captured = output.chunk_at(0).unwrap().content;
        assert!(!captured.contains("private-user"));
        assert!(!captured.contains(&"s".repeat(STREAM_REDACTION_LOOKBEHIND_BYTES)));
        assert!(captured.contains("[REDACTED URL CREDENTIAL]@example.test/path"));
        assert!(captured.ends_with("visible"));
        assert_eq!(output.redaction_count, 1);
    }

    #[test]
    fn preserves_normal_urls_with_ports() {
        let source = "dev http://localhost:5173/ live https://example.com:443/path\n";
        let mut output = ProcessOutput::create("url-port").unwrap();
        output.append("dev http://localhost:");
        output.append("5173/ live https://example.com:443");
        output.append("/path\n");
        output.finalize();

        let captured = output.chunk_at(0).unwrap().content;
        assert_eq!(captured, source);
        assert_eq!(output.redaction_count, 0);
    }

    #[test]
    fn publishes_complete_log_lines_before_process_exit() {
        let mut output = ProcessOutput::create("incremental-line").unwrap();
        output.append("build started\n");

        let page = output.chunk_at(0).unwrap();
        assert_eq!(page.content, "build started\n");
        assert_eq!(page.next_offset, "build started\n".len() as u64);
        assert!(!page.has_more);
    }

    #[test]
    fn streams_multi_megabyte_unterminated_output_with_bounded_lookbehind() {
        let mut output = ProcessOutput::create("unterminated-output").unwrap();
        let source = "x".repeat(3 * 1_024 * 1_024);

        output.append(&source);

        assert!(output.redaction_pending.len() <= STREAM_REDACTION_LOOKBEHIND_BYTES);
        assert_eq!(output.source_bytes, source.len() as u64);
        let streamed = output.chunk_at(0).unwrap();
        assert_eq!(streamed.content.len(), PROCESS_STATUS_CHUNK_BYTES);
        assert_eq!(output.redaction_count, 0);

        output.finalize();
        let mut captured = String::new();
        let mut offset = 0;
        loop {
            let chunk = output.chunk_at(offset).unwrap();
            captured.push_str(&chunk.content);
            offset = chunk.next_offset;
            if !chunk.has_more {
                break;
            }
        }
        assert_eq!(captured, source);
    }

    #[test]
    fn output_reader_preserves_utf8_split_across_reads() {
        let (sender, receiver) = mpsc::channel();
        let callback: Arc<dyn Fn(ManagedProcessEvent) + Send + Sync> = Arc::new(move |event| {
            let _ = sender.send(event);
        });
        let mut pending = Vec::new();
        for byte in "prima è dopo".as_bytes() {
            pending.push(*byte);
            emit_complete_utf8(&mut pending, 4, ProcessStream::Stdout, &callback);
        }
        assert!(pending.is_empty());

        let output = receiver
            .try_iter()
            .filter_map(|event| match event {
                ManagedProcessEvent::Output { chunk, .. } => Some(chunk),
                ManagedProcessEvent::Exited { .. } => None,
            })
            .collect::<String>();
        assert_eq!(output, "prima è dopo");
    }

    #[test]
    fn rejects_unknown_process_references() {
        let mut runtime = ManagedProcessRuntime::default();
        assert!(runtime.status_value(99, 0, 0).is_err());
        assert!(runtime.cancel(99).is_err());
    }

    #[test]
    fn accepts_only_explicit_http_loopback_preview_urls() {
        assert_eq!(
            normalize_local_preview_url("http://localhost:5173/app#state").as_deref(),
            Some("http://localhost:5173/app")
        );
        assert_eq!(
            normalize_local_preview_url("http://127.0.0.1:8080/").as_deref(),
            Some("http://127.0.0.1:8080/")
        );
        assert_eq!(
            normalize_local_preview_url("http://[::1]:3000/").as_deref(),
            Some("http://[::1]:3000/")
        );
        assert!(normalize_local_preview_url("https://localhost:5173/").is_none());
        assert!(normalize_local_preview_url("http://localhost/").is_none());
        assert!(normalize_local_preview_url("http://user@localhost:5173/").is_none());
        assert!(normalize_local_preview_url("http://192.168.1.4:5173/").is_none());
        assert!(normalize_local_preview_url("http://example.com:5173/").is_none());
    }

    #[test]
    fn extracts_deduplicated_local_preview_urls_from_process_output() {
        let urls = extract_local_preview_urls(
            "Local: http://localhost:5173/\nAgain http://localhost:5173/.\nAPI http://127.0.0.1:8080/v1",
        );
        assert_eq!(
            urls,
            vec![
                "http://localhost:5173/".to_owned(),
                "http://127.0.0.1:8080/v1".to_owned(),
            ]
        );
    }

    #[test]
    fn classifies_only_explicit_game_runtime_hints() {
        assert!(looks_like_game_command("cargo run --bin bevy_demo"));
        assert!(looks_like_game_command(".\\game.exe"));
        assert!(!looks_like_game_command("cargo run --bin desktop_editor"));
    }

    #[test]
    fn detects_a_preview_url_emitted_by_a_managed_process() {
        let directory = tempfile::tempdir().unwrap();
        let (sender, receiver) = mpsc::channel();
        let mut runtime = ManagedProcessRuntime::default();
        let result = runtime
            .start(
                if cfg!(windows) {
                    "Write-Output 'Local: http://127.0.0.1:4317/demo'; Start-Sleep -Seconds 30"
                        .to_owned()
                } else {
                    "printf 'Local: http://127.0.0.1:4317/demo'; sleep 30".to_owned()
                },
                directory.path().to_path_buf(),
                None,
                None,
                move |event| {
                    let _ = sender.send(event);
                },
            )
            .unwrap();
        let process_id = result.get("processId").and_then(Value::as_u64).unwrap();

        // Cold PowerShell startup on hosted Windows runners can take several
        // seconds. The loop stops at the first detection, so the generous
        // deadline only matters on slow machines; the child outlives it and is
        // cancelled below.
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if let Ok(event) = receiver.recv_timeout(Duration::from_millis(100)) {
                runtime.handle_event(event);
                if runtime
                    .find(process_id)
                    .is_ok_and(|process| !process.preview_urls.is_empty())
                {
                    break;
                }
            }
        }
        let process = runtime.find(process_id).unwrap();
        assert_eq!(
            process.preview_urls,
            vec!["http://127.0.0.1:4317/demo".to_owned()]
        );
        assert_eq!(
            runtime
                .validated_preview_url(process_id, "http://127.0.0.1:4317/demo#ignored")
                .unwrap(),
            "http://127.0.0.1:4317/demo"
        );
        assert!(
            runtime
                .validated_preview_url(process_id, "http://127.0.0.1:4318/")
                .is_err()
        );
        runtime.cancel(process_id).unwrap();
        collect_until_exit(&mut runtime, &receiver);
        assert!(
            runtime
                .validated_preview_url(process_id, "http://127.0.0.1:4317/demo")
                .is_err()
        );
    }
}
