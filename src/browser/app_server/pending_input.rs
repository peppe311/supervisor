//! Display-only local echoes. Never persisted as native history or replayed.
use super::*;
use central_agent_codex_runtime::mirror::Thread;

#[derive(Default)]
pub(super) struct PendingInputs(BTreeMap<String, Vec<Input>>);

const MAX_LOCAL_PREVIEW_ROWS_PER_OWNER: usize = 24;

struct Input {
    message_id: String,
    row: Value,
    turn_id: Option<String>,
    after: Option<String>,
}

pub(super) fn row_id(client_id: &str) -> String {
    format!("native-input:{client_id}")
}

impl PendingInputs {
    pub(super) fn begin(
        &mut self,
        owner: &str,
        message_id: &str,
        text: &str,
        snapshots: &SubmissionSnapshots,
        turn_id: Option<&str>,
        after: Option<String>,
    ) {
        let (attachments, terminal_attachments) =
            attachments::native_snapshot_views(&snapshots.tabs, &snapshots.terminals);
        let row = json!({
            "id":row_id(message_id), "role":"user", "kind":"message",
            "provider":"codex_app_server", "text":text,
            "renderedHtml":crate::markdown::render_safe_markdown(text),
            "nativeClientMessageId":message_id, "submissionStatus":"sending",
            "attachments":attachments,
            "terminalAttachments":terminal_attachments,
            "fileAttachments":snapshots.files.iter().map(PendingFileAttachment::view).collect::<Vec<_>>()
        });
        self.0.entry(owner.into()).or_default().push(Input {
            message_id: message_id.into(),
            row,
            turn_id: turn_id.map(str::to_owned),
            after,
        });
    }

    pub(super) fn accept(&mut self, owner: &str, message_id: &str, turn_id: &str) {
        if let Some(input) = self
            .0
            .get_mut(owner)
            .into_iter()
            .flatten()
            .find(|input| input.message_id == message_id)
        {
            input.turn_id = Some(turn_id.into());
            input.row["submissionStatus"] = json!("accepted");
        }
    }

    pub(super) fn remove(&mut self, owner: &str, message_id: &str) {
        if let Some(inputs) = self.0.get_mut(owner) {
            inputs.retain(|input| input.message_id != message_id);
            if inputs.is_empty() {
                self.0.remove(owner);
            }
        }
    }

    pub(super) fn forget(&mut self, owner: &str) {
        self.0.remove(owner);
    }
    pub(super) fn clear(&mut self) {
        self.0.clear();
    }

    pub(super) fn reconcile(&mut self, conversations: &Conversations) {
        self.0.retain(|owner, inputs| {
            let thread = conversations
                .binding(owner)
                .and_then(|binding| conversations.mirror.thread(&binding.thread_id));
            // Once native history observes an input, retain only a bounded local
            // thumbnail overlay. The image stays on this device and is never
            // added to App Server history or replayed as provider input.
            inputs.retain(|input| {
                !observed(thread, &input.message_id) || has_local_tab_preview(&input.row)
            });
            let mut observed_previews = inputs
                .iter()
                .filter(|input| {
                    observed(thread, &input.message_id) && has_local_tab_preview(&input.row)
                })
                .count();
            inputs.retain(|input| {
                if observed_previews > MAX_LOCAL_PREVIEW_ROWS_PER_OWNER
                    && observed(thread, &input.message_id)
                    && has_local_tab_preview(&input.row)
                {
                    observed_previews -= 1;
                    false
                } else {
                    true
                }
            });
            !inputs.is_empty()
        });
    }

    pub(super) fn project(&self, owner: &str, thread: Option<&Thread>, rows: &mut Vec<Value>) {
        for input in self.0.get(owner).into_iter().flatten() {
            // A user item may arrive before its turn/start or steer response.
            if observed(thread, &input.message_id) {
                if let Some(row) = rows.iter_mut().find(|row| {
                    row["nativeClientMessageId"].as_str() == Some(input.message_id.as_str())
                }) {
                    overlay_local_tab_previews(&input.row, row);
                }
                continue;
            }
            let mut row = input.row.clone();
            if let Some((thread, turn)) = thread.zip(input.turn_id.as_deref()) {
                row["nativeTurnId"] = json!(turn);
                row["runId"] = json!(format!("native:{}:{turn}", thread.id));
            }
            let position = match &input.after {
                // Send now stays after the activities visible at dispatch.
                Some(id) => rows.iter().position(|row| row["id"] == *id).map(|i| i + 1),
                None => input
                    .turn_id
                    .as_ref()
                    .and_then(|turn| rows.iter().position(|row| row["nativeTurnId"] == *turn)),
            }
            .unwrap_or(rows.len());
            rows.insert(position, row);
        }
    }
}

fn has_local_tab_preview(row: &Value) -> bool {
    row["attachments"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|attachment| attachment["previewDataUrl"].as_str().is_some())
}

fn overlay_local_tab_previews(source: &Value, destination: &mut Value) {
    let Some(source) = source["attachments"].as_array() else {
        return;
    };
    let Some(destination) = destination["attachments"].as_array_mut() else {
        return;
    };
    for attachment in source {
        let Some(tab_id) = attachment["tabId"].as_u64() else {
            continue;
        };
        let Some(preview) = attachment["previewDataUrl"].as_str() else {
            continue;
        };
        if let Some(target) = destination
            .iter_mut()
            .find(|target| target["tabId"].as_u64() == Some(tab_id))
        {
            target["previewDataUrl"] = json!(preview);
        }
    }
}

fn observed(thread: Option<&Thread>, message_id: &str) -> bool {
    thread.is_some_and(|thread| {
        thread
            .turns
            .iter()
            .flat_map(|turn| &turn.items)
            .any(|item| item.value["type"] == "userMessage" && item.value["clientId"] == message_id)
    })
}
