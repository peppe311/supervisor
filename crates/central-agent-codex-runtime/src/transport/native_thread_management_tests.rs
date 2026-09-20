//! P3-A production constructors against official Codex in a disposable profile.
//! Native durable history is real; model responses are loopback-only and inert.
use super::{CallError, Client, Event, reload_responses_fixture};
use crate::{
    api,
    conversations::{Action, Conversations, Request},
    runtime::Runtime,
    thread_sections::Sections,
};
use serde_json::Value;
use std::{
    path::Path,
    process::Command,
    sync::mpsc,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const OWNER: &str = "chat:p3-native";

fn execute(client: &Client, call: api::Call) -> Result<Value> {
    Ok(call
        .send(client)?
        .reply
        .recv_timeout(Duration::from_secs(30))??)
}
fn git(cwd: &Path, args: &[&str]) -> Result<()> {
    let mut command = Command::new("git");
    crate::runtime::hide_window(&mut command);
    if !command
        .current_dir(cwd)
        .args(args)
        .output()?
        .status
        .success()
    {
        return Err("Owned Git fixture setup failed".into());
    }
    Ok(())
}
fn notification(book: &mut Conversations, event: Event) -> Result<()> {
    match event {
        Event::Closed { reason } => return Err(reason.into()),
        Event::ServerRequest { .. } => {
            return Err("Unexpected native authority request; no grant was made".into());
        }
        Event::Notification { method, params } => {
            if method == "item/started"
                && !matches!(
                    params["item"]["type"].as_str(),
                    Some("userMessage" | "agentMessage" | "reasoning")
                )
            {
                return Err("Unexpected tool activity in P3 fixture".into());
            }
            book.notification(&method, &params)?;
        }
        _ => {}
    }
    Ok(())
}
fn drain(book: &mut Conversations, rx: &mpsc::Receiver<Event>) -> Result<()> {
    while let Ok(event) = rx.try_recv() {
        notification(book, event)?;
    }
    Ok(())
}
fn read_history(client: &Client, book: &mut Conversations) -> Result<Vec<String>> {
    let request = book.action(OWNER, Action::Read)?;
    book.complete(&request, Ok(execute(client, request.call.clone())?))?;
    let thread = book.binding(OWNER).unwrap().thread_id.clone();
    let revision = book.mirror.revision();
    let mut cursor = None;
    let mut turns = Vec::new();
    for _ in 0..8 {
        let page = execute(client, api::list_thread_turns(&thread, cursor.as_deref()))?;
        let data = page["data"].as_array().ok_or("Missing native turn page")?;
        if data.len() > 64 {
            return Err("Native fixture page exceeded its bound".into());
        }
        turns.extend(data.iter().cloned());
        cursor = crate::thread_sections::cursor(&page)?;
        if cursor.is_none() {
            book.hydrate_paginated_history(OWNER, &thread, &turns, revision)?;
            return Ok(turns
                .iter()
                .map(|t| t["id"].as_str().unwrap().into())
                .collect());
        }
    }
    Err("Native fixture pagination did not terminate".into())
}
fn run(client: &Client, book: &mut Conversations, request: Request) -> Result<()> {
    book.complete(&request, Ok(execute(client, request.call.clone())?))?;
    Ok(())
}
fn sections_reply(
    client: &Client,
    sections: &mut Sections,
    request: crate::thread_sections::Request,
) -> Result<()> {
    let mut next = Some(request);
    for _ in 0..8 {
        let Some(request) = next.take() else {
            return Ok(());
        };
        next = sections.complete(&request, Ok(execute(client, request.call())?))?;
    }
    Err("Native section pagination did not terminate".into())
}

#[test]
#[ignore = "Explicit isolated official P3-A Git/section/revert probe, loopback model only, no personal profile or tools"]
fn native_p3_git_sections_and_protected_revert() -> Result<()> {
    let mut fixture = reload_responses_fixture::Fixture::start()?;
    let root = tempfile::Builder::new()
        .prefix("central-native-p3-")
        .tempdir()?;
    let native = root.path().join("native");
    let work = root.path().join("workspace");
    std::fs::create_dir_all(&native)?;
    std::fs::create_dir_all(&work)?;
    git(&work, &["init", "--quiet", "-b", "p3-native"])?;
    git(
        &work,
        &[
            "config",
            "remote.origin.url",
            "https://example.test/owned/repo.git",
        ],
    )?;
    std::fs::write(
        work.join("untouched.txt"),
        "P3 fixture: preserve local files",
    )?;
    std::fs::write(
        native.join("config.toml"),
        format!(
            "model = \"gpt-5.6-luna\"\nmodel_provider = \"p3_fixture\"\nmodel_reasoning_effort = \"low\"\nsandbox_mode = \"read-only\"\napproval_policy = \"on-request\"\n[model_providers.p3_fixture]\nname = \"Owned P3 fixture\"\nbase_url = \"http://{}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = false\nsupports_websockets = false\nrequest_max_retries = 0\nstream_max_retries = 0\n",
            fixture.address
        ),
    )?;
    let runtime = Runtime::discover(&work)?;
    let mut command = runtime.command();
    command
        .env("CODEX_HOME", &native)
        .env("CODEX_SQLITE_HOME", &native)
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY");
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn_command(command, "p3-native-probe", move |event| {
        let _ = tx.send(event);
    })?;
    let outcome = (|| -> Result<()> {
        loop {
            match rx.recv_timeout(Duration::from_secs(30))? {
                Event::Ready { .. } => break,
                Event::Closed { reason } => return Err(reason.into()),
                Event::ServerRequest { .. } => {
                    return Err("Unexpected authority request at startup".into());
                }
                _ => {}
            }
        }
        assert!(execute(&client, api::account_read())?["account"].is_null());
        let mut book = Conversations::default();
        let request = book.open(
            OWNER,
            &work,
            &api::Profile::default(),
            api::Access::ReadOnly,
        )?;
        let started = execute(&client, request.call.clone())?;
        assert_eq!(started["sandbox"]["type"], "readOnly");
        assert_eq!(started["thread"]["ephemeral"], false);
        assert!(
            Path::new(
                started["thread"]["path"]
                    .as_str()
                    .ok_or("Missing owned native history path")?
            )
            .starts_with(&native)
        );
        book.complete(&request, Ok(started))?;
        drain(&mut book, &rx)?;
        assert_eq!(book.history_mode(OWNER), Some("paginated"));
        let thread = book.binding(OWNER).unwrap().thread_id.clone();
        for n in 0..3 {
            let request = book.start_turn(
                OWNER,
                &format!("fixture-p3-{n}"),
                vec![api::text_input("Reply READY; no tools or file access.")],
                &work,
                &api::Profile::default(),
                api::Access::ReadOnly,
            )?;
            let response = execute(&client, request.call.clone())?;
            let turn = response["turn"]["id"]
                .as_str()
                .ok_or("Missing native turn")?
                .to_owned();
            book.complete(&request, Ok(response))?;
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                let event = rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))?;
                let completed = matches!(&event,Event::Notification{method,params} if method=="turn/completed" && params["threadId"]==thread && params["turn"]["id"]==turn && params["turn"]["status"]=="completed");
                notification(&mut book, event)?;
                if completed {
                    break;
                }
            }
            drain(&mut book, &rx)?;
        }
        let original = read_history(&client, &mut book)?;
        assert_eq!(original.len(), 3);
        git(&work, &["symbolic-ref", "HEAD", "refs/heads/p3-updated"])?;
        let intent = book.prepare_git_metadata(OWNER, &work)?;
        assert_eq!(intent.metadata().branch.as_deref(), Some("p3-updated"));
        let request = book.confirm_git_metadata(intent)?;
        run(&client, &mut book, request)?;
        assert_eq!(
            book.native_metadata(OWNER)
                .unwrap()
                .git
                .as_ref()
                .unwrap()
                .branch
                .as_deref(),
            Some("p3-updated")
        );
        read_history(&client, &mut book)?;
        println!(
            "P3 native Git metadata: verified update and read-back in the bound temporary worktree."
        );

        let mut sections = Sections::default();
        let request = sections.refresh()?;
        sections_reply(&client, &mut sections, request)?;
        let baseline = sections.entries().unwrap().to_vec();
        assert!(
            baseline.len() <= 1 && baseline.iter().all(|s| s.name == "Pinned"),
            "Unexpected section baseline in the new isolated profile"
        );
        let intent = sections.prepare_create("P3 owned section")?;
        let request = sections.confirm(intent)?;
        sections_reply(&client, &mut sections, request)?;
        let section = sections
            .entries()
            .unwrap()
            .iter()
            .find(|s| s.name == "P3 owned section" && !baseline.iter().any(|old| old.id == s.id))
            .ok_or("Missing newly created owned section")?
            .id
            .clone();
        let intent = sections.prepare_rename(&section, "P3 renamed")?;
        let request = sections.confirm(intent)?;
        sections_reply(&client, &mut sections, request)?;
        assert_eq!(sections.section(&section)?.name, "P3 renamed");
        for target in [Some(section.as_str()), None] {
            drain(&mut book, &rx)?;
            read_history(&client, &mut book)?;
            let intent = book.prepare_section_move(OWNER, &work, &sections, target, None)?;
            let request = book.confirm_section_move(intent, &sections)?;
            run(&client, &mut book, request)?;
            read_history(&client, &mut book)?;
            assert_eq!(
                book.native_metadata(OWNER)
                    .unwrap()
                    .section
                    .as_ref()
                    .map(|s| s.id.as_str()),
                target
            );
        }
        let intent = sections.prepare_delete(&section)?;
        let request = sections.confirm(intent)?;
        sections_reply(&client, &mut sections, request)?;
        assert_eq!(sections.entries().unwrap(), baseline);
        println!(
            "P3 native sections: list/create/rename/move/remove/delete verified; native default Pinned section preserved."
        );

        drain(&mut book, &rx)?;
        assert_eq!(read_history(&client, &mut book)?, original);
        let intent = book.prepare_revert(OWNER, &work, &original[1], "fixture-p3-revert")?;
        assert_eq!(intent.removed_turn_count(), 2);
        let request = book.confirm_revert(intent)?;
        std::fs::write(
            root.path().join("bindings.json"),
            serde_json::to_vec(book.saved())?,
        )?;
        let response = execute(&client, request.call.clone())?;
        // The desktop event loop can receive native closure/revert events
        // before the RPC result. Exercise that order, not only ACK-first.
        drain(&mut book, &rx)?;
        book.complete(&request, Ok(response))?;
        assert!(book.saved().reverts.contains_key(OWNER));
        let retained = read_history(&client, &mut book)?;
        assert_eq!(retained, original[..1]);
        assert!(book.saved().reverts.is_empty());
        println!(
            "P3 post-revert lifecycle: loaded={}, busy={}, status={}",
            book.is_thread_loaded(OWNER),
            book.busy(OWNER),
            book.mirror
                .thread(&thread)
                .unwrap()
                .status
                .as_ref()
                .unwrap_or(&Value::Null)
        );
        assert!(!book.busy(OWNER));
        if !book.is_thread_loaded(OWNER) {
            let request = book.open(
                OWNER,
                &work,
                &api::Profile::default(),
                api::Access::ReadOnly,
            )?;
            run(&client, &mut book, request)?;
            drain(&mut book, &rx)?;
            assert_eq!(read_history(&client, &mut book)?, retained);
        }
        assert!(book.ready_for_turn(OWNER));
        // Restore the pre-ACK receipt from disk and reconcile from native data;
        // no mutation is resent, even when the original acknowledgement is lost.
        let saved = serde_json::from_slice(&std::fs::read(root.path().join("bindings.json"))?)?;
        let mut restored = Conversations::restore(saved)?;
        assert_eq!(read_history(&client, &mut restored)?, retained);
        assert!(restored.saved().reverts.is_empty());
        assert_eq!(
            std::fs::read_to_string(work.join("untouched.txt"))?,
            "P3 fixture: preserve local files"
        );
        assert_eq!(
            crate::thread_metadata::RepositorySnapshot::capture(&work)?
                .metadata()
                .branch
                .as_deref(),
            Some("p3-updated")
        );
        assert!(matches!(
            api::list_thread_items(&thread, None, None).send(&client),
            Err(CallError::Rejected(_))
        ));
        assert_eq!(fixture.requests.lock().unwrap().len(), 3);
        println!(
            "P3 native revert: 3 -> 1 retained turn, receipt reconciled after read and simulated client restart, Git/files unchanged, items/list blocked; exactly 3 loopback model calls, no tools or account inference."
        );
        Ok(())
    })();
    client.shutdown();
    fixture.finish()?;
    outcome
}
