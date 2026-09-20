//! State for actual model-triggered MCP requests in hidden desktop acceptance.
//! Native App Server owns tools/turns; this only drives observed consent cards.
use super::*;

const PERMISSION: &str =
    "Allow the elicitation_fixture MCP server to run tool \"fixture_question\"?";

#[cfg(test)]
#[test]
fn native_mcp_host_ack_requires_exact_turn_identity() {
    let mut turn = Turn::default();
    assert!(!turn.matches(&json!({"turnId":null})));
    assert!(
        turn.acknowledged(json!({"turn":{"id":"owned-turn"}}))
            .is_ok()
    );
    assert!(turn.matches(&json!({"turnId":"owned-turn"})));
    assert!(
        turn.acknowledged(json!({"turn":{"id":"owned-turn"}}))
            .is_ok()
    );
    assert!(
        turn.acknowledged(json!({"turn":{"id":"different-turn"}}))
            .is_err()
    );
    assert!(turn.acknowledged(json!({})).is_err());
    assert!(turn.matches(&json!({"turnId":"owned-turn"})));
}

#[derive(Default)]
pub(super) struct Turn {
    id: Option<String>,
    started: bool,
    permission_key: Option<ServerRequestKey>,
    submitted: bool,
    answered: bool,
    resolved: bool,
    item: Option<Value>,
    pub(super) done: bool,
}
impl Turn {
    pub(super) fn matches(&self, params: &Value) -> bool {
        self.id.as_deref().is_some_and(|id| params["turnId"] == id)
    }
    pub(super) fn acknowledged(&mut self, ack: Value) -> anyhow::Result<()> {
        let id = ack["turn"]["id"]
            .as_str()
            .context("Missing native turn ACK")?;
        anyhow::ensure!(
            self.id.as_deref().is_none_or(|previous| previous == id),
            "Turn ACK identity changed"
        );
        self.id = Some(id.into());
        Ok(())
    }
    pub(super) fn observe(
        &mut self,
        app: &BrowserApp,
        owner: &str,
        event: &BrowserEvent,
        reply: &mut Option<Value>,
    ) -> anyhow::Result<bool> {
        let same = |params: &Value| {
            app.app_server
                .conversations
                .binding(owner)
                .is_some_and(|b| params["threadId"] == b.thread_id)
        };
        match event {
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::Notification { method, params },
                ..
            }) => {
                if method == "turn/started" {
                    anyhow::ensure!(
                        same(params)
                            && !self.started
                            && self
                                .id
                                .as_deref()
                                .is_none_or(|id| params["turn"]["id"] == id),
                        "Unexpected native turn"
                    );
                    self.started = true;
                    self.id = Some(
                        params["turn"]["id"]
                            .as_str()
                            .context("Missing native turn ID")?
                            .into(),
                    );
                    return Ok(true);
                }
                if method == "turn/completed" {
                    anyhow::ensure!(
                        same(params)
                            && self.id.as_deref() == params["turn"]["id"].as_str()
                            && params["turn"]["status"] == "completed",
                        "Fixture native turn failed: {}",
                        params["turn"]["error"]
                    );
                    self.done = true;
                    return Ok(true);
                }
                if method == "item/completed" && params["item"]["type"] == "mcpToolCall" {
                    let item = &params["item"];
                    anyhow::ensure!(
                        same(params)
                            && self.matches(params)
                            && item["server"] == "elicitation_fixture"
                            && item["tool"] == "fixture_question"
                            && item["status"] == "completed"
                            && self.item.is_none(),
                        "Unexpected MCP result"
                    );
                    *reply = Some(item["result"].clone());
                    self.item = Some(item.clone());
                    return Ok(true);
                }
                if method == "serverRequest/resolved"
                    && self
                        .permission_key
                        .as_ref()
                        .is_some_and(|k| json!(k.id) == params["requestId"])
                {
                    anyhow::ensure!(same(params) && !self.resolved, "Wrong pre-call resolution");
                    self.resolved = true;
                    return Ok(true);
                }
                if method == "item/started" {
                    anyhow::ensure!(
                        same(params)
                            && matches!(
                                params["item"]["type"].as_str(),
                                Some("userMessage" | "agentMessage" | "reasoning" | "mcpToolCall")
                            ),
                        "Unexpected tool activity"
                    );
                }
            }
            BrowserEvent::AppServer(Event::Transport {
                event:
                    TransportEvent::ServerRequest {
                        key,
                        method,
                        params,
                    },
                ..
            }) if params["message"] == PERMISSION => {
                anyhow::ensure!(
                    same(params)
                        && self.matches(params)
                        && method == "mcpServer/elicitation/request"
                        && params["serverName"] == "elicitation_fixture"
                        && params["mode"] == "form"
                        && params["requestedSchema"] == json!({"type":"object","properties":{}})
                        && self.permission_key.is_none(),
                    "Unexpected MCP pre-call permission"
                );
                self.permission_key = Some(key.clone());
                return Ok(true);
            }
            BrowserEvent::AppServer(Event::Answered {
                owner: actual,
                key,
                result,
                ..
            }) if self.permission_key.as_ref() == Some(key) => {
                anyhow::ensure!(
                    actual == owner && result.is_ok(),
                    "MCP permission answer failed"
                );
                self.answered = true;
                return Ok(true);
            }
            _ => {}
        }
        Ok(false)
    }
    pub(super) fn permission(
        &mut self,
        app: &BrowserApp,
        owner: &str,
        cards: &[Value],
    ) -> anyhow::Result<bool> {
        if self.permission_key.is_none() {
            return Ok(false);
        }
        if !self.answered || !self.resolved {
            if !self.submitted
                && cards.len() == 1
                && cards[0]["text"]
                    .as_str()
                    .is_some_and(|s| s.contains(PERMISSION))
            {
                approvals::script(
                    app,
                    owner,
                    "const form=root.querySelector('.native-requests form');if(!form)throw new Error('Missing permission form');form.requestSubmit();",
                )?;
                self.submitted = true;
            }
            return Ok(true);
        }
        // A sampled card may precede native resolution. Never submit the next
        // decision against the previous card, even if both are form requests.
        Ok(cards
            .iter()
            .any(|c| c["text"].as_str().is_some_and(|s| s.contains(PERMISSION))))
    }
    pub(super) fn verify_history(
        &self,
        app: &BrowserApp,
        thread: &str,
        count: usize,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.done && self.submitted && self.answered && self.resolved,
            "MCP native turn not complete"
        );
        let history = api::read_thread(thread)
            .send(app.app_server.client.as_ref().context("Missing client")?)?
            .wait()?;
        let turns = history["thread"]["turns"]
            .as_array()
            .context("Missing stored turns")?;
        let turn = turns.last().context("No stored turn")?;
        let item = self.item.as_ref().context("Missing native tool item")?;
        anyhow::ensure!(
            turns.len() == count
                && turn["id"].as_str() == self.id.as_deref()
                && turn["items"]
                    .as_array()
                    .is_some_and(|items| items.contains(item)
                        && items
                            .iter()
                            .any(|i| i["type"] == "agentMessage" && i["text"] == "READY")),
            "Native history lost MCP result or continuation"
        );
        println!("Desktop in-turn MCP: native history {count} and continuation verified.");
        Ok(())
    }
}
