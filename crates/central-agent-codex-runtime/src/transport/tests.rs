use super::*;
use serde_json::json;
use std::{path::Path, time::Duration};

fn fixture(mode: &str) -> (Client, mpsc::Receiver<Event>) {
    let (tx, rx) = mpsc::channel();
    let mut command = Command::new("node");
    crate::runtime::hide_window(&mut command);
    command
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fake-server.mjs"))
        .arg(mode);
    let client = Client::spawn_command(command, "test", move |e| {
        let _ = tx.send(e);
    })
    .unwrap();
    (client, rx)
}
fn event(events: &mpsc::Receiver<Event>) -> Event {
    events
        .recv_timeout(Duration::from_secs(10))
        .expect("fixture did not respond")
}

#[cfg(windows)]
#[test]
fn hidden_console_transport_preserves_bidirectional_jsonl() {
    let (tx, rx) = mpsc::channel();
    let node = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|directory| directory.join("node.exe"))
        .find(|candidate| candidate.is_file())
        .expect("Node is required by the deterministic transport fixture");
    let mut command = Command::new(node);
    command
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fake-server.mjs"))
        .arg("normal");
    let client = Client::spawn_command_with_console(
        command,
        "hidden-test",
        move |event| {
            let _ = tx.send(event);
        },
        true,
    )
    .unwrap();
    assert!(matches!(event(&rx), Event::Ready { .. }));
    let reply = client
        .request("fixture/echo", json!({"hidden":true}))
        .unwrap()
        .wait()
        .unwrap();
    assert_eq!(reply, json!({"hidden":true}));
    client.shutdown();
    assert_eq!(client.process_id(), None);
}

#[test]
fn item_listing_is_blocked_before_transport_even_with_an_untyped_request() {
    let (client, events) = fixture("normal");
    while !matches!(event(&events), Event::Ready { .. }) {}
    assert!(matches!(
        client.request(
            "thread/items/list",
            serde_json::json!({"threadId":"fixture"})
        ),
        Err(CallError::Rejected(_))
    ));
    assert!(matches!(
        crate::api::list_thread_items("fixture", None, None).send(&client),
        Err(CallError::Rejected(_))
    ));
    client.shutdown();
}

#[test]
#[ignore = "Explicit real Codex probe; writes only its isolated temporary config, no account or inference"]
fn native_config_null_removes_only_the_selected_fixture_override() {
    use crate::{
        api,
        configuration::{Snapshot, WriteOutcome},
        runtime::Runtime,
    };
    let profile = tempfile::tempdir().unwrap();
    let config_file = profile.path().join("config.toml");
    std::fs::write(
        &config_file,
        "model_verbosity = \"high\"\nmodel_reasoning_effort = \"medium\"\n",
    )
    .unwrap();
    let runtime = Runtime::discover(profile.path()).unwrap();
    let mut command = runtime.command();
    // Child-only native profile isolation, never change the parent environment
    // or point a write at an existing user's configuration/history directory.
    command
        .env("CODEX_HOME", profile.path())
        .env_remove("CODEX_SQLITE_HOME");
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn_command(command, "isolated-config-probe", move |event| {
        let _ = tx.send(event);
    })
    .unwrap();
    let execute = |call: api::Call| {
        call.send(&client)
            .unwrap()
            .reply
            .recv_timeout(Duration::from_secs(20))
            .expect("Native fixture RPC did not respond")
            .unwrap()
    };
    loop {
        match event(&rx) {
            Event::Ready { .. } => break,
            Event::Closed { .. } | Event::ServerRequest { .. } => {
                panic!("Unexpected native fixture startup state")
            }
            _ => {}
        }
    }
    let before =
        Snapshot::read(&execute(api::config_read(profile.path().to_str().unwrap()))).unwrap();
    let target = before
        .target
        .as_ref()
        .expect("Native user layer is required");
    assert_eq!(
        Path::new(&target.file).canonicalize().unwrap(),
        config_file.canonicalize().unwrap()
    );
    assert_eq!(
        before
            .preferences
            .iter()
            .find(|p| p.key == "model_verbosity")
            .unwrap()
            .user_value
            .as_deref(),
        Some("high")
    );
    let response = execute(before.clear("model_verbosity").unwrap());
    WriteOutcome::read(&response, target, "model_verbosity").unwrap();
    let after =
        Snapshot::read(&execute(api::config_read(profile.path().to_str().unwrap()))).unwrap();
    assert_eq!(
        after
            .preferences
            .iter()
            .find(|p| p.key == "model_verbosity")
            .unwrap()
            .user_value,
        None
    );
    assert_eq!(
        after
            .preferences
            .iter()
            .find(|p| p.key == "model_reasoning_effort")
            .unwrap()
            .user_value
            .as_deref(),
        Some("medium")
    );
    assert!(
        !std::fs::read_to_string(&config_file)
            .unwrap()
            .contains("model_verbosity")
    );
    client.shutdown();
    println!(
        "Codex {} confirmed null removes only the selected temporary override; no personal config, account or inference used.",
        runtime.version()
    );
}

#[test]
fn diagnostic_pid_belongs_to_owned_child_and_disappears_after_shutdown_or_exit() {
    let (client, events) = fixture("normal");
    assert!(matches!(event(&events), Event::Ready { .. }));
    let id = client.process_id().unwrap();
    assert_ne!(id, std::process::id());
    assert_eq!(client.clone().process_id(), Some(id));
    client.shutdown();
    assert_eq!(client.process_id(), None);

    let (exiting, events) = fixture("normal");
    assert!(matches!(event(&events), Event::Ready { .. }));
    let pending = exiting.request("fixture/exit", json!({})).unwrap();
    assert!(matches!(
        pending.wait(),
        Err(CallError::Disconnected { .. })
    ));
    // Native pipe EOF may precede the OS signalled-process state very briefly.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while exiting.process_id().is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(exiting.process_id(), None);
    exiting.shutdown();
}

#[test]
fn real_pipes_handle_chunking_stderr_and_bidirectional_requests() {
    let (client, events) = fixture("normal");
    assert!(matches!(event(&events), Event::Ready { .. }));
    assert!(client.ready());
    let ticket = client
        .request("fixture/requestApproval", json!({"threadId":"a"}))
        .unwrap();
    let (key, params) = match event(&events) {
        Event::ServerRequest { key, params, .. } => (key, params),
        e => panic!("{e:?}"),
    };
    assert_eq!(params["threadId"], "a");
    client
        .answer(&key, Ok(json!({"decision":"decline"})))
        .unwrap();
    assert_eq!(ticket.wait().unwrap()["decision"], "decline");
    assert!(
        client
            .answer(&key, Ok(json!({"decision":"accept"})))
            .is_err()
    );
    client.shutdown();
}

#[test]
fn concurrent_replies_are_correlated_not_assigned_to_current_selection() {
    let (client, events) = fixture("normal");
    assert!(matches!(event(&events), Event::Ready { .. }));
    let a = client
        .request("fixture/hold", json!({"owner":"chat:a"}))
        .unwrap();
    let b = client
        .request("fixture/reverse", json!({"owner":"graph:b"}))
        .unwrap();
    assert_eq!(b.wait().unwrap()["owner"], "graph:b");
    assert_eq!(a.wait().unwrap()["owner"], "chat:a");
}

#[test]
fn unknown_notifications_remain_observable_without_becoming_actions() {
    let (client, events) = fixture("normal");
    assert!(matches!(event(&events), Event::Ready { .. }));
    let mut conversations = crate::conversations::Conversations::default();
    let before = serde_json::to_value(conversations.saved()).unwrap();
    let reply = client
        .request("fixture/unknownNotification", json!({}))
        .unwrap();
    let Event::Notification { method, params } = event(&events) else {
        panic!("An unknown notification must reach the host, not become a request");
    };
    assert_eq!(method, "future/notification");
    assert!(!conversations.notification(&method, &params).unwrap());
    assert_eq!(serde_json::to_value(conversations.saved()).unwrap(), before);
    assert!(!conversations.any_busy());
    assert_eq!(reply.wait().unwrap(), json!({"ok":true}));
    assert!(client.ready());
    let next = client
        .request("fixture/echo", json!({"stillConnected":true}))
        .unwrap();
    assert_eq!(next.wait().unwrap(), json!({"stillConnected":true}));
    client.shutdown();
}

#[test]
fn exit_makes_unacknowledged_turn_ambiguous_and_never_replays() {
    let (client, events) = fixture("normal");
    assert!(matches!(event(&events), Event::Ready { .. }));
    let ticket = client
        .request("fixture/exit", json!({"input":"do not replay"}))
        .unwrap();
    assert!(matches!(
        ticket.wait(),
        Err(CallError::Disconnected {
            delivery_unknown: true,
            ..
        })
    ));
    assert!(matches!(event(&events), Event::Closed { .. }));
    assert!(!client.ready());
    assert!(client.request("fixture/replay", json!({})).is_err());
}

#[test]
fn failed_initialize_never_announces_ready() {
    let (client, events) = fixture("reject-initialize");
    assert!(matches!(event(&events), Event::Closed { .. }));
    assert!(!client.ready());
    assert!(client.request("model/list", json!({})).is_err());
}

#[test]
fn steering_real_pipes_preserve_two_turns_and_stream_input_before_acceptance() {
    use crate::{
        api::{self, Access, Profile},
        conversations::{Conversations, Outcome},
    };
    let cwd = tempfile::tempdir().unwrap();
    let (client, events) = fixture("steering");
    assert!(matches!(event(&events), Event::Ready { .. }));
    let mut conversations = Conversations::default();
    for owner in ["chat:main", "graph:node"] {
        let open = conversations
            .open(owner, cwd.path(), &Profile::default(), Access::ReadOnly)
            .unwrap();
        let result = open.call.clone().send(&client).unwrap().wait();
        conversations.complete(&open, result).unwrap();
        let start = conversations
            .start_turn(
                owner,
                "first",
                vec![api::text_input("Start")],
                cwd.path(),
                &Profile::default(),
                Access::ReadOnly,
            )
            .unwrap();
        let reply = start.call.clone().send(&client).unwrap();
        let Event::Notification { method, params } = event(&events) else {
            panic!("Expected native turn/started")
        };
        conversations.notification(&method, &params).unwrap();
        conversations.complete(&start, reply.wait()).unwrap();
    }
    let mut replies = vec![];
    for (owner, turn) in [
        ("chat:main", "turn-native-1"),
        ("graph:node", "turn-native-2"),
    ] {
        let request = conversations
            .steer(owner, turn, "followup", vec![api::text_input(owner)])
            .unwrap();
        let ticket = request.call.clone().send(&client).unwrap();
        replies.push((request, ticket));
    }
    for _ in 0..2 {
        let Event::Notification { method, params } = event(&events) else {
            panic!("Expected native item")
        };
        assert_eq!(method, "item/completed"); // Steering creates no extra turn/started.
        conversations.notification(&method, &params).unwrap();
    }
    for (request, ticket) in replies.into_iter().rev() {
        assert!(conversations.saved().unresolved[&request.owner].steer);
        let outcome = conversations.complete(&request, ticket.wait()).unwrap();
        assert!(matches!(outcome,Outcome::Accepted {owner,..} if owner==request.owner));
        assert!(conversations.steer_target(&request.owner).is_some());
    }
    for (thread_id, owner) in [("native-1", "chat:main"), ("native-2", "graph:node")] {
        let thread = conversations.mirror.thread(thread_id).unwrap();
        assert_eq!(thread.turns.len(), 1);
        assert_eq!(thread.turns[0].items[0].value["content"][0]["text"], owner);
        assert_eq!(thread.turns[0].items[0].value["clientId"], "followup");
    }
    client.shutdown();
}

#[test]
fn native_thread_turn_flow_streams_two_owners_without_transcript_reconstruction() {
    use crate::{
        api::{self, Access, Profile},
        conversations::{Conversations, Outcome},
    };
    let cwd = tempfile::tempdir().unwrap();
    let (client, events) = fixture("conversations");
    assert!(matches!(event(&events), Event::Ready { .. }));
    let mut conversations = Conversations::default();
    for (owner, id) in [("chat:main", "native-1"), ("graph:node", "native-2")] {
        let request = conversations
            .open(owner, cwd.path(), &Profile::default(), Access::ReadOnly)
            .unwrap();
        let result = request.call.clone().send(&client).unwrap().wait();
        assert_eq!(
            conversations.complete(&request, result).unwrap(),
            Outcome::Opened {
                owner: owner.into(),
                thread_id: id.into()
            }
        );
    }
    let a = conversations
        .start_turn(
            "chat:main",
            "message-a",
            vec![api::text_input("fixture A")],
            cwd.path(),
            &Profile::default(),
            Access::ReadOnly,
        )
        .unwrap();
    let b = conversations
        .start_turn(
            "graph:node",
            "message-b",
            vec![api::text_input("fixture B")],
            cwd.path(),
            &Profile::default(),
            Access::ReadOnly,
        )
        .unwrap();
    let a_reply = a.call.clone().send(&client).unwrap();
    let b_reply = b.call.clone().send(&client).unwrap();
    let mut completed = 0;
    while completed < 2 {
        match event(&events) {
            Event::Notification { method, params } => {
                if method == "turn/completed" {
                    completed += 1;
                }
                conversations.notification(&method, &params).unwrap();
            }
            other => panic!("Unexpected fixture event: {other:?}"),
        }
    }
    for (request, reply) in [(&b, b_reply), (&a, a_reply)] {
        assert!(matches!(
            conversations.complete(request, reply.wait()).unwrap(),
            Outcome::Accepted { .. }
        ));
    }
    assert!(!conversations.busy("chat:main"));
    assert!(!conversations.busy("graph:node"));
    for id in ["native-1", "native-2"] {
        let turn = &conversations.mirror.thread(id).unwrap().turns[0];
        assert_eq!(turn.status, "completed");
        assert_eq!(turn.items[0].value["text"], format!("Authoritative {id}"));
    }
    client.shutdown();
}
