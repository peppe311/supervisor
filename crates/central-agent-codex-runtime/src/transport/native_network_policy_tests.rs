//! Opt-in real native network-policy denial. Temporary config, loopback model
//! and a fixed reserved .invalid destination; no public service or network grant.
//! Diagnostic, not certified acceptance: recorded Windows loopback and reserved
//! host probes received command approval, but neither emitted a network callback.
use super::{native_exec_policy_fixture::shell, reload_responses_fixture, *};
use crate::{
    api,
    requests::{Decision, Requests},
};
use serde_json::{Value, json};
use std::{
    path::Path,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const INPUT: &str = "NATIVE_NETWORK_DENIAL_PORT=";
const HOST: &str = "central-agent-network-denial.invalid";
fn expression(port: u16) -> String {
    format!(
        "if (-not $env:HTTP_PROXY) {{ throw 'Native HTTP proxy was not configured' }}; (Invoke-WebRequest -UseBasicParsing -Method Post -Uri 'http://{HOST}:{port}/v1/responses' -Proxy $env:HTTP_PROXY -ContentType 'application/json' -Body '{{\"input\":[]}}').StatusCode"
    )
}

fn network_resolution(params: &Value, thread: &str, key: Option<&ServerRequestKey>) -> bool {
    params["threadId"] == thread
        && key.is_some_and(|key| {
            serde_json::from_value::<RequestId>(params["requestId"].clone())
                .ok()
                .as_ref()
                == Some(&key.id)
        })
}
fn owned_command(params: &Value, port: u16) -> bool {
    let Some(parts) = params["proposedExecpolicyAmendment"].as_array() else {
        return false;
    };
    if parts.len() != 4
        || parts[1] != "-NoProfile"
        || parts[2] != "-Command"
        || parts[3] != expression(port)
        || params["commandActions"] != json!([{"type":"unknown","command":expression(port)}])
    {
        return false;
    }
    let actual = parts[0]
        .as_str()
        .and_then(|p| Path::new(p).canonicalize().ok());
    let bundled = std::path::PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default())
        .join(
            ".cache/codex-runtimes/codex-primary-runtime/dependencies/native/powershell/pwsh.exe",
        );
    // Check the native complete argv and whole unknown-command action, not the
    // platform-quoted display string. The response allows once, never saves it.
    actual.is_some_and(|actual| {
        [shell(), bundled]
            .iter()
            .any(|p| p.canonicalize().ok().as_ref() == Some(&actual))
    })
}

fn output(body: &Value, index: usize) -> std::result::Result<Value, String> {
    let input = body["input"].as_array().ok_or("Missing input")?;
    let last = input
        .iter()
        .rposition(|i| i["role"] == "user")
        .ok_or("Missing fixture input")?;
    if input[last + 1..]
        .iter()
        .any(|i| i["type"] == "custom_tool_call_output" || i["type"] == "function_call_output")
    {
        return reload_responses_fixture::ready_output(body, index);
    }
    let text = input[last]["content"]
        .as_array()
        .and_then(|c| c.iter().find_map(|c| c["text"].as_str()))
        .ok_or("Missing fixture text")?;
    let port = text
        .strip_prefix(INPUT)
        .and_then(|p| p.parse::<u16>().ok())
        .filter(|p| *p > 0)
        .ok_or("Invalid owned target port")?;
    let command = expression(port);
    let args = json!({"cmd":command,"shell":shell(),"login":false});
    Ok(
        json!({"id":format!("network-{index}"),"type":"custom_tool_call","call_id":format!("network-call-{index}"),"name":"exec","namespace":"functions","input":format!("text(await tools.exec_command({args}));"),"status":"completed"}),
    )
}

#[test]
#[ignore = "Explicit native network proxy denial diagnostic; fixed .invalid host, owned profile, no network grant or Windows setup"]
fn native_network_policy_denial_uses_only_the_reserved_invalid_target() -> Result<()> {
    // Keep the old loopback destination as a sentinel: the fixed expression must
    // never fall back to it. Its empty trace alone does NOT prove a native denial.
    let mut target = reload_responses_fixture::Fixture::start()?;
    let mut model = reload_responses_fixture::Fixture::with_output(output)?;
    let proxy_reservation = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    let proxy_address = proxy_reservation.local_addr()?;
    let root = tempfile::Builder::new()
        .prefix("central-native-network-")
        .tempdir()?;
    let native = root.path().join("native");
    let work = root.path().join("workspace");
    std::fs::create_dir_all(&native)?;
    std::fs::create_dir_all(&work)?;
    std::fs::write(
        native.join("config.toml"),
        format!(
            "model = \"gpt-5.6-luna\"\nmodel_provider = \"network_fixture\"\napproval_policy = \"on-request\"\ndefault_permissions = \"network_fixture\"\n[features]\nnetwork_proxy = true\n[permissions.network_fixture]\nextends = \":read-only\"\n[permissions.network_fixture.network]\nenabled = true\nproxy_url = \"http://{proxy_address}\"\nenable_socks5 = false\n[model_providers.network_fixture]\nname = \"Owned network fixture\"\nbase_url = \"http://{}/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = false\nsupports_websockets = false\nrequest_max_retries = 0\nstream_max_retries = 0\n",
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
    drop(proxy_reservation);
    let client = Client::spawn_command(command, "owned-network-denial-probe", move |e| {
        let _ = tx.send(e);
    })?;
    let result = (|| -> Result<()> {
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
            return Err("Network probe must be unauthenticated".into());
        }
        // Omit legacy sandbox overrides to inspect the native configured profile;
        // this diagnostic does not add profile selection to production UI.
        let response = client
            .request(
                "thread/start",
                json!({"cwd":work,"approvalPolicy":"on-request","approvalsReviewer":"user"}),
            )?
            .wait()?;
        println!(
            "Native configured network profile returned sandbox kind: {}",
            response["sandbox"]["type"]
        );
        if !response["thread"]["path"]
            .as_str()
            .is_some_and(|p| Path::new(p).starts_with(&native))
            || !matches!(
                response["sandbox"]["type"].as_str(),
                Some("readOnly" | "workspaceWrite")
            )
        {
            return Err("Native network profile escaped the owned sandbox; no turn sent".into());
        }
        let thread = response["thread"]["id"]
            .as_str()
            .ok_or("Missing native thread")?;
        let ack=client.request("turn/start",json!({"threadId":thread,"input":[api::text_input(&format!("{INPUT}{}",target.address.port()))]}))?.wait()?;
        let turn = ack["turn"]["id"].as_str().ok_or("Missing native turn")?;
        let deadline = Instant::now() + Duration::from_secs(40);
        let mut requests = Requests::default();
        let mut approved_command = false;
        let mut denied = false;
        let mut resolved = false;
        let mut network_key = None;
        let mut command_failed = false;
        let mut failure = None;
        loop {
            match rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
                Event::ServerRequest {
                    key,
                    method,
                    params,
                } => {
                    if !approved_command
                        && method == "item/commandExecution/requestApproval"
                        && params["networkApprovalContext"].is_null()
                        && params["threadId"] == thread
                        && params["turnId"] == turn
                        && params["cwd"].as_str().is_some_and(|p| {
                            Path::new(p).canonicalize().ok() == work.canonicalize().ok()
                        })
                        && owned_command(&params, target.address.port())
                    {
                        requests.insert("chat:network-test", key, &method, params)?;
                        let view = requests
                            .views("chat:network-test")
                            .pop()
                            .ok_or("Missing command card")?;
                        let (key, answer) = requests.answer(
                            "chat:network-test",
                            &view.ticket,
                            Decision::Accept {},
                        )?;
                        client.answer(&key, Ok(answer))?;
                        approved_command = true;
                        println!(
                            "Accepted only the exact owned proxy-only command once; no shell rule or separate network-policy decision."
                        );
                        continue;
                    }
                    if denied
                        || method != "item/commandExecution/requestApproval"
                        || params["threadId"] != thread
                        || params["turnId"] != turn
                        || params["networkApprovalContext"]["host"] != HOST
                    {
                        return Err(format!("Unexpected request; no authority granted: method={method}, kind={}, network={}, actions={}, proposedPrefix={}",params["kind"],params["networkApprovalContext"],params["commandActions"],params["proposedExecpolicyAmendment"]).into());
                    }
                    let proposals = params["proposedNetworkPolicyAmendments"]
                        .as_array()
                        .ok_or("No native network proposals")?;
                    let index = proposals
                        .iter()
                        .position(|p| p == &json!({"host":HOST,"action":"deny"}))
                        .ok_or("No exact native reserved-host denial offered")?;
                    network_key = Some(key.clone());
                    requests.insert("chat:network-test", key, &method, params)?;
                    let view = requests
                        .views("chat:network-test")
                        .pop()
                        .ok_or("Missing network decision card")?;
                    if requests
                        .answer(
                            "graph:other",
                            &view.ticket,
                            Decision::NetworkPolicy { index },
                        )
                        .is_ok()
                    {
                        return Err("Network policy escaped owner".into());
                    }
                    let (key, answer) = requests.answer(
                        "chat:network-test",
                        &view.ticket,
                        Decision::NetworkPolicy { index },
                    )?;
                    client.answer(&key, Ok(answer))?;
                    denied = true;
                }
                Event::Notification { method, params } => {
                    if method == "serverRequest/resolved" {
                        let owners = requests.notify(&method, &params);
                        resolved |= network_resolution(&params, thread, network_key.as_ref())
                            && owners == vec!["chat:network-test".to_owned()];
                    }
                    if method == "item/completed"
                        && params["item"]["type"] == "commandExecution"
                        && params["threadId"] == thread
                        && params["turnId"] == turn
                    {
                        command_failed = params["item"]["status"] == "declined"
                            || (params["item"]["status"] == "failed"
                                && params["item"]["exitCode"].as_i64().is_some_and(|c| c != 0));
                        failure = Some(
                            params["item"]["aggregatedOutput"]
                                .as_str()
                                .unwrap_or("")
                                .to_owned(),
                        );
                        println!(
                            "Native network command status: {}",
                            params["item"]["status"]
                        );
                    }
                    if method == "turn/completed"
                        && params["turn"]["id"] == turn
                        && params["threadId"] == thread
                    {
                        break;
                    }
                }
                Event::Closed { reason } => return Err(reason.into()),
                _ => {}
            }
        }
        if !command_failed {
            return Err("Native denial did not produce a failed or declined command".into());
        }
        if !denied || !resolved {
            return Err(format!(
                "No resolved native network denial; command result: {}",
                failure.as_deref().unwrap_or("not reported")
            )
            .into());
        }
        if !target
            .requests
            .lock()
            .map_err(|_| "Target fixture lock poisoned")?
            .is_empty()
        {
            return Err("Denied network target received a request".into());
        }
        if std::fs::read_dir(&work)?.next().is_some() {
            return Err("Network fixture changed workspace files".into());
        }
        Ok(())
    })();
    client.shutdown();
    let model_result = model.finish();
    let target_result = target.finish();
    println!(
        "Owned endpoint observations: {} model requests, {} loopback sentinel requests.",
        model
            .requests
            .lock()
            .map_err(|_| "Model trace poisoned")?
            .len(),
        target
            .requests
            .lock()
            .map_err(|_| "Target trace poisoned")?
            .len()
    );
    result?;
    model_result?;
    target_result?;
    println!(
        "PASS: actual native network deny amendment and resolution, correct owner, zero target requests; no network grant, account inference or personal configuration."
    );
    Ok(())
}

#[test]
fn network_probe_command_authority_is_exact_and_once_only() {
    let port = 41827;
    let mut params = json!({"proposedExecpolicyAmendment":[shell(),"-NoProfile","-Command",expression(port)],"commandActions":[{"type":"unknown","command":expression(port)}]});
    assert!(owned_command(&params, port));
    assert!(!owned_command(&params, port + 1));
    params["proposedExecpolicyAmendment"]
        .as_array_mut()
        .unwrap()
        .push(json!("extra"));
    assert!(!owned_command(&params, port));
    params["proposedExecpolicyAmendment"]
        .as_array_mut()
        .unwrap()
        .pop();
    params["commandActions"][0]["command"] = json!("another command");
    assert!(!owned_command(&params, port));
}

#[test]
fn network_probe_resolution_cannot_accept_the_prerequisite_command_or_another_owner() {
    let key = ServerRequestKey {
        generation: 1,
        id: RequestId::Integer(7),
    };
    assert!(network_resolution(
        &json!({"threadId":"thread","requestId":7}),
        "thread",
        Some(&key)
    ));
    for params in [
        json!({"threadId":"thread","requestId":6}),
        json!({"threadId":"sibling","requestId":7}),
        json!({"threadId":"thread","requestId":"7"}),
        json!({"threadId":"thread"}),
    ] {
        assert!(!network_resolution(&params, "thread", Some(&key)));
    }
    assert!(!network_resolution(
        &json!({"threadId":"thread","requestId":7}),
        "thread",
        None
    ));
}

#[test]
fn network_probe_never_interpolates_non_port_user_text() {
    let request = |text: &str| json!({"input":[{"role":"user","content":[{"type":"input_text","text":text}]}]});
    for text in [
        "NATIVE_NETWORK_DENIAL_PORT=0",
        "NATIVE_NETWORK_DENIAL_PORT=65536",
        "NATIVE_NETWORK_DENIAL_PORT=80;Write-Output injected",
        "http://outside.example",
    ] {
        assert!(output(&request(text), 1).is_err());
    }
    let actual = output(&request("NATIVE_NETWORK_DENIAL_PORT=41827"), 1).unwrap();
    let args = json!({"cmd":expression(41827),"shell":shell(),"login":false});
    assert!(expression(41827).contains("http://central-agent-network-denial.invalid:41827/"));
    assert!(!expression(41827).contains("127.0.0.1"));
    assert_eq!(
        actual["input"],
        format!("text(await tools.exec_command({args}));")
    );
}
