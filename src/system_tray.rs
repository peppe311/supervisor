use anyhow::{Context, anyhow, ensure};
use windows::{
    Win32::{
        Foundation::{
            CloseHandle, ERROR_ALREADY_EXISTS, ERROR_CLASS_ALREADY_EXISTS, GetLastError, HANDLE,
            HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM,
        },
        System::LibraryLoader::GetModuleHandleW,
        System::Threading::CreateMutexW,
        UI::{
            Shell::{
                NIF_GUID, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE,
                NIM_MODIFY, NIM_SETVERSION, NIN_SELECT, NOTIFYICON_VERSION_4, NOTIFYICONDATAW,
                Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu,
                DestroyWindow, GWLP_USERDATA, GetCursorPos, GetWindowLongPtrW, HWND_BROADCAST,
                LoadIconW, MF_SEPARATOR, MF_STRING, PostMessageW, RegisterClassW,
                RegisterWindowMessageW, SetForegroundWindow, SetMenuDefaultItem, SetWindowLongPtrW,
                TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTALIGN, TPM_RIGHTBUTTON,
                TrackPopupMenu, WM_CONTEXTMENU, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NULL,
                WM_RBUTTONUP, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_OVERLAPPED,
            },
        },
    },
    core::{GUID, PCWSTR, w},
};
use winit::event_loop::EventLoopProxy;

use crate::browser::BrowserEvent;

const CALLBACK_MESSAGE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 0x4a;
const TRAY_ICON_ID: u32 = 1;
const OPEN_COMMAND_ID: usize = 1;
const QUIT_COMMAND_ID: usize = 2;
const NIN_KEYSELECT: u32 = NIN_SELECT | 1;
const TRAY_ICON_GUID: GUID = GUID::from_u128(0x830913cc_c562_49ab_8ea3_57a9771a63b1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Action {
    Open,
    StopAllAndQuit,
    Recreate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MenuPosition {
    x: i32,
    y: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NotificationAction {
    Open,
    ShowContextMenu(Option<MenuPosition>),
}

struct TrayWindowState {
    proxy: EventLoopProxy<BrowserEvent>,
    taskbar_created: u32,
    open_existing: u32,
    stop_all_and_quit: u32,
}

pub(crate) struct SingleInstanceGuard(HANDLE);

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

pub(crate) fn acquire_single_instance() -> anyhow::Result<Option<SingleInstanceGuard>> {
    // The object disappears automatically when Supervisor exits or crashes. A
    // second launch never reaches application storage; it asks the live process
    // to restore its windows and exits successfully instead.
    let created = unsafe {
        CreateMutexW(
            None,
            false,
            w!("Local\\Supervisor.CentralAgent.SingleInstance"),
        )
    };
    let creation_status = unsafe { GetLastError() };
    let mutex = created.context("Supervisor's single-instance guard is unavailable")?;
    if creation_status == ERROR_ALREADY_EXISTS {
        let _ = unsafe { CloseHandle(mutex) };
        request_existing_instance_open()?;
        return Ok(None);
    }
    Ok(Some(SingleInstanceGuard(mutex)))
}

fn request_existing_instance_open() -> anyhow::Result<()> {
    let message = unsafe { RegisterWindowMessageW(w!("Supervisor.OpenExistingInstance")) };
    ensure!(
        message != 0,
        "the existing Supervisor instance could not be addressed"
    );
    unsafe { PostMessageW(Some(HWND_BROADCAST), message, WPARAM(0), LPARAM(0)) }
        .context("the existing Supervisor instance could not be restored")?;
    Ok(())
}

fn create_tray_window(
    proxy: EventLoopProxy<BrowserEvent>,
) -> anyhow::Result<(HWND, Box<TrayWindowState>)> {
    let module = unsafe { GetModuleHandleW(None) }.context("executable module is unavailable")?;
    let instance = HINSTANCE(module.0);
    register_tray_window_class(instance)?;

    let taskbar_created = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
    let open_existing = unsafe { RegisterWindowMessageW(w!("Supervisor.OpenExistingInstance")) };
    let stop_all_and_quit = unsafe { RegisterWindowMessageW(w!("Supervisor.StopAllWorkAndQuit")) };
    ensure!(
        taskbar_created != 0 && open_existing != 0 && stop_all_and_quit != 0,
        "Supervisor's tray messages could not be registered"
    );
    let state = Box::new(TrayWindowState {
        proxy,
        taskbar_created,
        open_existing,
        stop_all_and_quit,
    });
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
            w!("SupervisorSystemTrayWindow"),
            w!("Supervisor background service"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance),
            None,
        )
    }
    .context("Supervisor's dedicated tray receiver could not be created")?;
    let state_pointer = state.as_ref() as *const TrayWindowState as isize;
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_pointer);
    }
    if unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } != state_pointer {
        let _ = unsafe { DestroyWindow(hwnd) };
        return Err(anyhow!(
            "Supervisor's dedicated tray receiver could not store its event target"
        ));
    }
    Ok((hwnd, state))
}

fn register_tray_window_class(instance: HINSTANCE) -> anyhow::Result<()> {
    let class = WNDCLASSW {
        lpfnWndProc: Some(tray_window_proc),
        hInstance: instance,
        lpszClassName: w!("SupervisorSystemTrayWindow"),
        ..Default::default()
    };
    if unsafe { RegisterClassW(&class) } == 0 {
        let status = unsafe { GetLastError() };
        ensure!(
            status == ERROR_CLASS_ALREADY_EXISTS,
            "Supervisor's dedicated tray receiver class could not be registered: {}",
            windows::core::Error::from_win32()
        );
    }
    Ok(())
}

unsafe extern "system" fn tray_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state_pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const TrayWindowState;
    if !state_pointer.is_null() {
        let state = unsafe { &*state_pointer };
        if message == state.open_existing {
            send_action(&state.proxy, Action::Open);
            return LRESULT(0);
        }
        if message == state.stop_all_and_quit {
            send_action(&state.proxy, Action::StopAllAndQuit);
            return LRESULT(0);
        }
        if message == state.taskbar_created {
            send_action(&state.proxy, Action::Recreate);
            return LRESULT(0);
        }
        if message == CALLBACK_MESSAGE {
            match notification_action(wparam, lparam) {
                Some(NotificationAction::Open) => send_action(&state.proxy, Action::Open),
                Some(NotificationAction::ShowContextMenu(position)) => {
                    match show_context_menu(hwnd, position) {
                        Ok(Some(action)) => send_action(&state.proxy, action),
                        Ok(None) => {}
                        Err(error) => {
                            tracing::warn!(%error, "system tray menu could not be opened");
                        }
                    }
                }
                None => {}
            }
            return LRESULT(0);
        }
    }

    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn notification_action(wparam: WPARAM, lparam: LPARAM) -> Option<NotificationAction> {
    let packed = lparam.0 as u32;
    let version_four = packed >> 16 == TRAY_ICON_ID;
    let legacy = wparam.0 as u32 == TRAY_ICON_ID;
    if !version_four && !legacy {
        return None;
    }

    let notification = if version_four {
        packed & 0xffff
    } else {
        packed
    };
    match notification {
        WM_LBUTTONUP | WM_LBUTTONDBLCLK | NIN_SELECT | NIN_KEYSELECT => {
            Some(NotificationAction::Open)
        }
        WM_RBUTTONUP | WM_CONTEXTMENU => Some(NotificationAction::ShowContextMenu(
            version_four.then(|| menu_position(wparam)).flatten(),
        )),
        _ => None,
    }
}

fn menu_position(wparam: WPARAM) -> Option<MenuPosition> {
    let packed = wparam.0 as u32;
    let x = (packed as u16 as i16) as i32;
    let y = ((packed >> 16) as u16 as i16) as i32;
    // Windows reports (-1, -1) when the context menu was requested from
    // the keyboard. In that case the live cursor is the useful fallback.
    (x != -1 || y != -1).then_some(MenuPosition { x, y })
}

fn send_action(proxy: &EventLoopProxy<BrowserEvent>, action: Action) {
    let _ = proxy.send_event(BrowserEvent::SystemTray(action));
}

pub(crate) struct SystemTray {
    data: NOTIFYICONDATAW,
    _window_state: Box<TrayWindowState>,
}

impl SystemTray {
    pub(crate) fn new(proxy: EventLoopProxy<BrowserEvent>) -> anyhow::Result<Self> {
        let module =
            unsafe { GetModuleHandleW(None) }.context("executable module is unavailable")?;
        let icon = unsafe {
            LoadIconW(
                Some(HINSTANCE(module.0)),
                PCWSTR(TRAY_ICON_ID as usize as *const u16),
            )
        }
        .context("embedded Supervisor tray icon is unavailable")?;
        let (hwnd, window_state) = create_tray_window(proxy)?;

        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP | NIF_GUID,
            uCallbackMessage: CALLBACK_MESSAGE,
            hIcon: icon,
            guidItem: TRAY_ICON_GUID,
            ..Default::default()
        };
        set_tip(&mut data, "Supervisor — running in background");
        let mut tray = Self {
            data,
            _window_state: window_state,
        };
        tray.add()?;
        Ok(tray)
    }

    pub(crate) fn recreate(&mut self) -> anyhow::Result<()> {
        self.add()
    }

    pub(crate) fn set_stopping(&mut self) -> anyhow::Result<()> {
        set_tip(&mut self.data, "Supervisor — stopping all work…");
        ensure!(
            unsafe { Shell_NotifyIconW(NIM_MODIFY, &self.data) }.as_bool(),
            "Windows could not update the Supervisor tray icon"
        );
        Ok(())
    }

    fn add(&mut self) -> anyhow::Result<()> {
        // A stable GUID lets a new Supervisor process replace any notification
        // icon left behind by an interrupted older process instead of creating
        // a dead duplicate that cannot receive clicks.
        let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &self.data) };
        ensure!(
            unsafe { Shell_NotifyIconW(NIM_ADD, &self.data) }.as_bool(),
            "Windows could not add the Supervisor tray icon"
        );
        let mut version = self.data;
        version.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        ensure!(
            unsafe { Shell_NotifyIconW(NIM_SETVERSION, &version) }.as_bool(),
            "Windows could not enable the current tray behavior"
        );
        Ok(())
    }
}

impl Drop for SystemTray {
    fn drop(&mut self) {
        let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &self.data) };
        unsafe {
            SetWindowLongPtrW(self.data.hWnd, GWLP_USERDATA, 0);
        }
        let _ = unsafe { DestroyWindow(self.data.hWnd) };
    }
}

fn show_context_menu(hwnd: HWND, position: Option<MenuPosition>) -> anyhow::Result<Option<Action>> {
    let menu = unsafe { CreatePopupMenu() }.context("tray menu could not be created")?;
    let open = wide("Open Supervisor");
    let quit = wide("Close Supervisor");
    let result = (|| -> windows::core::Result<Option<Action>> {
        unsafe {
            AppendMenuW(menu, MF_STRING, OPEN_COMMAND_ID, PCWSTR(open.as_ptr()))?;
            AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null())?;
            AppendMenuW(menu, MF_STRING, QUIT_COMMAND_ID, PCWSTR(quit.as_ptr()))?;
            SetMenuDefaultItem(menu, OPEN_COMMAND_ID as u32, 0)?;

            let position = match position {
                Some(position) => position,
                None => {
                    let mut cursor = POINT::default();
                    GetCursorPos(&mut cursor)?;
                    MenuPosition {
                        x: cursor.x,
                        y: cursor.y,
                    }
                }
            };
            let _ = SetForegroundWindow(hwnd);
            let command = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | TPM_RIGHTALIGN | TPM_BOTTOMALIGN,
                position.x,
                position.y,
                None,
                hwnd,
                None,
            )
            .0 as usize;
            let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
            Ok(match command {
                OPEN_COMMAND_ID => Some(Action::Open),
                QUIT_COMMAND_ID => Some(Action::StopAllAndQuit),
                _ => None,
            })
        }
    })();
    let _ = unsafe { DestroyMenu(menu) };
    result.context("tray menu could not be shown")
}

fn set_tip(data: &mut NOTIFYICONDATAW, value: &str) {
    data.szTip.fill(0);
    let limit = data.szTip.len().saturating_sub(1);
    for (slot, unit) in data.szTip.iter_mut().take(limit).zip(value.encode_utf16()) {
        *slot = unit;
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_labels_are_nul_terminated() {
        for label in [wide("Open Supervisor"), wide("Close Supervisor")] {
            assert_eq!(label.last(), Some(&0));
            assert_eq!(label.iter().filter(|unit| **unit == 0).count(), 1);
        }
    }

    #[test]
    fn tooltip_is_bounded_and_nul_terminated() {
        let mut data = NOTIFYICONDATAW::default();
        set_tip(&mut data, &"x".repeat(256));
        assert_eq!(data.szTip[127], 0);
        assert_eq!(
            data.szTip[..127].iter().filter(|unit| **unit == 0).count(),
            0
        );
    }

    #[test]
    fn version_four_right_click_preserves_signed_screen_coordinates() {
        let x = -120_i16;
        let y = 940_i16;
        let coordinates = (u16::from_ne_bytes(x.to_ne_bytes()) as u32)
            | ((u16::from_ne_bytes(y.to_ne_bytes()) as u32) << 16);
        let callback = (TRAY_ICON_ID << 16) | WM_CONTEXTMENU;

        assert_eq!(
            notification_action(WPARAM(coordinates as usize), LPARAM(callback as isize)),
            Some(NotificationAction::ShowContextMenu(Some(MenuPosition {
                x: -120,
                y: 940
            })))
        );
    }

    #[test]
    fn legacy_right_click_uses_the_live_cursor_position() {
        assert_eq!(
            notification_action(WPARAM(TRAY_ICON_ID as usize), LPARAM(WM_RBUTTONUP as isize)),
            Some(NotificationAction::ShowContextMenu(None))
        );
    }

    #[test]
    fn keyboard_context_menu_uses_the_live_cursor_position() {
        let callback = (TRAY_ICON_ID << 16) | WM_CONTEXTMENU;
        assert_eq!(
            notification_action(WPARAM(u32::MAX as usize), LPARAM(callback as isize)),
            Some(NotificationAction::ShowContextMenu(None))
        );
    }
}
