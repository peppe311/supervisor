//! Probe the official named-profile contract without changing user configuration.
//! A fixed local model emits READY only. No account inference or tool execution.
use super::{native_config_reload_tests::complete_fixture_turn, reload_responses_fixture};
use crate::{
    api,
    runtime::Runtime,
    transport::{CallError, Client, Event},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "Explicit native named-permission probe; isolated config, local fixed model, no account or tools"]
fn native_named_permission_profile_is_reported_and_inherited() -> Result<()> {
    probe_named_profiles(true)
}

#[test]
#[ignore = "Explicit native stable/beta boundary probe; isolated config, no model requests or tools"]
fn native_permission_profiles_respect_stable_api_boundary() -> Result<()> {
    probe_named_profiles(false)
}

fn probe_named_profiles(require_native_provenance: bool) -> Result<()> {
    let mut fixture = reload_responses_fixture::Fixture::start()?;
    let root = tempfile::Builder::new()
        .prefix("central-native-permission-profile-")
        .tempdir()?;
    let native = root.path().join("native");
    let work = root.path().join("workspace");
    std::fs::create_dir_all(&native)?;
    std::fs::create_dir_all(&work)?;
    // No legacy sandbox_mode: it takes precedence over default_permissions.
    let config = format!(
        "model = \"gpt-5.6-luna\"\nmodel_provider = \"permission_fixture\"\n[permissions.fixture_read]\nextends = \":read-only\"\ndescription = \"Owned named read-only fixture\"\n[model_providers.permission_fixture]\nname = \"Owned permission fixture\"\nbase_url = \"http://{}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = false\nsupports_websockets = false\nrequest_max_retries = 0\nstream_max_retries = 0\n",
        fixture.address,
    );
    std::fs::write(native.join("config.toml"), &config)?;
    let mut command = Runtime::discover(&work)?.command();
    command
        .env("CODEX_HOME", &native)
        .env_remove("CODEX_SQLITE_HOME")
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY");
    let settings = Arc::new(Mutex::new(Vec::<Value>::new()));
    let captured = settings.clone();
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn_command(command, "native-permission-profile-probe", move |event| {
        if let Event::Notification { method, params } = &event
            && method == "thread/settings/updated"
        {
            // Owned fixture values only; retain exact native provenance, not an
            // inferred profile derived from a legacy sandbox compatibility view.
            captured.lock().unwrap().push(params.clone());
        }
        let _ = tx.send(event);
    })?;
    let outcome = (|| -> Result<()> {
        loop {
            match rx.recv_timeout(Duration::from_secs(30))? {
                Event::Ready { .. } => break,
                Event::Closed { reason } => return Err(reason.into()),
                Event::ServerRequest { .. } => return Err("Unexpected authority request".into()),
                _ => {}
            }
        }
        if !api::account_read().send(&client)?.wait()?["account"].is_null() {
            return Err("Named-profile fixture must remain unauthenticated".into());
        }
        let cwd = work.to_str().ok_or("Non-UTF8 fixture directory")?;
        let mut cursor = Value::Null;
        let mut cursors = BTreeSet::new();
        let mut profiles = Vec::new();
        loop {
            let page = client
                .request(
                    "permissionProfile/list",
                    json!({"cwd":cwd,"cursor":cursor,"limit":1}),
                )?
                .wait()?;
            profiles.extend(
                page["data"]
                    .as_array()
                    .ok_or("Missing native profile inventory")?
                    .iter()
                    .cloned(),
            );
            cursor = page["nextCursor"].clone();
            if cursor.is_null() {
                break;
            }
            let next = cursor.as_str().ok_or("Invalid profile cursor")?;
            if !cursors.insert(next.to_owned()) {
                return Err("Repeated native profile cursor".into());
            }
        }
        let profile = profiles
            .iter()
            .find(|p| p["id"] == "fixture_read")
            .ok_or("Named profile absent from native inventory")?;
        if profile["allowed"] != true || profile["description"] != "Owned named read-only fixture" {
            return Err(format!("Unexpected native profile: {profile}").into());
        }
        println!(
            "Native paginated inventory reports the owned named profile as allowed ({} profiles).",
            profiles.len()
        );
        let rejected = client
            .request(
                "thread/start",
                json!({"cwd":cwd,"permissions":"fixture_read","approvalPolicy":"untrusted"}),
            )?
            .wait();
        match rejected {
            Err(CallError::Rpc(error))
                if error.code == -32600
                    && error.message.contains("experimentalApi")
                    && error.message.contains("permissions") =>
            {
                println!(
                    "Native stable client rejects named selection: {} ({})",
                    error.message, error.code
                );
            }
            other => return Err(format!("Named-profile beta boundary changed: {other:?}").into()),
        }
        if !require_native_provenance {
            let threads = client.request("thread/list", json!({"limit":1}))?.wait()?;
            if threads["data"]
                .as_array()
                .is_none_or(|items| !items.is_empty())
                || !fixture
                    .requests
                    .lock()
                    .map_err(|_| "Poisoned fixture")?
                    .is_empty()
                || std::fs::read_to_string(native.join("config.toml"))? != config
                || std::fs::read_dir(&work)?.next().is_some()
            {
                return Err("Rejected selection created work or changed the fixture".into());
            }
            return Ok(());
        }
        let started = client.request("thread/start", json!({"cwd":cwd,"approvalPolicy":"untrusted","approvalsReviewer":"user","config":{"default_permissions":"fixture_read"}}))?.wait()?;
        if started["sandbox"]["type"] != "readOnly"
            || !started["thread"]["path"]
                .as_str()
                .is_some_and(|p| std::path::Path::new(p).starts_with(&native))
        {
            return Err("Thread escaped owned read-only profile".into());
        }
        let thread = started["thread"]["id"]
            .as_str()
            .ok_or("Missing native thread")?;
        for step in 0..2 {
            // No sandboxPolicy override: the native thread owns named access.
            complete_fixture_turn(
                &client,
                &rx,
                api::Call {
                    method: "turn/start",
                    params: json!({"threadId":thread,"cwd":cwd,"effort":if step == 0 {"low"} else {"medium"},"input":[api::text_input("Reply READY; no tools or file access.")]}),
                },
            )?;
            let reported = settings.lock().map_err(|_| "Poisoned settings capture")?;
            let latest = reported.iter().rev().find(|p| p["threadId"] == thread);
            let active = latest.map(|p| &p["threadSettings"]["activePermissionProfile"]);
            if active != Some(&json!({"id":"fixture_read","extends":":read-only"})) {
                return Err(format!("Native named-profile provenance absent or changed after turn {step}: {active:?}; settings events: {}", reported.len()).into());
            }
            println!(
                "Native turn {step} completed READY and reports fixture_read extending :read-only."
            );
        }
        if fixture
            .requests
            .lock()
            .map_err(|_| "Poisoned fixture")?
            .len()
            != 2
            || std::fs::read_to_string(native.join("config.toml"))? != config
            || std::fs::read_dir(&work)?.next().is_some()
        {
            return Err("Probe changed config/workspace or duplicated model work".into());
        }
        Ok(())
    })();
    client.shutdown();
    let finished = fixture.finish();
    outcome?;
    finished?;
    println!(
        "Owned profile probe finished (require_native_provenance={require_native_provenance}); no account inference, tools or personal config. Only owned temporary data removed."
    );
    Ok(())
}
