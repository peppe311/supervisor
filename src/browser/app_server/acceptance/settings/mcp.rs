//! Compiled global Settings controls, official writes and real fixture inventory.
use super::*;

const NAME: &str = "host_mcp_fixture";
const CONFIRM: &str = "[aria-label='Confirm native MCP configuration change']";
const RELOAD: &str = "[aria-label='Reload native MCP configuration']";

pub(super) fn sample(owner: &str) -> String {
    format!(
        r#"(()=>{{try{{const root=document;const state={};
      const mcp=document.querySelector('[data-central-agent-svelte="app-server-account"] .native-mcp');
      const config=mcp?.querySelector('.mcp-configuration');
      const row=[...config?.querySelectorAll('.server-setting')||[]].find(r=>r.querySelector('strong')?.textContent==={});
      state.mcp={{phase:window.__mcpHostPhase||0,ready:!!mcp,settings:document.body.classList.contains('settings-open'),
        refresh:[...config?.querySelectorAll('button')||[]].some(b=>b.textContent==='Refresh configuration'&&!b.disabled),
        add:config?.querySelector('form fieldset')?.disabled===false,
        present:!!row,enabled:row?.querySelector('p')?.textContent?.includes(' · Enabled ·'),
        confirmation:document.querySelector({})?.open===true,reload:document.querySelector({})?.open===true,
        runtime:[...mcp?.querySelectorAll('.server')||[]].find(r=>r.querySelector('summary')?.textContent==={})?.textContent||'',
        error:mcp?.querySelector('[role=alert]')?.textContent||'',
        privateExposed:document.body.textContent.includes({})}};return state;
      }}catch(error){{return {{errors:['MCP settings sample failed: '+String(error)]}};}}}})()"#,
        delivery::sample(owner),
        json!(NAME),
        json!(CONFIRM),
        json!(RELOAD),
        json!(NAME),
        json!(PRIVATE)
    )
}

fn row(app: &BrowserApp, owner: &str, label: &str) -> anyhow::Result<()> {
    script(
        app,
        owner,
        &format!(
            "const row=[...root.querySelectorAll('.mcp-configuration .server-setting')].find(r=>r.querySelector('strong')?.textContent==={});const button=[...row?.querySelectorAll('button')||[]].find(b=>b.textContent==={});if(!button||button.disabled)throw new Error('MCP row control unavailable');button.click();",
            json!(NAME),
            json!(label)
        ),
    )
}
fn read(app: &BrowserApp) -> anyhow::Result<Value> {
    api::config_read(app.workspace.root().unwrap().to_str().unwrap())
        .send(app.app_server.client.as_ref().unwrap())?
        .wait()
        .map_err(Into::into)
}

#[derive(Default)]
pub(super) struct Check {
    phase: u8,
    writes: usize,
    reloads: usize,
    observed_reload: bool,
    error: Option<String>,
}
impl Check {
    pub(super) fn observe(&mut self, event: &BrowserEvent) {
        if let BrowserEvent::AppServer(Event::McpReply { result, .. }) = event {
            match result {
                Ok(value) if value.get("filePath").is_some() => self.writes += 1,
                Ok(value) if value.as_object().is_some_and(|v| v.is_empty()) => self.reloads += 1,
                Err(error) => self.error = Some(format!("Native MCP host request failed: {error}")),
                _ => {}
            }
        }
    }
    pub(super) fn tick(
        &mut self,
        app: &mut BrowserApp,
        owner: &str,
        state: &Value,
    ) -> anyhow::Result<bool> {
        if let Some(error) = &self.error {
            anyhow::bail!("{error}");
        }
        let m = &state["mcp"];
        if m["phase"].as_u64() != Some(u64::from(self.phase)) {
            return Ok(false);
        }
        anyhow::ensure!(
            m["privateExposed"] != true,
            "Private native configuration entered Settings"
        );
        anyhow::ensure!(
            m["error"].as_str().is_none_or(str::is_empty),
            "MCP UI failed: {}",
            m["error"]
        );
        if self.phase > 0 && state["draft"] != DRAFT {
            return Ok(false);
        }
        if self.phase == 13
            && self.reloads == 1
            && !app.app_server.mcp.view.busy
            && !self.observed_reload
        {
            self.observed_reload = true;
            println!(
                "MCP Settings host reload inventory: {}",
                serde_json::to_string(&app.app_server.mcp.view)?
            );
        }
        let previous = self.phase;
        match self.phase {
            0 if m["ready"] == true => {
                input(app, owner, DRAFT, false)?;
                app.open_settings();
                script(
                    app,
                    owner,
                    "root.querySelector('[data-settings-target=\"settings-ai\"]').click();root.querySelector('.native-mcp').open=true;root.querySelector('.mcp-configuration').open=true;",
                )?;
                click(app, owner, ".mcp-configuration", "Refresh configuration")?;
                self.phase = 1;
            }
            1 if m["settings"] == true && m["add"] == true => {
                let root = context()?.context("Missing owned MCP settings root")?;
                // Node's script loader rejects the verbatim Windows prefix
                // returned by canonicalize; only normalize these owned fixture
                // arguments, never arbitrary user configuration or cleanup paths.
                let script_path = root.join("inventory-mcp.mjs");
                let trace_path = root.join("inventory-methods.txt");
                let script_text = script_path.to_string_lossy();
                let trace_text = trace_path.to_string_lossy();
                let args = json!([
                    script_text.strip_prefix(r"\\?\").unwrap_or(&script_text),
                    trace_text.strip_prefix(r"\\?\").unwrap_or(&trace_text)
                ])
                .to_string();
                script(
                    app,
                    owner,
                    &format!(
                        r#"const form=root.querySelector('.mcp-configuration form');form.closest('details').open=true;const fields=form.querySelectorAll('input');fields[0].value={};fields[1].value='node';for(const field of [fields[0],fields[1]])field.dispatchEvent(new Event('input',{{bubbles:true}}));const args=form.querySelector('textarea');args.value={};args.dispatchEvent(new Event('input',{{bubbles:true}}));queueMicrotask(()=>form.requestSubmit());"#,
                        json!(NAME),
                        json!(args)
                    ),
                )?;
                self.phase = 2;
            }
            2 if m["confirmation"] == true => {
                anyhow::ensure!(self.writes == 0, "MCP add wrote before confirmation");
                click(app, owner, CONFIRM, "Cancel")?;
                self.phase = 3;
            }
            3 if m["confirmation"] == false => {
                anyhow::ensure!(self.writes == 0, "Cancelled MCP add wrote configuration");
                script(
                    app,
                    owner,
                    "root.querySelector('.mcp-configuration form').requestSubmit();",
                )?;
                self.phase = 4;
            }
            4 if m["confirmation"] == true => {
                click(app, owner, CONFIRM, "Save change")?;
                self.phase = 5;
            }
            5 if self.writes == 1 && m["refresh"] == true => {
                anyhow::ensure!(
                    read(app)?["config"]["mcp_servers"][NAME]["enabled"] == false,
                    "New server was not saved disabled"
                );
                anyhow::ensure!(
                    !context()?.unwrap().join("inventory-methods.txt").exists(),
                    "Saving a disabled entry unexpectedly started the fixture"
                );
                click(app, owner, ".mcp-configuration", "Refresh configuration")?;
                self.phase = 6;
            }
            6 if m["present"] == true && m["add"] == true => {
                row(app, owner, "Enable…")?;
                self.phase = 7;
            }
            7 if m["confirmation"] == true => {
                click(app, owner, CONFIRM, "Save change")?;
                self.phase = 8;
            }
            8 if self.writes == 2 && m["refresh"] == true => {
                anyhow::ensure!(
                    read(app)?["config"]["mcp_servers"][NAME]["enabled"] == true,
                    "Native enable was not persisted"
                );
                click(app, owner, ".native-mcp", "Refresh servers")?;
                self.phase = 9;
            }
            9 if !app.app_server.mcp.view.busy && app.app_server.mcp.view.current => {
                click(app, owner, ".native-mcp", "Reload configuration…")?;
                self.phase = 10;
            }
            10 if m["reload"] == true => {
                anyhow::ensure!(self.reloads == 0, "MCP reload ran before confirmation");
                click(app, owner, RELOAD, "Cancel")?;
                self.phase = 11;
            }
            11 if m["reload"] == false => {
                anyhow::ensure!(self.reloads == 0, "Cancelled MCP reload was sent");
                click(app, owner, ".native-mcp", "Reload configuration…")?;
                self.phase = 12;
            }
            12 if m["reload"] == true => {
                click(app, owner, RELOAD, "Reload configuration")?;
                self.phase = 13;
            }
            13 if self.reloads == 1
                && !app.app_server.mcp.view.busy
                && m["runtime"]
                    .as_str()
                    .is_some_and(|s| s.contains("fixture_echo")) =>
            {
                click(app, owner, ".mcp-configuration", "Refresh configuration")?;
                self.phase = 14;
            }
            14 if m["add"] == true && m["enabled"] == true => {
                row(app, owner, "Disable…")?;
                self.phase = 15;
            }
            15 if m["confirmation"] == true => {
                click(app, owner, CONFIRM, "Save change")?;
                self.phase = 16;
            }
            16 if self.writes == 3 && m["refresh"] == true => {
                anyhow::ensure!(
                    read(app)?["config"]["mcp_servers"][NAME]["enabled"] == false,
                    "Native disable was not persisted"
                );
                click(app, owner, ".mcp-configuration", "Refresh configuration")?;
                self.phase = 17;
            }
            17 if m["add"] == true && m["present"] == true && m["enabled"] == false => {
                row(app, owner, "Remove user entry…")?;
                self.phase = 18;
            }
            18 if m["confirmation"] == true => {
                click(app, owner, CONFIRM, "Cancel")?;
                self.phase = 19;
            }
            19 if m["confirmation"] == false => {
                anyhow::ensure!(self.writes == 3, "Cancelled remove changed MCP config");
                row(app, owner, "Remove user entry…")?;
                self.phase = 20;
            }
            20 if m["confirmation"] == true => {
                click(app, owner, CONFIRM, "Save change")?;
                self.phase = 21;
            }
            21 if self.writes == 4 && m["refresh"] == true => {
                let config = read(app)?;
                anyhow::ensure!(
                    config["config"]["mcp_servers"].get(NAME).is_none(),
                    "Native remove retained server entry"
                );
                anyhow::ensure!(
                    config["config"]["mcp_servers"]["disabled_fixture"]["env"]["FIXTURE_VALUE"]
                        == PRIVATE,
                    "MCP writes changed unrelated private config"
                );
                click(app, owner, ".mcp-configuration", "Refresh configuration")?;
                self.phase = 22;
            }
            22 if m["add"] == true && m["present"] == false => {
                let trace = fs::read_to_string(context()?.unwrap().join("inventory-methods.txt"))?;
                anyhow::ensure!(
                    trace.lines().any(|s| s == "tools/list")
                        && trace.lines().any(|s| s == "resources/list")
                        && !trace
                            .lines()
                            .any(|s| s == "tools/call" || s == "resources/read"),
                    "MCP inventory did not stay inspection-only"
                );
                anyhow::ensure!(
                    self.writes == 4
                        && self.reloads == 1
                        && app.app_server.conversations.saved().bindings.is_empty()
                        && app
                            .project_chats
                            .iter()
                            .all(|chat| chat.messages.is_empty()),
                    "MCP Settings created unexpected work"
                );
                println!(
                    "MCP Settings host: add disabled, enable, disable, remove, explicit reload, cancellation, real tool/resource inventory, unchanged draft/private settings verified; zero inference/tool calls."
                );
                return Ok(true);
            }
            _ => {}
        }
        if previous != self.phase {
            script(
                app,
                owner,
                &format!("window.__mcpHostPhase={};", self.phase),
            )?;
            println!("MCP Settings host phase {}", self.phase);
        }
        Ok(false)
    }
}
