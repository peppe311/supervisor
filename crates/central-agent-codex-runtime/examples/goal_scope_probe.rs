//! Opt-in native goal-state probe. Never activates a goal or submits a prompt.
//! Creates one disposable native thread and removes only that exact thread.
use central_agent_codex_runtime::{
    api,
    goals::{self, Edit, UserStatus},
    runtime::Runtime,
    transport::{Client, Event},
};
use serde_json::{Value, json};
use std::{sync::mpsc, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !std::env::args().any(|arg| arg == "--allow-disposable-state") {
        return Err("Pass --allow-disposable-state to create and remove one test thread and paused goal; no inference.".into());
    }
    let workspace = tempfile::tempdir()?;
    let runtime = Runtime::discover(workspace.path())?;
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn(&runtime, env!("CARGO_PKG_VERSION"), move |event| {
        let _ = tx.send(event);
    })?;
    let mut own_id: Option<String> = None;
    let run = (|| -> Result<(), Box<dyn std::error::Error>> {
        loop {
            match rx.recv_timeout(Duration::from_secs(30))? {
                Event::Ready { .. } => break,
                Event::Closed { reason } => return Err(reason.into()),
                Event::ServerRequest { .. } => {
                    return Err("Unexpected request; no approval sent".into());
                }
                _ => {}
            }
        }
        let requirements = api::requirements().send(&client)?.wait()?;
        api::Access::ReadOnly.validate_requirements(requirements.get("requirements"))?;
        let started=client.request("thread/start",json!({"cwd":workspace.path(),"sandbox":"read-only","approvalPolicy":"untrusted","approvalsReviewer":"user"}))?.wait()?;
        let thread = started["thread"]["id"]
            .as_str()
            .ok_or("Missing owned thread")?
            .to_owned();
        own_id = Some(thread.clone());
        let get = || -> Result<Value, Box<dyn std::error::Error>> {
            Ok(goals::get(&thread).send(&client)?.wait()?)
        };
        if goals::decode(&thread, &get()?)?.is_some() {
            return Err("Fresh test thread unexpectedly has a goal".into());
        }
        let created = Edit::Replace {
            objective: "Disposable paused-state probe; do not perform work.".into(),
            status: UserStatus::Paused,
            token_budget: None,
        }
        .call(&thread)?
        .send(&client)?
        .wait()?;
        goals::decode(&thread, &created)?;
        if created["goal"]["status"] != "paused"
            || created["goal"]["threadId"] != thread
            || created["goal"]["tokensUsed"] != 0
        {
            return Err("Native goal did not remain paused/unused in the owned scope".into());
        }
        println!("Native goal: paused creation and exact thread scope confirmed.");
        let budget = Edit::Budget {
            token_budget: Some(1000),
        }
        .call(&thread)?
        .send(&client)?
        .wait()?;
        if budget["goal"]["status"] != "paused"
            || budget["goal"]["tokenBudget"] != 1000
            || budget["goal"]["createdAt"] != created["goal"]["createdAt"]
        {
            return Err("Budget edit changed goal state/accounting origin".into());
        }
        let cleared_budget = Edit::Budget { token_budget: None }
            .call(&thread)?
            .send(&client)?
            .wait()?;
        if goals::decode(&thread, &cleared_budget)?
            .ok_or("Missing edited goal")?
            .token_budget
            .is_some()
        {
            return Err("Explicit null did not remove the optional budget".into());
        }
        println!(
            "Native goal: explicit null budget result = {}.",
            cleared_budget["goal"]["tokenBudget"]
        );
        let read = get()?;
        if read["goal"] != cleared_budget["goal"] {
            return Err("Native goal readback disagrees with write result".into());
        }
        let cleared = goals::clear(&thread).send(&client)?.wait()?;
        if !goals::decode_clear(&cleared)? {
            return Err("Existing owned goal was not cleared".into());
        }
        if goals::decode(&thread, &get()?)?.is_some() {
            return Err("Cleared native goal remained present".into());
        }
        if goals::decode_clear(&goals::clear(&thread).send(&client)?.wait()?)? {
            return Err("Second clear unexpectedly removed another goal".into());
        }
        for event in rx.try_iter() {
            match event {
                Event::ServerRequest { .. } => {
                    return Err("Unexpected request in paused probe".into());
                }
                Event::Notification { method, .. } if method == "turn/started" => {
                    return Err("Paused goal started an unsolicited turn".into());
                }
                _ => {}
            }
        }
        Ok(())
    })();
    let cleanup = if let Some(thread) = own_id {
        let cleared = goals::clear(&thread)
            .send(&client)
            .and_then(|ticket| ticket.wait());
        let deleted = api::delete_thread(&thread)
            .send(&client)
            .and_then(|ticket| ticket.wait());
        match (cleared, deleted) {
            (Ok(_), Ok(_)) => {
                println!("Deleted exact owned test thread {thread}.");
                Ok(())
            }
            (a, b) => Err(format!("Owned goal/thread cleanup failed: {a:?}; {b:?}")),
        }
    } else {
        Ok(())
    };
    client.shutdown();
    cleanup?;
    run?;
    println!(
        "PASS: native paused goal create/get/budget/clear; no prompt, activation, visual test or personal configuration mutation."
    );
    Ok(())
}
