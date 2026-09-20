use serde::{Deserialize, Serialize};

pub(crate) const MAX_CHAT_ATTACHMENTS: usize = 12;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentPhase {
    Idle,
    Planning,
    Observing,
    WaitingApproval,
    Acting,
    Verifying,
    Finalizing,
    Completed,
    Stopped,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentStepKind {
    Context,
    Observe,
    Act,
    Verify,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentStepStatus {
    Queued,
    AwaitingApproval,
    Running,
    Success,
    Error,
    Denied,
}

pub(crate) struct PlannedStep {
    pub label: String,
    pub kind: AgentStepKind,
    pub status: AgentStepStatus,
    pub detail: Option<String>,
}

impl PlannedStep {}

pub(crate) struct AgentRun {
    pub id: u64,
    pub prompt: String,
    pub steps: Vec<PlannedStep>,
    pub current_index: usize,
    pub active_request_id: Option<String>,
    pub active_provider_call_id: Option<String>,
    pub phase: AgentPhase,
    pub status: String,
    pub profile_label: Option<String>,
    pub context_usage: Option<AgentContextUsage>,
}

impl AgentRun {
    pub(crate) fn planning_for_provider(id: u64, prompt: String, provider_label: &str) -> Self {
        Self {
            id,
            prompt,
            steps: Vec::new(),
            current_index: 0,
            active_request_id: None,
            active_provider_call_id: None,
            phase: AgentPhase::Planning,
            status: format!("{provider_label} is evaluating the first step…"),
            profile_label: None,
            context_usage: None,
        }
    }

    pub(crate) fn set_profile(&mut self, label: String) {
        self.profile_label = Some(label);
    }

    pub(crate) fn append_streamed_tool_step(
        &mut self,
        label: String,
        kind: AgentStepKind,
    ) -> usize {
        self.steps.push(PlannedStep {
            label,
            kind,
            status: AgentStepStatus::Running,
            detail: None,
        });
        self.current_index = self.steps.len().saturating_sub(1);
        self.phase = match kind {
            AgentStepKind::Act => AgentPhase::Acting,
            AgentStepKind::Verify => AgentPhase::Verifying,
            AgentStepKind::Context | AgentStepKind::Observe => AgentPhase::Observing,
        };
        self.steps.len() - 1
    }

    pub(crate) fn is_active(&self) -> bool {
        matches!(
            self.phase,
            AgentPhase::Planning
                | AgentPhase::Observing
                | AgentPhase::WaitingApproval
                | AgentPhase::Acting
                | AgentPhase::Verifying
                | AgentPhase::Finalizing
        )
    }

    pub(crate) fn view(&self) -> AgentRuntimeView {
        AgentRuntimeView {
            phase: self.phase,
            status: self.status.clone(),
            active: self.is_active(),
            run_id: Some(self.id),
            prompt: Some(self.prompt.clone()),
            profile_label: self.profile_label.clone(),
            context_usage: self.context_usage.clone(),
            steps: self
                .steps
                .iter()
                .map(|step| AgentPlanStepView {
                    label: step.label.clone(),
                    kind: step.kind,
                    status: step.status,
                    detail: step.detail.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentRuntimeView {
    pub phase: AgentPhase,
    pub status: String,
    pub active: bool,
    pub run_id: Option<u64>,
    pub prompt: Option<String>,
    pub profile_label: Option<String>,
    pub context_usage: Option<AgentContextUsage>,
    pub steps: Vec<AgentPlanStepView>,
}

impl AgentRuntimeView {
    pub(crate) fn idle() -> Self {
        Self {
            phase: AgentPhase::Idle,
            status: "Waiting for a request".to_owned(),
            active: false,
            run_id: None,
            prompt: None,
            profile_label: None,
            context_usage: None,
            steps: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentContextUsage {
    /// Tokens occupying the latest active model context, not cumulative account usage.
    pub used_tokens: u64,
    pub context_window: Option<u64>,
    pub selected_context_window: Option<u64>,
    pub cumulative_tokens: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentPlanStepView {
    pub label: String,
    pub kind: AgentStepKind,
    pub status: AgentStepStatus,
    pub detail: Option<String>,
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn finalizing_run_remains_stoppable_while_background_commands_exit() {
        let mut run = AgentRun::planning_for_provider(9, "Build project".to_owned(), "Agent");
        run.phase = AgentPhase::Finalizing;
        run.status = "Waiting for background commands to finish; Stop cancels them.".to_owned();

        let view = run.view();

        assert!(view.active);
        assert_eq!(view.phase, AgentPhase::Finalizing);
    }

    #[test]
    fn runtime_view_exposes_reported_context_usage() {
        let mut run = AgentRun::planning_for_provider(10, "Inspect project".to_owned(), "Agent");
        run.context_usage = Some(AgentContextUsage {
            used_tokens: 47_500,
            context_window: Some(128_000),
            selected_context_window: Some(128_000),
            cumulative_tokens: 81_200,
            input_tokens: 44_000,
            cached_input_tokens: 30_000,
            cache_write_input_tokens: 800,
            output_tokens: 3_500,
            reasoning_output_tokens: 1_400,
        });

        let view = run.view();

        assert_eq!(view.context_usage.unwrap().used_tokens, 47_500);
    }
}
