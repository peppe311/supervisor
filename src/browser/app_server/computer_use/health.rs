//! Passive connection evidence from native tool events. No probes, timers,
//! private pipe clients, screenshot cache or automatic input replay.
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::browser::app_server) enum Status {
    #[default]
    Unchecked,
    Responded,
    ConnectionError,
}

#[derive(Default)]
pub(in crate::browser::app_server) struct Health {
    pub status: Status,
    pending: HashSet<(String, String, String)>,
}

impl Health {
    pub fn notice(&mut self, method: &str, params: &Value) -> bool {
        let before = self.status;
        let item = &params["item"];
        let thread = params["threadId"].as_str().unwrap_or_default();
        let turn = params["turnId"].as_str().unwrap_or_default();
        let id = item["id"].as_str().unwrap_or_default();
        if thread.is_empty() {
            return false;
        }
        if matches!(
            method,
            "turn/completed" | "thread/closed" | "thread/archived" | "thread/deleted"
        ) {
            self.pending.retain(|key| key.0 != thread);
        } else if !turn.is_empty() && !id.is_empty() {
            let key = (thread.to_owned(), turn.to_owned(), id.to_owned());
            if method == "item/started" && is_desktop_operation(item) && self.pending.len() < 64 {
                self.pending.insert(key);
            } else if method == "item/completed" && self.pending.remove(&key) {
                let failed = item["status"] == "failed"
                    || !item["error"].is_null()
                    || item["result"]["isError"] == true;
                if failed && connection_error(item) {
                    self.status = Status::ConnectionError;
                    // A late completion from another already-started call must
                    // not clear an error. A newly started call can supply new evidence.
                    self.pending.clear();
                } else if !failed && item["status"] == "completed" {
                    self.status = Status::Responded;
                }
            }
        }
        before != self.status
    }
}

fn is_desktop_operation(item: &Value) -> bool {
    if item["type"] != "mcpToolCall" || item["server"] != "node_repl" || item["tool"] != "js" {
        return false;
    }
    let code = ["code", "js", "script"]
        .iter()
        .find_map(|key| item["arguments"][*key].as_str())
        .unwrap_or_default();
    // This is only a display hint about a completed native tool invocation,
    // never authority to skip, replay or rewrite any operation within JavaScript.
    [
        "list_apps",
        "list_windows",
        "get_window",
        "get_window_state",
        "activate_window",
        "launch_app",
        "click",
        "press_key",
        "type_text",
        "scroll",
        "drag",
        "set_value",
    ]
    .iter()
    .any(|name| code.contains(&format!("sky.{name}(")))
}

fn connection_error(item: &Value) -> bool {
    let messages = item["error"]["message"].as_str().into_iter().chain(
        item["result"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|block| block["type"] == "text")
            .filter_map(|block| block["text"].as_str()),
    );
    messages.take(16).any(|text| {
        let text: String = text
            .chars()
            .take(8192)
            .flat_map(char::to_lowercase)
            .collect();
        text.contains("computer use native pipe")
            || text.contains("computer-use native pipe")
            || text.contains("sky_cua_native_pipe_directory")
            || text.contains("computer-use request timed out")
            || (text.contains("codex-computer-use-")
                && ["enoent", "econnrefused", "epipe"]
                    .iter()
                    .any(|error| text.contains(error)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn call(id: &str) -> Value {
        json!({"threadId":"thread","turnId":"turn","item":{"id":id,"type":"mcpToolCall",
            "server":"node_repl","tool":"js","arguments":{"code":"await sky.get_window_state({window});"},
            "status":"completed","error":null,"result":{"content":[]}}})
    }

    #[test]
    fn discovery_or_import_is_not_a_verified_desktop_connection() {
        let mut health = Health::default();
        let mut item = call("import");
        item["item"]["arguments"]["code"] =
            json!("globalThis.sky = (await import('@oai/sky')).sky");
        health.notice("item/started", &item);
        health.notice("item/completed", &item);
        assert_eq!(health.status, Status::Unchecked);
        // History/completed items that were not started in this connection do not count.
        health.notice("item/completed", &call("old"));
        assert_eq!(health.status, Status::Unchecked);
    }

    #[test]
    fn connection_failure_is_passive_scoped_and_cannot_replay_or_accept_late_success() {
        let mut health = Health::default();
        let mut failed = call("failure");
        health.notice("item/started", &failed);
        health.notice("item/started", &call("late"));
        failed["item"]["status"] = json!("failed");
        failed["item"]["error"] =
            json!({"message":"Computer Use native pipe connection timed out"});
        assert!(health.notice("item/completed", &failed));
        assert_eq!(health.status, Status::ConnectionError);
        health.notice("item/completed", &call("late"));
        assert_eq!(health.status, Status::ConnectionError);
        let fresh = call("fresh");
        health.notice("item/started", &fresh);
        assert!(health.notice("item/completed", &fresh));
        assert_eq!(health.status, Status::Responded);
    }

    #[test]
    fn application_errors_and_success_text_do_not_claim_connection_loss() {
        let mut health = Health::default();
        let mut item = call("app-error");
        health.notice("item/started", &item);
        item["item"]["status"] = json!("failed");
        item["item"]["error"] = json!({"message":"The selected window no longer exists"});
        health.notice("item/completed", &item);
        assert_eq!(health.status, Status::Unchecked);
        item = call("text");
        health.notice("item/started", &item);
        item["item"]["result"]["content"] = json!([{"type":"text","text":"Example: Computer Use native pipe connection timed out"}]);
        health.notice("item/completed", &item);
        assert_eq!(health.status, Status::Responded);
    }
}
