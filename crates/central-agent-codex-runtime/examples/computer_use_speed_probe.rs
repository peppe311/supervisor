//! Explicit, ephemeral account-backed A/B probe. Does not write native settings,
//! grant approvals or save reasoning/screenshots. Reports contain timing metadata
//! and the short final verdict only.
//! Run the two variants serially, restoring the same Calculator state between runs.
use central_agent_codex_runtime::{
    api,
    runtime::Runtime,
    transport::{Client, Event},
    wire::RpcError,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::mpsc,
    time::{Duration, Instant},
};

type ProbeResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> ProbeResult<()> {
    let args: Vec<_> = std::env::args().collect();
    if !matches!(args.len(), 7 | 9) || args[1] != "--allow-test-inference" {
        return Err("Usage: --allow-test-inference <baseline|optimized> <codex-home> <official-skill> <retired-companion-fixture> <report.json> [calculator|supervisor-search <timing-module.mjs>]".into());
    }
    let optimized = match args[2].as_str() {
        "baseline" => false,
        "optimized" => true,
        _ => return Err("Unknown variant".into()),
    };
    let home = PathBuf::from(&args[3]);
    let official = PathBuf::from(&args[4]);
    let companion = PathBuf::from(&args[5]);
    if !official.is_file() || !companion.is_file() || !home.is_dir() {
        return Err("Existing runtime home and both skill files are required".into());
    }
    let report = PathBuf::from(&args[6]);
    let scenario = args.get(7).map(String::as_str).unwrap_or("calculator");
    if !matches!(scenario, "calculator" | "supervisor-search") {
        return Err("Unknown Computer Use test scenario".into());
    }
    let timing = args.get(8).map(fs::canonicalize).transpose()?;
    let work = tempfile::Builder::new()
        .prefix("computer-use-speed-")
        .tempdir()?;
    // An empty project boundary keeps unrelated repository instructions out of
    // both variants. This is a disposable directory in the managed build TEMP.
    fs::create_dir(work.path().join(".git"))?;
    let cwd = work.path().to_str().ok_or("Non-UTF8 working directory")?;
    let runtime = Runtime::discover(work.path())?.with_home(&home)?;
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn(
        &runtime,
        "supervisor-computer-use-speed-probe",
        move |event| {
            let _ = tx.send((Instant::now(), event));
        },
    )?;
    let trial = Trial {
        cwd,
        official: &args[4],
        companion: &args[5],
        optimized,
        scenario,
        timing: timing.as_deref(),
    };
    let result = measure(&client, &rx, &trial);
    client.shutdown();
    let mut value = match &result {
        Ok(value) => value.clone(),
        Err(error) => json!({"valid":false,"error":error.to_string()}),
    };
    value["variant"] = json!(args[2]);
    value["runtime"] = json!(runtime.version());
    value["scenario"] = json!(scenario);
    fs::write(&report, serde_json::to_vec_pretty(&value)?)?;
    println!("{value}");
    result.and_then(|value| {
        if value["valid"] == true {
            Ok(())
        } else {
            Err("The trial completed without a valid verdict or adequate timing coverage".into())
        }
    })
}

struct Trial<'a> {
    cwd: &'a str,
    official: &'a str,
    companion: &'a str,
    optimized: bool,
    scenario: &'a str,
    timing: Option<&'a std::path::Path>,
}

fn measure(
    client: &Client,
    rx: &mpsc::Receiver<(Instant, Event)>,
    trial: &Trial<'_>,
) -> ProbeResult<Value> {
    let Trial {
        cwd,
        official,
        companion,
        optimized,
        ..
    } = *trial;
    loop {
        match rx.recv_timeout(Duration::from_secs(30))?.1 {
            Event::Ready { .. } => break,
            Event::Closed { reason } => return Err(reason.into()),
            _ => {}
        }
    }
    let skill_root = PathBuf::from(official)
        .parent()
        .and_then(|p| p.parent())
        .ok_or("Missing official skills root")?
        .to_string_lossy()
        .into_owned();
    api::skills_extra_roots(&[skill_root])
        .send(client)?
        .wait()?;
    let profile = api::Profile {
        model: Some("gpt-5.6-luna".into()),
        effort: Some("low".into()),
        service_tier: Some("default".into()),
        summary: Some(api::ReasoningSummary::Auto),
        ..Default::default()
    };
    // Fixed for both ephemeral variants; shared Supervisor settings are untouched.
    // Calculator operations and reading installed guidance are explicitly scoped
    // in the prompt. The probe never answers an approval request affirmatively.
    let access = api::Access::FullAccess;
    let started = api::start_thread_with_options(
        cwd,
        &profile,
        access,
        &api::ThreadStartOptions {
            ephemeral: Some(true),
            // A per-thread override; never calls skills/config/write.
            config: Some(BTreeMap::from([(
                "skills.config".into(),
                json!([{"path":companion,"enabled":optimized}]),
            )])),
            ..Default::default()
        },
    )
    .send(client)?
    .wait()?;
    if started["model"].as_str() != profile.model.as_deref() {
        return Err(format!("Unexpected model: {}", started["model"]).into());
    }
    let thread = started["thread"]["id"].as_str().ok_or("Missing thread")?;
    let task = if trial.scenario == "calculator" {
        "Operate the already open Windows Calculator. Clear the current calculation, calculate 7 + 8 in its interface, and verify the displayed result. Finish with only VERIFIED: followed by the displayed value."
    } else {
        "Operate only the already open Supervisor Settings window. In Find a setting, enter Reasoning summaries, verify the matching setting is shown, then clear the search field. Do not change any setting or open a conversation. Finish with only VERIFIED: Reasoning summaries."
    };
    let expected = if trial.scenario == "calculator" {
        "VERIFIED: 15"
    } else {
        "VERIFIED: Reasoning summaries"
    };
    let mut prompt = format!(
        "This is an explicitly authorized Computer Use speed test. Use the official Computer Use plugin. {task} Use only the attached skills for this task. Do not access other applications, edit files, change settings, browse, or spawn agents. Shell commands may only read the attached skill documentation and the provided timing module. Stop if a permission request is needed. Keep all messages brief."
    );
    if let Some(timing) = trial.timing {
        let url = url::Url::from_file_path(timing).map_err(|_| "Invalid timing module path")?;
        prompt.push_str(&format!(" After the official sky initialization, import the local measurement helper: globalThis.cuMeasure = (await import({})).createComputerUseTimer(sky, text => nodeRepl.write(text)). Route each public sky operation through await cuMeasure(methodName, originalArguments), for example await cuMeasure('get_window_state', {{window: targetWindow}}). This helper preserves the returned state and automatic image display and emits bounded timing metadata. Keep the official observe, inspect, one action, refresh cycle. Never print full state, typed content or images. If instrumentation is unavailable, stop; do not replace the plugin or synthesize timing samples.", serde_json::to_string(url.as_str())?));
    }
    let mut input = vec![
        api::text_input(&prompt),
        api::skill_input("computer-use", official),
    ];
    if optimized {
        input.push(api::skill_input("supervisor-computer-use", companion));
    }
    let clock = Instant::now();
    let ack = api::start_turn(
        thread,
        "computer-use-speed-probe",
        input,
        cwd,
        &profile,
        access,
    )
    .send(client)?
    .wait()?;
    let ack_ms = clock.elapsed().as_millis();
    let turn = ack["turn"]["id"].as_str().ok_or("Missing turn")?;
    println!(
        "Measuring {} with Luna / low / Standard",
        if optimized { "optimized" } else { "baseline" }
    );
    let deadline = clock + Duration::from_secs(240);
    let mut tools: HashMap<String, Value> = HashMap::new();
    let mut completed = HashSet::new();
    let mut final_text = String::new();
    let mut usage = Value::Null;
    let mut settings = Value::Null;
    let mut samples: BTreeMap<u16, Sample> = BTreeMap::new();
    loop {
        let (received, event) =
            match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(event) => event,
                Err(error) => {
                    let _ = api::interrupt_turn(thread, turn).send(client);
                    return Err(format!(
                        "Timed out/interrupted after {} ms: {error}",
                        clock.elapsed().as_millis()
                    )
                    .into());
                }
            };
        let at_ms = received.saturating_duration_since(clock).as_millis();
        match event {
            Event::Closed { reason } => return Err(reason.into()),
            Event::ServerRequest { key, method, .. } => {
                client.answer(
                    &key,
                    Err(RpcError {
                        code: -32601,
                        message: "The speed probe does not grant approvals".into(),
                        data: None,
                    }),
                )?;
                let _ = api::interrupt_turn(thread, turn).send(client);
                return Err(
                    format!("Benchmark stopped for request {method}; no approval granted").into(),
                );
            }
            Event::Notification { method, params } => {
                if params["threadId"].as_str() != Some(thread) {
                    continue;
                }
                let item = &params["item"];
                let kind = item["type"].as_str().unwrap_or_default();
                let id = item["id"].as_str().unwrap_or_default();
                if matches!(
                    kind,
                    "mcpToolCall"
                        | "commandExecution"
                        | "dynamicToolCall"
                        | "webSearch"
                        | "collabAgentToolCall"
                ) {
                    if method == "item/started" {
                        let source = if kind == "commandExecution" {
                            item["command"].to_string()
                        } else {
                            item["arguments"].to_string()
                        };
                        let operations: Vec<_> = [
                            "list_apps",
                            "get_window",
                            "get_window_state",
                            "activate_window",
                            "click",
                            "press_key",
                            "type_text",
                            "scroll",
                            "sleep",
                        ]
                        .into_iter()
                        .filter(|op| source.contains(&format!(".{op}(")))
                        .collect();
                        tools.insert(id.into(), json!({
                            "kind":kind,"tool":item["tool"],"server":item["server"],"startMs":at_ms,
                            "methodsMentionedInCode":operations,"companionMentioned":source.contains("supervisor-computer-use"),
                        }));
                        println!("{at_ms} ms: {kind} {} started", item["tool"]);
                    } else if method == "item/completed" && completed.insert(id.to_owned()) {
                        if kind == "mcpToolCall"
                            && item["server"] == "node_repl"
                            && item["tool"] == "js"
                        {
                            collect_samples(&item["result"]["content"], &mut samples);
                        }
                        let entry = tools
                            .entry(id.into())
                            .or_insert_with(|| json!({"kind":kind,"tool":item["tool"]}));
                        entry["endMs"] = json!(at_ms);
                        entry["durationMs"] = item["durationMs"].clone();
                        entry["status"] = item["status"].clone();
                        entry["exitCode"] = item["exitCode"].clone();
                        entry["failed"] = json!(
                            item["status"] == "failed"
                                || !item["error"].is_null()
                                || item["success"] == false
                                || item["result"]["isError"] == true
                                || item["exitCode"].as_i64().is_some_and(|code| code != 0)
                        );
                        println!(
                            "{at_ms} ms: {kind} {} completed status={}",
                            item["tool"], item["status"]
                        );
                    }
                }
                if method == "item/completed" && kind == "agentMessage" {
                    final_text = item["text"].as_str().unwrap_or_default().to_owned();
                }
                if method == "thread/tokenUsage/updated" {
                    usage = params["tokenUsage"].clone();
                }
                if method == "thread/settings/updated" {
                    let s = &params["threadSettings"];
                    settings = json!({"model":s["model"],"effort":s["reasoningEffort"],"serviceTier":s["serviceTier"]});
                }
                if method == "turn/completed" && params["turn"]["id"] == turn {
                    let mut calls: Vec<_> = tools.into_values().collect();
                    calls.sort_by_key(|call| call["startMs"].as_u64().unwrap_or_default());
                    let tool_ms: u64 = calls
                        .iter()
                        .filter_map(|call| call["durationMs"].as_u64())
                        .sum();
                    let failed = calls.iter().filter(|call| call["failed"] == true).count();
                    let contaminated =
                        !optimized && calls.iter().any(|call| call["companionMentioned"] == true);
                    let has_observation = samples
                        .values()
                        .any(|s| s.phase == "observation" && s.completed);
                    let has_input = samples.values().any(|s| s.phase == "input" && s.completed);
                    let recorded_cycle_valid = recorded_observation_cycle_valid(&samples);
                    let timing_coverage = trial.timing.is_none()
                        || (has_observation && has_input && recorded_cycle_valid);
                    return Ok(json!({
                        "valid":params["turn"]["status"] == "completed" && !contaminated && final_text.trim() == expected && timing_coverage,
                        "model":profile.model,"effort":profile.effort,"serviceTier":profile.service_tier,
                        "wallMs":at_ms,"ackMs":ack_ms,"toolDurationSumMs":tool_ms,"failedTools":failed,
                        "toolCalls":calls.len(),"calls":calls,"verdictMatched":final_text.trim() == expected,
                        "finalText":(final_text.trim() == expected).then_some(expected),
                        "publicMethodSamples":samples.values().collect::<Vec<_>>(),
                        "hasObservationAndInputSamples":has_observation && has_input,
                        "recordedObservationCycleValid":trial.timing.map(|_| recorded_cycle_valid),
                        "timingScope":"Only public methods routed through the explicit helper; not a trace of private service stages",
                        "status":params["turn"]["status"],"tokenUsage":usage,"settings":settings,
                        "approvalRequests":0,"baselineContaminated":contaminated,
                        "ephemeral":true,
                    }));
                }
            }
            _ => {}
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Sample {
    sequence: u16,
    method: String,
    phase: String,
    completed: bool,
    duration_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    screenshot_requested: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text_requested: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_returned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text_returned: Option<bool>,
}

// A completed empty accessibility response is not evidence for the next input.
// This checks the recorded single-window sequence, not uninstrumented operations
// or whether the model actually inspected the available image/text.
fn recorded_observation_cycle_valid(samples: &BTreeMap<u16, Sample>) -> bool {
    let mut observed = false;
    let mut had_input = false;
    for (index, sample) in samples.values().enumerate() {
        if usize::from(sample.sequence) != index + 1 {
            return false;
        }
        match sample.phase.as_str() {
            "observation" => {
                observed |= sample.completed
                    && (sample.image_returned == Some(true) || sample.text_returned == Some(true));
            }
            "input" => {
                if !observed {
                    return false;
                }
                had_input = true;
                observed = false;
            }
            _ if matches!(sample.method.as_str(), "activate_window" | "launch_app") => {
                observed = false;
            }
            _ => {}
        }
    }
    had_input && observed
}

fn collect_samples(content: &Value, samples: &mut BTreeMap<u16, Sample>) {
    for block in content.as_array().into_iter().flatten().take(64) {
        if block["type"] != "text" {
            continue;
        }
        let Some(text) = block["text"]
            .as_str()
            .filter(|text| text.len() <= 256 * 1024)
        else {
            continue;
        };
        for line in text.lines() {
            let Some(raw) = line
                .strip_prefix("SUPERVISOR_CU_TIMING ")
                .filter(|raw| raw.len() <= 1024)
            else {
                continue;
            };
            let Ok(sample) = serde_json::from_str::<Sample>(raw) else {
                continue;
            };
            let phase = match sample.method.as_str() {
                "get_window_state" => "observation",
                "list_apps" | "list_windows" | "get_window" | "launch_app" | "activate_window" => {
                    "setup"
                }
                "click"
                | "press_key"
                | "type_text"
                | "scroll"
                | "drag"
                | "set_value"
                | "perform_secondary_action" => "input",
                _ => continue,
            };
            if !(1..=256).contains(&sample.sequence)
                || sample.phase != phase
                || !sample.duration_ms.is_finite()
                || !(0.0..=300_000.0).contains(&sample.duration_ms)
            {
                continue;
            }
            samples.entry(sample.sequence).or_insert(sample);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recorded_inputs_need_fresh_usable_observations_and_a_final_check() {
        let observation = |sequence, usable| {
            serde_json::from_value::<Sample>(json!({
                "sequence":sequence,"method":"get_window_state","phase":"observation",
                "completed":true,"durationMs":3,"imageReturned":usable,"textReturned":false
            }))
            .unwrap()
        };
        let input = |sequence| {
            serde_json::from_value::<Sample>(json!({
                "sequence":sequence,"method":"click","phase":"input",
                "completed":true,"durationMs":1
            }))
            .unwrap()
        };
        let mut samples = BTreeMap::from([(1, observation(1, true)), (2, input(2))]);
        assert!(!recorded_observation_cycle_valid(&samples));
        samples.insert(3, observation(3, false));
        assert!(!recorded_observation_cycle_valid(&samples));
        samples.insert(4, observation(4, true));
        assert!(recorded_observation_cycle_valid(&samples));
        samples.insert(5, input(5));
        samples.insert(6, observation(6, false));
        samples.insert(7, input(7));
        samples.insert(8, observation(8, true));
        assert!(!recorded_observation_cycle_valid(&samples));
        samples.remove(&7);
        assert!(!recorded_observation_cycle_valid(&samples));
    }

    #[test]
    fn measurements_are_bounded_typed_and_do_not_copy_extra_payloads() {
        let valid =
            r#"{"sequence":1,"method":"click","phase":"input","completed":true,"durationMs":25.5}"#;
        let content = json!([
            {"type":"text","text":format!("SUPERVISOR_CU_TIMING {valid}\nSUPERVISOR_CU_TIMING {valid}")},
            {"type":"image","data":"private pixels"},
            {"type":"text","text":"SUPERVISOR_CU_TIMING {\"sequence\":2,\"method\":\"click\",\"phase\":\"input\",\"completed\":true,\"durationMs\":1,\"text\":\"private\"}"},
            {"type":"text","text":format!("SUPERVISOR_CU_TIMING {}", valid.replace("25.5", "-1"))},
        ]);
        let mut samples = BTreeMap::new();
        collect_samples(&content, &mut samples);
        assert_eq!(samples.len(), 1);
        let sample = &samples[&1];
        assert_eq!(sample.method, "click");
        assert_eq!(sample.duration_ms, 25.5);
        assert!(!serde_json::to_string(&samples).unwrap().contains("private"));
    }
}
