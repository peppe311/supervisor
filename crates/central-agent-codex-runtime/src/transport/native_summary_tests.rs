//! Real App Server SSE-to-JSON-RPC summary translation against a loopback fixture.
//! No account inference, tools, personal configuration or visual test.
use super::reload_responses_fixture;
use crate::{
    api,
    mirror::Mirror,
    runtime::Runtime,
    transport::{Client, Event},
};
use serde_json::json;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

#[test]
#[ignore = "Runs official App Server against an owned local SSE fixture; no account inference"]
fn native_public_summaries_stream_before_final_and_survive_history_read()
-> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = reload_responses_fixture::Fixture::with_summary_stream()?;
    let root = tempfile::Builder::new()
        .prefix("central-native-summary-")
        .tempdir()?;
    let native = root.path().join("native");
    let work = root.path().join("workspace");
    std::fs::create_dir_all(&native)?;
    std::fs::create_dir_all(&work)?;
    std::fs::write(
        native.join("config.toml"),
        format!(
            "model = \"gpt-5.6-luna\"\nmodel_provider = \"summary_fixture\"\n[model_providers.summary_fixture]\nname = \"Owned summary stream\"\nbase_url = \"http://{}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = false\nsupports_websockets = false\nrequest_max_retries = 0\nstream_max_retries = 0\n",
            fixture.address
        ),
    )?;
    let original = std::fs::read(native.join("config.toml"))?;
    let mut command = Runtime::discover(&work)?.command();
    command
        .env("CODEX_HOME", &native)
        .env_remove("CODEX_SQLITE_HOME")
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY");
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn_command(command, "native-summary-probe", move |event| {
        let _ = tx.send(event);
    })?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        loop {
            match rx.recv_timeout(Duration::from_secs(30))? {
                Event::Ready { .. } => break,
                Event::Closed { reason } => return Err(reason.into()),
                Event::ServerRequest { .. } => return Err("Unexpected authority request".into()),
                _ => {}
            }
        }
        assert!(api::account_read().send(&client)?.wait()?["account"].is_null());
        let cwd = work.to_str().ok_or("Non-UTF8 fixture path")?;
        let profile = api::Profile {
            summary: Some(api::ReasoningSummary::Auto),
            effort: Some("low".into()),
            ..Default::default()
        };
        let mut mirror = Mirror::default();
        let mut completed_threads: Vec<String> = Vec::new();
        for owner in ["main", "graph"] {
            let started = api::start_thread(cwd, &profile, api::Access::ReadOnly)
                .send(&client)?
                .wait()?;
            assert!(
                std::path::Path::new(
                    started["thread"]["path"]
                        .as_str()
                        .ok_or("Missing native path")?
                )
                .starts_with(&native)
            );
            let thread = started["thread"]["id"].as_str().ok_or("Missing thread")?;
            let ack = api::start_turn(
                thread,
                owner,
                vec![api::text_input("Reply using the local test stream.")],
                cwd,
                &profile,
                api::Access::ReadOnly,
            )
            .send(&client)?
            .wait()?;
            let turn = ack["turn"]["id"].as_str().ok_or("Missing turn")?;
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut summary_deltas = Vec::new();
            let mut commentary_deltas = String::new();
            let mut parts = 0;
            let mut reasoning_id = None;
            loop {
                match rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
                    Event::Closed { reason } => return Err(reason.into()),
                    Event::ServerRequest { .. } => {
                        return Err("Unexpected authority request; no approval granted".into());
                    }
                    Event::Notification { method, params } => {
                        if params["threadId"] != thread {
                            continue;
                        }
                        mirror.notify(&method, &params)?;
                        if method == "item/reasoning/summaryTextDelta" {
                            let state = mirror.thread(thread).ok_or("Missing summary thread")?;
                            assert!(
                                state.active_turn().is_some(),
                                "Summary arrived only after completion"
                            );
                            assert!(
                                !state
                                    .turns
                                    .last()
                                    .unwrap()
                                    .items
                                    .iter()
                                    .any(|i| i.value["phase"] == "final_answer")
                            );
                            let id = params["itemId"]
                                .as_str()
                                .ok_or("Missing reasoning item")?
                                .to_owned();
                            if let Some(previous) = &reasoning_id {
                                assert_eq!(previous, &id);
                            } else {
                                reasoning_id = Some(id);
                            }
                            let text = params["delta"].as_str().ok_or("Missing summary delta")?;
                            summary_deltas.push(text.to_owned());
                        }
                        if method == "item/reasoning/summaryPartAdded" {
                            parts += 1;
                        }
                        if method == "item/agentMessage/delta" {
                            assert!(mirror.thread(thread).unwrap().active_turn().is_some());
                            commentary_deltas
                                .push_str(params["delta"].as_str().ok_or("Missing message delta")?);
                        }
                        if method == "turn/completed" && params["turn"]["id"] == turn {
                            assert_eq!(params["turn"]["status"], "completed");
                            break;
                        }
                    }
                    _ => {}
                }
            }
            assert_eq!(
                summary_deltas,
                ["Checking ", "the fixture.", "Ready ", "to answer."]
            );
            assert_eq!(parts, 2);
            assert_eq!(commentary_deltas, "Fixture checked.");
            let history = api::read_thread(thread).send(&client)?.wait()?;
            let mut restored = Mirror::default();
            restored.hydrate(&history["thread"], 0)?;
            for state in [&mirror, &restored] {
                let items = &state
                    .thread(thread)
                    .ok_or("Missing thread")?
                    .turns
                    .last()
                    .ok_or("Missing turn")?
                    .items;
                let reasoning = items
                    .iter()
                    .find(|i| i.value["type"] == "reasoning")
                    .ok_or("Missing public summaries")?;
                assert_eq!(Some(&reasoning.id), reasoning_id.as_ref());
                assert_eq!(
                    reasoning.value["summary"],
                    json!(["Checking the fixture.", "Ready to answer."])
                );
                assert!(reasoning.value.get("content").is_none());
                assert!(
                    items.iter().any(|i| i.value["phase"] == "commentary"
                        && i.value["text"] == "Fixture checked.")
                );
                assert!(
                    items
                        .iter()
                        .any(|i| i.value["phase"] == "final_answer" && i.value["text"] == "READY")
                );
            }
            for prior in &completed_threads {
                assert!(mirror.thread(prior).unwrap().active_turn().is_none());
            }
            completed_threads.push(thread.to_owned());
            println!(
                "{owner}: four public-summary deltas, two parts and commentary preceded the final result; live mirror and native history agree."
            );
        }
        let requests = fixture
            .requests
            .lock()
            .map_err(|_| "Fixture lock poisoned")?;
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|r| r["reasoning"]["summary"] == "auto"));
        assert_eq!(std::fs::read(native.join("config.toml"))?, original);
        Ok(())
    })();
    client.shutdown();
    let finished = fixture.finish();
    result?;
    finished?;
    Ok(())
}
