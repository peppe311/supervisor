use std::{
    collections::BTreeMap,
    env,
    ffi::{OsStr, OsString, c_void},
    fs::File,
    io,
    mem::{size_of, size_of_val},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, IntoRawHandle, OwnedHandle, RawHandle},
    },
    process::Command,
    ptr,
};
use windows::{
    Win32::{
        Foundation::{
            HANDLE, HANDLE_FLAG_INHERIT, HANDLE_FLAGS, SetHandleInformation, WAIT_FAILED,
            WAIT_OBJECT_0, WAIT_TIMEOUT,
        },
        Security::SECURITY_ATTRIBUTES,
        System::{
            Pipes::CreatePipe,
            Threading::{
                CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessW,
                DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess,
                INFINITE, InitializeProcThreadAttributeList, LPPROC_THREAD_ATTRIBUTE_LIST,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROCESS_INFORMATION, ResumeThread,
                STARTF_USESHOWWINDOW, STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess,
                UpdateProcThreadAttribute, WaitForSingleObject,
            },
        },
        UI::WindowsAndMessaging::SW_HIDE,
    },
    core::{PCWSTR, PWSTR},
};

pub(crate) struct HiddenSpawn {
    pub(crate) child: HiddenChild,
    pub(crate) input: File,
    pub(crate) output: File,
    pub(crate) errors: File,
}

pub(crate) struct HiddenChild {
    process: OwnedHandle,
    primary_thread: Option<OwnedHandle>,
    pid: u32,
    exit_code: Option<u32>,
}

impl HiddenChild {
    pub(crate) fn id(&self) -> u32 {
        self.pid
    }

    pub(crate) fn resume(&mut self) -> io::Result<()> {
        let Some(thread) = self.primary_thread.take() else {
            return Ok(());
        };
        let previous = unsafe { ResumeThread(HANDLE(thread.as_raw_handle())) };
        if previous == u32::MAX {
            return Err(last_windows_error("Could not resume Codex App Server"));
        }
        Ok(())
    }

    pub(crate) fn try_wait(&mut self) -> io::Result<Option<u32>> {
        if let Some(code) = self.exit_code {
            return Ok(Some(code));
        }
        match unsafe { WaitForSingleObject(HANDLE(self.process.as_raw_handle()), 0) } {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let code = process_exit_code(HANDLE(self.process.as_raw_handle()))?;
                self.exit_code = Some(code);
                Ok(Some(code))
            }
            WAIT_FAILED => Err(last_windows_error("Could not read Codex process status")),
            other => Err(io::Error::other(format!(
                "Unexpected Codex process wait result: {}",
                other.0
            ))),
        }
    }

    pub(crate) fn kill(&mut self) -> io::Result<()> {
        if self.try_wait()?.is_some() {
            return Ok(());
        }
        unsafe { TerminateProcess(HANDLE(self.process.as_raw_handle()), 1) }
            .map_err(|error| windows_error("Could not stop Codex App Server", error))
    }

    pub(crate) fn wait(&mut self) -> io::Result<u32> {
        if let Some(code) = self.exit_code {
            return Ok(code);
        }
        match unsafe { WaitForSingleObject(HANDLE(self.process.as_raw_handle()), INFINITE) } {
            WAIT_OBJECT_0 => {
                let code = process_exit_code(HANDLE(self.process.as_raw_handle()))?;
                self.exit_code = Some(code);
                Ok(code)
            }
            WAIT_FAILED => Err(last_windows_error("Could not wait for Codex App Server")),
            other => Err(io::Error::other(format!(
                "Unexpected Codex process wait result: {}",
                other.0
            ))),
        }
    }
}

impl AsRawHandle for HiddenChild {
    fn as_raw_handle(&self) -> RawHandle {
        self.process.as_raw_handle()
    }
}

pub(crate) fn spawn_hidden(command: &Command) -> Result<HiddenSpawn, String> {
    let stdin = anonymous_pipe(true)?;
    let stdout = anonymous_pipe(false)?;
    let stderr = anonymous_pipe(false)?;
    let child_handles = [
        stdin.child_handle(),
        stdout.child_handle(),
        stderr.child_handle(),
    ];

    let mut attribute_bytes = 0usize;
    let _ = unsafe { InitializeProcThreadAttributeList(None, 1, None, &mut attribute_bytes) };
    if attribute_bytes == 0 {
        return Err("Windows did not size the Codex process attribute list".to_owned());
    }
    let word = size_of::<usize>();
    let mut attribute_storage = vec![0usize; attribute_bytes.div_ceil(word)];
    let attributes = LPPROC_THREAD_ATTRIBUTE_LIST(attribute_storage.as_mut_ptr().cast());
    unsafe { InitializeProcThreadAttributeList(Some(attributes), 1, None, &mut attribute_bytes) }
        .map_err(|error| format!("Could not initialize Codex process isolation: {error}"))?;
    let attribute_result = unsafe {
        UpdateProcThreadAttribute(
            attributes,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            Some(child_handles.as_ptr().cast()),
            size_of_val(&child_handles),
            None,
            None,
        )
    };
    if let Err(error) = attribute_result {
        unsafe { DeleteProcThreadAttributeList(attributes) };
        return Err(format!("Could not isolate Codex standard streams: {error}"));
    }

    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES | STARTF_USESHOWWINDOW;
    startup.StartupInfo.wShowWindow = SW_HIDE.0 as u16;
    startup.StartupInfo.hStdInput = child_handles[0];
    startup.StartupInfo.hStdOutput = child_handles[1];
    startup.StartupInfo.hStdError = child_handles[2];

    let executable = wide_z(command.get_program())?;
    let mut command_line = command_line(command)?;
    let mut environment = environment_block(command)?;
    let current_directory = command
        .get_current_dir()
        .map(|path| path.as_os_str().to_os_string())
        .unwrap_or_else(|| env::current_dir().unwrap_or_default().into_os_string());
    let mut current_directory = wide_z(&current_directory)?;
    let mut process = PROCESS_INFORMATION::default();
    let created = unsafe {
        CreateProcessW(
            PCWSTR(executable.as_ptr()),
            Some(PWSTR(command_line.as_mut_ptr())),
            None,
            None,
            true,
            CREATE_NO_WINDOW
                | CREATE_SUSPENDED
                | CREATE_UNICODE_ENVIRONMENT
                | EXTENDED_STARTUPINFO_PRESENT,
            Some(environment.as_mut_ptr().cast::<c_void>()),
            PCWSTR(current_directory.as_mut_ptr()),
            &startup.StartupInfo,
            &mut process,
        )
    };
    unsafe { DeleteProcThreadAttributeList(attributes) };
    created.map_err(|error| format!("Could not start hidden Codex App Server: {error}"))?;

    let process_handle = unsafe { OwnedHandle::from_raw_handle(process.hProcess.0) };
    let thread_handle = unsafe { OwnedHandle::from_raw_handle(process.hThread.0) };
    let PipeEnds {
        child: stdin_child,
        parent: stdin_parent,
    } = stdin;
    let PipeEnds {
        child: stdout_child,
        parent: stdout_parent,
    } = stdout;
    let PipeEnds {
        child: stderr_child,
        parent: stderr_parent,
    } = stderr;
    drop(stdin_child);
    drop(stdout_child);
    drop(stderr_child);

    Ok(HiddenSpawn {
        child: HiddenChild {
            process: process_handle,
            primary_thread: Some(thread_handle),
            pid: process.dwProcessId,
            exit_code: None,
        },
        input: owned_handle_file(stdin_parent),
        output: owned_handle_file(stdout_parent),
        errors: owned_handle_file(stderr_parent),
    })
}

struct PipeEnds {
    child: OwnedHandle,
    parent: OwnedHandle,
}

impl PipeEnds {
    fn child_handle(&self) -> HANDLE {
        HANDLE(self.child.as_raw_handle())
    }
}

fn owned_handle_file(handle: OwnedHandle) -> File {
    unsafe { File::from_raw_handle(handle.into_raw_handle()) }
}

fn anonymous_pipe(child_reads: bool) -> Result<PipeEnds, String> {
    let mut read = HANDLE::default();
    let mut write = HANDLE::default();
    let security = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: ptr::null_mut(),
        bInheritHandle: true.into(),
    };
    unsafe { CreatePipe(&mut read, &mut write, Some(&security), 0) }
        .map_err(|error| format!("Could not create Codex standard stream: {error}"))?;
    let read = unsafe { OwnedHandle::from_raw_handle(read.0) };
    let write = unsafe { OwnedHandle::from_raw_handle(write.0) };
    let (child, parent) = if child_reads {
        (read, write)
    } else {
        (write, read)
    };
    unsafe {
        SetHandleInformation(
            HANDLE(parent.as_raw_handle()),
            HANDLE_FLAG_INHERIT.0,
            HANDLE_FLAGS(0),
        )
    }
    .map_err(|error| format!("Could not isolate a Codex standard stream: {error}"))?;
    Ok(PipeEnds { child, parent })
}

fn command_line(command: &Command) -> Result<Vec<u16>, String> {
    let mut line = Vec::new();
    append_quoted_argument(&mut line, command.get_program())?;
    for argument in command.get_args() {
        line.push(b' ' as u16);
        append_quoted_argument(&mut line, argument)?;
    }
    line.push(0);
    Ok(line)
}

fn append_quoted_argument(target: &mut Vec<u16>, argument: &OsStr) -> Result<(), String> {
    let value = argument.encode_wide().collect::<Vec<_>>();
    if value.contains(&0) {
        return Err("Codex process arguments cannot contain a null character".to_owned());
    }
    let quote = value.is_empty()
        || value
            .iter()
            .any(|character| matches!(*character, 9 | 32 | 34));
    if !quote {
        target.extend_from_slice(&value);
        return Ok(());
    }
    target.push(b'"' as u16);
    let mut backslashes = 0usize;
    for character in value {
        if character == b'\\' as u16 {
            backslashes += 1;
            continue;
        }
        if character == b'"' as u16 {
            target.extend(std::iter::repeat_n(b'\\' as u16, backslashes * 2 + 1));
        } else {
            target.extend(std::iter::repeat_n(b'\\' as u16, backslashes));
        }
        backslashes = 0;
        target.push(character);
    }
    target.extend(std::iter::repeat_n(b'\\' as u16, backslashes * 2));
    target.push(b'"' as u16);
    Ok(())
}

fn environment_block(command: &Command) -> Result<Vec<u16>, String> {
    let mut entries = BTreeMap::<String, (OsString, OsString)>::new();
    for (key, value) in env::vars_os() {
        entries.insert(environment_sort_key(&key), (key, value));
    }
    for (key, value) in command.get_envs() {
        let sort_key = environment_sort_key(key);
        if let Some(value) = value {
            entries.insert(sort_key, (key.to_os_string(), value.to_os_string()));
        } else {
            entries.remove(&sort_key);
        }
    }
    let mut block = Vec::new();
    for (_, (key, value)) in entries {
        let mut pair = key.encode_wide().collect::<Vec<_>>();
        let value = value.encode_wide().collect::<Vec<_>>();
        if pair.contains(&0) || value.contains(&0) {
            return Err("Codex process environment cannot contain a null character".to_owned());
        }
        pair.push(b'=' as u16);
        block.extend(pair);
        block.extend(value);
        block.push(0);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    Ok(block)
}

fn environment_sort_key(key: &OsStr) -> String {
    key.to_string_lossy().to_uppercase()
}

fn wide_z(value: &OsStr) -> Result<Vec<u16>, String> {
    let mut encoded = value.encode_wide().collect::<Vec<_>>();
    if encoded.contains(&0) {
        return Err("Codex process paths cannot contain a null character".to_owned());
    }
    encoded.push(0);
    Ok(encoded)
}

fn process_exit_code(process: HANDLE) -> io::Result<u32> {
    let mut code = 0u32;
    unsafe { GetExitCodeProcess(process, &mut code) }
        .map_err(|error| windows_error("Could not read Codex process exit code", error))?;
    Ok(code)
}

fn windows_error(context: &str, error: windows::core::Error) -> io::Error {
    io::Error::other(format!("{context}: {error}"))
}

fn last_windows_error(context: &str) -> io::Error {
    windows_error(context, windows::core::Error::from_win32())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_argument_quoting_round_trips_common_edge_cases() {
        let cases = [
            ("", "\"\""),
            ("plain", "plain"),
            ("two words", "\"two words\""),
            (r#"say "hello""#, r#""say \"hello\"""#),
            (r#"C:\folder with space\"#, r#""C:\folder with space\\""#),
        ];
        for (input, expected) in cases {
            let mut actual = Vec::new();
            append_quoted_argument(&mut actual, OsStr::new(input)).unwrap();
            assert_eq!(String::from_utf16(&actual).unwrap(), expected);
        }
    }
}
