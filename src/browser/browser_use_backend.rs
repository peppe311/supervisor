use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::sync::oneshot;
use webview2_com::{
    CallDevToolsProtocolMethodCompletedHandler, DevToolsProtocolEventReceivedEventHandler,
    Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_11, ICoreWebView2DevToolsProtocolEventReceivedEventArgs,
        ICoreWebView2DevToolsProtocolEventReceivedEventArgs2,
    },
};
use windows::{
    Win32::System::Com::CoTaskMemFree,
    core::{HSTRING, Interface, PWSTR},
};
use wry::{WebView, WebViewExtWindows};

use super::{BrowserApp, browser_use_bridge};

const FORWARDED_CDP_EVENTS: &[&str] = &[
    "Accessibility.loadComplete",
    "Accessibility.nodesUpdated",
    "DOM.documentUpdated",
    "Fetch.requestPaused",
    "Log.entryAdded",
    "Network.loadingFailed",
    "Network.loadingFinished",
    "Network.requestWillBeSent",
    "Network.responseReceived",
    "Page.domContentEventFired",
    "Page.fileChooserOpened",
    "Page.frameAttached",
    "Page.frameDetached",
    "Page.frameNavigated",
    "Page.frameStartedLoading",
    "Page.frameStartedNavigating",
    "Page.frameStoppedLoading",
    "Page.javascriptDialogClosed",
    "Page.javascriptDialogOpening",
    "Page.lifecycleEvent",
    "Page.loadEventFired",
    "Page.navigatedWithinDocument",
    "Page.screencastFrame",
    "Page.screencastVisibilityChanged",
    "Runtime.consoleAPICalled",
    "Runtime.bindingCalled",
    "Runtime.executionContextCreated",
    "Runtime.executionContextDestroyed",
    "Runtime.executionContextsCleared",
    "Runtime.exceptionThrown",
    "Target.attachedToTarget",
    "Target.detachedFromTarget",
    "Target.targetCrashed",
    "Target.targetDestroyed",
    "Target.targetInfoChanged",
];
const REQUIRED_CDP_EVENTS: &[&str] = &[
    "Page.frameNavigated",
    "Runtime.executionContextCreated",
    "Target.attachedToTarget",
];
const MAX_CDP_EVENT_BYTES: usize = 8 * 1024 * 1024;

pub(super) struct OwnedTab {
    session_id: String,
    last_turn_id: String,
    marked_turn_id: Option<String>,
    _mark: Option<String>,
}

#[derive(Clone)]
pub(super) struct TabActivity {
    session_id: String,
    turn_id: String,
}

type Reply = oneshot::Sender<Result<Value, String>>;
type SharedReply = Arc<Mutex<Option<Reply>>>;

pub(super) fn install_devtools_events(
    webview: &WebView,
    tab_id: u64,
    bridge: browser_use_bridge::Handle,
) -> Result<(), String> {
    let mut required_failures = Vec::new();
    for event_name in FORWARDED_CDP_EVENTS {
        let result = (|| -> Result<(), String> {
            let receiver = unsafe {
                webview
                    .webview()
                    .GetDevToolsProtocolEventReceiver(&HSTRING::from(*event_name))
            }
            .map_err(|error| error.to_string())?;
            let forwarded_name = (*event_name).to_owned();
            let forwarded_bridge = bridge.clone();
            let handler = DevToolsProtocolEventReceivedEventHandler::create(Box::new(
                move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let Some(params) = read_parameter_json(&args) else {
                        return Ok(());
                    };
                    let session_id = read_session_id(&args);
                    forwarded_bridge.notify_cdp_event(
                        tab_id,
                        session_id.as_deref(),
                        &forwarded_name,
                        params,
                    );
                    Ok(())
                },
            ));
            let mut token = 0_i64;
            unsafe { receiver.add_DevToolsProtocolEventReceived(&handler, &mut token) }
                .map_err(|error| error.to_string())
        })();
        if let Err(error) = result
            && REQUIRED_CDP_EVENTS.contains(event_name)
        {
            required_failures.push(format!("{event_name}: {error}"));
        }
    }
    if !required_failures.is_empty() {
        return Err(format!(
            "required CDP events could not be installed ({})",
            required_failures.join(", ")
        ));
    }
    Ok(())
}

fn read_parameter_json(
    args: &ICoreWebView2DevToolsProtocolEventReceivedEventArgs,
) -> Option<Value> {
    let raw = read_com_string(|output| unsafe { args.ParameterObjectAsJson(output) })?;
    if raw.len() > MAX_CDP_EVENT_BYTES {
        return None;
    }
    serde_json::from_str(&raw).ok()
}

fn read_session_id(args: &ICoreWebView2DevToolsProtocolEventReceivedEventArgs) -> Option<String> {
    let args: ICoreWebView2DevToolsProtocolEventReceivedEventArgs2 = args.cast().ok()?;
    read_com_string(|output| unsafe { args.SessionId(output) }).filter(|value| !value.is_empty())
}

fn read_com_string(getter: impl FnOnce(*mut PWSTR) -> windows::core::Result<()>) -> Option<String> {
    let mut raw = PWSTR::null();
    getter(&mut raw).ok()?;
    let value = unsafe { raw.to_string().ok() };
    if !raw.is_null() {
        unsafe { CoTaskMemFree(Some(raw.0.cast())) };
    }
    value
}

impl BrowserApp {
    pub(super) fn handle_browser_use_bridge_request(
        &mut self,
        request: browser_use_bridge::Request,
    ) {
        let browser_use_bridge::Request {
            method,
            params,
            reply,
        } = request;
        match method.as_str() {
            "ping" => respond(reply, Ok(Value::String("pong".into()))),
            "getInfo" => {
                let codex_session_id = string_param(&params, "session_id").unwrap_or_default();
                respond(
                    reply,
                    Ok(json!({
                        "name": "Supervisor Browser",
                        "version": env!("CARGO_PKG_VERSION"),
                        "type": "iab",
                        "apiSupportOverrides": {
                            "Browser.user": false,
                            "Browser.history": false,
                            "Tab.markDeliverable": true,
                            "Tab.markHandoff": true,
                        },
                        "capabilities": { "browser": [], "tab": [] },
                        "metadata": {
                            "codexAppBuildFlavor": "supervisor",
                            "codexAppSessionId": self.native_session_id,
                            "codexSessionId": codex_session_id,
                        },
                    })),
                );
            }
            "getTabs" | "getUserTabs" => respond(reply, Ok(self.browser_use_tabs())),
            "createTab" => match self.create_tab(None, true) {
                Ok(tab_id) => {
                    self.browser_use_owned_tabs.insert(
                        tab_id,
                        OwnedTab {
                            session_id: string_param(&params, "session_id").unwrap_or_default(),
                            last_turn_id: string_param(&params, "turn_id").unwrap_or_default(),
                            marked_turn_id: None,
                            _mark: None,
                        },
                    );
                    self.record_browser_use_activity(tab_id, &params);
                    respond(reply, self.browser_use_tab(tab_id));
                }
                Err(error) => respond(reply, Err(format!("Could not create a tab: {error}"))),
            },
            "attach" | "detach" | "allowDownload" | "nameSession" => {
                if let Some(tab_id) = tab_id_param(&params) {
                    self.record_browser_use_activity(tab_id, &params);
                }
                respond(reply, Ok(json!({})));
            }
            "markTab" => {
                let result = tab_id_param(&params)
                    .ok_or_else(|| "markTab requires a numeric tabId".to_owned())
                    .and_then(|tab_id| {
                        let turn_id = string_param(&params, "turn_id").unwrap_or_default();
                        let Some(owned) = self.browser_use_owned_tabs.get_mut(&tab_id) else {
                            return Err("Only an agent-created tab can be marked".to_owned());
                        };
                        owned.last_turn_id = turn_id.clone();
                        owned.marked_turn_id = Some(turn_id);
                        owned._mark = string_param(&params, "status");
                        Ok(json!({}))
                    });
                respond(reply, result);
            }
            "turnEnded" => {
                self.finish_browser_use_turn(&params);
                respond(reply, Ok(json!({})));
            }
            "moveMouse" => {
                if let Some(tab_id) = tab_id_param(&params) {
                    self.record_browser_use_activity(tab_id, &params);
                }
                // Browser Use sends the corresponding Input.dispatchMouseEvent
                // through CDP immediately after this optional native pointer hint.
                respond(reply, Ok(json!({})));
            }
            "attachTarget" => {
                let Some(tab_id) = tab_id_param(&params) else {
                    respond(reply, Err("attachTarget requires a numeric tabId".into()));
                    return;
                };
                let Some(target_id) = string_param(&params, "targetId") else {
                    respond(reply, Err("attachTarget requires targetId".into()));
                    return;
                };
                self.record_browser_use_activity(tab_id, &params);
                self.call_browser_cdp(
                    tab_id,
                    None,
                    "Target.attachToTarget".into(),
                    json!({ "targetId": target_id, "flatten": true }),
                    reply,
                );
            }
            "detachTarget" => {
                let Some(tab_id) = tab_id_param(&params) else {
                    respond(reply, Err("detachTarget requires a numeric tabId".into()));
                    return;
                };
                let Some(target_id) = string_param(&params, "targetId") else {
                    respond(reply, Err("detachTarget requires targetId".into()));
                    return;
                };
                self.record_browser_use_activity(tab_id, &params);
                self.call_browser_cdp(
                    tab_id,
                    None,
                    "Target.detachFromTarget".into(),
                    json!({ "targetId": target_id }),
                    reply,
                );
            }
            "executeCdp" => self.execute_browser_cdp(params, reply),
            // The official service detects this exact response and falls back
            // to ordinary executeCdp while keeping its expression cache local.
            "executeCdpWithCachedExpression" => respond(
                reply,
                Err("No handler registered for method: executeCdpWithCachedExpression".into()),
            ),
            _ => respond(
                reply,
                Err(format!("No handler registered for method: {method}")),
            ),
        }
    }

    fn browser_use_tabs(&self) -> Value {
        Value::Array(
            self.tabs
                .iter()
                .filter(|tab| Some(tab.id) != self.remote_desktop_tab_id)
                .map(|tab| self.browser_use_tab_value(tab.id))
                .collect(),
        )
    }

    fn browser_use_tab(&self, tab_id: u64) -> Result<Value, String> {
        self.tabs
            .iter()
            .any(|tab| tab.id == tab_id && Some(tab.id) != self.remote_desktop_tab_id)
            .then(|| self.browser_use_tab_value(tab_id))
            .ok_or_else(|| "The browser tab is no longer available".to_owned())
    }

    fn browser_use_tab_value(&self, tab_id: u64) -> Value {
        let tab = self
            .tabs
            .iter()
            .find(|tab| tab.id == tab_id)
            .expect("browser tab was selected from the current collection");
        json!({
            "id": tab.id,
            "title": tab.title,
            "url": tab.url,
            "active": self.active_tab_id == Some(tab.id)
                || self.pending_tab_activation == Some(tab.id),
        })
    }

    fn record_browser_use_activity(&mut self, tab_id: u64, params: &Value) {
        let Some(session_id) = string_param(params, "session_id") else {
            return;
        };
        let Some(turn_id) = string_param(params, "turn_id") else {
            return;
        };
        self.browser_use_activity.insert(
            tab_id,
            TabActivity {
                session_id,
                turn_id: turn_id.clone(),
            },
        );
        if let Some(owned) = self.browser_use_owned_tabs.get_mut(&tab_id) {
            owned.last_turn_id = turn_id;
        }
    }

    pub(super) fn adopt_browser_use_popup(&mut self, opener_tab_id: u64, tab_id: u64) {
        let Some(activity) = self.browser_use_activity.get(&opener_tab_id).cloned() else {
            return;
        };
        self.browser_use_activity.insert(tab_id, activity.clone());
        self.browser_use_owned_tabs.insert(
            tab_id,
            OwnedTab {
                session_id: activity.session_id,
                last_turn_id: activity.turn_id,
                marked_turn_id: None,
                _mark: None,
            },
        );
    }

    fn finish_browser_use_turn(&mut self, params: &Value) {
        let session_id = string_param(params, "session_id").unwrap_or_default();
        let turn_id = string_param(params, "turn_id").unwrap_or_default();
        let mut close = Vec::new();
        for (&tab_id, owned) in &mut self.browser_use_owned_tabs {
            if owned.session_id != session_id || owned.last_turn_id != turn_id {
                continue;
            }
            if owned.marked_turn_id.as_deref() == Some(turn_id.as_str()) {
                owned.marked_turn_id = None;
                owned._mark = None;
            } else {
                close.push(tab_id);
            }
        }
        for tab_id in close {
            self.close_tab(tab_id);
        }
        self.browser_use_activity
            .retain(|_, activity| activity.session_id != session_id || activity.turn_id != turn_id);
    }

    fn execute_browser_cdp(&mut self, params: Value, reply: Reply) {
        let Some(tab_id) = params
            .get("target")
            .and_then(|target| target.get("tabId"))
            .and_then(numeric_id)
        else {
            respond(reply, Err("executeCdp requires target.tabId".into()));
            return;
        };
        let Some(method) = string_param(&params, "method") else {
            respond(reply, Err("executeCdp requires method".into()));
            return;
        };
        let session_id = params
            .get("target")
            .and_then(|target| target.get("sessionId"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        let command_params = params
            .get("commandParams")
            .cloned()
            .unwrap_or_else(|| json!({}));
        self.record_browser_use_activity(tab_id, &params);

        if matches!(method.as_str(), "Page.close" | "Target.closeTarget") {
            if self.tabs.iter().any(|tab| tab.id == tab_id) {
                self.close_tab(tab_id);
                respond(reply, Ok(json!({ "success": true })));
            } else {
                respond(reply, Err("The browser tab is no longer available".into()));
            }
            return;
        }
        self.call_browser_cdp(tab_id, session_id, method, command_params, reply);
    }

    fn call_browser_cdp(
        &self,
        tab_id: u64,
        session_id: Option<String>,
        method: String,
        command_params: Value,
        reply: Reply,
    ) {
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == tab_id) else {
            respond(reply, Err("The browser tab is no longer available".into()));
            return;
        };
        if Some(tab_id) == self.remote_desktop_tab_id {
            respond(
                reply,
                Err("The remote desktop surface is not a browser tab".into()),
            );
            return;
        }
        let shared = Arc::new(Mutex::new(Some(reply)));
        let callback_reply = Arc::clone(&shared);
        let result_method = method.clone();
        let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(
            move |result, raw_result| {
                let result = result
                    .map_err(|error| error.to_string())
                    .and_then(|()| decode_cdp_result(&result_method, tab_id, &raw_result));
                finish_reply(&callback_reply, result);
                Ok(())
            },
        ));
        let method = HSTRING::from(method);
        let command_params = HSTRING::from(command_params.to_string());
        let call_result = match session_id {
            Some(session_id) => {
                let Ok(webview): Result<ICoreWebView2_11, _> = tab.webview.webview().cast() else {
                    finish_reply(
                        &shared,
                        Err("This WebView2 runtime does not support session-scoped CDP".into()),
                    );
                    return;
                };
                unsafe {
                    webview.CallDevToolsProtocolMethodForSession(
                        &HSTRING::from(session_id),
                        &method,
                        &command_params,
                        &handler,
                    )
                }
            }
            None => unsafe {
                tab.webview
                    .webview()
                    .CallDevToolsProtocolMethod(&method, &command_params, &handler)
            },
        };
        if let Err(error) = call_result {
            finish_reply(&shared, Err(error.to_string()));
        }
    }
}

fn decode_cdp_result(method: &str, tab_id: u64, raw: &str) -> Result<Value, String> {
    let mut value: Value = serde_json::from_str(raw)
        .map_err(|error| format!("WebView2 returned invalid CDP JSON: {error}"))?;
    if method == "Target.getTargets" {
        if let Some(targets) = value.get_mut("targetInfos").and_then(Value::as_array_mut) {
            for target in targets {
                if target.get("type").and_then(Value::as_str) == Some("page") {
                    target["tabId"] = json!(tab_id);
                }
            }
        }
    } else if method == "Target.getTargetInfo"
        && let Some(target) = value.get_mut("targetInfo")
        && target.get("type").and_then(Value::as_str) == Some("page")
    {
        target["tabId"] = json!(tab_id);
    }
    Ok(value)
}

fn tab_id_param(params: &Value) -> Option<u64> {
    params.get("tabId").and_then(numeric_id)
}

fn numeric_id(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
}

fn string_param(params: &Value, name: &str) -> Option<String> {
    params.get(name).and_then(Value::as_str).map(str::to_owned)
}

fn respond(reply: Reply, result: Result<Value, String>) {
    let _ = reply.send(result);
}

fn finish_reply(reply: &SharedReply, result: Result<Value, String>) {
    if let Some(reply) = reply
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
    {
        respond(reply, result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cdp_target_lists_are_bound_to_the_supervisor_tab() {
        let result = decode_cdp_result(
            "Target.getTargets",
            42,
            r#"{"targetInfos":[{"targetId":"page","type":"page"},{"targetId":"worker","type":"worker"}]}"#,
        )
        .unwrap();
        assert_eq!(result["targetInfos"][0]["tabId"], 42);
        assert!(result["targetInfos"][1].get("tabId").is_none());
    }
}
