//! Display-only projection of authoritative thread/turn/item events. This state
//! is NEVER fed back as conversation history, instructions or tool output.
use crate::model_notices::{self, ModelNotice, Notice};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub value: Value,
    pub completed: bool,
    /// Transient native notifications, never written into the native item/history.
    #[serde(skip)]
    pub progress_messages: Vec<String>,
    #[serde(skip)]
    revision: u64,
    #[serde(skip)]
    summaries: BTreeMap<u64, String>,
}

impl Item {
    fn new(id: &str, kind: &str, revision: u64) -> Self {
        Self {
            id: id.into(),
            value: json!({"id":id,"type":kind}),
            completed: false,
            progress_messages: Vec::new(),
            revision,
            summaries: BTreeMap::new(),
        }
    }
    fn replace(&mut self, mut value: Value, completed: bool, revision: u64) {
        if value["type"] == "reasoning" {
            // Only exposed readable summaries belong in this application's UI.
            if let Some(object) = value.as_object_mut() {
                object.remove("content");
            }
            self.summaries = value["summary"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
                .filter_map(|(i, v)| v.as_str().map(|s| (i as u64, s.into())))
                .collect();
        }
        self.value = value;
        self.completed = completed;
        self.revision = revision;
    }
    fn append(&mut self, field: &str, delta: &str, revision: u64) {
        if self.completed {
            return;
        }
        let mut text = self.value[field].as_str().unwrap_or_default().to_owned();
        text.push_str(delta);
        self.value[field] = Value::String(text);
        self.revision = revision;
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Turn {
    pub id: String,
    pub items_view: Option<String>,
    pub status: String,
    pub items: Vec<Item>,
    pub error: Option<Value>,
    pub diff: Option<String>,
    pub plan: Option<Value>,
    pub started_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    #[serde(skip)]
    revision: u64,
}
impl Turn {
    fn new(id: &str, revision: u64) -> Self {
        Self {
            id: id.into(),
            status: "inProgress".into(),
            items_view: None,
            items: vec![],
            error: None,
            diff: None,
            plan: None,
            started_at_ms: None,
            completed_at_ms: None,
            duration_ms: None,
            revision,
        }
    }
    pub fn active(&self) -> bool {
        self.status == "inProgress"
    }
    fn timing(&mut self, data: &Value) {
        if let Some(s) = data["startedAt"].as_u64() {
            self.started_at_ms = s.checked_mul(1000);
        }
        if let Some(s) = data["completedAt"].as_u64() {
            self.completed_at_ms = s.checked_mul(1000);
        }
        if let Some(d) = data["durationMs"].as_u64() {
            self.duration_ms = Some(d);
        }
    }
    fn item(&mut self, id: &str, kind: &str, revision: u64) -> &mut Item {
        if let Some(index) = self.items.iter().position(|item| item.id == id) {
            return &mut self.items[index];
        }
        self.items.push(Item::new(id, kind, revision));
        self.items.last_mut().unwrap()
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Thread {
    pub id: String,
    pub name: Option<String>,
    pub history_mode: Option<String>,
    pub turns: Vec<Turn>,
    pub notices: Vec<Notice>,
    pub status: Option<Value>,
    pub token_usage: Option<Value>,
    pub token_usage_turn_id: Option<String>,
    pub token_usage_current: bool,
    /// Local receipt time for an actual native usage notification, never history data.
    #[serde(skip)]
    pub token_usage_observed_at_ms: Option<u64>,
    /// The model at the time of that report; later profile changes cannot relabel it.
    #[serde(skip)]
    pub token_usage_model: Option<String>,
    pub settings: Option<crate::thread_settings::ThreadSettings>,
    pub settings_current: bool,
    /// P3 backend metadata is not projected into the WebView or persisted.
    #[serde(skip)]
    pub metadata: Option<crate::thread_metadata::NativeMetadata>,
    #[serde(skip)]
    pub metadata_current: bool,
    #[serde(skip)]
    metadata_revision: u64,
    #[serde(skip)]
    history_floor: u64,
    #[serde(skip)]
    status_revision: u64,
    #[serde(skip)]
    name_revision: u64,
    #[serde(skip)]
    settings_revision: u64,
}
impl Thread {
    fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            name: None,
            history_mode: None,
            turns: vec![],
            notices: vec![],
            status: None,
            token_usage: None,
            token_usage_turn_id: None,
            token_usage_current: false,
            token_usage_observed_at_ms: None,
            token_usage_model: None,
            settings: None,
            settings_current: false,
            metadata: None,
            metadata_current: false,
            metadata_revision: 0,
            history_floor: 0,
            status_revision: 0,
            name_revision: 0,
            settings_revision: 0,
        }
    }
    fn turn(&mut self, id: &str, revision: u64) -> &mut Turn {
        if let Some(index) = self.turns.iter().position(|t| t.id == id) {
            return &mut self.turns[index];
        }
        self.turns.push(Turn::new(id, revision));
        self.turns.last_mut().unwrap()
    }
    pub fn active_turn(&self) -> Option<&Turn> {
        self.turns.iter().rev().find(|t| t.active())
    }
}

#[derive(Default)]
pub struct Mirror {
    revision: u64,
    threads: HashMap<String, Thread>,
}
impl Mirror {
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn thread(&self, id: &str) -> Option<&Thread> {
        self.threads.get(id)
    }
    pub fn forget(&mut self, id: &str) {
        self.threads.remove(id);
    }
    pub fn unload(&mut self, id: &str) {
        let revision = self.next_revision();
        if let Some(thread) = self.threads.get_mut(id) {
            thread.token_usage_current = false;
            thread.settings_current = false;
            thread.metadata_current = false;
            thread.metadata_revision = revision;
            thread.status = None;
            for notice in &mut thread.notices {
                notice.current = false;
            }
            thread.status_revision = revision;
            thread.settings_revision = revision;
        }
    }
    pub fn rename(&mut self, id: &str, name: Option<&str>, requested_at: u64) {
        let revision = self.next_revision();
        let thread = self.thread_mut(id);
        if thread.name_revision <= requested_at {
            thread.name = name.map(str::to_owned);
            thread.name_revision = revision;
        }
    }
    pub fn disconnect(&mut self) {
        let revision = self.next_revision();
        for thread in self.threads.values_mut() {
            thread.token_usage_current = false;
            thread.settings_current = false;
            thread.settings_revision = revision;
            thread.metadata_current = false;
            thread.metadata_revision = revision;
            for notice in &mut thread.notices {
                notice.current = false;
            }
        }
    }
    fn next_revision(&mut self) -> u64 {
        self.revision = self.revision.saturating_add(1);
        self.revision
    }
    fn thread_mut(&mut self, id: &str) -> &mut Thread {
        self.threads
            .entry(id.into())
            .or_insert_with(|| Thread::new(id))
    }

    /// A destructive history change creates a barrier for every older read/page.
    /// Clearing the display is not evidence that the native operation succeeded.
    pub(crate) fn invalidate_history(&mut self, id: &str) {
        let revision = self.next_revision();
        let thread = self.thread_mut(id);
        thread.history_floor = revision;
        thread.turns.clear();
        thread.token_usage = None;
        thread.token_usage_turn_id = None;
        thread.token_usage_current = false;
        thread.token_usage_observed_at_ms = None;
        thread.token_usage_model = None;
        thread.notices.clear();
    }

    pub(crate) fn invalidate_metadata(&mut self, id: &str) {
        let revision = self.next_revision();
        let thread = self.thread_mut(id);
        thread.metadata_current = false;
        thread.metadata_revision = revision;
    }

    pub(crate) fn hydrate_metadata(
        &mut self,
        thread: &Value,
        requested_at: u64,
    ) -> Result<(), String> {
        let id = required(thread, "id")?;
        let metadata = crate::thread_metadata::NativeMetadata::read(thread)?;
        let revision = self.next_revision();
        let target = self.thread_mut(id);
        if target.metadata_revision > requested_at {
            return Err("Native metadata changed while the request was pending".into());
        }
        target.metadata = Some(metadata);
        target.metadata_current = true;
        target.metadata_revision = revision;
        Ok(())
    }

    /// Hydrate an official response; preserve events received AFTER the caller
    /// issued its read/resume request. Empty response item arrays do not discard
    /// already-streamed items. A late start response cannot reopen a completed turn.
    pub fn hydrate(&mut self, thread: &Value, requested_at: u64) -> Result<(), String> {
        let id = required(thread, "id")?;
        if self
            .thread(id)
            .is_some_and(|t| requested_at < t.history_floor)
        {
            return Err("History changed after this native read was issued; read again".into());
        }
        let revision = self.next_revision();
        let target = self.thread_mut(id);
        if target.metadata_revision <= requested_at {
            target.metadata = crate::thread_metadata::NativeMetadata::read(thread).ok();
            target.metadata_current = target.metadata.is_some();
            target.metadata_revision = revision;
        }
        if target.name_revision <= requested_at {
            target.name = thread
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_owned);
            target.name_revision = revision;
        }
        if target.status_revision <= requested_at {
            target.status = thread.get("status").cloned();
            target.history_mode = thread["historyMode"]
                .as_str()
                .filter(|mode| matches!(*mode, "legacy" | "paginated"))
                .map(str::to_owned);
            target.status_revision = revision;
        }
        merge_turn_snapshot(
            target,
            thread["turns"].as_array().map(Vec::as_slice).unwrap_or(&[]),
            requested_at,
            revision,
        )?;
        Ok(())
    }

    /// Merge pages returned by `thread/turns/list` without replacing the
    /// metadata and live session settings established by `thread/read` or
    /// `thread/resume`.
    pub fn hydrate_turns(
        &mut self,
        thread_id: &str,
        turns: &[Value],
        requested_at: u64,
    ) -> Result<(), String> {
        if self
            .thread(thread_id)
            .is_some_and(|t| requested_at < t.history_floor)
        {
            return Err("History changed after this native page sequence began; read again".into());
        }
        let revision = self.next_revision();
        let mut target = self.thread_mut(thread_id).clone();
        merge_turn_snapshot(&mut target, turns, requested_at, revision)?;
        self.threads.insert(thread_id.into(), target);
        Ok(())
    }

    /// Expand one known turn without reordering unrelated turns or touching
    /// delivery receipts, metadata, native storage or session permissions.
    pub fn hydrate_turn_detail(
        &mut self,
        thread_id: &str,
        turn: &Value,
        requested_at: u64,
    ) -> Result<(), String> {
        let id = required(turn, "id")?;
        let thread = self
            .thread(thread_id)
            .ok_or("The conversation is no longer available")?;
        if turn["itemsView"] != "full" || !thread.turns.iter().any(|t| t.id == id) {
            return Err("Full history must belong to a known turn".into());
        }
        let order: HashMap<String, usize> = thread
            .turns
            .iter()
            .enumerate()
            .map(|(i, t)| (t.id.clone(), i))
            .collect();
        self.hydrate_turns(thread_id, std::slice::from_ref(turn), requested_at)?;
        self.thread_mut(thread_id)
            .turns
            .sort_by_key(|t| order.get(&t.id).copied().unwrap_or(usize::MAX));
        Ok(())
    }

    /// Hydrate a start/resume/fork response and publish only its stable,
    /// display-safe settings envelope. Parsing happens before mirror mutation.
    pub fn hydrate_session_response(
        &mut self,
        thread: &Value,
        response: &Value,
        requested_at: u64,
    ) -> Result<(), String> {
        let id = required(thread, "id")?.to_owned();
        let has_settings_envelope = [
            "cwd",
            "model",
            "modelProvider",
            "approvalPolicy",
            "approvalsReviewer",
            "sandbox",
        ]
        .iter()
        .any(|field| response.get(field).is_some());
        let settings = has_settings_envelope
            .then(|| crate::thread_settings::ThreadSettings::read_session_response(response))
            .transpose()?;
        self.hydrate(thread, requested_at)?;
        if let Some(settings) = settings {
            let revision = self.revision;
            let target = self
                .threads
                .get_mut(&id)
                .ok_or("Native session settings targeted an unknown thread")?;
            if target.settings_revision <= requested_at {
                target.settings = Some(settings);
                target.settings_current = true;
                target.settings_revision = revision;
            }
        }
        Ok(())
    }

    /// Unknown notifications are left to the host's diagnostic handling.
    pub fn notify(&mut self, method: &str, params: &Value) -> Result<bool, String> {
        if let Some(notice) = ModelNotice::parse(method, params)? {
            let thread_id = required(params, "threadId")?;
            let turn_id = if matches!(method, "warning" | "guardianWarning") {
                params.get("turnId").and_then(Value::as_str).unwrap_or("")
            } else {
                required(params, "turnId")?
            };
            let revision = self.next_revision();
            model_notices::update(
                &mut self.thread_mut(thread_id).notices,
                turn_id,
                notice,
                revision,
            );
            // A notification can precede turn/started. Do not create a phantom
            // active turn, clear a pending submission, or infer native completion.
            return Ok(true);
        }
        let supported = matches!(
            method,
            "turn/started"
                | "turn/completed"
                | "item/started"
                | "item/completed"
                | "item/agentMessage/delta"
                | "item/plan/delta"
                | "item/reasoning/summaryTextDelta"
                | "item/reasoning/summaryPartAdded"
                | "item/commandExecution/outputDelta"
                | "item/commandExecution/terminalInteraction"
                | "item/fileChange/patchUpdated"
                | "item/mcpToolCall/progress"
                | "turn/diff/updated"
                | "turn/plan/updated"
                | "thread/status/changed"
                | "thread/tokenUsage/updated"
                | "thread/name/updated"
                | "thread/settings/updated"
        );
        if !supported {
            return Ok(false);
        }
        if method == "item/mcpToolCall/progress" {
            required(params, "turnId")?;
            required(params, "itemId")?;
            required(params, "message")?;
        }
        if method == "item/fileChange/patchUpdated" {
            required(params, "turnId")?;
            required(params, "itemId")?;
            let changes = params["changes"]
                .as_array()
                .ok_or("Native patch update omitted its changes array")?;
            for change in changes {
                required(change, "path")?;
                required(change, "diff")?;
            }
        }
        if method == "item/commandExecution/terminalInteraction" {
            required(params, "turnId")?;
            required(params, "itemId")?;
            required(params, "processId")?;
            if !params["stdin"].is_string() {
                return Err("Native terminal interaction omitted its input".into());
            }
        }
        let thread_id = required(params, "threadId")?;
        let revision = self.next_revision();
        let thread = self.thread_mut(thread_id);
        if method == "thread/settings/updated" {
            thread.settings_current = false;
            thread.settings_revision = revision;
            thread.settings = Some(crate::thread_settings::ThreadSettings::read(
                &params["threadSettings"],
            )?);
            thread.settings_current = true;
            return Ok(true);
        }
        if method == "thread/name/updated" {
            if params.get("threadName").is_some_and(|v| !v.is_string()) {
                return Err("Native conversation name has an invalid type".into());
            }
            thread.name = params
                .get("threadName")
                .and_then(Value::as_str)
                .map(str::to_owned);
            thread.name_revision = revision;
            return Ok(true);
        }
        if method == "thread/status/changed" {
            thread.status = Some(params["status"].clone());
            if params["status"]["type"] == "notLoaded" {
                thread.settings_current = false;
                thread.settings_revision = revision;
                for notice in &mut thread.notices {
                    notice.current = false;
                }
            }
            thread.status_revision = revision;
            return Ok(true);
        }
        if method == "thread/tokenUsage/updated" {
            let turn_id = required(params, "turnId")?;
            if !params["tokenUsage"].is_object() {
                return Err("Native token usage event omitted its report".into());
            }
            let incoming = thread.turns.iter().position(|t| t.id == turn_id);
            let previous = thread
                .token_usage_turn_id
                .as_ref()
                .and_then(|id| thread.turns.iter().position(|t| &t.id == id));
            if incoming
                .zip(previous)
                .is_some_and(|(incoming, previous)| incoming < previous)
            {
                return Ok(false);
            }
            thread.token_usage = Some(params["tokenUsage"].clone());
            thread.token_usage_turn_id = Some(turn_id.into());
            thread.token_usage_current = true;
            thread.token_usage_model = thread
                .settings_current
                .then_some(thread.settings.as_ref())
                .flatten()
                .map(|settings| settings.model.clone());
            thread.token_usage_observed_at_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .and_then(|duration| u64::try_from(duration.as_millis()).ok());
            return Ok(true);
        }
        if matches!(method, "turn/started" | "turn/completed") {
            let data = &params["turn"];
            let turn = thread.turn(required(data, "id")?, revision);
            let complete = method == "turn/completed";
            if turn.active() || complete {
                turn.status = data["status"]
                    .as_str()
                    .unwrap_or(if complete { "failed" } else { "inProgress" })
                    .into();
                turn.error = data.get("error").filter(|v| !v.is_null()).cloned();
                turn.timing(data);
                turn.revision = revision;
            }
            for item in data["items"].as_array().into_iter().flatten() {
                let id = required(item, "id")?;
                let kind = required(item, "type")?;
                let existing = turn.item(id, kind, revision);
                if complete || !existing.completed {
                    existing.replace(item.clone(), complete || item_finished(item), revision);
                }
            }
            return Ok(true);
        }
        let turn = thread.turn(required(params, "turnId")?, revision);
        if method == "turn/diff/updated" {
            turn.diff = params["diff"].as_str().map(str::to_owned);
            return Ok(true);
        }
        if method == "turn/plan/updated" {
            turn.plan = Some(params.clone());
            return Ok(true);
        }
        if matches!(method, "item/started" | "item/completed") {
            let data = &params["item"];
            let item = turn.item(required(data, "id")?, required(data, "type")?, revision);
            if !item.completed || method == "item/completed" {
                item.replace(data.clone(), method == "item/completed", revision);
            }
            return Ok(true);
        }
        if method == "item/mcpToolCall/progress" {
            let item = turn.item(required(params, "itemId")?, "mcpToolCall", revision);
            if item.value["type"] != "mcpToolCall" {
                return Err("Native MCP progress addressed a different item type".into());
            }
            if !item.completed {
                item.progress_messages
                    .push(required(params, "message")?.into());
                item.revision = revision;
            }
            return Ok(true);
        }
        if method == "item/fileChange/patchUpdated" {
            let item = turn.item(required(params, "itemId")?, "fileChange", revision);
            if item.value["type"] != "fileChange" {
                return Err("Native patch update addressed a different item type".into());
            }
            if !item.completed {
                // A complete replacement snapshot, not patch text to append or
                // apply. Only item/completed may establish the final outcome.
                item.value["changes"] = params["changes"].clone();
                item.revision = revision;
            }
            return Ok(true);
        }
        if method == "item/commandExecution/terminalInteraction" {
            let item = turn.item(required(params, "itemId")?, "commandExecution", revision);
            if item.value["type"] != "commandExecution" {
                return Err("Native terminal interaction addressed a different item type".into());
            }
            if !item.completed && item.progress_messages.len() < 64 {
                // stdin may contain credentials or user input. Retain only its
                // UTF-8 byte count and never the process/input payload itself.
                let bytes = params["stdin"].as_str().unwrap().len();
                item.progress_messages.push(format!(
                    "Sent {bytes} bytes of input to the running native process"
                ));
                item.revision = revision;
            }
            return Ok(true);
        }
        let (kind, field) = match method {
            "item/agentMessage/delta" => ("agentMessage", "text"),
            "item/plan/delta" => ("plan", "text"),
            "item/commandExecution/outputDelta" => ("commandExecution", "aggregatedOutput"),
            _ => ("reasoning", "summary"),
        };
        let item = turn.item(required(params, "itemId")?, kind, revision);
        if item.completed {
            return Ok(true);
        }
        if kind == "reasoning" {
            let index = params["summaryIndex"]
                .as_u64()
                .ok_or("Invalid reasoning summary index")?;
            let section = item.summaries.entry(index).or_default();
            if method == "item/reasoning/summaryTextDelta" {
                section.push_str(required(params, "delta")?);
            }
            item.value["summary"] = json!(item.summaries.values().collect::<Vec<_>>());
            item.revision = revision;
        } else {
            item.append(field, required(params, "delta")?, revision);
        }
        Ok(true)
    }
}

fn merge_turn_snapshot(
    target: &mut Thread,
    turns: &[Value],
    requested_at: u64,
    revision: u64,
) -> Result<(), String> {
    let turn_order: HashMap<&str, usize> = turns
        .iter()
        .enumerate()
        .filter_map(|(index, turn)| turn["id"].as_str().map(|id| (id, index)))
        .collect();
    for turn in turns {
        let turn_id = required(turn, "id")?;
        let destination = target.turn(turn_id, 0);
        let incoming_status = turn["status"].as_str().unwrap_or("inProgress");
        if destination.revision <= requested_at
            && (destination.active() || incoming_status != "inProgress")
        {
            destination.status = incoming_status.into();
            destination.error = turn.get("error").filter(|v| !v.is_null()).cloned();
            destination.timing(turn);
            destination.revision = revision;
        }
        for item in turn["items"].as_array().into_iter().flatten() {
            let item_id = required(item, "id")?;
            let kind = required(item, "type")?;
            let dest = destination.item(item_id, kind, 0);
            if dest.revision <= requested_at {
                dest.replace(
                    item.clone(),
                    incoming_status != "inProgress" || item_finished(item),
                    revision,
                );
            }
        }
        // A later lightweight reload must not move the final answer ahead of
        // already-expanded activity or downgrade the turn's complete history.
        if turn["itemsView"] != "summary" || destination.items_view.as_deref() != Some("full") {
            let order = snapshot_order(&turn["items"]);
            destination
                .items
                .sort_by_key(|item| order.get(item.id.as_str()).copied().unwrap_or(usize::MAX));
        }
        if turn["itemsView"] == "full" || destination.items_view.is_none() {
            destination.items_view = turn["itemsView"].as_str().map(str::to_owned);
        }
    }
    target.turns.sort_by_key(|turn| {
        turn_order
            .get(turn.id.as_str())
            .copied()
            .unwrap_or(usize::MAX)
    });
    Ok(())
}

fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value[key]
        .as_str()
        .ok_or_else(|| format!("App Server event is missing {key}"))
}
fn snapshot_order(items: &Value) -> HashMap<&str, usize> {
    items
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(i, item)| item["id"].as_str().map(|id| (id, i)))
        .collect()
}
fn item_finished(item: &Value) -> bool {
    item["status"]
        .as_str()
        .is_some_and(|s| matches!(s, "completed" | "failed" | "declined" | "interrupted"))
}

#[cfg(test)]
#[path = "mirror/file_changes_tests.rs"]
mod file_changes_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanding_one_turn_preserves_turn_order_and_survives_summary_reload() {
        let mut mirror = Mirror::default();
        let summary = |id: &str| {
            json!({"id":id,"status":"completed","itemsView":"summary","items":[
            {"id":format!("{id}-user"),"type":"userMessage","content":[]},
            {"id":format!("{id}-final"),"type":"agentMessage","text":"Done","phase":"final_answer"}]})
        };
        mirror.hydrate(&json!({"id":"thread","turns":[summary("before"),summary("selected"),summary("after")]}), 0).unwrap();
        let detail = json!({"id":"selected","status":"completed","itemsView":"full","items":[
            {"id":"selected-user","type":"userMessage","content":[]},
            {"id":"command","type":"commandExecution","status":"completed"},
            {"id":"checkpoint","type":"agentMessage","text":"Files verified","phase":"commentary"},
            {"id":"reasoning","type":"reasoning","summary":["Public summary"],"content":["PRIVATE"]},
            {"id":"selected-final","type":"agentMessage","text":"Done","phase":"final_answer"}]});
        mirror
            .hydrate_turn_detail("thread", &detail, mirror.revision())
            .unwrap();
        mirror
            .hydrate_turns(
                "thread",
                &[summary("before"), summary("selected"), summary("after")],
                mirror.revision(),
            )
            .unwrap();
        let thread = mirror.thread("thread").unwrap();
        assert_eq!(
            thread
                .turns
                .iter()
                .map(|t| t.id.as_str())
                .collect::<Vec<_>>(),
            ["before", "selected", "after"]
        );
        let turn = &thread.turns[1];
        assert_eq!(turn.items_view.as_deref(), Some("full"));
        assert_eq!(
            turn.items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            [
                "selected-user",
                "command",
                "checkpoint",
                "reasoning",
                "selected-final"
            ]
        );
        assert!(turn.items[3].value["content"].is_null());
        assert_eq!(turn.items[3].value["summary"][0], "Public summary");
        let before = serde_json::to_value(thread).unwrap();
        let mut foreign = detail.clone();
        foreign["id"] = json!("not-present");
        assert!(
            mirror
                .hydrate_turn_detail("thread", &foreign, mirror.revision())
                .is_err()
        );
        assert_eq!(
            serde_json::to_value(mirror.thread("thread").unwrap()).unwrap(),
            before
        );
    }

    fn session_response(model: &str) -> Value {
        json!({
            "cwd":"C:\\fixture",
            "model":model,
            "modelProvider":"openai",
            "serviceTier":null,
            "reasoningEffort":"low",
            "approvalPolicy":"on-request",
            "approvalsReviewer":"user",
            "sandbox":{"type":"readOnly","networkAccess":false}
        })
    }

    fn session_thread() -> Value {
        json!({"id":"native-a","status":{"type":"idle"},"turns":[]})
    }

    #[test]
    fn paginated_turn_hydration_preserves_metadata_and_newer_live_events() {
        let mut mirror = Mirror::default();
        mirror
            .hydrate(
                &json!({
                    "id":"native",
                    "name":"Fixture",
                    "historyMode":"paginated",
                    "status":{"type":"idle"},
                    "turns":[]
                }),
                0,
            )
            .unwrap();
        let requested_at = mirror.revision();
        mirror
            .notify(
                "turn/started",
                &json!({"threadId":"native","turn":{"id":"live","status":"inProgress","items":[]}}),
            )
            .unwrap();
        mirror
            .hydrate_turns(
                "native",
                &[
                    json!({"id":"older","status":"completed","items":[],"itemsView":"summary"}),
                    json!({"id":"live","status":"completed","items":[],"itemsView":"summary"}),
                ],
                requested_at,
            )
            .unwrap();
        let thread = mirror.thread("native").unwrap();
        assert_eq!(thread.name.as_deref(), Some("Fixture"));
        assert_eq!(thread.history_mode.as_deref(), Some("paginated"));
        assert_eq!(thread.status.as_ref().unwrap()["type"], "idle");
        assert_eq!(thread.turns[0].id, "older");
        assert_eq!(thread.turns[1].id, "live");
        assert_eq!(thread.turns[1].status, "inProgress");
    }

    #[test]
    fn native_mcp_progress_keeps_identity_order_and_completion_without_native_history_fields() {
        let samples: Vec<Value> =
            serde_json::from_str(include_str!("../tests/native-mcp-progress.json")).unwrap();
        let mut mirror = Mirror::default();
        let snapshot = json!({"id":"main","turns":[{"id":"turn","status":"inProgress","items":[{"id":"tool","type":"mcpToolCall","status":"inProgress","server":"fixture","tool":"test","arguments":{}}]}]});
        mirror.hydrate(&snapshot, 0).unwrap();
        let read_revision = mirror.revision();
        for event in &samples {
            assert!(mirror.notify("item/mcpToolCall/progress", event).unwrap());
        }
        mirror.hydrate(&snapshot, read_revision).unwrap();
        assert!(mirror.thread("graph").is_none());
        let completed = json!({"id":"tool","type":"mcpToolCall","status":"failed","server":"fixture","tool":"test","arguments":{},"error":{"message":"Test failure"}});
        mirror
            .notify(
                "item/completed",
                &json!({"threadId":"main","turnId":"turn","item":completed}),
            )
            .unwrap();
        mirror
            .notify("item/mcpToolCall/progress", &samples[0])
            .unwrap();
        let item = &mirror.thread("main").unwrap().turns[0].items[0];
        assert_eq!(
            item.progress_messages,
            samples
                .iter()
                .map(|v| v["message"].as_str().unwrap())
                .collect::<Vec<_>>()
        );
        assert_eq!(item.value, completed);
        assert!(
            serde_json::to_value(item)
                .unwrap()
                .get("progressMessages")
                .is_none()
        );
        let mut restored = Mirror::default();
        restored.hydrate(&snapshot, 0).unwrap();
        assert!(
            restored.thread("main").unwrap().turns[0].items[0]
                .progress_messages
                .is_empty()
        );
    }
    #[test]
    fn native_mcp_progress_rejects_malformed_or_mismatched_items() {
        let mut mirror = Mirror::default();
        let mut event = json!({"threadId":"main","turnId":"turn","itemId":"tool","message":{}});
        assert!(mirror.notify("item/mcpToolCall/progress", &event).is_err());
        assert!(mirror.thread("main").is_none());
        event["message"] = json!("Progress");
        mirror
            .notify(
                "item/agentMessage/delta",
                &json!({"threadId":"main","turnId":"turn","itemId":"tool","delta":"Original"}),
            )
            .unwrap();
        assert!(mirror.notify("item/mcpToolCall/progress", &event).is_err());
        assert_eq!(
            mirror.thread("main").unwrap().turns[0].items[0].value["text"],
            "Original"
        );
    }
    #[test]
    fn terminal_interaction_reports_activity_without_retaining_stdin_or_process_id() {
        let mut mirror = Mirror::default();
        mirror
            .hydrate(
                &json!({"id":"main","turns":[{"id":"turn","status":"inProgress","items":[{
                    "id":"command","type":"commandExecution","status":"inProgress","command":"fixture"
                }]}]}),
                0,
            )
            .unwrap();
        let stdin = "PRIVATE_STDIN\n";
        assert!(
            mirror
                .notify(
                    "item/commandExecution/terminalInteraction",
                    &json!({"threadId":"main","turnId":"turn","itemId":"command","processId":"PRIVATE_PROCESS","stdin":stdin})
                )
                .unwrap()
        );
        let item = &mirror.thread("main").unwrap().turns[0].items[0];
        assert_eq!(
            item.progress_messages,
            vec![format!(
                "Sent {} bytes of input to the running native process",
                stdin.len()
            )]
        );
        let public = serde_json::to_string(mirror.thread("main").unwrap()).unwrap();
        assert!(!public.contains("PRIVATE_STDIN"));
        assert!(!public.contains("PRIVATE_PROCESS"));

        mirror
            .notify(
                "item/completed",
                &json!({"threadId":"main","turnId":"turn","item":{"id":"command","type":"commandExecution","status":"completed","command":"fixture"}}),
            )
            .unwrap();
        mirror
            .notify(
                "item/commandExecution/terminalInteraction",
                &json!({"threadId":"main","turnId":"turn","itemId":"command","processId":"late","stdin":"late"}),
            )
            .unwrap();
        assert_eq!(
            mirror.thread("main").unwrap().turns[0].items[0]
                .progress_messages
                .len(),
            1
        );
    }
    #[test]
    fn history_format_is_native_metadata_and_unknown_is_not_legacy() {
        let mut mirror = Mirror::default();
        for mode in [
            json!("paginated"),
            json!("legacy"),
            Value::Null,
            json!("future-format"),
        ] {
            let revision = mirror.revision();
            mirror
                .hydrate(
                    &json!({"id":"native","historyMode":mode,"status":{"type":"idle"},"turns":[]}),
                    revision,
                )
                .unwrap();
            let expected = mode
                .as_str()
                .filter(|mode| matches!(*mode, "legacy" | "paginated"));
            assert_eq!(
                mirror.thread("native").unwrap().history_mode.as_deref(),
                expected
            );
        }
        let old = mirror.revision();
        mirror
            .notify(
                "thread/status/changed",
                &json!({"threadId":"native","status":{"type":"active"}}),
            )
            .unwrap();
        mirror
            .hydrate(
                &json!({"id":"native","historyMode":"legacy","status":{"type":"idle"},"turns":[]}),
                old,
            )
            .unwrap();
        assert!(mirror.thread("native").unwrap().history_mode.is_none());
    }
    #[test]
    fn resume_restores_native_order_without_losing_newer_live_text() {
        let mut m = Mirror::default();
        let requested_at = m.revision();
        m.notify(
            "item/agentMessage/delta",
            &json!({"threadId":"a","turnId":"t2","itemId":"i2","delta":"new"}),
        )
        .unwrap();
        m.notify(
            "thread/status/changed",
            &json!({"threadId":"a","status":{"type":"active"}}),
        )
        .unwrap();
        m.hydrate(&json!({"id":"a","status":{"type":"idle"},"turns":[
            {"id":"t1","status":"completed","items":[]},
            {"id":"t2","status":"inProgress","items":[{"id":"i1","type":"userMessage","content":[]},{"id":"i2","type":"agentMessage","text":"old"}]}
        ]}),requested_at).unwrap();
        let t = m.thread("a").unwrap();
        assert_eq!(
            t.turns.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
            vec!["t1", "t2"]
        );
        assert_eq!(
            t.turns[1]
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec!["i1", "i2"]
        );
        assert_eq!(t.turns[1].items[1].value["text"], "new");
        assert_eq!(t.status.as_ref().unwrap()["type"], "active");
    }
    #[test]
    fn authoritative_final_replaces_deltas_and_late_start_does_not_reopen_turn() {
        let mut m = Mirror::default();
        let requested_at = m.revision();
        m.notify(
            "item/agentMessage/delta",
            &json!({"threadId":"a","turnId":"t","itemId":"i","delta":"partial"}),
        )
        .unwrap();
        m.notify("item/completed",&json!({"threadId":"a","turnId":"t","item":{"id":"i","type":"agentMessage","text":"authoritative","phase":"final_answer"}})).unwrap();
        m.notify(
            "turn/completed",
            &json!({"threadId":"a","turn":{"id":"t","status":"completed","items":[]}}),
        )
        .unwrap();
        m.hydrate(
            &json!({"id":"a","turns":[{"id":"t","status":"inProgress","items":[]}]}),
            requested_at,
        )
        .unwrap();
        m.notify(
            "item/agentMessage/delta",
            &json!({"threadId":"a","turnId":"t","itemId":"i","delta":"late"}),
        )
        .unwrap();
        let t = m.thread("a").unwrap();
        assert!(t.active_turn().is_none());
        assert_eq!(t.turns[0].items.len(), 1);
        assert_eq!(t.turns[0].items[0].value["text"], "authoritative");
    }
    #[test]
    fn equal_item_ids_in_different_threads_do_not_mix() {
        let mut m = Mirror::default();
        for (thread, text) in [("main", "one"), ("graph", "two")] {
            m.notify(
                "item/agentMessage/delta",
                &json!({"threadId":thread,"turnId":"t","itemId":"i","delta":text}),
            )
            .unwrap();
        }
        assert_eq!(
            m.thread("main").unwrap().turns[0].items[0].value["text"],
            "one"
        );
        assert_eq!(
            m.thread("graph").unwrap().turns[0].items[0].value["text"],
            "two"
        );
    }
    #[test]
    fn reasoning_uses_only_exposed_summaries_and_sparse_indexes() {
        let mut m = Mirror::default();
        m.notify("item/reasoning/summaryTextDelta",&json!({"threadId":"a","turnId":"t","itemId":"i","summaryIndex":99999,"delta":"Summary"})).unwrap();
        assert!(
            !m.notify(
                "item/reasoning/textDelta",
                &json!({"threadId":"a","turnId":"t","itemId":"i","delta":"private"})
            )
            .unwrap()
        );
        m.notify("item/completed",&json!({"threadId":"a","turnId":"t","item":{"id":"i","type":"reasoning","summary":["Final summary"],"content":["raw reasoning"]}})).unwrap();
        let data = &m.thread("a").unwrap().turns[0].items[0].value;
        assert_eq!(data["summary"], json!(["Final summary"]));
        assert!(data.get("content").is_none());
    }
    #[test]
    fn read_response_preserves_events_newer_than_the_request() {
        let mut m = Mirror::default();
        let before = m.revision();
        m.notify(
            "item/agentMessage/delta",
            &json!({"threadId":"a","turnId":"t","itemId":"i","delta":"new"}),
        )
        .unwrap();
        m.hydrate(&json!({"id":"a","turns":[{"id":"t","status":"inProgress","items":[{"id":"i","type":"agentMessage","text":"stale"}]}]}),before).unwrap();
        assert_eq!(
            m.thread("a").unwrap().turns[0].items[0].value["text"],
            "new"
        );
    }

    #[test]
    fn session_settings_respect_notification_order_in_both_directions() {
        let event: Value =
            serde_json::from_str(include_str!("../tests/thread-settings.json")).unwrap();

        let mut notification_first = Mirror::default();
        let requested_at = notification_first.revision();
        notification_first
            .notify("thread/settings/updated", &event)
            .unwrap();
        notification_first
            .hydrate_session_response(
                &session_thread(),
                &session_response("stale-response-model"),
                requested_at,
            )
            .unwrap();
        let settings = notification_first
            .thread("native-a")
            .unwrap()
            .settings
            .as_ref()
            .unwrap();
        assert_eq!(settings.model, "fixture-model");
        assert_eq!(settings.summary.as_deref(), Some("concise"));

        let mut response_first = Mirror::default();
        let requested_at = response_first.revision();
        response_first
            .hydrate_session_response(
                &session_thread(),
                &session_response("response-model"),
                requested_at,
            )
            .unwrap();
        assert_eq!(
            response_first
                .thread("native-a")
                .unwrap()
                .settings
                .as_ref()
                .unwrap()
                .model,
            "response-model"
        );
        response_first
            .notify("thread/settings/updated", &event)
            .unwrap();
        let thread = response_first.thread("native-a").unwrap();
        assert!(thread.settings_current);
        assert_eq!(thread.settings.as_ref().unwrap().model, "fixture-model");
    }

    #[test]
    fn not_loaded_or_disconnected_thread_cannot_be_revalidated_by_a_late_ack() {
        let event: Value =
            serde_json::from_str(include_str!("../tests/thread-settings.json")).unwrap();
        for disconnected in [false, true] {
            let mut mirror = Mirror::default();
            mirror.notify("thread/settings/updated", &event).unwrap();
            let requested_at = mirror.revision();
            if disconnected {
                mirror.disconnect();
            } else {
                mirror
                    .notify(
                        "thread/status/changed",
                        &json!({"threadId":"native-a","status":{"type":"notLoaded"}}),
                    )
                    .unwrap();
            }
            mirror
                .hydrate_session_response(
                    &session_thread(),
                    &session_response("late-response-model"),
                    requested_at,
                )
                .unwrap();
            let thread = mirror.thread("native-a").unwrap();
            assert!(!thread.settings_current);
            assert_eq!(thread.settings.as_ref().unwrap().model, "fixture-model");
        }
    }
    #[test]
    fn native_usage_tracks_turns_decreases_and_connection_freshness() {
        let samples: Vec<Value> =
            serde_json::from_str(include_str!("../tests/token-usage.json")).unwrap();
        let mut mirror = Mirror::default();
        for id in ["turn-1", "turn-2"] {
            mirror.notify("turn/started", &json!({"threadId":"usage-thread","turn":{"id":id,"status":"inProgress","items":[]}})).unwrap();
        }
        for sample in &samples {
            assert!(mirror.notify("thread/tokenUsage/updated", sample).unwrap());
        }
        let thread = mirror.thread("usage-thread").unwrap();
        assert_eq!(
            thread.token_usage.as_ref().unwrap()["last"]["totalTokens"],
            200
        );
        assert_eq!(
            thread.token_usage.as_ref().unwrap()["total"]["totalTokens"],
            95200
        );
        assert_eq!(thread.token_usage_turn_id.as_deref(), Some("turn-2"));
        assert!(
            !mirror
                .notify("thread/tokenUsage/updated", &samples[0])
                .unwrap()
        );
        assert_eq!(
            mirror
                .thread("usage-thread")
                .unwrap()
                .token_usage
                .as_ref()
                .unwrap()["last"]["totalTokens"],
            200
        );
        assert_eq!(
            mirror
                .thread("another-thread")
                .unwrap()
                .token_usage
                .as_ref()
                .unwrap()["last"]["totalTokens"],
            0
        );
        assert!(
            mirror
                .notify(
                    "thread/tokenUsage/updated",
                    &json!({"threadId":"usage-thread","turnId":"turn-2"})
                )
                .is_err()
        );
        mirror.disconnect();
        assert!(!mirror.thread("usage-thread").unwrap().token_usage_current);
        // A history read carries no token usage and cannot make old data fresh.
        mirror
            .hydrate(&json!({"id":"usage-thread","turns":[]}), mirror.revision())
            .unwrap();
        assert!(!mirror.thread("usage-thread").unwrap().token_usage_current);
        mirror
            .notify("thread/tokenUsage/updated", &samples[1])
            .unwrap();
        assert!(mirror.thread("usage-thread").unwrap().token_usage_current);
    }
    #[test]
    fn cache_report_records_local_receipt_and_original_model_only_for_fresh_usage() {
        let settings: Value =
            serde_json::from_str(include_str!("../tests/thread-settings.json")).unwrap();
        let mut mirror = Mirror::default();
        mirror.notify("thread/settings/updated", &settings).unwrap();
        mirror
            .notify(
                "thread/tokenUsage/updated",
                &json!({
                    "threadId":"native-a","turnId":"turn-a",
                    "tokenUsage":{"last":{"cachedInputTokens":100},"total":{}}
                }),
            )
            .unwrap();
        let thread = mirror.thread("native-a").unwrap();
        assert_eq!(thread.token_usage_model.as_deref(), Some("fixture-model"));
        assert!(
            thread
                .token_usage_observed_at_ms
                .is_some_and(|time| time > 0)
        );

        let mut changed = settings.clone();
        changed["threadSettings"]["model"] = json!("gpt-6-sol");
        mirror.notify("thread/settings/updated", &changed).unwrap();
        assert_eq!(
            mirror
                .thread("native-a")
                .unwrap()
                .token_usage_model
                .as_deref(),
            Some("fixture-model")
        );
        mirror.invalidate_history("native-a");
        let thread = mirror.thread("native-a").unwrap();
        assert!(thread.token_usage_observed_at_ms.is_none());
        assert!(thread.token_usage_model.is_none());
    }
    #[test]
    fn absent_usage_is_not_zero_and_diff_does_not_destroy_items() {
        let mut m = Mirror::default();
        m.notify(
            "item/commandExecution/outputDelta",
            &json!({"threadId":"a","turnId":"t","itemId":"i","delta":"output"}),
        )
        .unwrap();
        m.notify(
            "turn/diff/updated",
            &json!({"threadId":"a","turnId":"t","diff":"+line"}),
        )
        .unwrap();
        assert!(m.thread("a").unwrap().token_usage.is_none());
        assert_eq!(
            m.thread("a").unwrap().turns[0].items[0].value["aggregatedOutput"],
            "output"
        );
    }
}
