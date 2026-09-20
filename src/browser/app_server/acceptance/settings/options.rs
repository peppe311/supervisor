//! Real typed MCP option controls, with isolated disabled STDIO/HTTP entries.
use super::*;
const CONFIRM: &str = "[aria-label='Confirm native MCP configuration change']";
pub(super) fn enabled() -> bool {
    std::env::args_os().any(|a| a == "--mcp-options")
}
struct Case {
    server: &'static str,
    key: &'static str,
    value: Value,
}
fn cases(root: &Path) -> Vec<Case> {
    let mut cases = Vec::new();
    for (server, key, value) in [
        (
            "disabled_fixture",
            "command",
            json!("never-run-options-fixture"),
        ),
        ("disabled_fixture", "args", json!([])),
        ("disabled_fixture", "cwd", json!(root)),
        (
            "disabled_fixture",
            "env_vars",
            json!(["CENTRAL_AGENT_OPTION_TEST_NAME"]),
        ),
        (
            "options_http_fixture",
            "url",
            json!("http://127.0.0.1:9/changed"),
        ),
        (
            "options_http_fixture",
            "bearer_token_env_var",
            json!("CENTRAL_AGENT_OPTION_TEST_TOKEN"),
        ),
        (
            "options_http_fixture",
            "env_http_headers",
            json!({"X-Fixture":"CENTRAL_AGENT_OPTION_TEST_HEADER"}),
        ),
        ("disabled_fixture", "startup_timeout_sec", json!(17.5)),
        ("disabled_fixture", "tool_timeout_sec", json!(23)),
        ("disabled_fixture", "required", json!(false)),
        ("disabled_fixture", "enabled_tools", json!([])),
        (
            "disabled_fixture",
            "disabled_tools",
            json!(["fixture_unused"]),
        ),
    ] {
        cases.push(Case { server, key, value });
    }
    cases
}
pub(super) fn sample(owner: &str) -> String {
    format!(
        r#"(()=>{{try{{const root=document;const state={};
      const config=root.querySelector('.mcp-configuration');
      state.options={{phase:window.__mcpOptionsPhase||0,ready:!!config,
        editable:[...config?.querySelectorAll('.server-setting button')||[]].some(b=>b.textContent==='Edit connection options…'&&!b.disabled),
        confirmation:root.querySelector({})?.open===true,
        privateExposed:document.body.textContent.includes({}),
        error:root.querySelector('.native-mcp [role=alert]')?.textContent||''}};
      return state;}}catch(error){{return {{errors:['MCP options sample failed: '+String(error)]}};}}}})()"#,
        delivery::sample(owner),
        json!(CONFIRM),
        json!(PRIVATE)
    )
}
fn row_script(app: &BrowserApp, owner: &str, server: &str, body: &str) -> anyhow::Result<()> {
    script(
        app,
        owner,
        &format!(
            "const row=[...root.querySelectorAll('.server-setting')].find(r=>r.querySelector('strong')?.textContent==={});if(!row)throw new Error('Missing option fixture row');{body}",
            json!(server)
        ),
    )
}
fn read(app: &BrowserApp) -> anyhow::Result<Value> {
    let response = api::config_read(app.workspace.root().unwrap().to_str().unwrap())
        .send(app.app_server.client.as_ref().unwrap())?
        .wait()?;
    let layer = response["layers"]
        .as_array()
        .context("Missing native configuration layers")?
        .iter()
        .find(|layer| layer["name"]["type"] == "user" && layer["name"]["profile"].is_null())
        .context("Missing native saved user layer")?;
    let file = layer["name"]["file"]
        .as_str()
        .context("Missing native user file")?;
    anyhow::ensure!(
        fs::canonicalize(file)?
            == context()?
                .context("Missing owned profile")?
                .join("native/config.toml")
                .canonicalize()?,
        "Unexpected native option write target"
    );
    for server in ["disabled_fixture", "options_http_fixture"] {
        anyhow::ensure!(
            response["config"]["mcp_servers"][server]["enabled"] == false,
            "Option fixture became effectively enabled"
        );
    }
    // Effective config can include default nulls that were never saved. Restore
    // only a real user override; absence must use the actual Clear control.
    Ok(layer["config"].clone())
}

// Codex projects native timeout seconds as floats, while JS JSON encodes an
// integral Number as an integer. Accept only that numeric representation change;
// every other field must remain exactly equal, including false and empty lists.
fn matches_configuration(actual: &Value, expected: &Value, case: &Case) -> bool {
    if actual == expected {
        return true;
    }
    if !matches!(case.key, "startup_timeout_sec" | "tool_timeout_sec") {
        return false;
    }
    let observed = &actual["mcp_servers"][case.server][case.key];
    let desired = &expected["mcp_servers"][case.server][case.key];
    if observed.as_f64().is_none() || observed.as_f64() != desired.as_f64() {
        return false;
    }
    let mut normalized = expected.clone();
    normalized["mcp_servers"][case.server][case.key] = observed.clone();
    *actual == normalized
}
fn open(app: &BrowserApp, owner: &str, case: &Case) -> anyhow::Result<()> {
    row_script(
        app,
        owner,
        case.server,
        &format!(
            r#"
      const button=[...row.querySelectorAll('button')].find(b=>b.textContent==='Edit connection options…');
      if(!button||button.disabled)throw new Error('Option editor unavailable');button.click();
      queueMicrotask(()=>{{const select=row.querySelector('form select');if(!select)throw new Error('Option selector missing');select.value={};select.dispatchEvent(new Event('change',{{bubbles:true}}));}});"#,
            json!(case.key)
        ),
    )
}
fn review(app: &BrowserApp, owner: &str, case: &Case, value: Option<&Value>) -> anyhow::Result<()> {
    let body = if let Some(value) = value {
        let text = value
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| value.to_string());
        format!(
            r#"const form=row.querySelector('form');const field=[...form.querySelectorAll('input,textarea,select')].find(f=>f!==form.querySelector('select'));
          if(!field||field.disabled)throw new Error('Option value field unavailable');field.value={};field.dispatchEvent(new Event(field.tagName==='SELECT'?'change':'input',{{bubbles:true}}));queueMicrotask(()=>form.requestSubmit());"#,
            json!(text)
        )
    } else {
        "const button=[...row.querySelectorAll('button')].find(b=>b.textContent==='Clear saved option…');if(!button||button.disabled)throw new Error('Option clear unavailable');button.click();".into()
    };
    row_script(app, owner, case.server, &body)
}
#[derive(Default)]
pub(super) struct Check {
    phase: usize,
    index: usize,
    writes: usize,
    initial: Option<Value>,
    cases: Vec<Case>,
    error: Option<String>,
}
impl Check {
    pub(super) fn observe(&mut self, event: &BrowserEvent) {
        if let BrowserEvent::AppServer(Event::McpReply { result, .. }) = event {
            match result {
                Ok(value) if value.get("filePath").is_some() => self.writes += 1,
                Ok(value) if value.as_object().is_some_and(|v| v.is_empty()) => {
                    self.error = Some("Option edits unexpectedly reloaded MCP".into())
                }
                Err(error) => self.error = Some(format!("Native option request failed: {error}")),
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
        let m = &state["options"];
        if m["phase"].as_u64() != Some(self.phase as u64) {
            return Ok(false);
        }
        anyhow::ensure!(
            m["privateExposed"] != true,
            "Private saved option entered the UI"
        );
        anyhow::ensure!(
            m["error"].as_str().is_none_or(str::is_empty),
            "Option UI failed: {}",
            m["error"]
        );
        if self.phase > 0 && state["draft"] != DRAFT {
            return Ok(false);
        }
        let previous = self.phase;
        if self.phase == 0 {
            if m["ready"] != true {
                return Ok(false);
            }
            self.initial = Some(read(app)?);
            self.cases = cases(&context()?.context("Missing isolated options root")?);
            input(app, owner, DRAFT, false)?;
            app.open_settings();
            script(
                app,
                owner,
                "root.querySelector('[data-settings-target=\"settings-ai\"]').click();root.querySelector('.native-mcp').open=true;root.querySelector('.mcp-configuration').open=true;",
            )?;
            click(app, owner, ".mcp-configuration", "Refresh configuration")?;
            self.phase = 1;
        } else {
            let case = &self.cases[self.index];
            let baseline = self.initial.as_ref().unwrap();
            let original = baseline["mcp_servers"][case.server].get(case.key);
            let mut expected = baseline.clone();
            expected["mcp_servers"][case.server][case.key] = case.value.clone();
            let step = (self.phase - 1) % 12;
            match step {
                0 if m["editable"] == true => {
                    open(app, owner, case)?;
                    self.phase += 1;
                }
                1 => {
                    review(app, owner, case, Some(&case.value))?;
                    self.phase += 1;
                }
                2 if m["confirmation"] == true => {
                    anyhow::ensure!(
                        self.writes == self.index * 2 && read(app)? == *baseline,
                        "Option changed before confirmation"
                    );
                    click(app, owner, CONFIRM, "Cancel")?;
                    self.phase += 1;
                }
                3 if m["confirmation"] == false => {
                    anyhow::ensure!(
                        read(app)? == *baseline,
                        "Cancelled option changed configuration"
                    );
                    review(app, owner, case, Some(&case.value))?;
                    self.phase += 1;
                }
                4 if m["confirmation"] == true => {
                    click(app, owner, CONFIRM, "Save change")?;
                    self.phase += 1;
                }
                5 if self.writes == self.index * 2 + 1 && !app.app_server.mcp.view.busy => {
                    anyhow::ensure!(
                        matches_configuration(&read(app)?, &expected, case),
                        "Native option save did not preserve exact configuration for {}",
                        case.key
                    );
                    click(app, owner, ".mcp-configuration", "Refresh configuration")?;
                    self.phase += 1;
                }
                6 if m["editable"] == true => {
                    open(app, owner, case)?;
                    self.phase += 1;
                }
                7 => {
                    review(app, owner, case, original)?;
                    self.phase += 1;
                }
                8 if m["confirmation"] == true => {
                    anyhow::ensure!(
                        matches_configuration(&read(app)?, &expected, case),
                        "Option restore changed configuration before confirmation"
                    );
                    click(app, owner, CONFIRM, "Cancel")?;
                    self.phase += 1;
                }
                9 if m["confirmation"] == false => {
                    anyhow::ensure!(
                        matches_configuration(&read(app)?, &expected, case),
                        "Cancelled option restore changed configuration"
                    );
                    review(app, owner, case, original)?;
                    self.phase += 1;
                }
                10 if m["confirmation"] == true => {
                    click(app, owner, CONFIRM, "Save change")?;
                    self.phase += 1;
                }
                11 if self.writes == self.index * 2 + 2 && !app.app_server.mcp.view.busy => {
                    anyhow::ensure!(
                        read(app)? == *baseline,
                        "Option restore did not preserve exact original configuration for {}",
                        case.key
                    );
                    println!(
                        "MCP option {}: confirmed typed save/restore, both cancellations, exact configuration preservation",
                        case.key
                    );
                    self.index += 1;
                    if self.index == self.cases.len() {
                        anyhow::ensure!(
                            self.writes == 24
                                && app.app_server.conversations.saved().bindings.is_empty()
                                && app.project_chats.iter().all(|c| c.messages.is_empty()),
                            "Options created unexpected work"
                        );
                        println!(
                            "PASS: all 12 native MCP options, 24 confirmed writes; no reload, thread, prompt or tool invocation."
                        );
                        return Ok(true);
                    }
                    click(app, owner, ".mcp-configuration", "Refresh configuration")?;
                    self.phase += 1;
                }
                _ => {}
            }
        }
        if previous != self.phase {
            println!("MCP options host phase {}", self.phase);
            script(
                app,
                owner,
                &format!("window.__mcpOptionsPhase={};", self.phase),
            )?;
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_equivalent_timeout_number_representations_are_accepted() {
        let case = Case {
            server: "fixture",
            key: "tool_timeout_sec",
            value: json!(23),
        };
        let expected = json!({"mcp_servers":{"fixture":{"tool_timeout_sec":23,"enabled":false}}});
        let float = json!({"mcp_servers":{"fixture":{"tool_timeout_sec":23.0,"enabled":false}}});
        assert!(matches_configuration(&float, &expected, &case));
        let mut different = float.clone();
        different["mcp_servers"]["fixture"]["tool_timeout_sec"] = json!(24.0);
        assert!(!matches_configuration(&different, &expected, &case));
        different["mcp_servers"]["fixture"]["tool_timeout_sec"] = json!("23");
        assert!(!matches_configuration(&different, &expected, &case));
        different = float;
        different["mcp_servers"]["fixture"]["enabled"] = json!(true);
        assert!(!matches_configuration(&different, &expected, &case));
    }
    #[test]
    fn optional_false_and_empty_lists_are_not_treated_as_absence() {
        for (key, value) in [("required", json!(false)), ("enabled_tools", json!([]))] {
            let case = Case {
                server: "fixture",
                key,
                value: value.clone(),
            };
            let mut expected = json!({"mcp_servers":{"fixture":{}}});
            expected["mcp_servers"]["fixture"][key] = value;
            assert!(!matches_configuration(
                &json!({"mcp_servers":{"fixture":{}}}),
                &expected,
                &case
            ));
        }
    }
}
