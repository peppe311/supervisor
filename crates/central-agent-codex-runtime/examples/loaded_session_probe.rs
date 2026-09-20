//! Optional no-inference probe of a private App Server process. Creates only
//! an ephemeral native session, verifies it is loaded, then shuts down its process.
//! No prompt, user history, configuration write or persistent local binding.
use central_agent_codex_runtime::{
    api,
    conversations::Conversations,
    runtime::Runtime,
    transport::{Client, Event},
};
use serde_json::json;
use std::{sync::mpsc, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let workspace = tempfile::tempdir()?;
    let runtime = Runtime::discover(workspace.path())?;
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn(&runtime, env!("CARGO_PKG_VERSION"), move |event| {
        let _ = tx.send(event);
    })?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        loop {
            match rx.recv_timeout(Duration::from_secs(30))? {
                Event::Ready { .. } => break,
                Event::Closed { reason } => return Err(reason.into()),
                Event::ServerRequest { .. } => {
                    return Err("Unexpected server request; no decision was sent".into());
                }
                _ => {}
            }
        }
        let requirements = api::requirements().send(&client)?.wait()?;
        api::Access::ReadOnly.validate_requirements(requirements.get("requirements"))?;
        let mut book = Conversations::default();
        let mut start = book.open(
            "chat:ephemeral-probe",
            workspace.path(),
            &api::Profile::default(),
            api::Access::ReadOnly,
        )?;
        start.call.params["ephemeral"] = json!(true);
        let reply = start.call.clone().send(&client)?.wait()?;
        if reply["thread"]["ephemeral"] != true {
            return Err("The server did not confirm an ephemeral session".into());
        }
        book.complete(&start, Ok(reply))?;
        if !book.ready_for_turn("chat:ephemeral-probe") {
            return Err("The new native session is not reusable before its first turn".into());
        }
        // Generated 0.153.4 ThreadLoadedListParams: default page size has no limit.
        // Only compare our newly created ID; never print other session IDs.
        let loaded = client.request("thread/loaded/list", json!({}))?.wait()?;
        let own_id = &book.binding("chat:ephemeral-probe").unwrap().thread_id;
        if !loaded["data"]
            .as_array()
            .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(own_id)))
        {
            return Err("The native loaded-session list did not contain the new session".into());
        }
        book.disconnect();
        if book.ready_for_turn("chat:ephemeral-probe") {
            return Err("Disconnected session was still marked reusable".into());
        }
        book.reset_unused_link("chat:ephemeral-probe")?;
        Ok(())
    })();
    client.shutdown();
    result?;
    println!(
        "Codex {}: an empty ephemeral session is idle, reusable and present in thread/loaded/list. Disconnect invalidates reuse; explicit local unlink succeeds. No inference, native history deletion or persistent local binding.",
        runtime.version()
    );
    Ok(())
}
