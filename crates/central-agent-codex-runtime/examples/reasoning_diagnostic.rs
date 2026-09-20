//! Read-only diagnosis for explicitly linked Central Agent conversations.
//! No prompt, resume, configuration write, credential-file read or transcript log.
use central_agent_codex_runtime::{
    api,
    conversations::Saved,
    runtime::Runtime,
    transport::{Client, Event},
};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::mpsc, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Provide Central Agent's app-server-threads.json path")?,
    );
    if !path.is_absolute()
        || path.file_name().and_then(|s| s.to_str()) != Some("app-server-threads.json")
    {
        return Err("Expected an absolute Central Agent binding-store path".into());
    }
    let saved: Saved = serde_json::from_slice(&std::fs::read(&path)?)?;
    saved.validate()?;
    let runtime = Runtime::discover(path.parent().ok_or("Missing parent")?)?;
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn(&runtime, env!("CARGO_PKG_VERSION"), move |event| {
        let _ = tx.send(event);
    })?;
    loop {
        match rx.recv_timeout(Duration::from_secs(30))? {
            Event::Ready { .. } => break,
            Event::Closed { reason } => return Err(reason.into()),
            Event::ServerRequest { .. } => {
                return Err("Unexpected server request; no approval sent".into());
            }
            _ => {}
        }
    }
    for (index, binding) in saved
        .bindings
        .values()
        .filter(|b| !b.deleted && !b.archived)
        .enumerate()
    {
        let result = api::read_thread(&binding.thread_id).send(&client)?.wait()?;
        let thread = &result["thread"];
        let cwd = thread["cwd"].as_str().ok_or("Missing native directory")?;
        let config = api::config_read(cwd).send(&client)?.wait()?;
        let turns: Vec<_> = thread["turns"]
            .as_array()
            .into_iter()
            .flatten()
            .map(turn_report)
            .collect();
        println!(
            "{}",
            json!({"bindingNumber":index+1,"onDiskSummary":config["config"]["model_reasoning_summary"],"onDiskEffort":config["config"]["model_reasoning_effort"],"turns":turns})
        );
    }
    Ok(())
}

fn turn_report(turn: &Value) -> Value {
    let items: Vec<_> = turn["items"].as_array().into_iter().flatten().collect();
    let reasoning: Vec<_> = items.iter().filter(|i| i["type"] == "reasoning").collect();
    json!({"status":turn["status"],"itemCount":items.len(),"reasoningItems":reasoning.len(),
        "summaryCharacters":reasoning.iter().flat_map(|i|i["summary"].as_array().into_iter().flatten()).filter_map(Value::as_str).map(|s|s.chars().count()).sum::<usize>(),
        "commentaryMessages":items.iter().filter(|i|i["type"]=="agentMessage" && i["phase"]=="commentary").count(),
        "finalMessages":items.iter().filter(|i|i["type"]=="agentMessage" && i["phase"]=="final_answer").count(),
        "commandItems":items.iter().filter(|i|i["type"]=="commandExecution").count()})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reports_only_counts_not_private_content() {
        let report = turn_report(&json!({"status":"completed","items":[
            {"type":"reasoning","summary":["Visible summary"],"content":["PRIVATE"]},
            {"type":"agentMessage","phase":"commentary","text":"PRIVATE"},
            {"type":"commandExecution","command":"PRIVATE"}]}));
        assert_eq!(report["summaryCharacters"], 15);
        assert_eq!(report["commentaryMessages"], 1);
        assert!(!report.to_string().contains("PRIVATE"));
    }
}
