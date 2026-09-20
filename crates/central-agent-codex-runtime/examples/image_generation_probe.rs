//! One opted-in account-backed native image, no substitute tool or image API.
//! Creates/deletes only its own native thread; keeps the owned result directory
//! for subsequent nonvisual decoder/host verification, never prints image bytes.
use central_agent_codex_runtime::{
    api,
    runtime::Runtime,
    transport::{Client, Event},
};
use serde_json::{Value, json};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const PROMPT: &str = "Generate exactly one small square image using your built-in image generation tool: a solid blue circle centered on a plain white background. This is a native image-output integration test. Native preparation reads required by your built-in image-generation instructions are allowed; do not inspect unrelated files or credentials, use MCP/web services, request additional permissions, or change configuration. Keep any output files in the current temporary workspace. Do not create scripts or substitute an SVG/code rendering. Do not use an image viewer or perform visual testing. If native image generation is unavailable, report that and stop without retrying. No text or logos in the image.";

fn main() -> Result<()> {
    if !std::env::args().any(|arg| arg == "--allow-test-inference") {
        return Err("Pass --allow-test-inference for one native generation using Codex account quota, with an owned temporary workspace. No visual test.".into());
    }
    let root = tempfile::Builder::new()
        .prefix("central-native-image-")
        .tempdir()?;
    let work = root.path().join("workspace");
    std::fs::create_dir(&work)?;
    let runtime = Runtime::discover(&work)?;
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn(&runtime, "native-image-probe", move |event| {
        let _ = tx.send(event);
    })?;
    let mut owned_thread = None;
    let mut active_turn = None;
    let mut captured_result = false;
    let result = (|| -> Result<Value> {
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
        let capabilities = client
            .request("modelProvider/capabilities/read", json!({}))?
            .wait()?;
        if capabilities["imageGeneration"] != true {
            return Err("Native provider does not advertise image generation".into());
        }
        let requirements = api::requirements().send(&client)?.wait()?;
        api::Access::WorkspaceWrite.validate_requirements(Some(&requirements["requirements"]))?;
        let profile = api::Profile::default();
        let started = api::start_thread(
            work.to_str().ok_or("Invalid owned workspace")?,
            &profile,
            api::Access::WorkspaceWrite,
        )
        .send(&client)?
        .wait()?;
        let thread = started["thread"]["id"]
            .as_str()
            .ok_or("Missing native thread")?
            .to_owned();
        owned_thread = Some(thread.clone());
        if started["sandbox"]["type"] != "workspaceWrite"
            || started["sandbox"]["networkAccess"] != false
        {
            return Err(
                "Native image thread did not retain requested workspace/network policy".into(),
            );
        }
        println!(
            "Native image probe model: {}. One account-backed generation; no grants or configuration changes.",
            started["model"]
        );
        let ack = api::start_turn(
            &thread,
            "native-image-once",
            vec![api::text_input(PROMPT)],
            work.to_str().ok_or("Invalid cwd")?,
            &profile,
            api::Access::WorkspaceWrite,
        )
        .send(&client)?
        .wait()?;
        let turn = ack["turn"]["id"]
            .as_str()
            .ok_or("Missing native turn")?
            .to_owned();
        active_turn = Some(turn.clone());
        let deadline = Instant::now() + Duration::from_secs(300);
        let mut generated = None;
        let mut final_text = String::new();
        loop {
            match rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))? {
                Event::ServerRequest { .. } => {
                    return Err("Unexpected authority request; no approval granted".into());
                }
                Event::Closed { reason } => return Err(reason.into()),
                Event::Notification { method, params } if params["threadId"] == thread => {
                    if method == "item/started" {
                        match params["item"]["type"].as_str() {
                            Some(
                                "userMessage" | "agentMessage" | "reasoning" | "imageGeneration"
                                | "commandExecution",
                            ) => {}
                            _ => {
                                return Err(format!(
                                    "Unexpected native tool kind {}; stopping owned turn",
                                    params["item"]["type"]
                                )
                                .into());
                            }
                        }
                    }
                    if method == "item/completed" && params["item"]["type"] == "commandExecution" {
                        println!(
                            "Native preparation command finished: status={}, exitCode={}",
                            params["item"]["status"], params["item"]["exitCode"]
                        );
                    }
                    if method == "item/completed"
                        && params["item"]["type"] == "agentMessage"
                        && params["item"]["phase"] == "final_answer"
                    {
                        final_text = params["item"]["text"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned();
                    }
                    if method == "item/completed" && params["item"]["type"] == "imageGeneration" {
                        if generated.is_some() || params["turnId"] != turn {
                            return Err("Unexpected extra or unowned image".into());
                        }
                        println!(
                            "Native image item completed: status={}, embedded characters={}, failure={}",
                            params["item"]["status"],
                            params["item"]["result"].as_str().map_or(0, str::len),
                            params["item"]["failure"]
                        );
                        generated = Some(params["item"].clone());
                        // Preserve actual wire evidence before history readback
                        // and cleanup. A later check must not erase a valid image.
                        std::fs::write(
                            root.path().join("native-image-item.json"),
                            serde_json::to_vec(&params["item"])?,
                        )?;
                        std::fs::write(
                            root.path().join("native-image-event.json"),
                            serde_json::to_vec(&params)?,
                        )?;
                        captured_result = true;
                    }
                    if method == "turn/completed" && params["turn"]["id"] == turn {
                        active_turn = None;
                        if params["turn"]["status"] != "completed" {
                            return Err(format!(
                                "Native generation turn failed: {}",
                                params["turn"]["error"]
                            )
                            .into());
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        let item = generated.ok_or_else(||format!("No native imageGeneration item; a text answer is not generation acceptance. Native final answer: {final_text}"))?;
        if item["status"] != "completed"
            || !item["failure"].is_null()
            || item["result"].as_str().is_none_or(str::is_empty)
        {
            return Err("Native image is failed or has no embedded result".into());
        }
        // savedPath is native metadata, not authority to read/delete an output.
        // Codex may store generated media independently from the working folder.
        // Acceptance uses embedded result bytes and never follows this path.
        let history = api::read_thread(&thread).send(&client)?.wait()?;
        std::fs::write(
            root.path().join("native-image-history.json"),
            serde_json::to_vec(&history)?,
        )?;
        if !history["thread"]["turns"].as_array().is_some_and(|turns| {
            turns.iter().any(|t| {
                t["id"] == turn
                    && t["items"].as_array().is_some_and(|items| {
                        items.iter().any(|i| {
                            i["id"] == item["id"]
                                && i["type"] == "imageGeneration"
                                && i["result"] == item["result"]
                        })
                    })
            })
        }) {
            return Err("Native history does not retain the generated result".into());
        }
        Ok(item)
    })();
    if let (Some(thread), Some(turn)) = (&owned_thread, &active_turn) {
        let _ = api::interrupt_turn(thread, turn)
            .send(&client)
            .and_then(|t| t.wait());
    }
    let cleanup = owned_thread.as_ref().map(|thread| {
        api::delete_thread(thread)
            .send(&client)
            .and_then(|t| t.wait())
    });
    client.shutdown();
    let retained = captured_result.then(|| root.keep());
    if let Some(path) = &retained {
        println!(
            "Retained actual native image evidence: {}. Any native savedPath was left unopened and untouched.",
            path.display()
        );
    }
    if let Some(cleanup) = cleanup {
        cleanup?;
        println!("Deleted only the owned native test thread.");
    }
    result?;
    println!(
        "PASS: native image item and identical persisted result. Pixel decoding and desktop-host projection still require verification; no visual test performed."
    );
    Ok(())
}
