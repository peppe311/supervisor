//! Display-only service notifications. They never mutate turn state or profile.
//! Opaque or secret-bearing payloads are reduced to a bounded public projection
//! before they can cross the host/UI boundary.
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_TEXT_BYTES: usize = 16 * 1024;
const MAX_ID_BYTES: usize = 1024;
const MAX_LIST_ITEMS: usize = 32;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reroute {
    pub from_model: String,
    pub to_model: String,
    pub reason: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Verification {
    pub verifications: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Buffering {
    pub model: String,
    pub use_cases: Vec<String>,
    pub reasons: Vec<String>,
    pub show_buffering_ui: bool,
    pub faster_model: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnFailure {
    pub message: String,
    pub will_retry: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RuntimeWarning {
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StrictReview {
    pub started_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoReview {
    pub review_id: String,
    pub target_item_id: Option<String>,
    pub action_kind: String,
    pub status: String,
    pub risk_level: Option<String>,
    pub started_at_ms: u64,
    pub completed_at_ms: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum ModelNotice {
    Rerouted(Reroute),
    Verification(Verification),
    Buffering(Buffering),
    TurnFailure(TurnFailure),
    Warning(RuntimeWarning),
    GuardianWarning(RuntimeWarning),
    StrictReview(StrictReview),
    AutoReview(AutoReview),
    /// The schema intentionally declares metadata as opaque JSON. Presence is
    /// presented, but none of that value is retained or sent to the WebView.
    Moderation,
}
impl ModelNotice {
    pub fn parse(method: &str, params: &Value) -> Result<Option<Self>, String> {
        let parsed = match method {
            "model/rerouted" => serde_json::from_value(params.clone()).map(Self::Rerouted),
            "model/verification" => serde_json::from_value(params.clone()).map(Self::Verification),
            "model/safetyBuffering/updated" => {
                serde_json::from_value(params.clone()).map(Self::Buffering)
            }
            "error" => return parse_turn_failure(params).map(Some),
            "warning" if params.get("threadId").is_some_and(Value::is_string) => {
                return parse_warning(params).map(Self::Warning).map(Some);
            }
            "guardianWarning" => {
                return parse_warning(params).map(Self::GuardianWarning).map(Some);
            }
            "autoApprovalReview/strictReviewRequired" => {
                required_id(params, "threadId")?;
                required_id(params, "turnId")?;
                return Ok(Some(Self::StrictReview(StrictReview {
                    started_at_ms: required_u64(params, "startedAtMs")?,
                })));
            }
            "item/autoApprovalReview/started" => {
                return parse_auto_review(params, false)
                    .map(Self::AutoReview)
                    .map(Some);
            }
            "item/autoApprovalReview/completed" => {
                return parse_auto_review(params, true)
                    .map(Self::AutoReview)
                    .map(Some);
            }
            "turn/moderationMetadata" => {
                required_id(params, "threadId")?;
                required_id(params, "turnId")?;
                if params.get("metadata").is_none() {
                    return Err("Invalid native turn/moderationMetadata notification".into());
                }
                return Ok(Some(Self::Moderation));
            }
            _ => return Ok(None),
        };
        let notice = parsed.map_err(|_| format!("Invalid native {method} notification"))?;
        notice.validate(method)?;
        Ok(Some(notice))
    }
    fn same_kind(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    fn validate(&self, method: &str) -> Result<(), String> {
        let valid = match self {
            Self::Rerouted(value) => {
                text_ok(&value.from_model, MAX_ID_BYTES)
                    && text_ok(&value.to_model, MAX_ID_BYTES)
                    && text_ok(&value.reason, MAX_TEXT_BYTES)
            }
            Self::Verification(value) => {
                value.verifications.len() <= MAX_LIST_ITEMS
                    && value
                        .verifications
                        .iter()
                        .all(|value| text_ok(value, MAX_ID_BYTES))
            }
            Self::Buffering(value) => {
                text_ok(&value.model, MAX_ID_BYTES)
                    && value
                        .faster_model
                        .as_ref()
                        .is_none_or(|value| text_ok(value, MAX_ID_BYTES))
                    && value.use_cases.len() <= MAX_LIST_ITEMS
                    && value.reasons.len() <= MAX_LIST_ITEMS
                    && value
                        .use_cases
                        .iter()
                        .chain(&value.reasons)
                        .all(|value| text_ok(value, MAX_TEXT_BYTES))
            }
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(format!("Invalid native {method} notification"))
        }
    }
}

fn parse_turn_failure(params: &Value) -> Result<ModelNotice, String> {
    required_id(params, "threadId")?;
    required_id(params, "turnId")?;
    let message = required_text(&params["error"], "message", MAX_TEXT_BYTES)?;
    let will_retry = params["willRetry"]
        .as_bool()
        .ok_or("Invalid native error notification")?;
    Ok(ModelNotice::TurnFailure(TurnFailure {
        message: message.into(),
        will_retry,
    }))
}

fn parse_warning(params: &Value) -> Result<RuntimeWarning, String> {
    if let Some(thread) = params.get("threadId").filter(|value| !value.is_null()) {
        let thread = thread
            .as_str()
            .ok_or("Invalid native warning notification")?;
        if !text_ok(thread, MAX_ID_BYTES) {
            return Err("Invalid native warning notification".into());
        }
    }
    Ok(RuntimeWarning {
        message: required_text(params, "message", MAX_TEXT_BYTES)?.into(),
    })
}

fn parse_auto_review(params: &Value, completed: bool) -> Result<AutoReview, String> {
    required_id(params, "threadId")?;
    required_id(params, "turnId")?;
    let review_id = required_id(params, "reviewId")?.to_owned();
    let target_item_id = match params.get("targetItemId") {
        Some(Value::Null) | None => None,
        Some(Value::String(value)) if text_ok(value, MAX_ID_BYTES) => Some(value.clone()),
        _ => return Err("Invalid native approval auto-review notification".into()),
    };
    let action_kind = required_text(&params["action"], "type", 64)?;
    if !matches!(
        action_kind,
        "command"
            | "execve"
            | "writeStdin"
            | "applyPatch"
            | "networkAccess"
            | "mcpToolCall"
            | "requestPermissions"
    ) {
        return Err("Invalid native approval auto-review action".into());
    }
    let status = required_text(&params["review"], "status", 32)?;
    if !matches!(
        status,
        "inProgress" | "approved" | "denied" | "timedOut" | "aborted"
    ) {
        return Err("Invalid native approval auto-review status".into());
    }
    let risk_level = match params["review"].get("riskLevel") {
        Some(Value::Null) | None => None,
        Some(Value::String(value))
            if matches!(value.as_str(), "low" | "medium" | "high" | "critical") =>
        {
            Some(value.clone())
        }
        _ => return Err("Invalid native approval auto-review risk level".into()),
    };
    Ok(AutoReview {
        review_id,
        target_item_id,
        action_kind: action_kind.into(),
        status: status.into(),
        risk_level,
        started_at_ms: required_u64(params, "startedAtMs")?,
        completed_at_ms: if completed {
            Some(required_u64(params, "completedAtMs")?)
        } else {
            None
        },
    })
}

fn required_id<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    required_text(value, key, MAX_ID_BYTES)
}

fn required_text<'a>(value: &'a Value, key: &str, limit: usize) -> Result<&'a str, String> {
    value[key]
        .as_str()
        .filter(|text| text_ok(text, limit))
        .ok_or_else(|| format!("Invalid native notification field {key}"))
}

fn required_u64(value: &Value, key: &str) -> Result<u64, String> {
    value[key]
        .as_u64()
        .ok_or_else(|| format!("Invalid native notification field {key}"))
}

fn text_ok(value: &str, limit: usize) -> bool {
    !value.is_empty() && value.len() <= limit && !value.contains('\0')
}

#[derive(Clone, Debug, Serialize)]
pub struct Notice {
    pub sequence: u64,
    pub turn_id: String,
    pub current: bool,
    pub value: ModelNotice,
}

pub(crate) fn update(notices: &mut Vec<Notice>, turn: &str, value: ModelNotice, sequence: u64) {
    // Verification/buffering are latest snapshots. Reroutes are a sequence;
    // only an identical immediately preceding reroute for this turn is redundant.
    let previous = notices.iter_mut().rev().find(|notice| {
        notice.turn_id == turn
            && match (&notice.value, &value) {
                (ModelNotice::AutoReview(previous), ModelNotice::AutoReview(next)) => {
                    previous.review_id == next.review_id
                }
                (previous, next) => previous.same_kind(next),
            }
    });
    let replaces_previous = !matches!(
        value,
        ModelNotice::Rerouted(_) | ModelNotice::Warning(_) | ModelNotice::GuardianWarning(_)
    ) || previous
        .as_ref()
        .is_some_and(|previous| previous.value == value);
    if let Some(previous) = previous
        && replaces_previous
    {
        previous.value = value;
        previous.current = true;
        return;
    }
    notices.push(Notice {
        sequence,
        turn_id: turn.into(),
        current: true,
        value,
    });
    if notices.len() > 64 {
        notices.remove(0);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum GlobalNoticeValue {
    Warning {
        message: String,
    },
    ConfigWarning {
        summary: String,
        details: Option<String>,
        path: Option<String>,
        line: Option<u64>,
        column: Option<u64>,
    },
    Deprecation {
        summary: String,
        details: Option<String>,
    },
    WindowsWorldWritable {
        sample_paths: Vec<String>,
        extra_count: u64,
        failed_scan: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalNotice {
    pub sequence: u64,
    pub current: bool,
    pub value: GlobalNoticeValue,
}

pub fn parse_global(method: &str, params: &Value) -> Result<Option<GlobalNoticeValue>, String> {
    let value = match method {
        "warning" => match params.get("threadId") {
            Some(Value::Null) => GlobalNoticeValue::Warning {
                message: required_text(params, "message", MAX_TEXT_BYTES)?.into(),
            },
            Some(Value::String(_)) => return Ok(None),
            _ => return Err("Invalid native warning notification".into()),
        },
        "configWarning" => {
            let details = optional_text(params, "details", MAX_TEXT_BYTES)?;
            let path = optional_text(params, "path", MAX_TEXT_BYTES)?;
            let (line, column) = match params.get("range") {
                Some(Value::Null) | None => (None, None),
                Some(range) => (
                    Some(required_u64(&range["start"], "line")?),
                    Some(required_u64(&range["start"], "column")?),
                ),
            };
            GlobalNoticeValue::ConfigWarning {
                summary: required_text(params, "summary", MAX_TEXT_BYTES)?.into(),
                details,
                path,
                line,
                column,
            }
        }
        "deprecationNotice" => GlobalNoticeValue::Deprecation {
            summary: required_text(params, "summary", MAX_TEXT_BYTES)?.into(),
            details: optional_text(params, "details", MAX_TEXT_BYTES)?,
        },
        "windows/worldWritableWarning" => {
            let samples = params["samplePaths"]
                .as_array()
                .ok_or("Invalid native Windows world-writable warning")?;
            let mut sample_paths = Vec::new();
            for sample in samples.iter().take(8) {
                let path = sample
                    .as_str()
                    .filter(|path| text_ok(path, MAX_TEXT_BYTES))
                    .ok_or("Invalid native Windows world-writable path")?;
                sample_paths.push(path.into());
            }
            let omitted = samples.len().saturating_sub(sample_paths.len()) as u64;
            GlobalNoticeValue::WindowsWorldWritable {
                sample_paths,
                extra_count: required_u64(params, "extraCount")?.saturating_add(omitted),
                failed_scan: params["failedScan"]
                    .as_bool()
                    .ok_or("Invalid native Windows world-writable warning")?,
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}

fn optional_text(value: &Value, key: &str, limit: usize) -> Result<Option<String>, String> {
    match value.get(key) {
        Some(Value::Null) | None => Ok(None),
        Some(Value::String(text)) if text_ok(text, limit) => Ok(Some(text.clone())),
        _ => Err(format!("Invalid native notification field {key}")),
    }
}

pub fn push_global(notices: &mut Vec<GlobalNotice>, value: GlobalNoticeValue, sequence: u64) {
    if let Some(previous) = notices.last_mut().filter(|notice| notice.value == value) {
        previous.current = true;
        return;
    }
    notices.push(GlobalNotice {
        sequence,
        current: true,
        value,
    });
    if notices.len() > 32 {
        notices.remove(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mirror::Mirror;
    use serde_json::json;
    fn samples() -> Vec<Value> {
        serde_json::from_str(include_str!("../tests/native-model-notices.json")).unwrap()
    }
    fn send(mirror: &mut Mirror, sample: &Value) {
        assert!(
            mirror
                .notify(sample["method"].as_str().unwrap(), &sample["params"])
                .unwrap()
        );
    }
    #[test]
    fn native_model_notices_do_not_create_turns_or_carry_extra_payload() {
        let mut mirror = Mirror::default();
        for mut sample in samples().into_iter().take(3) {
            sample["params"]["private"] = json!({"token":"PRIVATE_EXTRA"});
            send(&mut mirror, &sample);
        }
        let thread = mirror.thread("notice-thread").unwrap();
        assert!(thread.turns.is_empty());
        assert!(thread.active_turn().is_none());
        assert_eq!(thread.notices.len(), 3);
        assert!(
            !serde_json::to_string(thread)
                .unwrap()
                .contains("PRIVATE_EXTRA")
        );
        let before = mirror.revision();
        let mut invalid = samples()[2].clone();
        invalid["params"]["showBufferingUi"] = json!("true");
        assert!(
            mirror
                .notify(invalid["method"].as_str().unwrap(), &invalid["params"])
                .is_err()
        );
        assert_eq!(mirror.revision(), before);
    }
    #[test]
    fn native_model_notice_updates_keep_identity_and_preserve_reroute_sequence() {
        let mut mirror = Mirror::default();
        let samples = samples();
        for sample in &samples[..3] {
            send(&mut mirror, sample);
        }
        let ids: Vec<_> = mirror
            .thread("notice-thread")
            .unwrap()
            .notices
            .iter()
            .map(|notice| notice.sequence)
            .collect();
        for sample in &samples {
            send(&mut mirror, sample);
        }
        let notices = &mirror.thread("notice-thread").unwrap().notices;
        assert_eq!(
            notices
                .iter()
                .map(|notice| notice.sequence)
                .collect::<Vec<_>>(),
            ids
        );
        assert!(
            matches!(&notices[2].value, ModelNotice::Buffering(value) if !value.show_buffering_ui)
        );
        assert!(
            matches!(&notices[1].value, ModelNotice::Verification(value) if value.verifications.is_empty())
        );
        let mut next = samples[0].clone();
        next["params"]["fromModel"] = json!("service-model");
        next["params"]["toModel"] = json!("another-model");
        send(&mut mirror, &next);
        assert_eq!(mirror.thread("notice-thread").unwrap().notices.len(), 4);
        next["params"]["turnId"] = json!("another-turn");
        send(&mut mirror, &next);
        assert_eq!(mirror.thread("notice-thread").unwrap().notices.len(), 5);
    }
    #[test]
    fn native_model_notices_remain_stale_after_history_read_until_new_event() {
        let mut mirror = Mirror::default();
        let samples = samples();
        send(&mut mirror, &samples[2]);
        mirror.disconnect();
        mirror.hydrate(&json!({"id":"notice-thread","turns":[{"id":"notice-turn","status":"completed","items":[]}]}), mirror.revision()).unwrap();
        assert!(!mirror.thread("notice-thread").unwrap().notices[0].current);
        send(&mut mirror, &samples[2]);
        assert!(mirror.thread("notice-thread").unwrap().notices[0].current);
        assert!(!mirror.thread("notice-thread").unwrap().turns[0].active());
        mirror
            .notify(
                "thread/status/changed",
                &json!({"threadId":"notice-thread","status":{"type":"notLoaded"}}),
            )
            .unwrap();
        assert!(!mirror.thread("notice-thread").unwrap().notices[0].current);
        send(&mut mirror, &samples[1]);
        mirror.unload("notice-thread");
        assert!(
            mirror
                .thread("notice-thread")
                .unwrap()
                .notices
                .iter()
                .all(|notice| !notice.current)
        );
    }

    #[test]
    fn actionable_notices_keep_only_bounded_public_state_and_do_not_create_turns() {
        let mut mirror = Mirror::default();
        let events = [
            (
                "error",
                json!({"threadId":"thread","turnId":"turn","error":{"message":"Immediate failure","additionalDetails":"PRIVATE_ERROR"},"willRetry":true}),
            ),
            (
                "warning",
                json!({"threadId":"thread","message":"Check the runtime configuration","private":"PRIVATE_WARNING"}),
            ),
            (
                "guardianWarning",
                json!({"threadId":"thread","message":"Review this approval","private":"PRIVATE_GUARDIAN"}),
            ),
            (
                "autoApprovalReview/strictReviewRequired",
                json!({"threadId":"thread","turnId":"turn","startedAtMs":10,"private":"PRIVATE_STRICT"}),
            ),
            (
                "item/autoApprovalReview/started",
                json!({
                    "threadId":"thread","turnId":"turn","startedAtMs":11,"reviewId":"review",
                    "targetItemId":"item","review":{"status":"inProgress","riskLevel":"high","rationale":"PRIVATE_RATIONALE"},
                    "action":{"type":"command","command":"PRIVATE_COMMAND","cwd":"C:\\private","source":"model"}
                }),
            ),
            (
                "item/autoApprovalReview/completed",
                json!({
                    "threadId":"thread","turnId":"turn","startedAtMs":11,"completedAtMs":12,"reviewId":"review",
                    "targetItemId":"item","decisionSource":"agent","review":{"status":"approved","riskLevel":"high","rationale":"PRIVATE_RATIONALE"},
                    "action":{"type":"command","command":"PRIVATE_COMMAND","cwd":"C:\\private","source":"model"}
                }),
            ),
            (
                "turn/moderationMetadata",
                json!({"threadId":"thread","turnId":"turn","metadata":{"token":"PRIVATE_MODERATION"}}),
            ),
        ];
        for (method, params) in events {
            assert!(mirror.notify(method, &params).unwrap());
        }
        let thread = mirror.thread("thread").unwrap();
        assert!(thread.turns.is_empty());
        assert_eq!(thread.notices.len(), 6);
        assert!(matches!(
            &thread.notices[4].value,
            ModelNotice::AutoReview(value)
                if value.status == "approved" && value.completed_at_ms == Some(12)
        ));
        let public = serde_json::to_string(thread).unwrap();
        for private in [
            "PRIVATE_ERROR",
            "PRIVATE_WARNING",
            "PRIVATE_GUARDIAN",
            "PRIVATE_STRICT",
            "PRIVATE_RATIONALE",
            "PRIVATE_COMMAND",
            "PRIVATE_MODERATION",
        ] {
            assert!(!public.contains(private));
        }
    }

    #[test]
    fn actionable_notice_validation_fails_closed_and_global_warnings_require_null_scope() {
        assert!(
            ModelNotice::parse(
                "item/autoApprovalReview/started",
                &json!({
                    "threadId":"thread","turnId":"turn","startedAtMs":1,"reviewId":"review",
                    "targetItemId":null,"review":{"status":"inProgress","riskLevel":null},
                    "action":{"type":"futureAction"}
                })
            )
            .is_err()
        );
        assert!(parse_global("warning", &json!({"message":"missing scope"})).is_err());
        assert!(
            parse_global(
                "warning",
                &json!({"threadId":null,"message":"global warning"})
            )
            .unwrap()
            .is_some()
        );
        assert!(
            parse_global(
                "warning",
                &json!({"threadId":"thread","message":"thread warning"})
            )
            .unwrap()
            .is_none()
        );
    }
}
