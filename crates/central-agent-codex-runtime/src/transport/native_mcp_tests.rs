use super::*;
use crate::api;
use serde_json::json;
use std::{path::Path, time::Duration};

#[test]
#[ignore = "Explicit real runtime probe; isolated child profile and local fixture MCP only, no login or inference"]
fn native_thread_inventory_reports_only_fixture_tools_without_calling_them() {
    let profile = tempfile::tempdir().unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let trace = profile.path().join("fixture-methods.txt");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/inventory-mcp.mjs");
    // Only this temporary child profile is written. JSON quoting is compatible
    // with TOML basic strings and preserves Windows path separators.
    let args =
        serde_json::to_string(&[fixture.to_str().unwrap(), trace.to_str().unwrap()]).unwrap();
    let cwd = serde_json::to_string(workspace.path().to_str().unwrap()).unwrap();
    std::fs::write(profile.path().join("config.toml"), format!(
        "[mcp_servers.inventory_fixture]\ncommand = \"node\"\nargs = {args}\n[projects.{cwd}]\ntrust_level = \"trusted\"\n"
    )).unwrap();
    let runtime = Runtime::discover(workspace.path()).unwrap();
    let mut command = runtime.command();
    command
        .env("CODEX_HOME", profile.path())
        .env_remove("CODEX_SQLITE_HOME")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY");
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn_command(command, "isolated-thread-mcp-probe", move |event| {
        let _ = tx.send(event);
    })
    .unwrap();
    let next = || {
        rx.recv_timeout(Duration::from_secs(30))
            .expect("Native test fixture did not respond")
    };
    loop {
        match next() {
            Event::Ready { .. } => break,
            Event::Closed { .. } | Event::ServerRequest { .. } => {
                panic!("Unexpected native startup state")
            }
            _ => {}
        }
    }
    let execute = |call: api::Call| {
        call.send(&client)
            .unwrap()
            .reply
            .recv_timeout(Duration::from_secs(30))
            .expect("Native fixture RPC did not respond")
            .unwrap()
    };
    let mut start = api::start_thread(
        workspace.path().to_str().unwrap(),
        &api::Profile::default(),
        api::Access::default(),
    );
    start.params["ephemeral"] = json!(true);
    let opened = execute(start);
    let thread = opened["thread"]["id"].as_str().unwrap();
    assert_eq!(opened["thread"]["ephemeral"], true);
    loop {
        match next() {
            Event::Notification { method, params }
                if method == "mcpServer/startupStatus/updated"
                    && params["threadId"] == thread
                    && params["name"] == "inventory_fixture" =>
            {
                if params["status"] == "ready" {
                    break;
                }
                assert_eq!(
                    params["status"], "starting",
                    "Fixture MCP did not become ready: {params}"
                );
            }
            Event::Closed { .. } | Event::ServerRequest { .. } => {
                panic!("Unexpected native MCP startup state")
            }
            Event::Notification { method, .. } => assert_ne!(method, "turn/started"),
            _ => {}
        }
    }
    let inventory = execute(api::thread_mcp_status(thread, None));
    assert!(inventory["nextCursor"].is_null());
    let servers = inventory["data"].as_array().unwrap();
    assert_eq!(
        servers.len(),
        1,
        "No personal MCP configuration may enter the fixture"
    );
    assert_eq!(servers[0]["name"], "inventory_fixture");
    assert_eq!(servers[0]["runtimeStatus"], "connected");
    assert_eq!(servers[0]["tools"].as_object().unwrap().len(), 1);
    assert_eq!(servers[0]["tools"]["fixture_echo"]["name"], "fixture_echo");
    assert_eq!(servers[0]["resources"][0]["name"], "Fixture overview");
    assert!(
        servers[0]["resourceTemplates"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let foreign = api::thread_mcp_status("00000000-0000-0000-0000-000000000000", None)
        .send(&client)
        .unwrap()
        .wait();
    assert!(
        foreign.is_err(),
        "Unknown thread must not fall back to global MCP inventory"
    );
    client.shutdown();
    let methods = std::fs::read_to_string(&trace).unwrap();
    for required in [
        "initialize",
        "tools/list",
        "resources/list",
        "resources/templates/list",
    ] {
        assert!(methods.lines().any(|method| method == required));
    }
    assert!(
        !methods
            .lines()
            .any(|method| matches!(method, "tools/call" | "resources/read"))
    );
    println!(
        "Official Codex {}: ephemeral thread MCP inventory and unknown-thread rejection passed. Only isolated fixture metadata was requested; no tool call, resource read, login, model turn or personal configuration access.",
        runtime.version()
    );
}
