//! Three real desktop processes: save, restore-and-clear, restore-cleared.
use super::*;

const TEXT: &str = "Unsent local draft\nItaliano: perché 🦀";
const SIBLING: &str = "Independent graph sibling draft";
pub(super) fn enabled() -> bool {
    std::env::args_os().any(|a| a == "--draft-persistence")
}
pub(super) fn stage() -> anyhow::Result<u8> {
    let value: u8 = std::env::var("CENTRAL_AGENT_DRAFT_TEST_STAGE")?.parse()?;
    anyhow::ensure!((1..=3).contains(&value), "Invalid owned draft test stage");
    Ok(value)
}
pub(super) fn sample(owner: &str) -> String {
    format!(
        r#"(()=>{{{}const state={};const input=root?.querySelector('textarea[data-role="message"],#chat-input');
      state.draftStatus=input?.dataset.draftStatus;
      state.siblingDrafts=[...document.querySelectorAll('.agent-console[data-node-key]')].filter(card=>card!==root).map(card=>({{text:card.querySelector('textarea[data-role="message"]')?.value,status:card.querySelector('textarea[data-role="message"]')?.dataset.draftStatus}}));
      return state;}})()"#,
        delivery::root_script(owner),
        delivery::sample(owner)
    )
}
#[derive(Default)]
pub(super) struct Check {
    opened: bool,
    edited: bool,
}
impl Check {
    pub(super) fn tick(
        &mut self,
        app: &mut BrowserApp,
        owner: &str,
        state: &Value,
        graph: Option<&graph::Graph>,
    ) -> anyhow::Result<bool> {
        let stage = stage()?;
        if let Some(graph) = graph {
            if !self.opened {
                graph.open_cards(app)?;
                self.opened = true;
                return Ok(false);
            }
            if state["ready"] != true {
                return Ok(false);
            }
        }
        if state["draftStatus"] != "saved" {
            return Ok(false);
        }
        anyhow::ensure!(
            app.app_server.conversations.saved().bindings.is_empty()
                && app.project_chats.iter().all(|c| c.messages.is_empty()),
            "Draft persistence created conversation history"
        );
        if stage == 1 && !self.edited {
            input(app, owner, TEXT, false)?;
            if let Some(graph) = graph {
                for sibling in graph.owners().into_iter().filter(|s| s != owner) {
                    input(app, &sibling, SIBLING, false)?;
                }
            }
            self.edited = true;
            return Ok(false);
        }
        let expected = if stage == 3 || stage == 2 && self.edited {
            ""
        } else {
            TEXT
        };
        if state["draft"] != expected {
            return Ok(false);
        }
        if graph.is_some()
            && !state["siblingDrafts"].as_array().is_some_and(|siblings| {
                !siblings.is_empty()
                    && siblings
                        .iter()
                        .all(|s| s["text"] == SIBLING && s["status"] == "saved")
            })
        {
            return Ok(false);
        }
        if stage == 2 && !self.edited {
            println!(
                "Draft restart: exact main/graph text and sibling restored from a fresh process"
            );
            input(app, owner, "", false)?;
            self.edited = true;
            return Ok(false);
        }
        app.save_session();
        println!(
            "Draft restart stage {stage}: durable text/clear and owner isolation verified, zero prompts"
        );
        Ok(true)
    }
}
