//! Isolated native review diagnostic: fixed print-only model tool output, never
//! a fabricated approval callback or an elevation request. No account inference.
use super::{native_exec_policy_fixture, reload_responses_fixture, *};
use crate::{
    api,
    requests::{Decision, Requests},
};
use serde_json::json;
use std::{
    path::Path,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const INPUT: &str = "CENTRAL_REVIEW_POLICY_FIXTURE";
const PRINT: &str = "[string]::Concat('CENTRAL_REVIEW_PRINT_ONLY')";

fn output(body: &Value, index: usize) -> std::result::Result<Value, String> {
    let input = body["input"].as_array().ok_or("Missing review input")?;
    let last = input
        .iter()
        .rposition(|i| i["role"] == "user")
        .ok_or("Missing review user input")?;
    if !input[last].to_string().contains(INPUT) || index > 2 {
        return Err("Unexpected review fixture input; no additional command emitted".into());
    }
    if input[last + 1..].iter().any(|i| {
        matches!(
            i["type"].as_str(),
            Some("custom_tool_call_output" | "function_call_output")
        )
    }) {
        for item in &input[last + 1..] {
            if matches!(
                item["type"].as_str(),
                Some("custom_tool_call_output" | "function_call_output")
            ) {
                println!("Owned native review tool result: {}", item["output"]);
            }
        }
        let review = json!({"findings":[],"overall_correctness":"patch is correct","overall_explanation":"Owned print-only diagnostic; no source changes were requested.","overall_confidence_score":1.0});
        return Ok(
            json!({"id":format!("review-{index}"),"type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":review.to_string(),"annotations":[]}],"phase":"final_answer"}),
        );
    }
    // Deliberately no sandbox_permissions or prefix_rule: observe the native
    // review policy without trying to override it or request wider authority.
    let args = json!({"cmd":PRINT,"shell":native_exec_policy_fixture::shell(),"login":false});
    Ok(
        json!({"id":"review-print","type":"custom_tool_call","call_id":"review-print-call","name":"exec","namespace":"functions","input":format!("text(await tools.exec_command({args}));"),"status":"completed"}),
    )
}

#[test]
#[ignore = "Explicit real native review approval diagnostic; owned profile, fixed local print command, no grants or account inference"]
fn native_review_print_only_observes_actual_approval_policy() -> Result<()> {
    let mut model = reload_responses_fixture::Fixture::with_output(output)?;
    let root = tempfile::Builder::new()
        .prefix("central-native-review-policy-")
        .tempdir()?;
    let native = root.path().join("native");
    let work = root.path().join("workspace");
    std::fs::create_dir_all(&native)?;
    std::fs::create_dir_all(&work)?;
    std::fs::write(
        native.join("config.toml"),
        format!(
            "model = \"gpt-5.6-luna\"\nmodel_provider = \"review_fixture\"\n[model_providers.review_fixture]\nname = \"Owned review fixture\"\nbase_url = \"http://{}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = false\nsupports_websockets = false\nrequest_max_retries = 0\nstream_max_retries = 0\n",
            model.address
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
    let client = Client::spawn_command(command, "native-review-policy-probe", move |e| {
        let _ = tx.send(e);
    })?;
    let result = (|| -> Result<()> {
        loop {
            match rx.recv_timeout(Duration::from_secs(30))? {
                Event::Ready { .. } => break,
                Event::Closed { reason } => return Err(reason.into()),
                Event::ServerRequest { .. } => return Err("Unexpected startup authority".into()),
                _ => {}
            }
        }
        if !api::account_read().send(&client)?.wait()?["account"].is_null() {
            return Err("Review fixture must be unauthenticated".into());
        }
        let started = api::start_thread(
            work.to_str().ok_or("Invalid owned cwd")?,
            &api::Profile::default(),
            api::Access::ReadOnly,
        )
        .send(&client)?
        .wait()?;
        if started["sandbox"]["type"] != "readOnly"
            || !started["thread"]["path"]
                .as_str()
                .is_some_and(|p| Path::new(p).starts_with(&native))
        {
            return Err("Native review escaped owned read-only profile".into());
        }
        let thread = started["thread"]["id"]
            .as_str()
            .ok_or("Missing native thread")?;
        let ack = api::start_review(
            thread,
            &api::ReviewTarget::Custom {
                instructions: INPUT.into(),
            },
        )
        .send(&client)?
        .wait()?;
        if ack["reviewThreadId"] != thread {
            return Err("Unexpected detached review".into());
        }
        let review = ack["turn"]["id"].as_str().ok_or("Missing review turn")?;
        let deadline = Instant::now() + Duration::from_secs(40);
        let mut requests = Requests::default();
        let mut denied_key = None;
        let mut resolved = false;
        let mut command_item = None;
        let mut exited = false;
        loop {
            match rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
                Event::ServerRequest {
                    key,
                    method,
                    params,
                } => {
                    if denied_key.is_some()
                        || method != "item/commandExecution/requestApproval"
                        || params["threadId"] != thread
                    {
                        return Err("Unexpected review authority request; no grant sent".into());
                    }
                    // Refuse only: no command or network authority is granted.
                    requests.insert("chat:review-probe", key.clone(), &method, params)?;
                    let view = requests
                        .views("chat:review-probe")
                        .pop()
                        .ok_or("Missing review request")?;
                    let choice = if view.choices.as_ref().is_some_and(|c| c.decline) {
                        Decision::Decline {}
                    } else {
                        Decision::Cancel {}
                    };
                    if requests
                        .answer("graph:other", &view.ticket, choice.clone())
                        .is_ok()
                    {
                        return Err("Review request escaped owner".into());
                    }
                    let (key, answer) =
                        requests.answer("chat:review-probe", &view.ticket, choice)?;
                    client.answer(&key, Ok(answer))?;
                    denied_key = Some(key);
                }
                Event::Notification { method, params } if params["threadId"] == thread => {
                    if method == "serverRequest/resolved" {
                        resolved |= denied_key
                            .as_ref()
                            .is_some_and(|k| json!(k.id) == params["requestId"])
                            && requests.notify(&method, &params)
                                == vec!["chat:review-probe".to_owned()];
                    }
                    if method == "item/completed" && params["item"]["type"] == "commandExecution" {
                        println!(
                            "Native review command: status={}, exitCode={}, output={}",
                            params["item"]["status"],
                            params["item"]["exitCode"],
                            params["item"]["aggregatedOutput"]
                        );
                        command_item = Some(params["item"].clone());
                    }
                    if method == "item/completed" && params["item"]["type"] == "exitedReviewMode" {
                        exited = true;
                    }
                    if method == "turn/completed" && params["turn"]["id"] == review {
                        break;
                    }
                }
                Event::Closed { reason } => return Err(reason.into()),
                _ => {}
            }
        }
        let history = api::read_thread(thread).send(&client)?.wait()?;
        println!(
            "Native review evidence: approval={}, resolved={}, exitedReviewMode={}, persistedTurns={}",
            denied_key.is_some(),
            resolved,
            exited,
            history["thread"]["turns"].as_array().map_or(0, Vec::len)
        );
        if std::fs::read_dir(&work)?.next().is_some() {
            return Err("Review fixture changed workspace".into());
        }
        if denied_key.is_none() || !resolved {
            return Err("No resolved native review approval; diagnostic does not certify review approval acceptance".into());
        }
        if command_item
            .as_ref()
            .is_some_and(|i| i["status"] == "completed" && i["exitCode"] == 0)
        {
            return Err("Refused review command executed successfully".into());
        }
        Ok(())
    })();
    client.shutdown();
    let fixture_result = model.finish();
    result?;
    fixture_result?;
    Ok(())
}

#[test]
fn review_model_fixture_has_no_elevation_or_arbitrary_command() {
    let body = json!({"input":[{"role":"user","content":[{"text":INPUT}]}]});
    let item = output(&body, 1).unwrap();
    let args = json!({"cmd":PRINT,"shell":native_exec_policy_fixture::shell(),"login":false});
    assert_eq!(
        item["input"],
        format!("text(await tools.exec_command({args}));")
    );
    assert!(output(&body, 3).is_err());
    assert!(
        output(
            &json!({"input":[{"role":"user","content":[{"text":"other"}]}]}),
            1
        )
        .is_err()
    );
}
