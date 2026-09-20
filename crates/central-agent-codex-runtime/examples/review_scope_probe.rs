//! Optional real-runtime probe. Explicit opt-in creates one empty native test
//! thread, submits one tiny read-only inference to materialize its rollout,
//! verifies review preparation, and deletes exactly that new thread.
//! --exercise-review additionally invokes the production inline review path on
//! an in-prompt code fixture. No existing-history read, tools or local binding.
use central_agent_codex_runtime::{
    api,
    conversations::{Action, Conversations},
    mirror::Turn,
    runtime::Runtime,
    transport::{Client, Event},
};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn wait_turn(
    events: &mpsc::Receiver<Event>,
    conversations: &mut Conversations,
    thread: &str,
    turn: &str,
    client: Option<&Client>,
) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match events.recv_timeout(remaining)? {
            Event::Notification { method, params } => {
                conversations.notification(&method, &params)?;
                if params["threadId"] != thread {
                    continue;
                }
                if matches!(
                    method.as_str(),
                    "turn/started"
                        | "turn/completed"
                        | "thread/status/changed"
                        | "item/started"
                        | "item/completed"
                ) {
                    println!(
                        "Native event {}",
                        serde_json::json!({"method":method,"turnId":params["turnId"],"turn":params["turn"]["id"],"status":params["status"],"itemType":params["item"]["type"]})
                    );
                }
                if matches!(method.as_str(), "item/started" | "item/completed") {
                    let kind = params["item"]["type"].as_str().unwrap_or_default();
                    if !matches!(
                        kind,
                        "userMessage"
                            | "agentMessage"
                            | "reasoning"
                            | "plan"
                            | "enteredReviewMode"
                            | "exitedReviewMode"
                    ) {
                        return Err(format!(
                            "The no-tool review probe received unexpected activity: {kind}"
                        )
                        .into());
                    }
                }
                if let Some(client) = client {
                    while let Some(read) = conversations.take_review_refresh("chat:probe")? {
                        let result = read.call.clone().send(client)?.wait()?;
                        conversations.complete(&read, Ok(result))?;
                        hydrate_pages(conversations, client, thread)?;
                        println!("Production native review reconciliation read completed.");
                    }
                    if !conversations.busy("chat:probe")
                        && conversations.mirror.thread(thread).is_some_and(|thread| {
                            thread
                                .turns
                                .iter()
                                .any(|native| native.id == turn && native.status == "completed")
                        })
                    {
                        println!(
                            "Native thread/read confirms completion of the acknowledged review turn."
                        );
                        return Ok(());
                    }
                }
                if method == "turn/completed" && params["turn"]["id"] == turn {
                    if params["turn"]["status"] != "completed" {
                        return Err("The native probe turn did not complete".into());
                    }
                    if client.is_none() || !conversations.busy("chat:probe") {
                        return Ok(());
                    }
                }
            }
            Event::Closed { reason } => return Err(reason.into()),
            Event::ServerRequest { .. } => return Err(
                "The no-tool probe unexpectedly requested a decision; no permission was granted"
                    .into(),
            ),
            Event::Ready { .. } => {}
        }
    }
}

fn hydrate_pages(conversations: &mut Conversations, client: &Client, thread: &str) -> Result<()> {
    if conversations.history_mode("chat:probe") != Some("paginated") {
        return Ok(());
    }
    let revision = conversations.mirror.revision();
    let mut cursor = None;
    let mut cursors = std::collections::HashSet::new();
    let mut turns = Vec::new();
    for _ in 0..8 {
        let page = api::list_thread_turns(thread, cursor.as_deref())
            .send(client)?
            .wait()?;
        turns.extend(
            page["data"]
                .as_array()
                .ok_or("Missing native turn page")?
                .iter()
                .cloned(),
        );
        if turns.len() > 64 {
            return Err("The disposable review exceeded its expected history bound".into());
        }
        if page["nextCursor"].is_null() {
            conversations.hydrate_paginated_history("chat:probe", thread, &turns, revision)?;
            println!(
                "Native paginated review history {}",
                serde_json::json!(
                    turns
                        .iter()
                        .map(|turn| serde_json::json!({"id":turn["id"],"status":turn["status"]}))
                        .collect::<Vec<_>>()
                )
            );
            return Ok(());
        }
        let next = page["nextCursor"]
            .as_str()
            .filter(|cursor| !cursor.is_empty())
            .ok_or("Invalid native page cursor")?;
        if !cursors.insert(next.to_owned()) {
            return Err("Native history repeated its cursor".into());
        }
        cursor = Some(next.to_owned());
    }
    Err("The disposable review exceeded its expected page bound".into())
}

fn verify_review(turn: &Turn) -> Result<()> {
    if turn.status != "completed" || turn.error.is_some() {
        return Err("The review has no authoritative successful completion".into());
    }
    for kind in ["enteredReviewMode", "exitedReviewMode"] {
        if !turn.items.iter().any(|item| {
            item.completed
                && item.value["type"] == kind
                && item.value["review"]
                    .as_str()
                    .is_some_and(|text| !text.trim().is_empty())
        }) {
            return Err(format!("The review is missing a completed, nonempty {kind} item").into());
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    if !std::env::args().any(|arg| arg == "--allow-test-inference") {
        return Err("Pass --allow-test-inference to create a temporary native conversation, send one minimal read-only prompt using your account allowance, and delete that test conversation. Empty threads have no rollout to resume in this runtime.".into());
    }
    let exercise_review = std::env::args().any(|arg| arg == "--exercise-review");
    let workspace = tempfile::tempdir()?;
    let runtime = Runtime::discover(workspace.path())?;
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn(&runtime, env!("CARGO_PKG_VERSION"), move |event| {
        let _ = tx.send(event);
    })?;
    loop {
        match rx.recv()? {
            Event::Ready { .. } => break,
            Event::Closed { reason } => return Err(reason.into()),
            Event::ServerRequest { .. } => {
                return Err("Unexpected server request before the probe was initialized".into());
            }
            _ => {}
        }
    }
    let requirements = api::requirements().send(&client)?.wait()?;
    api::Access::ReadOnly.validate_requirements(requirements.get("requirements"))?;
    let mut conversations = Conversations::default();
    let start = conversations.open(
        "chat:probe",
        workspace.path(),
        &api::Profile::default(),
        api::Access::ReadOnly,
    )?;
    let result = start.call.clone().send(&client)?.wait()?;
    let created_id = result["thread"]["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or("Native thread/start omitted its ID")?
        .to_owned();
    println!(
        "Disposable probe: {} · {created_id}",
        workspace.path().display()
    );
    let prepared = (|| -> Result<()> {
        conversations.complete(&start, Ok(result))?;
        let turn = conversations.start_turn(
            "chat:probe",
            &created_id,
            vec![api::text_input(
                "Reply exactly READY. Do not use tools, access files, or modify anything.",
            )],
            workspace.path(),
            &api::Profile::default(),
            api::Access::ReadOnly,
        )?;
        let response = turn.call.clone().send(&client)?.wait()?;
        let turn_id = response["turn"]["id"]
            .as_str()
            .ok_or("Missing initial turn ID")?
            .to_owned();
        conversations.complete(&turn, Ok(response))?;
        wait_turn(&rx, &mut conversations, &created_id, &turn_id, None)?;
        let prepare = conversations.prepare_review("chat:probe", workspace.path())?;
        let result = prepare.call.clone().send(&client)?.wait()?;
        println!(
            "Review preparation readback {}",
            serde_json::json!({
                "cwd": result["cwd"],
                "sandbox": result["sandbox"],
                "approvalPolicy": result["approvalPolicy"],
                "approvalsReviewer": result["approvalsReviewer"],
                "sameThread": result["thread"]["id"] == created_id,
            })
        );
        conversations.complete(&prepare, Ok(result))?;
        println!("Native read-only review scope confirmed.");
        if exercise_review {
            let original = serde_json::to_value(
                &conversations
                    .mirror
                    .thread(&created_id)
                    .ok_or("Missing prepared history")?
                    .turns,
            )?;
            let review = conversations.start_review("chat:probe", &api::ReviewTarget::Custom {
                instructions: "Review only this self-contained Rust snippet: `fn is_positive(n: i32) -> bool { n < 0 }`. Its specification is to return true exactly for positive integers. Report the correctness issue concisely using the normal review format. Do not use tools, inspect files, or change anything; all necessary code is in this instruction.".into(),
            }, "review-probe-request")?;
            let result = review.call.clone().send(&client)?.wait()?;
            let review_id = result["turn"]["id"]
                .as_str()
                .ok_or("Missing review turn ID")?
                .to_owned();
            conversations.complete(&review, Ok(result))?;
            println!(
                "Native review/start acknowledged turn {review_id}; waiting for actual review completion."
            );
            wait_turn(
                &rx,
                &mut conversations,
                &created_id,
                &review_id,
                Some(&client),
            )?;
            let history = conversations
                .mirror
                .thread(&created_id)
                .ok_or("Missing reviewed history")?;
            verify_review(
                history
                    .turns
                    .iter()
                    .find(|turn| turn.id == review_id)
                    .ok_or("Missing review turn")?,
            )?;
            let prior: Vec<_> = history
                .turns
                .iter()
                .filter(|turn| {
                    original
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|saved| saved["id"] == turn.id)
                })
                .collect();
            if serde_json::to_value(prior)? != original
                || conversations.saved().unresolved.contains_key("chat:probe")
            {
                return Err(
                    "Review changed earlier turns or retained its acceptance receipt".into(),
                );
            }
            let read = conversations.action("chat:probe", Action::Read)?;
            let result = read.call.clone().send(&client)?.wait()?;
            conversations.complete(&read, Ok(result))?;
            hydrate_pages(&mut conversations, &client, &created_id)?;
            let history = conversations
                .mirror
                .thread(&created_id)
                .ok_or("Missing reread history")?;
            verify_review(
                history
                    .turns
                    .iter()
                    .find(|turn| turn.id == review_id)
                    .ok_or("Review absent from native thread/read")?,
            )?;
            println!(
                "Native entered/exited review items and final review verified live and through thread/read; prior history unchanged."
            );
        }
        Ok(())
    })();
    // A failed probe must not leave its own inference running. Never interrupt
    // any ID except the thread created by this process and its observed turn.
    if prepared.is_err()
        && let Some(turn) = conversations
            .mirror
            .thread(&created_id)
            .and_then(|thread| thread.active_turn())
    {
        let _ = api::interrupt_turn(&created_id, &turn.id)
            .send(&client)
            .and_then(|ticket| ticket.wait());
    }
    let cleanup = api::delete_thread(&created_id)
        .send(&client)
        .and_then(|ticket| ticket.wait());
    client.shutdown();
    cleanup.map_err(|error| {
        format!("Could not delete the newly created test thread {created_id}: {error}")
    })?;
    prepared?;
    println!(
        "Codex {}: native review probe passed (review/start exercised: {}). The new test thread was deleted. No tool permission granted or local binding written.",
        runtime.version(),
        exercise_review
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use central_agent_codex_runtime::mirror::Mirror;
    use serde_json::json;

    fn completed_review() -> Turn {
        let mut mirror = Mirror::default();
        mirror
            .notify(
                "turn/completed",
                &json!({"threadId":"owned", "turn": {
                    "id":"review", "status":"completed", "error":null, "items":[
                        {"type":"enteredReviewMode","id":"entered","review":"fixture"},
                        {"type":"exitedReviewMode","id":"exited","review":"A correctness issue"}
                    ]
                }}),
            )
            .unwrap();
        mirror.thread("owned").unwrap().turns[0].clone()
    }

    #[test]
    fn acceptance_requires_both_completed_review_items_and_successful_turn() {
        let valid = completed_review();
        verify_review(&valid).unwrap();
        for status in ["inProgress", "failed", "interrupted"] {
            let mut invalid = valid.clone();
            invalid.status = status.into();
            assert!(verify_review(&invalid).is_err());
        }
        for index in 0..2 {
            let mut invalid = valid.clone();
            invalid.items[index].completed = false;
            assert!(verify_review(&invalid).is_err());
            invalid.items[index].completed = true;
            invalid.items[index].value["review"] = json!("  ");
            assert!(verify_review(&invalid).is_err());
        }
        let mut invalid = valid;
        invalid.error = Some(json!({"message":"failed"}));
        assert!(verify_review(&invalid).is_err());
    }

    #[test]
    fn another_threads_completion_cannot_satisfy_the_probe() {
        let (tx, rx) = mpsc::channel();
        for (thread, turn, status) in [
            ("other", "review", "completed"),
            ("owned", "old", "completed"),
            ("owned", "review", "failed"),
        ] {
            tx.send(Event::Notification {
                method: "turn/completed".into(),
                params: json!({
                    "threadId":thread,"turn":{"id":turn,"status":status,"items":[]}
                }),
            })
            .unwrap();
        }
        drop(tx);
        let error =
            wait_turn(&rx, &mut Conversations::default(), "owned", "review", None).unwrap_err();
        assert!(error.to_string().contains("did not complete"));
    }
}
