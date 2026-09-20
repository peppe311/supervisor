//! JSONL envelope and connection state. No provider prompts, tools, or history
//! are reconstructed here. IDs for requests in each direction are independent.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum RequestId {
    Text(String),
    Integer(i64),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (RPC {})", self.message, self.code)
    }
}

impl std::error::Error for RpcError {}

#[derive(Clone, Debug, PartialEq)]
pub enum Incoming {
    Response {
        id: RequestId,
        result: Result<Value, RpcError>,
    },
    Request {
        id: RequestId,
        method: String,
        params: Value,
    },
    Notification {
        method: String,
        params: Value,
    },
}

/// Parse an entire frame, never a truncated output chunk. Optional unknown
/// fields are tolerated but ambiguous response/request shapes are rejected.
pub fn decode(line: &str) -> Result<Incoming, String> {
    let value: Value =
        serde_json::from_str(line).map_err(|_| "Invalid JSON from App Server".to_owned())?;
    let object = value
        .as_object()
        .ok_or("App Server frame must be an object")?;
    if let Some(method) = object.get("method") {
        let method = method
            .as_str()
            .filter(|m| !m.is_empty())
            .ok_or("Invalid RPC method")?
            .to_owned();
        if object.contains_key("result") || object.contains_key("error") {
            return Err("RPC method frame also contains a response".into());
        }
        let params = object.get("params").cloned().unwrap_or(Value::Null);
        return match object.get("id") {
            Some(id) => Ok(Incoming::Request {
                id: parse_id(id)?,
                method,
                params,
            }),
            None => Ok(Incoming::Notification { method, params }),
        };
    }
    let id = parse_id(object.get("id").ok_or("RPC response is missing its ID")?)?;
    let result = match (object.get("result"), object.get("error")) {
        (Some(result), None) => Ok(result.clone()),
        (None, Some(error)) => {
            Err(serde_json::from_value(error.clone()).map_err(|_| "Invalid RPC error")?)
        }
        _ => return Err("RPC response must contain either result or error".into()),
    };
    Ok(Incoming::Response { id, result })
}

fn parse_id(id: &Value) -> Result<RequestId, String> {
    serde_json::from_value(id.clone()).map_err(|_| "RPC ID must be an integer or string".into())
}

pub fn request(id: &RequestId, method: &str, params: Value) -> Value {
    json!({"id": id, "method": method, "params": params})
}

pub fn response(id: &RequestId, result: Result<Value, RpcError>) -> Value {
    match result {
        Ok(value) => json!({"id": id, "result": value}),
        Err(error) => json!({"id": id, "error": error}),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    New,
    Initializing,
    Ready,
    Closed,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ServerRequestKey {
    pub generation: u64,
    pub id: RequestId,
}

/// Tracks wire correlation only. Reconnection creates a new Session; no
/// pending user request is automatically replayed into the new connection.
pub struct Session {
    phase: Phase,
    generation: u64,
    sequence: u64,
    initialize_id: Option<RequestId>,
    pending: HashMap<RequestId, String>,
    server_requests: HashSet<RequestId>,
}

impl Session {
    pub fn new(generation: u64) -> Self {
        Self {
            phase: Phase::New,
            generation,
            sequence: 0,
            initialize_id: None,
            pending: HashMap::new(),
            server_requests: HashSet::new(),
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    fn next_id(&mut self) -> Result<RequestId, String> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("RPC ID space exhausted")?;
        Ok(RequestId::Text(format!(
            "ca:{}:{}",
            self.generation, self.sequence
        )))
    }

    pub fn initialize(&mut self, version: &str) -> Result<Value, String> {
        if self.phase != Phase::New {
            return Err("Initialize is allowed once per connection".into());
        }
        let id = self.next_id()?;
        self.initialize_id = Some(id.clone());
        self.pending.insert(id.clone(), "initialize".into());
        self.phase = Phase::Initializing;
        Ok(request(
            &id,
            "initialize",
            json!({
                "clientInfo": {"name": crate::CLIENT_NAME, "title": "Supervisor", "version": version},
                "capabilities": {
                    "experimentalApi": false,
                    "requestAttestation": false,
                    "mcpServerOpenaiFormElicitation": true,
                    "extensions": {"openai/form": {}}
                }
            }),
        ))
    }

    pub fn begin(&mut self, method: &str, params: Value) -> Result<(RequestId, Value), String> {
        if self.phase != Phase::Ready {
            return Err("App Server connection is not ready".into());
        }
        if method.is_empty() || matches!(method, "initialize" | "initialized") {
            return Err("Invalid application RPC method".into());
        }
        let id = self.next_id()?;
        self.pending.insert(id.clone(), method.into());
        Ok((id.clone(), request(&id, method, params)))
    }

    /// Returns the method associated with an acknowledged client request.
    /// The caller writes `initialized` BEFORE reporting the connection ready.
    pub fn resolve(
        &mut self,
        id: &RequestId,
        success: bool,
    ) -> Result<(String, Option<Value>), String> {
        if self.phase == Phase::Closed {
            return Err("Response arrived on a closed connection".into());
        }
        let method = self
            .pending
            .remove(id)
            .ok_or("Unrecognized or duplicate RPC response ID")?;
        let mut notification = None;
        if self.initialize_id.as_ref() == Some(id) {
            self.initialize_id = None;
            if success {
                self.phase = Phase::Ready;
                notification = Some(json!({"method": "initialized"}));
            } else {
                self.close();
            }
        }
        Ok((method, notification))
    }

    pub fn receive_request(&mut self, id: RequestId) -> Result<ServerRequestKey, String> {
        if self.phase == Phase::Closed {
            return Err("Request arrived on a closed connection".into());
        }
        if !self.server_requests.insert(id.clone()) {
            return Err("Duplicate server request ID".into());
        }
        Ok(ServerRequestKey {
            generation: self.generation,
            id,
        })
    }

    pub fn answer(
        &mut self,
        key: &ServerRequestKey,
        result: Result<Value, RpcError>,
    ) -> Result<Value, String> {
        if key.generation != self.generation
            || self.phase == Phase::Closed
            || !self.server_requests.remove(&key.id)
        {
            return Err("This server request is stale or already resolved".into());
        }
        Ok(response(&key.id, result))
    }

    pub fn dismiss(&mut self, id: &RequestId) {
        self.server_requests.remove(id);
    }

    pub fn close(&mut self) -> Vec<RequestId> {
        self.phase = Phase::Closed;
        self.initialize_id = None;
        self.server_requests.clear();
        self.pending.drain().map(|(id, _)| id).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready(generation: u64) -> Session {
        let mut s = Session::new(generation);
        let start = s.initialize("0.1.0").unwrap();
        let id = parse_id(&start["id"]).unwrap();
        let (_, ack) = s.resolve(&id, true).unwrap();
        assert_eq!(ack.unwrap(), json!({"method":"initialized"}));
        s
    }

    #[test]
    fn handshake_is_mandatory_once_and_uses_official_wire_shape() {
        let mut s = Session::new(1);
        assert!(s.begin("model/list", json!({})).is_err());
        let frame = s.initialize("0.1.0").unwrap();
        assert!(frame.get("jsonrpc").is_none());
        assert_eq!(frame["params"]["clientInfo"]["name"], crate::CLIENT_NAME);
        assert_eq!(frame["params"]["capabilities"]["experimentalApi"], false);
        assert_eq!(
            frame["params"]["capabilities"]["mcpServerOpenaiFormElicitation"],
            true
        );
        assert_eq!(
            frame["params"]["capabilities"]["extensions"]["openai/form"],
            json!({})
        );
        assert_eq!(frame["params"]["capabilities"]["requestAttestation"], false);
        assert!(s.initialize("0.1.0").is_err());
        assert!(s.begin("model/list", json!({})).is_err());
        assert!(ready(2).begin("initialize", json!({})).is_err());
    }

    #[test]
    fn out_of_order_responses_and_opposite_direction_ids_are_independent() {
        let mut s = ready(3);
        let (a, _) = s.begin("account/read", json!({})).unwrap();
        let (b, _) = s.begin("model/list", json!({})).unwrap();
        let server = s.receive_request(a.clone()).unwrap();
        assert_eq!(s.resolve(&b, true).unwrap().0, "model/list");
        assert_eq!(s.resolve(&a, false).unwrap().0, "account/read");
        assert!(s.answer(&server, Ok(json!({"decision":"decline"}))).is_ok());
        assert!(s.resolve(&a, true).is_err());
    }

    #[test]
    fn closure_fails_pending_calls_and_does_not_allow_stale_approvals() {
        let mut s = ready(4);
        let (id, _) = s.begin("turn/start", json!({})).unwrap();
        let key = s.receive_request(RequestId::Integer(9)).unwrap();
        assert_eq!(s.close(), vec![id]);
        assert!(s.answer(&key, Ok(json!({}))).is_err());
        let mut next = ready(5);
        next.receive_request(RequestId::Integer(9)).unwrap();
        assert!(next.answer(&key, Ok(json!({}))).is_err());
    }

    #[test]
    fn server_resolution_invalidates_visible_approval() {
        let mut s = ready(6);
        let key = s
            .receive_request(RequestId::Text("request-a".into()))
            .unwrap();
        s.dismiss(&key.id);
        assert!(s.answer(&key, Ok(json!({"decision":"accept"}))).is_err());
    }

    #[test]
    fn malformed_frames_never_become_actions() {
        for raw in [
            "[]",
            "null",
            "not json",
            r#"{"id":1.5,"result":{}}"#,
            r#"{"id":null,"result":{}}"#,
            r#"{"id":1,"result":{},"error":{}}"#,
            r#"{"method":"x","result":{}}"#,
        ] {
            assert!(decode(raw).is_err(), "{raw}");
        }
        assert!(matches!(
            decode(r#"{"method":"future/event","params":{"value":1}}"#).unwrap(),
            Incoming::Notification { .. }
        ));
        assert!(matches!(
            decode(r#"{"id":-9,"result":null}"#).unwrap(),
            Incoming::Response {
                result: Ok(Value::Null),
                ..
            }
        ));
    }
}
