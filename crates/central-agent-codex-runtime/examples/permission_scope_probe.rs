//! Explicit one-prompt native permission probe. Never grants permissions or runs
//! shell commands. Deletes only the native history created by this process.
use central_agent_codex_runtime::{
    api,
    requests::{Decision, Kind, Requests},
    runtime::Runtime,
    transport::{Client, Event},
    wire::RequestId,
};
use serde_json::{Value, json};
use std::{path::Path, sync::mpsc};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn native_availability(client: &Client) -> Result<bool> {
    let mut cursor: Option<String> = None;
    let mut seen = std::collections::HashSet::new();
    let mut permission_ready = false;
    loop {
        let response = client
            .request(
                "experimentalFeature/list",
                json!({"cursor":cursor,"limit":20}),
            )?
            .wait()?;
        for feature in response["data"]
            .as_array()
            .ok_or("Invalid native feature inventory")?
        {
            if matches!(
                feature["name"].as_str(),
                Some(
                    "request_permissions_tool"
                        | "default_mode_request_user_input"
                        | "tool_call_mcp_elicitation"
                )
            ) {
                println!(
                    "Native availability: {}",
                    json!({"name":feature["name"],"stage":feature["stage"],"enabled":feature["enabled"]})
                );
            }
            if feature["name"] == "request_permissions_tool" {
                permission_ready = feature["stage"] == "stable" && feature["enabled"] == true;
            }
        }
        match &response["nextCursor"] {
            Value::Null => break,
            Value::String(next) if !next.is_empty() && seen.insert(next.clone()) => {
                cursor = Some(next.clone())
            }
            _ => return Err("Invalid or repeated native feature cursor".into()),
        }
    }
    Ok(permission_ready)
}

fn validate_scope(params: &Value, thread: &str, turn: &str, directory: &Path) -> Result<()> {
    if params["threadId"] != thread || params["turnId"] != turn {
        return Err("Permission request belongs to an unexpected native turn".into());
    }
    let profile = &params["permissions"];
    if !profile["network"].is_null() {
        return Err("This probe must not request network permission".into());
    }
    let fs = &profile["fileSystem"];
    let paths = fs["write"]
        .as_array()
        .ok_or("Missing exact write request")?;
    if paths.len() != 1
        || !fs["read"].as_array().is_none_or(Vec::is_empty)
        || !fs["entries"].as_array().is_none_or(Vec::is_empty)
        || !fs["globScanMaxDepth"].is_null()
    {
        return Err("Unexpected extra permission scope".into());
    }
    let path = Path::new(paths[0].as_str().ok_or("Invalid requested write path")?);
    if !path.is_absolute() || path.canonicalize()? != directory.canonicalize()? {
        return Err("Requested path is not the disposable probe directory".into());
    }
    Ok(())
}

fn main() -> Result<()> {
    let availability_only = std::env::args().any(|arg| arg == "--check-availability");
    if !availability_only && !std::env::args().any(|arg| arg == "--allow-test-inference") {
        return Err("Pass --allow-test-inference: one account-backed prompt, denial only, disposable native history.".into());
    }
    let workspace = tempfile::Builder::new()
        .prefix("central-permission-work-")
        .tempdir()?;
    let requested = tempfile::Builder::new()
        .prefix("central-permission-scope-")
        .tempdir()?;
    let runtime = Runtime::discover(workspace.path())?;
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn(&runtime, env!("CARGO_PKG_VERSION"), move |event| {
        let _ = tx.send(event);
    })?;
    let mut owned_thread = None;
    let mut active_turn = None;
    let outcome = (|| -> Result<()> {
        loop {
            match rx.recv()? {
                Event::Ready { .. } => break,
                Event::Closed { reason } => return Err(reason.into()),
                Event::ServerRequest { .. } => return Err("Unexpected startup request".into()),
                _ => {}
            }
        }
        let permission_ready = native_availability(&client)?;
        if availability_only {
            println!(
                "Availability inspection complete: no model prompt, native thread or configuration write."
            );
            return Ok(());
        }
        if !permission_ready {
            return Err("request_permissions_tool is not both stable and enabled; no test prompt submitted and no feature was enabled.".into());
        }
        let requirements = api::requirements().send(&client)?.wait()?;
        api::Access::ReadOnly.validate_requirements(requirements.get("requirements"))?;
        let profile = api::Profile::default();
        let cwd = workspace.path().to_str().ok_or("Non-UTF8 test directory")?;
        let start = api::start_thread(cwd, &profile, api::Access::ReadOnly)
            .send(&client)?
            .wait()?;
        let thread = start["thread"]["id"]
            .as_str()
            .ok_or("Missing native thread ID")?
            .to_owned();
        owned_thread = Some(thread.clone());
        println!("Owned native permission probe: {thread}");
        let prompt = format!(
            "Native client acceptance test. Call the built-in request_permissions tool exactly once, requesting only filesystem write permission for this existing directory: {}. Do not request network, reads, any other directory, or any other permission. This is a denial test: after the answer, do not retry or perform any work. Do not invoke shell, file, web, MCP, or any other tool; do not inspect or modify files or configuration. If request_permissions is unavailable, reply exactly PERMISSION_TOOL_UNAVAILABLE and stop. Otherwise, after receiving the decision reply exactly PERMISSION_DECISION_RECEIVED and stop.",
            requested.path().display()
        );
        let response = api::start_turn(
            &thread,
            "permission-probe-message",
            vec![api::text_input(&prompt)],
            cwd,
            &profile,
            api::Access::ReadOnly,
        )
        .send(&client)?
        .wait()?;
        let turn = response["turn"]["id"]
            .as_str()
            .ok_or("Missing native turn ID")?
            .to_owned();
        active_turn = Some(turn.clone());
        let mut requests = Requests::default();
        let mut answered = None;
        let mut resolved = false;
        let mut final_text = String::new();
        loop {
            match rx.recv()? {
                Event::ServerRequest {
                    key,
                    method,
                    params,
                } => {
                    if method != "item/permissions/requestApproval" || answered.is_some() {
                        return Err(
                            "Unexpected or repeated native request; no permission granted".into(),
                        );
                    }
                    validate_scope(&params, &thread, &turn, requested.path())?;
                    requests.insert("chat:permission-probe", key.clone(), &method, params)?;
                    let view = requests
                        .views("chat:permission-probe")
                        .pop()
                        .ok_or("Missing request view")?;
                    if view.kind != Kind::Permissions || !requests.views("graph:other").is_empty() {
                        return Err("Permission request owner isolation failed".into());
                    }
                    let deny = || Decision::Permissions {
                        grant: false,
                        session: false,
                    };
                    if requests.answer("graph:other", &view.ticket, deny()).is_ok() {
                        return Err("Cross-owner answer was accepted".into());
                    }
                    let (key, result) =
                        requests.answer("chat:permission-probe", &view.ticket, deny())?;
                    if result != json!({"permissions":{},"scope":"turn"}) {
                        return Err("Denial must grant no permissions".into());
                    }
                    if requests
                        .answer("chat:permission-probe", &view.ticket, deny())
                        .is_ok()
                    {
                        return Err("Duplicate decision was accepted".into());
                    }
                    client.answer(&key, Ok(result))?;
                    answered = Some(key);
                    println!(
                        "Exact disposable permission request denied through production request store."
                    );
                }
                Event::Notification { method, params } if params["threadId"] == thread => {
                    if method == "serverRequest/resolved" {
                        let id: RequestId = serde_json::from_value(params["requestId"].clone())?;
                        if Some(&id) != answered.as_ref().map(|key| &key.id)
                            || requests.notify(&method, &params)
                                != vec!["chat:permission-probe".to_owned()]
                        {
                            return Err(
                                "Native permission resolution did not match the exact request"
                                    .into(),
                            );
                        }
                        resolved = true;
                    }
                    if matches!(method.as_str(), "item/started" | "item/completed") {
                        let item = &params["item"];
                        let kind = item["type"].as_str().unwrap_or_default();
                        if !matches!(
                            kind,
                            "userMessage" | "agentMessage" | "reasoning" | "functionCallOutput"
                        ) || (kind == "functionCallOutput"
                            && item["name"] != "request_permissions")
                        {
                            return Err(format!(
                                "Unexpected tool activity in permission-only test: {kind}"
                            )
                            .into());
                        }
                        if method == "item/completed" && kind == "agentMessage" {
                            final_text = item["text"].as_str().unwrap_or_default().to_owned();
                        }
                    }
                    if method == "turn/completed" && params["turn"]["id"] == turn {
                        active_turn = None;
                        if params["turn"]["status"] != "completed" {
                            return Err("Native permission probe turn did not complete".into());
                        }
                        break;
                    }
                }
                Event::Closed { reason } => return Err(reason.into()),
                _ => {}
            }
        }
        if answered.is_none() {
            return Err(format!(
                "No native permission callback observed. Model response: {final_text}"
            )
            .into());
        }
        if !resolved || !requests.views("chat:permission-probe").is_empty() {
            return Err("Native permission request remained unresolved".into());
        }
        let history = api::read_thread(&thread).send(&client)?.wait()?;
        if history["thread"]["id"] != thread
            || !history["thread"]["turns"].as_array().is_some_and(|turns| {
                turns.len() == 1 && turns[0]["id"] == turn && turns[0]["status"] == "completed"
            })
        {
            return Err("Native history does not confirm the single completed test turn".into());
        }
        if std::fs::read_dir(workspace.path())?.next().is_some()
            || std::fs::read_dir(requested.path())?.next().is_some()
        {
            return Err("A no-write test directory changed unexpectedly".into());
        }
        println!(
            "Native permission denial, exact resolution, completed history and unchanged directories verified."
        );
        Ok(())
    })();
    if let (Some(thread), Some(turn)) = (&owned_thread, &active_turn) {
        let _ = api::interrupt_turn(thread, turn)
            .send(&client)
            .and_then(|ticket| ticket.wait());
    }
    let cleanup = if let Some(thread) = &owned_thread {
        api::delete_thread(thread)
            .send(&client)
            .and_then(|ticket| ticket.wait())
            .map(|_| {
                println!("Deleted owned native probe history: {thread}");
            })
    } else {
        Ok(())
    };
    client.shutdown();
    cleanup?;
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_probe_rejects_any_scope_outside_its_exact_directory() {
        let own = tempfile::tempdir().unwrap();
        let other = tempfile::tempdir().unwrap();
        let base = json!({"threadId":"t","turnId":"u","permissions":{"network":null,"fileSystem":{"read":null,"write":[own.path()]}}});
        assert!(validate_scope(&base, "t", "u", own.path()).is_ok());
        assert!(validate_scope(&base, "other", "u", own.path()).is_err());
        assert!(validate_scope(&base, "t", "other", own.path()).is_err());
        for (pointer, value) in [
            ("/permissions/network", json!({"enabled":true})),
            ("/permissions/fileSystem/read", json!([own.path()])),
            ("/permissions/fileSystem/write", json!([other.path()])),
            (
                "/permissions/fileSystem/write",
                json!([own.path(), other.path()]),
            ),
            (
                "/permissions/fileSystem/entries",
                json!([{"path":other.path(),"access":"write"}]),
            ),
            ("/permissions/fileSystem/globScanMaxDepth", json!(2)),
        ] {
            let mut invalid = base.clone();
            // The last two optional fields are absent in the base fixture.
            if let Some(field) = invalid.pointer_mut(pointer) {
                *field = value;
            } else {
                invalid["permissions"]["fileSystem"][pointer.rsplit('/').next().unwrap()] = value;
            }
            assert!(
                validate_scope(&invalid, "t", "u", own.path()).is_err(),
                "{pointer}"
            );
        }
    }
}
