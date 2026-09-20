//! Bounded metadata journal. This is the host's observation order, never a
//! replayable transcript or an authorization source. Payload text is excluded.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    io::{BufRead, BufReader, Write},
    sync::OnceLock,
};

const SEGMENT: usize = 1024;
const CURRENT: &str = "app-server-events.jsonl";
const PREVIOUS: &str = "app-server-events.previous.jsonl";
mod writer;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Entry {
    sequence: u64,
    received_at_ms: u64,
    connection_epoch: u64,
    direction: String,
    method: String,
    owner: Option<String>,
    thread_id: Option<String>,
    turn_id: Option<String>,
    item_id: Option<String>,
    request_id: Option<String>,
    state: Option<String>,
    summary_index: Option<u64>,
    delta_bytes: Option<usize>,
}

#[derive(Default)]
pub(super) struct Journal {
    entries: VecDeque<Entry>,
    sequence: u64,
    segment_count: usize,
    directory: Option<PathBuf>,
    error: Option<String>,
    writer: Option<writer::Writer>,
}

fn identifier(value: &Value) -> Option<String> {
    let text = value
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|n| n.to_string()))?;
    (!text.is_empty()
        && text.len() <= 180
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b)))
    .then_some(text)
}

fn methods() -> &'static HashSet<String> {
    static METHODS: OnceLock<HashSet<String>> = OnceLock::new();
    METHODS.get_or_init(|| {
        [
            include_str!("../../../protocol/app-server/0.153.4/json/ServerNotification.json"),
            include_str!("../../../protocol/app-server/0.153.4/json/ServerRequest.json"),
            include_str!("../../../protocol/app-server/0.153.4/json/ClientRequest.json"),
        ]
        .iter()
        .flat_map(|schema| {
            let schema: Value = serde_json::from_str(schema).expect("official schema");
            schema["oneOf"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|branch| {
                    branch["properties"]["method"]["enum"][0]
                        .as_str()
                        .map(str::to_owned)
                })
                .collect::<Vec<_>>()
        })
        .chain(["connection/closed".into(), "request/answer".into()])
        .collect()
    })
}

fn method_name(method: &str) -> String {
    if methods().contains(method) {
        method.into()
    } else {
        format!("unknown:{:x}", Sha256::digest(method.as_bytes()))
    }
}

fn state_name(value: &Value) -> Option<String> {
    let state = value.as_str()?;
    matches!(
        state,
        "idle"
            | "active"
            | "notLoaded"
            | "inProgress"
            | "completed"
            | "failed"
            | "interrupted"
            | "declined"
            | "pendingInit"
            | "running"
            | "shutdown"
            | "notFound"
            | "accepted"
            | "rejected"
            | "queued"
            | "sent"
            | "transport_error"
            | "delivery_unknown"
            | "closed"
    )
    .then(|| state.into())
}

impl Journal {
    pub(super) fn next_epoch(&self) -> u64 {
        self.entries
            .back()
            .map(|e| e.connection_epoch.saturating_add(1))
            .unwrap_or(0)
    }
    pub(super) fn load(directory: &Path) -> Self {
        // Parse the protocol allowlist at startup, not on the first prompt.
        let _ = methods();
        let mut log = Self {
            directory: Some(directory.into()),
            ..Self::default()
        };
        let result = (|| -> Result<(), String> {
            for name in [PREVIOUS, CURRENT] {
                let path = directory.join(name);
                let file = match fs::File::open(path) {
                    Ok(file) => file,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(_) => return Err("Saved event metadata could not be read".into()),
                };
                if file
                    .metadata()
                    .map_err(|_| "Cannot inspect saved event metadata")?
                    .len()
                    > 4 * 1024 * 1024
                {
                    return Err("Saved event metadata exceeds its size limit".into());
                }
                let mut count = 0;
                for line in BufReader::new(file).lines().take(SEGMENT + 1) {
                    let line = line.map_err(|_| "Cannot read saved event metadata")?;
                    let entry: Entry = serde_json::from_str(&line)
                        .map_err(|_| "Saved event metadata is incomplete")?;
                    if !entry.valid() || entry.sequence <= log.sequence {
                        return Err("Saved event metadata is invalid".into());
                    }
                    log.sequence = entry.sequence;
                    log.entries.push_back(entry);
                    count += 1;
                }
                if count > SEGMENT {
                    return Err("Saved event metadata exceeds its event limit".into());
                }
                if name == CURRENT {
                    log.segment_count = count;
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            log.error = Some(error);
        } else {
            log.writer = Some(writer::Writer::new(directory.into(), log.segment_count));
        }
        log
    }

    pub(super) fn record(
        &mut self,
        epoch: u64,
        owner: Option<&str>,
        direction: &str,
        method: &str,
        params: &Value,
        request_id: Option<String>,
    ) {
        let thread = params
            .get("threadId")
            .or_else(|| params["thread"].get("id"));
        let entry = Entry {
            sequence: self.sequence.saturating_add(1),
            received_at_ms: unix_time_ms().min(u64::MAX as u128) as u64,
            connection_epoch: epoch,
            direction: direction.into(),
            method: method_name(method),
            owner: owner.and_then(|v| identifier(&json!(v))),
            thread_id: thread.and_then(identifier),
            turn_id: params
                .get("turnId")
                .or_else(|| params["turn"].get("id"))
                .and_then(identifier),
            item_id: params
                .get("itemId")
                .or_else(|| params["item"].get("id"))
                .and_then(identifier),
            request_id: request_id
                .and_then(|id| identifier(&json!(id)))
                .or_else(|| params.get("requestId").and_then(identifier)),
            state: [
                &params["outcome"],
                &params["status"]["type"],
                &params["item"]["status"],
                &params["turn"]["status"],
                &params["status"],
            ]
            .into_iter()
            .find_map(state_name),
            summary_index: params["summaryIndex"].as_u64(),
            delta_bytes: params["delta"].as_str().map(str::len),
        };
        self.sequence = entry.sequence;
        if self.error.is_none()
            && let Some(writer) = &self.writer
            && let Err(error) = writer.append(entry.clone())
        {
            self.error = Some(error);
        }
        self.entries.push_back(entry);
        while self.entries.len() > SEGMENT * 2 {
            self.entries.pop_front();
        }
    }

    pub(super) fn view(&self, owner: &str, thread: Option<&str>) -> Value {
        let matching = self
            .entries
            .iter()
            .filter(|e| {
                e.owner.as_deref() == Some(owner)
                    || thread.is_some() && e.thread_id.as_deref() == thread
            })
            .collect::<Vec<_>>();
        let omitted = matching.len().saturating_sub(200);
        let error = self
            .error
            .clone()
            .or_else(|| self.writer.as_ref().and_then(writer::Writer::error));
        json!({"entries":matching.into_iter().skip(omitted).collect::<Vec<_>>(),"retainedLimit":SEGMENT*2,"viewLimit":200,"omitted":omitted,"error":error,"persistent":self.directory.is_some() && error.is_none()})
    }
}

impl Entry {
    fn valid(&self) -> bool {
        let method_valid = methods().contains(&self.method)
            || self.method.strip_prefix("unknown:").is_some_and(|hash| {
                hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
            });
        method_valid
            && matches!(
                self.direction.as_str(),
                "dispatch" | "notification" | "request" | "response" | "connection"
            )
            && [
                &self.owner,
                &self.thread_id,
                &self.turn_id,
                &self.item_id,
                &self.request_id,
            ]
            .into_iter()
            .flatten()
            .all(|s| identifier(&json!(s)).as_ref() == Some(s))
            && self
                .state
                .as_ref()
                .is_none_or(|s| state_name(&json!(s)).is_some())
    }
}

impl BrowserApp {
    pub(super) fn record_native_event(
        &mut self,
        owner: Option<&str>,
        direction: &str,
        method: &str,
        params: &Value,
        request_id: Option<String>,
    ) {
        self.app_server.event_log.record(
            self.app_server.epoch,
            owner,
            direction,
            method,
            params,
            request_id,
        );
    }
}

pub(super) fn call_outcome(result: &Result<Value, CallError>) -> &'static str {
    match result {
        Ok(_) => "accepted",
        Err(CallError::Disconnected {
            delivery_unknown: true,
            ..
        }) => "delivery_unknown",
        Err(_) => "rejected",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lost_ack_is_uncertain_and_native_resolution_keeps_its_request_id() {
        assert_eq!(
            call_outcome(&Err(CallError::Disconnected {
                reason: "PRIVATE".into(),
                delivery_unknown: true
            })),
            "delivery_unknown"
        );
        let mut log = Journal::default();
        log.record(
            1,
            Some("chat:a"),
            "notification",
            "serverRequest/resolved",
            &json!({"threadId":"a","requestId":42}),
            None,
        );
        assert_eq!(
            log.view("chat:a", Some("a"))["entries"][0]["requestId"],
            "42"
        );
    }
    #[test]
    fn journal_preserves_receive_order_scopes_owners_and_excludes_payloads() {
        let temp = tempfile::tempdir().unwrap();
        let mut log = Journal::load(temp.path());
        let params = json!({"threadId":"child","turnId":"r","itemId":"i","summaryIndex":0,"delta":"PRIVATE_REASONING","command":"PRIVATE_COMMAND","token":"PRIVATE_TOKEN","prompt":"PRIVATE_PROMPT","outcome":"accepted"});
        log.record(
            3,
            None,
            "notification",
            "item/reasoning/summaryTextDelta",
            &params,
            None,
        );
        log.record(
            3,
            Some("chat:parent"),
            "response",
            "turn/start",
            &json!({"threadId":"parent","outcome":"accepted"}),
            Some("1.2".into()),
        );
        log.record(
            3,
            Some("chat:child"),
            "request",
            "future/PRIVATE_METHOD",
            &params,
            Some("19".into()),
        );
        log.writer.as_ref().unwrap().flush();
        drop(log);
        let restored = Journal::load(temp.path());
        assert!(restored.error.is_none());
        let view = restored.view("chat:child", Some("child"));
        assert_eq!(view["entries"][0]["sequence"], 1);
        assert_eq!(view["entries"][1]["sequence"], 3);
        assert_eq!(view["entries"][0]["deltaBytes"], 17);
        assert_eq!(view["entries"][1]["requestId"], "19");
        for text in [
            view.to_string(),
            fs::read_to_string(temp.path().join(CURRENT)).unwrap(),
        ] {
            assert!(!text.contains("PRIVATE"));
        }
        assert!(!view.to_string().contains("chat:parent"));
    }
    #[test]
    fn journal_rotates_resumes_sequence_and_never_changes_native_bindings() {
        let temp = tempfile::tempdir().unwrap();
        let mut log = Journal::load(temp.path());
        for index in 0..SEGMENT * 2 + 5 {
            log.record(
                1,
                Some("chat:a"),
                "notification",
                "turn/completed",
                &json!({"threadId":"a","turn":{"id":"r","status":"completed"}}),
                None,
            );
            if index % 128 == 0 {
                log.writer.as_ref().unwrap().flush();
            }
        }
        log.writer.as_ref().unwrap().flush();
        drop(log);
        let mut restored = Journal::load(temp.path());
        assert!(restored.error.is_none());
        assert_eq!(restored.sequence, (SEGMENT * 2 + 5) as u64);
        assert!(restored.entries.len() <= SEGMENT * 2);
        restored.record(
            2,
            Some("chat:a"),
            "connection",
            "connection/closed",
            &json!({"outcome":"closed"}),
            None,
        );
        assert_eq!(restored.sequence, (SEGMENT * 2 + 6) as u64);
        assert_eq!(
            restored.view("chat:a", Some("a"))["entries"]
                .as_array()
                .unwrap()
                .len(),
            200
        );
    }
}
