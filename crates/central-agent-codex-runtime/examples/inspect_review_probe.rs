//! Diagnostic for an explicitly selected disposable probe workspace. Read-only
//! unless --delete-completed-probe explicitly deletes that selected test root.
//! Never resumes a thread, submits inference, or changes native configuration.
use central_agent_codex_runtime::{
    api,
    runtime::Runtime,
    transport::{Client, Event},
};
use serde_json::json;
use std::{path::PathBuf, sync::mpsc};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let selected = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Specify the exact disposable probe workspace")?,
    );
    if !selected.is_absolute() {
        return Err("Specify an absolute disposable probe workspace".into());
    }
    let cwd = selected.canonicalize()?;
    let temp = std::env::temp_dir().canonicalize()?;
    if cwd.parent() != Some(temp.as_path())
        || !cwd
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with(".tmp"))
    {
        return Err("Only a direct tempfile probe directory is accepted".into());
    }
    let runtime = Runtime::discover(&cwd)?;
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn(&runtime, env!("CARGO_PKG_VERSION"), move |event| {
        let _ = tx.send(event);
    })?;
    loop {
        match rx.recv()? {
            Event::Ready { .. } => break,
            Event::Closed { reason } => return Err(reason.into()),
            Event::ServerRequest { .. } => return Err("Unexpected native decision request".into()),
            _ => {}
        }
    }
    let listed = api::Call { method:"thread/list", params:json!({
        "cwd":selected, "useStateDbOnly":true, "limit":100,
        "sourceKinds":["cli","vscode","exec","appServer","subAgent","subAgentReview","subAgentCompact","subAgentThreadSpawn","subAgentOther","unknown"]
    }) }.send(&client)?.wait()?;
    let entries = listed["data"]
        .as_array()
        .ok_or("Missing native list result")?;
    let selected_id = std::env::args().nth(2);
    let matching: Vec<_> = entries
        .iter()
        .filter(|entry| selected_id.as_deref().is_none_or(|id| entry["id"] == id))
        .collect();
    if matching.len() != 1 || !listed["nextCursor"].is_null() {
        println!(
            "{}",
            json!({"matches":entries.iter().map(|entry|json!({"id":entry["id"],"cwd":entry["cwd"],"createdAt":entry["createdAt"],"source":entry["source"],"status":entry["status"]})).collect::<Vec<_>>()})
        );
        return Err(format!("Expected exactly one native thread in the disposable probe workspace, found {}; no history read", entries.len()).into());
    }
    let entry = matching[0];
    if PathBuf::from(entry["cwd"].as_str().ok_or("Missing native cwd")?).canonicalize()? != cwd {
        return Err("Native thread does not match the exact requested workspace".into());
    }
    let id = entry["id"].as_str().ok_or("Missing native thread ID")?;
    let read = api::read_thread(id).send(&client)?.wait()?;
    println!(
        "{}",
        json!({"threadId":id,"createdAt":entry["createdAt"],"cwd":entry["cwd"],"status":read["thread"]["status"],"turns":read["thread"]["turns"].as_array().into_iter().flatten().map(|turn|json!({
        "id":turn["id"],"status":turn["status"],"error":turn["error"],"items":turn["items"].as_array().into_iter().flatten().map(|item|json!({"id":item["id"],"type":item["type"],"hasReviewText":item["review"].as_str().is_some_and(|s|!s.trim().is_empty())})).collect::<Vec<_>>()
    })).collect::<Vec<_>>()})
    );
    if std::env::args().any(|arg| arg == "--delete-completed-probe") {
        let turns = read["thread"]["turns"]
            .as_array()
            .ok_or("Missing native turns")?;
        if selected_id.is_none()
            || read["thread"]["source"] != "vscode"
            || turns.is_empty()
            || turns.iter().any(|turn| {
                !matches!(
                    turn["status"].as_str(),
                    Some("completed" | "failed" | "interrupted")
                )
            })
        {
            return Err(
                "Cleanup requires an explicitly selected test root with only terminal turns".into(),
            );
        }
        api::delete_thread(id).send(&client)?.wait()?;
        println!(
            "Deleted the explicitly selected completed probe root and its native descendants: {id}"
        );
    }
    client.shutdown();
    Ok(())
}
