use super::*;
use crate::api;
use serde_json::json;
use std::{path::Path, time::Duration};

struct Fixture(Child);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore = "Explicit native OAuth probe: local fixture, ephemeral thread, isolated file credential store; no external login/inference"]
fn native_thread_oauth_completes_in_exact_scope_without_external_login() {
    for scoped in [true, false] {
        let profile = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let trace = profile.path().join("oauth-fixture-methods.txt");
        let mut fixture_command = Command::new("node");
        crate::runtime::hide_window(&mut fixture_command);
        fixture_command
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oauth-mcp.mjs"))
            .arg(&trace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut fixture = Fixture(fixture_command.spawn().unwrap());
        let mut fixture_output = BufReader::new(fixture.0.stdout.take().unwrap());
        let mut origin = String::new();
        fixture_output.read_line(&mut origin).unwrap();
        let origin = origin.trim();
        assert!(origin.starts_with("http://127.0.0.1:"));
        let cwd = serde_json::to_string(workspace.path().to_str().unwrap()).unwrap();
        // Force file-only credentials inside this fresh child profile, never keyring.
        std::fs::write(profile.path().join("config.toml"), format!(
        "mcp_oauth_credentials_store = \"file\"\n[mcp_servers.oauth_fixture]\nurl = \"{origin}/mcp\"\n[projects.{cwd}]\ntrust_level = \"trusted\"\n"
    )).unwrap();
        let runtime = Runtime::discover(workspace.path()).unwrap();
        let mut command = runtime.command();
        command
            .env("CODEX_HOME", profile.path())
            .env_remove("CODEX_SQLITE_HOME")
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_API_KEY");
        let (tx, rx) = mpsc::channel();
        let client = Client::spawn_command(command, "isolated-thread-oauth-probe", move |event| {
            let _ = tx.send(event);
        })
        .unwrap();
        let next = || {
            rx.recv_timeout(Duration::from_secs(30))
                .expect("Native OAuth fixture did not respond")
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
        let inventory_call = || {
            if scoped {
                api::thread_mcp_status(thread, None)
            } else {
                api::mcp_status(None)
            }
        };
        let inventory = execute(inventory_call());
        assert_eq!(inventory["data"].as_array().unwrap().len(), 1);
        assert_eq!(inventory["data"][0]["name"], "oauth_fixture");
        assert_eq!(inventory["data"][0]["authStatus"], "notLoggedIn");
        let response = execute(if scoped {
            api::thread_mcp_login(thread, "oauth_fixture")
        } else {
            api::mcp_login("oauth_fixture")
        });
        let authorization = response["authorizationUrl"].as_str().unwrap();
        assert!(authorization.starts_with(&format!("{origin}/authorize?")));
        // Simulate consent only against our owned loopback fixture; no browser/UI.
        writeln!(
            fixture.0.stdin.as_mut().unwrap(),
            "{}",
            json!({"authorize":authorization})
        )
        .unwrap();
        let mut result = String::new();
        fixture_output.read_line(&mut result).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&result).unwrap()["authorized"],
            true
        );
        loop {
            match next() {
                Event::Notification { method, params }
                    if method == "mcpServer/oauthLogin/completed" =>
                {
                    assert_eq!(
                        params["threadId"],
                        if scoped { json!(thread) } else { Value::Null }
                    );
                    assert_eq!(params["name"], "oauth_fixture");
                    assert_eq!(
                        params["success"], true,
                        "Native fixture OAuth failed: {params}"
                    );
                    break;
                }
                Event::Notification { method, .. } => assert_ne!(method, "turn/started"),
                Event::Closed { .. } | Event::ServerRequest { .. } => {
                    panic!("Unexpected native OAuth state")
                }
                _ => {}
            }
        }
        let after = execute(inventory_call());
        assert_eq!(after["data"][0]["authStatus"], "oAuth");
        client.shutdown();
        let methods = std::fs::read_to_string(trace).unwrap();
        assert!(methods.lines().any(|line| line == "POST /token"));
        assert!(
            !methods
                .lines()
                .any(|line| matches!(line, "MCP tools/call" | "MCP resources/read"))
        );
        println!(
            "Official Codex {}: {} native OAuth discovery, authorization URL, PKCE exchange, completion event and auth inventory passed. Only synthetic loopback credentials in an isolated file store; no external account, model prompt or tool execution.",
            runtime.version(),
            if scoped { "thread-scoped" } else { "global" }
        );
    }
}
