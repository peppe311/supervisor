//! Real native stdio loss before ACK, not a delayed host callback. Only the
//! model backend is simulated. No account inference or private profile access.
use super::reload_responses_fixture as responses;
use super::*;
use crate::{
    api,
    conversations::{Action, Conversations, Request},
    runtime::Runtime,
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SEED: &str = "Seed: reply READY without tools.";
const LOST: &str = "Lost prompt: reply READY without tools.";
const NEXT: &str = "Continue: reply READY without tools.";

fn wait_for_owned_rollout(path: &Path, native: &Path, expected: &[&str]) -> Result<()> {
    if !path.starts_with(native) {
        return Err("Wire-loss fixture rollout escaped the owned native profile".into());
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if std::fs::read_to_string(path)
            .is_ok_and(|text| expected.iter().all(|needle| text.contains(needle)))
        {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "Owned rollout did not persist the expected native turn before forced wire loss: {}",
                path.display()
            )
            .into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn assert_history(history: &Value, expected: &[&str], native: &Path, work: &Path) -> Result<()> {
    let thread = &history["thread"];
    assert!(
        Path::new(
            thread["path"]
                .as_str()
                .ok_or("Missing native storage path")?
        )
        .starts_with(native)
    );
    assert_eq!(
        Path::new(thread["cwd"].as_str().ok_or("Missing native cwd")?).canonicalize()?,
        work.canonicalize()?
    );
    let turns = thread["turns"].as_array().ok_or("Missing native turns")?;
    assert_eq!(turns.len(), expected.len());
    for (turn, text) in turns.iter().zip(expected) {
        assert_eq!(turn["status"], "completed");
        let items = turn["items"].as_array().ok_or("Missing native items")?;
        assert!(items.iter().all(|i| matches!(
            i["type"].as_str(),
            Some("userMessage" | "agentMessage" | "reasoning")
        )));
        let users: Vec<_> = items
            .iter()
            .filter(|i| i["type"] == "userMessage")
            .collect();
        assert_eq!(users.len(), 1);
        let content = users[0]["content"]
            .as_array()
            .ok_or("Missing native input content")?;
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], *text);
        assert!(
            items
                .iter()
                .any(|i| i["type"] == "agentMessage" && i["text"] == "READY")
        );
    }
    Ok(())
}

#[test]
#[ignore = "Real isolated App Server and loopback model; requires Node test relay, no account inference"]
fn native_wire_loss_before_ack_restores_exact_receipts_without_replay() -> Result<()> {
    for owner in ["chat:wire-loss", "graph:entity:wire-loss"] {
        for accepted in [false, true] {
            exercise(owner, accepted)?;
        }
    }
    Ok(())
}

fn spawn(mut command: Command, native: &Path) -> Result<(Client, mpsc::Receiver<Event>)> {
    command
        .env("CODEX_HOME", native)
        .env_remove("CODEX_SQLITE_HOME")
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY");
    crate::runtime::hide_window(&mut command);
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn_command(command, "wire-loss-probe", move |e| {
        let _ = tx.send(e);
    })?;
    loop {
        match rx.recv_timeout(Duration::from_secs(30))? {
            Event::Ready { .. } => break,
            Event::Closed { reason } => return Err(reason.into()),
            Event::ServerRequest { .. } => return Err("Unexpected authority request".into()),
            _ => {}
        }
    }
    assert!(reply(&client, api::account_read())?["account"].is_null());
    Ok((client, rx))
}
fn reply(client: &Client, call: api::Call) -> Result<Value> {
    Ok(call
        .send(client)?
        .reply
        .recv_timeout(Duration::from_secs(30))??)
}
fn complete(client: &Client, book: &mut Conversations, request: Request) -> Result<Value> {
    let value = reply(client, request.call.clone())?;
    book.complete(&request, Ok(value.clone()))?;
    Ok(value)
}
fn read_history(
    client: &Client,
    book: &mut Conversations,
    owner: &str,
    thread: &str,
) -> Result<Value> {
    let requested_at = book.mirror.revision();
    let request = book.action(owner, Action::Read)?;
    let omitted = request.omits_turns();
    let mut value = complete(client, book, request)?;
    if omitted {
        if book.history_mode(owner) != Some("paginated") {
            return Err("Metadata read did not select paginated native history".into());
        }
        let mut cursor = None;
        let mut turns = Vec::new();
        loop {
            let page = reply(client, api::list_thread_turns(thread, cursor.as_deref()))?;
            turns.extend(
                page["data"]
                    .as_array()
                    .ok_or("Missing native history page")?
                    .iter()
                    .cloned(),
            );
            cursor = match &page["nextCursor"] {
                Value::Null => None,
                Value::String(next) if !next.is_empty() => Some(next.clone()),
                _ => return Err("Invalid native history page cursor".into()),
            };
            if cursor.is_none() {
                break;
            }
        }
        book.hydrate_paginated_history(owner, thread, &turns, requested_at)?;
        value["thread"]["turns"] = Value::Array(turns);
    }
    Ok(value)
}
fn finished(
    client: &Client,
    events: &mpsc::Receiver<Event>,
    book: &mut Conversations,
    owner: &str,
    turn: &str,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match events.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
            Event::Closed { reason } => return Err(reason.into()),
            Event::ServerRequest { .. } => return Err("Unexpected tool authority request".into()),
            Event::Notification { method, params } => {
                if method == "item/started" {
                    assert!(matches!(
                        params["item"]["type"].as_str(),
                        Some("userMessage" | "agentMessage" | "reasoning")
                    ));
                }
                book.notification(&method, &params)?;
                if method == "turn/completed" && params["turn"]["id"] == turn {
                    assert_eq!(params["turn"]["status"], "completed");
                    let thread = book.binding(owner).unwrap().thread_id.clone();
                    read_history(client, book, owner, &thread)?;
                    return Ok(());
                }
            }
            _ => {}
        }
    }
}
fn exercise(owner: &str, accepted: bool) -> Result<()> {
    let mut model = responses::Fixture::start()?;
    let root = tempfile::Builder::new()
        .prefix("central-native-wire-loss-")
        .tempdir()?;
    let native = root.path().join("native");
    let work = root.path().join("workspace");
    std::fs::create_dir_all(&native)?;
    std::fs::create_dir_all(&work)?;
    std::fs::write(
        native.join("config.toml"),
        format!(
            "model=\"gpt-5.6-luna\"\nmodel_provider=\"wire_fixture\"\nmodel_reasoning_effort=\"low\"\nsandbox_mode=\"read-only\"\napproval_policy=\"on-request\"\n[model_providers.wire_fixture]\nname=\"Owned wire fixture\"\nbase_url=\"http://{}/v1\"\nwire_api=\"responses\"\nrequires_openai_auth=false\nsupports_websockets=false\nrequest_max_retries=0\nstream_max_retries=0\n",
            model.address
        ),
    )?;
    let marker = root.path().join("wire-marker.json");
    let saved_file = root.path().join("owned-binding.json");
    let runtime = Runtime::discover(&work)?;
    let profile = api::Profile::default();
    let mut book = Conversations::default();

    // Persist an unrelated seed through a direct native process first. Killing
    // the relay's Windows job intentionally terminates its whole child tree and
    // can discard an otherwise completed rollout that has not reached Codex's
    // durable index yet. That race is outside the receipt boundary under test.
    let (seed_client, seed_events) = spawn(runtime.command(), &native)?;
    let open = book.open(owner, &work, &profile, api::Access::ReadOnly)?;
    let opened = complete(&seed_client, &mut book, open)?;
    let rollout = std::path::PathBuf::from(
        opened["thread"]["path"]
            .as_str()
            .ok_or("Missing owned rollout path")?,
    );
    let thread = book.binding(owner).unwrap().thread_id.clone();
    let start = book.start_turn(
        owner,
        "seed-input",
        vec![api::text_input(SEED)],
        &work,
        &profile,
        api::Access::ReadOnly,
    )?;
    let seed = complete(&seed_client, &mut book, start)?;
    finished(
        &seed_client,
        &seed_events,
        &mut book,
        owner,
        seed["turn"]["id"].as_str().unwrap(),
    )?;
    wait_for_owned_rollout(&rollout, &native, &[SEED, "READY"])?;
    seed_client.shutdown();
    book.disconnect();
    drop(seed_client);
    drop(seed_events);

    let mut relay = Command::new("node");
    relay
        .current_dir(&work)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/native-wire-loss.mjs"))
        .arg(runtime.executable())
        .arg(&marker)
        .arg(if accepted { "after" } else { "before" })
        .arg("1");
    let (client, events) = spawn(relay, &native)?;
    let resume = book.open(owner, &work, &profile, api::Access::ReadOnly)?;
    let resumed = complete(&client, &mut book, resume)?;
    assert_eq!(resumed["thread"]["id"], thread);
    assert_eq!(book.binding(owner).unwrap().thread_id, thread);
    let seeded_history = read_history(&client, &mut book, owner, &thread)?;
    assert_history(&seeded_history, &[SEED], &native, &work)?;
    // Drain only already-delivered seed notifications, never the lost response.
    while let Ok(event) = events.try_recv() {
        if let Event::Notification { method, params } = event {
            book.notification(&method, &params)?;
        }
    }
    let lost = book.start_turn(
        owner,
        "lost-input",
        vec![api::text_input(LOST)],
        &work,
        &profile,
        api::Access::ReadOnly,
    )?;
    std::fs::write(&saved_file, serde_json::to_vec(book.saved())?)?;
    let ticket = lost.call.send(&client)?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let evidence = loop {
        if let Ok(bytes) = std::fs::read(&marker)
            && let Ok(value) = serde_json::from_slice::<Value>(&bytes)
        {
            break value;
        }
        if Instant::now() >= deadline {
            return Err("Owned wire relay did not reach the requested loss boundary".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(evidence["starts"], 1);
    assert_eq!(evidence["acknowledged"], accepted);
    assert_eq!(
        evidence["phase"],
        if accepted {
            "completed-with-ack-lost"
        } else {
            "request-dropped"
        }
    );
    if accepted {
        // The relay has observed native completion while withholding the ACK;
        // wait for that owned rollout write before intentionally killing the
        // transport so the accepted/not-accepted branches stay deterministic.
        wait_for_owned_rollout(&rollout, &native, &[SEED, LOST])?;
    }
    assert!(
        matches!(ticket.try_result(), Err(mpsc::TryRecvError::Empty)),
        "Wire ACK leaked into the production client"
    );
    client.shutdown();
    assert!(matches!(
        ticket.reply.recv_timeout(Duration::from_secs(5))?,
        Err(CallError::Disconnected {
            delivery_unknown: true,
            ..
        })
    ));
    book.disconnect();
    drop(book);
    drop(client);
    // Restore only durable production metadata and restart the actual official
    // process. No model transcript, turn or ACK is reconstructed by this test.
    let mut restored =
        Conversations::restore(serde_json::from_slice(&std::fs::read(saved_file)?)?)?;
    assert_eq!(restored.saved().unresolved[owner].message_id, "lost-input");
    assert!(
        restored
            .resolve_delivery_after_user_review(owner, "lost-input")
            .is_err()
    );
    assert!(
        restored
            .open(owner, &work, &profile, api::Access::ReadOnly)
            .is_err()
    );
    let (fresh, fresh_events) = spawn(runtime.command(), &native)?;
    let history = read_history(&fresh, &mut restored, owner, &thread)?;
    assert_history(
        &history,
        if accepted { &[SEED, LOST] } else { &[SEED] },
        &native,
        &work,
    )?;
    let turns = history["thread"]["turns"]
        .as_array()
        .ok_or("Missing native turns")?;
    assert_eq!(turns.len(), if accepted { 2 } else { 1 });
    let exact = turns
        .iter()
        .flat_map(|t| t["items"].as_array().unwrap())
        .filter(|i| i["type"] == "userMessage" && i["clientId"] == "lost-input")
        .count();
    assert!(exact <= usize::from(accepted));
    // Only exact native client correlation may clear uncertainty automatically.
    assert_eq!(restored.saved().unresolved.contains_key(owner), exact == 0);
    if exact == 0 {
        assert!(
            restored
                .resolve_delivery_after_user_review(owner, "another-input")
                .is_err()
        );
        restored.resolve_delivery_after_user_review(owner, "lost-input")?;
    }
    let resume = restored.open(owner, &work, &profile, api::Access::ReadOnly)?;
    complete(&fresh, &mut restored, resume)?;
    assert_eq!(restored.binding(owner).unwrap().thread_id, thread);
    let start = restored.start_turn(
        owner,
        "continuation-input",
        vec![api::text_input(NEXT)],
        &work,
        &profile,
        api::Access::ReadOnly,
    )?;
    let ack = complete(&fresh, &mut restored, start)?;
    finished(
        &fresh,
        &fresh_events,
        &mut restored,
        owner,
        ack["turn"]["id"].as_str().unwrap(),
    )?;
    let final_thread = restored.mirror.thread(&thread).unwrap();
    assert_eq!(final_thread.turns.len(), if accepted { 3 } else { 2 });
    assert!(restored.saved().unresolved.is_empty());
    let final_history = read_history(&fresh, &mut restored, owner, &thread)?;
    assert_history(
        &final_history,
        if accepted {
            &[SEED, LOST, NEXT]
        } else {
            &[SEED, NEXT]
        },
        &native,
        &work,
    )?;
    fresh.shutdown();
    model.finish()?;
    assert_eq!(
        model.requests.lock().unwrap().len(),
        if accepted { 3 } else { 2 }
    );
    println!(
        "PASS {owner}: wire loss {}, persisted receipt, exact native inputs/replies/cwd readback, correlation matches {exact}, explicit/exact reconciliation and same-thread continuation without replay (loopback model only).",
        if accepted {
            "after native acceptance, before client ACK"
        } else {
            "before native acceptance"
        }
    );
    Ok(())
}
