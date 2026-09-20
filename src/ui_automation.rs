use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    commands::{UiAutomationCommand, UiScrollAmount},
    tab_context::sanitize_ui_text,
    window_runtime::WindowAutomationTarget,
};

const UI_REFERENCE_TTL: Duration = Duration::from_secs(12);
const MAX_UI_ELEMENTS: usize = 48;
const MAX_UI_DEPTH: usize = 8;
const MAX_UI_SIBLINGS: usize = 40;
const MAX_UI_NAME_CHARS: usize = 160;
const MAX_UI_AUTOMATION_ID_CHARS: usize = 96;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UiAutomationView {
    pub available: bool,
    pub busy: bool,
    pub target_title: Option<String>,
    pub target_pid: Option<u32>,
    pub target_owned: bool,
    pub element_count: usize,
    pub reference_ttl_seconds: u64,
    pub expires_in_seconds: u64,
    pub status: String,
}

#[derive(Clone, Debug)]
struct UiElementIdentity {
    digest: [u8; 32],
    name: String,
}

#[derive(Clone, Debug)]
struct UiElementReference {
    id: String,
    target: WindowAutomationTarget,
    path: Vec<usize>,
    identity: UiElementIdentity,
}

#[derive(Debug)]
pub(crate) struct UiInspection {
    target: WindowAutomationTarget,
    elements: Vec<UiElementCandidate>,
    truncated: bool,
    redaction_count: usize,
}

#[derive(Debug)]
struct UiElementCandidate {
    path: Vec<usize>,
    depth: usize,
    name: String,
    automation_id: String,
    control_type: i32,
    role: String,
    enabled: bool,
    offscreen: bool,
    keyboard_focusable: bool,
    bounds: UiBounds,
    patterns: Vec<&'static str>,
    identity: UiElementIdentity,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UiBounds {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Debug)]
pub(crate) struct UiElementTarget {
    target: WindowAutomationTarget,
    path: Vec<usize>,
    identity: UiElementIdentity,
}

#[derive(Debug)]
pub(crate) enum UiAutomationTaskResult {
    Inspection(UiInspection),
    Action(Value),
}

#[derive(Debug)]
pub(crate) enum UiElementOperation {
    Focus,
    Invoke,
    SetValue(String),
    Select,
    Expand(bool),
    Scroll {
        horizontal: Option<UiScrollAmount>,
        vertical: Option<UiScrollAmount>,
    },
    TextInput(String),
}

pub(crate) struct UiAutomationRuntime {
    generation: u64,
    refreshed_at: Option<Instant>,
    references: Vec<UiElementReference>,
    target_title: Option<String>,
    target_pid: Option<u32>,
    target_owned: bool,
    busy: bool,
    status: String,
}

impl Default for UiAutomationRuntime {
    fn default() -> Self {
        Self {
            generation: 0,
            refreshed_at: None,
            references: Vec::new(),
            target_title: None,
            target_pid: None,
            target_owned: false,
            busy: false,
            status: if cfg!(windows) {
                "Semantic computer control is ready; inspect a fresh window reference first."
                    .to_owned()
            } else {
                "Semantic computer control is available only on Windows.".to_owned()
            },
        }
    }
}

impl UiAutomationRuntime {
    pub(crate) fn view(&self) -> UiAutomationView {
        let expires_in_seconds = self
            .refreshed_at
            .map(|refreshed| {
                UI_REFERENCE_TTL
                    .saturating_sub(refreshed.elapsed())
                    .as_secs()
            })
            .unwrap_or_default();
        UiAutomationView {
            available: cfg!(windows),
            busy: self.busy,
            target_title: self.target_title.clone(),
            target_pid: self.target_pid,
            target_owned: self.target_owned,
            element_count: self.references.len(),
            reference_ttl_seconds: UI_REFERENCE_TTL.as_secs(),
            expires_in_seconds,
            status: self.status.clone(),
        }
    }

    pub(crate) fn clear(&mut self, status: impl Into<String>) {
        self.references.clear();
        self.refreshed_at = None;
        self.target_title = None;
        self.target_pid = None;
        self.target_owned = false;
        self.busy = false;
        self.status = status.into();
    }

    pub(crate) fn begin_inspection(
        &mut self,
        target: &WindowAutomationTarget,
    ) -> Result<(), String> {
        if self.busy {
            return Err("A semantic computer-control operation is already running".to_owned());
        }
        self.references.clear();
        self.refreshed_at = None;
        self.target_title = Some(target.title.clone());
        self.target_pid = Some(target.pid);
        self.target_owned = target.owned;
        self.busy = true;
        self.status = format!("Inspecting the semantic UI tree for {}…", target.title);
        Ok(())
    }

    pub(crate) fn accept_inspection(&mut self, inspection: UiInspection) -> Value {
        self.busy = false;
        self.generation = self.generation.saturating_add(1);
        let generation = self.generation;
        self.references.clear();
        let mut ids_by_path = HashMap::<Vec<usize>, String>::new();
        for (index, element) in inspection.elements.iter().enumerate() {
            ids_by_path.insert(
                element.path.clone(),
                format!("ui-{generation}-{}", index + 1),
            );
        }

        let elements = inspection
            .elements
            .into_iter()
            .enumerate()
            .map(|(index, element)| {
                let id = format!("ui-{generation}-{}", index + 1);
                let parent_id = element
                    .path
                    .split_last()
                    .and_then(|(_, parent)| ids_by_path.get(parent).cloned());
                self.references.push(UiElementReference {
                    id: id.clone(),
                    target: inspection.target.clone(),
                    path: element.path,
                    identity: element.identity,
                });
                json!({
                    "id": id,
                    "parentId": parent_id,
                    "depth": element.depth,
                    "name": element.name,
                    "automationId": element.automation_id,
                    "role": element.role,
                    "controlType": element.control_type,
                    "enabled": element.enabled,
                    "offscreen": element.offscreen,
                    "keyboardFocusable": element.keyboard_focusable,
                    "bounds": element.bounds,
                    "patterns": element.patterns,
                })
            })
            .collect::<Vec<_>>();

        self.refreshed_at = Some(Instant::now());
        self.target_title = Some(inspection.target.title.clone());
        self.target_pid = Some(inspection.target.pid);
        self.target_owned = inspection.target.owned;
        self.status = format!(
            "{} semantic element reference(s) available for {} seconds",
            self.references.len(),
            UI_REFERENCE_TTL.as_secs()
        );
        json!({
            "windowId": inspection.target.window_id,
            "title": inspection.target.title,
            "pid": inspection.target.pid,
            "owned": inspection.target.owned,
            "generation": generation,
            "referenceTtlSeconds": UI_REFERENCE_TTL.as_secs(),
            "elements": elements,
            "truncated": inspection.truncated,
            "redactionCount": inspection.redaction_count,
        })
    }

    pub(crate) fn is_destructive_reference(&self, element_id: &str) -> bool {
        self.references
            .iter()
            .find(|element| element.id == element_id)
            .is_some_and(|element| destructive_label(&element.identity.name))
    }

    pub(crate) fn take_action(
        &mut self,
        command: UiAutomationCommand,
    ) -> Result<(UiElementTarget, UiElementOperation), String> {
        if self
            .refreshed_at
            .is_none_or(|refreshed| refreshed.elapsed() > UI_REFERENCE_TTL)
        {
            self.clear("Semantic UI references expired; inspect the window again.");
            return Err("Semantic UI references expired; inspect the window again".to_owned());
        }
        if self.busy {
            return Err("A semantic computer-control operation is already running".to_owned());
        }
        let (element_id, operation) = match command {
            UiAutomationCommand::Inspect { .. } => {
                return Err("UI inspection requires a fresh window reference".to_owned());
            }
            UiAutomationCommand::Focus { element_id } => (element_id, UiElementOperation::Focus),
            UiAutomationCommand::Invoke { element_id } => (element_id, UiElementOperation::Invoke),
            UiAutomationCommand::SetValue { element_id, value } => {
                (element_id, UiElementOperation::SetValue(value))
            }
            UiAutomationCommand::Select { element_id } => (element_id, UiElementOperation::Select),
            UiAutomationCommand::Expand {
                element_id,
                expanded,
            } => (element_id, UiElementOperation::Expand(expanded)),
            UiAutomationCommand::Scroll {
                element_id,
                horizontal,
                vertical,
            } => (
                element_id,
                UiElementOperation::Scroll {
                    horizontal,
                    vertical,
                },
            ),
            UiAutomationCommand::TextInput { element_id, text } => {
                (element_id, UiElementOperation::TextInput(text))
            }
        };

        let index = self
            .references
            .iter()
            .position(|element| element.id == element_id)
            .ok_or_else(|| {
                "Unknown or consumed semantic UI reference; inspect the window again".to_owned()
            })?;
        let reference = self.references.remove(index);
        self.references.clear();
        self.refreshed_at = None;
        self.busy = true;
        self.status = format!("Executing a semantic action on {}", reference.identity.name);
        Ok((
            UiElementTarget {
                target: reference.target,
                path: reference.path,
                identity: reference.identity,
            },
            operation,
        ))
    }

    pub(crate) fn record_result(&mut self, ok: bool, message: &str) {
        self.busy = false;
        self.status = if ok {
            format!("{message}; inspect the target again before another action")
        } else {
            format!("{message}; semantic references were cleared")
        };
    }
}

pub(crate) fn inspect_window(target: WindowAutomationTarget) -> Result<UiInspection, String> {
    #[cfg(not(windows))]
    {
        let _ = target;
        Err("Semantic computer control is available only on Windows".to_owned())
    }

    #[cfg(windows)]
    {
        inspect_window_windows(target)
    }
}

pub(crate) fn execute_action(
    target: UiElementTarget,
    operation: UiElementOperation,
) -> Result<Value, String> {
    #[cfg(not(windows))]
    {
        let _ = (target, operation);
        Err("Semantic computer control is available only on Windows".to_owned())
    }

    #[cfg(windows)]
    {
        execute_action_windows(target, operation)
    }
}

#[cfg(windows)]
fn inspect_window_windows(target: WindowAutomationTarget) -> Result<UiInspection, String> {
    crate::window_runtime::ensure_default_input_desktop()?;
    validate_native_target(&target)?;
    let (_com, automation) = create_automation()?;
    let root = unsafe {
        automation.ElementFromHandle(windows::Win32::Foundation::HWND(
            target.native_token as *mut core::ffi::c_void,
        ))
    }
    .map_err(|error| format!("Could not open the target semantic UI tree: {error}"))?;
    validate_element_process(&root, target.pid)?;
    let walker = unsafe { automation.ControlViewWalker() }
        .map_err(|error| format!("Could not create the semantic UI walker: {error}"))?;
    let mut elements = Vec::new();
    let mut redaction_count = 0usize;
    let mut truncated = false;
    visit_element(
        &walker,
        &root,
        Vec::new(),
        0,
        &mut elements,
        &mut redaction_count,
        &mut truncated,
    );
    Ok(UiInspection {
        target,
        elements,
        truncated,
        redaction_count,
    })
}

#[cfg(windows)]
fn execute_action_windows(
    target: UiElementTarget,
    operation: UiElementOperation,
) -> Result<Value, String> {
    use windows::Win32::UI::Accessibility::{
        IUIAutomationElement, IUIAutomationExpandCollapsePattern, IUIAutomationInvokePattern,
        IUIAutomationScrollPattern, IUIAutomationSelectionItemPattern, ScrollAmount_NoAmount,
        UIA_ExpandCollapsePatternId, UIA_InvokePatternId, UIA_ScrollPatternId,
        UIA_SelectionItemPatternId, UIA_ValuePatternId,
    };

    crate::window_runtime::ensure_default_input_desktop()?;
    validate_native_target(&target.target)?;
    let (_com, automation) = create_automation()?;
    let mut element: IUIAutomationElement = unsafe {
        automation.ElementFromHandle(windows::Win32::Foundation::HWND(
            target.target.native_token as *mut core::ffi::c_void,
        ))
    }
    .map_err(|error| format!("Could not reopen the target semantic UI tree: {error}"))?;
    let walker = unsafe { automation.ControlViewWalker() }
        .map_err(|error| format!("Could not create the semantic UI walker: {error}"))?;
    for child_index in &target.path {
        let mut child = unsafe { walker.GetFirstChildElement(&element) }
            .map_err(|_| "The semantic UI tree changed; inspect it again".to_owned())?;
        for _ in 0..*child_index {
            child = unsafe { walker.GetNextSiblingElement(&child) }
                .map_err(|_| "The semantic UI tree changed; inspect it again".to_owned())?;
        }
        element = child;
    }
    validate_element_process(&element, target.target.pid)?;
    let current_identity = element_identity(&element)?;
    if current_identity.digest != target.identity.digest {
        return Err("The semantic UI element changed; inspect the window again".to_owned());
    }
    if unsafe { element.CurrentIsPassword() }
        .map(|value| value.as_bool())
        .unwrap_or(true)
    {
        return Err("Password and credential controls are blocked".to_owned());
    }
    if !unsafe { element.CurrentIsEnabled() }
        .map(|value| value.as_bool())
        .unwrap_or(false)
    {
        return Err("The semantic UI element is disabled".to_owned());
    }

    let (action, length) = match operation {
        UiElementOperation::Focus => {
            unsafe { element.SetFocus() }
                .map_err(|error| format!("Could not focus the semantic UI element: {error}"))?;
            ("focused", None)
        }
        UiElementOperation::Invoke => {
            let pattern: IUIAutomationInvokePattern =
                unsafe { element.GetCurrentPatternAs(UIA_InvokePatternId) }
                    .map_err(|_| "The semantic UI element does not support invoke".to_owned())?;
            unsafe { pattern.Invoke() }
                .map_err(|error| format!("Could not invoke the semantic UI element: {error}"))?;
            ("invoked", None)
        }
        UiElementOperation::SetValue(value) => {
            set_semantic_value(&element, &value, UIA_ValuePatternId)?;
            ("value_set", Some(value.chars().count()))
        }
        UiElementOperation::TextInput(text) => {
            let _ = unsafe { element.SetFocus() };
            set_semantic_value(&element, &text, UIA_ValuePatternId)?;
            ("text_entered", Some(text.chars().count()))
        }
        UiElementOperation::Select => {
            let pattern: IUIAutomationSelectionItemPattern =
                unsafe { element.GetCurrentPatternAs(UIA_SelectionItemPatternId) }
                    .map_err(|_| "The semantic UI element does not support selection".to_owned())?;
            unsafe { pattern.Select() }
                .map_err(|error| format!("Could not select the semantic UI element: {error}"))?;
            ("selected", None)
        }
        UiElementOperation::Expand(expanded) => {
            let pattern: IUIAutomationExpandCollapsePattern =
                unsafe { element.GetCurrentPatternAs(UIA_ExpandCollapsePatternId) }.map_err(
                    |_| "The semantic UI element does not support expand or collapse".to_owned(),
                )?;
            if expanded {
                unsafe { pattern.Expand() }.map_err(|error| {
                    format!("Could not expand the semantic UI element: {error}")
                })?;
                ("expanded", None)
            } else {
                unsafe { pattern.Collapse() }.map_err(|error| {
                    format!("Could not collapse the semantic UI element: {error}")
                })?;
                ("collapsed", None)
            }
        }
        UiElementOperation::Scroll {
            horizontal,
            vertical,
        } => {
            let pattern: IUIAutomationScrollPattern =
                unsafe { element.GetCurrentPatternAs(UIA_ScrollPatternId) }
                    .map_err(|_| "The semantic UI element does not support scrolling".to_owned())?;
            let horizontal = horizontal
                .map(scroll_amount)
                .unwrap_or(ScrollAmount_NoAmount);
            let vertical = vertical.map(scroll_amount).unwrap_or(ScrollAmount_NoAmount);
            unsafe { pattern.Scroll(horizontal, vertical) }
                .map_err(|error| format!("Could not scroll the semantic UI element: {error}"))?;
            ("scrolled", None)
        }
    };

    Ok(json!({
        "elementId": "consumed",
        "windowId": target.target.window_id,
        "pid": target.target.pid,
        "owned": target.target.owned,
        "action": action,
        "length": length,
    }))
}

#[cfg(windows)]
fn set_semantic_value(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
    value: &str,
    pattern_id: windows::Win32::UI::Accessibility::UIA_PATTERN_ID,
) -> Result<(), String> {
    use windows::{Win32::UI::Accessibility::IUIAutomationValuePattern, core::BSTR};

    let pattern: IUIAutomationValuePattern = unsafe { element.GetCurrentPatternAs(pattern_id) }
        .map_err(|_| "The semantic UI element does not support setting a value".to_owned())?;
    if unsafe { pattern.CurrentIsReadOnly() }
        .map(|value| value.as_bool())
        .unwrap_or(true)
    {
        return Err("The semantic UI value is read-only".to_owned());
    }
    let value = BSTR::from(value);
    unsafe { pattern.SetValue(&value) }
        .map_err(|error| format!("Could not set the semantic UI value: {error}"))
}

#[cfg(windows)]
fn visit_element(
    walker: &windows::Win32::UI::Accessibility::IUIAutomationTreeWalker,
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
    path: Vec<usize>,
    depth: usize,
    elements: &mut Vec<UiElementCandidate>,
    redaction_count: &mut usize,
    truncated: &mut bool,
) {
    if elements.len() >= MAX_UI_ELEMENTS {
        *truncated = true;
        return;
    }
    let password = unsafe { element.CurrentIsPassword() }
        .map(|value| value.as_bool())
        .unwrap_or(true);
    if password {
        *redaction_count = redaction_count.saturating_add(1);
    } else if let Ok(candidate) = inspect_element(element, path.clone(), depth, redaction_count) {
        elements.push(candidate);
    }

    if depth >= MAX_UI_DEPTH || elements.len() >= MAX_UI_ELEMENTS {
        *truncated = true;
        return;
    }
    let Ok(mut child) = (unsafe { walker.GetFirstChildElement(element) }) else {
        return;
    };
    for child_index in 0..MAX_UI_SIBLINGS {
        let mut child_path = path.clone();
        child_path.push(child_index);
        visit_element(
            walker,
            &child,
            child_path,
            depth + 1,
            elements,
            redaction_count,
            truncated,
        );
        if elements.len() >= MAX_UI_ELEMENTS {
            *truncated = true;
            return;
        }
        match unsafe { walker.GetNextSiblingElement(&child) } {
            Ok(sibling) => child = sibling,
            Err(_) => return,
        }
    }
    *truncated = true;
}

#[cfg(windows)]
fn inspect_element(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
    path: Vec<usize>,
    depth: usize,
    redaction_count: &mut usize,
) -> Result<UiElementCandidate, String> {
    use windows::Win32::UI::Accessibility::{
        IUIAutomationExpandCollapsePattern, IUIAutomationInvokePattern, IUIAutomationScrollPattern,
        IUIAutomationSelectionItemPattern, IUIAutomationValuePattern, UIA_ExpandCollapsePatternId,
        UIA_InvokePatternId, UIA_ScrollPatternId, UIA_SelectionItemPatternId, UIA_ValuePatternId,
    };

    let raw_name = unsafe { element.CurrentName() }
        .map(|value| value.to_string())
        .unwrap_or_default();
    let (name, count) = sanitize_ui_text(&raw_name, MAX_UI_NAME_CHARS);
    *redaction_count = redaction_count.saturating_add(count);
    let raw_automation_id = unsafe { element.CurrentAutomationId() }
        .map(|value| value.to_string())
        .unwrap_or_default();
    let (automation_id, count) = sanitize_ui_text(&raw_automation_id, MAX_UI_AUTOMATION_ID_CHARS);
    *redaction_count = redaction_count.saturating_add(count);
    let control_type = unsafe { element.CurrentControlType() }
        .map(|value| value.0)
        .unwrap_or_default();
    let role = control_type_name(control_type).to_owned();
    let enabled = unsafe { element.CurrentIsEnabled() }
        .map(|value| value.as_bool())
        .unwrap_or(false);
    let offscreen = unsafe { element.CurrentIsOffscreen() }
        .map(|value| value.as_bool())
        .unwrap_or(true);
    let keyboard_focusable = unsafe { element.CurrentIsKeyboardFocusable() }
        .map(|value| value.as_bool())
        .unwrap_or(false);
    let rectangle = unsafe { element.CurrentBoundingRectangle() }.unwrap_or_default();
    let mut patterns = Vec::new();
    if keyboard_focusable {
        patterns.push("focus");
    }
    if unsafe { element.GetCurrentPatternAs::<IUIAutomationInvokePattern>(UIA_InvokePatternId) }
        .is_ok()
    {
        patterns.push("invoke");
    }
    if unsafe { element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId) }
        .is_ok()
    {
        patterns.extend(["set_value", "text_input"]);
    }
    if unsafe {
        element.GetCurrentPatternAs::<IUIAutomationSelectionItemPattern>(UIA_SelectionItemPatternId)
    }
    .is_ok()
    {
        patterns.push("select");
    }
    if unsafe {
        element
            .GetCurrentPatternAs::<IUIAutomationExpandCollapsePattern>(UIA_ExpandCollapsePatternId)
    }
    .is_ok()
    {
        patterns.push("expand");
    }
    if unsafe { element.GetCurrentPatternAs::<IUIAutomationScrollPattern>(UIA_ScrollPatternId) }
        .is_ok()
    {
        patterns.push("scroll");
    }
    Ok(UiElementCandidate {
        path,
        depth,
        name: if name.is_empty() {
            format!("Unnamed {role}")
        } else {
            name
        },
        automation_id,
        control_type,
        role,
        enabled,
        offscreen,
        keyboard_focusable,
        bounds: UiBounds {
            x: rectangle.left,
            y: rectangle.top,
            width: rectangle.right.saturating_sub(rectangle.left),
            height: rectangle.bottom.saturating_sub(rectangle.top),
        },
        patterns,
        identity: element_identity(element)?,
    })
}

#[cfg(windows)]
fn element_identity(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> Result<UiElementIdentity, String> {
    let name = unsafe { element.CurrentName() }
        .map(|value| value.to_string())
        .unwrap_or_default();
    let automation_id = unsafe { element.CurrentAutomationId() }
        .map(|value| value.to_string())
        .unwrap_or_default();
    let class_name = unsafe { element.CurrentClassName() }
        .map(|value| value.to_string())
        .unwrap_or_default();
    let control_type = unsafe { element.CurrentControlType() }
        .map(|value| value.0)
        .unwrap_or_default();
    let process_id = unsafe { element.CurrentProcessId() }.unwrap_or_default();
    let mut hash = Sha256::new();
    hash.update(process_id.to_le_bytes());
    hash.update(control_type.to_le_bytes());
    hash.update(automation_id.as_bytes());
    hash.update([0]);
    hash.update(class_name.as_bytes());
    hash.update([0]);
    hash.update(name.as_bytes());
    Ok(UiElementIdentity {
        digest: hash.finalize().into(),
        name: sanitize_ui_text(&name, MAX_UI_NAME_CHARS).0,
    })
}

#[cfg(windows)]
fn create_automation()
-> Result<(ComGuard, windows::Win32::UI::Accessibility::IUIAutomation), String> {
    use windows::Win32::{
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
        },
        UI::Accessibility::{CUIAutomation8, IUIAutomation, IUIAutomation2},
    };
    use windows::core::Interface;

    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
        .ok()
        .map_err(|error| format!("Could not initialize semantic computer control: {error}"))?;
    let guard = ComGuard;
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }
            .map_err(|error| format!("Could not start Windows UI Automation: {error}"))?;
    if let Ok(automation2) = automation.cast::<IUIAutomation2>() {
        let _ = unsafe { automation2.SetConnectionTimeout(2_000) };
        let _ = unsafe { automation2.SetTransactionTimeout(2_000) };
    }
    Ok((guard, automation))
}

#[cfg(windows)]
struct ComGuard;

#[cfg(windows)]
impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { windows::Win32::System::Com::CoUninitialize() };
    }
}

#[cfg(windows)]
fn validate_native_target(target: &WindowAutomationTarget) -> Result<(), String> {
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow},
    };
    let hwnd = HWND(target.native_token as *mut core::ffi::c_void);
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return Err("The selected window no longer exists; scan windows again".to_owned());
    }
    let mut pid = 0_u32;
    if unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) } == 0 || pid != target.pid {
        return Err("The selected window identity changed; scan windows again".to_owned());
    }
    Ok(())
}

#[cfg(windows)]
fn validate_element_process(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
    expected_pid: u32,
) -> Result<(), String> {
    let pid = unsafe { element.CurrentProcessId() }
        .map_err(|error| format!("Could not verify the semantic UI process: {error}"))?;
    if pid <= 0 || pid as u32 != expected_pid {
        return Err("The semantic UI element escaped the selected process boundary".to_owned());
    }
    Ok(())
}

#[cfg(windows)]
fn scroll_amount(value: UiScrollAmount) -> windows::Win32::UI::Accessibility::ScrollAmount {
    use windows::Win32::UI::Accessibility::{
        ScrollAmount_LargeDecrement, ScrollAmount_LargeIncrement, ScrollAmount_SmallDecrement,
        ScrollAmount_SmallIncrement,
    };
    match value {
        UiScrollAmount::LargeDecrement => ScrollAmount_LargeDecrement,
        UiScrollAmount::SmallDecrement => ScrollAmount_SmallDecrement,
        UiScrollAmount::SmallIncrement => ScrollAmount_SmallIncrement,
        UiScrollAmount::LargeIncrement => ScrollAmount_LargeIncrement,
    }
}

#[cfg(windows)]
#[allow(non_upper_case_globals)]
fn control_type_name(control_type: i32) -> &'static str {
    use windows::Win32::UI::Accessibility::*;
    match UIA_CONTROLTYPE_ID(control_type) {
        UIA_ButtonControlTypeId => "button",
        UIA_CalendarControlTypeId => "calendar",
        UIA_CheckBoxControlTypeId => "checkbox",
        UIA_ComboBoxControlTypeId => "combobox",
        UIA_EditControlTypeId => "edit",
        UIA_HyperlinkControlTypeId => "link",
        UIA_ImageControlTypeId => "image",
        UIA_ListItemControlTypeId => "list_item",
        UIA_ListControlTypeId => "list",
        UIA_MenuControlTypeId => "menu",
        UIA_MenuBarControlTypeId => "menu_bar",
        UIA_MenuItemControlTypeId => "menu_item",
        UIA_ProgressBarControlTypeId => "progress_bar",
        UIA_RadioButtonControlTypeId => "radio",
        UIA_ScrollBarControlTypeId => "scrollbar",
        UIA_SliderControlTypeId => "slider",
        UIA_SpinnerControlTypeId => "spinner",
        UIA_StatusBarControlTypeId => "status_bar",
        UIA_TabControlTypeId => "tab_list",
        UIA_TabItemControlTypeId => "tab",
        UIA_TextControlTypeId => "text",
        UIA_ToolBarControlTypeId => "toolbar",
        UIA_TreeControlTypeId => "tree",
        UIA_TreeItemControlTypeId => "tree_item",
        UIA_WindowControlTypeId => "window",
        UIA_PaneControlTypeId => "pane",
        UIA_DocumentControlTypeId => "document",
        UIA_GroupControlTypeId => "group",
        UIA_CustomControlTypeId => "custom",
        _ => "control",
    }
}

fn destructive_label(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    [
        "delete",
        "remove",
        "erase",
        "uninstall",
        "factory reset",
        "format disk",
        "close account",
        "permanently",
    ]
    .iter()
    .any(|term| normalized.contains(term))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destructive_semantic_labels_are_classified_locally() {
        assert!(destructive_label("Delete permanently"));
        assert!(destructive_label("Uninstall application"));
        assert!(!destructive_label("Save document"));
    }

    #[test]
    fn semantic_runtime_starts_without_live_references() {
        let runtime = UiAutomationRuntime::default();
        let view = runtime.view();
        assert_eq!(view.element_count, 0);
        assert_eq!(view.reference_ttl_seconds, 12);
    }
}
