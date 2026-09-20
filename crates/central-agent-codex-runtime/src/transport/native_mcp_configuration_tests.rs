use super::*;
use crate::{
    api,
    mcp_configuration::{self, Edit, OptionKey, Snapshot, Transport},
};
use serde_json::json;
use std::time::Duration;

#[test]
#[ignore = "Explicit native MCP config writes on a new isolated profile only; no model, server startup, login or personal config"]
fn native_mcp_configuration_preserves_other_values_and_rejects_stale_writes() {
    let profile = tempfile::Builder::new()
        .prefix("central-mcp-config-")
        .tempdir()
        .unwrap();
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(profile.path().join("config.toml"),
        "model_reasoning_summary = \"concise\"\n[mcp_servers.original]\ncommand = \"owned-fixture-never-launch\"\nenabled = false\n[mcp_servers.original.env]\nTOKEN = \"synthetic-test-secret\"\n"
    ).unwrap();
    let runtime = Runtime::discover(workspace.path()).unwrap();
    let mut command = runtime.command();
    command
        .env("CODEX_HOME", profile.path())
        .env_remove("CODEX_SQLITE_HOME")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY");
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn_command(command, "native-mcp-config-probe", move |event| {
        let _ = tx.send(event);
    })
    .unwrap();
    loop {
        match rx.recv_timeout(Duration::from_secs(30)).unwrap() {
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
            .unwrap()
    };
    let read = || execute(api::config_read(workspace.path().to_str().unwrap())).unwrap();
    let before = read();
    let initial = Snapshot::read(&before).unwrap();
    let target = initial.target.as_ref().unwrap();
    assert_eq!(
        std::fs::canonicalize(&target.file).unwrap(),
        profile.path().join("config.toml").canonicalize().unwrap()
    );
    assert_eq!(initial.servers.len(), 1);
    let add = |name: &str| Edit::Add {
        name: name.into(),
        transport: Transport::Stdio {
            command: "owned-fixture-never-launch".into(),
            args: vec!["--stdio".into()],
            cwd: None,
            env_vars: vec!["MCP_TOKEN".into()],
        },
    };
    let created = execute(initial.edit(&add("created")).unwrap()).unwrap();
    assert!(!mcp_configuration::saved(&created, target).unwrap());
    let raw = read();
    let after_add = Snapshot::read(&raw).unwrap();
    assert_eq!(after_add.servers.len(), 2);
    assert_eq!(raw["config"]["mcp_servers"]["created"]["enabled"], false);
    assert_eq!(
        raw["config"]["mcp_servers"]["original"],
        before["config"]["mcp_servers"]["original"]
    );
    assert_eq!(
        raw["config"]["model_reasoning_summary"],
        before["config"]["model_reasoning_summary"]
    );
    assert!(
        !serde_json::to_string(&after_add)
            .unwrap()
            .contains("synthetic-test-secret")
    );

    let stale = execute(initial.edit(&add("must_not_exist")).unwrap());
    assert!(
        matches!(stale, Err(CallError::Rpc(_))),
        "Stale configuration must be rejected by native RPC"
    );
    let after_stale = read();
    assert!(after_stale["config"]["mcp_servers"]["must_not_exist"].is_null());
    assert_eq!(after_stale["config"], raw["config"]);
    println!(
        "Native MCP config: disabled creation, private-value preservation and stale-version rejection verified."
    );

    for enabled in [true, false] {
        let raw = read();
        let snapshot = Snapshot::read(&raw).unwrap();
        let ack = execute(
            snapshot
                .edit(&Edit::SetEnabled {
                    name: "created".into(),
                    enabled,
                })
                .unwrap(),
        )
        .unwrap();
        mcp_configuration::saved(&ack, snapshot.target.as_ref().unwrap()).unwrap();
        let current = read();
        assert_eq!(
            current["config"]["mcp_servers"]["created"]["enabled"],
            enabled
        );
        assert_eq!(
            current["config"]["mcp_servers"]["created"]["command"],
            raw["config"]["mcp_servers"]["created"]["command"]
        );
        assert_eq!(
            current["config"]["mcp_servers"]["original"],
            before["config"]["mcp_servers"]["original"]
        );
    }
    for (key, value) in [
        (
            OptionKey::Command,
            json!("owned-fixture-changed-never-launch"),
        ),
        (OptionKey::Args, json!(["--changed", "with spaces"])),
        (OptionKey::Cwd, json!(workspace.path())),
        (OptionKey::EnvVars, json!(["CHANGED_TOKEN"])),
        (OptionKey::StartupTimeoutSec, json!(12.5)),
        (OptionKey::ToolTimeoutSec, json!(123.0)),
        (OptionKey::Required, json!(true)),
        (OptionKey::EnabledTools, json!([])),
        (OptionKey::DisabledTools, json!(["fixture-write"])),
    ] {
        let snapshot = Snapshot::read(&read()).unwrap();
        let ack = execute(
            snapshot
                .edit(&Edit::SetOption {
                    name: "created".into(),
                    key,
                    value: value.clone(),
                })
                .unwrap(),
        )
        .unwrap();
        mcp_configuration::saved(&ack, snapshot.target.as_ref().unwrap()).unwrap();
        let raw = read();
        let user = raw["layers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["name"]["type"] == "user" && l["name"]["profile"].is_null())
            .unwrap();
        let key_name = serde_json::to_value(key).unwrap();
        assert_eq!(
            user["config"]["mcp_servers"]["created"][key_name.as_str().unwrap()],
            value,
            "{key:?}"
        );
        assert_eq!(
            raw["config"]["mcp_servers"]["original"],
            before["config"]["mcp_servers"]["original"]
        );
        if key != OptionKey::Command {
            let snapshot = Snapshot::read(&raw).unwrap();
            execute(
                snapshot
                    .edit(&Edit::ClearOption {
                        name: "created".into(),
                        key,
                    })
                    .unwrap(),
            )
            .unwrap();
            let after = Snapshot::read(&read()).unwrap();
            assert!(
                !after
                    .servers
                    .iter()
                    .find(|s| s.name == "created")
                    .unwrap()
                    .saved_options
                    .contains(&key)
            );
        }
    }
    let snapshot = Snapshot::read(&read()).unwrap();
    execute(
        snapshot
            .edit(&Edit::Remove {
                name: "created".into(),
            })
            .unwrap(),
    )
    .unwrap();
    let snapshot = Snapshot::read(&read()).unwrap();
    let http = Edit::Add {
        name: "http_fixture".into(),
        transport: Transport::Http {
            url: "https://example.invalid/mcp".into(),
            bearer_token_env_var: Some("MCP_TEST_TOKEN".into()),
        },
    };
    execute(snapshot.edit(&http).unwrap()).unwrap();
    let raw = read();
    assert_eq!(
        raw["config"]["mcp_servers"]["http_fixture"]["enabled"],
        false
    );
    assert_eq!(
        raw["config"]["mcp_servers"]["http_fixture"]["bearer_token_env_var"],
        "MCP_TEST_TOKEN"
    );
    for (key, value) in [
        (OptionKey::Url, json!("https://changed.invalid/mcp")),
        (OptionKey::BearerTokenEnvVar, json!("NEW_TOKEN")),
        (OptionKey::EnvHttpHeaders, json!({"X-Old":"OLD_TOKEN"})),
        (OptionKey::EnvHttpHeaders, json!({"X-New":"NEW_TOKEN"})),
    ] {
        let snapshot = Snapshot::read(&read()).unwrap();
        execute(
            snapshot
                .edit(&Edit::SetOption {
                    name: "http_fixture".into(),
                    key,
                    value: value.clone(),
                })
                .unwrap(),
        )
        .unwrap();
        let current = read();
        let key_name = serde_json::to_value(key).unwrap();
        assert_eq!(
            current["config"]["mcp_servers"]["http_fixture"][key_name.as_str().unwrap()],
            value
        );
        assert_eq!(
            current["config"]["mcp_servers"]["http_fixture"]["enabled"],
            false
        );
        assert_eq!(
            current["config"]["mcp_servers"]["original"],
            before["config"]["mcp_servers"]["original"]
        );
    }
    for key in [OptionKey::BearerTokenEnvVar, OptionKey::EnvHttpHeaders] {
        let snapshot = Snapshot::read(&read()).unwrap();
        execute(
            snapshot
                .edit(&Edit::ClearOption {
                    name: "http_fixture".into(),
                    key,
                })
                .unwrap(),
        )
        .unwrap();
        assert!(
            !Snapshot::read(&read())
                .unwrap()
                .servers
                .iter()
                .find(|s| s.name == "http_fixture")
                .unwrap()
                .saved_options
                .contains(&key)
        );
    }
    let snapshot = Snapshot::read(&read()).unwrap();
    execute(
        snapshot
            .edit(&Edit::Remove {
                name: "http_fixture".into(),
            })
            .unwrap(),
    )
    .unwrap();
    let final_read = read();
    assert_eq!(final_read["config"], before["config"]);
    let public = Snapshot::read(&final_read).unwrap();
    assert_eq!(public.servers.len(), 1);
    assert!(
        std::fs::read_dir(workspace.path())
            .unwrap()
            .next()
            .is_none()
    );
    for event in rx.try_iter() {
        match event {
            Event::Notification { method, .. } => assert!(!matches!(
                method.as_str(),
                "turn/started" | "thread/started" | "mcpServer/startupStatus/updated"
            )),
            Event::ServerRequest { .. } | Event::Closed { .. } => {
                panic!("Unexpected model/server work during config-only test")
            }
            _ => {}
        }
    }
    client.shutdown();
    // All files belong to newly created fixture roots. No user profile is removed.
    workspace.close().unwrap();
    profile.close().unwrap();
    println!(
        "Native MCP config: enable/disable, 12 typed option setters, exact option clearing and map replacement, server removal and original config restoration verified; isolated profiles removed, no inference or MCP startup."
    );
}
