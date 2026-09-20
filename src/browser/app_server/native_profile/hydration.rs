//! Rebuild native paginated projections through the pinned public API. This is
//! a one-time migration operation, never turn replay or normal startup resume.
use super::*;
use central_agent_codex_runtime::{
    api,
    transport::{Client, Event},
};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

pub(super) fn finish(runtime: &Runtime) -> Result<(), String> {
    let home = runtime
        .home()
        .ok_or("A dedicated native profile is required")?;
    let marker = home.join(MARKER);
    let mut report: Report =
        serde_json::from_slice(&fs::read(&marker).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    if report.hydrated && report.metadata_imported {
        return Ok(());
    }
    super::metadata::capture(runtime, &mut report)?;
    save(home, &report)?;
    if !report.copied.is_empty() {
        let (client, events) = connect(runtime)?;
        let outcome: Result<(), String> = (|| {
            for (id, record) in &report.copied {
                let archived = record.path.starts_with("archived_sessions");
                if archived {
                    let observed = call(&client, &events, api::read_thread_metadata(id))?;
                    let still_archived = observed["thread"]["path"].as_str().is_some_and(|path| {
                        Path::new(path)
                            .canonicalize()
                            .is_ok_and(|path| path.starts_with(home.join("archived_sessions")))
                    });
                    if still_archived {
                        call(&client, &events, api::unarchive_thread(id))?;
                    }
                }
                // Hydrate from a neutral directory, including when the original
                // project was removed. Keep stored project metadata; no prompt,
                // project configuration or tool approval is supplied.
                let resume = call(
                    &client,
                    &events,
                    api::resume_thread_with_options(
                        id,
                        &api::ThreadResumeOptions {
                            access: Some(api::Access::ReadOnly),
                            exclude_turns: true,
                            cwd: Some(home.to_string_lossy().into_owned()),
                            ..api::ThreadResumeOptions::default()
                        },
                    ),
                )
                .and_then(|_| call(&client, &events, api::list_thread_turns(id, None)))
                .and_then(|_| {
                    super::metadata::import(&client, &events, id, report.metadata.get(id))
                });
                if archived {
                    let restore = call(&client, &events, api::archive_thread(id));
                    resume?;
                    restore?;
                } else {
                    resume?;
                    call(&client, &events, api::unsubscribe_thread(id))?;
                }
            }
            Ok(())
        })();
        client.shutdown();
        outcome?;
    }
    report.hydrated = true;
    report.metadata_imported = true;
    save(home, &report)
}

fn save(home: &Path, report: &Report) -> Result<(), String> {
    let mut replacement =
        tempfile::NamedTempFile::new_in(home).map_err(|error| error.to_string())?;
    replacement
        .write_all(&serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?)
        .and_then(|()| replacement.as_file().sync_all())
        .map_err(|error| error.to_string())?;
    replacement
        .persist(home.join(MARKER))
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(super) fn connect(runtime: &Runtime) -> Result<(Client, mpsc::Receiver<Event>), String> {
    let (sender, events) = mpsc::channel();
    let client = Client::spawn(runtime, env!("CARGO_PKG_VERSION"), move |event| {
        let _ = sender.send(event);
    })
    .map_err(|error| error.to_string())?;
    let ready: Result<(), String> = (|| {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match events.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(Event::Ready { .. }) => return Ok(()),
                Ok(event) => passive(event)?,
                Err(_) => return Err("The native migration connection did not become ready".into()),
            }
        }
    })();
    if let Err(error) = ready {
        client.shutdown();
        return Err(error);
    }
    Ok((client, events))
}

fn passive(event: Event) -> Result<(), String> {
    match event {
        Event::ServerRequest { .. } => Err(
            "The native migration requested an interactive action; no permission was granted"
                .into(),
        ),
        Event::Notification { method, .. } if method == "turn/started" => {
            Err("The native migration unexpectedly started work".into())
        }
        Event::Closed { reason } => Err(reason),
        _ => Ok(()),
    }
}

pub(super) fn call(
    client: &Client,
    events: &mpsc::Receiver<Event>,
    call: api::Call,
) -> Result<serde_json::Value, String> {
    let ticket = call.send(client).map_err(|error| error.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        while let Ok(event) = events.try_recv() {
            passive(event)?;
        }
        match ticket.try_result() {
            Ok(result) => return result.map_err(|error| error.to_string()),
            Err(mpsc::TryRecvError::Disconnected) => {
                return Err("The native migration response was lost".into());
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if Instant::now() >= deadline {
            return Err("Native history reconstruction timed out".into());
        }
        match events.recv_timeout(Duration::from_millis(50)) {
            Ok(event) => passive(event)?,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("The native migration connection closed".into());
            }
        }
    }
}
