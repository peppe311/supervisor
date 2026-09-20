//! Read-only display of question replies persisted by the official desktop app.
//! Recognize the complete envelope and an existing question in this same turn;
//! never interpret arbitrary XML/JSON as a command or an approval.
use super::*;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Reply {
    question_item_id: String,
    question: String,
    answer: String,
}

#[derive(Default)]
pub(super) struct Replies {
    answers: HashMap<String, Value>,
    questions: HashMap<String, Vec<String>>,
}

impl Replies {
    pub(super) fn from_turn(turn: &Turn) -> Self {
        let mut result = Self::default();
        for item in &turn.items {
            let Some(text) = user_text(item).filter(|text| text.len() <= 128 * 1024) else {
                continue;
            };
            let Some(body) = text
                .trim()
                .strip_prefix("<send_user_message_question_reply>")
                .and_then(|text| text.strip_suffix("</send_user_message_question_reply>"))
            else {
                continue;
            };
            let Ok(entries) = serde_json::from_str::<Vec<Reply>>(body) else {
                continue;
            };
            if entries.is_empty() || entries.len() > 16 {
                continue;
            }
            let parsed: Option<Vec<_>> = entries
                .into_iter()
                .map(|entry| {
                    if entry.question.is_empty()
                        || entry.question.len() > 32 * 1024
                        || entry.answer.len() > 32 * 1024
                        || entry.question_item_id.len() > 1024
                    {
                        return None;
                    }
                    let identity: Vec<Value> =
                        serde_json::from_str(&entry.question_item_id).ok()?;
                    if identity.len() != 3
                        || identity[0] != "request_user_input_async"
                        || identity[2].as_u64().is_none()
                    {
                        return None;
                    }
                    let call = identity[1].as_str()?;
                    let question = turn.items.iter().find(|candidate| {
                        candidate.id == call && candidate.value["type"] == "agentMessage"
                    })?;
                    if !question.value["text"]
                        .as_str()?
                        .trim()
                        .contains(entry.question.trim())
                    {
                        return None;
                    }
                    Some((call.to_owned(), entry))
                })
                .collect();
            let Some(parsed) = parsed else {
                continue;
            };
            result.answers.insert(item.id.clone(), json!({"entries": parsed.iter().map(|(_, entry)| json!({"question":entry.question,"answer":entry.answer})).collect::<Vec<_>>()}));
            for (call, entry) in parsed {
                let questions = result.questions.entry(call).or_default();
                if !questions.contains(&entry.question) {
                    questions.push(entry.question);
                }
            }
        }
        result
    }

    pub(super) fn project(&self, item: &Item, row: &mut Value) {
        let text = if let Some(reply) = self.answers.get(&item.id) {
            row["nativeQuestionReply"] = reply.clone();
            Some(
                reply["entries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|entry| entry["answer"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            )
        } else if let Some(questions) = self.questions.get(&item.id) {
            // This answered question is an intermediate checkpoint, even when
            // the source app persisted its presentation as final_answer.
            row["messagePhase"] = json!("commentary");
            Some(questions.join("\n\n"))
        } else {
            None
        };
        if let Some(text) = text {
            row["renderedHtml"] = json!(crate::markdown::render_safe_markdown(&text));
            row["text"] = json!(text);
        }
    }
}
