//! Read-only native metadata probe; never runs a skill or changes its configuration.
use central_agent_codex_runtime::{
    api,
    runtime::Runtime,
    transport::{Client, Event},
};
use std::{path::Path, sync::mpsc, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?;
    let runtime = Runtime::discover(&cwd)?;
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn(&runtime, env!("CARGO_PKG_VERSION"), move |event| {
        let _ = tx.send(event);
    })?;
    let result = (|| -> Result<(usize, usize), Box<dyn std::error::Error>> {
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
        let response = api::skills_list(cwd.to_str().ok_or("The probe directory must be UTF-8")?)
            .send(&client)?
            .wait()?;
        let entries = response["data"].as_array().ok_or("No native skills data")?;
        if entries.len() != 1 || entries[0]["cwd"].as_str().map(Path::new) != Some(cwd.as_path()) {
            return Err("Native skill discovery returned a different directory".into());
        }
        let skills = entries[0]["skills"]
            .as_array()
            .ok_or("Missing native skills")?;
        let errors = entries[0]["errors"]
            .as_array()
            .ok_or("Missing native discovery issues")?;
        if skills.iter().any(|s| {
            s["enabled"].as_bool().is_none()
                || s["name"].as_str().is_none()
                || s["path"]
                    .as_str()
                    .is_none_or(|p| !Path::new(p).is_absolute())
        }) {
            return Err("Native skill metadata is malformed".into());
        }
        Ok((skills.len(), errors.len()))
    })();
    client.shutdown();
    let (skills, issues) = result?;
    println!(
        "Codex {}: skills/list returned {skills} skill entries and {issues} discovery issues for the requested directory. No skill content, names or paths printed; no inference, configuration writes or native conversation created.",
        runtime.version()
    );
    Ok(())
}
