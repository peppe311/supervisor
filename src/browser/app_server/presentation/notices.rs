use central_agent_codex_runtime::{
    mirror::Thread,
    model_notices::{ModelNotice, Notice},
};
use serde_json::{Value, json};

pub(super) fn message(thread: &Thread, notice: &Notice) -> Option<Value> {
    let turn = thread.turns.iter().find(|turn| turn.id == notice.turn_id);
    let mut text = match &notice.value {
        ModelNotice::Rerouted(value) => format!(
            "The service changed the model for this request: {} → {}.\nService-reported reason: {}.\nYour selected profile has not been changed.",
            value.from_model, value.to_model, value.reason
        ),
        ModelNotice::Verification(value) => {
            if value.verifications.is_empty() {
                return None;
            }
            format!(
                "Codex reported required account verification: {}.\nComplete the requested verification with your Codex account before retrying. No retry is sent automatically.",
                value.verifications.join(", ")
            )
        }
        ModelNotice::Buffering(value) => {
            if !value.show_buffering_ui {
                return None;
            }
            let live = notice.current && turn.is_some_and(|turn| turn.active());
            let mut text = format!(
                "{}\nModel: {}",
                if live {
                    "The service reports temporary response buffering."
                } else {
                    "The service reported temporary response buffering during this request."
                },
                value.model
            );
            if !value.use_cases.is_empty() {
                text.push_str(&format!("\nUse cases: {}", value.use_cases.join(", ")));
            }
            if !value.reasons.is_empty() {
                text.push_str(&format!("\nReasons: {}", value.reasons.join(", ")));
            }
            if let Some(model) = &value.faster_model {
                text.push_str(&format!("\nService-suggested alternative: {model}. No model change has been made by Supervisor."));
            }
            text
        }
        ModelNotice::TurnFailure(value) => format!(
            "Codex reported an error during this request: {}\n{}",
            value.message,
            if value.will_retry {
                "The native runtime reports that it will retry. Supervisor has not sent another request."
            } else {
                "The native runtime will not retry. Wait for the authoritative turn completion before deciding whether to send new input."
            }
        ),
        ModelNotice::Warning(value) => format!(
            "Codex reported a runtime warning: {}\nNo permission, retry or profile change was made by Supervisor.",
            value.message
        ),
        ModelNotice::GuardianWarning(value) => format!(
            "Codex reported an approval-safety warning: {}\nReview any native approval card before continuing; nothing was approved automatically by Supervisor.",
            value.message
        ),
        ModelNotice::StrictReview(value) => format!(
            "Codex requires strict review for an approval in this request.\nNative review started at {} ms (Unix time). No approval was granted by Supervisor.",
            value.started_at_ms
        ),
        ModelNotice::AutoReview(value) => {
            let action = match value.action_kind.as_str() {
                "command" | "execve" => "command",
                "writeStdin" => "process input",
                "applyPatch" => "file change",
                "networkAccess" => "network access",
                "mcpToolCall" => "MCP tool call",
                "requestPermissions" => "permission request",
                _ => "approval request",
            };
            let status = match value.status.as_str() {
                "inProgress" => "in progress",
                "approved" => "approved by the native reviewer",
                "denied" => "denied by the native reviewer",
                "timedOut" => "timed out",
                "aborted" => "aborted",
                _ => "not reported",
            };
            let risk = value
                .risk_level
                .as_ref()
                .map(|risk| format!("\nReported risk: {risk}."))
                .unwrap_or_default();
            format!(
                "Native automatic review for {action}: {status}.{risk}\nThis is review state, not a separate Supervisor permission grant."
            )
        }
        ModelNotice::Moderation => "Codex supplied opaque moderation metadata for this request. Supervisor did not expose or store that payload. Follow only the public turn error, verification or final status shown here; no action was taken automatically.".into(),
    };
    if !notice.current {
        text.insert_str(
            0,
            "Previously reported — connection state is no longer current.\n",
        );
    }
    // Plain text is escaped before safe Markdown; model names and reasons cannot
    // create links, images or active HTML just by arriving in a notification.
    let html = format!(
        "<p>{}</p>",
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('\n', "<br>")
    );
    Some(json!({
        "id":format!("native-notice:{}:{}:{}",thread.id,notice.turn_id,notice.sequence),
        "role":"system","kind":"service_notice","provider":"codex_app_server",
        "nativeTurnId":notice.turn_id,"runId":format!("native:{}:{}",thread.id,notice.turn_id),
        "text":text,"renderedHtml":html,"streaming":false
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use central_agent_codex_runtime::mirror::Mirror;
    fn samples() -> Vec<Value> {
        serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/crates/central-agent-codex-runtime/tests/native-model-notices.json"
        )))
        .unwrap()
    }
    #[test]
    fn native_model_notices_are_not_answers_and_hidden_buffering_is_not_rendered() {
        let mut mirror = Mirror::default();
        let samples = samples();
        for sample in &samples[..3] {
            mirror
                .notify(sample["method"].as_str().unwrap(), &sample["params"])
                .unwrap();
        }
        let thread = mirror.thread("notice-thread").unwrap();
        let rows = super::super::messages(thread);
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|row| row["kind"] == "service_notice"
            && row["role"] == "system"
            && row["streaming"] == false));
        assert!(
            rows[0]["text"]
                .as_str()
                .unwrap()
                .contains("requested-model → service-model")
        );
        assert!(
            rows[1]["text"]
                .as_str()
                .unwrap()
                .contains("trustedAccessForCyber")
        );
        assert!(
            rows[2]["text"]
                .as_str()
                .unwrap()
                .contains("suggested-model")
        );
        let id = rows[2]["id"].clone();
        mirror.notify("turn/started", &json!({"threadId":"notice-thread","turn":{"id":"notice-turn","status":"inProgress","items":[]}})).unwrap();
        let rows = super::super::messages(mirror.thread("notice-thread").unwrap());
        assert_eq!(rows[2]["id"], id);
        assert!(
            rows[2]["text"]
                .as_str()
                .unwrap()
                .contains("reports temporary")
        );
        for sample in &samples[3..] {
            mirror
                .notify(sample["method"].as_str().unwrap(), &sample["params"])
                .unwrap();
        }
        let rows = super::super::messages(mirror.thread("notice-thread").unwrap());
        assert_eq!(rows.len(), 1);
        assert!(
            rows[0]["text"]
                .as_str()
                .unwrap()
                .contains("changed the model")
        );
    }
    #[test]
    fn native_model_notice_text_cannot_create_links_or_images_and_disconnect_is_stale() {
        let mut mirror = Mirror::default();
        let mut sample = samples()[0].clone();
        sample["params"]["toModel"] =
            json!("<img src=x onerror=alert(1)> ![image](https://example.com/image) & model");
        mirror
            .notify(sample["method"].as_str().unwrap(), &sample["params"])
            .unwrap();
        mirror.disconnect();
        let rows = super::super::messages(mirror.thread("notice-thread").unwrap());
        let html = rows[0]["renderedHtml"].as_str().unwrap();
        assert!(html.contains("&lt;img"));
        assert!(html.contains("&amp; model"));
        assert!(!html.contains("<img"));
        assert!(!html.contains("<a "));
        assert!(
            rows[0]["text"]
                .as_str()
                .unwrap()
                .starts_with("Previously reported")
        );
        assert!(rows[0].get("checkpoint").is_none());
    }

    #[test]
    fn actionable_runtime_notices_are_visible_service_rows_without_opaque_review_data() {
        let mut mirror = Mirror::default();
        mirror
            .notify(
                "error",
                &json!({"threadId":"thread","turnId":"turn","error":{"message":"Failure <script>","additionalDetails":"PRIVATE_ERROR"},"willRetry":false}),
            )
            .unwrap();
        mirror
            .notify(
                "guardianWarning",
                &json!({"threadId":"thread","message":"Review the approval"}),
            )
            .unwrap();
        mirror
            .notify(
                "item/autoApprovalReview/started",
                &json!({
                    "threadId":"thread","turnId":"turn","startedAtMs":1,"reviewId":"review","targetItemId":"item",
                    "review":{"status":"inProgress","riskLevel":"high","rationale":"PRIVATE_RATIONALE"},
                    "action":{"type":"command","command":"PRIVATE_COMMAND","cwd":"C:\\private","source":"model"}
                }),
            )
            .unwrap();
        mirror
            .notify(
                "turn/moderationMetadata",
                &json!({"threadId":"thread","turnId":"turn","metadata":{"raw":"PRIVATE_MODERATION"}}),
            )
            .unwrap();
        let rows = super::super::messages(mirror.thread("thread").unwrap());
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().all(|row| {
            row["kind"] == "service_notice"
                && row["role"] == "system"
                && row.get("checkpoint").is_none()
        }));
        assert!(
            rows[0]["text"]
                .as_str()
                .unwrap()
                .contains("Failure <script>")
        );
        assert!(
            rows[0]["renderedHtml"]
                .as_str()
                .unwrap()
                .contains("Failure &lt;script&gt;")
        );
        let public = serde_json::to_string(&rows).unwrap();
        assert!(!public.contains("PRIVATE_ERROR"));
        assert!(!public.contains("PRIVATE_RATIONALE"));
        assert!(!public.contains("PRIVATE_COMMAND"));
        assert!(!public.contains("PRIVATE_MODERATION"));
        assert!(public.contains("approval-safety warning"));
        assert!(public.contains("automatic review"));
        assert!(public.contains("opaque moderation metadata"));
    }
}
