//! Read-only native hook inventory and lifecycle projection. Supervisor never
//! executes, retries, edits or reconstructs a hook handler.
use super::*;
use crate::tab_context::sanitize_terminal_snapshot_text;

const MAX_HOOKS: usize = 512;
const MAX_RUNS: usize = 100;
type ParsedInventory = (Vec<HookView>, Vec<String>, Vec<String>);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HookView {
    key: String,
    event_name: String,
    matcher: Option<String>,
    status_message: Option<String>,
    source_path: String,
    source: String,
    handler_type: String,
    enabled: bool,
    managed: bool,
    trust_status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunView {
    id: String,
    event_name: String,
    handler_type: String,
    execution_mode: String,
    scope: String,
    source: String,
    status: String,
    status_message: Option<String>,
    started_at: String,
    completed_at: Option<String>,
    duration_ms: Option<String>,
    entries: Vec<RunEntry>,
}

#[derive(Clone, Debug, Serialize)]
struct RunEntry {
    kind: String,
    text: String,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct View {
    thread_id: String,
    directory: PathBuf,
    view_id: Option<String>,
    hooks: Vec<HookView>,
    warnings: Vec<String>,
    errors: Vec<String>,
    runs: Vec<RunView>,
    current: bool,
    busy: bool,
    error: Option<String>,
    notice: Option<String>,
    #[serde(skip)]
    pending: Option<String>,
}

#[derive(Default)]
pub(super) struct Hooks {
    views: BTreeMap<String, View>,
}

impl Hooks {
    pub(super) fn view(&self, owner: &str, thread: &str) -> Option<&View> {
        self.views
            .get(owner)
            .filter(|view| view.thread_id == thread)
    }

    fn refresh(
        &mut self,
        owner: &str,
        thread: &str,
        directory: PathBuf,
    ) -> Result<(String, Call), String> {
        if self
            .views
            .get(owner)
            .is_some_and(|view| view.pending.is_some())
        {
            return Err("A native hook inventory request is already pending".into());
        }
        let cwd = directory
            .to_str()
            .ok_or("The hook inventory directory must be UTF-8")?
            .to_owned();
        let id = Uuid::new_v4().to_string();
        let runs = self
            .views
            .remove(owner)
            .filter(|view| view.thread_id == thread && view.directory == directory)
            .map(|view| view.runs)
            .unwrap_or_default();
        self.views.insert(
            owner.into(),
            View {
                thread_id: thread.into(),
                directory,
                runs,
                busy: true,
                pending: Some(id.clone()),
                ..View::default()
            },
        );
        Ok((id, api::hooks_list(&[cwd])))
    }

    fn reply(&mut self, owner: &str, thread: &str, id: &str, result: Result<Value, CallError>) {
        let Some(view) = self
            .views
            .get_mut(owner)
            .filter(|view| view.thread_id == thread && view.pending.as_deref() == Some(id))
        else {
            return;
        };
        view.pending = None;
        view.busy = false;
        let parsed = result
            .map_err(|error| error.to_string())
            .and_then(|value| parse_inventory(&value, &view.directory));
        match parsed {
            Ok((hooks, warnings, errors)) => {
                view.hooks = hooks;
                view.warnings = warnings;
                view.errors = errors;
                view.view_id = Some(Uuid::new_v4().to_string());
                view.current = true;
                view.error = None;
                view.notice = Some(
                    "Hook handlers are listed read-only. Codex remains the only hook engine."
                        .into(),
                );
            }
            Err(error) => {
                view.current = false;
                view.error = Some(format!(
                    "Native hook inventory failed: {error}. No hook was run, edited or retried."
                ));
            }
        }
    }

    pub(super) fn notify(&mut self, method: &str, params: &Value) -> Vec<String> {
        if !matches!(method, "hook/started" | "hook/completed") {
            return Vec::new();
        }
        let Some(thread) = params.get("threadId").and_then(Value::as_str) else {
            return Vec::new();
        };
        self.views
            .iter_mut()
            .filter(|(_, view)| view.thread_id == thread)
            .map(|(owner, view)| {
                match params
                    .get("run")
                    .ok_or_else(|| "Hook event is missing its run".to_owned())
                    .and_then(parse_run)
                {
                    Ok(run) => {
                        if let Some(existing) = view.runs.iter_mut().find(|item| item.id == run.id)
                        {
                            *existing = run;
                        } else {
                            view.runs.push(run);
                            if view.runs.len() > MAX_RUNS {
                                let excess = view.runs.len() - MAX_RUNS;
                                view.runs.drain(..excess);
                            }
                        }
                        view.error = None;
                    }
                    Err(error) => view.error = Some(error),
                }
                owner.clone()
            })
            .collect()
    }

    pub(super) fn invalidate_all(&mut self, notice: &str) -> Vec<String> {
        self.views
            .iter_mut()
            .map(|(owner, view)| {
                view.pending = None;
                view.busy = false;
                view.current = false;
                view.notice = Some(notice.into());
                owner.clone()
            })
            .collect()
    }

    pub(super) fn invalidate_thread(&mut self, thread: &str, notice: &str) -> Vec<String> {
        self.views
            .iter_mut()
            .filter(|(_, view)| view.thread_id == thread)
            .map(|(owner, view)| {
                view.pending = None;
                view.busy = false;
                view.current = false;
                view.notice = Some(notice.into());
                owner.clone()
            })
            .collect()
    }

    pub(super) fn remove_owner(&mut self, owner: &str) {
        self.views.remove(owner);
    }
}

impl BrowserApp {
    pub(super) fn refresh_native_hooks(
        &mut self,
        owner: &str,
        thread: &str,
        expected_directory: &str,
    ) -> Result<(), String> {
        self.app_server
            .conversations
            .binding(owner)
            .filter(|binding| binding.thread_id == thread && !binding.archived && !binding.deleted)
            .ok_or("Select an active native conversation before inspecting hooks")?;
        if !self.app_server.conversations.is_thread_loaded(owner) {
            return Err("Resume this native conversation before inspecting hooks".into());
        }
        let target = self.conversation_target(owner)?;
        let directory = target
            .root
            .filter(|path| !target.remote && path.is_absolute() && path.is_dir())
            .ok_or("Select an existing local project directory before inspecting hooks")?;
        if !same_local_path(Path::new(expected_directory), &directory) {
            return Err("The project changed. Reopen its hook inventory.".into());
        }
        let (id, call) = self.app_server.hooks.refresh(owner, thread, directory)?;
        self.native_hooks_call(owner.into(), thread.into(), id, call);
        self.emit_app_server_conversation(owner, "stream");
        Ok(())
    }

    fn native_hooks_call(&self, owner: String, thread: String, id: String, call: Call) {
        let Some(client) = self.app_server.client.clone() else {
            return;
        };
        let epoch = self.app_server.epoch;
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = call.send(&client).and_then(|ticket| ticket.wait());
            let _ = proxy.send_event(BrowserEvent::AppServer(Event::HooksReply {
                epoch,
                owner,
                thread,
                id,
                result,
            }));
        });
    }

    pub(super) fn native_hooks_reply(
        &mut self,
        owner: &str,
        thread: &str,
        id: &str,
        result: Result<Value, CallError>,
    ) {
        if self
            .app_server
            .conversations
            .binding(owner)
            .is_none_or(|binding| binding.thread_id != thread || binding.deleted)
        {
            return;
        }
        self.app_server.hooks.reply(owner, thread, id, result);
        self.emit_app_server_conversation(owner, "stream");
    }
}

fn parse_inventory(value: &Value, directory: &Path) -> Result<ParsedInventory, String> {
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or("Hook inventory response is incomplete")?;
    if data.len() != 1 || data[0].get("cwd").and_then(Value::as_str) != directory.to_str() {
        return Err("Hook inventory returned a different project directory".into());
    }
    let entry = &data[0];
    let raw_hooks = entry
        .get("hooks")
        .and_then(Value::as_array)
        .filter(|hooks| hooks.len() <= MAX_HOOKS)
        .ok_or("Hook inventory contains too many or invalid entries")?;
    let mut keys = HashSet::new();
    let hooks = raw_hooks
        .iter()
        .map(|hook| {
            let key = text(hook, "key", 1024)?;
            if !keys.insert(key.clone()) {
                return Err("Hook inventory contains a repeated hook key".into());
            }
            let handler_type = enum_text(
                hook,
                "handlerType",
                &["command", "mcpTool", "prompt", "agent"],
            )?;
            Ok(HookView {
                key,
                event_name: enum_text(hook, "eventName", HOOK_EVENTS)?,
                matcher: optional_text(hook, "matcher", 8 * 1024)?,
                status_message: optional_text(hook, "statusMessage", 8 * 1024)?,
                source_path: text(hook, "sourcePath", 16 * 1024)?,
                source: enum_text(hook, "source", HOOK_SOURCES)?,
                handler_type,
                enabled: boolean(hook, "enabled")?,
                managed: boolean(hook, "isManaged")?,
                trust_status: enum_text(
                    hook,
                    "trustStatus",
                    &["managed", "untrusted", "trusted", "modified"],
                )?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let warnings = text_array(entry, "warnings", 128, 8 * 1024)?;
    let errors = entry
        .get("errors")
        .and_then(Value::as_array)
        .filter(|errors| errors.len() <= 128)
        .ok_or("Hook inventory contains invalid errors")?
        .iter()
        .map(|error| {
            Ok(format!(
                "{}: {}",
                text(error, "path", 16 * 1024)?,
                text(error, "message", 8 * 1024)?
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((hooks, warnings, errors))
}

fn parse_run(value: &Value) -> Result<RunView, String> {
    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .filter(|entries| entries.len() <= 64)
        .ok_or("Hook run contains invalid output entries")?;
    let mut total = 0usize;
    let entries = entries
        .iter()
        .map(|entry| {
            let raw_entry_text = text(entry, "text", 16 * 1024)?;
            let (entry_text, _, _, _) = sanitize_terminal_snapshot_text(&raw_entry_text);
            total = total.saturating_add(entry_text.len());
            if total > 64 * 1024 {
                return Err("Hook run output exceeds the display limit".into());
            }
            Ok(RunEntry {
                kind: text(entry, "kind", 128)?,
                text: entry_text,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(RunView {
        id: text(value, "id", 1024)?,
        event_name: enum_text(value, "eventName", HOOK_EVENTS)?,
        handler_type: enum_text(
            value,
            "handlerType",
            &["command", "mcpTool", "prompt", "agent"],
        )?,
        execution_mode: enum_text(value, "executionMode", &["sync", "async"])?,
        scope: enum_text(value, "scope", &["thread", "turn"])?,
        source: enum_text(value, "source", HOOK_SOURCES)?,
        status: enum_text(
            value,
            "status",
            &["running", "completed", "failed", "blocked", "stopped"],
        )?,
        status_message: optional_text(value, "statusMessage", 8 * 1024)?,
        started_at: integer_string(value, "startedAt")?,
        completed_at: optional_integer_string(value, "completedAt")?,
        duration_ms: optional_integer_string(value, "durationMs")?,
        entries,
    })
}

const HOOK_EVENTS: &[&str] = &[
    "preToolUse",
    "permissionRequest",
    "postToolUse",
    "preCompact",
    "postCompact",
    "sessionStart",
    "sessionEnd",
    "userPromptSubmit",
    "subagentStart",
    "subagentStop",
    "stop",
    "interrupt",
];
const HOOK_SOURCES: &[&str] = &[
    "system",
    "user",
    "project",
    "mdm",
    "sessionFlags",
    "plugin",
    "cloudRequirements",
    "cloudManagedConfig",
    "legacyManagedConfigFile",
    "legacyManagedConfigMdm",
    "unknown",
];

fn text(value: &Value, key: &str, maximum: usize) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty() && text.len() <= maximum && !text.contains('\0'))
        .map(str::to_owned)
        .ok_or_else(|| format!("Hook data contains an invalid {key}"))
}
fn optional_text(value: &Value, key: &str, maximum: usize) -> Result<Option<String>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text))
            if !text.is_empty() && text.len() <= maximum && !text.contains('\0') =>
        {
            Ok(Some(text.clone()))
        }
        _ => Err(format!("Hook data contains an invalid {key}")),
    }
}
fn enum_text(value: &Value, key: &str, allowed: &[&str]) -> Result<String, String> {
    let value = text(value, key, 128)?;
    allowed
        .contains(&value.as_str())
        .then_some(value)
        .ok_or_else(|| format!("Hook data contains an unsupported {key}"))
}
fn boolean(value: &Value, key: &str) -> Result<bool, String> {
    value
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("Hook data contains an invalid {key}"))
}
fn integer_string(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .map(|number| number.to_string())
        .ok_or_else(|| format!("Hook data contains an invalid {key}"))
}
fn optional_integer_string(value: &Value, key: &str) -> Result<Option<String>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(number) => number
            .as_i64()
            .map(|number| Some(number.to_string()))
            .ok_or_else(|| format!("Hook data contains an invalid {key}")),
    }
}
fn text_array(
    value: &Value,
    key: &str,
    maximum_items: usize,
    maximum_text: usize,
) -> Result<Vec<String>, String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .filter(|items| items.len() <= maximum_items)
        .ok_or_else(|| format!("Hook data contains invalid {key}"))?
        .iter()
        .map(|item| {
            item.as_str()
                .filter(|text| {
                    !text.is_empty() && text.len() <= maximum_text && !text.contains('\0')
                })
                .map(str::to_owned)
                .ok_or_else(|| format!("Hook data contains invalid {key}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inventory_redacts_handlers_and_lifecycle_updates_one_native_run() {
        let root = if cfg!(windows) {
            "C:\\fixture"
        } else {
            "/fixture"
        };
        let value = json!({"data":[{"cwd":root,"hooks":[{
            "key":"hook-a","eventName":"preToolUse","matcher":null,"timeoutSec":10,
            "statusMessage":"Checking","additionalContextLimit":null,"sourcePath":format!("{root}/hook.toml"),
            "source":"project","pluginId":null,"displayOrder":1,"enabled":true,"isManaged":false,
            "currentHash":"secret-hash","trustStatus":"trusted","handlerType":"command","command":"secret-command","async":false
        }],"warnings":[],"errors":[]}]});
        let (hooks, _, _) = parse_inventory(&value, Path::new(root)).unwrap();
        let encoded = serde_json::to_string(&hooks).unwrap();
        assert!(!encoded.contains("secret-command"));
        assert!(!encoded.contains("secret-hash"));

        let mut hooks = Hooks::default();
        hooks.views.insert(
            "chat:a".into(),
            View {
                thread_id: "thread-a".into(),
                directory: PathBuf::from(root),
                ..View::default()
            },
        );
        let run = json!({"threadId":"thread-a","turnId":"turn-a","run":{
            "id":"run-a","eventName":"preToolUse","handlerType":"command","executionMode":"sync","scope":"turn",
            "sourcePath":format!("{root}/hook.toml"),"source":"project","displayOrder":1,"status":"running",
            "statusMessage":null,"startedAt":1,"completedAt":null,"durationMs":null,
            "entries":[{"kind":"feedback","text":"TOKEN=plain-token-value"}]}});
        assert_eq!(hooks.notify("hook/started", &run), vec!["chat:a"]);
        let mut completed = run;
        completed["run"]["status"] = json!("completed");
        completed["run"]["completedAt"] = json!(2);
        completed["run"]["durationMs"] = json!(1);
        assert_eq!(hooks.notify("hook/completed", &completed), vec!["chat:a"]);
        assert_eq!(hooks.views["chat:a"].runs.len(), 1);
        assert_eq!(hooks.views["chat:a"].runs[0].status, "completed");
        assert!(
            !serde_json::to_string(&hooks.views["chat:a"].runs)
                .unwrap()
                .contains("plain-token-value")
        );
    }
}
