//! Explicit native config hot-reload probe, isolated profile and owned threads.
//! No personal configuration, credentials, account inference or tool executions.
//! Real native turns use a loopback Responses fixture. The reload probe remains
//! characterized for 0.153.4: saved defaults apply to newly opened sessions,
//! while already-loaded sessions keep their captured settings. A saved ACK is
//! not proof of session reload.
use super::reload_responses_fixture as responses;
use crate::{
    api,
    configuration::Snapshot,
    runtime::Runtime,
    transport::{Client, Event},
};
use serde_json::json;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "Separate native profiles with one loopback fixture turn; no account inference, credentials or project tools"]
fn native_dedicated_profile_keeps_history_and_account_separate_after_restart() -> Result<()> {
    let mut responses = responses::Fixture::start()?;
    let root = tempfile::tempdir()?;
    let native = root.path().join("Supervisor profile");
    let foreign = root.path().join("Other Codex profile");
    let work = root.path().join("workspace");
    for path in [&native, &foreign, &work] {
        std::fs::create_dir(path)?;
    }
    std::fs::write(
        native.join("config.toml"),
        format!(
            "sqlite_home = {}\ncli_auth_credentials_store = \"auto\"\nmodel = \"gpt-5.6-luna\"\nmodel_reasoning_effort = \"low\"\nmodel_provider = \"reload_fixture\"\n[model_providers.reload_fixture]\nname = \"Owned isolation fixture\"\nbase_url = \"http://{}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = false\nsupports_websockets = false\nrequest_max_retries = 0\nstream_max_retries = 0\n",
            serde_json::to_string(&foreign.to_string_lossy())?,
            responses.address
        ),
    )?;
    let runtime = Runtime::discover(&work)?;
    let spawn = |home: &std::path::Path| -> Result<(Client, mpsc::Receiver<Event>)> {
        let mut command = runtime.clone().with_home(home)?.command();
        // Even a conflicting environment and config must lose to the pinned
        // process-level SQLite override. No parent environment is changed.
        command
            .env("CODEX_SQLITE_HOME", &foreign)
            .env_remove("CODEX_ACCESS_TOKEN")
            .env_remove("OPENAI_API_KEY")
            .env_remove("CODEX_API_KEY");
        let (tx, rx) = mpsc::channel();
        let client = Client::spawn_command(command, "isolated-profile-probe", move |event| {
            let _ = tx.send(event);
        })?;
        loop {
            match rx.recv_timeout(Duration::from_secs(30))? {
                Event::Ready { .. } => break,
                Event::Closed { reason } => return Err(reason.into()),
                Event::ServerRequest { .. } => return Err("Unexpected authority request".into()),
                _ => {}
            }
        }
        Ok((client, rx))
    };
    let (client, events) = spawn(&native)?;
    let outcome = (|| -> Result<(String, serde_json::Value)> {
        let config = api::config_read(work.to_str().unwrap())
            .send(&client)?
            .wait()?;
        assert_eq!(
            std::fs::canonicalize(
                config["config"]["sqlite_home"]
                    .as_str()
                    .ok_or("Missing SQLite configuration")?
            )?,
            native.canonicalize()?
        );
        assert_eq!(config["config"]["cli_auth_credentials_store"], "file");
        let opened = api::start_thread(
            work.to_str().unwrap(),
            &api::Profile::default(),
            api::Access::ReadOnly,
        )
        .send(&client)?
        .wait()?;
        let id = opened["thread"]["id"]
            .as_str()
            .ok_or("Missing native thread ID")?
            .to_owned();
        fixture_turn(
            &client,
            &events,
            &id,
            work.to_str().unwrap(),
            "profile-isolation",
        )?;
        let history = api::read_thread(&id).send(&client)?.wait()?;
        assert!(
            std::path::Path::new(
                history["thread"]["path"]
                    .as_str()
                    .ok_or("Missing native path")?
            )
            .canonicalize()?
            .starts_with(native.canonicalize()?)
        );
        Ok((id, history["thread"]["turns"].clone()))
    })();
    client.shutdown();
    let (id, turns) = outcome?;
    let (reopened, _) = spawn(&native)?;
    let restored = api::read_thread(&id).send(&reopened)?.wait();
    reopened.shutdown();
    let restored = restored?;
    assert_eq!(restored["thread"]["turns"], turns);
    let rollout = std::path::Path::new(
        restored["thread"]["path"]
            .as_str()
            .ok_or("Missing saved rollout")?,
    );
    for archived in [false, true] {
        let transferred = root.path().join(if archived {
            "transferred-archive"
        } else {
            "transferred-history"
        });
        let relative = if archived {
            std::path::PathBuf::from("archived_sessions").join(rollout.file_name().unwrap())
        } else {
            rollout
                .canonicalize()?
                .strip_prefix(native.canonicalize()?)?
                .to_owned()
        };
        std::fs::create_dir_all(transferred.join(&relative).parent().unwrap())?;
        std::fs::copy(rollout, transferred.join(&relative))?;
        let (migrated, _) = spawn(&transferred)?;
        let migration: Result<()> = (|| {
            if archived {
                api::unarchive_thread(&id).send(&migrated)?.wait()?;
            }
            api::resume_thread_with_options(
                &id,
                &api::ThreadResumeOptions {
                    cwd: Some(transferred.to_string_lossy().into_owned()),
                    // The source's loopback-only provider is deliberately not
                    // imported. Rehydration sends no turn to this provider.
                    model_provider: Some("openai".into()),
                    access: Some(api::Access::ReadOnly),
                    exclude_turns: true,
                    ..api::ThreadResumeOptions::default()
                },
            )
            .send(&migrated)?
            .wait()?;
            assert_eq!(
                api::read_thread(&id).send(&migrated)?.wait()?["thread"]["turns"],
                turns
            );
            if archived {
                api::archive_thread(&id).send(&migrated)?.wait()?;
            }
            Ok(())
        })();
        migrated.shutdown();
        migration?;
        if archived {
            assert!(transferred.join(relative).exists());
        }
    }
    let (other, _) = spawn(&foreign)?;
    let absent = api::read_thread(&id).send(&other)?.wait();
    other.shutdown();
    assert!(
        absent.is_err(),
        "A separate profile must not see Supervisor history"
    );
    assert!(!native.join("auth.json").exists());
    assert!(!foreign.join("auth.json").exists());
    responses.finish()?;
    Ok(())
}

#[test]
#[ignore = "Explicit isolated native reload characterization with loopback model fixture; no account inference, personal config or tools"]
fn native_config_reload_keeps_loaded_settings_static_and_updates_new_threads() -> Result<()> {
    exercise_loaded_settings(true)
}

#[test]
#[ignore = "Two native turns with loopback Responses fixture; no account inference, personal config or tools"]
fn native_turn_constructor_preserves_configured_reasoning_summary() -> Result<()> {
    exercise_loaded_settings(false)
}

fn exercise_loaded_settings(check_reload: bool) -> Result<()> {
    let mut responses = responses::Fixture::start()?;
    let root = tempfile::Builder::new()
        .prefix("central-native-config-reload-")
        .tempdir()?;
    let native = root.path().join("native");
    let work = root.path().join("workspace");
    std::fs::create_dir_all(&native)?;
    std::fs::create_dir_all(&work)?;
    std::fs::write(
        native.join("config.toml"),
        format!(
            "model = \"gpt-5.6-luna\"\nmodel_provider = \"reload_fixture\"\nmodel_reasoning_effort = \"low\"\nmodel_reasoning_summary = \"concise\"\nsandbox_mode = \"read-only\"\napproval_policy = \"on-request\"\n[model_providers.reload_fixture]\nname = \"Owned reload test fixture\"\nbase_url = \"http://{}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = false\nsupports_websockets = false\nrequest_max_retries = 0\nstream_max_retries = 0\n[mcp_servers.never_start]\nenabled = false\ncommand = \"never-start-reload-fixture\"\n[mcp_servers.never_start.env]\nPRIVATE = \"SYNTHETIC_RELOAD_PRIVATE\"\n",
            responses.address
        ),
    )?;
    let runtime = Runtime::discover(&work)?;
    let mut command = runtime.command();
    command
        .env("CODEX_HOME", &native)
        .env_remove("CODEX_SQLITE_HOME")
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY");
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn_command(command, "config-reload-probe", move |event| {
        let _ = tx.send(event);
    })?;
    let outcome = (|| -> Result<()> {
        loop {
            match rx.recv_timeout(Duration::from_secs(30))? {
                Event::Ready { .. } => break,
                Event::Closed { reason } => {
                    // Diagnose only this synthetic profile, after its failed child
                    // exited. EOF-only startup sends no RPC or model request.
                    let output = runtime
                        .command()
                        .env("CODEX_HOME", &native)
                        .env_remove("CODEX_SQLITE_HOME")
                        .env_remove("CODEX_ACCESS_TOKEN")
                        .env_remove("OPENAI_API_KEY")
                        .env_remove("CODEX_API_KEY")
                        .stdin(std::process::Stdio::null())
                        .output()?;
                    eprintln!(
                        "Owned fixture startup: {}",
                        String::from_utf8_lossy(&output.stderr)
                            .replace("SYNTHETIC_RELOAD_PRIVATE", "[fixture]")
                    );
                    return Err(reason.into());
                }
                Event::ServerRequest { .. } => {
                    return Err("Unexpected native authority request".into());
                }
                _ => {}
            }
        }
        if !api::account_read().send(&client)?.wait()?["account"].is_null() {
            return Err("The reload fixture must remain unauthenticated".into());
        }
        let cwd = work.to_str().ok_or("Non-UTF8 fixture path")?;
        let before = api::config_read(cwd).send(&client)?.wait()?;
        let snapshot = Snapshot::read(&before)?;
        let target = snapshot.target.ok_or("Missing native user target")?;
        if std::fs::canonicalize(&target.file)? != native.join("config.toml").canonicalize()? {
            return Err("Native reload target is not the isolated profile".into());
        }
        let mut threads = Vec::new();
        for _ in 0..2 {
            let call = api::start_thread(cwd, &api::Profile::default(), api::Access::ReadOnly);
            let started = call.send(&client)?.wait()?;
            if started["thread"]["ephemeral"] != false
                || !started["thread"]["path"]
                    .as_str()
                    .is_some_and(|path| std::path::Path::new(path).starts_with(&native))
                || started["sandbox"]["type"] != "readOnly"
            {
                return Err(
                    "Native fixture thread did not start in the owned profile/read-only".into(),
                );
            }
            threads.push(
                started["thread"]["id"]
                    .as_str()
                    .ok_or("Missing native thread")?
                    .to_owned(),
            );
        }
        for thread in &threads {
            fixture_turn(&client, &rx, thread, cwd, "before-reload")?;
        }
        {
            let captured = responses
                .requests
                .lock()
                .map_err(|_| "Fixture lock poisoned")?;
            if captured.len() != 2
                || captured.iter().any(|r| {
                    r["model"] != "gpt-5.6-luna"
                        || r["reasoning"]["effort"] != "low"
                        || r["reasoning"]["summary"] != "concise"
                })
            {
                return Err(
                    "Production turn constructor overrode the native configured profile/summary"
                        .into(),
                );
            }
        }
        if !check_reload {
            println!(
                "Two real native turns preserved configured concise summaries through the production constructor; persisted READY replies verified, loopback model only, no config writes or account inference."
            );
            return Ok(());
        }
        // Discard only startup notifications before the explicit write.
        while let Ok(event) = rx.try_recv() {
            reject_work(&event)?;
        }
        let mut call = api::config_write(
            &target.file,
            &target.version,
            "model_reasoning_summary",
            json!("detailed"),
        );
        call.params["reloadUserConfig"] = json!(true);
        let edits = call.params["edits"].as_array_mut().unwrap();
        for (key, value) in [
            ("model", json!("gpt-5.6-sol")),
            ("model_reasoning_effort", json!("high")),
            ("sandbox_mode", json!("workspace-write")),
            ("approval_policy", json!("on-request")),
        ] {
            edits.push(json!({"keyPath":key,"value":value,"mergeStrategy":"upsert"}));
        }
        let ack = call.send(&client)?.wait()?;
        if ack["status"] != "ok" {
            return Err("Native reload write was not accepted".into());
        }
        let saved = api::config_read(cwd).send(&client)?.wait()?;
        if saved["config"]["model_reasoning_summary"] != "detailed"
            || saved["config"]["model"] != "gpt-5.6-sol"
            || saved["config"]["mcp_servers"] != before["config"]["mcp_servers"]
        {
            return Err("Native write did not preserve the expected saved configuration".into());
        }
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut observed = std::collections::BTreeMap::new();
        while Instant::now() < deadline && observed.len() < threads.len() {
            match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(event) => {
                    reject_work(&event)?;
                    if let Event::Notification { method, params } = event
                        && method == "thread/settings/updated"
                        && let Some(id) = params["threadId"].as_str()
                        && threads.iter().any(|thread| thread == id)
                    {
                        let settings = &params["threadSettings"];
                        if settings["summary"] == "detailed" {
                            observed.insert(id.to_owned(), settings.clone());
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(error) => return Err(error.into()),
            }
        }
        println!(
            "Native settings notifications after reload: {}/{}",
            observed.len(),
            threads.len()
        );
        if !observed.is_empty() {
            return Err("Native reload unexpectedly changed a loaded session; review the session-settings contract".into());
        }
        for (index, thread) in threads.iter().enumerate() {
            // Real persisted fixture turns make native resume available.
            let current = api::resume_thread(thread).send(&client)?.wait()?;
            println!(
                "Owned fixture {}: {}",
                index + 1,
                json!({"model":current["model"],"effort":current["reasoningEffort"],"approval":current["approvalPolicy"],"sandbox":current["sandbox"]["type"],"settingsEvent":observed.get(thread).map(|s|json!({"summary":s["summary"],"effort":s["effort"],"model":s["model"],"approval":s["approvalPolicy"],"sandbox":s["sandboxPolicy"]["type"]}))})
            );
            if current["model"] != "gpt-5.6-luna" || current["reasoningEffort"] != "low" {
                return Err("Reload changed a documented session-static default".into());
            }
        }
        for thread in &threads {
            fixture_turn(&client, &rx, thread, cwd, "after-reload")?;
        }
        let started = api::start_thread(cwd, &api::Profile::default(), api::Access::ReadOnly)
            .send(&client)?
            .wait()?;
        if started["model"] != "gpt-5.6-sol"
            || started["reasoningEffort"] != "high"
            || started["sandbox"]["type"] != "readOnly"
        {
            return Err("A new native session did not load the saved defaults".into());
        }
        let new_thread = started["thread"]["id"]
            .as_str()
            .ok_or("Missing reloaded native thread")?;
        fixture_turn(&client, &rx, new_thread, cwd, "new-session-after-reload")?;
        let captured = responses
            .requests
            .lock()
            .map_err(|_| "Fixture lock poisoned")?;
        if captured.len() != 5 {
            return Err("Expected five actual native requests to the local model fixture".into());
        }
        for (index, request) in captured.iter().enumerate() {
            let (model, effort, summary) = if index < 4 {
                ("gpt-5.6-luna", "low", "concise")
            } else {
                ("gpt-5.6-sol", "high", "detailed")
            };
            if request["model"] != model
                || request["reasoning"]["effort"] != effort
                || request["reasoning"]["summary"] != summary
            {
                return Err(format!("Native fixture request {} did not preserve loaded-session settings or apply new-session defaults: {}", index+1, request).into());
            }
        }
        while let Ok(event) = rx.try_recv() {
            reject_work(&event)?;
        }
        println!(
            "Native reload probe finished; two loaded sessions stayed static and one new session loaded the saved defaults; five local fixture turns, zero account inference/tool calls."
        );
        Ok(())
    })();
    client.shutdown();
    let fixture_result = responses.finish();
    if let Err(error) = &fixture_result {
        eprintln!("Local Responses fixture: {error}");
    }
    outcome?;
    fixture_result?;
    println!("Only the owned temporary profile is removed on probe exit.");
    Ok(())
}
fn fixture_turn(
    client: &Client,
    events: &mpsc::Receiver<Event>,
    thread: &str,
    cwd: &str,
    message: &str,
) -> Result<()> {
    let call = api::start_turn(
        thread,
        message,
        vec![api::text_input("Reply READY; no tools or file access.")],
        cwd,
        &api::Profile::default(),
        api::Access::ReadOnly,
    );
    complete_fixture_turn(client, events, call)
}

pub(super) fn complete_fixture_turn(
    client: &Client,
    events: &mpsc::Receiver<Event>,
    call: api::Call,
) -> Result<()> {
    let thread = call.params["threadId"]
        .as_str()
        .ok_or("Missing fixture thread ID")?
        .to_owned();
    let ack = call.send(client)?.wait()?;
    let turn = ack["turn"]["id"]
        .as_str()
        .ok_or("Fixture turn missing ID")?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let event = events.recv_timeout(deadline.saturating_duration_since(Instant::now()))?;
        match event {
            Event::ServerRequest { .. } => {
                return Err("Unexpected authority request in loopback fixture turn".into());
            }
            Event::Closed { reason } => return Err(reason.into()),
            Event::Notification { method, params } => {
                if method == "item/started"
                    && !matches!(
                        params["item"]["type"].as_str(),
                        Some("userMessage" | "agentMessage" | "reasoning")
                    )
                {
                    return Err("Unexpected tool activity in fixture turn".into());
                }
                if method == "turn/completed"
                    && params["threadId"] == thread
                    && params["turn"]["id"] == turn
                {
                    if params["turn"]["status"] != "completed" {
                        return Err(format!(
                            "Fixture native turn failed: {}",
                            params["turn"]["error"]
                        )
                        .into());
                    }
                    let history = api::read_thread(&thread).send(client)?.wait()?;
                    if !history["thread"]["turns"].as_array().is_some_and(|turns| {
                        turns.iter().any(|t| {
                            t["id"] == turn
                                && t["status"] == "completed"
                                && t["items"].as_array().is_some_and(|items| {
                                    items.iter().any(|i| {
                                        i["type"] == "agentMessage" && i["text"] == "READY"
                                    })
                                })
                        })
                    }) {
                        return Err(
                            "Native stored history omitted the completed fixture reply".into()
                        );
                    }
                    return Ok(());
                }
            }
            _ => {}
        }
    }
}
fn reject_work(event: &Event) -> Result<()> {
    match event {
        Event::ServerRequest { .. } => {
            Err("Unexpected native authority request; none was answered".into())
        }
        Event::Notification { method, .. } if method == "turn/started" => {
            Err("Unexpected model turn".into())
        }
        Event::Closed { reason } => Err(reason.clone().into()),
        _ => Ok(()),
    }
}

#[test]
#[ignore = "Explicit isolated compaction-scope writes; no inference, threads, personal config or tools"]
fn native_compaction_scope_save_clear_and_conflict() -> Result<()> {
    let root = tempfile::Builder::new()
        .prefix("central-native-compaction-scope-")
        .tempdir()?;
    let native = root.path().join("native");
    let work = root.path().join("workspace");
    std::fs::create_dir_all(&native)?;
    std::fs::create_dir_all(&work)?;
    std::fs::write(
        native.join("config.toml"),
        "model_reasoning_summary = \"concise\"\n[mcp_servers.never_start]\nenabled = false\ncommand = \"never-start-scope-fixture\"\n[mcp_servers.never_start.env]\nPRIVATE = \"SYNTHETIC_SCOPE_PRIVATE\"\n",
    )?;
    let runtime = Runtime::discover(&work)?;
    let mut command = runtime.command();
    command
        .env("CODEX_HOME", &native)
        .env_remove("CODEX_SQLITE_HOME")
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY");
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn_command(command, "compaction-scope-probe", move |event| {
        let _ = tx.send(event);
    })?;
    let result = (|| -> Result<()> {
        loop {
            let event = rx.recv_timeout(Duration::from_secs(30))?;
            reject_work(&event)?;
            if matches!(event, Event::Ready { .. }) {
                break;
            }
        }
        if !api::account_read().send(&client)?.wait()?["account"].is_null() {
            return Err("Compaction fixture must remain unauthenticated".into());
        }
        let cwd = work.to_str().ok_or("Non-UTF8 fixture directory")?;
        let before = api::config_read(cwd).send(&client)?.wait()?;
        let key = "model_auto_compact_token_limit_scope";
        let initial = Snapshot::read(&before)?;
        let target = initial.target.as_ref().ok_or("Missing native target")?;
        if std::fs::canonicalize(&target.file)? != native.join("config.toml").canonicalize()? {
            return Err("Compaction target is outside the isolated profile".into());
        }
        let stale = initial.edit(key, "total")?;
        for option in [Some("body_after_prefix"), Some("total"), None] {
            let snapshot = Snapshot::read(&api::config_read(cwd).send(&client)?.wait()?)?;
            let call = match option {
                Some(value) => snapshot.edit(key, value)?,
                None => snapshot.clear(key)?,
            };
            if call.params["reloadUserConfig"] != false {
                return Err("Unexpected live reload".into());
            }
            let ack = call.send(&client)?.wait()?;
            let target = snapshot
                .target
                .as_ref()
                .ok_or("Missing current native target")?;
            if crate::configuration::WriteOutcome::read(&ack, target, key)?.overridden {
                return Err("Unexpected fixture override".into());
            }
            let raw = api::config_read(cwd).send(&client)?.wait()?;
            let current = Snapshot::read(&raw)?;
            let preference = current
                .preferences
                .iter()
                .find(|p| p.key == key)
                .ok_or("Missing scope projection")?;
            if preference.user_value.as_deref() != option {
                return Err("Saved scope does not match native user-layer readback".into());
            }
            if option.is_some() && preference.effective.as_deref() != option {
                return Err("Effective scope does not match native readback".into());
            }
            let mut expected = before["config"].clone();
            let mut actual = raw["config"].clone();
            expected
                .as_object_mut()
                .ok_or("Missing initial config")?
                .remove(key);
            actual
                .as_object_mut()
                .ok_or("Missing current config")?
                .remove(key);
            if expected != actual
                || serde_json::to_string(&current)?.contains("SYNTHETIC_SCOPE_PRIVATE")
            {
                return Err("Unrelated native values changed or private fixture exposed".into());
            }
            if option == Some("body_after_prefix") {
                if !matches!(
                    stale.clone().send(&client)?.wait(),
                    Err(super::CallError::Rpc(_))
                ) {
                    return Err("Stale native write was not rejected".into());
                }
                if api::config_read(cwd).send(&client)?.wait()?["config"] != raw["config"] {
                    return Err("Rejected stale write changed config".into());
                }
            }
        }
        while let Ok(event) = rx.try_recv() {
            reject_work(&event)?;
            if matches!(event, Event::Notification { ref method, .. } if method == "thread/started")
            {
                return Err("Unexpected native thread creation".into());
            }
        }
        println!(
            "Native compaction counting: both enums saved, exact override cleared, stale write rejected, unrelated values preserved; zero inference/threads/reload."
        );
        Ok(())
    })();
    client.shutdown();
    result
}
