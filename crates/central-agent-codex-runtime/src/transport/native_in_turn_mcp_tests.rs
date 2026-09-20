//! Actual App Server turn -> native MCP tool -> elicitation -> resumed turn.
//! Model transport is a fixed loopback fixture, not account inference. The MCP
//! server has no filesystem/network effects and the client never executes tools.
use super::{
    reload_responses_fixture::{self, Fixture},
    *,
};
use crate::{
    api,
    requests::{Decision, ElicitationAction, Requests},
};
use serde_json::json;
use std::{
    path::Path,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

use reload_responses_fixture::mcp_output as model_output;

#[test]
#[ignore = "Six actual native MCP turns against owned loopback model/server fixtures; no account inference or external effects"]
fn native_in_turn_mcp_elicitation_is_owned_resolved_and_persisted() -> Result<()> {
    run_native_mcp(false)
}

#[test]
#[ignore = "Explicit diagnostic requiring actual native MCP progress callbacks, isolated local fixtures only"]
fn native_in_turn_mcp_progress_reaches_display_mirror() -> Result<()> {
    run_native_mcp(true)
}

fn run_native_mcp(require_progress: bool) -> Result<()> {
    let mut fixture = Fixture::with_output(model_output)?;
    let root = tempfile::Builder::new()
        .prefix("central-native-in-turn-mcp-")
        .tempdir()?;
    let native = root.path().join("native");
    let work = root.path().join("workspace");
    std::fs::create_dir_all(&native)?;
    std::fs::create_dir_all(&work)?;
    let server = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/elicitation-mcp.mjs");
    std::fs::write(
        native.join("config.toml"),
        format!(
            "model = \"gpt-5.6-luna\"\nmodel_provider = \"mcp_fixture\"\n[model_providers.mcp_fixture]\nname = \"Owned MCP turn fixture\"\nbase_url = \"http://{}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = false\nsupports_websockets = false\nrequest_max_retries = 0\nstream_max_retries = 0\n[mcp_servers.elicitation_fixture]\ncommand = \"node\"\nargs = {}\n[projects.{}]\ntrust_level = \"trusted\"\n",
            fixture.address,
            json!([server]),
            json!(work)
        ),
    )?;
    let mut command = Runtime::discover(&work)?.command();
    command
        .env("CODEX_HOME", &native)
        .env_remove("CODEX_SQLITE_HOME")
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY");
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn_command(command, "in-turn-mcp-probe", move |e| {
        let _ = tx.send(e);
    })?;
    let result = (|| -> Result<()> {
        loop {
            match rx.recv_timeout(Duration::from_secs(30))? {
                Event::Ready { .. } => break,
                Event::Closed { reason } => return Err(reason.into()),
                Event::ServerRequest { .. } => return Err("Unexpected startup request".into()),
                _ => {}
            }
        }
        if !api::account_read().send(&client)?.wait()?["account"].is_null() {
            return Err("Fixture is authenticated".into());
        }
        let cwd = work.to_str().ok_or("Invalid cwd")?;
        let mut threads = Vec::new();
        for _ in 0..2 {
            let started = api::start_thread(cwd, &api::Profile::default(), api::Access::ReadOnly)
                .send(&client)?
                .wait()?;
            if !started["thread"]["path"]
                .as_str()
                .is_some_and(|p| Path::new(p).starts_with(&native))
            {
                return Err("Unowned native history".into());
            }
            let id = started["thread"]["id"]
                .as_str()
                .ok_or("Missing thread")?
                .to_owned();
            loop {
                match rx.recv_timeout(Duration::from_secs(30))? {
                    Event::Notification { method, params }
                        if method == "mcpServer/startupStatus/updated"
                            && params["threadId"] == id
                            && params["name"] == "elicitation_fixture" =>
                    {
                        if params["status"] == "ready" {
                            break;
                        }
                        if params["status"] != "starting" {
                            return Err("MCP fixture failed to start".into());
                        }
                    }
                    Event::Closed { reason } => return Err(reason.into()),
                    Event::ServerRequest { .. } => {
                        return Err("Unexpected MCP startup request".into());
                    }
                    _ => {}
                }
            }
            threads.push(id);
        }
        let mut requests = Requests::default();
        let mut mirror = crate::mirror::Mirror::default();
        for (case, (mode, action)) in [
            ("form", "accept"),
            ("form", "decline"),
            ("form", "cancel"),
            ("url", "accept"),
            ("url", "decline"),
            ("url", "cancel"),
        ]
        .into_iter()
        .enumerate()
        {
            let thread = &threads[case % 2];
            let owner = if case % 2 == 0 {
                "chat:fixture"
            } else {
                "graph:fixture"
            };
            let other = if case % 2 == 0 {
                "graph:fixture"
            } else {
                "chat:fixture"
            };
            let marker = if mode == "form" {
                "MCP_FIXTURE_FORM"
            } else {
                "MCP_FIXTURE_URL"
            };
            let ack = api::start_turn(
                thread,
                &format!("mcp-{case}"),
                vec![api::text_input(marker)],
                cwd,
                &api::Profile::default(),
                api::Access::ReadOnly,
            )
            .send(&client)?
            .wait()?;
            let turn = ack["turn"]["id"].as_str().ok_or("Missing turn")?;
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut key = None;
            let mut answered = Vec::new();
            let mut resolved = Vec::new();
            let mut permission_seen = false;
            let mut tool_result = None;
            let mut progress_messages = Vec::new();
            loop {
                match rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
                    Event::ServerRequest {
                        key: incoming,
                        method,
                        params,
                    } => {
                        let permission = params["mode"] == "form"
                            && params["message"]
                                == "Allow the elicitation_fixture MCP server to run tool \"fixture_question\"?"
                            && params["requestedSchema"]
                                == json!({"type":"object","properties":{}});
                        if method != "mcpServer/elicitation/request"
                            || params["threadId"] != *thread
                            || params["turnId"] != turn
                            || params["serverName"] != "elicitation_fixture"
                            || !permission && (params["mode"] != mode || key.is_some())
                            || permission && (permission_seen || key.is_some())
                        {
                            return Err("Unexpected request or wrong native turn ownership".into());
                        }
                        requests.insert(owner, incoming.clone(), &method, params)?;
                        let view = requests.views(owner).pop().ok_or("Missing request view")?;
                        if !requests.views(other).is_empty() {
                            return Err("MCP request leaked to sibling".into());
                        }
                        let decision = || Decision::Elicitation {
                            action: match if permission { "accept" } else { action } {
                                "accept" => ElicitationAction::Accept,
                                "decline" => ElicitationAction::Decline,
                                _ => ElicitationAction::Cancel,
                            },
                            content: if permission {
                                Some(json!({}))
                            } else if action == "accept" && mode == "form" {
                                Some(json!({"label":"fixture","enabled":false,"count":3}))
                            } else {
                                None
                            },
                        };
                        if requests.answer(other, &view.ticket, decision()).is_ok() {
                            return Err("Wrong owner answered MCP request".into());
                        }
                        let (actual, response) =
                            requests.answer(owner, &view.ticket, decision())?;
                        client.answer(&actual, Ok(response))?;
                        answered.push(incoming.clone());
                        if permission {
                            permission_seen = true;
                        } else {
                            key = Some(incoming);
                        }
                    }
                    Event::Notification { method, params } => {
                        mirror.notify(&method, &params)?;
                        if method == "item/mcpToolCall/progress" {
                            if params["threadId"] != *thread || params["turnId"] != turn {
                                return Err("MCP progress lost native owner".into());
                            }
                            progress_messages
                                .push((params["itemId"].clone(), params["message"].clone()));
                        }
                        if method == "serverRequest/resolved" {
                            if params["threadId"] != *thread
                                || !answered.iter().any(|k| json!(k.id) == params["requestId"])
                                || resolved.contains(&params["requestId"])
                            {
                                return Err("Wrong request resolved".into());
                            }
                            if requests.notify(&method, &params) != vec![owner.to_owned()] {
                                return Err("Request removal lost owner".into());
                            }
                            resolved.push(params["requestId"].clone());
                        }
                        if method == "item/completed" && params["item"]["type"] == "mcpToolCall" {
                            if params["threadId"] != *thread
                                || params["turnId"] != turn
                                || params["item"]["status"] != "completed"
                            {
                                return Err(
                                    format!("Native MCP item failed: {}", params["item"]).into()
                                );
                            }
                            tool_result = Some(params["item"].clone());
                        }
                        if method == "item/started"
                            && !matches!(
                                params["item"]["type"].as_str(),
                                Some("userMessage" | "agentMessage" | "reasoning" | "mcpToolCall")
                            )
                        {
                            return Err("Unexpected tool execution".into());
                        }
                        if method == "turn/completed"
                            && params["threadId"] == *thread
                            && params["turn"]["id"] == turn
                        {
                            if params["turn"]["status"] != "completed" {
                                return Err(format!(
                                    "MCP native turn failed: {}",
                                    params["turn"]["error"]
                                )
                                .into());
                            }
                            break;
                        }
                    }
                    Event::Closed { reason } => return Err(reason.into()),
                    _ => {}
                }
            }
            if resolved.len() != answered.len()
                || !permission_seen
                || key.is_none()
                || !requests.views(owner).is_empty()
            {
                return Err("Turn ended without MCP resolution".into());
            }
            let item = tool_result.ok_or("Missing completed MCP tool")?;
            let expected_progress = vec![
                (item["id"].clone(), json!("Fixture waiting for decision")),
                (item["id"].clone(), json!("Fixture received decision")),
            ];
            if require_progress && progress_messages != expected_progress {
                return Err(format!("Native progress mismatch: {progress_messages:?}; MCP call included progress token: {}", item["result"]["structuredContent"]["fixtureProgressTokenPresent"]).into());
            }
            let projected = mirror
                .thread(thread)
                .ok_or("Missing projected thread")?
                .turns
                .iter()
                .find(|t| t.id == turn)
                .ok_or("Missing projected turn")?
                .items
                .iter()
                .find(|i| i.id == item["id"])
                .ok_or("Missing projected MCP item")?;
            if require_progress
                && (!projected.completed
                    || projected.progress_messages
                        != ["Fixture waiting for decision", "Fixture received decision"])
            {
                return Err("Actual native progress was lost in the display mirror".into());
            }
            let content = if action != "accept" {
                Value::Null
            } else if mode == "url" {
                json!({})
            } else {
                json!({"label":"fixture","enabled":false,"count":3})
            };
            if item["server"] != "elicitation_fixture"
                || item["tool"] != "fixture_question"
                || item["result"]["structuredContent"]["action"] != action
                || item["result"]["structuredContent"]["content"] != content
            {
                return Err(format!("Wrong MCP result: {item}").into());
            }
            let history = api::read_thread(thread).send(&client)?.wait()?;
            let turns = history["thread"]["turns"]
                .as_array()
                .ok_or("Missing history")?;
            let last = turns.last().ok_or("Missing persisted turn")?;
            if turns.len() != case / 2 + 1
                || last["id"] != turn
                || !last["items"].as_array().is_some_and(|items| {
                    items.contains(&item)
                        && items
                            .iter()
                            .any(|i| i["type"] == "agentMessage" && i["text"] == "READY")
                })
            {
                return Err("Native history omitted tool result or continuation".into());
            }
            println!(
                "Native in-turn MCP {mode}/{action}: exact thread/turn, owner rejection, resolution, tool result and persisted continuation passed."
            );
        }
        if fixture
            .requests
            .lock()
            .map_err(|_| "Poisoned fixture")?
            .len()
            != 12
        {
            return Err("Unexpected model request count".into());
        }
        if std::fs::read_dir(&work)?.next().is_some() {
            return Err("MCP fixture unexpectedly changed workspace files".into());
        }
        Ok(())
    })();
    client.shutdown();
    let finished = fixture.finish();
    if let Err(error) = &finished {
        eprintln!("Local model fixture: {error}");
    }
    result?;
    finished?;
    println!(
        "Six native turns, twelve local responses; no account inference, personal config or external URL opening."
    );
    Ok(())
}

#[test]
fn local_mcp_model_fixture_only_emits_its_owned_tool_and_continuation() {
    let mut body = json!({"input":[{"type":"additional_tools","role":"developer","tools":[{"type":"namespace","name":"functions","tools":[{"type":"custom","name":"exec"}]}]},{"role":"user","content":"MCP_FIXTURE_FORM"}]});
    let call = model_output(&body, 1).unwrap();
    assert_eq!(call["type"], "custom_tool_call");
    assert_eq!(call["name"], "exec");
    assert_eq!(call["namespace"], "functions");
    let code = call["input"].as_str().unwrap();
    assert!(
        code.contains("elicitation_fixture")
            && code.contains("fixture_question")
            && code.contains("mode:\"form\"")
    );
    assert!(!code.contains("exec_command") && !code.contains("fetch("));
    body["input"][1]["content"] = json!("MCP_FIXTURE_URL");
    assert!(
        model_output(&body, 2).unwrap()["input"]
            .as_str()
            .unwrap()
            .contains("mode:\"url\"")
    );
    body["input"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type":"custom_tool_call_output","call_id":"mcp-call-2","output":"fixture"}));
    assert_eq!(
        model_output(&body, 3).unwrap()["content"][0]["text"],
        "READY"
    );
    body["input"].as_array_mut().unwrap().pop();
    body["input"][1]["content"] = json!("Unrelated user request");
    assert!(model_output(&body, 4).is_err());
    assert!(model_output(&json!({}), 5).is_err());
}
