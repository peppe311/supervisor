//! Native settings transitions through production constructors. Only a local
//! fixed-response fixture is contacted; no account, personal config or tools.
use super::{native_config_reload_tests::complete_fixture_turn, reload_responses_fixture};
use crate::{
    api,
    runtime::Runtime,
    transport::{Client, Event},
};
use serde_json::json;
use std::{sync::mpsc, time::Duration};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "Explicit native profile transitions with local Responses fixture; no account inference or tools"]
fn native_profile_transitions_apply_model_effort_speed_and_access() -> Result<()> {
    let mut fixture = reload_responses_fixture::Fixture::start()?;
    let root = tempfile::Builder::new()
        .prefix("central-native-profile-")
        .tempdir()?;
    let native = root.path().join("native");
    let work = root.path().join("workspace");
    std::fs::create_dir_all(&native)?;
    std::fs::create_dir_all(&work)?;
    std::fs::write(
        native.join("config.toml"),
        format!(
            "model = \"gpt-5.6-luna\"\nmodel_provider = \"profile_fixture\"\nmodel_reasoning_summary = \"concise\"\n[model_providers.profile_fixture]\nname = \"Owned profile fixture\"\nbase_url = \"http://{}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = false\nsupports_websockets = false\nrequest_max_retries = 0\nstream_max_retries = 0\n",
            fixture.address
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
    let client = Client::spawn_command(command, "native-profile-probe", move |e| {
        let _ = tx.send(e);
    })?;
    let result = (|| -> Result<()> {
        loop {
            match rx.recv_timeout(Duration::from_secs(30))? {
                Event::Ready { .. } => break,
                Event::Closed { reason } => return Err(reason.into()),
                Event::ServerRequest { .. } => return Err("Unexpected authority request".into()),
                _ => {}
            }
        }
        if !api::account_read().send(&client)?.wait()?["account"].is_null() {
            return Err("Profile fixture must remain unauthenticated".into());
        }
        let cwd = work.to_str().ok_or("Non-UTF8 fixture cwd")?;
        let mut threads = Vec::new();
        for _ in 0..2 {
            let started = api::start_thread(cwd, &api::Profile::default(), api::Access::ReadOnly)
                .send(&client)?
                .wait()?;
            if !started["thread"]["path"]
                .as_str()
                .is_some_and(|p| std::path::Path::new(p).starts_with(&native))
                || started["sandbox"]["type"] != "readOnly"
            {
                return Err("Thread escaped the owned read-only fixture".into());
            }
            threads.push(
                started["thread"]["id"]
                    .as_str()
                    .ok_or("Missing thread")?
                    .to_owned(),
            );
        }
        let choices = [
            (
                "gpt-5.6-luna",
                "low",
                Some("fast"),
                api::Access::ReadOnly,
                "readOnly",
                "untrusted",
            ),
            (
                "gpt-5.6-sol",
                "high",
                None,
                api::Access::WorkspaceWrite,
                "workspaceWrite",
                "on-request",
            ),
            (
                "gpt-5.6-luna",
                "medium",
                Some("fast"),
                api::Access::FullAccess,
                "dangerFullAccess",
                "never",
            ),
            (
                "gpt-5.6-luna",
                "low",
                None,
                api::Access::ReadOnly,
                "readOnly",
                "untrusted",
            ),
        ];
        // Two independent native owners; alternate after each profile change.
        // Full access is exercised only against fixed local text, never a tool.
        let mut last_settings = vec![None; threads.len()];
        for (step, (model, effort, tier, access, sandbox, approval)) in
            choices.into_iter().enumerate()
        {
            for (owner, thread) in threads.iter().enumerate() {
                let profile = api::Profile {
                    model: Some(model.into()),
                    effort: Some(effort.into()),
                    service_tier: tier.map(str::to_owned),
                    personality: None,
                    summary: match step {
                        0 => None,
                        1 => Some(api::ReasoningSummary::Auto),
                        2 => Some(api::ReasoningSummary::Detailed),
                        _ => Some(api::ReasoningSummary::None),
                    },
                };
                complete_fixture_turn(
                    &client,
                    &rx,
                    api::start_turn(
                        thread,
                        &format!("profile-{owner}-{step}"),
                        vec![api::text_input("Reply READY; no tools or file access.")],
                        cwd,
                        &profile,
                        access,
                    ),
                )?;
                let current = api::resume_thread(thread).send(&client)?.wait()?;
                let settings = json!({"model":current["model"],"effort":current["reasoningEffort"],"tier":current["serviceTier"],"sandbox":current["sandbox"]["type"],"approval":current["approvalPolicy"]});
                if settings
                    != json!({"model":model,"effort":effort,"tier":if tier.is_some() { "priority" } else { "default" },"sandbox":sandbox,"approval":approval})
                {
                    return Err(format!(
                        "Native owner {owner} transition {step} settings mismatch: {settings}"
                    )
                    .into());
                }
                if std::path::Path::new(current["cwd"].as_str().ok_or("Missing cwd")?)
                    .canonicalize()?
                    != work.canonicalize()?
                {
                    return Err("Native cwd changed".into());
                }
                if access == api::Access::WorkspaceWrite {
                    let policy = &current["sandbox"];
                    if policy["networkAccess"] != false
                        || policy["excludeTmpdirEnvVar"] != true
                        || policy["excludeSlashTmp"] != true
                    {
                        return Err("Native workspace restrictions changed".into());
                    }
                    let roots = policy["writableRoots"]
                        .as_array()
                        .ok_or("Missing writable roots")?;
                    // 0.153.4 normalizes the cwd out of this additional-roots
                    // list. Check exact native readback, not a byte-for-byte
                    // echo of the outbound policy. This is not an OS sandbox
                    // enforcement test; no write command is executed here.
                    if !roots.is_empty() {
                        return Err("Native workspace policy gained additional write roots".into());
                    }
                }
                last_settings[owner] = Some(settings.clone());
                for (sibling, previous) in last_settings.iter().enumerate() {
                    if sibling != owner
                        && let Some(previous) = previous
                    {
                        let other = api::resume_thread(&threads[sibling])
                            .send(&client)?
                            .wait()?;
                        if json!({"model":other["model"],"effort":other["reasoningEffort"],"tier":other["serviceTier"],"sandbox":other["sandbox"]["type"],"approval":other["approvalPolicy"]})
                            != *previous
                        {
                            return Err("Changing one owner changed the sibling profile".into());
                        }
                    }
                }
                let history = api::read_thread(thread).send(&client)?.wait()?;
                if history["thread"]["turns"].as_array().map(Vec::len) != Some(step + 1) {
                    return Err("Profile change replaced or duplicated native history".into());
                }
                let requests = fixture.requests.lock().map_err(|_| "Poisoned fixture")?;
                if requests.len() != step * 2 + owner + 1 {
                    return Err("Unexpected number of model requests".into());
                }
                let actual = requests.last().ok_or("Missing model request")?;
                // Native App Server translates its fast tier into Responses priority.
                if actual["model"] != model
                    || actual["reasoning"]["effort"] != effort
                    || actual["reasoning"]["summary"]
                        != match step {
                            0 => json!("concise"),
                            1 => json!("auto"),
                            2 => json!("detailed"),
                            _ => json!(null),
                        }
                    || actual["service_tier"] != json!(tier.map(|_| "priority"))
                {
                    return Err(format!(
                        "Actual model request disagrees with selected profile: {actual}"
                    )
                    .into());
                }
                println!(
                    "Owner {owner}, transition {step}: native readback and model request match {settings}"
                );
            }
        }
        Ok(())
    })();
    client.shutdown();
    let finished = fixture.finish();
    result?;
    finished?;
    println!(
        "Eight native local-fixture turns verified; no account inference, tools or personal config; only owned temporary histories removed."
    );
    Ok(())
}
