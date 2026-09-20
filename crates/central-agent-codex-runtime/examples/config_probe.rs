//! Read-only native configuration probe. Prints counts/booleans, never settings,
//! account details, file paths, raw layers, instructions or environment values.
use central_agent_codex_runtime::{
    api,
    configuration::Snapshot,
    runtime::Runtime,
    transport::{Client, Event},
};
use std::{sync::mpsc, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cwd = std::env::current_dir()?;
    let runtime = Runtime::discover(&cwd)?;
    let (tx, rx) = mpsc::channel();
    let client = Client::spawn(&runtime, env!("CARGO_PKG_VERSION"), move |event| {
        let _ = tx.send(event);
    })?;
    let result = (|| -> Result<Snapshot, Box<dyn std::error::Error>> {
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
        let response = api::config_read(cwd.to_str().ok_or("The probe directory must be UTF-8")?)
            .send(&client)?
            .wait()?;
        Ok(Snapshot::read(&response)?)
    })();
    client.shutdown();
    let snapshot = result?;
    println!(
        "Codex {}: config/read projected {} public preferences and {} layer descriptors; versioned user target available: {}. No values, paths or private config printed; no writes, inference or conversation created.",
        runtime.version(),
        snapshot.preferences.len(),
        snapshot.layers.len(),
        snapshot.target.is_some()
    );
    Ok(())
}
