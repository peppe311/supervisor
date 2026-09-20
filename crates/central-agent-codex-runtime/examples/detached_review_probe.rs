//! Opt-in real-runtime detached review contract acceptance on disposable native
//! histories. No existing history, private files, model replay or config writes.
use central_agent_codex_runtime::{
    api,
    conversations::Conversations,
    runtime::Runtime,
    transport::{CallError, Client, Event},
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::mpsc};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn review_finished(snapshot: &Value, thread: &str, turn: &str) -> bool {
    snapshot["thread"]["id"] == thread
        && snapshot["thread"]["turns"].as_array().is_some_and(|turns| {
            turns.iter().any(|entry| {
                entry["id"] == turn
                    && entry["status"] == "completed"
                    && entry["error"].is_null()
                    && ["enteredReviewMode", "exitedReviewMode"]
                        .iter()
                        .all(|kind| {
                            entry["items"].as_array().is_some_and(|items| {
                                items.iter().any(|item| {
                                    item["type"] == *kind
                                        && item["review"]
                                            .as_str()
                                            .is_some_and(|text| !text.trim().is_empty())
                                })
                            })
                        })
            })
        })
}

fn event_is_terminal(event: Event, thread: &str, turn: &str) -> Result<bool> {
    match event {
        Event::Notification { method, params } if params["threadId"] == thread => {
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
                    return Err(format!("Unexpected activity in no-tool probe: {kind}").into());
                }
            }
            if method == "turn/completed" && params["turn"]["id"] == turn {
                if params["turn"]["status"] != "completed" {
                    return Err("Native probe turn failed or was interrupted".into());
                }
                return Ok(true);
            }
            Ok(
                method == "item/completed" && params["item"]["type"] == "exitedReviewMode"
                    || method == "thread/status/changed" && params["status"]["type"] == "idle",
            )
        }
        Event::Closed { reason } => Err(reason.into()),
        Event::ServerRequest { .. } => {
            Err("Unexpected native decision; no permission granted".into())
        }
        _ => Ok(false),
    }
}

fn main() -> Result<()> {
    if !std::env::args().any(|arg| arg == "--allow-test-inference") {
        return Err("Pass --allow-test-inference to create disposable source/review histories, use account allowance for one tiny prompt plus a detached review, and delete only those histories.".into());
    }
    let expect_rejection = std::env::args().any(|arg| arg == "--expect-paginated-rejection");
    let workspace = tempfile::tempdir()?;
    let runtime = Runtime::discover(workspace.path())?;
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn(&runtime, env!("CARGO_PKG_VERSION"), move |event| {
        let _ = tx.send(event);
    })?;
    loop {
        let event = rx.recv()?;
        if matches!(event, Event::Ready { .. }) {
            break;
        }
        event_is_terminal(event, "", "")?;
    }
    let requirements = api::requirements().send(&client)?.wait()?;
    api::Access::ReadOnly.validate_requirements(requirements.get("requirements"))?;
    let mut book = Conversations::default();
    let start = book.open(
        "chat:detached-probe",
        workspace.path(),
        &api::Profile::default(),
        api::Access::ReadOnly,
    )?;
    let response = start.call.clone().send(&client)?.wait()?;
    let source = response["thread"]["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .ok_or("Missing created source ID")?
        .to_owned();
    let mut owned = BTreeSet::from([source.clone()]);
    println!(
        "Disposable source: {} · {source}",
        workspace.path().display()
    );
    let acceptance = (|| -> Result<&str> {
        book.complete(&start, Ok(response))?;
        let request = book.start_turn(
            "chat:detached-probe",
            "detached-probe-initial",
            vec![api::text_input(
                "Reply exactly READY. Do not use tools, read files or modify anything.",
            )],
            workspace.path(),
            &api::Profile::default(),
            api::Access::ReadOnly,
        )?;
        let response = request.call.clone().send(&client)?.wait()?;
        let turn = response["turn"]["id"]
            .as_str()
            .ok_or("Missing source turn ID")?
            .to_owned();
        book.complete(&request, Ok(response))?;
        loop {
            let event = rx.recv()?;
            if let Event::Notification {
                ref method,
                ref params,
            } = event
            {
                book.notification(method, params)?;
            }
            if event_is_terminal(event, &source, &turn)? && !book.busy("chat:detached-probe") {
                break;
            }
        }
        let prepare = book.prepare_review("chat:detached-probe", workspace.path())?;
        let response = prepare.call.clone().send(&client)?.wait()?;
        book.complete(&prepare, Ok(response))?;
        let before = api::read_thread(&source).send(&client)?.wait()?;
        println!(
            "Native source history format: {}",
            before["thread"]["historyMode"]
        );
        let mut list = api::search_threads(None, false, None);
        list.params["cwd"] = json!(workspace.path());
        list.params["useStateDbOnly"] = json!(true);
        let listed_before = list.clone().send(&client)?.wait()?;
        if !listed_before["nextCursor"].is_null() {
            return Err("Unexpected additional probe-history page".into());
        }
        let target = api::ReviewTarget::Custom { instructions:"Review only this Rust snippet: `fn is_positive(n: i32) -> bool { n < 0 }`. It must return true exactly for positive integers. Report the correctness issue concisely in the normal review format. Do not use tools, access files or change anything; all necessary code is included here.".into() };
        let response = match api::start_detached_review(&source, &target)
            .send(&client)?
            .wait()
        {
            Ok(response) => response,
            Err(CallError::Rpc(error))
                if expect_rejection
                    && before["thread"]["historyMode"] == "paginated"
                    && error.code == -32600
                    && error.message == "paginated threads do not support detached review" =>
            {
                let after = api::read_thread(&source).send(&client)?.wait()?;
                let listed_after = list.send(&client)?.wait()?;
                let ids = |page: &Value| -> BTreeSet<String> {
                    page["data"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|entry| entry["id"].as_str().map(str::to_owned))
                        .collect()
                };
                if before["thread"]["turns"] != after["thread"]["turns"]
                    || ids(&listed_before) != ids(&listed_after)
                    || !listed_after["nextCursor"].is_null()
                {
                    return Err(
                        "Native rejection changed the source or created additional probe histories"
                            .into(),
                    );
                }
                println!(
                    "Verified native rejection: {}. Source history unchanged; no detached history created.",
                    error.message
                );
                return Ok("unavailable: native paginated-history rejection");
            }
            Err(error) => return Err(error.into()),
        };
        let review = response["reviewThreadId"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or("Missing detached review ID")?
            .to_owned();
        if review == source {
            return Err("Detached review reused its source thread".into());
        }
        owned.insert(review.clone());
        let review_turn = response["turn"]["id"]
            .as_str()
            .ok_or("Missing detached review turn")?
            .to_owned();
        println!("Detached review acknowledged: {review} · turn {review_turn}");
        let mut saw_started = false;
        let completed = loop {
            let event = rx.recv()?;
            if let Event::Notification {
                ref method,
                ref params,
            } = event
                && method == "thread/started"
                && params["thread"]["id"] == review
            {
                saw_started = true;
                println!(
                    "New native thread observed: {}",
                    json!({"forkedFromId":params["thread"]["forkedFromId"],"source":params["thread"]["source"]})
                );
            }
            if event_is_terminal(event, &review, &review_turn)? {
                let snapshot = api::read_thread(&review).send(&client)?.wait()?;
                if review_finished(&snapshot, &review, &review_turn) {
                    break snapshot;
                }
            }
        };
        let after = api::read_thread(&source).send(&client)?.wait()?;
        if before["thread"]["turns"] != after["thread"]["turns"] {
            return Err("Detached review modified source history".into());
        }
        if !saw_started {
            return Err("No matching native thread/started event observed".into());
        }
        if completed["thread"]["forkedFromId"] != source {
            return Err("Native detached review did not preserve the source fork identity".into());
        }
        println!(
            "Native detached review completed with both review items and exact fork provenance; source history unchanged."
        );
        if expect_rejection {
            return Err("Runtime now accepts detached review; update compatibility handling and test its full UI before enabling it".into());
        }
        Ok("completed detached review")
    })();
    // Exact IDs captured only from this probe's create/review responses. Delete
    // independent review first; native deletion includes its spawned reviewer.
    let mut failures = vec![];
    let mut ids: Vec<_> = owned.into_iter().collect();
    ids.sort_by_key(|id| id == &source);
    for id in ids {
        if let Err(error) = api::delete_thread(&id)
            .send(&client)
            .and_then(|ticket| ticket.wait())
        {
            failures.push(format!("Cleanup {id}: {error}"));
        } else {
            println!("Deleted disposable native history: {id}");
        }
    }
    client.shutdown();
    if !failures.is_empty() {
        return Err(failures.join("; ").into());
    }
    let outcome = acceptance?;
    println!(
        "Codex {} detached-review compatibility probe passed ({outcome}); only disposable histories deleted. No native configuration or personal history changed.",
        runtime.version()
    );
    Ok(())
}
