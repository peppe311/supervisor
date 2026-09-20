//! Display-only native projection into the existing shared timeline.
use super::*;
use central_agent_codex_runtime::mirror::{Item, Thread, Turn};
mod activity;
mod media;
mod notices;
mod question_replies;

// A completed phase-less agentMessage can echo the same turn's native review
// result. Paginated history may place that echo before the exit item; adjacency
// is not stable. Keep the canonical exit row without changing native history.
fn review_result_echo(items: &[Item], item: &Item) -> bool {
    item.completed
        && item.value["type"] == "agentMessage"
        && item.value["phase"].is_null()
        && items.iter().any(|exit| {
            exit.completed
                && exit.value["type"] == "exitedReviewMode"
                && exit.value["review"]
                    .as_str()
                    .zip(item.value["text"].as_str())
                    .is_some_and(|(review, text)| {
                        !review.trim().is_empty() && review.trim() == text.trim()
                    })
        })
}

fn user_text(item: &Item) -> Option<String> {
    (item.value["type"] == "userMessage").then(|| visible_user_text(&item.value["content"]))
}

fn visible_user_text(content: &Value) -> String {
    let Some(parts) = content.as_array() else {
        return String::new();
    };
    let injected = parts
        .iter()
        .filter(|part| part["type"] == "skill")
        .filter_map(|part| part["name"].as_str())
        .map(|name| format!("${name}"))
        .collect::<HashSet<_>>();
    parts
        .iter()
        .filter(|part| part["type"] == "text")
        .filter_map(|part| part["text"].as_str())
        .filter(|text| !injected.contains(*text))
        .filter(|text| !attachments::is_native_context_input(text))
        .filter(|text| !crate::browser::agent_graph::is_supervised_agent_context_input(text))
        .filter(|text| !crate::browser::supervision::is_review_control_input(text))
        .collect::<Vec<_>>()
        .join("\n")
}

// Inline review can report a turn-ID prompt placeholder and a copy of the
// worker's user item in its supervising turn. Pagination also places that exact
// worker item in its own turn. Project one prompt, using native provenance; do
// not collapse unrelated equal text or mutate either native turn's history.
fn review_prompt_echo(thread: &Thread, turn: &Turn, item: &Item) -> bool {
    let Some(text) = user_text(item).filter(|text| !text.trim().is_empty()) else {
        return false;
    };
    if !turn.items.iter().any(|entry| {
        entry.value["type"] == "enteredReviewMode"
            && entry.value["review"]
                .as_str()
                .is_some_and(|review| review.trim() == text.trim())
    }) {
        return false;
    }
    if item.id == turn.id {
        return turn.items.iter().any(|worker| {
            worker.id != item.id && user_text(worker).is_some_and(|worker_text| worker_text == text)
        });
    }
    thread
        .turns
        .iter()
        .filter(|other| {
            other.id != turn.id
                && !other
                    .items
                    .iter()
                    .any(|entry| entry.value["type"] == "enteredReviewMode")
        })
        .flat_map(|other| &other.items)
        .any(|worker| {
            worker.id == item.id
                && worker.value["type"] == "userMessage"
                && worker.value["content"] == item.value["content"]
        })
}

impl BrowserApp {
    pub(in crate::browser) fn app_server_messages(&self, owner: &str) -> Vec<Value> {
        let thread = self
            .app_server
            .conversations
            .binding(owner)
            .and_then(|b| self.app_server.conversations.mirror.thread(&b.thread_id));
        let mut rows = thread
            .map(|thread| {
                let mut rows = messages(thread);
                for row in &mut rows {
                    if row["role"] != "user" {
                        continue;
                    }
                    if let Some(turn) = thread
                        .turns
                        .iter()
                        .find(|turn| row["nativeTurnId"] == turn.id)
                    {
                        row["nativeWorkHistory"] =
                            self.app_server.work_history.view(owner, &thread.id, turn);
                    }
                }
                rows
            })
            .unwrap_or_default();
        self.app_server
            .pending_inputs
            .project(owner, thread, &mut rows);
        rows
    }
    pub(in crate::browser) fn app_server_run_view(&self, owner: &str) -> Option<AgentRuntimeView> {
        let thread = self
            .app_server
            .conversations
            .binding(owner)
            .and_then(|b| self.app_server.conversations.mirror.thread(&b.thread_id));
        if thread.is_none() && !self.app_server.busy(owner) {
            return None;
        }
        let active = self.app_server.busy(owner);
        let phase = if active {
            AgentPhase::Acting
        } else {
            match thread
                .and_then(|t| t.turns.last())
                .map(|t| t.status.as_str())
            {
                Some("completed") => AgentPhase::Completed,
                Some("failed") => AgentPhase::Error,
                Some("interrupted") => AgentPhase::Stopped,
                _ => AgentPhase::Idle,
            }
        };
        Some(AgentRuntimeView {
            active,
            phase,
            status: if active { "Codex is working" } else { "" }.into(),
            ..AgentRuntimeView::idle()
        })
    }
}

fn messages(thread: &Thread) -> Vec<Value> {
    let mut rows = Vec::new();
    for turn in &thread.turns {
        let replies = question_replies::Replies::from_turn(turn);
        // The plan is a turn summary, not a late action after the final answer.
        // Keep it with the work after the leading user input, in both hosts.
        let mut plan = turn
            .plan
            .as_ref()
            .map(|plan| plan_message(thread, turn, plan));
        for item in &turn.items {
            if review_result_echo(&turn.items, item) || review_prompt_echo(thread, turn, item) {
                continue;
            }
            // Empty native reasoning is not a visible summary. Later deltas can
            // populate this same item; keep the raw native history untouched.
            if item.value["type"] == "reasoning"
                && !item.value["summary"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .any(|s| !s.trim().is_empty())
            {
                continue;
            }
            if item.value["type"] != "userMessage" {
                rows.extend(plan.take());
            }
            let mut row = item_message(thread, turn, item);
            replies.project(item, &mut row);
            rows.push(row);
            if let Some(output) = media::output(thread, turn, item) {
                rows.push(output);
            }
        }
        rows.extend(plan);
        if let Some(error) = &turn.error {
            let mut row = base(thread, turn, "error");
            row["role"] = json!("system");
            row["text"] = json!(error["message"].as_str().unwrap_or("The Codex turn failed"));
            rows.push(row);
        }
        if let Some(diff) = &turn.diff {
            let mut row = base(thread, turn, "turn-diff");
            row["kind"] = json!("activity");
            row["text"] = json!("Turn changes");
            row["activityCategory"] = json!("file_change");
            row["activityDiff"] = json!(diff);
            row["activityStatus"] = json!(if turn.active() {
                "running"
            } else {
                "completed"
            });
            rows.push(row);
        }
        rows.extend(
            thread
                .notices
                .iter()
                .filter(|notice| notice.turn_id == turn.id)
                .filter_map(|notice| notices::message(thread, notice)),
        );
    }
    rows.extend(
        thread
            .notices
            .iter()
            .filter(|notice| !thread.turns.iter().any(|turn| turn.id == notice.turn_id))
            .filter_map(|notice| notices::message(thread, notice)),
    );
    rows
}

// Redacted native structure used only by the explicit hidden startup check.
pub(super) fn work_reference_messages() -> Vec<Value> {
    let fixture: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/ui/tests/fixtures/native-work-outline.json"
    )))
    .expect("native work fixture");
    let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
    mirror
        .hydrate(&fixture, 0)
        .expect("valid native work fixture");
    let mut rows = messages(mirror.thread("reference-thread").unwrap());
    for row in &mut rows {
        if row["role"] == "user" {
            row["nativeWorkHistory"] = json!({"owner":"chat:work-reference","threadId":"reference-thread","turnId":"reference-turn","state":"loaded"});
        }
    }
    rows
}
fn plan_message(thread: &Thread, turn: &Turn, plan: &Value) -> Value {
    let steps = plan["plan"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let complete = !steps.is_empty() && steps.iter().all(|step| step["status"] == "completed");
    let status = if complete {
        "completed"
    } else if turn.active() {
        "running"
    } else if turn.status == "failed" {
        "error"
    } else {
        "stopped"
    };
    let mut row = base(thread, turn, "native-plan");
    row["kind"] = json!("activity");
    row["text"] = json!(if !complete && !turn.active() {
        "Plan · not completed"
    } else {
        "Plan"
    });
    row["activityCategory"] = json!("tool");
    row["activityStatus"] = json!(status);
    let detail = steps
        .iter()
        .map(|step| {
            let status = match step["status"].as_str() {
                Some("completed") => "Completed",
                Some("inProgress") => "In progress",
                Some("pending") => "Pending",
                _ => "Not reported",
            };
            format!("[{status}] {}", step["step"].as_str().unwrap_or_default())
        })
        .collect::<Vec<_>>()
        .join("\n");
    row["activityDetail"] = json!(
        format!(
            "{}\n{detail}",
            plan["explanation"].as_str().unwrap_or_default()
        )
        .trim()
    );
    row
}
fn base(thread: &Thread, turn: &Turn, id: &str) -> Value {
    json!({"id":format!("native:{}:{}:{id}",thread.id,turn.id),"role":"assistant","kind":"message",
        "provider":"codex_app_server","runId":format!("native:{}:{}",thread.id,turn.id),
        "nativeTurnId":turn.id,"timestampMs":turn.started_at_ms,
        "nativeTurnTiming":{"startedAtMs":turn.started_at_ms,"completedAtMs":turn.completed_at_ms,"durationMs":turn.duration_ms}})
}
fn item_message(thread: &Thread, turn: &Turn, item: &Item) -> Value {
    let mut row = base(thread, turn, &item.id);
    let v = &item.value;
    let kind = v["type"].as_str().unwrap_or("unknown");
    let text = match kind {
        "userMessage" => {
            row["role"] = json!("user");
            if let Some(client_id) = v["clientId"].as_str().filter(|id| !id.is_empty()) {
                row["id"] = json!(pending_input::row_id(client_id));
                row["nativeClientMessageId"] = json!(client_id);
            }
            row["fileAttachments"] = json!(attachments::native_file_views(&v["content"]));
            let (tab_attachments, terminal_attachments) =
                attachments::native_context_views(&v["content"]);
            row["attachments"] = json!(tab_attachments);
            row["terminalAttachments"] = json!(terminal_attachments);
            visible_user_text(&v["content"])
        }
        "exitedReviewMode" => {
            row["messagePhase"] = json!("final_answer");
            v["review"].as_str().unwrap_or_default().to_owned()
        }
        "agentMessage" | "plan" => {
            row["messagePhase"] = if kind == "plan" {
                json!("commentary")
            } else {
                v["phase"].clone()
            };
            let text = v["text"].as_str().unwrap_or_default();
            if kind == "agentMessage" {
                crate::browser::supervision::visible_review_output(text)
                    .unwrap_or_else(|| text.to_owned())
            } else {
                text.to_owned()
            }
        }
        "reasoning" => {
            row["kind"] = json!("reasoning");
            v["summary"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("\n\n")
        }
        kind => {
            row["kind"] = json!("activity");
            row["activityCategory"] = json!(match kind {
                "commandExecution" => "command",
                "fileChange" => "file_change",
                "contextCompaction" => "compaction",
                "webSearch" => match v["action"]["type"].as_str() {
                    Some("openPage" | "findInPage") => "web_read",
                    _ => "web_search",
                },
                _ => "tool",
            });
            row["activityStatus"] = json!(match v["status"].as_str() {
                Some("declined") if kind == "commandExecution" => "denied",
                Some("failed" | "declined") => "error",
                Some("interrupted") => "stopped",
                Some("completed") => "completed",
                // A terminal turn cannot leave a proposed action looking live.
                // Preserve explicit native outcomes above; never infer success
                // from a missing item/completed or a hydrated inProgress item.
                Some("inProgress") if turn.status == "failed" => "error",
                Some("inProgress") if !turn.active() => "stopped",
                Some("inProgress") => "running",
                _ if item.completed => "completed",
                _ if turn.status == "failed" => "error",
                _ if !turn.active() => "stopped",
                _ => "running",
            });
            match kind {
                "enteredReviewMode" => {
                    row["activityDetail"] = v["review"].clone();
                    "Started code review".into()
                }
                "commandExecution" => {
                    row["activityContext"] = v["cwd"].clone();
                    let interactions = if item.progress_messages.is_empty() {
                        String::new()
                    } else {
                        format!("\n{}", item.progress_messages.join("\n"))
                    };
                    row["activityDetail"] = json!(format!(
                        "{}\n{}{interactions}",
                        v["command"].as_str().unwrap_or_default(),
                        v["aggregatedOutput"].as_str().unwrap_or_default()
                    ));
                    match row["activityStatus"].as_str() {
                        Some("completed") => "Comando eseguito",
                        Some("error") => "Comando non riuscito",
                        Some("denied") => "Comando non autorizzato",
                        Some("stopped") => "Comando interrotto",
                        _ => "Esecuzione di un comando…",
                    }
                    .into()
                }
                "fileChange" => {
                    let changes: Vec<_> = v["changes"].as_array().into_iter().flatten().collect();
                    row["activityFileCount"] = json!(
                        changes
                            .iter()
                            .filter_map(|c| c["path"].as_str())
                            .collect::<std::collections::HashSet<_>>()
                            .len()
                    );
                    row["activityDetail"] = json!(
                        changes
                            .iter()
                            .filter_map(|c| c["path"].as_str())
                            .collect::<Vec<_>>()
                            .join("\n")
                    );
                    row["activityDiff"] = json!(
                        changes
                            .iter()
                            .filter_map(|c| c["diff"].as_str())
                            .collect::<Vec<_>>()
                            .join("\n")
                    );
                    if v["status"] == "completed" {
                        "Changed files"
                    } else {
                        "Proposed file changes"
                    }
                    .into()
                }
                "mcpToolCall" => {
                    row["activityToolName"] = v["tool"].clone();
                    let progress = if item.progress_messages.is_empty() {
                        String::new()
                    } else {
                        format!("\nProgress:\n{}", item.progress_messages.join("\n"))
                    };
                    row["activityDetail"] = json!(activity::safe_output_text(&format!(
                        "Arguments:\n{}\nResult:\n{}\nError:\n{}{progress}",
                        v["arguments"], v["result"], v["error"]
                    )));
                    format!(
                        "{} · {}",
                        v["server"].as_str().unwrap_or("MCP"),
                        v["tool"].as_str().unwrap_or("Tool")
                    )
                }
                "contextCompaction" => if item.completed {
                    "Conversazione ottimizzata"
                } else if turn.active() {
                    "Ottimizzazione della conversazione…"
                } else {
                    "Ottimizzazione della conversazione interrotta"
                }
                .into(),
                "imageView" => {
                    row["activityDetail"] = v["path"].clone();
                    "Viewed an image".into()
                }
                // Don't guess new variants or show hidden instructions/raw JSON.
                other => activity::details(v, &mut row)
                    .unwrap_or_else(|| format!("Codex activity · {other}")),
            }
        }
    };
    row["streaming"] = json!(turn.active() && !item.completed && kind != "userMessage");
    row["renderedHtml"] = json!(crate::markdown::render_safe_markdown(&text));
    row["text"] = json!(text);
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn skill_transport_marker_is_hidden_only_when_native_skill_is_attached() {
        let content = json!([
            {"type":"text","text":"Open Calculator"},
            {"type":"text","text":"$computer-use:computer-use"},
            {"type":"skill","name":"computer-use:computer-use","path":"C:/official/SKILL.md"},
            {"type":"text","text":"$supervisor-computer-use"},
            {"type":"skill","name":"supervisor-computer-use","path":"C:/supervisor/SKILL.md"}
        ]);
        assert_eq!(visible_user_text(&content), "Open Calculator");
        assert_eq!(
            visible_user_text(&json!([{"type":"text","text":"$computer-use:computer-use"}])),
            "$computer-use:computer-use"
        );
    }
    #[test]
    fn native_context_payload_is_hidden_from_prompt_text_and_restored_as_attachment() {
        let mut tab = TabContextSnapshot::from_value(
            7,
            8,
            "Page",
            "https://example.com",
            1,
            100,
            json!({"title":"Reference","url":"https://example.com/docs","text":"Frozen page"}),
        )
        .unwrap();
        tab.visual_thumbnail_data_url = Some("data:image/jpeg;base64,/9j/2Q==".into());
        let content =
            attachments::native_inputs("Inspect the reference", &[7], &[tab], &[], &[], &[], &[])
                .unwrap();
        assert_eq!(
            visible_user_text(&Value::Array(content.clone())),
            "Inspect the reference"
        );
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        mirror
            .hydrate(
                &json!({"id":"thread","turns":[{"id":"turn","status":"completed","items":[{
                    "id":"user","type":"userMessage","clientId":"client","content":content
                }]}]}),
                0,
            )
            .unwrap();
        let rows = messages(mirror.thread("thread").unwrap());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["text"], "Inspect the reference");
        assert_eq!(rows[0]["attachments"][0]["title"], "Reference");
        assert_eq!(rows[0]["attachments"][0]["stale"], false);
        assert_eq!(rows[0]["terminalAttachments"], json!([]));
        assert!(!rows[0].to_string().contains("Frozen page"));
    }
    #[test]
    fn observed_agent_snapshot_is_model_context_but_not_visible_prompt_copy() {
        let marker = format!(
            "{}{}",
            crate::browser::agent_graph::SUPERVISED_AGENT_CONTEXT_PREFIX,
            r#"{"events":[{"kind":"activity","text":"Ran tests"}]}"#
        );
        let content = json!([
            {"type":"text","text":"Check the worker"},
            {"type":"text","text":marker}
        ]);
        assert_eq!(visible_user_text(&content), "Check the worker");
    }
    #[test]
    fn supervisor_protocol_is_hidden_and_structured_output_is_readable() {
        let control = format!(
            "{}Return the structured decision.",
            crate::browser::supervision::REVIEW_CONTROL_PREFIX
        );
        let content = json!([
            {"type":"text","text":"Monitor this worker"},
            {"type":"text","text":control}
        ]);
        assert_eq!(visible_user_text(&content), "Monitor this worker");
        let output = json!({
            "schema":"supervisor.review.v1",
            "assessment":"The worker is on track.",
            "decision":"observe",
            "instruction":""
        })
        .to_string();
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        mirror
            .hydrate(
                &json!({"id":"thread","turns":[{"id":"turn","status":"completed","items":[
                    {"id":"user","type":"userMessage","content":content},
                    {"id":"final","type":"agentMessage","phase":"final_answer","text":output}
                ]}]}),
                0,
            )
            .unwrap();
        let rows = messages(mirror.thread("thread").unwrap());
        assert_eq!(rows[0]["text"], "Monitor this worker");
        assert_eq!(rows[1]["text"], "The worker is on track.");
        assert!(!rows[1]["text"].as_str().unwrap().contains("schema"));
    }
    #[test]
    fn hydrated_computer_use_turn_hides_transport_and_screenshot_bytes() {
        let image = format!("data:image/jpeg;base64,{}", "A".repeat(400));
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        mirror.hydrate(&json!({"id":"thread","turns":[{
            "id":"turn","status":"completed","items":[
                {"id":"user","type":"userMessage","content":[
                    {"type":"text","text":"Open Calculator"},
                    {"type":"text","text":"$computer-use:computer-use"},
                    {"type":"skill","name":"computer-use:computer-use","path":"C:/official/SKILL.md"}
                ]},
                {"id":"tool","type":"mcpToolCall","server":"node_repl","tool":"js",
                    "status":"completed","arguments":{"code":"inspect visible result"},
                    "result":{"content":[{"type":"image","url":image}]},"error":"retryable warning"}
            ]
        }]}), 0).unwrap();
        let rows = messages(mirror.thread("thread").unwrap());
        let prompt = rows.iter().find(|row| row["role"] == "user").unwrap();
        assert_eq!(prompt["text"], "Open Calculator");
        let tool = rows
            .iter()
            .find(|row| row["activityToolName"] == "js")
            .unwrap();
        let detail = tool["activityDetail"].as_str().unwrap();
        assert!(detail.contains("retryable warning"));
        assert!(detail.contains("[Image output"));
        assert!(!detail.contains(&"A".repeat(400)));
    }
    #[test]
    fn local_prompt_is_immediate_and_native_echo_reconciles_in_either_order() {
        for owner in ["chat:main", "graph:card"] {
            for ack_first in [true, false] {
                let mut pending = pending_input::PendingInputs::default();
                let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
                pending.begin(
                    owner,
                    "client-a",
                    "Repeat this prompt",
                    &SubmissionSnapshots::default(),
                    None,
                    None,
                );
                let mut rows = vec![];
                pending.project(owner, None, &mut rows);
                assert_eq!(rows.len(), 1); // No binding, session or native reply yet.
                assert_eq!(rows[0]["text"], "Repeat this prompt");
                assert_eq!(rows[0]["submissionStatus"], "sending");
                let id = rows[0]["id"].clone();
                let mut sibling = vec![];
                pending.project("chat:sibling", None, &mut sibling);
                assert!(sibling.is_empty());
                mirror.hydrate(&json!({"id":"thread","turns":[{"id":"turn","status":"inProgress","items":[]}]}), 0).unwrap();
                if ack_first {
                    pending.accept(owner, "client-a", "turn");
                    rows.clear();
                    pending.project(owner, mirror.thread("thread"), &mut rows);
                    assert_eq!(rows.len(), 1);
                    assert_eq!(rows[0]["id"], id);
                    assert_eq!(rows[0]["submissionStatus"], "accepted");
                }
                mirror
                    .notify(
                        "item/started",
                        &json!({"threadId":"thread","turnId":"turn","item":{
                            "id":"server-a","type":"userMessage","clientId":"client-a",
                            "content":[{"type":"text","text":"Repeat this prompt"}]
                        }}),
                    )
                    .unwrap();
                let thread = mirror.thread("thread").unwrap();
                rows = messages(thread);
                pending.project(owner, Some(thread), &mut rows);
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0]["id"], id);
                assert!(rows[0].get("submissionStatus").is_none());
                if !ack_first {
                    pending.accept(owner, "client-a", "turn");
                }
                pending.begin(
                    owner,
                    "client-b",
                    "Repeat this prompt",
                    &SubmissionSnapshots::default(),
                    None,
                    None,
                );
                rows = messages(thread);
                pending.project(owner, Some(thread), &mut rows);
                assert_eq!(rows.len(), 2); // Same text is a distinct submission.
                assert_ne!(rows[0]["id"], rows[1]["id"]);
                pending.remove(owner, "client-b"); // Rejection / cancellation before dispatch.
                rows = messages(thread);
                pending.project(owner, Some(thread), &mut rows);
                assert_eq!(rows.len(), 1);
                assert_eq!(thread.turns[0].items.len(), 1); // Display never rewrites native history.
            }
        }
    }

    #[test]
    fn pending_attachment_only_input_and_steering_keep_snapshots_and_order() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("note.txt");
        fs::write(&path, "Captured contents").unwrap();
        let file = PendingFileAttachment::load(&path).unwrap();
        let mut tab = TabContextSnapshot::from_value(
            7,
            8,
            "Page",
            "https://example.com",
            1,
            100,
            json!({"title":"Reference","url":"https://example.com/docs","text":"Frozen page"}),
        )
        .unwrap();
        tab.visual_thumbnail_data_url = Some("data:image/jpeg;base64,/9j/2Q==".into());
        let terminal = DraftTerminalContext {
            snapshot: TerminalContextSnapshot {
                session_id: 9,
                label: "Build shell".into(),
                kind: TerminalKind::Local,
                profile_id: None,
                remote_target: None,
                phase: TerminalPhase::Running,
                busy: false,
                shell: "PowerShell".into(),
                cwd: "C:\\project".into(),
                status: "Ready".into(),
                output: "Tests passed".into(),
                source_output_char_count: 12,
                output_revision: 1,
                last_exit_code: Some(0),
                captured_at_ms: 100,
                estimated_token_count: 51,
                redaction_count: 0,
                truncated: false,
            },
            follow_live: false,
            last_live_render_ms: 100,
            live_update_scheduled: false,
        };
        let snapshots = SubmissionSnapshots {
            tabs: vec![tab],
            terminals: vec![terminal],
            files: vec![file],
        };
        let mut pending = pending_input::PendingInputs::default();
        pending.begin(
            "graph:card",
            "file-input",
            "",
            &snapshots,
            Some("turn"),
            Some("prior-action".into()),
        );
        fs::write(&path, "Changed after sending").unwrap();
        let mut rows = vec![json!({"id":"prior-action"}), json!({"id":"later-action"})];
        pending.project("graph:card", None, &mut rows);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1]["text"], "");
        assert_eq!(rows[1]["attachments"][0]["title"], "Reference");
        assert_eq!(
            rows[1]["attachments"][0]["previewDataUrl"],
            "data:image/jpeg;base64,/9j/2Q=="
        );
        assert_eq!(
            rows[1]["terminalAttachments"][0]["outputPreview"],
            "Tests passed"
        );
        assert_eq!(rows[1]["fileAttachments"][0]["name"], "note.txt");
        assert_eq!(rows[1]["fileAttachments"][0]["byteCount"], 17);
        assert!(!rows[1].to_string().contains("Captured contents"));
        assert!(
            !rows[1]
                .to_string()
                .contains(&directory.path().display().to_string())
        );
        let native_content = attachments::native_inputs(
            "",
            &[7],
            &snapshots.tabs,
            &[9],
            &snapshots.terminals,
            &[snapshots.files[0].id().to_owned()],
            &snapshots.files,
        )
        .unwrap();
        assert!(
            !Value::Array(native_content.clone())
                .to_string()
                .contains("previewDataUrl")
        );
        let mut saved = central_agent_codex_runtime::conversations::Saved::default();
        saved.bindings.insert(
            "graph:card".into(),
            central_agent_codex_runtime::conversations::Binding {
                thread_id: "thread".into(),
                session_id: None,
                archived: false,
                deleted: false,
                never_submitted: false,
            },
        );
        let mut conversations =
            central_agent_codex_runtime::conversations::Conversations::restore(saved).unwrap();
        conversations
            .mirror
            .hydrate(
                &json!({"id":"thread","turns":[{"id":"turn","status":"inProgress","items":[{
                    "id":"native-user","type":"userMessage","clientId":"file-input","content":native_content
                }]}]}),
                0,
            )
            .unwrap();
        pending.reconcile(&conversations);
        let thread = conversations.mirror.thread("thread").unwrap();
        let mut mirrored_rows = messages(thread);
        pending.project("graph:card", Some(thread), &mut mirrored_rows);
        assert_eq!(mirrored_rows.len(), 1);
        assert_eq!(
            mirrored_rows[0]["attachments"][0]["previewDataUrl"],
            "data:image/jpeg;base64,/9j/2Q=="
        );
        pending.accept("graph:other", "file-input", "other-turn");
        rows.clear();
        pending.project("graph:card", None, &mut rows);
        assert_eq!(rows[0]["submissionStatus"], "sending");
        pending.clear(); // Disconnect cannot leave an apparent sending operation.
        rows.clear();
        pending.project("graph:card", None, &mut rows);
        assert!(rows.is_empty());
    }

    #[test]
    fn native_reference_turn_projects_one_answered_question_and_preserves_all_actions() {
        let rows = work_reference_messages();
        assert_eq!(rows.iter().filter(|row| row["role"] == "user").count(), 2);
        assert_eq!(
            rows.iter()
                .filter(|row| row["messagePhase"] == "final_answer")
                .count(),
            1
        );
        assert_eq!(
            rows.iter()
                .filter(|row| row["messagePhase"] == "commentary")
                .count(),
            6
        );
        assert_eq!(
            rows.iter()
                .filter(|row| row["activityCategory"] == "command")
                .count(),
            21
        );
        assert_eq!(
            rows.iter()
                .filter(|row| row["activityCategory"] == "web_search")
                .count(),
            1
        );
        let reply = rows
            .iter()
            .find(|row| row.get("nativeQuestionReply").is_some())
            .unwrap();
        assert_eq!(reply["text"], "Conserva i file");
        assert_eq!(
            reply["nativeQuestionReply"]["entries"][0]["question"],
            "Conservare i file selezionati?"
        );
        assert!(
            rows.iter()
                .any(|row| row["text"] == "Conservare i file selezionati?")
        );
        let projected = serde_json::to_string(&rows).unwrap();
        assert!(!projected.contains("send_user_message_question_reply"));
        assert!(!projected.contains("PRIVATE_FIXTURE_REASONING"));
    }
    #[test]
    fn question_reply_format_never_rewrites_unrelated_or_malformed_native_text() {
        let source: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/ui/tests/fixtures/native-work-outline.json"
        )))
        .unwrap();
        for replacement in ["foreign_call", "wrong_format"] {
            let mut fixture = source.clone();
            for item in fixture["turns"][0]["items"].as_array_mut().unwrap() {
                if item["type"] == "userMessage"
                    && let Some(text) = item["content"][0]["text"]
                        .as_str()
                        .filter(|text| text.contains("question_reply"))
                {
                    item["content"][0]["text"] = json!(if replacement == "foreign_call" {
                        text.replace("call_fixture", "foreign_call")
                    } else {
                        format!("Unrelated text: {text}")
                    });
                }
            }
            let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
            mirror.hydrate(&fixture, 0).unwrap();
            let rows = messages(mirror.thread("reference-thread").unwrap());
            assert!(
                !rows
                    .iter()
                    .any(|row| row.get("nativeQuestionReply").is_some())
            );
            assert_eq!(
                rows.iter()
                    .filter(|row| row["messagePhase"] == "final_answer")
                    .count(),
                2
            );
        }
    }
    #[test]
    fn empty_reasoning_appears_only_when_public_summary_arrives_for_main_and_graph() {
        for thread in ["main", "graph"] {
            let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
            mirror.notify("item/started", &json!({"threadId":thread,"turnId":"turn","item":{"id":"r","type":"reasoning","summary":[],"content":[]}})).unwrap();
            assert!(messages(mirror.thread(thread).unwrap()).is_empty());
            mirror.notify("item/reasoning/summaryTextDelta", &json!({"threadId":thread,"turnId":"turn","itemId":"r","summaryIndex":0,"delta":"Checking "})).unwrap();
            let first = messages(mirror.thread(thread).unwrap());
            assert_eq!(first[0]["text"], "Checking ");
            assert_eq!(first[0]["streaming"], true);
            mirror.notify("item/reasoning/summaryTextDelta", &json!({"threadId":thread,"turnId":"turn","itemId":"r","summaryIndex":0,"delta":"files"})).unwrap();
            let next = messages(mirror.thread(thread).unwrap());
            assert_eq!(first[0]["id"], next[0]["id"]);
            assert_eq!(next[0]["text"], "Checking files");
            mirror.notify("item/completed", &json!({"threadId":thread,"turnId":"turn","item":{"id":"r","type":"reasoning","summary":["Checked files"],"content":["PRIVATE"]}})).unwrap();
            let done = messages(mirror.thread(thread).unwrap());
            assert_eq!(done[0]["id"], first[0]["id"]);
            assert_eq!(done[0]["text"], "Checked files");
            assert!(!serde_json::to_string(&done).unwrap().contains("PRIVATE"));
        }
    }
    #[test]
    fn native_mcp_progress_is_plain_detail_on_the_same_main_graph_activity() {
        let samples: Vec<Value> = serde_json::from_str(include_str!(
            "../../../crates/central-agent-codex-runtime/tests/native-mcp-progress.json"
        ))
        .unwrap();
        for thread in ["main", "graph"] {
            let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
            mirror.notify("item/started",&json!({"threadId":thread,"turnId":"turn","item":{"id":"tool","type":"mcpToolCall","status":"inProgress","server":"fixture","tool":"test","arguments":{}}})).unwrap();
            let id = messages(mirror.thread(thread).unwrap())[0]["id"].clone();
            for sample in &samples {
                let mut event = sample.clone();
                event["threadId"] = json!(thread);
                mirror.notify("item/mcpToolCall/progress", &event).unwrap();
            }
            let rows = messages(mirror.thread(thread).unwrap());
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["id"], id);
            assert_eq!(rows[0]["activityStatus"], "running");
            assert!(
                rows[0]["activityDetail"]
                    .as_str()
                    .unwrap()
                    .ends_with("Progress:\nInspecting metadata\n<script>plain tool text</script>")
            );
            assert!(
                !rows[0]["renderedHtml"]
                    .as_str()
                    .unwrap()
                    .contains("<script>")
            );
        }
    }
    #[test]
    fn terminal_interaction_updates_the_command_row_without_exposing_input() {
        for thread in ["main", "graph"] {
            let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
            mirror
                .notify(
                    "item/started",
                    &json!({"threadId":thread,"turnId":"turn","item":{"id":"command","type":"commandExecution","status":"inProgress","command":"fixture","cwd":"C:\\fixture"}}),
                )
                .unwrap();
            mirror
                .notify(
                    "item/commandExecution/terminalInteraction",
                    &json!({"threadId":thread,"turnId":"turn","itemId":"command","processId":"PRIVATE_PROCESS","stdin":"PRIVATE_STDIN"}),
                )
                .unwrap();
            let rows = messages(mirror.thread(thread).unwrap());
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["activityStatus"], "running");
            assert!(
                rows[0]["activityDetail"]
                    .as_str()
                    .unwrap()
                    .contains("Sent 13 bytes of input")
            );
            let public = serde_json::to_string(&rows).unwrap();
            assert!(!public.contains("PRIVATE_STDIN"));
            assert!(!public.contains("PRIVATE_PROCESS"));
        }
    }
    #[test]
    fn native_patch_updates_refresh_the_shared_main_graph_diff_without_a_new_row() {
        let samples: Vec<Value> = serde_json::from_str(include_str!(
            "../../../crates/central-agent-codex-runtime/tests/native-patch-updates.json"
        ))
        .unwrap();
        for thread in ["main", "graph"] {
            let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
            let mut previous_id = None;
            for sample in &samples {
                let mut event = sample.clone();
                event["threadId"] = json!(thread);
                mirror
                    .notify("item/fileChange/patchUpdated", &event)
                    .unwrap();
                let rows = messages(mirror.thread(thread).unwrap());
                assert_eq!(rows.len(), 1);
                let row = &rows[0];
                if let Some(id) = &previous_id {
                    assert_eq!(&row["id"], id);
                }
                previous_id = Some(row["id"].clone());
                assert_eq!(row["text"], "Proposed file changes");
                assert_eq!(row["activityStatus"], "running");
                assert_eq!(
                    row["activityDiff"],
                    sample["changes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|c| c["diff"].as_str().unwrap())
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                assert_eq!(
                    row["activityDetail"],
                    sample["changes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|c| c["path"].as_str().unwrap())
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
            mirror.notify("item/completed", &json!({"threadId":thread,"turnId":"turn","item":{"id":"patch","type":"fileChange","status":"declined","changes":samples[0]["changes"]}})).unwrap();
            let mut late = samples[1].clone();
            late["threadId"] = json!(thread);
            mirror
                .notify("item/fileChange/patchUpdated", &late)
                .unwrap();
            let rows = messages(mirror.thread(thread).unwrap());
            assert_eq!(rows.len(), 1);
            assert_eq!(Some(&rows[0]["id"]), previous_id.as_ref());
            assert_eq!(rows[0]["activityStatus"], "error");
            assert_eq!(rows[0]["text"], "Proposed file changes");
            assert_eq!(rows[0]["activityDiff"], samples[0]["changes"][0]["diff"]);
        }
    }
    #[test]
    fn inline_review_projects_one_native_prompt_across_worker_hydration() {
        let prompt = |id| json!({"id":id,"type":"userMessage","content":[{"type":"text","text":"Review the snippet"}]});
        let entry = json!({"id":"entry","type":"enteredReviewMode","review":"Review the snippet"});
        for worker_hydrated in [false, true] {
            let mut turns = vec![
                json!({"id":"review","status":"completed","items":[prompt("review"),entry,prompt("worker-input"),{"id":"exit","type":"exitedReviewMode","review":"Found an issue"}]}),
            ];
            if worker_hydrated {
                turns.insert(
                    0,
                    json!({"id":"worker","status":"interrupted","items":[prompt("worker-input")]}),
                );
            }
            let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
            mirror.hydrate(&json!({"id":"t","turns":turns}), 0).unwrap();
            let before = serde_json::to_value(mirror.thread("t").unwrap()).unwrap();
            let rows = messages(mirror.thread("t").unwrap());
            let prompts: Vec<_> = rows.iter().filter(|row| row["role"] == "user").collect();
            assert_eq!(prompts.len(), 1);
            assert_eq!(prompts[0]["text"], "Review the snippet");
            assert_eq!(
                prompts[0]["id"],
                if worker_hydrated {
                    "native:t:worker:worker-input"
                } else {
                    "native:t:review:worker-input"
                }
            );
            assert_eq!(
                before,
                serde_json::to_value(mirror.thread("t").unwrap()).unwrap()
            );
        }
    }

    #[test]
    fn review_prompt_projection_keeps_unrelated_repetition_and_initial_prompt() {
        let prompt = |id| json!({"id":id,"type":"userMessage","content":[{"type":"text","text":"Same instruction"}]});
        for has_entry in [false, true] {
            let mut items = vec![prompt("review")];
            if has_entry {
                items.push(
                    json!({"id":"entry","type":"enteredReviewMode","review":"Same instruction"}),
                );
            }
            let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
            mirror.hydrate(&json!({"id":"t","turns":[{"id":"review","status":"inProgress","items":items}]}),0).unwrap();
            assert_eq!(
                messages(mirror.thread("t").unwrap())
                    .iter()
                    .filter(|row| row["role"] == "user")
                    .count(),
                1
            );
        }
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        mirror.hydrate(&json!({"id":"t","turns":[
            {"id":"first","status":"completed","items":[prompt("first-input")]},
            {"id":"review","status":"completed","items":[prompt("review"),{"id":"entry","type":"enteredReviewMode","review":"Same instruction"}]},
            {"id":"next","status":"completed","items":[prompt("next-input")]}
        ]}),0).unwrap();
        assert_eq!(
            messages(mirror.thread("t").unwrap())
                .iter()
                .filter(|row| row["role"] == "user")
                .count(),
            3
        );
    }

    #[test]
    fn native_review_echo_has_one_canonical_row_without_changing_native_history() {
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        let entry = json!({"id":"entry","type":"enteredReviewMode","review":"fixture"});
        let exit = json!({"id":"exit","type":"exitedReviewMode","review":"**Result**"});
        let echo = json!({"id":"echo","type":"agentMessage","phase":null,"text":"**Result**\n"});
        mirror
            .notify(
                "turn/started",
                &json!({"threadId":"t","turn":{"id":"r","status":"inProgress","items":[]}}),
            )
            .unwrap();
        for item in [&entry, &exit, &echo] {
            mirror
                .notify(
                    "item/completed",
                    &json!({"threadId":"t","turnId":"r","item":item}),
                )
                .unwrap();
        }
        let rows = messages(mirror.thread("t").unwrap());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1]["id"], "native:t:r:exit");
        assert_eq!(rows[1]["messagePhase"], "final_answer");
        assert_eq!(rows[1]["text"], "**Result**");
        assert_eq!(mirror.thread("t").unwrap().turns[0].items[2].value, echo);
        for items in [
            vec![entry.clone(), exit.clone(), echo.clone()],
            vec![echo, entry, exit],
        ] {
            let history = json!({"id":"t","turns":[{"id":"r","status":"completed","items":items}]});
            mirror.hydrate(&history, mirror.revision()).unwrap();
            let loaded = messages(mirror.thread("t").unwrap());
            assert_eq!(loaded.len(), 2);
            assert_eq!(loaded[1]["id"], rows[1]["id"]);
            assert_eq!(loaded[1]["renderedHtml"], rows[1]["renderedHtml"]);
            assert_eq!(mirror.thread("t").unwrap().turns[0].items.len(), 3);
        }
    }

    #[test]
    fn native_review_does_not_deduplicate_other_messages_or_turns() {
        for (phase, text, adjacent, exit_text, expected) in [
            (Value::Null, "Result", true, "Result", 1usize),
            (json!("final_answer"), "Result", true, "Result", 2),
            (json!("commentary"), "Result", true, "Result", 2),
            (Value::Null, "Different result", true, "Result", 2),
            (Value::Null, "Result", false, "Result", 2),
            (Value::Null, "", true, "", 2),
        ] {
            let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
            let mut items = vec![json!({"id":"exit","type":"exitedReviewMode","review":exit_text})];
            if !adjacent {
                items.push(json!({"id":"other","type":"plan","text":"Keep this"}));
            }
            items.push(json!({"id":"echo","type":"agentMessage","phase":phase,"text":text}));
            mirror.hydrate(&json!({"id":"t","turns":[
                {"id":"r","status":"completed","items":items},
                {"id":"next","status":"completed","items":[{"id":"same","type":"agentMessage","text":"Result"}]}
            ]}),0).unwrap();
            let rows = messages(mirror.thread("t").unwrap());
            assert_eq!(rows.len(), expected + 1);
            assert_eq!(rows.last().unwrap()["id"], "native:t:next:same");
            assert_eq!(
                mirror.thread("t").unwrap().turns[0].items.len(),
                items.len()
            );
        }
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        mirror
            .notify(
                "turn/started",
                &json!({"threadId":"t","turn":{"id":"r","status":"inProgress","items":[]}}),
            )
            .unwrap();
        mirror.notify("item/completed", &json!({"threadId":"t","turnId":"r","item":{"id":"exit","type":"exitedReviewMode","review":"Result"}})).unwrap();
        mirror.notify("item/started", &json!({"threadId":"t","turnId":"r","item":{"id":"echo","type":"agentMessage","text":"Result"}})).unwrap();
        assert_eq!(
            messages(mirror.thread("t").unwrap()).len(),
            2,
            "Unfinished message must not be discarded"
        );
    }
    #[test]
    fn native_ended_turn_preserves_completed_activities_without_explicit_status() {
        for terminal in ["completed", "interrupted", "failed"] {
            let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
            mirror
                .notify(
                    "turn/started",
                    &json!({"threadId":"t","turn":{"id":"r","status":"inProgress","items":[]}}),
                )
                .unwrap();
            for (id, complete) in [("done", true), ("pending", false)] {
                let params = json!({"threadId":"t","turnId":"r","item":{"id":id,"type":"webSearch","query":"example"}});
                mirror.notify("item/started", &params).unwrap();
                if complete {
                    mirror.notify("item/completed", &params).unwrap();
                }
            }
            mirror
                .notify(
                    "turn/completed",
                    &json!({"threadId":"t","turn":{"id":"r","status":terminal,"items":[]}}),
                )
                .unwrap();
            let rows = messages(mirror.thread("t").unwrap());
            assert_eq!(rows[0]["activityStatus"], "completed");
            assert_eq!(
                rows[1]["activityStatus"],
                if terminal == "failed" {
                    "error"
                } else {
                    "stopped"
                }
            );
            assert!(rows.iter().all(|row| row["streaming"] == false));
        }
    }
    #[test]
    fn native_terminal_turn_never_leaves_file_changes_running_or_claims_application() {
        for (turn_status, item_status, expected) in [
            ("inProgress", "inProgress", "running"),
            ("interrupted", "inProgress", "stopped"),
            ("interrupted", "failed", "error"),
            ("interrupted", "declined", "error"),
            ("interrupted", "completed", "completed"),
            ("failed", "inProgress", "error"),
            ("completed", "inProgress", "stopped"),
        ] {
            for hydrated in [false, true] {
                let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
                let item = json!({"id":"patch","type":"fileChange","status":item_status,
                    "changes":[{"path":"example.txt","kind":{"type":"update","move_path":null},"diff":"-before\n+after"}]});
                if hydrated {
                    mirror.hydrate(&json!({"id":"t","turns":[{"id":"r","status":turn_status,"items":[item]}]}),0).unwrap();
                } else {
                    mirror.notify("turn/started", &json!({"threadId":"t","turn":{"id":"r","status":"inProgress","items":[]}})).unwrap();
                    mirror
                        .notify(
                            "item/started",
                            &json!({"threadId":"t","turnId":"r","item":item}),
                        )
                        .unwrap();
                    if turn_status != "inProgress" {
                        mirror.notify("turn/completed", &json!({"threadId":"t","turn":{"id":"r","status":turn_status,"items":[]}})).unwrap();
                    }
                }
                let thread = mirror.thread("t").unwrap();
                let rows = messages(thread);
                assert_eq!(
                    rows[0]["activityStatus"], expected,
                    "{turn_status}/{item_status}, hydrated={hydrated}"
                );
                assert_eq!(
                    rows[0]["text"],
                    if item_status == "completed" {
                        "Changed files"
                    } else {
                        "Proposed file changes"
                    }
                );
                assert_eq!(rows[0]["activityDiff"], "-before\n+after");
                assert_eq!(thread.turns[0].items[0].value["status"], item_status);
                if turn_status != "inProgress" {
                    assert_eq!(rows[0]["streaming"], false);
                }
            }
        }
    }
    fn activity_items() -> Vec<Value> {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/crates/central-agent-codex-runtime/tests/native-activity-items.json"
        )))
        .unwrap()
    }
    fn activity_rows() -> Vec<Value> {
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        mirror.hydrate(&json!({"id":"events-thread","turns":[{"id":"events-turn","status":"completed","items":activity_items()}]}),0).unwrap();
        messages(mirror.thread("events-thread").unwrap())
    }
    #[test]
    fn native_compaction_label_tracks_item_completion_without_claiming_an_early_success() {
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        mirror
            .notify(
                "turn/started",
                &json!({"threadId":"t","turn":{"id":"r","status":"inProgress","items":[]}}),
            )
            .unwrap();
        let params =
            json!({"threadId":"t","turnId":"r","item":{"id":"compact","type":"contextCompaction"}});
        mirror.notify("item/started", &params).unwrap();
        let before = messages(mirror.thread("t").unwrap());
        assert_eq!(before[0]["text"], "Ottimizzazione della conversazione…");
        assert_eq!(before[0]["activityCategory"], "compaction");
        mirror.notify("item/completed", &params).unwrap();
        let after = messages(mirror.thread("t").unwrap());
        assert_eq!(after[0]["text"], "Conversazione ottimizzata");
        assert_eq!(before[0]["id"], after[0]["id"]);
        assert!(after[0].get("checkpoint").is_none());
    }
    #[test]
    fn native_command_labels_keep_reported_outcomes_distinct() {
        for (native, status, text) in [
            ("inProgress", "running", "Esecuzione di un comando…"),
            ("completed", "completed", "Comando eseguito"),
            ("declined", "denied", "Comando non autorizzato"),
            ("failed", "error", "Comando non riuscito"),
            ("interrupted", "stopped", "Comando interrotto"),
        ] {
            let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
            mirror.hydrate(&json!({"id":"t","turns":[{"id":"r","status":"inProgress","items":[{"id":"command","type":"commandExecution","status":native,"command":"fixture","aggregatedOutput":""}]}]}),0).unwrap();
            let rows = messages(mirror.thread("t").unwrap());
            assert_eq!(rows[0]["activityStatus"], status);
            assert_eq!(rows[0]["text"], text);
        }
    }
    #[test]
    fn native_web_actions_show_exact_queries_urls_and_patterns() {
        let rows = activity_rows();
        assert_eq!(rows[0]["text"], "Searched the web");
        let detail = rows[0]["activityDetail"].as_str().unwrap();
        assert_eq!(detail.matches("Query: Rust native clients").count(), 1);
        assert!(detail.contains("Query: Svelte event streams"));
        assert!(detail.contains("1 structured results reported"));
        assert_eq!(rows[1]["text"], "Opened a web page");
        assert!(
            rows[1]["activityDetail"]
                .as_str()
                .unwrap()
                .contains("https://example.com/docs")
        );
        assert_eq!(rows[2]["text"], "Searched within a web page");
        assert!(
            rows[2]["activityDetail"]
                .as_str()
                .unwrap()
                .contains("Find: item/completed")
        );
    }
    #[test]
    fn native_collaboration_keeps_call_and_child_statuses_distinct() {
        let rows = activity_rows();
        assert_eq!(rows[3]["text"], "Spawn agent");
        assert_eq!(rows[3]["activityStatus"], "completed");
        let detail = rows[3]["activityDetail"].as_str().unwrap();
        for expected in [
            "From thread: events-thread",
            "To thread: child-1",
            "Requested model: fixture-model",
            "Requested effort: high",
            "Message: Inspect the parser",
            "Agent child-1: running",
            "Response: Checking callers",
        ] {
            assert!(detail.contains(expected), "{expected}");
        }
        assert_eq!(rows[4]["activityStatus"], "stopped");
        let detail = rows[4]["activityDetail"].as_str().unwrap();
        assert!(detail.contains("Agent child-1: errored"));
        assert!(detail.contains("Agent child-2: completed"));
        assert_eq!(rows[5]["text"], "Subagent completed");
        assert!(
            rows[5]["activityDetail"]
                .as_str()
                .unwrap()
                .contains("/root/parser")
        );
    }
    #[test]
    fn native_extra_outputs_exclude_opaque_payloads_and_keep_explicit_fallbacks() {
        let rows = activity_rows();
        let serialized = serde_json::to_string(&rows).unwrap();
        assert!(!serialized.contains("PRIVATE_"));
        assert!(
            rows[6]["activityDetail"]
                .as_str()
                .unwrap()
                .contains("Public tool output")
        );
        assert!(
            rows[6]["activityDetail"]
                .as_str()
                .unwrap()
                .contains("Encrypted output — not displayed")
        );
        // Detail is plain text (shared main/graph renderer), never trusted HTML.
        assert!(
            !rows[6]["renderedHtml"]
                .as_str()
                .unwrap()
                .contains("<script>")
        );
        assert_eq!(rows[7]["activityDetail"], "Requested wait: 60000 ms");
        assert_eq!(rows[8]["activityStatus"], "error");
        assert!(
            rows[8]["activityDetail"]
                .as_str()
                .unwrap()
                .contains("usage limit exceeded")
        );
        assert_eq!(rows[9]["activityStatus"], "error");
        assert!(
            rows[9]["activityDetail"]
                .as_str()
                .unwrap()
                .contains("Historical tool result")
        );
        assert!(
            rows.iter()
                .all(|row| row.get("checkpoint").is_none() && row.get("artifacts").is_none())
        );
    }
    #[test]
    fn native_activity_completion_replaces_started_details_without_new_identity() {
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        let mut item = activity_items()[3].clone();
        item["status"] = json!("inProgress");
        mirror.notify("item/started", &json!({"threadId":"events-thread","turnId":"events-turn","startedAtMs":1000,"item":item})).unwrap();
        let before = messages(mirror.thread("events-thread").unwrap());
        assert_eq!(before[0]["streaming"], true);
        item["status"] = json!("completed");
        item["prompt"] = json!("Authoritative final message");
        mirror.notify("item/completed", &json!({"threadId":"events-thread","turnId":"events-turn","completedAtMs":2000,"item":item})).unwrap();
        let after = messages(mirror.thread("events-thread").unwrap());
        assert_eq!(after.len(), 1);
        assert_eq!(after[0]["id"], before[0]["id"]);
        assert_eq!(after[0]["streaming"], false);
        let detail = after[0]["activityDetail"].as_str().unwrap();
        assert!(detail.contains("Authoritative final message"));
        assert!(!detail.contains("Inspect the parser"));
    }
    #[test]
    fn native_hooks_and_unknown_items_never_reveal_instructions() {
        let mut row = json!({});
        assert_eq!(
            activity::details(
                &json!({"type":"hookPrompt","fragments":[{"text":"PRIVATE_HOOK"}]}),
                &mut row
            )
            .as_deref(),
            Some("Native hook")
        );
        assert!(!row.to_string().contains("PRIVATE_HOOK"));
        assert!(
            activity::details(
                &json!({"type":"futureVariant","instructions":"PRIVATE_UNKNOWN"}),
                &mut row
            )
            .is_none()
        );
        assert!(!row.to_string().contains("PRIVATE_UNKNOWN"));
    }
    #[test]
    fn native_review_output_is_final_and_live_plan_uses_reported_step_states() {
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        let samples: Vec<Value> = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/crates/central-agent-codex-runtime/tests/native-review.json"
        )))
        .unwrap();
        mirror
            .notify(
                "turn/started",
                &json!({"threadId":"native-review","turn":samples[0]["params"]["turn"]}),
            )
            .unwrap();
        for sample in &samples[1..4] {
            mirror
                .notify(sample["method"].as_str().unwrap(), &sample["params"])
                .unwrap();
        }
        let rows = messages(mirror.thread("native-review").unwrap());
        assert_eq!(rows[1]["text"], "Started code review");
        assert_eq!(rows[2]["messagePhase"], "final_answer");
        assert_eq!(rows[2]["kind"], "message");
        assert!(
            rows[2]["renderedHtml"]
                .as_str()
                .unwrap()
                .contains("<strong>Review complete</strong>")
        );
        assert!(
            !rows[2]["renderedHtml"]
                .as_str()
                .unwrap()
                .contains("<script>")
        );
        assert!(
            rows[0]["activityDetail"]
                .as_str()
                .unwrap()
                .contains("[In progress] Check callers")
        );
        let id = rows[0]["id"].clone();
        let mut updated = samples[2]["params"].clone();
        updated["plan"][1]["status"] = json!("completed");
        mirror.notify("turn/plan/updated", &updated).unwrap();
        let rows = messages(mirror.thread("native-review").unwrap());
        assert_eq!(rows[0]["id"], id);
        assert!(
            rows[0]["activityDetail"]
                .as_str()
                .unwrap()
                .contains("[Completed] Check callers")
        );
    }
    #[test]
    fn plan_completion_requires_completed_steps_and_precedes_final_output() {
        for status in ["inProgress", "completed", "interrupted", "failed"] {
            for step_status in ["pending", "inProgress", "completed", "unknown"] {
                let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
                mirror.hydrate(&json!({"id":"t","turns":[{"id":"r","status":status,"items":[
                    {"id":"u","type":"userMessage","content":[{"type":"text","text":"Inspect"}]},
                    {"id":"reason","type":"reasoning","summary":[],"content":["PRIVATE"]},
                    {"id":"final","type":"agentMessage","phase":"final_answer","text":"Result"}
                ]}]}), 0).unwrap();
                mirror.notify("turn/plan/updated", &json!({"threadId":"t","turnId":"r","plan":[{"step":"Verify","status":step_status}]})).unwrap();
                let rows = messages(mirror.thread("t").unwrap());
                assert_eq!(rows.len(), 3);
                assert_eq!(rows[0]["role"], "user");
                assert_eq!(rows[1]["id"], "native:t:r:native-plan");
                assert_eq!(rows[2]["messagePhase"], "final_answer");
                let expected = if step_status == "completed" {
                    "completed"
                } else if status == "inProgress" {
                    "running"
                } else if status == "failed" {
                    "error"
                } else {
                    "stopped"
                };
                assert_eq!(
                    rows[1]["activityStatus"], expected,
                    "{status}/{step_status}"
                );
                assert_eq!(mirror.thread("t").unwrap().turns[0].items.len(), 3);
                assert!(!serde_json::to_string(&rows).unwrap().contains("PRIVATE"));
                mirror
                    .notify(
                        "turn/plan/updated",
                        &json!({"threadId":"t","turnId":"r","plan":[]}),
                    )
                    .unwrap();
                assert_ne!(
                    messages(mirror.thread("t").unwrap())[1]["activityStatus"],
                    "completed"
                );
            }
        }
    }
    #[test]
    fn native_items_keep_identity_summaries_and_sanitized_markdown() {
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        mirror.hydrate(&json!({"id":"t","turns":[{"id":"r","status":"completed","startedAt":10,"completedAt":12,"durationMs":2000,"items":[
            {"id":"u","type":"userMessage","content":[{"type":"text","text":"Inspect"}]},
            {"id":"reason","type":"reasoning","summary":["Checking files"],"content":["PRIVATE CONTENT"]},
            {"id":"a","type":"agentMessage","text":"**Done** <script>bad()</script>","phase":"final_answer"}
        ]}]}),0).unwrap();
        let rows = messages(mirror.thread("t").unwrap());
        assert_eq!(rows[0]["role"], "user");
        assert_eq!(rows[0]["nativeTurnTiming"]["durationMs"], 2000);
        assert_eq!(rows[1]["text"], "Checking files");
        assert_eq!(rows[1]["kind"], "reasoning");
        assert!(
            rows[2]["renderedHtml"]
                .as_str()
                .unwrap()
                .contains("<strong>Done</strong>")
        );
        assert!(
            !rows[2]["renderedHtml"]
                .as_str()
                .unwrap()
                .contains("<script>")
        );
        assert!(
            !serde_json::to_string(&rows)
                .unwrap()
                .contains("PRIVATE CONTENT")
        );
        assert!(rows.iter().all(|r| r.get("checkpoint").is_none()));
    }
}
