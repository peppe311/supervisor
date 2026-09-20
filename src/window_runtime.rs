use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use serde::Serialize;
use serde_json::{Value, json};

use crate::commands::WindowCommand;

const MAX_WINDOWS: usize = 64;
const WINDOW_REFERENCE_TTL: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowSummary {
    pub id: String,
    pub title: String,
    pub process_name: String,
    pub pid: u32,
    pub owned: bool,
    pub minimized: bool,
    pub maximized: bool,
    pub blocked: bool,
    pub blocked_reason: Option<String>,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowRuntimeView {
    pub available: bool,
    pub generation: u64,
    pub reference_ttl_seconds: u64,
    pub windows: Vec<WindowSummary>,
    pub status: String,
}

struct WindowReference {
    summary: WindowSummary,
    native_token: isize,
}

#[derive(Clone, Debug)]
pub(crate) struct WindowCaptureTarget {
    pub(crate) window_id: String,
    pub(crate) title: String,
    pub(crate) pid: u32,
    pub(crate) owned: bool,
    pub(crate) native_token: isize,
}

#[derive(Clone, Debug)]
pub(crate) struct WindowAutomationTarget {
    pub(crate) window_id: String,
    pub(crate) title: String,
    pub(crate) pid: u32,
    pub(crate) owned: bool,
    pub(crate) native_token: isize,
}

pub(crate) struct WindowRuntime {
    generation: u64,
    refreshed_at: Option<Instant>,
    windows: Vec<WindowReference>,
    status: String,
}

impl Default for WindowRuntime {
    fn default() -> Self {
        Self {
            generation: 0,
            refreshed_at: None,
            windows: Vec::new(),
            status: if cfg!(windows) {
                "Window awareness is ready. References are created only by a fresh scan.".to_owned()
            } else {
                "Window awareness is available only on Windows.".to_owned()
            },
        }
    }
}

impl WindowRuntime {
    pub(crate) fn view(&self) -> WindowRuntimeView {
        WindowRuntimeView {
            available: cfg!(windows),
            generation: self.generation,
            reference_ttl_seconds: WINDOW_REFERENCE_TTL.as_secs(),
            windows: self
                .windows
                .iter()
                .map(|window| window.summary.clone())
                .collect(),
            status: self.status.clone(),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.windows.clear();
        self.refreshed_at = None;
        self.status = "Window references were cleared".to_owned();
    }

    pub(crate) fn refresh(&mut self, owned_pids: &[u32]) -> Result<Value, String> {
        #[cfg(not(windows))]
        {
            let _ = owned_pids;
            return Err("Window awareness is available only on Windows".to_owned());
        }

        #[cfg(windows)]
        {
            ensure_default_input_desktop()?;
            let mut native_windows = enumerate_windows()?;
            let owned = owned_pids.iter().copied().collect::<HashSet<_>>();
            native_windows.sort_by_key(|window| (!owned.contains(&window.pid), window.pid));
            self.generation = self.generation.saturating_add(1);
            let generation = self.generation;
            self.windows = native_windows
                .into_iter()
                .take(MAX_WINDOWS)
                .enumerate()
                .map(|(index, window)| {
                    let blocked_reason = protected_window_reason(
                        &window.title,
                        &window.class_name,
                        &window.process_name,
                    );
                    let blocked = blocked_reason.is_some();
                    let summary = WindowSummary {
                        id: format!("window-{generation}-{}", index + 1),
                        title: if blocked {
                            "Protected window".to_owned()
                        } else {
                            sanitize_label(&window.title, 240)
                        },
                        process_name: if blocked {
                            "Protected process".to_owned()
                        } else {
                            sanitize_label(&window.process_name, 120)
                        },
                        pid: window.pid,
                        owned: owned.contains(&window.pid),
                        minimized: window.minimized,
                        maximized: window.maximized,
                        blocked,
                        blocked_reason: blocked_reason.map(str::to_owned),
                        x: window.x,
                        y: window.y,
                        width: window.width,
                        height: window.height,
                    };
                    WindowReference {
                        summary,
                        native_token: window.native_token,
                    }
                })
                .collect();
            self.refreshed_at = Some(Instant::now());
            self.status = format!(
                "{} top-level window reference(s) available for 15 seconds",
                self.windows.len()
            );
            Ok(json!({
                "generation": generation,
                "referenceTtlSeconds": WINDOW_REFERENCE_TTL.as_secs(),
                "windows": self.windows.iter().map(|window| &window.summary).collect::<Vec<_>>(),
            }))
        }
    }

    pub(crate) fn execute(&mut self, command: WindowCommand) -> Result<Value, String> {
        match command {
            WindowCommand::List => Err("Window list requires an explicit refresh".to_owned()),
            WindowCommand::Focus { window_id } => {
                let target = self.take_target(&window_id)?;
                focus_window(target.native_token)?;
                Ok(action_result(&target.summary, "focused"))
            }
            WindowCommand::MoveResize {
                window_id,
                x,
                y,
                width,
                height,
            } => {
                let target = self.take_target(&window_id)?;
                move_resize_window(target.native_token, x, y, width, height)?;
                Ok(json!({
                    "windowId": target.summary.id,
                    "action": "moved_resized",
                    "bounds": { "x": x, "y": y, "width": width, "height": height },
                }))
            }
            WindowCommand::Minimize { window_id } => {
                let target = self.take_target(&window_id)?;
                minimize_window(target.native_token)?;
                Ok(action_result(&target.summary, "minimized"))
            }
            WindowCommand::Restore { window_id } => {
                let target = self.take_target(&window_id)?;
                restore_window(target.native_token)?;
                Ok(action_result(&target.summary, "restored"))
            }
            WindowCommand::Close { window_id } => {
                let target = self.take_target(&window_id)?;
                close_window(target.native_token)?;
                Ok(action_result(&target.summary, "close_requested"))
            }
        }
    }

    pub(crate) fn take_capture_target(
        &mut self,
        window_id: &str,
    ) -> Result<WindowCaptureTarget, String> {
        let target = self.take_target(window_id)?;
        Ok(WindowCaptureTarget {
            window_id: target.summary.id,
            title: target.summary.title,
            pid: target.summary.pid,
            owned: target.summary.owned,
            native_token: target.native_token,
        })
    }

    pub(crate) fn take_automation_target(
        &mut self,
        window_id: &str,
    ) -> Result<WindowAutomationTarget, String> {
        let target = self.take_target(window_id)?;
        Ok(WindowAutomationTarget {
            window_id: target.summary.id,
            title: target.summary.title,
            pid: target.summary.pid,
            owned: target.summary.owned,
            native_token: target.native_token,
        })
    }

    fn take_target(&mut self, id: &str) -> Result<WindowReference, String> {
        #[cfg(windows)]
        ensure_default_input_desktop()?;
        if self
            .refreshed_at
            .is_none_or(|refreshed| refreshed.elapsed() > WINDOW_REFERENCE_TTL)
        {
            self.clear();
            return Err("Window references expired; list windows again".to_owned());
        }
        let index = self
            .windows
            .iter()
            .position(|window| window.summary.id == id)
            .ok_or_else(|| "Unknown or consumed window reference; list windows again".to_owned())?;
        let target = self.windows.remove(index);
        if target.summary.blocked {
            return Err("Protected windows cannot be controlled".to_owned());
        }
        if !native_window_is_valid(target.native_token) {
            return Err("The selected window no longer exists; list windows again".to_owned());
        }
        Ok(target)
    }
}

#[cfg(windows)]
pub(crate) fn ensure_default_input_desktop() -> Result<(), String> {
    use windows::Win32::{
        Foundation::HANDLE,
        System::StationsAndDesktops::{
            CloseDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_READOBJECTS, DESKTOP_SWITCHDESKTOP,
            GetUserObjectInformationW, OpenInputDesktop, UOI_NAME,
        },
    };

    let desktop = unsafe {
        OpenInputDesktop(
            Default::default(),
            false,
            DESKTOP_ACCESS_FLAGS(DESKTOP_READOBJECTS.0 | DESKTOP_SWITCHDESKTOP.0),
        )
    }
    .map_err(|_| {
        "Computer control is unavailable while Windows is locked or on a secure desktop".to_owned()
    })?;
    let result = (|| {
        let mut needed = 0_u32;
        let _ = unsafe {
            GetUserObjectInformationW(HANDLE(desktop.0), UOI_NAME, None, 0, Some(&mut needed))
        };
        if !(2..=1_024).contains(&needed) {
            return Err("Could not verify the active Windows desktop".to_owned());
        }
        let mut buffer = vec![0_u16; (needed as usize).div_ceil(2)];
        unsafe {
            GetUserObjectInformationW(
                HANDLE(desktop.0),
                UOI_NAME,
                Some(buffer.as_mut_ptr().cast()),
                needed,
                Some(&mut needed),
            )
        }
        .map_err(|_| "Could not verify the active Windows desktop".to_owned())?;
        let end = buffer
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(buffer.len());
        let name = String::from_utf16_lossy(&buffer[..end]);
        if !name.eq_ignore_ascii_case("default") {
            return Err(
                "Computer control is unavailable while Windows is locked or on a secure desktop"
                    .to_owned(),
            );
        }
        Ok(())
    })();
    let _ = unsafe { CloseDesktop(desktop) };
    result
}

#[cfg(windows)]
pub(crate) fn validate_capture_target(target: &WindowCaptureTarget) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetWindowTextW, GetWindowThreadProcessId,
    };

    ensure_default_input_desktop()?;
    if !native_window_is_valid(target.native_token) {
        return Err("The selected preview window no longer exists".to_owned());
    }
    let hwnd = hwnd(target.native_token);
    let mut current_pid = 0_u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut current_pid)) };
    if current_pid == 0 || current_pid != target.pid {
        return Err("The selected preview window identity changed; scan windows again".to_owned());
    }
    let mut title_buffer = [0_u16; 1_024];
    let title_length = unsafe { GetWindowTextW(hwnd, &mut title_buffer) };
    let title = if title_length > 0 {
        String::from_utf16_lossy(&title_buffer[..title_length as usize])
    } else {
        String::new()
    };
    let mut class_buffer = [0_u16; 256];
    let class_length = unsafe { GetClassNameW(hwnd, &mut class_buffer) };
    let class_name = if class_length > 0 {
        String::from_utf16_lossy(&class_buffer[..class_length as usize])
    } else {
        String::new()
    };
    let process_name =
        process_image_name(current_pid).unwrap_or_else(|| format!("PID {current_pid}"));
    if protected_window_reason(&title, &class_name, &process_name).is_some() {
        return Err("Protected windows cannot be captured".to_owned());
    }
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn validate_capture_target(_target: &WindowCaptureTarget) -> Result<(), String> {
    Err("Window capture is available only on Windows".to_owned())
}

#[cfg(not(windows))]
pub(crate) fn ensure_default_input_desktop() -> Result<(), String> {
    Err("Computer control is available only on Windows".to_owned())
}

fn action_result(summary: &WindowSummary, action: &str) -> Value {
    json!({
        "windowId": summary.id,
        "pid": summary.pid,
        "owned": summary.owned,
        "action": action,
    })
}

fn sanitize_label(value: &str, max_chars: usize) -> String {
    let mut output = value
        .chars()
        .filter(|character| !character.is_control())
        .take(max_chars)
        .collect::<String>();
    if value.chars().count() > max_chars {
        output.push('…');
    }
    output
}

fn protected_window_reason(
    title: &str,
    class_name: &str,
    process_name: &str,
) -> Option<&'static str> {
    let title = title.to_ascii_lowercase();
    let class_name = class_name.to_ascii_lowercase();
    let process_name = process_name.to_ascii_lowercase();
    let protected_processes = [
        "credentialuibroker.exe",
        "logonui.exe",
        "lockapp.exe",
        "consent.exe",
        "securityhealthsystray.exe",
    ];
    let protected_titles = [
        "windows security",
        "credential",
        "enter password",
        "enter your pin",
        "sign in",
        "incognito",
        "inprivate",
        "private browsing",
        "1password",
        "bitwarden",
        "keepass",
    ];
    if protected_processes
        .iter()
        .any(|value| process_name.ends_with(value))
        || class_name.contains("credential")
        || protected_titles.iter().any(|value| title.contains(value))
    {
        Some("credential_or_private_surface")
    } else {
        None
    }
}

#[cfg(windows)]
struct NativeWindow {
    native_token: isize,
    title: String,
    class_name: String,
    process_name: String,
    pid: u32,
    minimized: bool,
    maximized: bool,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[cfg(windows)]
fn enumerate_windows() -> Result<Vec<NativeWindow>, String> {
    use windows::{
        Win32::{
            Foundation::{HWND, LPARAM, RECT},
            UI::WindowsAndMessaging::{
                EnumWindows, GetClassNameW, GetWindowRect, GetWindowTextW,
                GetWindowThreadProcessId, IsIconic, IsWindowVisible, IsZoomed,
            },
        },
        core::BOOL,
    };

    unsafe extern "system" fn callback(hwnd: HWND, parameter: LPARAM) -> BOOL {
        let windows = unsafe { &mut *(parameter.0 as *mut Vec<NativeWindow>) };
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return BOOL(1);
        }
        let mut title_buffer = [0_u16; 1_024];
        let title_length = unsafe { GetWindowTextW(hwnd, &mut title_buffer) };
        if title_length <= 0 {
            return BOOL(1);
        }
        let title = String::from_utf16_lossy(&title_buffer[..title_length as usize]);
        if title.trim().is_empty() {
            return BOOL(1);
        }
        let mut pid = 0_u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid == 0 || pid == std::process::id() {
            return BOOL(1);
        }
        let mut class_buffer = [0_u16; 256];
        let class_length = unsafe { GetClassNameW(hwnd, &mut class_buffer) };
        let class_name = if class_length > 0 {
            String::from_utf16_lossy(&class_buffer[..class_length as usize])
        } else {
            String::new()
        };
        let mut bounds = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut bounds) }.is_err() {
            return BOOL(1);
        }
        let width = bounds.right.saturating_sub(bounds.left);
        let height = bounds.bottom.saturating_sub(bounds.top);
        if width <= 1 || height <= 1 {
            return BOOL(1);
        }
        windows.push(NativeWindow {
            native_token: hwnd.0 as isize,
            title,
            class_name,
            process_name: process_image_name(pid).unwrap_or_else(|| format!("PID {pid}")),
            pid,
            minimized: unsafe { IsIconic(hwnd) }.as_bool(),
            maximized: unsafe { IsZoomed(hwnd) }.as_bool(),
            x: bounds.left,
            y: bounds.top,
            width,
            height,
        });
        BOOL(1)
    }

    let mut windows = Vec::new();
    unsafe {
        EnumWindows(
            Some(callback),
            LPARAM((&raw mut windows).cast::<Vec<NativeWindow>>() as isize),
        )
    }
    .map_err(|error| format!("Could not enumerate top-level windows: {error}"))?;
    Ok(windows)
}

#[cfg(windows)]
fn process_image_name(pid: u32) -> Option<String> {
    use windows::{
        Win32::{
            Foundation::CloseHandle,
            System::Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
        },
        core::PWSTR,
    };

    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buffer = vec![0_u16; 1_024];
    let mut size = buffer.len() as u32;
    let result = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
    };
    let _ = unsafe { CloseHandle(process) };
    result.ok()?;
    let path = String::from_utf16_lossy(&buffer[..size as usize]);
    std::path::Path::new(&path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

#[cfg(windows)]
fn hwnd(token: isize) -> windows::Win32::Foundation::HWND {
    windows::Win32::Foundation::HWND(token as *mut core::ffi::c_void)
}

#[cfg(windows)]
fn native_window_is_valid(token: isize) -> bool {
    unsafe { windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(hwnd(token))) }.as_bool()
}

#[cfg(not(windows))]
fn native_window_is_valid(_token: isize) -> bool {
    false
}

#[cfg(windows)]
fn focus_window(token: isize) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{SW_RESTORE, SetForegroundWindow, ShowWindow};
    let hwnd = hwnd(token);
    let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
    if unsafe { SetForegroundWindow(hwnd) }.as_bool() {
        Ok(())
    } else {
        Err("Windows refused to move the selected window to the foreground".to_owned())
    }
}

#[cfg(not(windows))]
fn focus_window(_token: isize) -> Result<(), String> {
    Err("Window control is available only on Windows".to_owned())
}

#[cfg(windows)]
fn move_resize_window(token: isize, x: i32, y: i32, width: i32, height: i32) -> Result<(), String> {
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::MoveWindow(hwnd(token), x, y, width, height, true)
    }
    .map_err(|error| format!("Could not move or resize the selected window: {error}"))
}

#[cfg(not(windows))]
fn move_resize_window(
    _token: isize,
    _x: i32,
    _y: i32,
    _width: i32,
    _height: i32,
) -> Result<(), String> {
    Err("Window control is available only on Windows".to_owned())
}

#[cfg(windows)]
fn minimize_window(token: isize) -> Result<(), String> {
    let _ = unsafe {
        windows::Win32::UI::WindowsAndMessaging::ShowWindow(
            hwnd(token),
            windows::Win32::UI::WindowsAndMessaging::SW_MINIMIZE,
        )
    };
    Ok(())
}

#[cfg(not(windows))]
fn minimize_window(_token: isize) -> Result<(), String> {
    Err("Window control is available only on Windows".to_owned())
}

#[cfg(windows)]
fn restore_window(token: isize) -> Result<(), String> {
    let _ = unsafe {
        windows::Win32::UI::WindowsAndMessaging::ShowWindow(
            hwnd(token),
            windows::Win32::UI::WindowsAndMessaging::SW_RESTORE,
        )
    };
    Ok(())
}

#[cfg(not(windows))]
fn restore_window(_token: isize) -> Result<(), String> {
    Err("Window control is available only on Windows".to_owned())
}

#[cfg(windows)]
fn close_window(token: isize) -> Result<(), String> {
    use windows::Win32::{
        Foundation::{LPARAM, WPARAM},
        UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE},
    };
    unsafe { PostMessageW(Some(hwnd(token)), WM_CLOSE, WPARAM(0), LPARAM(0)) }
        .map_err(|error| format!("Could not request the selected window to close: {error}"))
}

#[cfg(not(windows))]
fn close_window(_token: isize) -> Result<(), String> {
    Err("Window control is available only on Windows".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_window_detection_covers_credentials_and_private_browsing() {
        assert!(protected_window_reason("Windows Security", "Dialog", "app.exe").is_some());
        assert!(
            protected_window_reason("Private tab - Incognito", "Chrome", "chrome.exe").is_some()
        );
        assert!(protected_window_reason("Project", "Editor", "code.exe").is_none());
    }

    #[test]
    fn stale_or_invented_references_are_rejected() {
        let mut runtime = WindowRuntime::default();
        assert!(
            runtime
                .execute(WindowCommand::Focus {
                    window_id: "window-1-999".to_owned()
                })
                .is_err()
        );
    }

    #[cfg(windows)]
    #[test]
    fn enumerates_windows_with_opaque_short_lived_references() {
        let mut runtime = WindowRuntime::default();
        let result = runtime.refresh(&[]).unwrap();
        assert_eq!(
            result.get("referenceTtlSeconds").and_then(Value::as_u64),
            Some(15)
        );
        for window in result
            .get("windows")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let id = window.get("id").and_then(Value::as_str).unwrap();
            assert!(id.starts_with("window-1-"));
            assert!(!id.contains("0x"));
        }
    }
}
