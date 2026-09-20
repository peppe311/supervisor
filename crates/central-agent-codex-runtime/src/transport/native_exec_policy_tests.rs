//! Opt-in native command-policy persistence probe. The local model fixture can
//! request only one exact print expression; App Server owns command execution,
//! approval and saved rules. No account inference or personal profile is used.
use super::{native_exec_policy_fixture::*, reload_responses_fixture, *};
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

fn spawn(native: &Path, work: &Path) -> Result<(Client, mpsc::Receiver<Event>)> {
    let mut command = Runtime::discover(work)?.command();
    command
        .env("CODEX_HOME", native)
        .env_remove("CODEX_SQLITE_HOME")
        .env_remove("CODEX_ACCESS_TOKEN")
        .env_remove("OPENAI_API_KEY")
        .env_remove("CODEX_API_KEY");
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn_command(command, "owned-command-policy-probe", move |e| {
        let _ = tx.send(e);
    })?;
    loop {
        match rx.recv_timeout(Duration::from_secs(30))? {
            Event::Ready { .. } => break,
            Event::Closed { reason } => return Err(reason.into()),
            Event::ServerRequest { .. } => {
                return Err("Unexpected startup authority request".into());
            }
            _ => {}
        }
    }
    if !api::account_read().send(&client)?.wait()?["account"].is_null() {
        return Err("Native policy fixture must be unauthenticated".into());
    }
    Ok((client, rx))
}

fn run(
    client: &Client,
    rx: &mpsc::Receiver<Event>,
    native: &Path,
    work: &Path,
    allow_rule: bool,
) -> Result<()> {
    let owner = "chat:policy-fixture";
    let other = "graph:policy-sibling";
    let cwd = work.to_str().ok_or("Invalid owned cwd")?;
    let started = api::start_thread(cwd, &api::Profile::default(), api::Access::WorkspaceWrite)
        .send(client)?
        .wait()?;
    if !started["thread"]["path"]
        .as_str()
        .is_some_and(|p| Path::new(p).starts_with(native))
    {
        return Err("Unowned native history".into());
    }
    let thread = started["thread"]["id"].as_str().ok_or("Missing thread")?;
    let ack = api::start_turn(
        thread,
        "policy-probe",
        vec![api::text_input(INPUT)],
        cwd,
        &api::Profile::default(),
        api::Access::WorkspaceWrite,
    )
    .send(client)?
    .wait()?;
    let turn = ack["turn"]["id"].as_str().ok_or("Missing turn")?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut requests = Requests::default();
    let mut key = None;
    let mut resolved = false;
    let mut item = None;
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
            Event::ServerRequest {
                key: incoming,
                method,
                params,
            } => {
                if !allow_rule
                    || key.is_some()
                    || method != "item/commandExecution/requestApproval"
                    || params["threadId"] != thread
                    || params["turnId"] != turn
                    || !owned_policy(&params)
                    || params["cwd"].as_str().is_none_or(|p| Path::new(p) != work)
                {
                    return Err(format!(
                        "Unexpected approval; no authority granted: {method} {params}"
                    )
                    .into());
                }
                let policy = params["proposedExecpolicyAmendment"].clone();
                requests.insert(owner, incoming.clone(), &method, params)?;
                let view = requests.views(owner).pop().ok_or("Missing request card")?;
                let choices = view.choices.as_ref().ok_or("Missing native choices")?;
                if !choices.accept
                    || !choices.cancel
                    || !choices.exec_policy
                    || choices.session
                    || choices.decline
                    || requests
                        .answer(owner, &view.ticket, Decision::Session {})
                        .is_ok()
                    || requests
                        .answer(owner, &view.ticket, Decision::Decline {})
                        .is_ok()
                {
                    return Err(
                        "Client offered a choice excluded by the native command request".into(),
                    );
                }
                if !requests.views(other).is_empty()
                    || requests
                        .answer(other, &view.ticket, Decision::ExecPolicy {})
                        .is_ok()
                {
                    return Err("Command rule escaped its owner".into());
                }
                let (actual, response) =
                    requests.answer(owner, &view.ticket, Decision::ExecPolicy {})?;
                if response
                    != json!({"decision":{"acceptWithExecpolicyAmendment":{"execpolicy_amendment":policy}}})
                {
                    return Err("Client changed native rule".into());
                }
                client.answer(&actual, Ok(response))?;
                key = Some(incoming);
            }
            Event::Notification { method, params } => {
                if method == "serverRequest/resolved" {
                    if resolved
                        || params["threadId"] != thread
                        || !key
                            .as_ref()
                            .is_some_and(|k| json!(k.id) == params["requestId"])
                        || requests.notify(&method, &params) != vec![owner.to_owned()]
                    {
                        return Err("Wrong command request resolution".into());
                    }
                    resolved = true;
                }
                if method == "item/started"
                    && !matches!(
                        params["item"]["type"].as_str(),
                        Some("userMessage" | "agentMessage" | "reasoning" | "commandExecution")
                    )
                {
                    return Err("Unexpected tool in print-only fixture".into());
                }
                if method == "item/completed" && params["item"]["type"] == "commandExecution" {
                    if item.is_some()
                        || params["threadId"] != thread
                        || params["turnId"] != turn
                        || params["item"]["status"] != "completed"
                        || params["item"]["exitCode"] != 0
                        || params["item"]["aggregatedOutput"]
                            .as_str()
                            .is_none_or(|s| s.trim() != MARKER)
                    {
                        return Err(format!("Print-only command failed: {}", params["item"]).into());
                    }
                    item = Some(params["item"].clone());
                }
                if method == "turn/completed"
                    && params["threadId"] == thread
                    && params["turn"]["id"] == turn
                {
                    if params["turn"]["status"] != "completed" {
                        return Err(
                            format!("Native turn failed: {}", params["turn"]["error"]).into()
                        );
                    }
                    break;
                }
            }
            Event::Closed { reason } => return Err(reason.into()),
            _ => {}
        }
    }
    if key.is_some() != allow_rule || resolved != allow_rule || !requests.views(owner).is_empty() {
        return Err("Incorrect native approval lifecycle".into());
    }
    let item = item.ok_or("Missing actual command execution")?;
    let history = api::read_thread(thread).send(client)?.wait()?;
    if !history["thread"]["turns"].as_array().is_some_and(|ts| {
        ts.len() == 1
            && ts[0]["id"] == turn
            && ts[0]["items"].as_array().is_some_and(|items| {
                items.contains(&item)
                    && items
                        .iter()
                        .any(|i| i["type"] == "agentMessage" && i["text"] == "READY")
            })
    }) {
        return Err("Missing persisted command and continuation".into());
    }
    Ok(())
}

#[test]
#[ignore = "Owned native print-only execpolicy amendment and restart persistence; no account inference or personal rules"]
fn native_execpolicy_amendment_is_saved_and_reused_after_restart() -> Result<()> {
    let mut fixture = reload_responses_fixture::Fixture::with_output(model_output)?;
    let root = tempfile::Builder::new()
        .prefix("central-native-execpolicy-")
        .tempdir()?;
    let native = root.path().join("native");
    let work = root.path().join("workspace");
    std::fs::create_dir_all(&native)?;
    std::fs::create_dir_all(&work)?;
    std::fs::write(
        native.join("config.toml"),
        format!(
            "model = \"gpt-5.6-luna\"\nmodel_provider = \"policy_fixture\"\n[model_providers.policy_fixture]\nname = \"Owned command policy fixture\"\nbase_url = \"http://{}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = false\nsupports_websockets = false\nrequest_max_retries = 0\nstream_max_retries = 0\n",
            fixture.address
        ),
    )?;
    let result = (|| -> Result<()> {
        let (client, rx) = spawn(&native, &work)?;
        let first = run(&client, &rx, &native, &work, true);
        client.shutdown();
        first?;
        println!("Native exact print-rule approval, resolution and persisted command passed.");
        let (client, rx) = spawn(&native, &work)?;
        let second = run(&client, &rx, &native, &work, false);
        client.shutdown();
        second?;
        println!("Fresh App Server reused its saved rule without a client approval decision.");
        if std::fs::read_dir(&work)?.next().is_some()
            || fixture
                .requests
                .lock()
                .map_err(|_| "Fixture lock poisoned")?
                .len()
                != 4
        {
            return Err("Unexpected workspace writes or model requests".into());
        }
        Ok(())
    })();
    let finished = fixture.finish();
    if let Err(e) = &finished {
        eprintln!("Local model fixture: {e}");
    }
    result?;
    finished?;
    Ok(())
}

#[test]
fn execpolicy_fixture_never_interpolates_user_code() {
    let mut body = json!({"input":[{"type":"additional_tools","tools":[{"type":"namespace","name":"functions","tools":[{"type":"custom","name":"exec"}]}]},{"role":"user","content":INPUT}]});
    let output = model_output(&body, 1).unwrap();
    assert_eq!(output["type"], "custom_tool_call");
    let expected = json!({"cmd":expression(),"shell":shell(),"login":false,"sandbox_permissions":"require_escalated","justification":"Allow this exact owned print-only policy fixture?","prefix_rule":prefix()});
    assert_eq!(
        output["input"],
        format!("text(await tools.exec_command({expected}));")
    );
    body["input"][1]["content"] = json!(format!("{INPUT}; Write-Output not-executed"));
    assert_eq!(model_output(&body, 1).unwrap(), output);
    body["input"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type":"custom_tool_call_output","output":"test"}));
    assert_eq!(
        model_output(&body, 2).unwrap()["content"][0]["text"],
        "READY"
    );
    body["input"][1]["content"] = json!("unrelated");
    assert!(model_output(&body, 3).is_err());
}

#[test]
fn execpolicy_probe_never_accepts_a_broader_shell_rule() {
    let executable = shell().display().to_string();
    let params = json!({"proposedExecpolicyAmendment":prefix(),
        "command":format!("{} -NoProfile -Command \"{}\"",json!(executable),expression()),
        "commandActions":[{"type":"unknown","command":expression()}]});
    assert!(owned_policy(&params));
    for policy in [
        json!([shell()]),
        json!([shell(), "-NoProfile", "-Command"]),
        json!([
            shell(),
            "-NoProfile",
            "-Command",
            format!("{}; Write-Output extra", expression())
        ]),
        json!([shell(), "-NoProfile", "-Command", expression(), "extra"]),
        json!(["unknown.exe", "-NoProfile", "-Command", expression()]),
    ] {
        let mut changed = params.clone();
        changed["proposedExecpolicyAmendment"] = policy;
        assert!(!owned_policy(&changed));
    }
    let mut changed = params.clone();
    changed["command"] = json!(format!(
        "{}; Write-Output extra",
        params["command"].as_str().unwrap()
    ));
    assert!(!owned_policy(&changed));
    let mut changed = params;
    changed["commandActions"] = json!([]);
    assert!(!owned_policy(&changed));
}
