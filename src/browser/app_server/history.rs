//! On-demand native history browsing. List results confer no authority until
//! the user explicitly imports/forks an observed ID into an unbound local chat.
use super::*;
use crate::tab_context::{redact_sensitive, sanitize_ui_text};

const MAX_HISTORY_PROJECT_LABEL_CHARS: usize = 96;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum HistoryAction {
    Open {
        request_id: String,
        archived: bool,
        search: String,
    },
    More {
        view_id: String,
    },
    Close {
        view_id: String,
    },
    Import {
        view_id: String,
        thread_id: String,
        fork: bool,
    },
    CloudRefresh {
        view_id: String,
    },
    CloudMore {
        view_id: String,
    },
    CloudOpen {
        view_id: String,
        task_id: String,
    },
    CloudDiff {
        view_id: String,
        task_id: String,
        attempt: u32,
    },
    CloudNew {
        view_id: String,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Summary {
    id: String,
    #[serde(rename = "name", skip_serializing)]
    _name: Option<String>,
    #[serde(rename = "preview", skip_serializing)]
    _preview: String,
    #[serde(skip_serializing)]
    cwd: String,
    #[serde(default)]
    project_label: String,
    model_provider: String,
    updated_at: i64,
    ephemeral: bool,
    forked_from_id: Option<String>,
    status: Status,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
struct Status {
    #[serde(rename = "type")]
    kind: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct History {
    request_id: String,
    view_id: String,
    archived: bool,
    search: String,
    items: Vec<Summary>,
    busy: bool,
    has_more: bool,
    error: Option<String>,
    cloud: Cloud,
    #[serde(skip)]
    root: Option<PathBuf>,
    #[serde(skip)]
    cursor: Option<String>,
    #[serde(skip)]
    cursors: HashSet<String>,
    #[serde(skip)]
    pending: Option<String>,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Cloud {
    items: Vec<central_agent_codex_runtime::cloud::Task>,
    busy: bool,
    has_more: bool,
    error: Option<String>,
    diff: Option<CloudDiff>,
    #[serde(skip)]
    cursor: Option<String>,
    #[serde(skip)]
    cursors: HashSet<String>,
    #[serde(skip)]
    pending: Option<String>,
    #[serde(skip)]
    replace_pending: bool,
    #[serde(skip)]
    diff_pending: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CloudDiff {
    task_id: String,
    attempt: u32,
    busy: bool,
    content: String,
    error: Option<String>,
}

impl Cloud {
    fn begin(&mut self, refresh: bool) -> Result<(String, Option<String>), String> {
        if self.pending.is_some() {
            return Err("Codex Cloud chats are already loading".into());
        }
        if refresh {
            self.cursor = None;
            self.cursors.clear();
            self.has_more = false;
        }
        let id = Uuid::new_v4().to_string();
        let cursor = self.cursor.clone();
        self.pending = Some(id.clone());
        self.replace_pending = refresh;
        self.busy = true;
        self.error = None;
        Ok((id, cursor))
    }

    fn reply(
        &mut self,
        id: &str,
        result: Result<central_agent_codex_runtime::cloud::TaskPage, String>,
    ) {
        if self.pending.as_deref() != Some(id) {
            return;
        }
        self.pending = None;
        self.busy = false;
        let replace = std::mem::take(&mut self.replace_pending);
        match result {
            Ok(page) => {
                if let Some(cursor) = page.cursor.as_ref()
                    && !self.cursors.insert(cursor.clone())
                {
                    self.error = Some(
                        "Codex Cloud repeated its pagination cursor. Refresh the list.".into(),
                    );
                    self.has_more = false;
                    return;
                }
                if replace {
                    self.items.clear();
                }
                for task in page.tasks {
                    if let Some(existing) = self.items.iter_mut().find(|item| item.id == task.id) {
                        *existing = task;
                    } else {
                        self.items.push(task);
                    }
                }
                self.items
                    .sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
                self.cursor = page.cursor;
                self.has_more = self.cursor.is_some();
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn task(&self, id: &str) -> Result<&central_agent_codex_runtime::cloud::Task, String> {
        if self.pending.is_some() || self.error.is_some() {
            return Err("Refresh Codex Cloud chats before choosing one".into());
        }
        self.items
            .iter()
            .find(|task| task.id == id)
            .ok_or_else(|| "Choose a task from the current Codex Cloud list".into())
    }

    fn begin_diff(&mut self, task_id: &str, attempt: u32) -> Result<String, String> {
        if self.diff_pending.is_some() {
            return Err("A Codex Cloud diff is already loading".into());
        }
        let task = self.task(task_id)?;
        if attempt == 0 || attempt > task.attempt_total.max(1) {
            return Err("Select an available Codex Cloud attempt".into());
        }
        let id = Uuid::new_v4().to_string();
        self.diff_pending = Some(id.clone());
        self.diff = Some(CloudDiff {
            task_id: task_id.to_owned(),
            attempt,
            busy: true,
            content: String::new(),
            error: None,
        });
        Ok(id)
    }

    fn diff_reply(&mut self, id: &str, result: Result<String, String>) {
        if self.diff_pending.as_deref() != Some(id) {
            return;
        }
        self.diff_pending = None;
        let Some(diff) = self.diff.as_mut() else {
            return;
        };
        diff.busy = false;
        match result {
            Ok(content) => diff.content = content,
            Err(error) => diff.error = Some(error),
        }
    }
}
impl History {
    fn new(request_id: String, archived: bool, search: String, root: Option<PathBuf>) -> Self {
        Self {
            request_id,
            view_id: Uuid::new_v4().to_string(),
            archived,
            search,
            items: vec![],
            busy: false,
            has_more: false,
            error: None,
            cloud: Cloud::default(),
            root,
            cursor: None,
            cursors: HashSet::new(),
            pending: None,
        }
    }
    fn next(&mut self) -> Result<(String, Call), String> {
        if self.pending.is_some() {
            return Err("A native history page is already loading".into());
        }
        let id = Uuid::new_v4().to_string();
        self.pending = Some(id.clone());
        self.busy = true;
        self.error = None;
        Ok((
            id,
            api::search_threads(
                self.cursor.as_deref(),
                self.archived,
                (!self.search.is_empty()).then_some(self.search.as_str()),
            ),
        ))
    }
    fn reply(&mut self, id: &str, result: Result<Value, CallError>) {
        if self.pending.as_deref() != Some(id) {
            return;
        }
        self.pending = None;
        self.busy = false;
        let result = result.map_err(|error| error.to_string()).and_then(|value| {
            let next = value
                .get("nextCursor")
                .ok_or("Missing native history pagination")?;
            let next = if next.is_null() {
                None
            } else {
                Some(
                    next.as_str()
                        .ok_or("Invalid native history cursor")?
                        .to_owned(),
                )
            };
            let mut data: Vec<Summary> = serde_json::from_value(
                value
                    .get("data")
                    .ok_or("Missing native history entries")?
                    .clone(),
            )
            .map_err(|_| "Invalid native thread summary")?;
            if data.iter().any(|t| {
                t.id.is_empty()
                    || t.id.len() > 1024
                    || t.id.contains('\0')
                    || t.cwd.is_empty()
                    || !matches!(
                        t.status.kind.as_str(),
                        "notLoaded" | "idle" | "active" | "systemError"
                    )
            }) {
                return Err("Invalid native thread identity or status".into());
            }
            for item in &mut data {
                item.project_label = history_project_label(&item.cwd);
                item.model_provider = compact_ui_text(&item.model_provider, 128).0;
            }
            if let Some(next) = &next
                && (next.is_empty() || !self.cursors.insert(next.clone()))
            {
                return Err(
                    "Native history repeated its pagination cursor. Refresh the list.".into(),
                );
            }
            // A thread can move between pages when its update timestamp changes.
            // Replace its summary in place rather than showing duplicate choices.
            for item in data {
                if let Some(existing) = self.items.iter_mut().find(|t| t.id == item.id) {
                    *existing = item;
                } else {
                    self.items.push(item);
                }
            }
            self.cursor = next;
            self.has_more = self.cursor.is_some();
            Ok(())
        });
        if let Err(error) = result {
            self.error = Some(error);
        }
    }
    fn choice(&self, view_id: &str, id: &str, root: &Option<PathBuf>) -> Result<&Summary, String> {
        if self.view_id != view_id
            || self.pending.is_some()
            || self.error.is_some()
            || &self.root != root
        {
            return Err(
                "The history view or project changed. Refresh before choosing a conversation."
                    .into(),
            );
        }
        self.items
            .iter()
            .find(|item| item.id == id && !item.ephemeral)
            .ok_or_else(|| {
                "Choose a persisted conversation from the current native history list".into()
            })
    }
}

fn compact_ui_text(value: &str, maximum: usize) -> (String, bool) {
    let (redacted, _) = redact_sensitive(value);
    let compact = redacted.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = compact.chars();
    let limited = chars.by_ref().take(maximum).collect();
    (limited, chars.next().is_some())
}

fn history_project_label(value: &str) -> String {
    let sanitized = sanitize_ui_text(value, 1024).0;
    let display = display_local_path(&sanitized);
    let trimmed = display.trim_end_matches(['\\', '/']);
    let label = trimmed.rsplit(['\\', '/']).next().unwrap_or_default();
    let label = compact_ui_text(label, MAX_HISTORY_PROJECT_LABEL_CHARS).0;
    if label.is_empty() {
        "Local project".into()
    } else {
        label
    }
}

impl BrowserApp {
    pub(in crate::browser) fn app_server_history(&mut self, owner: String, action: HistoryAction) {
        let result = (|| {
            if let HistoryAction::Close { view_id } = &action {
                if self
                    .app_server
                    .history
                    .get(&owner)
                    .is_some_and(|v| v.view_id == *view_id)
                {
                    self.app_server.history.remove(&owner);
                }
                return Ok(());
            }
            if !self.app_server.view.connected || self.app_server.client.is_none() {
                return Err("Connect Codex App Server first".into());
            }
            if !self.native_access_selected(&owner) {
                return Err("Select Codex in this conversation first".into());
            }
            let target = self.conversation_target(&owner)?;
            if target.remote {
                return Err(
                    "Native Codex history requires a local graph directory, not an SSH path".into(),
                );
            }
            match action {
                HistoryAction::Open {
                    request_id,
                    archived,
                    search,
                } => {
                    let mut history = History::new(request_id, archived, search, target.root);
                    let (id, call) = history.next()?;
                    let (cloud_id, cloud_cursor) = history.cloud.begin(true)?;
                    self.app_server.history.insert(owner.clone(), history);
                    self.app_server_history_call(&owner, id, call);
                    if let Err(error) =
                        self.app_server_cloud_history_call(&owner, cloud_id.clone(), cloud_cursor)
                        && let Some(history) = self.app_server.history.get_mut(&owner)
                    {
                        history.cloud.reply(&cloud_id, Err(error));
                    }
                }
                HistoryAction::More { view_id } => {
                    let history = self
                        .app_server
                        .history
                        .get_mut(&owner)
                        .filter(|v| v.view_id == view_id && v.root == target.root)
                        .ok_or("This history view is no longer active")?;
                    if !history.has_more {
                        return Err("The native history has no next page".into());
                    }
                    let (id, call) = history.next()?;
                    self.app_server_history_call(&owner, id, call);
                }
                HistoryAction::CloudRefresh { view_id } => {
                    let (id, cursor) = {
                        let history = self
                            .app_server
                            .history
                            .get_mut(&owner)
                            .filter(|value| value.view_id == view_id && value.root == target.root)
                            .ok_or("This history view is no longer active")?;
                        history.cloud.begin(true)?
                    };
                    if let Err(error) =
                        self.app_server_cloud_history_call(&owner, id.clone(), cursor)
                        && let Some(history) = self.app_server.history.get_mut(&owner)
                    {
                        history.cloud.reply(&id, Err(error));
                    }
                }
                HistoryAction::CloudMore { view_id } => {
                    let (id, cursor) = {
                        let history = self
                            .app_server
                            .history
                            .get_mut(&owner)
                            .filter(|value| value.view_id == view_id && value.root == target.root)
                            .ok_or("This history view is no longer active")?;
                        if !history.cloud.has_more {
                            return Err("Codex Cloud has no next page".into());
                        }
                        history.cloud.begin(false)?
                    };
                    if let Err(error) =
                        self.app_server_cloud_history_call(&owner, id.clone(), cursor)
                        && let Some(history) = self.app_server.history.get_mut(&owner)
                    {
                        history.cloud.reply(&id, Err(error));
                    }
                }
                HistoryAction::CloudOpen { view_id, task_id } => {
                    let url = self
                        .app_server
                        .history
                        .get(&owner)
                        .filter(|value| value.view_id == view_id && value.root == target.root)
                        .ok_or("This history view is no longer active")?
                        .cloud
                        .task(&task_id)?
                        .url
                        .clone();
                    self.proxy
                        .send_event(BrowserEvent::OpenInNewTab(url))
                        .map_err(|_| "Could not open the Codex Cloud chat in Supervisor")?;
                }
                HistoryAction::CloudDiff {
                    view_id,
                    task_id,
                    attempt,
                } => {
                    let id = {
                        let history = self
                            .app_server
                            .history
                            .get_mut(&owner)
                            .filter(|value| value.view_id == view_id && value.root == target.root)
                            .ok_or("This history view is no longer active")?;
                        history.cloud.begin_diff(&task_id, attempt)?
                    };
                    if let Err(error) =
                        self.app_server_cloud_diff_call(&owner, id.clone(), task_id, attempt)
                        && let Some(history) = self.app_server.history.get_mut(&owner)
                    {
                        history.cloud.diff_reply(&id, Err(error));
                    }
                }
                HistoryAction::CloudNew { view_id } => {
                    let history = self
                        .app_server
                        .history
                        .get(&owner)
                        .filter(|value| value.view_id == view_id && value.root == target.root)
                        .ok_or("This history view is no longer active")?;
                    if history.cloud.pending.is_some() {
                        return Err("Wait for Codex Cloud chats to finish loading".into());
                    }
                    self.proxy
                        .send_event(BrowserEvent::OpenInNewTab(
                            "https://chatgpt.com/codex".to_owned(),
                        ))
                        .map_err(|_| "Could not open Codex Cloud in Supervisor")?;
                }
                HistoryAction::Import {
                    view_id,
                    thread_id,
                    fork,
                } => {
                    if let Some(error) = &self.app_server.binding_store_error {
                        return Err(error.clone());
                    }
                    if self.owner_has_unfinished_work(&owner) || self.conversation_busy(&owner) {
                        return Err(
                            "Finish this conversation's work before linking native history".into(),
                        );
                    }
                    if owner.starts_with("draft:") {
                        return Err("Create a project chat first so imported history has a persistent local owner".into());
                    }
                    let history = self
                        .app_server
                        .history
                        .get(&owner)
                        .ok_or("Load native history before importing")?;
                    history.choice(&view_id, &thread_id, &target.root)?;
                    let root = target
                        .root
                        .ok_or("Choose a local project or graph directory before importing")?;
                    if !root.is_absolute() || !root.is_dir() {
                        return Err("Select an existing absolute local directory".into());
                    }
                    let request = self.app_server.conversations.import(
                        &owner,
                        &thread_id,
                        history.archived,
                        fork,
                    )?;
                    // This directory remains owned by the accepted local destination,
                    // never the selected project at the time the reply arrives.
                    self.app_server.roots.insert(owner.clone(), root);
                    self.dispatch_app_server_thread(request);
                }
                HistoryAction::Close { .. } => unreachable!(),
            }
            Ok(())
        })();
        self.emit_app_server_history(&owner, result.err());
        self.render_agent_panel();
    }
    fn app_server_history_call(&self, owner: &str, id: String, call: Call) {
        let Some(client) = self.app_server.client.clone() else {
            return;
        };
        let epoch = self.app_server.epoch;
        let owner = owner.to_owned();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = call.send(&client).and_then(|ticket| ticket.wait());
            let _ = proxy.send_event(BrowserEvent::AppServer(Event::HistoryReply {
                epoch,
                owner,
                id,
                result,
            }));
        });
    }
    fn app_server_cloud_history_call(
        &self,
        owner: &str,
        id: String,
        cursor: Option<String>,
    ) -> Result<(), String> {
        let runtime = self
            .app_server
            .runtime
            .clone()
            .ok_or("Reconnect Codex before loading Cloud chats")?;
        let epoch = self.app_server.epoch;
        let owner = owner.to_owned();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = runtime.list_cloud_tasks(cursor.as_deref(), 20);
            let _ = proxy.send_event(BrowserEvent::AppServer(Event::CloudHistoryReply {
                epoch,
                owner,
                id,
                result,
            }));
        });
        Ok(())
    }
    fn app_server_cloud_diff_call(
        &self,
        owner: &str,
        id: String,
        task_id: String,
        attempt: u32,
    ) -> Result<(), String> {
        let runtime = self
            .app_server
            .runtime
            .clone()
            .ok_or("Reconnect Codex before loading a Cloud diff")?;
        let epoch = self.app_server.epoch;
        let owner = owner.to_owned();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = runtime.cloud_task_diff(&task_id, attempt);
            let _ = proxy.send_event(BrowserEvent::AppServer(Event::CloudDiffReply {
                epoch,
                owner,
                id,
                result,
            }));
        });
        Ok(())
    }
    pub(super) fn app_server_history_reply(
        &mut self,
        owner: &str,
        id: &str,
        result: Result<Value, CallError>,
    ) {
        if let Some(history) = self.app_server.history.get_mut(owner) {
            history.reply(id, result);
            self.emit_app_server_history(owner, None);
        }
    }
    pub(super) fn app_server_cloud_history_reply(
        &mut self,
        owner: &str,
        id: &str,
        result: Result<central_agent_codex_runtime::cloud::TaskPage, String>,
    ) {
        if let Some(history) = self.app_server.history.get_mut(owner) {
            history.cloud.reply(id, result);
            self.emit_app_server_history(owner, None);
        }
    }
    pub(super) fn app_server_cloud_diff_reply(
        &mut self,
        owner: &str,
        id: &str,
        result: Result<String, String>,
    ) {
        if let Some(history) = self.app_server.history.get_mut(owner) {
            history.cloud.diff_reply(id, result);
            self.emit_app_server_history(owner, None);
        }
    }
    pub(super) fn emit_app_server_history(&self, owner: &str, error: Option<String>) {
        let imported = self.app_server.conversations.binding(owner).is_some();
        self.conversation_event("central-agent:app-server-history",json!({"owner":owner,"view":self.app_server.history.get(owner),"error":error,"bound":imported,"busy":self.app_server.busy(owner)}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_history_schema_fixture_is_accepted_without_extra_data() {
        let thread: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/crates/central-agent-codex-runtime/tests/native-history-thread.json"
        )))
        .unwrap();
        let mut h = History::new("ui".into(), false, String::new(), None);
        let (id, _) = h.next().unwrap();
        h.reply(
            &id,
            Ok(json!({"data":[thread],"nextCursor":null,"backwardsCursor":"newer"})),
        );
        assert!(h.error.is_none());
        assert_eq!(h.items[0].id, "fixture-native-history");
        assert!(!serde_json::to_string(&h).unwrap().contains("sessionId"));
    }
    fn thread(id: &str) -> Value {
        json!({"id":id,"name":"Native title","preview":"Preview","cwd":"C:\\fixture","modelProvider":"openai","updatedAt":123,"ephemeral":false,"status":{"type":"notLoaded"},"turns":[{"secret":"not-in-summary"}]})
    }
    #[test]
    fn history_pages_are_native_deduplicated_and_have_no_transcripts() {
        let mut h = History::new("ui".into(), true, "Title".into(), None);
        let (id, call) = h.next().unwrap();
        assert_eq!(call.params["searchTerm"], "Title");
        assert_eq!(call.params["archived"], true);
        h.reply(&id, Ok(json!({"data":[thread("a")],"nextCursor":"more"})));
        assert!(h.has_more);
        let (next, _) = h.next().unwrap();
        h.reply(&id, Ok(json!({"data":[],"nextCursor":null})));
        assert!(h.busy);
        h.reply(
            &next,
            Ok(json!({"data":[thread("a"),thread("b")],"nextCursor":null})),
        );
        assert_eq!(h.items.len(), 2);
        assert!(!h.has_more);
        assert!(
            !serde_json::to_string(&h)
                .unwrap()
                .contains("not-in-summary")
        );
        assert!(h.choice(&h.view_id, "a", &None).is_ok());
    }
    #[test]
    fn history_choices_require_the_observed_view_and_directory() {
        let mut h = History::new("ui".into(), false, String::new(), None);
        let (id, _) = h.next().unwrap();
        h.reply(&id, Ok(json!({"data":[thread("a")],"nextCursor":"cycle"})));
        assert!(h.choice("old", "a", &None).is_err());
        assert!(h.choice(&h.view_id, "foreign", &None).is_err());
        assert!(
            h.choice(&h.view_id, "a", &Some(PathBuf::from("different")))
                .is_err()
        );
        let (id, _) = h.next().unwrap();
        h.reply(&id, Ok(json!({"data":[],"nextCursor":"cycle"})));
        assert!(h.error.is_some());
        assert!(h.choice(&h.view_id, "a", &None).is_err());
        assert_eq!(h.items.len(), 1);
    }

    #[test]
    fn history_projection_omits_prompt_content_and_absolute_paths() {
        let mut value = thread("a");
        value["name"] = json!("SYNTHETIC_PRIVATE_TITLE");
        value["preview"] = json!(format!(
            "{}SYNTHETIC_SECRET_TAIL",
            "visible preview line\n".repeat(40)
        ));
        value["cwd"] = json!(r"\\?\C:\fixture");
        let mut history = History::new("ui".into(), false, String::new(), None);
        let (id, _) = history.next().unwrap();
        history.reply(&id, Ok(json!({"data":[value],"nextCursor":null})));
        let output = serde_json::to_string(&history).unwrap();
        assert!(!output.contains("SYNTHETIC_SECRET_TAIL"));
        assert!(!output.contains("SYNTHETIC_PRIVATE_TITLE"));
        assert!(!output.contains(r"\\?\C:\fixture"));
        assert_eq!(history.items[0].project_label, "fixture");
    }

    fn cloud_page(id: &str, cursor: Option<&str>) -> central_agent_codex_runtime::cloud::TaskPage {
        serde_json::from_value(json!({
            "tasks":[{
                "id":id,
                "url":format!("https://chatgpt.com/codex/tasks/{id}"),
                "title":"Cloud task",
                "status":"ready",
                "updated_at":"2026-09-17T10:00:00Z",
                "environment_id":null,
                "environment_label":"Supervisor",
                "summary":{"files_changed":2,"lines_added":4,"lines_removed":1},
                "is_review":false,
                "attempt_total":2
            }],
            "cursor":cursor
        }))
        .unwrap()
    }

    #[test]
    fn cloud_history_is_paginated_and_diff_is_bound_to_an_observed_task() {
        let mut cloud = Cloud::default();
        let (first, cursor) = cloud.begin(true).unwrap();
        assert!(cursor.is_none());
        cloud.reply(&first, Ok(cloud_page("task_e_firstfixture", Some("older"))));
        assert_eq!(cloud.items.len(), 1);
        assert!(cloud.has_more);

        let diff = cloud
            .begin_diff("task_e_firstfixture", 2)
            .expect("observed task can request its reported attempt");
        cloud.diff_reply(&diff, Ok("diff --git a/a b/a".into()));
        assert_eq!(cloud.diff.as_ref().unwrap().attempt, 2);
        assert!(cloud.begin_diff("task_e_missing", 1).is_err());

        let (next, cursor) = cloud.begin(false).unwrap();
        assert_eq!(cursor.as_deref(), Some("older"));
        cloud.reply(&next, Ok(cloud_page("task_e_secondfixture", None)));
        assert_eq!(cloud.items.len(), 2);
        assert!(!cloud.has_more);

        let (refresh, cursor) = cloud.begin(true).unwrap();
        assert!(cursor.is_none());
        assert_eq!(
            cloud.items.len(),
            2,
            "refresh keeps the visible list stable"
        );
        cloud.reply(&refresh, Ok(cloud_page("task_e_latestfixture", None)));
        assert_eq!(cloud.items.len(), 1);
        assert_eq!(cloud.items[0].id, "task_e_latestfixture");
    }

    #[test]
    fn repeated_cloud_cursor_invalidates_the_current_list() {
        let mut cloud = Cloud::default();
        let (first, _) = cloud.begin(true).unwrap();
        cloud.reply(&first, Ok(cloud_page("task_e_firstfixture", Some("older"))));
        let (next, _) = cloud.begin(false).unwrap();
        cloud.reply(&next, Ok(cloud_page("task_e_secondfixture", Some("older"))));
        assert!(
            cloud
                .error
                .as_deref()
                .unwrap()
                .contains("pagination cursor")
        );
        assert!(!cloud.has_more);
    }
}
