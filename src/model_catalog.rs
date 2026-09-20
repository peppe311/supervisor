//! Runtime-owned catalog refresh. Discovery never starts an inference turn.
use std::time::{Duration, Instant};

use crate::provider_types::{
    self, AgentModelOption, AgentProviderPhase, AgentProviderView, AgentSelection,
};

const REFRESH_INTERVAL: Duration = Duration::from_secs(15 * 60);
const RETRY_INTERVAL: Duration = Duration::from_secs(60);

pub(crate) struct ModelCatalogRefresh {
    next_check: Instant,
    automatic: Option<bool>,
    failures: u32,
}

impl Default for ModelCatalogRefresh {
    fn default() -> Self {
        Self {
            next_check: Instant::now(),
            automatic: None,
            failures: 0,
        }
    }
}

impl ModelCatalogRefresh {
    pub(crate) fn is_refreshing(&self) -> bool {
        self.automatic.is_some()
    }

    pub(crate) fn deadline(&self) -> Option<Instant> {
        (!self.is_refreshing()).then_some(self.next_check)
    }

    pub(crate) fn begin(&mut self, now: Instant, automatic: bool, busy: bool) -> bool {
        if self.is_refreshing() || (automatic && now < self.next_check) {
            return false;
        }
        if busy {
            self.next_check = now + RETRY_INTERVAL;
            return false;
        }
        self.automatic = Some(automatic);
        true
    }

    /// A run may have started after discovery: discard that result, and retry
    /// when idle rather than rewriting the profile of an active conversation.
    pub(crate) fn complete(&mut self, now: Instant, busy: bool) -> Option<bool> {
        let automatic = self.automatic.take()?;
        if busy {
            self.next_check = now + RETRY_INTERVAL;
            return None;
        }
        Some(automatic)
    }

    pub(crate) fn schedule_next(&mut self, now: Instant, loaded: bool) {
        let delay = if loaded {
            self.failures = 0;
            REFRESH_INTERVAL
        } else {
            self.failures = self.failures.saturating_add(1);
            (RETRY_INTERVAL * (1 << self.failures.saturating_sub(1).min(4))).min(REFRESH_INTERVAL)
        };
        self.next_check = now + delay;
    }
}

pub(crate) struct CatalogUpdate {
    pub loaded: bool,
    pub selection_changed: bool,
}

pub(crate) fn apply_catalog(
    provider: &mut AgentProviderView,
    models: &mut Vec<AgentModelOption>,
    selection: &mut AgentSelection,
    mut discovered_provider: AgentProviderView,
    discovered_models: Vec<AgentModelOption>,
    automatic: bool,
) -> CatalogUpdate {
    if discovered_provider.can_run() {
        if let Some(normalized) = provider_types::normalize_selection(&discovered_models, selection)
        {
            let selection_changed = *selection != normalized;
            *selection = normalized;
            *models = discovered_models;
            *provider = discovered_provider;
            return CatalogUpdate {
                loaded: true,
                selection_changed,
            };
        }
        discovered_provider.phase = AgentProviderPhase::Error;
        discovered_provider.detail = format!(
            "{} returned no usable model configuration",
            discovered_provider.name
        );
    }

    // A transient background error must not erase a good catalog. Explicit
    // sign-out/unavailability still disables execution; this is not an auth bypass.
    if automatic
        && provider.can_run()
        && !models.is_empty()
        && discovered_provider.phase == AgentProviderPhase::Error
    {
        provider.detail = format!(
            "Model refresh failed; showing the last available catalog. {} Automatic retry is scheduled.",
            discovered_provider.detail
        );
    } else {
        *provider = discovered_provider;
    }
    CatalogUpdate {
        loaded: false,
        selection_changed: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_types::{AgentReasoningEffortOption, AgentServiceTierOption};

    fn model(id: &str, is_default: bool) -> AgentModelOption {
        AgentModelOption {
            id: id.into(),
            model: id.into(),
            display_name: id.into(),
            is_default,
            default_reasoning_effort: "high".into(),
            supported_reasoning_efforts: vec![AgentReasoningEffortOption {
                reasoning_effort: "high".into(),
                description: String::new(),
            }],
            service_tiers: vec![AgentServiceTierOption {
                id: "fast".into(),
                name: "Fast".into(),
                description: String::new(),
            }],
            ..Default::default()
        }
    }

    fn ready() -> AgentProviderView {
        AgentProviderView::ready(None, "Connected")
    }

    #[test]
    fn refresh_is_single_flight_and_periodic_not_postponed_by_ui_events() {
        let mut refresh = ModelCatalogRefresh::default();
        let now = Instant::now();
        assert!(refresh.begin(now, true, false));
        assert!(!refresh.begin(now, false, false));
        assert_eq!(refresh.deadline(), None);
        assert_eq!(refresh.complete(now, false), Some(true));
        refresh.schedule_next(now, true);
        assert!(!refresh.begin(now + RETRY_INTERVAL, true, false));
        assert_eq!(refresh.deadline(), Some(now + REFRESH_INTERVAL));
        assert!(refresh.begin(now + REFRESH_INTERVAL, true, false));
    }

    #[test]
    fn active_runs_defer_both_discovery_and_late_results() {
        let mut refresh = ModelCatalogRefresh::default();
        let now = Instant::now();
        assert!(!refresh.begin(now, true, true));
        assert!(refresh.begin(now + RETRY_INTERVAL, true, false));
        assert_eq!(refresh.complete(now + RETRY_INTERVAL, true), None);
        assert_eq!(refresh.deadline(), Some(now + RETRY_INTERVAL * 2));
        assert!(!refresh.is_refreshing());
    }

    #[test]
    fn failures_back_off_manual_check_bypasses_delay_and_success_resets_it() {
        let mut refresh = ModelCatalogRefresh::default();
        let now = Instant::now();
        for seconds in [60, 120, 240, 480, 900, 900] {
            refresh.schedule_next(now, false);
            assert_eq!(refresh.deadline(), Some(now + Duration::from_secs(seconds)));
        }
        assert!(refresh.begin(now, false, false));
        assert_eq!(refresh.complete(now, false), Some(false));
        refresh.schedule_next(now, true);
        refresh.schedule_next(now, false);
        assert_eq!(refresh.deadline(), Some(now + RETRY_INTERVAL));
    }

    #[test]
    fn new_models_do_not_replace_a_valid_profile_even_if_default_changes() {
        let mut provider = ready();
        let mut models = vec![model("existing", true)];
        let mut selection = AgentSelection {
            model: "existing".into(),
            effort: "high".into(),
            service_tier: Some("fast".into()),
            personality: None,
            context_window: Some(128_000),
        };
        let previous = selection.clone();
        let update = apply_catalog(
            &mut provider,
            &mut models,
            &mut selection,
            ready(),
            vec![model("new-runtime-model", true), model("existing", false)],
            true,
        );
        assert!(update.loaded);
        assert!(!update.selection_changed);
        assert_eq!(selection, previous);
        assert_eq!(models.len(), 2);
    }

    #[test]
    fn withdrawn_model_falls_back_only_to_a_real_catalog_entry() {
        let mut provider = ready();
        let mut models = vec![model("old", true)];
        let mut selection = AgentSelection {
            model: "old".into(),
            ..Default::default()
        };
        let update = apply_catalog(
            &mut provider,
            &mut models,
            &mut selection,
            ready(),
            vec![model("replacement", true)],
            true,
        );
        assert!(update.selection_changed);
        assert_eq!(selection.model, "replacement");
        assert!(provider_types::validate_selection(&models, &selection).is_ok());
    }

    #[test]
    fn background_error_or_empty_catalog_retains_last_good_models_and_profile() {
        for failed in [
            AgentProviderView::error("Network unavailable", None),
            ready(),
        ] {
            let mut provider = ready();
            let mut models = vec![model("existing", true)];
            let mut selection = AgentSelection {
                model: "existing".into(),
                ..Default::default()
            };
            let previous = selection.clone();
            let update = apply_catalog(
                &mut provider,
                &mut models,
                &mut selection,
                failed,
                vec![],
                true,
            );
            assert!(!update.loaded);
            assert!(provider.can_run());
            assert!(provider.detail.contains("last available catalog"));
            assert_eq!(models.len(), 1);
            assert_eq!(selection, previous);
        }
    }

    #[test]
    fn sign_out_disables_execution_without_erasing_profile() {
        let mut provider = ready();
        let mut models = vec![model("existing", true)];
        let mut selection = AgentSelection {
            model: "existing".into(),
            ..Default::default()
        };
        apply_catalog(
            &mut provider,
            &mut models,
            &mut selection,
            AgentProviderView::unavailable("Not connected", None),
            vec![],
            true,
        );
        assert!(!provider.can_run());
        assert_eq!(models.len(), 1);
        assert_eq!(selection.model, "existing");
    }
}
