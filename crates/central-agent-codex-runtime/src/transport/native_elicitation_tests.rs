use super::*;
use crate::{
    api,
    requests::{Decision, ElicitationAction, Kind, Requests},
};
use serde_json::json;
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[test]
#[ignore = "Explicit native MCP elicitation probe: isolated profile/fixture, no authentication, inference or external URL opening"]
fn native_mcp_elicitation_round_trips_form_and_url_decisions_without_model_turns() {
    let profile = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/elicitation-mcp.mjs");
    let args = json!([fixture]).to_string();
    let cwd = json!(workspace.path()).to_string();
    std::fs::write(profile.path().join("config.toml"),format!(
        "[mcp_servers.elicitation_fixture]\ncommand = \"node\"\nargs = {args}\n[projects.{cwd}]\ntrust_level = \"trusted\"\n"
    )).unwrap();
    let runtime = Runtime::discover(workspace.path()).unwrap();
    let mut command = runtime.command();
    command
        .env("CODEX_HOME", profile.path())
        .env_remove("CODEX_SQLITE_HOME")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY");
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn_command(command, "native-elicitation-probe", move |event| {
        let _ = tx.send(event);
    })
    .unwrap();
    let next = || {
        rx.recv_timeout(Duration::from_secs(30))
            .expect("Native elicitation fixture did not respond")
    };
    loop {
        match next() {
            Event::Ready { .. } => break,
            Event::Closed { reason } => panic!("Native startup failed: {reason}"),
            Event::ServerRequest { .. } => panic!("Unexpected startup request"),
            _ => {}
        }
    }
    let execute = |call: api::Call| {
        call.send(&client)
            .unwrap()
            .reply
            .recv_timeout(Duration::from_secs(30))
            .unwrap()
            .unwrap()
    };
    let mut threads = Vec::new();
    for _ in 0..2 {
        let mut start = api::start_thread(
            workspace.path().to_str().unwrap(),
            &api::Profile::default(),
            api::Access::default(),
        );
        start.params["ephemeral"] = json!(true);
        let opened = execute(start);
        assert_eq!(opened["thread"]["ephemeral"], true);
        let thread = opened["thread"]["id"].as_str().unwrap().to_owned();
        loop {
            match next() {
                Event::Notification { method, params }
                    if method == "mcpServer/startupStatus/updated"
                        && params["threadId"] == thread
                        && params["name"] == "elicitation_fixture" =>
                {
                    if params["status"] == "ready" {
                        break;
                    }
                    assert_eq!(params["status"], "starting", "{params}");
                }
                Event::Notification { method, .. } => assert_ne!(method, "turn/started"),
                Event::Closed { .. } | Event::ServerRequest { .. } => {
                    panic!("Unexpected native startup state")
                }
                _ => {}
            }
        }
        let inventory = execute(api::thread_mcp_status(&thread, None));
        assert!(inventory["nextCursor"].is_null());
        assert_eq!(inventory["data"].as_array().unwrap().len(), 1);
        assert_eq!(inventory["data"][0]["name"], "elicitation_fixture");
        assert_eq!(inventory["data"][0]["runtimeStatus"], "connected");
        threads.push(thread);
    }
    assert_ne!(threads[0], threads[1]);
    let mut requests = Requests::default();
    for (i, (mode, action)) in [
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
        let thread = &threads[i % 2];
        let owner = if i % 2 == 0 {
            "chat:fixture"
        } else {
            "graph:fixture"
        };
        let other = if i % 2 == 0 {
            "graph:fixture"
        } else {
            "chat:fixture"
        };
        let ticket=client.request("mcpServer/tool/call",json!({"threadId":thread,"server":"elicitation_fixture","tool":"fixture_question","arguments":{"mode":mode}})).unwrap();
        let mut actual_key = None;
        let mut resolved = false;
        let mut reply = None;
        let deadline = Instant::now() + Duration::from_secs(30);
        while reply.is_none() || !resolved {
            assert!(
                Instant::now() < deadline,
                "Native elicitation did not finish: {mode}/{action}"
            );
            if reply.is_none() {
                match ticket.try_result() {
                    Ok(result) => reply = Some(result.unwrap()),
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(error) => panic!("Native tool reply lost: {error}"),
                }
            }
            if reply.is_some() && resolved {
                break;
            }
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Event::ServerRequest {
                    key,
                    method,
                    params,
                }) => {
                    assert!(actual_key.is_none(), "Unexpected extra request");
                    assert_eq!(method, "mcpServer/elicitation/request");
                    assert_eq!(params["threadId"], *thread);
                    assert!(
                        params["turnId"].is_null(),
                        "Standalone tool must not create a model turn"
                    );
                    assert_eq!(params["serverName"], "elicitation_fixture");
                    assert_eq!(params["mode"], mode);
                    requests
                        .insert(owner, key.clone(), &method, params)
                        .unwrap();
                    let view = requests.views(owner).pop().unwrap();
                    assert_eq!(view.kind, Kind::Mcp);
                    assert!(requests.views(other).is_empty());
                    let decision = || Decision::Elicitation {
                        action: match action {
                            "accept" => ElicitationAction::Accept,
                            "decline" => ElicitationAction::Decline,
                            _ => ElicitationAction::Cancel,
                        },
                        content: if action == "accept" && mode == "form" {
                            Some(json!({"label":"fixture","enabled":false,"count":3}))
                        } else {
                            None
                        },
                    };
                    assert!(requests.answer(other, &view.ticket, decision()).is_err());
                    let (key, response) = requests.answer(owner, &view.ticket, decision()).unwrap();
                    assert_eq!(response["action"], action);
                    if mode == "url" || action != "accept" {
                        assert!(response["content"].is_null());
                    }
                    assert!(requests.answer(owner, &view.ticket, decision()).is_err());
                    client.answer(&key, Ok(response)).unwrap();
                    actual_key = Some(key);
                }
                Ok(Event::Notification { method, params }) => {
                    assert_ne!(
                        method, "turn/started",
                        "No model turn may run in this probe"
                    );
                    if method == "serverRequest/resolved" {
                        assert_eq!(params["threadId"], *thread);
                        let id: RequestId =
                            serde_json::from_value(params["requestId"].clone()).unwrap();
                        assert_eq!(Some(&id), actual_key.as_ref().map(|key| &key.id));
                        assert_eq!(requests.notify(&method, &params), vec![owner.to_owned()]);
                        resolved = true;
                    }
                }
                Ok(Event::Closed { reason }) => panic!("Native client closed: {reason}"),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(error) => panic!("Native fixture closed: {error}"),
                _ => {}
            }
        }
        let reply = reply.unwrap();
        assert_eq!(reply["isError"], false, "{reply}");
        assert_eq!(reply["structuredContent"]["action"], action);
        let content = &reply["structuredContent"]["content"];
        if mode == "form" && action == "accept" {
            assert_eq!(
                *content,
                json!({"label":"fixture","enabled":false,"count":3})
            );
        } else if mode == "url" && action == "accept" {
            // 0.153.4 normalizes the null App Server answer to an empty MCP
            // content object for URL acceptance; no user data was supplied.
            assert_eq!(*content, json!({}));
        } else {
            assert!(
                content.is_null(),
                "Unexpected native {mode}/{action} content: {content}"
            );
        }
        assert!(requests.views(owner).is_empty());
        assert!(requests.views(other).is_empty());
        println!(
            "Native standalone MCP {mode}/{action}: exact owner, decision, server resolution and fixture result verified"
        );
    }
    client.shutdown();
    assert!(
        std::fs::read_dir(workspace.path())
            .unwrap()
            .next()
            .is_none()
    );
    println!(
        "Official Codex {}: six native elicitation round trips passed; isolated ephemeral threads, no model prompts, personal configuration, UI/visual tests or external URL opening",
        runtime.version()
    );
}
