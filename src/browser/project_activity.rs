//! Read-only, bounded project/file activity. Paths come from structured tool
//! events, never prose, command output, or the currently selected workspace.
use super::*;
use central_agent_codex_runtime::mirror::Turn;
use std::collections::BTreeMap;

const MAX_FILES: usize = 256;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FileActivity {
    pub path: String,
    pub state: &'static str,
    pub operation: &'static str,
    pub active_operations: usize,
}

pub(super) type NativeActivity = (&'static str, Vec<FileActivity>, bool);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Work {
    pub owner: String,
    pub chat_id: Option<String>,
    pub node_key: Option<String>,
    pub label: String,
    pub state: &'static str,
    pub files: Vec<FileActivity>,
    pub truncated: bool,
}

pub(super) fn normalized_path(path: &str, remote: bool) -> Option<String> {
    if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
        return None;
    }
    let path = path.replace('\\', "/");
    let path = path
        .strip_prefix("//?/UNC/")
        .map(|rest| format!("//{rest}"))
        .unwrap_or_else(|| path.strip_prefix("//?/").unwrap_or(&path).to_owned());
    let absolute = path.starts_with('/') || path.as_bytes().get(1) == Some(&b':');
    if !absolute {
        return None;
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.len() <= 1 {
                    return None;
                }
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    let prefix = if path.starts_with("//") {
        "//"
    } else if path.starts_with('/') {
        "/"
    } else {
        ""
    };
    let value = format!("{prefix}{}", parts.join("/"));
    Some(if remote { value } else { value.to_lowercase() })
}

pub(super) fn same_root(a: &str, b: &str, remote: bool) -> bool {
    normalized_path(a, remote)
        .zip(normalized_path(b, remote))
        .is_some_and(|(a, b)| a == b)
}

fn relative_file(root: &str, cwd: &str, path: &str, remote: bool) -> Option<String> {
    if path.is_empty() || path.len() > 4096 || path.chars().any(char::is_control) {
        return None;
    }
    let raw = path.replace('\\', "/");
    let raw = raw
        .strip_prefix("//?/UNC/")
        .map(|rest| format!("//{rest}"))
        .unwrap_or_else(|| raw.strip_prefix("//?/").unwrap_or(&raw).to_owned());
    // Never turn opaque URIs, Windows drive-relative paths or glob selectors into files.
    if raw.contains("://")
        || raw.contains(['*', '?', '|'])
        || (raw.as_bytes().get(1) == Some(&b':') && raw.as_bytes().get(2) != Some(&b'/'))
    {
        return None;
    }
    let absolute = if raw.starts_with('/') || raw.as_bytes().get(1) == Some(&b':') {
        raw
    } else {
        format!("{cwd}/{raw}")
    };
    let root_normal = normalized_path(root, remote)?;
    let absolute_normal = normalized_path(&absolute, remote)?;
    let relative = absolute_normal.strip_prefix(&format!("{root_normal}/"))?;
    if relative.is_empty() || relative.contains(':') {
        return None;
    }
    // Preserve display case after canonicalizing components without filesystem access.
    let display = normalized_path(&absolute, true)?;
    let display_parts: Vec<_> = display
        .rsplit('/')
        .take(relative.split('/').count())
        .collect();
    Some(
        display_parts
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("/"),
    )
}

fn item_state(
    status: Option<&str>,
    completed: bool,
    live: bool,
    turn_status: &str,
) -> &'static str {
    match status {
        Some("failed" | "declined") => "failed",
        Some("interrupted") => "stopped",
        Some("completed") => "done",
        _ if completed => "done",
        _ if turn_status == "failed" => "failed",
        _ if !live => "stopped",
        _ => "working",
    }
}

fn record(
    files: &mut BTreeMap<String, FileActivity>,
    file: FileActivity,
    remote: bool,
    truncated: &mut bool,
) {
    let key = if remote {
        file.path.clone()
    } else {
        file.path.to_lowercase()
    };
    if let Some(previous) = files.get_mut(&key) {
        if file.active_operations > 0 {
            let count = previous.active_operations + file.active_operations;
            let editing = previous.active_operations > 0 && previous.operation == "edit";
            *previous = file;
            previous.active_operations = count;
            if editing {
                previous.operation = "edit";
            }
        } else if previous.active_operations == 0
            && (file.state == "waiting" || previous.state != "waiting")
        {
            *previous = file;
        }
    } else if files.len() < MAX_FILES {
        files.insert(key, file);
    } else {
        *truncated = true;
    }
}

pub(super) fn native_files(
    turn: &Turn,
    root: &str,
    cwd: &str,
    connected: bool,
    waiting_items: &HashSet<String>,
) -> (Vec<FileActivity>, bool) {
    let mut files = BTreeMap::new();
    let mut truncated = false;
    let live = connected && turn.active();
    // Latest operations win for a completed file; unfinished operations keep it live.
    for item in &turn.items {
        let value = &item.value;
        let state = if live && !item.completed && waiting_items.contains(&item.id) {
            "waiting"
        } else if value["exitCode"].as_i64().is_some_and(|code| code != 0) {
            "failed"
        } else {
            item_state(value["status"].as_str(), item.completed, live, &turn.status)
        };
        truncated |= value["changes"]
            .as_array()
            .is_some_and(|items| items.len() > MAX_FILES)
            || value["commandActions"]
                .as_array()
                .is_some_and(|items| items.len() > MAX_FILES);
        let mut item_paths = std::collections::BTreeSet::new();
        let mut add = |path: &str, directory: &str, operation| {
            if let Some(path) = relative_file(root, directory, path, false)
                && item_paths.insert(path.to_lowercase())
            {
                record(
                    &mut files,
                    FileActivity {
                        path,
                        state,
                        operation,
                        active_operations: usize::from(state == "working"),
                    },
                    false,
                    &mut truncated,
                );
            }
        };
        match value["type"].as_str() {
            Some("fileChange") => {
                for change in value["changes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(MAX_FILES)
                {
                    if let Some(path) = change["path"].as_str() {
                        add(path, cwd, "edit");
                    }
                    if let Some(path) = change["kind"]["move_path"]
                        .as_str()
                        .or_else(|| change["kind"]["movePath"].as_str())
                    {
                        add(path, cwd, "edit");
                    }
                }
            }
            Some("commandExecution") => {
                let directory = value["cwd"].as_str().unwrap_or(cwd);
                for action in value["commandActions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(MAX_FILES)
                {
                    if action["type"] == "read"
                        && let Some(path) = action["path"].as_str()
                    {
                        add(path, directory, "read");
                    }
                }
            }
            _ => {}
        }
    }
    (files.into_values().collect(), truncated)
}

impl BrowserApp {
    pub(super) fn project_work(&self, root: &str, ssh_profile: Option<&str>) -> Vec<Work> {
        let mut result = Vec::new();
        if ssh_profile.is_none() {
            for chat in self
                .project_chats
                .iter()
                .filter(|chat| !chat.archived && same_root(root, &chat.project_root, false))
            {
                if let Some(work) = self.work_for_owner(
                    &format!("chat:{}", chat.id),
                    root,
                    false,
                    Some(chat.id.clone()),
                    None,
                    self.chat_summary(chat).title,
                ) {
                    result.push(work);
                }
            }
        }
        for binding in &self.agent_graph_bindings {
            if binding.ssh_profile_id.as_deref() != ssh_profile
                || !binding
                    .project_directory
                    .as_deref()
                    .is_some_and(|directory| same_root(root, directory, ssh_profile.is_some()))
            {
                continue;
            }
            let node = binding.node_key();
            if let Some(work) = self.work_for_owner(
                &format!("graph:{node}"),
                root,
                ssh_profile.is_some(),
                binding.project_chat_id.clone(),
                Some(node),
                binding.name.clone(),
            ) {
                result.push(work);
            }
        }
        result
    }

    fn work_for_owner(
        &self,
        owner: &str,
        root: &str,
        remote: bool,
        chat_id: Option<String>,
        node_key: Option<String>,
        label: String,
    ) -> Option<Work> {
        let run = self.run_for_owner(owner);
        let runtime_live = run.is_some_and(|run| run.is_active());
        let runtime_started = run.and_then(|run| {
            self.chat_messages
                .iter()
                .find(|message| message.run_id == Some(run.id))
                .map(|message| message.timestamp_ms)
        });
        let native = (!runtime_live && !remote)
            .then(|| {
                self.app_server
                    .project_activity(owner, root, runtime_started)
            })
            .flatten();
        let (state, files, truncated) = if let Some(native) = native {
            native
        } else {
            let run = run?;
            let state = match run.phase {
                AgentPhase::Stopped => "stopped",
                AgentPhase::Error => "failed",
                AgentPhase::WaitingApproval => "waiting",
                _ if run.is_active() => "working",
                _ => "done",
            };
            let mut files = BTreeMap::new();
            let mut truncated = false;
            for message in self
                .chat_messages
                .iter()
                .filter(|message| message.run_id == Some(run.id))
            {
                let operation = match message.activity_category {
                    Some(AgentActivityCategory::File) => "edit",
                    Some(AgentActivityCategory::Read) => "read",
                    _ => continue,
                };
                let status = match message.activity_status {
                    Some(AgentStepStatus::Success) => "done",
                    Some(AgentStepStatus::Error | AgentStepStatus::Denied) => "failed",
                    Some(AgentStepStatus::AwaitingApproval | AgentStepStatus::Queued)
                        if runtime_live =>
                    {
                        "waiting"
                    }
                    _ if runtime_live => "working",
                    _ => "stopped",
                };
                for path in message
                    .activity_context
                    .as_deref()
                    .unwrap_or_default()
                    .lines()
                    .take(MAX_FILES)
                    .filter_map(|path| relative_file(root, root, path, remote))
                {
                    record(
                        &mut files,
                        FileActivity {
                            path,
                            state: status,
                            operation,
                            active_operations: usize::from(status == "working"),
                        },
                        remote,
                        &mut truncated,
                    );
                }
            }
            (state, files.into_values().collect(), truncated)
        };
        Some(Work {
            owner: owner.into(),
            chat_id,
            node_key,
            label,
            state,
            files,
            truncated,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use central_agent_codex_runtime::mirror::Mirror;
    use serde_json::{Value, json};

    fn event(mirror: &mut Mirror, method: &str, item: Value) {
        mirror
            .notify(method, &json!({"threadId":"t","turnId":"turn","item":item}))
            .unwrap();
    }
    fn files(mirror: &Mirror, connected: bool) -> Vec<FileActivity> {
        native_files(
            &mirror.thread("t").unwrap().turns[0],
            "C:/app",
            "C:/app",
            connected,
            &HashSet::new(),
        )
        .0
    }
    fn read(id: &str, path: &str, status: &str) -> Value {
        json!({"id":id,"type":"commandExecution","status":status,"cwd":"C:/app","commandActions":[{"type":"read","path":path}]})
    }

    fn edit(id: &str, paths: &[&str], status: &str) -> Value {
        json!({"id":id,"type":"fileChange","status":status,"changes":paths.iter().map(|path|json!({"path":path,"kind":{"type":"update"}})).collect::<Vec<_>>()})
    }

    #[test]
    fn overlapping_operations_keep_each_file_live_until_its_own_completion() {
        let mut mirror = Mirror::default();
        event(
            &mut mirror,
            "item/started",
            read("r", "src/a.rs", "inProgress"),
        );
        event(
            &mut mirror,
            "item/started",
            edit("e", &["src/a.rs", "src/b.rs"], "inProgress"),
        );
        let live = files(&mirror, true);
        assert_eq!(live.len(), 2);
        assert_eq!(
            (live[0].state, live[0].operation, live[0].active_operations),
            ("working", "edit", 2)
        );
        event(
            &mut mirror,
            "item/completed",
            edit("e", &["src/a.rs", "src/b.rs"], "completed"),
        );
        let next = files(&mirror, true);
        assert_eq!(
            (next[0].state, next[0].operation, next[0].active_operations),
            ("working", "read", 1)
        );
        assert_eq!(next[1].state, "done");
        event(
            &mut mirror,
            "item/completed",
            read("r", "src/a.rs", "completed"),
        );
        assert!(
            files(&mirror, true)
                .iter()
                .all(|file| file.state == "done" && file.active_operations == 0)
        );
        // A new operation on a previously completed file makes only that file active again.
        event(
            &mut mirror,
            "item/started",
            edit("again", &["src/b.rs"], "inProgress"),
        );
        let next = files(&mirror, true);
        assert_eq!((next[0].state, next[1].state), ("done", "working"));
    }

    #[test]
    fn interruption_failure_and_disconnect_clear_active_markers_but_keep_completed_files() {
        for status in ["interrupted", "failed"] {
            let mut mirror = Mirror::default();
            event(
                &mut mirror,
                "item/completed",
                read("done", "done.rs", "completed"),
            );
            event(
                &mut mirror,
                "item/started",
                read("live", "live.rs", "inProgress"),
            );
            mirror
                .notify(
                    "turn/completed",
                    &json!({"threadId":"t","turn":{"id":"turn","status":status}}),
                )
                .unwrap();
            let result = files(&mirror, true);
            assert_eq!(result[0].state, "done");
            assert_eq!(
                result[1].state,
                if status == "failed" {
                    "failed"
                } else {
                    "stopped"
                }
            );
            assert!(result.iter().all(|file| file.active_operations == 0));
        }
        let mut mirror = Mirror::default();
        event(
            &mut mirror,
            "item/started",
            read("r", "src/a.rs", "inProgress"),
        );
        assert_eq!(files(&mirror, false)[0].state, "stopped");
        let mut failed = read("r", "src/a.rs", "completed");
        failed["exitCode"] = json!(1);
        event(&mut mirror, "item/completed", failed);
        assert_eq!(files(&mirror, true)[0].state, "failed");
    }

    #[test]
    fn file_attribution_is_project_relative_and_only_uses_structured_tool_targets() {
        let mut mirror = Mirror::default();
        event(
            &mut mirror,
            "item/started",
            json!({"id":"opaque","type":"commandExecution","command":"cat C:/app/secret.rs","aggregatedOutput":"C:/app/secret.rs","commandActions":[{"type":"unknown","command":"cat C:/app/secret.rs"}]}),
        );
        event(
            &mut mirror,
            "item/started",
            edit(
                "outside",
                &["../outside.rs", "C:/application/wrong.rs"],
                "inProgress",
            ),
        );
        assert!(files(&mirror, true).is_empty());
        event(
            &mut mirror,
            "item/started",
            edit(
                "inside",
                &["C:/APP/src/A.rs", "c:/app/src/a.rs"],
                "inProgress",
            ),
        );
        let result = files(&mirror, true);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "src/A.rs");
        assert_eq!(result[0].active_operations, 1);
    }

    #[test]
    fn native_file_lists_are_bounded_and_expose_truncation() {
        let mut mirror = Mirror::default();
        let names = (0..300).map(|i| format!("src/{i}.rs")).collect::<Vec<_>>();
        event(
            &mut mirror,
            "item/started",
            edit(
                "many",
                &names.iter().map(String::as_str).collect::<Vec<_>>(),
                "inProgress",
            ),
        );
        let result = native_files(
            &mirror.thread("t").unwrap().turns[0],
            "C:/app",
            "C:/app",
            true,
            &HashSet::new(),
        );
        assert_eq!(result.0.len(), MAX_FILES);
        assert!(result.1);
    }

    #[test]
    fn waiting_for_file_approval_does_not_claim_an_edit_is_running_or_block_another_read() {
        let mut mirror = Mirror::default();
        event(
            &mut mirror,
            "item/started",
            edit("edit", &["a.rs", "b.rs"], "inProgress"),
        );
        event(
            &mut mirror,
            "item/started",
            read("read", "a.rs", "inProgress"),
        );
        let waiting = HashSet::from(["edit".to_owned()]);
        let turn = &mirror.thread("t").unwrap().turns[0];
        let (result, _) = native_files(turn, "C:/app", "C:/app", true, &waiting);
        assert_eq!(
            (
                result[0].state,
                result[0].operation,
                result[0].active_operations
            ),
            ("working", "read", 1)
        );
        assert_eq!(
            (result[1].state, result[1].active_operations),
            ("waiting", 0)
        );
        let (offline, _) = native_files(turn, "C:/app", "C:/app", false, &waiting);
        assert!(offline.iter().all(|file| file.state == "stopped"));
    }

    #[test]
    fn path_scopes_handle_windows_verbatim_paths_and_remote_case_sensitively() {
        assert_eq!(
            relative_file("C:/App", "C:/App/src", "../lib/A.rs", false).as_deref(),
            Some("lib/A.rs")
        );
        assert_eq!(
            relative_file("C:/App", "C:/App", r"\\?\C:\app\src\A.rs", false).as_deref(),
            Some("src/A.rs")
        );
        assert_eq!(
            relative_file("/srv/App", "/srv/App/src", "a.rs", true).as_deref(),
            Some("src/a.rs")
        );
        assert!(relative_file("/srv/App", "/srv/app", "a.rs", true).is_none());
        for path in [
            "../outside",
            "C:/Application/wrong",
            "C:relative",
            "https://example.test/a",
            "*.rs",
            "a\nb",
        ] {
            assert!(
                relative_file("C:/App", "C:/App", path, false).is_none(),
                "{path}"
            );
        }
    }
}
