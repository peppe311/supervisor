//! Optional profile picker acceptance layered on the existing provider/history
//! test. Real UI intents only; no fabricated catalog, consent or configuration.
use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_host_oracle_rejects_wrong_frozen_selection_and_permissions() {
        let choices = vec![
            AgentSelection {
                model: "fixture-one".into(),
                effort: "low".into(),
                service_tier: Some("fast".into()),
                ..Default::default()
            },
            AgentSelection {
                model: "fixture-two".into(),
                effort: "high".into(),
                ..Default::default()
            },
        ];
        let profiles = Profiles {
            choices,
            ..Default::default()
        };
        for (index, choice) in profiles.choices.iter().enumerate() {
            let params = api::start_turn(
                "thread",
                "message",
                vec![api::text_input("fixture")],
                "C:\\fixture",
                &api::Profile {
                    model: Some(choice.model.clone()),
                    effort: Some(choice.effort.clone()),
                    service_tier: choice.service_tier.clone(),
                    personality: None,
                    summary: None,
                },
                if index == 0 {
                    api::Access::ReadOnly
                } else {
                    api::Access::WorkspaceWrite
                },
            )
            .params;
            assert!(profiles.verify_call(&params, index).is_ok());
            assert!(profiles.verify_call(&params, 1 - index).is_err());
            for (key, wrong) in [
                ("model", json!("wrong")),
                ("effort", json!("max")),
                ("serviceTier", json!("wrong")),
                ("approvalPolicy", json!("never")),
                ("sandboxPolicy", json!({"type":"dangerFullAccess"})),
            ] {
                let mut changed = params.clone();
                changed[key] = wrong;
                assert!(profiles.verify_call(&changed, index).is_err(), "{key}");
            }
        }
        assert!(profiles.verify_call(&json!({}), 2).is_err());
    }
}

#[derive(Default)]
pub(super) struct Profiles {
    choices: Vec<AgentSelection>,
    request: usize,
    phase: u8,
    siblings: Option<Value>,
}

fn select(app: &BrowserApp, owner: &str, field: &str, value: &str) -> anyhow::Result<()> {
    let body = if owner.starts_with("graph:") {
        format!(
            r#"const control=root.querySelector('[data-role="{field}"]');
            if(!control||control.disabled||![...control.options].some(o=>o.value==={value}))throw new Error('Profile option unavailable');
            control.value={value};control.dispatchEvent(new Event('change',{{bubbles:true}}));
            const save=root.querySelector('[data-action="save"]');if(!save||save.disabled)throw new Error('Assignment save unavailable');save.click();"#,
            value = json!(value)
        )
    } else {
        format!(
            r#"const trigger=document.getElementById('tetra-config-button');
            if(document.getElementById('tetra-config-menu').hidden)trigger.click();
            document.getElementById('{field}-picker-button').click();
            const option=[...document.querySelectorAll('#{field}-picker-menu button')].find(button=>button.dataset.value==={value});
            if(!option||option.disabled)throw new Error('Profile menu option unavailable');option.click();
            if(!document.getElementById('tetra-config-menu').hidden)trigger.click();"#,
            value = json!(value)
        )
    };
    script(app, owner, &body)
}
fn access(app: &BrowserApp, owner: &str, value: &str) -> anyhow::Result<()> {
    script(
        app,
        owner,
        &format!(
            r#"const select=root.querySelector('.native-access select');
        if(!select||select.disabled||![...select.options].some(o=>o.value==={}&&!o.disabled))throw new Error('Access option unavailable');
        select.value={};select.dispatchEvent(new Event('change',{{bubbles:true}}));"#,
            json!(value),
            json!(value)
        ),
    )
}
fn confirmation(app: &BrowserApp, owner: &str, text: &str) -> anyhow::Result<()> {
    script(
        app,
        owner,
        &format!(
            r#"const dialog=root.querySelector('dialog[aria-label="Allow full Codex access?"]');
        const button=[...dialog?.querySelectorAll('button')||[]].find(b=>b.textContent==={});
        if(!dialog?.open||!button||button.disabled)throw new Error('Missing full-access confirmation');button.click();"#,
            json!(text)
        ),
    )
}
fn selection<'a>(app: &'a BrowserApp, owner: &str) -> anyhow::Result<&'a AgentSelection> {
    if let Some(key) = owner.strip_prefix("graph:") {
        Ok(&app
            .agent_graph_bindings
            .iter()
            .find(|b| b.node_key() == key)
            .context("Missing selected binding")?
            .selection)
    } else {
        Ok(app.app_server.configuration().2)
    }
}
fn siblings(app: &BrowserApp, owner: &str) -> Value {
    json!(
        app.agent_graph_bindings
            .iter()
            .filter(|b| format!("graph:{}", b.node_key()) != owner)
            .map(|b| json!({"key":b.node_key(),"selection":b.selection,"provider":b.provider}))
            .collect::<Vec<_>>()
    )
}
impl Profiles {
    pub(super) fn verify_call(&self, params: &Value, index: usize) -> anyhow::Result<()> {
        let expected = self
            .choices
            .get(index)
            .context("Unexpected profile submission")?;
        anyhow::ensure!(
            params["model"] == expected.model
                && params["effort"] == expected.effort
                && params["serviceTier"] == json!(expected.service_tier)
                && params["approvalPolicy"]
                    == if index == 0 {
                        "untrusted"
                    } else {
                        "on-request"
                    }
                && params["sandboxPolicy"]["type"]
                    == if index == 0 {
                        "readOnly"
                    } else {
                        "workspaceWrite"
                    },
            "Actual native turn did not receive the profile selected through UI"
        );
        Ok(())
    }
    pub(super) fn tick(
        &mut self,
        app: &mut BrowserApp,
        owner: &str,
        sample: &Value,
        index: usize,
    ) -> anyhow::Result<bool> {
        if self.choices.is_empty() {
            for (model, effort, fast) in [
                ("gpt-5.6-luna", "low", true),
                ("gpt-5.6-sol", "high", false),
            ] {
                let entry = app
                    .app_server
                    .configuration()
                    .1
                    .iter()
                    .find(|m| m.model == model)
                    .context("Required native acceptance model not in actual catalog")?;
                let tier = if fast {
                    Some(
                        entry
                            .service_tiers
                            .iter()
                            .find(|t| matches!(t.id.as_str(), "fast" | "priority"))
                            .context("Fast not in native catalog")?
                            .id
                            .clone(),
                    )
                } else {
                    None
                };
                let choice = AgentSelection {
                    model: model.into(),
                    effort: effort.into(),
                    service_tier: tier,
                    ..Default::default()
                };
                provider_types::validate_selection(app.app_server.configuration().1, &choice)
                    .map_err(anyhow::Error::msg)?;
                self.choices.push(choice);
            }
            self.siblings = Some(siblings(app, owner));
        }
        anyhow::ensure!(
            self.siblings.as_ref() == Some(&siblings(app, owner)),
            "Profile picker changed a sibling assignment"
        );
        if self.request != index {
            self.request = index;
            self.phase = 0;
        }
        let expected = &self.choices[index];
        let actual = selection(app, owner)?;
        if self.phase > 0 && self.phase != 10 && sample["draft"] != DRAFT {
            anyhow::bail!("Profile selection lost unsent draft");
        }
        let previous = self.phase;
        match self.phase {
            0 => {
                input(app, owner, DRAFT, false)?;
                self.phase = 10;
            }
            10 if sample["draft"] == DRAFT => {
                select(app, owner, "model", &expected.model)?;
                self.phase = 1;
            }
            1 if actual.model == expected.model && sample["profile"]["model"] == expected.model => {
                select(app, owner, "effort", &expected.effort)?;
                self.phase = 2;
            }
            2 if actual.effort == expected.effort
                && sample["profile"]["effort"] == expected.effort =>
            {
                select(
                    app,
                    owner,
                    "speed",
                    expected.service_tier.as_deref().unwrap_or(""),
                )?;
                self.phase = 3;
            }
            3 if actual.service_tier == expected.service_tier
                && sample["profile"]["speed"] == expected.service_tier.as_deref().unwrap_or("") =>
            {
                access(
                    app,
                    owner,
                    if index == 0 {
                        "fullAccess"
                    } else {
                        "workspaceWrite"
                    },
                )?;
                self.phase = if index == 0 { 4 } else { 8 };
            }
            4 if sample["confirming"] == true => {
                anyhow::ensure!(
                    app.app_server.access() == api::Access::ReadOnly,
                    "Full access granted before confirmation"
                );
                confirmation(app, owner, "Cancel")?;
                self.phase = 5;
            }
            5 if sample["confirming"] == false => {
                anyhow::ensure!(
                    app.app_server.access() == api::Access::ReadOnly,
                    "Cancelled consent changed permissions"
                );
                access(app, owner, "fullAccess")?;
                self.phase = 6;
            }
            6 if sample["confirming"] == true => {
                confirmation(app, owner, "Allow full access")?;
                self.phase = 7;
            }
            7 if sample["access"] == "fullAccess"
                && app.app_server.access() == api::Access::FullAccess =>
            {
                access(app, owner, "readOnly")?;
                self.phase = 8;
            }
            8 => {
                let (value, access) = if index == 0 {
                    ("readOnly", api::Access::ReadOnly)
                } else {
                    ("workspaceWrite", api::Access::WorkspaceWrite)
                };
                if sample["access"] == value && app.app_server.access() == access {
                    anyhow::ensure!(
                        selection(app, owner)? == expected,
                        "Selected profile changed during permission choice"
                    );
                    println!(
                        "Profile host: request {index} real picker changes and consent passed; draft/sibling unchanged; full access never used for inference."
                    );
                    return Ok(true);
                }
            }
            _ => {}
        }
        if previous != self.phase {
            println!("Profile host: request {index}, phase {}", self.phase);
        }
        Ok(false)
    }
}
