//! Pending native server requests. This is a consent boundary, not a tool loop.
//! Requests and secret answers are memory-only; the native server owns their life.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::wire::{RequestId, ServerRequestKey};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Command,
    Files,
    Permissions,
    Questions,
    Mcp,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub ticket: String,
    pub owner: String,
    pub kind: Kind,
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub params: Value,
    pub responding: bool,
    pub choices: Option<ApprovalChoices>,
}

/// Presentation of supported native decisions, not a local execution policy.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalChoices {
    pub accept: bool,
    pub decline: bool,
    pub cancel: bool,
    pub session: bool,
    pub exec_policy: bool,
    pub network_policies: Vec<usize>,
}

struct Pending {
    key: ServerRequestKey,
    // Display order only. Neither opaque RPC IDs nor textual ticket ordering
    // describe arrival order ("request:7:10" sorts before "request:7:2").
    arrival: u64,
    view: View,
    params: Value,
}

#[derive(Default)]
pub struct Requests {
    serial: u64,
    pending: BTreeMap<String, Pending>,
}

/// Semantic user intent only. The UI cannot supply an RPC ID, method, policy,
/// filesystem grant, or result envelope of its own.
#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Decision {
    Accept {},
    Session {},
    Decline {},
    Cancel {},
    ExecPolicy {},
    NetworkPolicy {
        index: usize,
    },
    Permissions {
        grant: bool,
        session: bool,
    },
    Answers {
        answers: BTreeMap<String, Vec<String>>,
    },
    Elicitation {
        action: ElicitationAction,
        content: Option<Value>,
    },
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElicitationAction {
    Accept,
    Decline,
    Cancel,
}

impl Requests {
    pub fn views(&self, owner: &str) -> Vec<View> {
        let mut pending: Vec<_> = self
            .pending
            .values()
            .filter(|p| p.view.owner == owner)
            .collect();
        pending.sort_unstable_by_key(|p| p.arrival);
        pending.into_iter().map(|p| p.view.clone()).collect()
    }

    pub fn insert(
        &mut self,
        owner: &str,
        key: ServerRequestKey,
        method: &str,
        mut params: Value,
    ) -> Result<(), String> {
        if owner.is_empty() || self.pending.values().any(|p| p.key == key) {
            return Err("Unknown conversation or duplicate native request".into());
        }
        let kind = match method {
            "item/commandExecution/requestApproval" => Kind::Command,
            "item/fileChange/requestApproval" => Kind::Files,
            "item/permissions/requestApproval" => Kind::Permissions,
            "item/tool/requestUserInput" => Kind::Questions,
            "mcpServer/elicitation/request" => Kind::Mcp,
            _ => return Err("This native request is not supported by this client".into()),
        };
        let thread_id = text(&params, "threadId")?.to_owned();
        let turn_id = params
            .get("turnId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if kind != Kind::Mcp {
            text(&params, "turnId")?;
            text(&params, "itemId")?;
        }
        match kind {
            Kind::Questions => {
                let questions = params["questions"]
                    .as_array()
                    .ok_or("Missing native questions")?;
                let mut ids = std::collections::HashSet::new();
                for question in questions {
                    if !ids.insert(text(question, "id")?) {
                        return Err("Duplicate question ID".into());
                    }
                    text(question, "question")?;
                }
            }
            Kind::Permissions if !params["permissions"].is_object() => {
                return Err("Missing requested permissions".into());
            }
            Kind::Mcp => {
                text(&params, "serverName")?;
                match text(&params, "mode")? {
                    "url" => {
                        text(&params, "url")?;
                    }
                    "form" | "openai/form" | "openaiForm" => {
                        let schema = normalize_form_schema(&params["requestedSchema"])?;
                        params["requestedSchema"] = schema;
                    }
                    _ => {
                        return Err(
                            "This MCP form mode requires a client capability that is not enabled"
                                .into(),
                        );
                    }
                }
            }
            _ => {}
        }
        // _meta, tokens, internal IDs and unknown future fields never enter the UI.
        let keys: &[&str] = match kind {
            Kind::Command => &[
                "reason",
                "command",
                "cwd",
                "commandActions",
                "networkApprovalContext",
                "proposedExecpolicyAmendment",
                "proposedNetworkPolicyAmendments",
                "environmentId",
                "kind",
            ],
            Kind::Files => &["reason", "grantRoot", "itemId"],
            Kind::Permissions => &["reason", "cwd", "permissions", "environmentId"],
            Kind::Questions => &["questions", "isBlocking"],
            Kind::Mcp => &["serverName", "mode", "message", "url", "requestedSchema"],
        };
        let public = keys
            .iter()
            .filter_map(|key| params.get(*key).map(|v| ((*key).to_owned(), v.clone())))
            .collect::<serde_json::Map<_, _>>();
        self.serial = self
            .serial
            .checked_add(1)
            .ok_or("Request ID space exhausted")?;
        let ticket = format!("request:{}:{}", key.generation, self.serial);
        let view = View {
            ticket: ticket.clone(),
            owner: owner.into(),
            kind,
            thread_id,
            turn_id,
            params: Value::Object(public),
            responding: false,
            choices: approval_choices(kind, &params),
        };
        self.pending.insert(
            ticket,
            Pending {
                key,
                arrival: self.serial,
                view,
                params,
            },
        );
        Ok(())
    }

    pub fn answer(
        &mut self,
        owner: &str,
        ticket: &str,
        decision: Decision,
    ) -> Result<(ServerRequestKey, Value), String> {
        let pending = self
            .pending
            .get_mut(ticket)
            .filter(|p| p.view.owner == owner)
            .ok_or("This request has expired or belongs to another conversation")?;
        if pending.view.responding {
            return Err("This decision is already being sent".into());
        }
        let value = response(pending.view.kind, &pending.params, decision)?;
        pending.view.responding = true;
        Ok((pending.key.clone(), value))
    }

    /// Exact same key as the send: a late send result cannot consume a new request.
    pub fn sent(&mut self, key: &ServerRequestKey) -> Option<String> {
        let ticket = self
            .pending
            .iter()
            .find(|(_, p)| &p.key == key)
            .map(|(id, _)| id.clone())?;
        self.pending.remove(&ticket).map(|p| p.view.owner)
    }

    pub fn url(&self, owner: &str, ticket: &str) -> Result<&str, String> {
        let p = self
            .pending
            .get(ticket)
            .filter(|p| {
                p.view.owner == owner
                    && !p.view.responding
                    && p.view.kind == Kind::Mcp
                    && p.params["mode"] == "url"
            })
            .ok_or("This MCP request is no longer available")?;
        text(&p.params, "url")
    }

    pub fn notify(&mut self, method: &str, params: &Value) -> Vec<String> {
        let request_id = params
            .get("requestId")
            .cloned()
            .and_then(|v| serde_json::from_value::<RequestId>(v).ok());
        let thread = params.get("threadId").and_then(Value::as_str);
        let turn = params.pointer("/turn/id").and_then(Value::as_str);
        let mut owners = Vec::new();
        self.pending.retain(|_, p| {
            let same_thread = thread == Some(p.view.thread_id.as_str());
            let remove = match method {
                "serverRequest/resolved" => same_thread && request_id.as_ref() == Some(&p.key.id),
                "turn/completed" => {
                    same_thread && turn.is_some() && turn == p.view.turn_id.as_deref()
                }
                "thread/closed" | "thread/archived" | "thread/deleted" => same_thread,
                _ => false,
            };
            if remove && !owners.contains(&p.view.owner) {
                owners.push(p.view.owner.clone());
            }
            !remove
        });
        owners
    }

    pub fn disconnect(&mut self) -> Vec<String> {
        let owners = self
            .pending
            .values()
            .map(|p| p.view.owner.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        self.pending.clear();
        owners
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Native request is missing {key}"))
}

fn response(kind: Kind, params: &Value, decision: Decision) -> Result<Value, String> {
    let value = decision_response(kind, params, decision)?;
    // Observed on the selected 0.153.4 wire and documented by OpenAI, although
    // absent from its generated request type. It can only restrict the existing
    // generated response union; unknown decisions never add client authority.
    if kind == Kind::Command
        && let Some(available) = params.get("availableDecisions").filter(|v| !v.is_null())
        && !available
            .as_array()
            .is_some_and(|choices| choices.contains(&value["decision"]))
    {
        return Err("Codex did not offer this decision for the pending request".into());
    }
    Ok(value)
}

fn approval_choices(kind: Kind, params: &Value) -> Option<ApprovalChoices> {
    if !matches!(kind, Kind::Command | Kind::Files) {
        return None;
    }
    let allowed = |decision| response(kind, params, decision).is_ok();
    Some(ApprovalChoices {
        accept: allowed(Decision::Accept {}),
        decline: allowed(Decision::Decline {}),
        cancel: allowed(Decision::Cancel {}),
        session: allowed(Decision::Session {}),
        exec_policy: allowed(Decision::ExecPolicy {}),
        network_policies: params["proposedNetworkPolicyAmendments"]
            .as_array()
            .map(|policies| {
                (0..policies.len())
                    .filter(|index| allowed(Decision::NetworkPolicy { index: *index }))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn decision_response(kind: Kind, params: &Value, decision: Decision) -> Result<Value, String> {
    match (kind, decision) {
        (Kind::Command | Kind::Files, Decision::Accept {}) => Ok(json!({"decision":"accept"})),
        (Kind::Command | Kind::Files, Decision::Session {}) => {
            Ok(json!({"decision":"acceptForSession"}))
        }
        (Kind::Command | Kind::Files, Decision::Decline {}) => Ok(json!({"decision":"decline"})),
        (Kind::Command | Kind::Files, Decision::Cancel {}) => Ok(json!({"decision":"cancel"})),
        (Kind::Command, Decision::ExecPolicy {}) => {
            let policy = params
                .get("proposedExecpolicyAmendment")
                .filter(|p| p.is_array())
                .ok_or("No command policy was proposed")?;
            Ok(
                json!({"decision":{"acceptWithExecpolicyAmendment":{"execpolicy_amendment":policy}}}),
            )
        }
        (Kind::Command, Decision::NetworkPolicy { index }) => {
            let policy = params
                .get("proposedNetworkPolicyAmendments")
                .and_then(Value::as_array)
                .and_then(|v| v.get(index))
                .ok_or("This network policy was not proposed")?;
            Ok(
                json!({"decision":{"applyNetworkPolicyAmendment":{"network_policy_amendment":policy}}}),
            )
        }
        (Kind::Permissions, Decision::Permissions { grant, session }) => {
            // The user may grant exactly the displayed request, never arbitrary
            // paths/network permissions supplied by presentation code.
            let mut permissions = serde_json::Map::new();
            if grant {
                for key in ["network", "fileSystem"] {
                    if let Some(value) = params["permissions"].get(key).filter(|p| p.is_object()) {
                        permissions.insert(key.into(), value.clone());
                    }
                }
            }
            Ok(
                json!({"permissions":permissions,"scope":if grant && session {"session"} else {"turn"}}),
            )
        }
        (Kind::Questions, Decision::Answers { answers }) => {
            let questions = params["questions"].as_array().ok_or("Missing questions")?;
            if answers.len() != questions.len() {
                return Err("Answer each requested question before continuing".into());
            }
            let mut result = serde_json::Map::new();
            for q in questions {
                let id = text(q, "id")?;
                let values = answers
                    .get(id)
                    .filter(|v| !v.is_empty() && v.iter().all(|a| !a.trim().is_empty()))
                    .ok_or("An answer is missing")?;
                // isOther is a presentation hint, not a ban on the native free-text answer.
                result.insert(id.into(), json!({"answers":values}));
            }
            Ok(json!({"answers":result}))
        }
        (Kind::Questions, Decision::Cancel {}) => Ok(json!({"answers":{}})),
        (Kind::Mcp, Decision::Elicitation { action, content }) => {
            let (action, content) = match action {
                ElicitationAction::Accept => {
                    if matches!(
                        params["mode"].as_str(),
                        Some("form" | "openai/form" | "openaiForm")
                    ) {
                        let content = content.ok_or("Complete the requested form")?;
                        validate_form(&params["requestedSchema"], &content)?;
                        ("accept", content)
                    } else {
                        ("accept", Value::Null)
                    }
                }
                ElicitationAction::Decline => ("decline", Value::Null),
                ElicitationAction::Cancel => ("cancel", Value::Null),
            };
            Ok(json!({"action":action,"content":content,"_meta":null}))
        }
        _ => Err("This decision is not valid for this native request".into()),
    }
}

const MAX_FORM_DEPTH: usize = 6;
const MAX_FORM_NODES: usize = 256;
const MAX_FORM_PROPERTIES: usize = 64;
const MAX_FORM_ITEMS: u64 = 100;
const MAX_FORM_STRING: u64 = 65_536;

/// Normalize the safe JSON-Schema subset rendered by Supervisor. Unknown
/// annotations, executable UI, references and composition are rejected rather
/// than copied into the WebView. Objects are always closed at every depth.
fn normalize_form_schema(schema: &Value) -> Result<Value, String> {
    if serde_json::to_vec(schema)
        .map_err(|_| "Invalid MCP form schema")?
        .len()
        > 128 * 1024
    {
        return Err("MCP form schema is too large".into());
    }
    let mut nodes = 0;
    normalize_schema_node(schema, 0, &mut nodes)
}

fn normalize_schema_node(schema: &Value, depth: usize, nodes: &mut usize) -> Result<Value, String> {
    if depth > MAX_FORM_DEPTH {
        return Err("MCP form schema is nested too deeply".into());
    }
    *nodes += 1;
    if *nodes > MAX_FORM_NODES {
        return Err("MCP form schema contains too many fields".into());
    }
    let raw = schema.as_object().ok_or("Unsupported MCP form schema")?;
    let kind = raw
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| inferred_choice_type(raw))
        .ok_or("Every MCP form field needs an explicit type")?;
    if !matches!(
        kind,
        "object" | "array" | "string" | "number" | "integer" | "boolean" | "null"
    ) {
        return Err("Unsupported MCP form field type".into());
    }
    let allowed = [
        "$schema",
        "type",
        "title",
        "description",
        "properties",
        "required",
        "additionalProperties",
        "items",
        "minItems",
        "maxItems",
        "uniqueItems",
        "enum",
        "oneOf",
        "anyOf",
        "const",
        "default",
        "minimum",
        "maximum",
        "minLength",
        "maxLength",
        "format",
        "enumNames",
    ];
    if raw.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err("MCP form uses unsupported JSON-Schema keywords".into());
    }
    let mut clean = serde_json::Map::new();
    clean.insert("type".into(), json!(kind));
    for key in ["title", "description"] {
        if let Some(value) = raw.get(key) {
            let limit = if key == "title" { 512 } else { 8 * 1024 };
            let value = value
                .as_str()
                .filter(|value| value.len() <= limit && !value.contains('\0'))
                .ok_or("MCP form text is invalid")?;
            clean.insert(key.into(), json!(value));
        }
    }
    match kind {
        "object" => {
            let properties = raw
                .get("properties")
                .and_then(Value::as_object)
                .ok_or("MCP object form needs properties")?;
            if properties.len() > MAX_FORM_PROPERTIES {
                return Err("MCP form has too many properties".into());
            }
            let mut normalized = serde_json::Map::new();
            for (key, value) in properties {
                if key.is_empty() || key.len() > 128 || key.contains('\0') {
                    return Err("MCP form property name is invalid".into());
                }
                normalized.insert(key.clone(), normalize_schema_node(value, depth + 1, nodes)?);
            }
            let required = raw
                .get("required")
                .map(|value| {
                    value
                        .as_array()
                        .filter(|values| values.len() <= properties.len())
                        .ok_or("MCP form required list is invalid")?
                        .iter()
                        .map(|value| {
                            value
                                .as_str()
                                .filter(|key| properties.contains_key(*key))
                                .map(str::to_owned)
                                .ok_or("MCP form requires an unknown property")
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default();
            clean.insert("properties".into(), Value::Object(normalized));
            clean.insert("required".into(), json!(required));
            clean.insert("additionalProperties".into(), json!(false));
        }
        "array" => {
            let items = raw
                .get("items")
                .ok_or("MCP array form needs an item schema")?;
            clean.insert(
                "items".into(),
                normalize_schema_node(items, depth + 1, nodes)?,
            );
            bounded_u64(raw, &mut clean, "minItems", MAX_FORM_ITEMS)?;
            bounded_u64(raw, &mut clean, "maxItems", MAX_FORM_ITEMS)?;
            if clean.get("minItems").and_then(Value::as_u64).unwrap_or(0)
                > clean
                    .get("maxItems")
                    .and_then(Value::as_u64)
                    .unwrap_or(MAX_FORM_ITEMS)
            {
                return Err("MCP array bounds are invalid".into());
            }
            if raw.get("uniqueItems") == Some(&Value::Bool(true))
                || items.get("enum").is_some()
                || items.get("oneOf").is_some()
                || items.get("anyOf").is_some()
            {
                clean.insert("uniqueItems".into(), json!(true));
            }
        }
        "string" => {
            bounded_u64(raw, &mut clean, "minLength", MAX_FORM_STRING)?;
            bounded_u64(raw, &mut clean, "maxLength", MAX_FORM_STRING)?;
            if clean.get("minLength").and_then(Value::as_u64).unwrap_or(0)
                > clean
                    .get("maxLength")
                    .and_then(Value::as_u64)
                    .unwrap_or(MAX_FORM_STRING)
            {
                return Err("MCP string bounds are invalid".into());
            }
            if let Some(format) = raw.get("format") {
                let format = format
                    .as_str()
                    .filter(|format| matches!(*format, "email" | "uri" | "date" | "date-time"))
                    .ok_or("Unsupported MCP string format")?;
                clean.insert("format".into(), json!(format));
            }
        }
        "number" | "integer" => {
            for key in ["minimum", "maximum"] {
                if let Some(value) = raw.get(key) {
                    let number = value
                        .as_f64()
                        .filter(|value| value.is_finite())
                        .ok_or("MCP numeric bound is invalid")?;
                    clean.insert(key.into(), json!(number));
                }
            }
            if clean
                .get("minimum")
                .and_then(Value::as_f64)
                .zip(clean.get("maximum").and_then(Value::as_f64))
                .is_some_and(|(min, max)| min > max)
            {
                return Err("MCP numeric bounds are invalid".into());
            }
        }
        _ => {}
    }
    for key in ["enum", "oneOf", "anyOf"] {
        if let Some(value) = raw.get(key) {
            clean.insert(key.into(), normalize_choices(key, value)?);
        }
    }
    if let Some(names) = raw.get("enumNames") {
        let names = names
            .as_array()
            .filter(|names| {
                names.len() <= 100
                    && names.iter().all(|name| {
                        name.as_str()
                            .is_some_and(|name| name.len() <= 512 && !name.contains('\0'))
                    })
            })
            .ok_or("MCP form enum labels are invalid")?;
        if clean
            .get("enum")
            .and_then(Value::as_array)
            .is_none_or(|values| values.len() != names.len())
        {
            return Err("MCP form enum labels do not match its values".into());
        }
        clean.insert("enumNames".into(), Value::Array(names.clone()));
    }
    if let Some(value) = raw.get("const") {
        if !scalar(value) {
            return Err("MCP form const value is invalid".into());
        }
        clean.insert("const".into(), value.clone());
    }
    if let Some(value) = raw.get("default") {
        if serde_json::to_vec(value)
            .map_err(|_| "Invalid MCP form default")?
            .len()
            > 64 * 1024
        {
            return Err("MCP form default is too large".into());
        }
        clean.insert("default".into(), value.clone());
    }
    let clean = Value::Object(clean);
    if let Some(default) = clean.get("default") {
        validate_node(&clean, default, 0)?;
    }
    Ok(clean)
}

fn bounded_u64(
    raw: &serde_json::Map<String, Value>,
    clean: &mut serde_json::Map<String, Value>,
    key: &str,
    maximum: u64,
) -> Result<(), String> {
    if let Some(value) = raw.get(key) {
        let value = value
            .as_u64()
            .filter(|value| *value <= maximum)
            .ok_or("MCP form bound is invalid")?;
        clean.insert(key.into(), json!(value));
    }
    Ok(())
}

fn normalize_choices(key: &str, value: &Value) -> Result<Value, String> {
    let values = value
        .as_array()
        .filter(|values| !values.is_empty() && values.len() <= 100)
        .ok_or("MCP form choices are invalid")?;
    if key == "enum" {
        if values.iter().any(|value| !scalar(value)) {
            return Err("MCP form enum value is invalid".into());
        }
        return Ok(value.clone());
    }
    let mut clean = Vec::new();
    for option in values {
        let object = option.as_object().ok_or("MCP form choice is invalid")?;
        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "const" | "title"))
        {
            return Err("MCP form choice contains unsupported fields".into());
        }
        let constant = object
            .get("const")
            .filter(|value| scalar(value))
            .ok_or("MCP form choice has no scalar value")?;
        let mut item = json!({"const":constant});
        if let Some(title) = object.get("title") {
            let title = title
                .as_str()
                .filter(|text| text.len() <= 512 && !text.contains('\0'))
                .ok_or("MCP form choice title is invalid")?;
            item["title"] = json!(title);
        }
        clean.push(item);
    }
    Ok(Value::Array(clean))
}

fn scalar(value: &Value) -> bool {
    value.is_null() || value.is_boolean() || value.is_number() || value.is_string()
}

fn inferred_choice_type(raw: &serde_json::Map<String, Value>) -> Option<&'static str> {
    let value = raw
        .get("enum")
        .or_else(|| raw.get("oneOf"))
        .or_else(|| raw.get("anyOf"))?
        .as_array()?
        .first()?;
    let value = value.get("const").unwrap_or(value);
    match value {
        Value::Null => Some("null"),
        Value::Bool(_) => Some("boolean"),
        Value::Number(number) if number.is_i64() || number.is_u64() => Some("integer"),
        Value::Number(_) => Some("number"),
        Value::String(_) => Some("string"),
        _ => None,
    }
}

/// Validate both the standard flat MCP form and the negotiated nested
/// openai/form subset. Native MCP remains authoritative for domain-specific
/// formats; this function enforces structure, bounds and exact field ownership.
fn validate_form(schema: &Value, value: &Value) -> Result<(), String> {
    let schema = normalize_form_schema(schema)?;
    validate_node(&schema, value, 0)
}

fn validate_node(schema: &Value, value: &Value, depth: usize) -> Result<(), String> {
    if depth > MAX_FORM_DEPTH
        || serde_json::to_vec(value)
            .map_err(|_| "Invalid MCP form value")?
            .len()
            > 128 * 1024
    {
        return Err("MCP form response exceeds client limits".into());
    }
    if !enum_contains(schema, value)
        || schema
            .get("const")
            .is_some_and(|constant| constant != value)
    {
        return Err("MCP form value is not an offered choice".into());
    }
    match schema["type"].as_str() {
        Some("object") => {
            let fields = schema["properties"]
                .as_object()
                .ok_or("Unsupported MCP object schema")?;
            let values = value
                .as_object()
                .ok_or("The MCP response must be an object")?;
            if schema["required"].as_array().is_some_and(|required| {
                required
                    .iter()
                    .any(|key| key.as_str().is_none_or(|key| !values.contains_key(key)))
            }) {
                return Err("Complete all required MCP fields".into());
            }
            for (key, value) in values {
                let field = fields
                    .get(key)
                    .ok_or("The response contains an unrequested MCP field")?;
                validate_node(field, value, depth + 1)
                    .map_err(|error| format!("MCP field {key}: {error}"))?;
            }
        }
        Some("array") => {
            let values = value.as_array().ok_or("MCP form value must be an array")?;
            let length = values.len() as u64;
            if schema["minItems"].as_u64().is_some_and(|min| length < min)
                || schema["maxItems"].as_u64().is_some_and(|max| length > max)
                || length > MAX_FORM_ITEMS
            {
                return Err("MCP form array length is invalid".into());
            }
            if schema["uniqueItems"] == true
                && values
                    .iter()
                    .enumerate()
                    .any(|(index, value)| values[..index].contains(value))
            {
                return Err("MCP form array contains duplicate values".into());
            }
            let item = schema.get("items").ok_or("MCP array has no item schema")?;
            for value in values {
                validate_node(item, value, depth + 1)?;
            }
        }
        Some("string") => {
            let text = value.as_str().ok_or("MCP form value must be text")?;
            let length = text.chars().count() as u64;
            if length > MAX_FORM_STRING
                || schema["minLength"].as_u64().is_some_and(|min| length < min)
                || schema["maxLength"].as_u64().is_some_and(|max| length > max)
                || text.contains('\0')
            {
                return Err("MCP text value is invalid".into());
            }
        }
        Some("number" | "integer") => {
            let number = value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or("MCP form value must be numeric")?;
            if schema["type"] == "integer" && number.fract() != 0.0
                || schema["minimum"].as_f64().is_some_and(|min| number < min)
                || schema["maximum"].as_f64().is_some_and(|max| number > max)
            {
                return Err("MCP numeric value is out of range".into());
            }
        }
        Some("boolean") if !value.is_boolean() => {
            return Err("MCP form value must be boolean".into());
        }
        Some("null") if !value.is_null() => return Err("MCP form value must be null".into()),
        Some("boolean" | "null") => {}
        _ => return Err("Unsupported MCP form schema".into()),
    }
    Ok(())
}
fn enum_contains(schema: &Value, value: &Value) -> bool {
    for key in ["oneOf", "anyOf"] {
        if let Some(options) = schema[key].as_array()
            && !options.iter().any(|o| o.get("const") == Some(value))
        {
            return false;
        }
    }
    schema["enum"]
        .as_array()
        .is_none_or(|options| options.contains(value))
}
