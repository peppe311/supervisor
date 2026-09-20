//! A desktop-wide, input-transparent indicator for the verified official
//! Computer Use bridge. A prompt or selected skill alone never lights it up.
use std::collections::HashSet;
#[cfg(windows)]
use std::time::{Duration, Instant};

use serde_json::Value;
use winit::monitor::MonitorHandle;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TurnKey {
    thread: String,
    turn: String,
}

#[derive(Default)]
struct FrameTracker {
    turns: HashSet<TurnKey>,
}

impl FrameTracker {
    fn active(&self) -> bool {
        !self.turns.is_empty()
    }

    fn clear(&mut self) {
        self.turns.clear();
    }

    fn notice(&mut self, method: &str, params: &Value) {
        let thread = params.get("threadId").and_then(Value::as_str);
        match method {
            "item/started" | "item/completed" => {
                if official_bridge_call(&params["item"])
                    && let Some(key) = turn_key(params)
                {
                    self.turns.insert(key);
                }
            }
            "turn/completed" => {
                if let Some(key) = turn_key(params) {
                    self.turns.remove(&key);
                } else if let Some(thread) = thread {
                    self.turns.retain(|key| key.thread != thread);
                }
            }
            "turn/started" => {
                // A thread has one live turn. This also releases a stale frame
                // when a prior completion was missed during recovery.
                if let Some(thread) = thread {
                    self.turns.retain(|key| key.thread != thread);
                }
            }
            "thread/closed" | "thread/deleted" | "thread/archived" => {
                if let Some(thread) = thread {
                    self.turns.retain(|key| key.thread != thread);
                }
            }
            "thread/status/changed"
                if matches!(
                    params["status"]["type"].as_str(),
                    Some("idle" | "notLoaded")
                ) =>
            {
                if let Some(thread) = thread {
                    self.turns.retain(|key| key.thread != thread);
                }
            }
            _ => {}
        }
    }
}

fn turn_key(params: &Value) -> Option<TurnKey> {
    Some(TurnKey {
        thread: params.get("threadId")?.as_str()?.to_owned(),
        turn: params
            .get("turnId")
            .and_then(Value::as_str)
            .or_else(|| params["turn"]["id"].as_str())?
            .to_owned(),
    })
}

fn official_bridge_call(item: &Value) -> bool {
    if item["type"] != "mcpToolCall" || item["server"] != "node_repl" || item["tool"] != "js" {
        return false;
    }
    let code = ["code", "js", "script"]
        .iter()
        .find_map(|name| item["arguments"].get(*name).and_then(Value::as_str));
    code.is_some_and(|code| {
        code.contains("@oai/sky") || code.contains("globalThis.sky") || code.contains("sky.")
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScreenRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    outer: i32,
    inner: i32,
}

impl ScreenRect {
    fn from_monitor(monitor: MonitorHandle) -> Option<Self> {
        let position = monitor.position();
        let size = monitor.size();
        let width = i32::try_from(size.width).ok()?;
        let height = i32::try_from(size.height).ok()?;
        if width < 32 || height < 32 {
            return None;
        }
        let scale = monitor.scale_factor().clamp(1.0, 3.0);
        Some(Self {
            x: position.x,
            y: position.y,
            width,
            height,
            outer: (4.0 * scale).round() as i32,
            inner: (2.0 * scale).round() as i32,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Segment {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    light: bool,
}

fn segments(screen: &ScreenRect) -> Vec<Segment> {
    let mut parts = Vec::with_capacity(8);
    ring(&mut parts, screen, 0, screen.outer, false);
    ring(&mut parts, screen, screen.outer, screen.inner, true);
    parts
}

fn ring(parts: &mut Vec<Segment>, screen: &ScreenRect, inset: i32, thickness: i32, light: bool) {
    let w = screen.width - inset * 2;
    let h = screen.height - inset * 2;
    if w <= thickness * 2 || h <= thickness * 2 {
        return;
    }
    let Some(x) = screen.x.checked_add(inset) else {
        return;
    };
    let Some(y) = screen.y.checked_add(inset) else {
        return;
    };
    let Some(right) = x.checked_add(w - thickness) else {
        return;
    };
    let Some(bottom) = y.checked_add(h - thickness) else {
        return;
    };
    parts.extend([
        Segment {
            x,
            y,
            width: w,
            height: thickness,
            light,
        },
        Segment {
            x,
            y: bottom,
            width: w,
            height: thickness,
            light,
        },
        Segment {
            x,
            y: y + thickness,
            width: thickness,
            height: h - thickness * 2,
            light,
        },
        Segment {
            x: right,
            y: y + thickness,
            width: thickness,
            height: h - thickness * 2,
            light,
        },
    ]);
}

#[derive(Default)]
pub(crate) struct ComputerUseFrame {
    tracker: FrameTracker,
    #[cfg(windows)]
    native: Option<NativeFrame>,
    #[cfg(windows)]
    checked_monitors_at: Option<Instant>,
}

impl ComputerUseFrame {
    pub(crate) fn notice(
        &mut self,
        method: &str,
        params: &Value,
        official_runtime_available: bool,
    ) {
        if official_runtime_available {
            self.tracker.notice(method, params);
        } else {
            self.clear();
        }
    }

    pub(crate) fn clear(&mut self) {
        self.tracker.clear();
        #[cfg(windows)]
        {
            self.native.take();
            self.checked_monitors_at = None;
        }
    }

    pub(crate) fn sync(
        &mut self,
        monitors: impl IntoIterator<Item = MonitorHandle>,
    ) -> Result<(), String> {
        #[cfg(not(windows))]
        {
            let _ = monitors;
            return Ok(());
        }
        #[cfg(windows)]
        {
            if !self.tracker.active() {
                self.native.take();
                self.checked_monitors_at = None;
                return Ok(());
            }
            if self.native.is_some()
                && self
                    .checked_monitors_at
                    .is_some_and(|checked| checked.elapsed() < Duration::from_secs(2))
            {
                return Ok(());
            }
            let mut screens = monitors
                .into_iter()
                .filter_map(ScreenRect::from_monitor)
                .collect::<Vec<_>>();
            screens.sort_by_key(|screen| (screen.x, screen.y));
            self.checked_monitors_at = Some(Instant::now());
            if screens.is_empty() {
                return Err("No active screen is available for the Computer Use frame".into());
            }
            if self
                .native
                .as_ref()
                .is_some_and(|frame| frame.screens == screens)
            {
                return Ok(());
            }
            self.native = Some(NativeFrame::show(screens)?);
            Ok(())
        }
    }
}

#[cfg(windows)]
struct NativeFrame {
    screens: Vec<ScreenRect>,
    windows: Vec<windows::Win32::Foundation::HWND>,
}

#[cfg(windows)]
unsafe extern "system" fn frame_window_proc(
    hwnd: windows::Win32::Foundation::HWND,
    message: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, message, wparam, lparam)
    }
}

#[cfg(windows)]
fn frame_window_instance() -> Result<windows::Win32::Foundation::HINSTANCE, String> {
    use std::sync::OnceLock;
    use windows::{
        Win32::{
            Foundation::{COLORREF, HINSTANCE},
            Graphics::Gdi::CreateSolidBrush,
            System::LibraryLoader::GetModuleHandleW,
            UI::WindowsAndMessaging::{RegisterClassExW, WNDCLASSEXW},
        },
        core::w,
    };
    static REGISTERED: OnceLock<Result<(), String>> = OnceLock::new();
    REGISTERED
        .get_or_init(|| {
            let module = unsafe { GetModuleHandleW(None) }.map_err(|error| error.to_string())?;
            let instance = HINSTANCE(module.0);
            for (name, ink) in [
                (w!("SupervisorComputerUseFrameDark"), COLORREF(0x00111111)),
                (w!("SupervisorComputerUseFrameLight"), COLORREF(0x00f7f7f7)),
            ] {
                // Two process-lifetime brushes keep the indicator independent
                // of the user's Windows accent and color settings.
                let brush = unsafe { CreateSolidBrush(ink) };
                if brush.is_invalid() {
                    return Err("Could not create a Computer Use frame brush".into());
                }
                let class = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    lpfnWndProc: Some(frame_window_proc),
                    hInstance: instance,
                    hbrBackground: brush,
                    lpszClassName: name,
                    ..Default::default()
                };
                if unsafe { RegisterClassExW(&class) } == 0 {
                    return Err(windows::core::Error::from_win32().to_string());
                }
            }
            Ok(())
        })
        .clone()?;
    let module = unsafe { GetModuleHandleW(None) }.map_err(|error| error.to_string())?;
    Ok(HINSTANCE(module.0))
}

#[cfg(windows)]
impl NativeFrame {
    fn show(screens: Vec<ScreenRect>) -> Result<Self, String> {
        use windows::{
            Win32::{
                Foundation::COLORREF,
                UI::WindowsAndMessaging::{
                    CreateWindowExW, DestroyWindow, HWND_TOPMOST, LWA_ALPHA, SWP_NOACTIVATE,
                    SWP_SHOWWINDOW, SetLayeredWindowAttributes, SetWindowPos, WS_EX_LAYERED,
                    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
                },
            },
            core::w,
        };

        let instance = frame_window_instance()?;
        let mut frame = Self {
            screens,
            windows: Vec::new(),
        };
        for screen in &frame.screens {
            for segment in segments(screen) {
                let hwnd = unsafe {
                    CreateWindowExW(
                        WS_EX_LAYERED
                            | WS_EX_TRANSPARENT
                            | WS_EX_NOACTIVATE
                            | WS_EX_TOOLWINDOW
                            | WS_EX_TOPMOST,
                        if segment.light {
                            w!("SupervisorComputerUseFrameLight")
                        } else {
                            w!("SupervisorComputerUseFrameDark")
                        },
                        w!(""),
                        WS_POPUP,
                        segment.x,
                        segment.y,
                        segment.width,
                        segment.height,
                        None,
                        None,
                        Some(instance),
                        None,
                    )
                }
                .map_err(|error| error.to_string())?;
                let result = unsafe {
                    SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA).and_then(|_| {
                        SetWindowPos(
                            hwnd,
                            Some(HWND_TOPMOST),
                            segment.x,
                            segment.y,
                            segment.width,
                            segment.height,
                            SWP_NOACTIVATE | SWP_SHOWWINDOW,
                        )
                    })
                };
                if let Err(error) = result {
                    unsafe {
                        let _ = DestroyWindow(hwnd);
                    }
                    return Err(error.to_string());
                }
                frame.windows.push(hwnd);
            }
        }
        Ok(frame)
    }
}

#[cfg(windows)]
impl Drop for NativeFrame {
    fn drop(&mut self) {
        use windows::Win32::UI::WindowsAndMessaging::DestroyWindow;
        for hwnd in self.windows.drain(..) {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sky_item(code: &str) -> Value {
        json!({"type":"mcpToolCall","server":"node_repl","tool":"js","arguments":{"code":code}})
    }

    #[test]
    fn lights_only_for_the_verified_bridge_and_clears_on_completion() {
        let mut tracker = FrameTracker::default();
        tracker.notice("turn/started", &json!({"threadId":"a","turn":{"id":"one"}}));
        tracker.notice(
            "item/started",
            &json!({"threadId":"a","turnId":"one","item":sky_item("console.log(5)")}),
        );
        assert!(!tracker.active());
        tracker.notice("item/started", &json!({"threadId":"a","turnId":"one","item":sky_item("const {sky}=await import('@oai/sky');")}));
        assert!(tracker.active());
        tracker.notice(
            "item/completed",
            &json!({"threadId":"a","turnId":"one","item":sky_item("sky.click({x:1,y:2})")}),
        );
        assert!(tracker.active());
        tracker.notice(
            "turn/completed",
            &json!({"threadId":"a","turn":{"id":"one"}}),
        );
        assert!(!tracker.active());
    }

    #[test]
    fn scoped_turns_survive_unrelated_events_but_not_disconnect_boundaries() {
        let mut tracker = FrameTracker::default();
        for thread in ["main", "graph"] {
            tracker.notice("item/started", &json!({"threadId":thread,"turnId":"run","item":sky_item("globalThis.sky.get_app_state()") }));
        }
        tracker.notice("turn/completed", &json!({"threadId":"main","turnId":"run"}));
        assert!(tracker.active());
        tracker.notice("thread/closed", &json!({"threadId":"graph"}));
        assert!(!tracker.active());
        tracker.notice(
            "item/started",
            &json!({"threadId":"main","turnId":"old","item":sky_item("sky.click()") }),
        );
        tracker.notice(
            "turn/started",
            &json!({"threadId":"main","turn":{"id":"new"}}),
        );
        assert!(!tracker.active());
        tracker.notice(
            "item/started",
            &json!({"threadId":"main","turnId":"new","item":sky_item("sky.click()") }),
        );
        tracker.clear();
        assert!(!tracker.active());

        let mut frame = ComputerUseFrame::default();
        frame.notice(
            "item/started",
            &json!({"threadId":"main","turnId":"new","item":sky_item("sky.click()")}),
            true,
        );
        assert!(frame.tracker.active());
        frame.notice("connection/closed", &json!({}), false);
        assert!(!frame.tracker.active());
    }

    #[test]
    fn two_contrasting_rings_fit_negative_and_high_dpi_monitor_bounds() {
        let screen = ScreenRect {
            x: -2560,
            y: 0,
            width: 2560,
            height: 1440,
            outer: 6,
            inner: 3,
        };
        let parts = segments(&screen);
        assert_eq!(parts.len(), 8);
        assert_eq!(parts.iter().filter(|part| part.light).count(), 4);
        assert!(parts.iter().all(|part| part.width > 0 && part.height > 0));
        assert!(
            parts
                .iter()
                .all(|part| part.x >= screen.x && part.y >= screen.y)
        );
        assert!(
            parts
                .iter()
                .all(|part| part.x + part.width <= 0 && part.y + part.height <= 1440)
        );
        assert_eq!(parts[0].x, -2560);
        assert_eq!(parts[4].x, -2554);
    }

    #[cfg(windows)]
    #[test]
    fn native_frame_keeps_pointer_input_and_foreground_with_other_windows() {
        use windows::Win32::UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetForegroundWindow, GetWindowLongW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
            WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
        };
        let foreground = unsafe { GetForegroundWindow() };
        // Keep this OS probe outside the visible desktop. Its purpose is to
        // verify real Win32 window styles without flashing over user work.
        let screen = ScreenRect {
            x: -30000,
            y: -30000,
            width: 320,
            height: 180,
            outer: 4,
            inner: 2,
        };
        let frame = NativeFrame::show(vec![screen]).unwrap();
        assert_eq!(frame.windows.len(), 8);
        let required = WS_EX_LAYERED.0
            | WS_EX_TRANSPARENT.0
            | WS_EX_NOACTIVATE.0
            | WS_EX_TOOLWINDOW.0
            | WS_EX_TOPMOST.0;
        for hwnd in &frame.windows {
            let style = unsafe { GetWindowLongW(*hwnd, GWL_EXSTYLE) } as u32;
            assert_eq!(style & required, required);
        }
        assert_eq!(unsafe { GetForegroundWindow() }, foreground);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "manual synthetic desktop paint inspection without Computer Use"]
    fn synthetic_desktop_paint_probe() {
        use std::time::{Duration, Instant};
        use windows::{
            Win32::{
                Foundation::COLORREF,
                UI::WindowsAndMessaging::{
                    CreateWindowExW, DestroyWindow, DispatchMessageW, HWND_TOPMOST, LWA_ALPHA, MSG,
                    PM_REMOVE, PeekMessageW, SWP_NOACTIVATE, SWP_SHOWWINDOW,
                    SetLayeredWindowAttributes, SetWindowPos, TranslateMessage, WS_EX_LAYERED,
                    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
                },
            },
            core::w,
        };

        let mut backgrounds = Vec::new();
        let instance = frame_window_instance().unwrap();
        for (x, class) in [
            (80, w!("SupervisorComputerUseFrameLight")),
            (320, w!("SupervisorComputerUseFrameDark")),
        ] {
            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_LAYERED
                        | WS_EX_TRANSPARENT
                        | WS_EX_NOACTIVATE
                        | WS_EX_TOOLWINDOW
                        | WS_EX_TOPMOST,
                    class,
                    w!(""),
                    WS_POPUP,
                    x,
                    80,
                    240,
                    320,
                    None,
                    None,
                    Some(instance),
                    None,
                )
            }
            .unwrap();
            unsafe {
                SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA).unwrap();
                SetWindowPos(
                    hwnd,
                    Some(HWND_TOPMOST),
                    x,
                    80,
                    240,
                    320,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                )
                .unwrap();
            }
            backgrounds.push(hwnd);
        }
        let frame = NativeFrame::show(vec![ScreenRect {
            x: 80,
            y: 80,
            width: 480,
            height: 320,
            outer: 4,
            inner: 2,
        }])
        .unwrap();
        let until = Instant::now() + Duration::from_secs(8);
        while Instant::now() < until {
            let mut message = MSG::default();
            while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(frame);
        for hwnd in backgrounds {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}
