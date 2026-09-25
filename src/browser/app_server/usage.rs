//! Numeric display projection only. Never estimates, model input or billing data.
use super::*;
use central_agent_codex_runtime::mirror::Thread;

const COUNTERS: [&str; 6] = [
    "totalTokens",
    "inputTokens",
    "cachedInputTokens",
    "cacheWriteInputTokens",
    "outputTokens",
    "reasoningOutputTokens",
];

// Decimal strings preserve native integer precision across the WebView boundary.
fn count(value: &Value) -> Value {
    value
        .as_u64()
        .map(|n| Value::String(n.to_string()))
        .unwrap_or(Value::Null)
}
fn breakdown(value: &Value) -> Value {
    Value::Object(
        COUNTERS
            .into_iter()
            .map(|key| (key.into(), count(&value[key])))
            .collect(),
    )
}
fn projection(thread: Option<&Thread>, visible: bool, connected: bool) -> Value {
    let usage = thread.and_then(|t| t.token_usage.as_ref());
    json!({"visible":visible,"connected":connected,
        "current": connected && thread.is_some_and(|t| t.token_usage_current),
        "turnId": thread.and_then(|t| t.token_usage_turn_id.as_ref()),
        "activeTurnId": thread.and_then(Thread::active_turn).map(|t| &t.id),
        "model": thread.and_then(|t| t.token_usage_model.as_ref()),
        "cacheReportAtMs": thread.and_then(|t| t.token_usage_observed_at_ms),
        "report":usage.map(|u| json!({"last":breakdown(&u["last"]),"total":breakdown(&u["total"]),"modelContextWindow":count(&u["modelContextWindow"])}))})
}
impl BrowserApp {
    pub(in crate::browser) fn app_server_usage(&self, owner: &str) -> Value {
        let thread = self
            .app_server
            .conversations
            .binding(owner)
            .and_then(|b| self.app_server.conversations.mirror.thread(&b.thread_id));
        projection(
            thread,
            self.native_access_selected(owner),
            self.app_server.view.connected,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use central_agent_codex_runtime::mirror::Mirror;
    #[test]
    fn usage_preserves_missing_zero_large_values_and_native_counter_categories() {
        let mut mirror = Mirror::default();
        mirror.notify("thread/tokenUsage/updated", &json!({"threadId":"a","turnId":"turn-a","tokenUsage":{
            "last":{"totalTokens":42,"inputTokens":0,"outputTokens":-1,"cachedInputTokens":8,"reasoningOutputTokens":"invalid"},
            "total":{"totalTokens":9007199254740993u64},"modelContextWindow":null,"private":"not projected"}})).unwrap();
        let value = projection(mirror.thread("a"), true, true);
        assert_eq!(value["report"]["last"]["inputTokens"], "0");
        assert!(value["report"]["last"]["outputTokens"].is_null());
        assert!(value["report"]["last"]["cacheWriteInputTokens"].is_null());
        assert_eq!(value["report"]["total"]["totalTokens"], "9007199254740993");
        assert!(!value.to_string().contains("private"));
        assert_eq!(value["current"], true);
        assert!(
            value["cacheReportAtMs"]
                .as_u64()
                .is_some_and(|time| time > 0)
        );
        assert!(value["model"].is_null());
        mirror.disconnect();
        assert_eq!(projection(mirror.thread("a"), true, true)["current"], false);
        assert_eq!(
            projection(mirror.thread("a"), true, false)["connected"],
            false
        );
        assert_eq!(
            projection(mirror.thread("a"), false, true)["visible"],
            false
        );
        assert!(projection(None, true, true)["report"].is_null());
    }
}
