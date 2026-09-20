//! Event-driven supervision for a saved Supervisor -> project-chat link.
//!
//! Worker events only mark a bounded review checkpoint. The supervising model
//! receives the existing display-safe snapshot and returns a typed decision.
//! A correction is delivered only to the exact still-active native turn.
use super::*;

pub(super) const REVIEW_CONTROL_PREFIX: &str =
    "[Supervisor review protocol v1: host control, hidden from conversation display]\n";
pub(super) const AUTOMATIC_REVIEW_PREFIX: &str =
    "[Supervisor automatic checkpoint v1: host control, hidden from conversation display]\n";
const REVIEW_SCHEMA: &str = "supervisor.review.v1";
const REVIEW_DELAY_MS: u64 = 1_200;
const MAX_ASSESSMENT_CHARS: usize = 4_000;
const MAX_CORRECTION_CHARS: usize = 10_000;

#[derive(Clone, Debug)]
pub(super) struct ReviewDispatch {
    pub(super) node_key: String,
    pub(super) target_owner: String,
    pub(super) expected_turn_id: Option<String>,
    pub(super) terminal: bool,
    pub(super) generation: u64,
}

#[derive(Clone, Debug)]
pub(super) struct LoopState {
    target_owner: String,
    worker_turn_id: Option<String>,
    generation: u64,
    dirty: bool,
    terminal: bool,
    result: Option<DeliveryResult>,
}

#[derive(Clone, Debug)]
struct DeliveryResult {
    generation: u64,
    blocked: bool,
    summary: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DeliveryView {
    state: &'static str,
    summary: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ReviewDecision {
    Observe,
    Steer,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewOutput {
    schema: String,
    assessment: String,
    decision: ReviewDecision,
    instruction: String,
}

fn parse_review_output(text: &str) -> Result<ReviewOutput, String> {
    let output: ReviewOutput = serde_json::from_str(text.trim())
        .map_err(|_| "The Supervisor returned an invalid review decision.".to_owned())?;
    let assessment = output.assessment.trim();
    let instruction = output.instruction.trim();
    if output.schema != REVIEW_SCHEMA
        || assessment.is_empty()
        || assessment.chars().count() > MAX_ASSESSMENT_CHARS
        || assessment.contains('\0')
        || instruction.chars().count() > MAX_CORRECTION_CHARS
        || instruction.contains('\0')
        || output.decision == ReviewDecision::Steer && instruction.is_empty()
    {
        return Err("The Supervisor returned an invalid or oversized review decision.".into());
    }
    Ok(output)
}

pub(super) fn visible_review_output(text: &str) -> Option<String> {
    let output = parse_review_output(text).ok()?;
    Some(if output.decision == ReviewDecision::Steer {
        format!(
            "{}\n\nProposed correction for the observed agent:\n{}",
            output.assessment.trim(),
            output.instruction.trim()
        )
    } else {
        output.assessment.trim().to_owned()
    })
}

pub(super) fn is_review_control_input(text: &str) -> bool {
    text.starts_with(REVIEW_CONTROL_PREFIX) || text.starts_with(AUTOMATIC_REVIEW_PREFIX)
}

pub(super) fn review_output_schema() -> Value {
    json!({
        "type":"object",
        "properties":{
            "schema":{"type":"string","enum":[REVIEW_SCHEMA]},
            "assessment":{"type":"string"},
            "decision":{"type":"string","enum":["observe","steer"]},
            "instruction":{"type":"string"}
        },
        "required":["schema","assessment","decision","instruction"],
        "additionalProperties":false
    })
}

pub(super) fn review_instructions(review: &ReviewDispatch) -> String {
    let turn = review.expected_turn_id.as_deref().unwrap_or("unavailable");
    format!(
        "{REVIEW_CONTROL_PREFIX}You are the saved Supervisor for the linked worker. Review the supplied public observed-agent snapshot against the user's supervision goal and your prior reviews. Check progress, tool outcomes, file changes, pending requests, errors and whether the worker is still following the task. Do not expose or infer private chain of thought. Do not execute or modify the worker's task yourself. Return only the required structured decision. Choose `steer` for a specific correction. On a terminal snapshot, choose `observe` only when the work appears complete and the available evidence contains no unresolved failure, blocker or required validation; otherwise choose `steer` with the exact next instruction. On a live snapshot, choose `observe` when no correction is needed now. The host will independently verify the saved link and exact active turn before delivering a live correction; a terminal correction is surfaced as work that needs revision. Observed turn: {turn}. Terminal snapshot: {}.",
        review.terminal
    )
}

fn automatic_review_prompt() -> String {
    format!(
        "{AUTOMATIC_REVIEW_PREFIX}A meaningful worker checkpoint arrived. Re-evaluate the linked agent now using the attached observed-agent snapshot and the supervision objective already present in this conversation."
    )
}

fn notification_turn_id(params: &Value) -> Option<String> {
    params
        .get("turnId")
        .and_then(Value::as_str)
        .or_else(|| params["turn"].get("id").and_then(Value::as_str))
        .map(str::to_owned)
}

fn meaningful_notification(method: &str, params: &Value) -> Option<(Option<String>, bool)> {
    let meaningful = match method {
        "turn/started" | "turn/plan/updated" | "serverRequest/resolved" => true,
        "turn/completed" => true,
        "item/completed" => !matches!(
            params["item"]["type"].as_str(),
            Some("userMessage" | "agentMessage" | "reasoning" | "plan")
        ),
        _ => false,
    };
    meaningful.then(|| (notification_turn_id(params), method == "turn/completed"))
}

fn correction_is_current(terminal: bool, expected: Option<&str>, active: Option<&str>) -> bool {
    !terminal && expected.is_some() && expected == active
}

fn terminal_delivery(generation: u64, decision: &Result<ReviewOutput, String>) -> DeliveryResult {
    match decision {
        Ok(output) => DeliveryResult {
            generation,
            blocked: output.decision == ReviewDecision::Steer,
            summary: output.assessment.trim().to_owned(),
        },
        Err(error) => DeliveryResult {
            generation,
            blocked: true,
            summary: error.clone(),
        },
    }
}

impl BrowserApp {
    fn supervision_target(&self, node_key: &str) -> Option<(String, String)> {
        let binding = self
            .agent_graph_bindings
            .iter()
            .find(|binding| binding.node_key() == node_key)?;
        if binding.provider != AgentProviderKind::CodexAppServer {
            return None;
        }
        let chat_id = binding.project_chat_id.as_deref()?;
        if !project_board::chat_in_project(
            &self.project_chats,
            chat_id,
            binding.project_directory.as_deref(),
            binding.ssh_profile_id.is_some(),
        ) {
            return None;
        }
        let target_owner = format!("chat:{chat_id}");
        Some((format!("graph:{node_key}"), target_owner))
    }

    fn observed_turn_id(&self, owner: &str) -> Option<String> {
        self.app_server
            .conversations
            .active_turn(owner)
            .map(str::to_owned)
            .or_else(|| {
                let binding = self.app_server.conversations.binding(owner)?;
                self.app_server
                    .conversations
                    .mirror
                    .thread(&binding.thread_id)?
                    .turns
                    .last()
                    .map(|turn| turn.id.clone())
            })
    }

    pub(super) fn begin_user_supervision(&mut self, node_key: &str) -> Option<ReviewDispatch> {
        let (_, target_owner) = self.supervision_target(node_key)?;
        let active_turn = self
            .app_server
            .conversations
            .active_turn(&target_owner)
            .map(str::to_owned);
        let worker_turn_id = active_turn
            .clone()
            .or_else(|| self.observed_turn_id(&target_owner));
        let state = self
            .supervision_loops
            .entry(node_key.to_owned())
            .or_insert_with(|| LoopState {
                target_owner: target_owner.clone(),
                worker_turn_id: worker_turn_id.clone(),
                generation: 0,
                dirty: false,
                terminal: active_turn.is_none(),
                result: None,
            });
        if state.target_owner != target_owner {
            *state = LoopState {
                target_owner: target_owner.clone(),
                worker_turn_id: worker_turn_id.clone(),
                generation: state.generation.saturating_add(1),
                dirty: false,
                terminal: active_turn.is_none(),
                result: None,
            };
        } else {
            state.worker_turn_id = worker_turn_id.clone();
            state.terminal = active_turn.is_none();
            state.dirty = false;
            state.generation = state.generation.saturating_add(1);
        }
        Some(ReviewDispatch {
            node_key: node_key.to_owned(),
            target_owner,
            expected_turn_id: worker_turn_id,
            terminal: active_turn.is_none(),
            generation: state.generation,
        })
    }

    pub(super) fn automatic_supervision_active(&self, node_key: &str) -> bool {
        self.supervision_loops.get(node_key).is_some_and(|state| {
            self.supervision_target(node_key)
                .is_some_and(|(_, target)| target == state.target_owner)
        })
    }

    pub(super) fn supervision_delivery(&self, node_key: &str) -> Option<DeliveryView> {
        let state = self.supervision_loops.get(node_key)?;
        let (_, target_owner) = self.supervision_target(node_key)?;
        if state.target_owner != target_owner {
            return None;
        }
        if state.terminal
            && let Some(result) = state
                .result
                .as_ref()
                .filter(|result| result.generation == state.generation)
        {
            return Some(DeliveryView {
                state: if result.blocked { "blocked" } else { "ready" },
                summary: result.summary.clone(),
            });
        }
        if !self.app_server.is_connected() {
            return Some(DeliveryView {
                state: "blocked",
                summary: "The final check is waiting for the Codex connection.".into(),
            });
        }
        if self
            .app_server_request_views(&target_owner)
            .as_array()
            .is_some_and(|requests| !requests.is_empty())
        {
            return Some(DeliveryView {
                state: "blocked",
                summary: "The linked agent needs a decision before it can continue.".into(),
            });
        }
        if self.owner_has_running_work(&target_owner) {
            return Some(DeliveryView {
                state: "working",
                summary: "The linked agent is working. Supervisor checks meaningful checkpoints automatically.".into(),
            });
        }
        if state.terminal {
            return Some(DeliveryView {
                state: "checking",
                summary: "Supervisor is reviewing the final worker checkpoint.".into(),
            });
        }
        Some(DeliveryView {
            state: "working",
            summary: "Continuous supervision is active for the linked conversation.".into(),
        })
    }

    pub(super) fn supervision_delivery_for_chat(&self, chat_id: &str) -> Option<DeliveryView> {
        let target = format!("chat:{chat_id}");
        self.supervision_loops
            .iter()
            .filter(|(_, state)| state.target_owner == target)
            .filter_map(|(node_key, _)| self.supervision_delivery(node_key))
            .max_by_key(|delivery| match delivery.state {
                "blocked" => 4,
                "checking" => 3,
                "working" => 2,
                _ => 1,
            })
    }

    pub(super) fn reset_supervision(&mut self, node_key: &str) {
        self.supervision_loops.remove(node_key);
        self.active_supervisor_reviews
            .retain(|_, review| review.node_key != node_key);
    }

    pub(super) fn note_supervision_notification(
        &mut self,
        target_owner: &str,
        method: &str,
        params: &Value,
    ) {
        let Some((turn_id, terminal)) = meaningful_notification(method, params) else {
            return;
        };
        self.mark_supervision_checkpoint(target_owner, turn_id, terminal);
    }

    pub(super) fn note_supervision_request(&mut self, target_owner: &str) {
        let turn_id = self
            .app_server
            .conversations
            .active_turn(target_owner)
            .map(str::to_owned);
        self.mark_supervision_checkpoint(target_owner, turn_id, false);
    }

    fn mark_supervision_checkpoint(
        &mut self,
        target_owner: &str,
        turn_id: Option<String>,
        terminal: bool,
    ) {
        let nodes = self
            .supervision_loops
            .iter()
            .filter(|(_, state)| state.target_owner == target_owner)
            .map(|(node, _)| node.clone())
            .collect::<Vec<_>>();
        for node_key in nodes {
            let supervisor_owner = format!("graph:{node_key}");
            if let Some(state) = self.supervision_loops.get_mut(&node_key) {
                state.generation = state.generation.saturating_add(1);
                state.dirty = true;
                state.terminal = terminal;
                if turn_id.is_some() {
                    state.worker_turn_id = turn_id.clone();
                }
            }
            if !self.conversation_busy(&supervisor_owner) {
                self.schedule_supervision_review(&node_key);
            }
        }
    }

    fn schedule_supervision_review(&mut self, node_key: &str) {
        let Some(state) = self.supervision_loops.get(node_key) else {
            return;
        };
        let generation = state.generation;
        let node_key = node_key.to_owned();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(REVIEW_DELAY_MS));
            let _ = proxy.send_event(BrowserEvent::SupervisorReviewTick {
                node_key,
                generation,
            });
        });
    }

    pub(super) fn handle_supervision_review_tick(&mut self, node_key: String, generation: u64) {
        let Some(state) = self.supervision_loops.get_mut(&node_key) else {
            return;
        };
        if state.generation != generation {
            return;
        }
        if !state.dirty {
            return;
        }
        let generation = state.generation;
        let Some((supervisor_owner, target_owner)) = self.supervision_target(&node_key) else {
            self.reset_supervision(&node_key);
            return;
        };
        if self.conversation_busy(&supervisor_owner) || !self.app_server.is_connected() {
            return;
        }
        let (expected_turn_id, terminal) = self
            .supervision_loops
            .get(&node_key)
            .map(|state| (state.worker_turn_id.clone(), state.terminal))
            .unwrap_or((None, false));
        let binding = match self
            .agent_graph_bindings
            .iter()
            .find(|binding| binding.node_key() == node_key)
            .cloned()
        {
            Some(binding) => binding,
            None => {
                self.reset_supervision(&node_key);
                return;
            }
        };
        let mut submission = match self.prepare_agent_graph_submission(
            &binding.record_type,
            &binding.record_id,
            binding.conversation_id,
            Some(automatic_review_prompt()),
        ) {
            Ok(submission) => submission,
            Err(error) => {
                self.conversation_event(
                    "central-agent:conversation-error",
                    json!({"owner":supervisor_owner,"error":error}),
                );
                return;
            }
        };
        let review = ReviewDispatch {
            node_key: node_key.clone(),
            target_owner,
            expected_turn_id,
            terminal,
            generation,
        };
        submission.supervision_review = Some(review);
        if let Some(scope) = submission.scope.as_mut() {
            scope.native_access = central_agent_codex_runtime::api::Access::ReadOnly;
        }
        if let Some(state) = self.supervision_loops.get_mut(&node_key) {
            state.dirty = false;
        }
        self.submit_app_server(submission, false);
        if !self.owner_has_running_work(&supervisor_owner)
            && let Some(state) = self.supervision_loops.get_mut(&node_key)
        {
            state.dirty = true;
        }
    }

    pub(super) fn accept_supervisor_review(
        &mut self,
        supervisor_owner: &str,
        turn_id: &str,
        review: ReviewDispatch,
    ) {
        self.active_supervisor_reviews
            .insert(format!("{supervisor_owner}\0{turn_id}"), review);
        let already_terminal = self
            .app_server
            .conversations
            .binding(supervisor_owner)
            .and_then(|binding| {
                self.app_server
                    .conversations
                    .mirror
                    .thread(&binding.thread_id)
            })
            .and_then(|thread| thread.turns.iter().find(|turn| turn.id == turn_id))
            .is_some_and(|turn| !turn.active());
        if already_terminal {
            self.complete_supervisor_review(supervisor_owner, turn_id);
        }
    }

    pub(super) fn fail_supervisor_review(&mut self, review: &ReviewDispatch) {
        if let Some(state) = self.supervision_loops.get_mut(&review.node_key) {
            state.dirty = true;
        }
    }

    fn raw_review_output(&self, supervisor_owner: &str, turn_id: &str) -> Option<String> {
        let binding = self.app_server.conversations.binding(supervisor_owner)?;
        self.app_server
            .conversations
            .mirror
            .thread(&binding.thread_id)?
            .turns
            .iter()
            .find(|turn| turn.id == turn_id)?
            .items
            .iter()
            .rev()
            .filter(|item| item.value["type"] == "agentMessage")
            .filter_map(|item| item.value["text"].as_str())
            .find(|text| parse_review_output(text).is_ok())
            .map(str::to_owned)
    }

    pub(super) fn complete_supervisor_review(&mut self, supervisor_owner: &str, turn_id: &str) {
        let Some(review) = self
            .active_supervisor_reviews
            .remove(&format!("{supervisor_owner}\0{turn_id}"))
        else {
            return;
        };
        let decision = self
            .raw_review_output(supervisor_owner, turn_id)
            .ok_or_else(|| {
                "The Supervisor review completed without a structured decision.".to_owned()
            })
            .and_then(|text| parse_review_output(&text));
        let terminal_result = review
            .terminal
            .then(|| terminal_delivery(review.generation, &decision));
        match decision {
            Ok(output) if output.decision == ReviewDecision::Steer => {
                let expected = review.expected_turn_id.as_deref();
                let linked = self
                    .supervision_target(&review.node_key)
                    .is_some_and(|(_, target)| target == review.target_owner);
                if linked
                    && correction_is_current(
                        review.terminal,
                        expected,
                        self.app_server.steer_target(&review.target_owner),
                    )
                    && let Some(expected) = expected
                {
                    self.app_server_steer_for(
                        review.target_owner.clone(),
                        app_server::SteerInput::supervised(
                            expected.to_owned(),
                            format!("Supervisor correction:\n{}", output.instruction.trim()),
                            None,
                        ),
                        supervisor_owner.to_owned(),
                    );
                }
            }
            Ok(_) => {}
            Err(error) => {
                self.conversation_event(
                    "central-agent:conversation-error",
                    json!({"owner":supervisor_owner,"error":error}),
                );
            }
        }
        let mut schedule = false;
        if let Some(state) = self.supervision_loops.get_mut(&review.node_key) {
            if review.terminal && state.generation == review.generation && !state.dirty {
                state.worker_turn_id = None;
                state.result = terminal_result;
            }
            schedule = state.dirty;
        }
        if schedule && !self.conversation_busy(supervisor_owner) {
            self.schedule_supervision_review(&review.node_key);
        }
    }

    pub(super) fn supervision_disconnected(&mut self) {
        for review in self
            .active_supervisor_reviews
            .drain()
            .map(|(_, review)| review)
        {
            if let Some(state) = self.supervision_loops.get_mut(&review.node_key) {
                state.dirty = true;
            }
        }
    }

    pub(super) fn resume_supervision_loops(&mut self) {
        if !self.app_server.is_connected() {
            return;
        }
        let nodes = self
            .supervision_loops
            .iter()
            .filter(|(_, state)| state.dirty)
            .map(|(node, _)| node.clone())
            .collect::<Vec<_>>();
        for node in nodes {
            if !self.conversation_busy(&format!("graph:{node}")) {
                self.schedule_supervision_review(&node);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_corrections_cannot_be_delivered() {
        assert!(correction_is_current(false, Some("t"), Some("t")));
        for (terminal, expected, active) in [
            (true, Some("t"), Some("t")),
            (false, Some("t"), Some("next")),
            (false, None, None),
        ] {
            assert!(!correction_is_current(terminal, expected, active));
        }
    }

    #[test]
    fn review_output_is_strict_bounded_and_has_a_readable_projection() {
        let steer = json!({
            "schema":REVIEW_SCHEMA,
            "assessment":"The worker is editing the wrong file.",
            "decision":"steer",
            "instruction":"Return to src/main.rs and verify the failing test.",
        })
        .to_string();
        let visible = visible_review_output(&steer).unwrap();
        assert!(visible.contains("wrong file"));
        assert!(visible.contains("Proposed correction"));
        assert!(parse_review_output(&steer).is_ok());

        let ready = parse_review_output(
            &json!({"schema":REVIEW_SCHEMA,"assessment":"Tests passed.","decision":"observe","instruction":""}).to_string(),
        );
        let ready = terminal_delivery(7, &ready);
        assert!(!ready.blocked);
        assert_eq!(ready.generation, 7);
        let blocked = terminal_delivery(8, &parse_review_output(&steer));
        assert!(blocked.blocked);
        assert!(blocked.summary.contains("wrong file"));

        let terminal_prompt = review_instructions(&ReviewDispatch {
            node_key: "node".into(),
            target_owner: "chat:worker".into(),
            expected_turn_id: Some("turn".into()),
            terminal: true,
            generation: 1,
        });
        assert!(terminal_prompt.contains("needs revision"));

        for invalid in [
            json!({"schema":REVIEW_SCHEMA,"assessment":"ok","decision":"steer","instruction":""}),
            json!({"schema":"other","assessment":"ok","decision":"observe","instruction":""}),
            json!({"schema":REVIEW_SCHEMA,"assessment":"ok","decision":"observe","instruction":"","extra":true}),
        ] {
            assert!(parse_review_output(&invalid.to_string()).is_err());
        }
    }

    #[test]
    fn only_meaningful_worker_checkpoints_trigger_reviews() {
        assert!(meaningful_notification("turn/started", &json!({"turn":{"id":"t"}})).is_some());
        assert!(
            meaningful_notification("turn/completed", &json!({"turn":{"id":"t"}}))
                .unwrap()
                .1
        );
        assert!(
            meaningful_notification(
                "item/completed",
                &json!({"turnId":"t","item":{"type":"fileChange"}})
            )
            .is_some()
        );
        assert!(
            meaningful_notification(
                "item/completed",
                &json!({"turnId":"t","item":{"type":"agentMessage"}})
            )
            .is_none()
        );
        assert!(
            meaningful_notification("item/agentMessage/delta", &json!({"turnId":"t"})).is_none()
        );
    }
}
