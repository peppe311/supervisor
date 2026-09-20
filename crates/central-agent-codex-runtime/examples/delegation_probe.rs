//! Explicit, bounded account-backed delegation probe. Only the new test family
//! is observed; no existing history is inspected or deleted and no approval is
//! granted. All model work is requested as Luna / low / Standard.
use central_agent_codex_runtime::{
    api,
    runtime::Runtime,
    transport::{Client, Event},
    wire::RpcError,
};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !args.iter().any(|arg| arg == "--allow-test-inference") {
        return Err("Explicit --allow-test-inference and an output directory are required".into());
    }
    let root = PathBuf::from(args.last().ok_or("Missing output directory")?);
    fs::create_dir_all(root.join("project"))?;
    let root = fs::canonicalize(root)?;
    let work = root.join("project");
    fs::write(work.join("fixture.txt"), "SUPERVISOR_DELEGATE_OK\n")?;
    let mut log = fs::File::create(root.join("native-discovery.jsonl"))?;
    let runtime = Runtime::discover(&work)?;
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn(&runtime, "supervisor-delegation-probe", move |event| {
        let _ = tx.send(event);
    })?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        loop {
            match rx.recv_timeout(Duration::from_secs(30))? {
                Event::Ready { .. } => break,
                Event::Closed { reason } => return Err(reason.into()),
                _ => {}
            }
        }
        let profile = api::Profile {
            model: Some("gpt-5.6-luna".into()),
            effort: Some("low".into()),
            service_tier: Some("default".into()),
            summary: Some(api::ReasoningSummary::Auto),
            ..Default::default()
        };
        let started = api::start_thread(
            work.to_str().ok_or("Non-UTF8 path")?,
            &profile,
            api::Access::ReadOnly,
        )
        .send(&client)?
        .wait()?;
        let parent = started["thread"]["id"]
            .as_str()
            .ok_or("Missing parent")?
            .to_owned();
        fs::write(
            root.join("native-parent.json"),
            serde_json::to_vec_pretty(
                &json!({"threadId":parent,"model":profile.model,"effort":profile.effort,"serviceTier":profile.service_tier,"runtime":runtime.version()}),
            )?,
        )?;
        println!("Native parent created: {parent}. Starting one bounded delegation.");
        let prompt = "This is an explicitly authorized Supervisor integration test. Delegate exactly one bounded subtask to one subagent using model gpt-5.6-luna, reasoning effort low, and normal/Standard speed. The child must read only fixture.txt in the current directory with a command tool and return its exact marker. Do useful independent work yourself by checking that the expected marker format is SUPERVISOR_DELEGATE_OK, then wait for the child and compare its result. Finish with VERIFIED followed by the marker only if it matches. Do not edit files, spawn extra agents, browse, or use external services. Do not substitute another model if Luna is unavailable; report the limitation. Keep all messages brief.";
        let ack = api::start_turn(
            &parent,
            "supervisor-delegation-discovery",
            vec![api::text_input(prompt)],
            work.to_str().unwrap(),
            &profile,
            api::Access::ReadOnly,
        )
        .send(&client)?
        .wait()?;
        let turn = ack["turn"]["id"].as_str().ok_or("Missing turn")?.to_owned();
        let mut owned = HashSet::from([parent.clone()]);
        let deadline = Instant::now() + Duration::from_secs(180);
        let mut sequence = 0u64;
        loop {
            match rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
                Event::Closed { reason } => return Err(reason.into()),
                Event::ServerRequest {
                    key,
                    method,
                    params,
                } => {
                    println!(
                        "Native authority request observed: {method}; declining in this discovery probe."
                    );
                    client.answer(
                        &key,
                        Err(RpcError {
                            code: -32601,
                            message: "Discovery probe does not grant approvals".into(),
                            data: None,
                        }),
                    )?;
                    if owned.contains(params["threadId"].as_str().unwrap_or_default()) {
                        writeln!(
                            log,
                            "{}",
                            json!({"method":method,"threadId":params["threadId"],"turnId":params["turnId"],"itemId":params["itemId"],"kind":"request","decision":"declined_by_probe"})
                        )?;
                    }
                }
                Event::Notification { method, params } => {
                    let thread = params["threadId"]
                        .as_str()
                        .or_else(|| params["thread"]["id"].as_str())
                        .unwrap_or_default();
                    if method == "thread/started" && params["thread"]["source"]["subAgent"]["thread_spawn"]["parent_thread_id"].as_str().is_some_and(|p| owned.contains(p)) { owned.insert(thread.into()); }
                    if !owned.contains(thread) {
                        continue;
                    }
                    let item = &params["item"];
                    if item["type"] == "collabAgentToolCall" && item["tool"] == "spawnAgent" {
                        for child in item["receiverThreadIds"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                        {
                            owned.insert(child.into());
                        }
                    }
                    sequence += 1;
                    let record = json!({"sequence":sequence,"receivedAtMs":SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),"method":method,"threadId":thread,"turnId":params["turnId"].as_str().or_else(|| params["turn"]["id"].as_str()),"itemId":params["itemId"].as_str().or_else(|| item["id"].as_str()),"itemType":item["type"],"itemStatus":item["status"],"turnStatus":params["turn"]["status"],"threadStatus":params["status"],"source":params["thread"]["source"],"cwd":params["thread"]["cwd"],"tool":item["tool"],"receiverThreadIds":item["receiverThreadIds"],"agentsStates":if item["type"]=="collabAgentToolCall" {item["agentsStates"].clone()} else {Value::Null},"phase":item["phase"],"model":item["model"],"reasoningEffort":item["reasoningEffort"],"settings":params.get("threadSettings").map(|s|json!({"model":s["model"],"reasoningEffort":s["reasoningEffort"],"serviceTier":s["serviceTier"]})),"deltaBytes":params["delta"].as_str().map(str::len),"publicResult":if item["type"]=="agentMessage" && item["phase"]=="final_answer" {item["text"].clone()}else{Value::Null}});
                    writeln!(log, "{record}")?;
                    log.flush()?;
                    if matches!(
                        method.as_str(),
                        "thread/started" | "turn/started" | "turn/completed"
                    ) || item["type"] == "collabAgentToolCall"
                    {
                        println!(
                            "{sequence}: {method} thread={thread} item={} tool={} status={}",
                            item["type"], item["tool"], params["turn"]["status"]
                        );
                    }
                    if method == "turn/completed"
                        && thread == parent
                        && params["turn"]["id"] == turn
                    {
                        fs::write(
                            root.join("native-family.json"),
                            serde_json::to_vec_pretty(
                                &json!({"parent":parent,"turn":turn,"threads":owned,"status":params["turn"]["status"]}),
                            )?,
                        )?;
                        if params["turn"]["status"] != "completed" {
                            return Err("Parent did not complete".into());
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    })();
    client.shutdown();
    result
}
