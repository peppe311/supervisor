//! Public, display-only fields of the selected runtime's additional item variants.
//! Never execute tools, load paths/URLs or forward opaque payloads to the WebView.
use regex::Regex;
use serde_json::{Value, json};
use std::sync::OnceLock;

pub(super) fn details(item: &Value, row: &mut Value) -> Option<String> {
    let mut lines = Vec::new();
    let title = match item["type"].as_str()? {
        "webSearch" => {
            field(&mut lines, "Query", &item["query"]);
            let action = &item["action"];
            let title = match action["type"].as_str() {
                Some("search") => {
                    if action["query"] != item["query"] {
                        field(&mut lines, "Query", &action["query"]);
                    }
                    for query in action["queries"].as_array().into_iter().flatten() {
                        field(&mut lines, "Query", query);
                    }
                    "Searched the web"
                }
                Some("openPage") => {
                    field(&mut lines, "URL", &action["url"]);
                    "Opened a web page"
                }
                Some("findInPage") => {
                    field(&mut lines, "URL", &action["url"]);
                    field(&mut lines, "Find", &action["pattern"]);
                    "Searched within a web page"
                }
                _ => "Web search activity",
            };
            if let Some(results) = item["results"].as_array() {
                lines.push(format!(
                    "{} structured results reported; raw payload not displayed",
                    results.len()
                ));
            }
            title.to_owned()
        }
        "collabAgentToolCall" => {
            field(&mut lines, "From thread", &item["senderThreadId"]);
            for id in item["receiverThreadIds"].as_array().into_iter().flatten() {
                field(&mut lines, "To thread", id);
            }
            field(&mut lines, "Requested model", &item["model"]);
            field(&mut lines, "Requested effort", &item["reasoningEffort"]);
            field(&mut lines, "Message", &item["prompt"]);
            if let Some(states) = item["agentsStates"].as_object() {
                for (id, state) in states {
                    lines.push(format!(
                        "Agent {id}: {}",
                        state["status"].as_str().unwrap_or("not reported")
                    ));
                    field(&mut lines, "Response", &state["message"]);
                }
            }
            // A completed tool call does not imply the child agent finished.
            match item["tool"].as_str() {
                Some("spawnAgent") => "Spawn agent",
                Some("sendInput") => "Send input to agent",
                Some("sendMessage") => "Send message to agent",
                Some("followupTask") => "Follow up with agent",
                Some("resumeAgent") => "Resume agent",
                Some("wait") => "Wait for agents",
                Some("interruptAgent") => "Interrupt agent",
                Some("closeAgent") => "Close agent",
                Some("listAgents") => "List agents",
                _ => "Agent collaboration",
            }
            .to_owned()
        }
        "subAgentActivity" => {
            field(&mut lines, "Agent thread", &item["agentThreadId"]);
            field(&mut lines, "Agent path", &item["agentPath"]);
            match item["kind"].as_str() {
                Some("started") => "Subagent started",
                Some("interacted") => "Subagent interaction",
                Some("interrupted") => "Subagent interrupted",
                Some("completed") => "Subagent completed",
                _ => "Subagent activity",
            }
            .to_owned()
        }
        "functionCallOutput" => {
            field(&mut lines, "Namespace", &item["namespace"]);
            lines.push(output(&item["output"]));
            format!("Tool output · {}", item["name"].as_str().unwrap_or("Tool"))
        }
        // Historical display support is not opt-in to experimental dynamic tools.
        "dynamicToolCall" => {
            field(&mut lines, "Namespace", &item["namespace"]);
            lines.push(output(&item["contentItems"]));
            if item["success"] == false {
                row["activityStatus"] = json!("error");
            }
            format!("Dynamic tool · {}", item["tool"].as_str().unwrap_or("Tool"))
        }
        "sleep" => {
            if let Some(duration) = item["durationMs"].as_u64() {
                lines.push(format!("Requested wait: {duration} ms"));
            }
            "Wait".to_owned()
        }
        "imageGeneration" => {
            field(&mut lines, "Status", &item["status"]);
            field(&mut lines, "Revised prompt", &item["revisedPrompt"]);
            field(&mut lines, "Saved path", &item["savedPath"]);
            if item["failure"]["type"] == "usageLimitExceeded" {
                row["activityStatus"] = json!("error");
                lines.push("Image generation usage limit exceeded".into());
                field(&mut lines, "Limit", &item["failure"]["limitId"]);
                if let Some(reset) = item["failure"]["resetsAt"].as_i64() {
                    lines.push(format!("Reset time (Unix seconds): {reset}"));
                }
            }
            lines.push("Completed image results appear separately in the conversation".into());
            // result may contain a large encoded image; never send it as code/text.
            "Image generation".to_owned()
        }
        "hookPrompt" => {
            lines.push("Native hook instructions are not displayed".into());
            "Native hook".to_owned()
        }
        _ => return None,
    };
    row["activityDetail"] = json!(safe_output_text(&lines.join("\n")));
    Some(title)
}

fn field(lines: &mut Vec<String>, label: &str, value: &Value) {
    if let Some(text) = value.as_str().filter(|text| !text.is_empty()) {
        let line = format!("{label}: {text}");
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
}

fn output(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return safe_output_text(text);
    }
    value
        .as_array()
        .into_iter()
        .flatten()
        .map(|part| match part["type"].as_str() {
            Some("input_text" | "inputText") => {
                safe_output_text(part["text"].as_str().unwrap_or_default())
            }
            Some("input_image" | "inputImage") => {
                "[Image output — preview not available here]".into()
            }
            Some("input_audio" | "inputAudio") => {
                "[Audio output — playback not available here]".into()
            }
            Some("encrypted_content") => "[Encrypted output — not displayed]".into(),
            _ => "[Unsupported output content]".into(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn safe_output_text(text: &str) -> String {
    static IMAGE_DATA: OnceLock<Regex> = OnceLock::new();
    static ENCODED_RUN: OnceLock<Regex> = OnceLock::new();
    let image_data = IMAGE_DATA.get_or_init(|| {
        Regex::new(r"(?i)data:image/[a-z0-9.+-]+;base64,[a-z0-9+/=]+")
            .expect("valid image data pattern")
    });
    let encoded_run = ENCODED_RUN.get_or_init(|| {
        Regex::new(r"[A-Za-z0-9+/]{256,}={0,2}").expect("valid encoded payload pattern")
    });
    let without_images =
        image_data.replace_all(text, "[Image output — preview not available here]");
    let readable = encoded_run.replace_all(&without_images, "[Encoded payload — not displayed]");
    const MAX_CHARS: usize = 8_000;
    if readable.chars().count() <= MAX_CHARS {
        return readable.into_owned();
    }
    format!(
        "{}\n[Additional tool output omitted from this view]",
        readable.chars().take(MAX_CHARS).collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_payloads_do_not_enter_tool_details_but_diagnostics_remain() {
        let payload = "A".repeat(400);
        let text = format!("Screenshot: data:image/png;base64,{payload}\nRetry: timeout");
        let display = safe_output_text(&text);
        assert!(display.contains("Screenshot: [Image output"));
        assert!(display.contains("Retry: timeout"));
        assert!(!display.contains(&payload));
        assert_eq!(safe_output_text("2 + 3 = 5"), "2 + 3 = 5");
    }

    #[test]
    fn oversized_output_is_bounded_in_the_display_projection() {
        let display = safe_output_text(&"ordinary text ".repeat(1_000));
        assert!(display.ends_with("[Additional tool output omitted from this view]"));
        assert!(display.len() < 8_200);
    }
}
