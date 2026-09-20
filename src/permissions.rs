use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PermissionMode {
    InitialAuthorization,
    #[default]
    EveryAction,
    FullAccess,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CapabilityScope {
    Browser,
    Terminal,
    Ssh,
    Workspace,
    Process,
    Window,
    UiAutomation,
    Capture,
    RemoteDesktop,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ActionEffect {
    ReadOnly,
    ReversibleWrite,
    ProcessExecution,
    ExternalInteraction,
    Destructive,
    Privileged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ActionAuthorization {
    pub scope: CapabilityScope,
    pub effect: ActionEffect,
}

impl ActionAuthorization {
    pub(crate) const fn new(scope: CapabilityScope, effect: ActionEffect) -> Self {
        Self { scope, effect }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PermissionDecision {
    Execute,
    NeedsInitialAuthorization,
    NeedsActionApproval,
    Denied,
}

#[derive(Debug, Default)]
pub(crate) struct PermissionPolicy {
    mode: PermissionMode,
    authorized_scopes: BTreeSet<CapabilityScope>,
}

impl PermissionPolicy {
    pub(crate) fn mode(&self) -> PermissionMode {
        self.mode
    }

    pub(crate) fn session_authorized(&self) -> bool {
        !self.authorized_scopes.is_empty()
    }

    pub(crate) fn authorized_scopes(&self) -> Vec<CapabilityScope> {
        self.authorized_scopes.iter().copied().collect()
    }

    pub(crate) fn decision(&self, action: ActionAuthorization) -> PermissionDecision {
        match action.effect {
            ActionEffect::Privileged => return PermissionDecision::Denied,
            ActionEffect::Destructive => return PermissionDecision::NeedsActionApproval,
            _ => {}
        }

        match self.mode {
            PermissionMode::InitialAuthorization
                if self.authorized_scopes.contains(&action.scope) =>
            {
                PermissionDecision::Execute
            }
            PermissionMode::InitialAuthorization => PermissionDecision::NeedsInitialAuthorization,
            PermissionMode::EveryAction => PermissionDecision::NeedsActionApproval,
            PermissionMode::FullAccess => PermissionDecision::Execute,
        }
    }

    pub(crate) fn set_mode(&mut self, mode: PermissionMode) {
        self.mode = mode;
        self.authorized_scopes.clear();
    }

    pub(crate) fn authorize_scopes(
        &mut self,
        scopes: impl IntoIterator<Item = CapabilityScope>,
    ) -> bool {
        // Initial authorization is a single trusted-user gesture. Once any
        // scope has been granted, later pending actions must use their own
        // approve-once decision instead of widening the session in bulk.
        if self.mode != PermissionMode::InitialAuthorization || self.session_authorized() {
            return false;
        }
        self.authorized_scopes.extend(scopes);
        true
    }

    pub(crate) fn revoke_scope(&mut self, scope: CapabilityScope) {
        self.authorized_scopes.remove(&scope);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BROWSER_READ: ActionAuthorization =
        ActionAuthorization::new(CapabilityScope::Browser, ActionEffect::ReadOnly);
    const TERMINAL_EXECUTE: ActionAuthorization =
        ActionAuthorization::new(CapabilityScope::Terminal, ActionEffect::ProcessExecution);

    #[test]
    fn defaults_to_approval_for_every_action() {
        let policy = PermissionPolicy::default();
        assert_eq!(
            policy.decision(BROWSER_READ),
            PermissionDecision::NeedsActionApproval
        );
    }

    #[test]
    fn initial_authorization_is_scoped_and_session_only() {
        let mut policy = PermissionPolicy::default();
        policy.set_mode(PermissionMode::InitialAuthorization);
        assert_eq!(
            policy.decision(BROWSER_READ),
            PermissionDecision::NeedsInitialAuthorization
        );

        assert!(policy.authorize_scopes([CapabilityScope::Browser]));
        assert_eq!(policy.decision(BROWSER_READ), PermissionDecision::Execute);
        assert_eq!(
            policy.decision(TERMINAL_EXECUTE),
            PermissionDecision::NeedsInitialAuthorization
        );
        assert!(!policy.authorize_scopes([CapabilityScope::Terminal]));
        assert_eq!(
            policy.decision(TERMINAL_EXECUTE),
            PermissionDecision::NeedsInitialAuthorization
        );

        let restarted = PermissionPolicy::default();
        assert_eq!(
            restarted.decision(BROWSER_READ),
            PermissionDecision::NeedsActionApproval
        );
    }

    #[test]
    fn ssh_authorization_is_independent_from_the_local_terminal() {
        let mut policy = PermissionPolicy::default();
        policy.set_mode(PermissionMode::InitialAuthorization);
        assert!(policy.authorize_scopes([CapabilityScope::Terminal]));
        let ssh = ActionAuthorization::new(CapabilityScope::Ssh, ActionEffect::ProcessExecution);

        assert_eq!(
            policy.decision(ssh),
            PermissionDecision::NeedsInitialAuthorization
        );
    }

    #[test]
    fn changing_mode_and_scope_changes_revoke_grants() {
        let mut policy = PermissionPolicy::default();
        policy.set_mode(PermissionMode::InitialAuthorization);
        assert!(policy.authorize_scopes([CapabilityScope::Browser, CapabilityScope::Workspace,]));

        policy.revoke_scope(CapabilityScope::Workspace);
        assert_eq!(policy.authorized_scopes(), vec![CapabilityScope::Browser]);

        policy.set_mode(PermissionMode::EveryAction);
        policy.set_mode(PermissionMode::InitialAuthorization);
        assert!(!policy.session_authorized());
        assert_eq!(
            policy.decision(BROWSER_READ),
            PermissionDecision::NeedsInitialAuthorization
        );
    }

    #[test]
    fn full_access_executes_normal_scoped_actions() {
        let mut policy = PermissionPolicy::default();
        policy.set_mode(PermissionMode::FullAccess);
        assert_eq!(
            policy.decision(TERMINAL_EXECUTE),
            PermissionDecision::Execute
        );
    }

    #[test]
    fn destructive_actions_always_need_a_fresh_approval() {
        let destructive =
            ActionAuthorization::new(CapabilityScope::Workspace, ActionEffect::Destructive);
        let mut policy = PermissionPolicy::default();
        policy.set_mode(PermissionMode::FullAccess);
        assert_eq!(
            policy.decision(destructive),
            PermissionDecision::NeedsActionApproval
        );
    }

    #[test]
    fn privileged_actions_are_hard_denied() {
        let privileged =
            ActionAuthorization::new(CapabilityScope::Process, ActionEffect::Privileged);
        let mut policy = PermissionPolicy::default();
        policy.set_mode(PermissionMode::FullAccess);
        assert_eq!(policy.decision(privileged), PermissionDecision::Denied);
    }
}
