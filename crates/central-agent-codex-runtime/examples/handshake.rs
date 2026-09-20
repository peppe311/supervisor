//! Read-only, no-inference smoke check of a locally installed official runtime.
//! It initializes one private server and reads public account/catalog/requirements
//! responses, never credential files, threads, configuration writes, tools or prompts.
use central_agent_codex_runtime::{
    api,
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
    loop {
        match rx.recv_timeout(Duration::from_secs(30))? {
            Event::Ready { .. } => break,
            Event::Closed { reason } => return Err(reason.into()),
            Event::ServerRequest { key, .. } => {
                client.answer(
                    &key,
                    Err(central_agent_codex_runtime::wire::RpcError {
                        code: -32601,
                        message: "Not supported by the no-inference smoke check".into(),
                        data: None,
                    }),
                )?;
            }
            _ => {}
        }
    }
    let account = api::account_read().send(&client)?.wait()?;
    if account.get("account").is_none()
        || !account
            .get("requiresOpenaiAuth")
            .is_some_and(|value| value.is_boolean())
    {
        return Err("Invalid account/read response".into());
    }
    let requirements = api::requirements().send(&client)?.wait()?;
    if !requirements
        .get("requirements")
        .is_some_and(|value| value.is_null() || value.is_object())
    {
        return Err("Invalid configRequirements/read response".into());
    }
    let mut cursor = None;
    let mut cursors = std::collections::HashSet::new();
    let mut count = 0;
    loop {
        let models = api::model_list(cursor.as_deref()).send(&client)?.wait()?;
        count += models["data"]
            .as_array()
            .ok_or("Invalid model/list response")?
            .len();
        cursor = models
            .get("nextCursor")
            .and_then(|value| value.as_str())
            .map(str::to_owned);
        let Some(next) = cursor.as_ref() else { break };
        if !cursors.insert(next.clone()) {
            return Err("Repeated model pagination cursor".into());
        }
    }
    println!(
        "Codex {}: handshake, account/read, configRequirements/read and model/list passed ({count} entries). No inference requested; no account details printed.",
        runtime.version()
    );
    if std::env::args().any(|arg| arg == "--media-capabilities") {
        let capabilities = client
            .request("modelProvider/capabilities/read", json!({}))?
            .wait()?;
        for field in ["namespaceTools", "imageGeneration", "webSearch"] {
            if !capabilities[field].is_boolean() {
                return Err(format!("Missing native provider capability: {field}").into());
            }
        }
        println!(
            "Native provider bounds: namespaceTools={}, imageGeneration={}, webSearch={}",
            capabilities["namespaceTools"],
            capabilities["imageGeneration"],
            capabilities["webSearch"]
        );
        let mut cursor = None;
        let mut cursors = std::collections::HashSet::new();
        let mut image_features = 0;
        loop {
            let page = client
                .request(
                    "experimentalFeature/list",
                    json!({"cursor":cursor,"limit":100}),
                )?
                .wait()?;
            for feature in page["data"]
                .as_array()
                .ok_or("Invalid native feature page")?
            {
                // Only public image-related capability metadata, no descriptions,
                // announcements, model input, credentials or complete config.
                if feature["name"]
                    .as_str()
                    .is_some_and(|name| name.contains("image"))
                {
                    if !feature["enabled"].is_boolean() || !feature["stage"].is_string() {
                        return Err("Invalid native image feature metadata".into());
                    }
                    println!(
                        "Native image feature: name={}, stage={}, enabled={}",
                        feature["name"], feature["stage"], feature["enabled"]
                    );
                    image_features += 1;
                }
            }
            cursor = match &page["nextCursor"] {
                serde_json::Value::Null => None,
                serde_json::Value::String(value) => Some(value.clone()),
                _ => return Err("Invalid native feature cursor".into()),
            };
            let Some(next) = &cursor else { break };
            if !cursors.insert(next.clone()) {
                return Err("Repeated native feature cursor".into());
            }
        }
        println!(
            "Image-related feature entries: {image_features}. These are provider/config bounds, not proof of per-model availability or successful generation. No feature enabled, prompt submitted or image generated."
        );
    }
    client.shutdown();
    Ok(())
}
