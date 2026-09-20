use std::{sync::Arc, thread, time::Duration};

use serde::Serialize;

pub(crate) const EMERGENCY_SHORTCUT: &str = "Ctrl+Alt+Esc";

#[derive(Debug)]
pub(crate) enum SafetyEvent {
    HotkeyReady,
    HotkeyUnavailable(String),
    EmergencyStop,
    SecureDesktopEntered,
    DefaultDesktopRestored,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SafetyView {
    pub paused: bool,
    pub secure_desktop: bool,
    pub hotkey_available: bool,
    pub shortcut: &'static str,
    pub status: String,
}

pub(crate) struct SafetyRuntime {
    paused: bool,
    secure_desktop: bool,
    hotkey_available: bool,
    status: String,
}

impl Default for SafetyRuntime {
    fn default() -> Self {
        Self {
            paused: true,
            secure_desktop: false,
            hotkey_available: false,
            status: format!("Registering the {EMERGENCY_SHORTCUT} emergency stop…"),
        }
    }
}

impl SafetyRuntime {
    pub(crate) fn view(&self) -> SafetyView {
        SafetyView {
            paused: self.paused,
            secure_desktop: self.secure_desktop,
            hotkey_available: self.hotkey_available,
            shortcut: EMERGENCY_SHORTCUT,
            status: self.status.clone(),
        }
    }

    pub(crate) fn paused(&self) -> bool {
        self.paused
    }

    pub(crate) fn handle_status_event(&mut self, event: &SafetyEvent) {
        match event {
            SafetyEvent::HotkeyReady => {
                self.hotkey_available = true;
                if !self.secure_desktop && self.status.contains("Registering") {
                    self.paused = false;
                    self.status =
                        format!("Computer control ready · emergency stop {EMERGENCY_SHORTCUT}");
                }
            }
            SafetyEvent::HotkeyUnavailable(message) => {
                self.hotkey_available = false;
                self.paused = true;
                self.status = format!("Computer control blocked: {message}");
            }
            SafetyEvent::SecureDesktopEntered => {
                self.secure_desktop = true;
                self.paused = true;
                self.status =
                    "Computer control stopped because Windows entered a locked or secure desktop."
                        .to_owned();
            }
            SafetyEvent::DefaultDesktopRestored => {
                self.secure_desktop = false;
                self.paused = true;
                self.status = format!(
                    "Windows desktop restored; resume control explicitly. Emergency stop: {EMERGENCY_SHORTCUT}."
                );
            }
            SafetyEvent::EmergencyStop => {
                self.paused = true;
                self.status = format!(
                    "Emergency stop activated with {EMERGENCY_SHORTCUT}; resume explicitly to continue."
                );
            }
        }
    }

    pub(crate) fn resume(&mut self) -> Result<(), String> {
        if !self.hotkey_available {
            return Err("The global emergency shortcut is unavailable".to_owned());
        }
        if self.secure_desktop {
            return Err("Computer control cannot resume on a locked or secure desktop".to_owned());
        }
        crate::window_runtime::ensure_default_input_desktop()?;
        self.paused = false;
        self.status = format!("Computer control ready · emergency stop {EMERGENCY_SHORTCUT}");
        Ok(())
    }
}

pub(crate) fn start<F>(callback: F)
where
    F: Fn(SafetyEvent) + Send + Sync + 'static,
{
    let callback = Arc::new(callback);

    #[cfg(not(windows))]
    callback(SafetyEvent::HotkeyUnavailable(
        "global emergency stop is available only on Windows".to_owned(),
    ));

    #[cfg(windows)]
    {
        start_hotkey_thread(callback.clone());
        start_desktop_monitor(callback);
    }
}

#[cfg(windows)]
fn start_hotkey_thread(callback: Arc<dyn Fn(SafetyEvent) + Send + Sync>) {
    thread::spawn(move || {
        use windows::Win32::UI::{
            Input::KeyboardAndMouse::{
                MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey, VK_ESCAPE,
            },
            WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY},
        };

        const HOTKEY_ID: i32 = 0xCA11;
        let modifiers = MOD_ALT | MOD_CONTROL | MOD_NOREPEAT;
        if let Err(error) =
            unsafe { RegisterHotKey(None, HOTKEY_ID, modifiers, u32::from(VK_ESCAPE.0)) }
        {
            callback(SafetyEvent::HotkeyUnavailable(format!(
                "could not register {EMERGENCY_SHORTCUT}: {error}"
            )));
            return;
        }
        callback(SafetyEvent::HotkeyReady);
        let mut message = MSG::default();
        loop {
            let result = unsafe { GetMessageW(&raw mut message, None, 0, 0) };
            if result.0 <= 0 {
                break;
            }
            if message.message == WM_HOTKEY && message.wParam.0 as i32 == HOTKEY_ID {
                callback(SafetyEvent::EmergencyStop);
            }
        }
        let _ = unsafe { UnregisterHotKey(None, HOTKEY_ID) };
    });
}

#[cfg(windows)]
fn start_desktop_monitor(callback: Arc<dyn Fn(SafetyEvent) + Send + Sync>) {
    thread::spawn(move || {
        let mut default_desktop = crate::window_runtime::ensure_default_input_desktop().is_ok();
        if !default_desktop {
            callback(SafetyEvent::SecureDesktopEntered);
        }
        loop {
            thread::sleep(Duration::from_millis(300));
            let current = crate::window_runtime::ensure_default_input_desktop().is_ok();
            if current == default_desktop {
                continue;
            }
            default_desktop = current;
            callback(if current {
                SafetyEvent::DefaultDesktopRestored
            } else {
                SafetyEvent::SecureDesktopEntered
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emergency_stop_requires_explicit_resume() {
        let mut runtime = SafetyRuntime::default();
        runtime.handle_status_event(&SafetyEvent::HotkeyReady);
        assert!(!runtime.paused());
        runtime.handle_status_event(&SafetyEvent::EmergencyStop);
        assert!(runtime.paused());
        assert!(runtime.view().status.contains(EMERGENCY_SHORTCUT));
    }

    #[test]
    fn unavailable_hotkey_keeps_computer_control_blocked() {
        let mut runtime = SafetyRuntime::default();
        runtime.handle_status_event(&SafetyEvent::HotkeyUnavailable("collision".to_owned()));
        assert!(runtime.paused());
        assert!(!runtime.view().hotkey_available);
    }
}
