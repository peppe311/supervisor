//! Bounded, session-only latency samples. No prompts, file contents or credentials.
use super::*;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ClientTiming {
    clicked_at_ms: f64,
}

#[derive(Clone)]
pub(in crate::browser) struct Trace(Arc<Mutex<Sample>>);
impl std::fmt::Debug for Trace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeliveryTrace")
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Sample {
    #[serde(skip)]
    start: Instant,
    client_to_host_ms: Option<f64>,
    prepared_ms: Option<f64>,
    written_ms: Option<f64>,
    acknowledged_ms: Option<f64>,
    native_input_ms: Option<f64>,
    first_update_ms: Option<f64>,
    outcome: &'static str,
    mode: &'static str,
    owner: String,
    message_id: String,
    thread_id: Option<String>,
    turn_id: Option<String>,
}

impl Trace {
    pub(in crate::browser) fn new(client: Option<ClientTiming>) -> Self {
        let arrival = unix_time_ms() as f64;
        let client_to_host_ms = client
            .map(|c| arrival - c.clicked_at_ms)
            .filter(|ms| ms.is_finite() && *ms >= 0.0 && *ms <= 60_000.0);
        Self(Arc::new(Mutex::new(Sample {
            start: Instant::now(),
            client_to_host_ms,
            prepared_ms: None,
            written_ms: None,
            acknowledged_ms: None,
            native_input_ms: None,
            first_update_ms: None,
            outcome: "pending",
            mode: "ready",
            owner: String::new(),
            message_id: String::new(),
            thread_id: None,
            turn_id: None,
        })))
    }
    pub(super) fn prepared(&self) {
        let mut s = self.0.lock().unwrap();
        s.prepared_ms = Some(elapsed(&s, Instant::now()));
    }
    pub(super) fn written(&self) {
        let mut s = self.0.lock().unwrap();
        s.written_ms = Some(elapsed(&s, Instant::now()));
    }
    pub(super) fn finish(&self, outcome: &'static str) {
        let mut s = self.0.lock().unwrap();
        if s.outcome == "pending" {
            s.outcome = outcome;
        }
    }
    pub(super) fn reply(&self, result: &Result<Value, CallError>) {
        let mut s = self.0.lock().unwrap();
        s.outcome = event_log::call_outcome(result);
        if let Ok(value) = result {
            s.acknowledged_ms = Some(elapsed(&s, Instant::now()));
            if let Some(turn) = value["turn"]["id"]
                .as_str()
                .or_else(|| value["turnId"].as_str())
            {
                s.turn_id = Some(turn.into());
            }
        }
    }
}
fn elapsed(sample: &Sample, now: Instant) -> f64 {
    now.saturating_duration_since(sample.start).as_secs_f64() * 1000.0
}

#[derive(Default)]
pub(super) struct Timings(VecDeque<Trace>);
impl Timings {
    pub(super) fn disconnect(&self) {
        for trace in &self.0 {
            trace.finish("disconnected");
        }
    }
    pub(super) fn register(
        &mut self,
        owner: &str,
        message: &str,
        thread: Option<&str>,
        trace: Trace,
        mode: &'static str,
    ) {
        {
            let mut s = trace.0.lock().unwrap();
            s.owner = owner.into();
            s.message_id = message.into();
            s.thread_id = thread.map(str::to_owned);
            s.mode = mode;
        }
        self.0.push_back(trace);
        while self.0.len() > 64 {
            self.0.pop_front();
        }
    }
    pub(super) fn dispatch(&self, owner: &str, params: &Value) -> Option<Trace> {
        let message = params["clientUserMessageId"].as_str()?;
        let trace = self.0.iter().find(|t| {
            let s = t.0.lock().unwrap();
            s.owner == owner && s.message_id == message
        })?;
        trace.0.lock().unwrap().thread_id = params["threadId"].as_str().map(str::to_owned);
        Some(trace.clone())
    }
    pub(super) fn notification(&self, method: &str, params: &Value, received: Instant) {
        let Some(thread) = params["threadId"].as_str() else {
            return;
        };
        let Some(turn) = params["turnId"].as_str() else {
            return;
        };
        let user = matches!(method, "item/started" | "item/completed")
            && params["item"]["type"] == "userMessage";
        let update = (matches!(
            method,
            "item/agentMessage/delta" | "item/reasoning/summaryTextDelta"
        ) && params["delta"].as_str().is_some_and(|d| !d.is_empty()))
            || (matches!(method, "item/started" | "item/completed")
                && params["item"]["type"] == "agentMessage"
                && params["item"]["text"]
                    .as_str()
                    .is_some_and(|text| !text.is_empty()));
        if !user && !update {
            return;
        }
        for trace in &self.0 {
            let mut s = trace.0.lock().unwrap();
            if s.thread_id.as_deref() != Some(thread) {
                continue;
            }
            if user && params["item"]["clientId"] == s.message_id {
                s.turn_id = Some(turn.into());
                if s.native_input_ms.is_none() {
                    s.native_input_ms = Some(elapsed(&s, received));
                }
            } else if update
                && s.turn_id.as_deref() == Some(turn)
                && s.first_update_ms.is_none()
                && s.native_input_ms
                    .into_iter()
                    .chain(s.acknowledged_ms)
                    .any(|confirmation| elapsed(&s, received) >= confirmation)
            {
                s.first_update_ms = Some(elapsed(&s, received));
            }
        }
    }
    pub(super) fn view(&self, owner: &str, thread: &str) -> Vec<Value> {
        self.0
            .iter()
            .filter_map(|trace| {
                let s = trace.0.lock().unwrap();
                (s.owner == owner && s.thread_id.as_deref() == Some(thread))
                    .then(|| serde_json::to_value(&*s).unwrap())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn receipt_orders_and_foreign_turns_never_fabricate_latency() {
        for ack_first in [true, false] {
            let mut timings = Timings::default();
            let trace = Trace::new(None);
            let start = trace.0.lock().unwrap().start;
            timings.register(
                "chat:a",
                "message-a",
                Some("thread-a"),
                trace.clone(),
                "ready",
            );
            assert!(
                timings
                    .dispatch(
                        "chat:b",
                        &json!({"clientUserMessageId":"message-a","threadId":"thread-a"})
                    )
                    .is_none()
            );
            let dispatch = timings.dispatch("chat:a", &json!({"clientUserMessageId":"message-a","threadId":"thread-a","input":["PRIVATE"]})).unwrap();
            dispatch.prepared();
            dispatch.written();
            let ack = Ok(json!({"turn":{"id":"turn-a"}}));
            if ack_first {
                dispatch.reply(&ack);
            }
            timings.notification("item/started", &json!({"threadId":"thread-a","turnId":"turn-a","item":{"type":"userMessage","clientId":"foreign"}}), start+Duration::from_millis(10));
            assert!(timings.view("chat:a", "thread-a")[0]["nativeInputMs"].is_null());
            timings.notification("item/started", &json!({"threadId":"thread-a","turnId":"turn-a","item":{"type":"userMessage","clientId":"message-a"}}), start+Duration::from_millis(20));
            timings.notification(
                "item/agentMessage/delta",
                &json!({"threadId":"thread-a","turnId":"turn-b","delta":"PRIVATE"}),
                start + Duration::from_millis(25),
            );
            assert!(timings.view("chat:a", "thread-a")[0]["firstUpdateMs"].is_null());
            timings.notification(
                "item/reasoning/summaryTextDelta",
                &json!({"threadId":"thread-a","turnId":"turn-a","delta":"PRIVATE"}),
                start + Duration::from_millis(30),
            );
            if !ack_first {
                dispatch.reply(&ack);
            }
            timings.notification(
                "item/agentMessage/delta",
                &json!({"threadId":"thread-a","turnId":"turn-a","delta":"PRIVATE"}),
                start + Duration::from_millis(40),
            );
            let view = timings.view("chat:a", "thread-a");
            assert_eq!(view[0]["nativeInputMs"], 20.0);
            assert_eq!(view[0]["firstUpdateMs"], 30.0);
            assert_eq!(view[0]["outcome"], "accepted");
            assert!(!serde_json::to_string(&view).unwrap().contains("PRIVATE"));
            assert!(timings.view("chat:b", "thread-a").is_empty());
            assert!(timings.view("chat:a", "thread-b").is_empty());
        }
    }
    #[test]
    fn timings_are_bounded_and_invalid_client_clock_is_not_a_zero() {
        let mut timings = Timings::default();
        for index in 0..70 {
            let trace = Trace::new(Some(ClientTiming {
                clicked_at_ms: f64::INFINITY,
            }));
            timings.register(
                "graph:node",
                &index.to_string(),
                Some("thread"),
                trace,
                "ready",
            );
        }
        let view = timings.view("graph:node", "thread");
        assert_eq!(view.len(), 64);
        assert_eq!(view[0]["messageId"], "6");
        assert!(view[0]["clientToHostMs"].is_null());
        assert!(view[0]["writtenMs"].is_null());
    }
    #[test]
    fn steering_ignores_updates_before_its_own_native_confirmation() {
        let mut timings = Timings::default();
        let trace = Trace::new(None);
        timings.register("chat:a", "steer", Some("thread"), trace.clone(), "steer");
        let delta = json!({"threadId":"thread","turnId":"active","delta":"old ongoing text"});
        timings.notification("item/agentMessage/delta", &delta, Instant::now());
        assert!(timings.view("chat:a", "thread")[0]["firstUpdateMs"].is_null());
        trace.reply(&Ok(json!({"turnId":"active"})));
        let start = trace.0.lock().unwrap().start;
        trace.0.lock().unwrap().acknowledged_ms = Some(20.0);
        // An old update can reach the UI after the transport worker records ACK.
        timings.notification(
            "item/agentMessage/delta",
            &delta,
            start + Duration::from_millis(15),
        );
        assert!(timings.view("chat:a", "thread")[0]["firstUpdateMs"].is_null());
        timings.notification(
            "item/agentMessage/delta",
            &delta,
            start + Duration::from_millis(30),
        );
        assert!(timings.view("chat:a", "thread")[0]["firstUpdateMs"].is_number());
        trace.finish("failed");
        assert_eq!(timings.view("chat:a", "thread")[0]["outcome"], "accepted");
    }
}
