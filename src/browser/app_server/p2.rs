//! Explicit P2 App Server workflows. Values that can change filesystem,
//! account or process state stay behind a fresh native snapshot and an
//! explicit WebView confirmation. Raw migration items and process IDs never
//! cross into the WebView.
use super::*;
use crate::tab_context::sanitize_ui_text;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use central_agent_codex_runtime::api::{self, AddCreditsNudgeCreditType, Call};
use central_agent_codex_runtime::transport::CallError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use uuid::Uuid;

const MAX_ITEMS: usize = 512;
const MAX_TEXT: usize = 16 * 1024;
const MAX_OUTPUT: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Action {
    SyncScope {},
    RefreshFeatures {},
    SetFeature {
        view_id: String,
        name: String,
        enabled: bool,
    },
    DetectImport {
        scope_id: String,
        include_home: bool,
        include_project: bool,
    },
    Import {
        view_id: String,
        item_ids: Vec<String>,
    },
    ReadImportHistory {},
    ConsumeReset {},
    RetryReset {
        attempt_id: String,
    },
    SendNudge {
        credit_type: AddCreditsNudgeCreditType,
    },
    SubmitFeedback {
        classification: String,
        reason: String,
    },
    RunCommand {
        scope_id: String,
        command: Vec<String>,
        access: api::Access,
        rows: u16,
        cols: u16,
    },
    WriteCommand {
        process_id: String,
        input: String,
        close_stdin: bool,
    },
    ResizeCommand {
        process_id: String,
        rows: u16,
        cols: u16,
    },
    TerminateCommand {
        process_id: String,
    },
    ClearCommand {
        process_id: String,
    },
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct View {
    scope: ScopeView,
    features: FeatureInventory,
    migration: MigrationView,
    account_actions: AccountActionsView,
    feedback: FeedbackView,
    command: CommandView,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScopeView {
    id: Option<String>,
    project: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct FeatureInventory {
    view_id: Option<String>,
    loading: bool,
    loaded: bool,
    current: bool,
    entries: Vec<FeatureView>,
    error: Option<String>,
    notice: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FeatureView {
    name: String,
    stage: String,
    display_name: Option<String>,
    description: Option<String>,
    announcement: Option<String>,
    enabled: bool,
    default_enabled: bool,
    mutable: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationView {
    view_id: Option<String>,
    busy: bool,
    current: bool,
    items: Vec<MigrationItemView>,
    connectors: Vec<ConnectorView>,
    active_import: Option<ImportView>,
    histories: Vec<ImportHistoryView>,
    error: Option<String>,
    notice: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationItemView {
    id: String,
    item_type: String,
    description: String,
    scope: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectorView {
    name: String,
    source: String,
    session_count: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportView {
    status: String,
    results: Vec<ImportResultView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportResultView {
    item_type: String,
    successes: usize,
    failures: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportHistoryView {
    completed_at_ms: String,
    provider: bool,
    successes: usize,
    failures: usize,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountActionsView {
    busy: bool,
    reset_retry_id: Option<String>,
    reset_status: Option<String>,
    nudge_status: Option<String>,
    error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct FeedbackView {
    allowed: bool,
    busy: bool,
    status: Option<String>,
    error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandView {
    process: Option<CommandProcessView>,
    control_busy: bool,
    error: Option<String>,
    notice: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandProcessView {
    id: String,
    project: String,
    argv: Vec<String>,
    access: api::Access,
    status: String,
    exit_code: Option<i64>,
    output: Vec<OutputChunk>,
    truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OutputChunk {
    stream: String,
    text: String,
}

#[derive(Clone, Debug)]
enum Pending {
    FeaturePage,
    FeatureSet {
        name: String,
        enabled: bool,
    },
    Detect {
        include_home: bool,
        include_project: bool,
        scope_id: String,
        root: Option<String>,
    },
    Import,
    History,
    Reset {
        attempt_id: String,
        key: String,
    },
    Nudge,
    Feedback,
    CommandExec,
    CommandWrite,
    CommandResize,
    CommandTerminate,
}

#[derive(Clone, Debug)]
struct MigrationSnapshot {
    view_id: String,
    scope_id: String,
    items: BTreeMap<String, Value>,
}

#[derive(Clone, Debug)]
struct CommandProcess {
    handle: String,
    native_id: String,
    scope_id: String,
    output_bytes: usize,
}

#[derive(Default)]
pub(super) struct State {
    pub(super) view: View,
    scope_root: Option<String>,
    feature_catalog: Vec<FeatureView>,
    feature_cursors: HashSet<String>,
    migration: Option<MigrationSnapshot>,
    import_id: Option<String>,
    early_imports: BTreeMap<String, (bool, Vec<ImportResultView>)>,
    retry_reset: Option<(String, String)>,
    command: Option<CommandProcess>,
    pending: BTreeMap<String, Pending>,
}

pub(super) struct ReplyEffect {
    pub(super) follow_up: Option<(String, Call)>,
    pub(super) refresh_rate_limits: bool,
}

impl State {
    pub(super) fn sync_scope(&mut self, root: Option<&Path>) {
        let next = root.map(|root| root.display().to_string());
        if next == self.scope_root && self.view.scope.id.is_some() {
            return;
        }
        self.scope_root = next.clone();
        self.view.scope = ScopeView {
            id: Some(Uuid::new_v4().to_string()),
            project: next.as_deref().map(display_local_path),
        };
        self.migration = None;
        self.view.migration.view_id = None;
        self.view.migration.current = false;
        self.view.migration.items.clear();
        if self
            .command
            .as_ref()
            .is_some_and(|command| command.scope_id != self.view.scope.id.as_deref().unwrap_or(""))
            && self
                .view
                .command
                .process
                .as_ref()
                .is_some_and(|process| !command_is_active(&process.status))
        {
            self.command = None;
            self.view.command.process = None;
        }
    }

    pub(super) fn busy(&self) -> bool {
        !self.pending.is_empty()
            || self.import_id.is_some()
            || self
                .view
                .command
                .process
                .as_ref()
                .is_some_and(|process| command_is_active(&process.status))
    }

    pub(super) fn sync_requirements(&mut self, requirements: Option<&Value>) {
        self.view.feedback.allowed = feedback_allowed(requirements).unwrap_or(false);
    }

    pub(super) fn action(
        &mut self,
        action: Action,
        requirements: Option<&Value>,
    ) -> Result<Option<(String, Call)>, String> {
        self.sync_requirements(requirements);
        match action {
            Action::SyncScope {} => Ok(None),
            Action::RefreshFeatures {} => {
                self.ensure_no_pending(
                    |pending| matches!(pending, Pending::FeaturePage | Pending::FeatureSet { .. }),
                    "feature operation",
                )?;
                self.feature_catalog.clear();
                self.feature_cursors.clear();
                self.view.features.loading = true;
                self.view.features.current = false;
                self.view.features.error = None;
                self.view.features.notice = None;
                Ok(Some(self.insert(
                    Pending::FeaturePage,
                    api::experimental_features(None),
                )))
            }
            Action::SetFeature {
                view_id,
                name,
                enabled,
            } => {
                self.ensure_no_pending(
                    |pending| matches!(pending, Pending::FeaturePage | Pending::FeatureSet { .. }),
                    "feature operation",
                )?;
                if self.view.features.view_id.as_deref() != Some(&view_id)
                    || !self.view.features.current
                {
                    return Err("Refresh the feature inventory and confirm the current view".into());
                }
                let feature = self
                    .view
                    .features
                    .entries
                    .iter()
                    .find(|feature| feature.name == name)
                    .ok_or("The selected feature is not in the current inventory")?;
                if !feature.mutable {
                    return Err(
                        "Only features reported as beta or stable can be changed here".into(),
                    );
                }
                self.view.features.current = false;
                self.view.features.error = None;
                Ok(Some(self.insert(
                    Pending::FeatureSet {
                        name: name.clone(),
                        enabled,
                    },
                    api::set_experimental_feature(&name, enabled),
                )))
            }
            Action::DetectImport {
                scope_id,
                include_home,
                include_project,
            } => {
                self.ensure_scope(&scope_id)?;
                self.ensure_no_active_import()?;
                self.ensure_no_pending(
                    |pending| {
                        matches!(
                            pending,
                            Pending::Detect { .. } | Pending::Import | Pending::History
                        )
                    },
                    "migration operation",
                )?;
                if !include_home && !include_project {
                    return Err("Select the home scope, the current project, or both".into());
                }
                let cwds =
                    if include_project {
                        vec![self.scope_root.clone().ok_or(
                            "Open a local project before detecting project-scoped imports",
                        )?]
                    } else {
                        Vec::new()
                    };
                self.migration = None;
                self.view.migration.busy = true;
                self.view.migration.current = false;
                self.view.migration.view_id = None;
                self.view.migration.items.clear();
                self.view.migration.connectors.clear();
                self.view.migration.error = None;
                self.view.migration.notice = None;
                Ok(Some(self.insert(
                    Pending::Detect {
                        include_home,
                        include_project,
                        scope_id,
                        root: self.scope_root.clone(),
                    },
                    api::external_agent_detect(include_home, &cwds),
                )))
            }
            Action::Import { view_id, item_ids } => {
                self.ensure_no_active_import()?;
                self.ensure_no_pending(
                    |pending| {
                        matches!(
                            pending,
                            Pending::Detect { .. } | Pending::Import | Pending::History
                        )
                    },
                    "migration operation",
                )?;
                let snapshot = self
                    .migration
                    .as_ref()
                    .ok_or("Run detection before importing")?;
                if snapshot.view_id != view_id
                    || !self.view.migration.current
                    || self.view.scope.id.as_deref() != Some(&snapshot.scope_id)
                {
                    return Err(
                        "The import preview is stale. Detect again before confirming".into(),
                    );
                }
                if item_ids.is_empty() || item_ids.len() > snapshot.items.len() {
                    return Err("Select at least one detected item".into());
                }
                let mut seen = HashSet::new();
                let items = item_ids
                    .iter()
                    .map(|id| {
                        if !seen.insert(id.as_str()) {
                            return Err("The import selection contains a duplicate item".into());
                        }
                        snapshot.items.get(id).cloned().ok_or_else(|| {
                            "The import selection is not part of this preview".into()
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                self.migration = None;
                self.view.migration.busy = true;
                self.view.migration.current = false;
                self.view.migration.view_id = None;
                self.view.migration.items.clear();
                self.view.migration.connectors.clear();
                self.view.migration.error = None;
                self.early_imports.clear();
                self.view.migration.active_import = Some(ImportView {
                    status: "requesting".into(),
                    results: Vec::new(),
                });
                Ok(Some(self.insert(
                    Pending::Import,
                    api::external_agent_import(&items),
                )))
            }
            Action::ReadImportHistory {} => {
                self.ensure_no_active_import()?;
                self.ensure_no_pending(
                    |pending| {
                        matches!(
                            pending,
                            Pending::Detect { .. } | Pending::Import | Pending::History
                        )
                    },
                    "migration operation",
                )?;
                self.view.migration.busy = true;
                self.view.migration.error = None;
                Ok(Some(self.insert(
                    Pending::History,
                    api::external_agent_import_histories(),
                )))
            }
            Action::ConsumeReset {} => {
                self.ensure_no_pending(
                    |pending| matches!(pending, Pending::Reset { .. } | Pending::Nudge),
                    "account operation",
                )?;
                if self.retry_reset.is_some() {
                    return Err("Reconcile or retry the uncertain reset attempt first".into());
                }
                let attempt_id = Uuid::new_v4().to_string();
                let key = Uuid::new_v4().to_string();
                self.view.account_actions.busy = true;
                self.view.account_actions.error = None;
                self.view.account_actions.reset_status = Some("requesting".into());
                Ok(Some(self.insert(
                    Pending::Reset {
                        attempt_id,
                        key: key.clone(),
                    },
                    api::consume_rate_limit_reset_credit(&key),
                )))
            }
            Action::RetryReset { attempt_id } => {
                self.ensure_no_pending(
                    |pending| matches!(pending, Pending::Reset { .. } | Pending::Nudge),
                    "account operation",
                )?;
                let (expected, key) = self
                    .retry_reset
                    .clone()
                    .ok_or("There is no uncertain reset attempt to retry")?;
                if attempt_id != expected {
                    return Err("The reset retry confirmation is stale".into());
                }
                self.view.account_actions.busy = true;
                self.view.account_actions.error = None;
                self.view.account_actions.reset_status = Some("retrying_same_attempt".into());
                Ok(Some(self.insert(
                    Pending::Reset {
                        attempt_id,
                        key: key.clone(),
                    },
                    api::consume_rate_limit_reset_credit(&key),
                )))
            }
            Action::SendNudge { credit_type } => {
                self.ensure_no_pending(
                    |pending| matches!(pending, Pending::Reset { .. } | Pending::Nudge),
                    "account operation",
                )?;
                self.view.account_actions.busy = true;
                self.view.account_actions.error = None;
                self.view.account_actions.nudge_status = Some("requesting".into());
                Ok(Some(self.insert(
                    Pending::Nudge,
                    api::send_add_credits_nudge_email(credit_type),
                )))
            }
            Action::SubmitFeedback {
                classification,
                reason,
            } => {
                self.ensure_no_pending(
                    |pending| matches!(pending, Pending::Feedback),
                    "feedback upload",
                )?;
                if !feedback_allowed(requirements)? {
                    return Err("Managed Codex settings disable feedback upload".into());
                }
                if !matches!(
                    classification.as_str(),
                    "bug" | "bad_result" | "suggestion" | "other"
                ) {
                    return Err("Select a supported feedback category".into());
                }
                let reason = reason.trim();
                if reason.is_empty() || reason.len() > 4096 || reason.contains('\0') {
                    return Err("Feedback must contain between one and 4096 safe characters".into());
                }
                self.view.feedback.busy = true;
                self.view.feedback.error = None;
                self.view.feedback.status = Some("uploading_text_only".into());
                Ok(Some(self.insert(
                    Pending::Feedback,
                    api::feedback_upload(&classification, Some(reason)),
                )))
            }
            Action::RunCommand {
                scope_id,
                command,
                access,
                rows,
                cols,
            } => {
                self.ensure_scope(&scope_id)?;
                self.ensure_no_pending(
                    |pending| {
                        matches!(
                            pending,
                            Pending::CommandExec
                                | Pending::CommandWrite
                                | Pending::CommandResize
                                | Pending::CommandTerminate
                        )
                    },
                    "sandboxed command",
                )?;
                if self
                    .view
                    .command
                    .process
                    .as_ref()
                    .is_some_and(|process| command_is_active(&process.status))
                {
                    return Err("Terminate the current App Server command first".into());
                }
                access.validate_requirements(requirements)?;
                let root = self
                    .scope_root
                    .clone()
                    .ok_or("Open a local project before running a sandboxed command")?;
                let native_id = Uuid::new_v4().to_string();
                let handle = Uuid::new_v4().to_string();
                let call = api::command_exec(&command, &native_id, &root, access, rows, cols)?;
                self.command = Some(CommandProcess {
                    handle: handle.clone(),
                    native_id,
                    scope_id,
                    output_bytes: 0,
                });
                self.view.command = CommandView {
                    process: Some(CommandProcessView { id: handle, project: display_local_path(&root), argv: command, access, status: "running".into(), exit_code: None, output: Vec::new(), truncated: false }),
                    control_busy: false,
                    error: None,
                    notice: Some("Running through the Codex App Server sandbox, separate from Supervisor Terminal.".into()),
                };
                Ok(Some(self.insert(Pending::CommandExec, call)))
            }
            Action::WriteCommand {
                process_id,
                input,
                close_stdin,
            } => {
                self.ensure_command(&process_id)?;
                self.ensure_no_pending(
                    |pending| {
                        matches!(
                            pending,
                            Pending::CommandWrite
                                | Pending::CommandResize
                                | Pending::CommandTerminate
                        )
                    },
                    "command control",
                )?;
                if input.len() > 16 * 1024
                    || input.contains('\0')
                    || input.is_empty() && !close_stdin
                {
                    return Err("Provide bounded input or explicitly close stdin".into());
                }
                let encoded = (!input.is_empty()).then(|| STANDARD.encode(input.as_bytes()));
                let native = self.command.as_ref().unwrap().native_id.clone();
                self.view.command.control_busy = true;
                Ok(Some(self.insert(
                    Pending::CommandWrite,
                    api::command_write(&native, encoded.as_deref(), close_stdin),
                )))
            }
            Action::ResizeCommand {
                process_id,
                rows,
                cols,
            } => {
                self.ensure_command(&process_id)?;
                self.ensure_no_pending(
                    |pending| {
                        matches!(
                            pending,
                            Pending::CommandWrite
                                | Pending::CommandResize
                                | Pending::CommandTerminate
                        )
                    },
                    "command control",
                )?;
                if !(5..=200).contains(&rows) || !(20..=500).contains(&cols) {
                    return Err("Terminal size is outside the supported range".into());
                }
                let native = self.command.as_ref().unwrap().native_id.clone();
                self.view.command.control_busy = true;
                Ok(Some(self.insert(
                    Pending::CommandResize,
                    api::command_resize(&native, rows, cols),
                )))
            }
            Action::TerminateCommand { process_id } => {
                self.ensure_command(&process_id)?;
                self.ensure_no_pending(
                    |pending| {
                        matches!(
                            pending,
                            Pending::CommandWrite
                                | Pending::CommandResize
                                | Pending::CommandTerminate
                        )
                    },
                    "command control",
                )?;
                let native = self.command.as_ref().unwrap().native_id.clone();
                self.view.command.control_busy = true;
                if let Some(process) = &mut self.view.command.process {
                    process.status = "stopping".into();
                }
                Ok(Some(self.insert(
                    Pending::CommandTerminate,
                    api::command_terminate(&native),
                )))
            }
            Action::ClearCommand { process_id } => {
                let process = self
                    .view
                    .command
                    .process
                    .as_ref()
                    .ok_or("There is no command result to clear")?;
                if process.id != process_id || command_is_active(&process.status) {
                    return Err("The command result cannot be cleared yet".into());
                }
                self.command = None;
                self.view.command = CommandView::default();
                Ok(None)
            }
        }
    }

    pub(super) fn reply(&mut self, id: &str, result: Result<Value, CallError>) -> ReplyEffect {
        let Some(pending) = self.pending.remove(id) else {
            return ReplyEffect {
                follow_up: None,
                refresh_rate_limits: false,
            };
        };
        let mut effect = ReplyEffect {
            follow_up: None,
            refresh_rate_limits: false,
        };
        match (pending, result) {
            (Pending::FeaturePage, Ok(value)) => match self.feature_page(&value) {
                Ok(Some(cursor)) => {
                    effect.follow_up = Some(self.insert(
                        Pending::FeaturePage,
                        api::experimental_features(Some(&cursor)),
                    ));
                }
                Ok(None) => {}
                Err(error) => self.feature_failed(error),
            },
            (Pending::FeaturePage, Err(error)) => self.feature_failed(safe_call_error(error)),
            (Pending::FeatureSet { name, enabled }, Ok(value)) => {
                let accepted = value
                    .get("enablement")
                    .and_then(Value::as_object)
                    .and_then(|map| map.get(&name))
                    .and_then(Value::as_bool);
                if accepted != Some(enabled) {
                    self.feature_failed(
                        "Feature update response did not confirm the requested value".into(),
                    );
                } else {
                    if let Some(feature) = self
                        .view
                        .features
                        .entries
                        .iter_mut()
                        .find(|feature| feature.name == name)
                    {
                        feature.enabled = enabled;
                    }
                    self.view.features.current = true;
                    self.view.features.notice = Some(format!(
                        "{} is now {} for this Codex process. experimentalApi remains disabled.",
                        name,
                        if enabled { "enabled" } else { "disabled" }
                    ));
                }
            }
            (Pending::FeatureSet { .. }, Err(error)) => self.feature_failed(safe_call_error(error)),
            (
                Pending::Detect {
                    include_home,
                    include_project,
                    scope_id,
                    root,
                },
                Ok(value),
            ) => {
                if let Err(error) = self.detected(
                    &value,
                    include_home,
                    include_project,
                    &scope_id,
                    root.as_deref(),
                ) {
                    self.migration_failed(error);
                }
            }
            (Pending::Detect { .. }, Err(error)) => self.migration_failed(safe_call_error(error)),
            (Pending::Import, Ok(value)) => {
                let import_id = value
                    .get("importId")
                    .and_then(Value::as_str)
                    .filter(|value| bounded(value, 512));
                if let Some(import_id) = import_id {
                    self.import_id = Some(import_id.into());
                    self.view.migration.busy = true;
                    if let Some(import) = &mut self.view.migration.active_import {
                        import.status = "accepted".into();
                    }
                    if let Some((completed, results)) = self.early_imports.remove(import_id) {
                        self.apply_import_notification(completed, results);
                    }
                } else {
                    self.import_failed("Import response is invalid".into());
                }
            }
            (
                Pending::Import,
                Err(
                    error @ CallError::Disconnected {
                        delivery_unknown: true,
                        ..
                    },
                ),
            ) => self.import_uncertain(safe_call_error(error)),
            (Pending::Import, Err(error)) => self.import_failed(safe_call_error(error)),
            (Pending::History, Ok(value)) => match parse_histories(&value) {
                Ok(histories) => {
                    self.view.migration.histories = histories;
                    self.view.migration.busy = false;
                    self.view.migration.notice = Some(
                        "Import history refreshed without exposing source or target paths.".into(),
                    );
                }
                Err(error) => self.migration_failed(error),
            },
            (Pending::History, Err(error)) => self.migration_failed(safe_call_error(error)),
            (Pending::Reset { attempt_id, key }, result) => {
                self.view.account_actions.busy = false;
                match result {
                    Ok(value) => match value.get("outcome").and_then(Value::as_str) {
                        Some(
                            outcome @ ("reset" | "nothingToReset" | "noCredit" | "alreadyRedeemed"),
                        ) => {
                            self.retry_reset = None;
                            self.view.account_actions.reset_retry_id = None;
                            self.view.account_actions.reset_status = Some(outcome.into());
                            effect.refresh_rate_limits =
                                matches!(outcome, "reset" | "alreadyRedeemed");
                        }
                        _ => self.reset_uncertain(
                            attempt_id,
                            key,
                            "Reset response is invalid; the account outcome is unknown.".into(),
                        ),
                    },
                    Err(CallError::Disconnected {
                        reason,
                        delivery_unknown: true,
                    }) => {
                        self.retry_reset = Some((attempt_id.clone(), key));
                        self.view.account_actions.reset_retry_id = Some(attempt_id);
                        self.view.account_actions.reset_status = Some("uncertain".into());
                        let (reason, _) = sanitize_ui_text(&reason, 4096);
                        self.view.account_actions.error = Some(format!(
                            "{}. Retry uses the same idempotency key.",
                            if reason.is_empty() {
                                "The App Server connection closed"
                            } else {
                                &reason
                            }
                        ));
                    }
                    Err(error) => {
                        let error = safe_call_error(error);
                        if self
                            .retry_reset
                            .as_ref()
                            .is_some_and(|(known, _)| known == &attempt_id)
                        {
                            self.reset_uncertain(
                                attempt_id,
                                key,
                                format!(
                                    "{error} The earlier reset attempt remains uncertain; any retry will reuse its idempotency key."
                                ),
                            );
                        } else {
                            self.retry_reset = None;
                            self.view.account_actions.reset_retry_id = None;
                            self.view.account_actions.reset_status = Some("failed".into());
                            self.view.account_actions.error = Some(error);
                        }
                    }
                }
            }
            (Pending::Nudge, result) => {
                self.view.account_actions.busy = false;
                match result {
                    Ok(value) => match value.get("status").and_then(Value::as_str) {
                        Some(status @ ("sent" | "cooldown_active")) => {
                            self.view.account_actions.nudge_status = Some(status.into())
                        }
                        _ => {
                            self.view.account_actions.nudge_status = Some("uncertain".into());
                            self.view.account_actions.error =
                                Some("Email response is invalid; delivery is unknown and was not retried.".into())
                        }
                    },
                    Err(
                        error @ CallError::Disconnected {
                            delivery_unknown: true,
                            ..
                        },
                    ) => {
                        self.view.account_actions.nudge_status = Some("uncertain".into());
                        self.view.account_actions.error = Some(format!(
                            "{} The email was not requested again automatically.",
                            safe_call_error(error)
                        ));
                    }
                    Err(error) => {
                        self.view.account_actions.nudge_status = Some("failed".into());
                        self.view.account_actions.error = Some(safe_call_error(error));
                    }
                }
            }
            (Pending::Feedback, result) => {
                self.view.feedback.busy = false;
                match result {
                    Ok(value)
                        if value
                            .get("threadId")
                            .and_then(Value::as_str)
                            .is_some_and(|value| bounded(value, 512)) =>
                    {
                        self.view.feedback.status = Some("sent_without_logs_or_files".into())
                    }
                    Ok(_) => {
                        self.view.feedback.status = Some("uncertain".into());
                        self.view.feedback.error = Some(
                            "Feedback response is invalid; delivery is unknown and was not retried."
                                .into(),
                        );
                    }
                    Err(
                        error @ CallError::Disconnected {
                            delivery_unknown: true,
                            ..
                        },
                    ) => {
                        self.view.feedback.status = Some("uncertain".into());
                        self.view.feedback.error = Some(format!(
                            "{} The feedback was not uploaded again automatically.",
                            safe_call_error(error)
                        ));
                    }
                    Err(error) => {
                        self.view.feedback.status = Some("failed".into());
                        self.view.feedback.error = Some(safe_call_error(error));
                    }
                }
            }
            (Pending::CommandExec, result) => self.command_finished(result),
            (Pending::CommandWrite, result) => {
                self.command_control_finished(result, "Input sent to the App Server process.")
            }
            (Pending::CommandResize, result) => {
                self.command_control_finished(result, "App Server PTY resized.")
            }
            (Pending::CommandTerminate, result) => self.command_terminate_finished(result),
        }
        effect
    }

    pub(super) fn notification(&mut self, method: &str, params: &Value) -> Result<bool, String> {
        match method {
            "command/exec/outputDelta" => {
                let native = self
                    .command
                    .as_ref()
                    .ok_or("Command output has no owned process")?;
                if params.get("processId").and_then(Value::as_str)
                    != Some(native.native_id.as_str())
                {
                    return Ok(false);
                }
                let stream = params
                    .get("stream")
                    .and_then(Value::as_str)
                    .filter(|stream| matches!(*stream, "stdout" | "stderr"))
                    .ok_or("Command output stream is invalid")?;
                let encoded = params
                    .get("deltaBase64")
                    .and_then(Value::as_str)
                    .filter(|value| value.len() <= 2 * 1024 * 1024)
                    .ok_or("Command output chunk is invalid")?;
                let cap_reached = params
                    .get("capReached")
                    .and_then(Value::as_bool)
                    .ok_or("Command output cap state is invalid")?;
                let bytes = STANDARD
                    .decode(encoded)
                    .map_err(|_| "Command output is not valid base64")?;
                let remaining = MAX_OUTPUT.saturating_sub(native.output_bytes);
                let (text, locally_truncated) = clean_output(&bytes, remaining);
                let retained = text.len();
                let process = self.command.as_mut().unwrap();
                process.output_bytes = process.output_bytes.saturating_add(retained);
                if let Some(view) = &mut self.view.command.process {
                    append_output(view, stream, text);
                    if locally_truncated || cap_reached {
                        view.truncated = true;
                    }
                }
                Ok(true)
            }
            "externalAgentConfig/import/progress" | "externalAgentConfig/import/completed" => {
                let import_id = params
                    .get("importId")
                    .and_then(Value::as_str)
                    .filter(|value| bounded(value, 512))
                    .ok_or("Import notification has an invalid ID")?;
                let completed = method.ends_with("completed");
                let results = parse_import_results(
                    params
                        .get("itemTypeResults")
                        .ok_or("Import notification is incomplete")?,
                )?;
                if self.import_id.as_deref() == Some(import_id) {
                    self.apply_import_notification(completed, results);
                } else if self.early_imports.contains_key(import_id) || self.early_imports.len() < 8
                {
                    match self.early_imports.get(import_id) {
                        Some((true, _)) if !completed => {}
                        _ => {
                            self.early_imports
                                .insert(import_id.into(), (completed, results));
                        }
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub(super) fn disconnect(&mut self) {
        let feature_write_uncertain = self
            .pending
            .values()
            .any(|pending| matches!(pending, Pending::FeatureSet { .. }));
        let import_uncertain = self.import_id.is_some()
            || self
                .pending
                .values()
                .any(|pending| matches!(pending, Pending::Import));
        let nudge_uncertain = self
            .pending
            .values()
            .any(|pending| matches!(pending, Pending::Nudge));
        let feedback_uncertain = self
            .pending
            .values()
            .any(|pending| matches!(pending, Pending::Feedback));
        self.view.features.current = false;
        self.view.features.loading = false;
        self.view.migration.current = false;
        self.view.migration.busy = false;
        self.view.feedback.allowed = false;
        self.view.feedback.busy = false;
        self.view.account_actions.busy = false;
        if feature_write_uncertain {
            self.view.features.error = Some(
                "The feature update result is unknown. Refresh the inventory; it was not replayed."
                    .into(),
            );
        }
        if import_uncertain {
            if let Some(import) = &mut self.view.migration.active_import {
                import.status = "uncertain".into();
            }
            self.view.migration.error = Some(
                "Import outcome is unknown. Reconnect and refresh import history before choosing whether to try again."
                    .into(),
            );
        }
        if nudge_uncertain {
            self.view.account_actions.nudge_status = Some("uncertain".into());
            self.view.account_actions.error = Some(
                "Email delivery is unknown. The request was not replayed; a later request may be subject to cooldown."
                    .into(),
            );
        }
        if feedback_uncertain {
            self.view.feedback.status = Some("uncertain".into());
            self.view.feedback.error = Some(
                "Feedback delivery is unknown. It was not uploaded again automatically.".into(),
            );
        }
        if let Some(Pending::Reset { attempt_id, key }) = self
            .pending
            .values()
            .find(|pending| matches!(pending, Pending::Reset { .. }))
            .cloned()
        {
            self.retry_reset = Some((attempt_id.clone(), key));
            self.view.account_actions.reset_retry_id = Some(attempt_id);
            self.view.account_actions.reset_status = Some("uncertain".into());
        }
        if let Some(process) = &mut self.view.command.process
            && command_is_active(&process.status)
        {
            process.status = "connection_closed".into();
            self.view.command.error = Some(
                "The App Server connection closed and terminated this sandboxed process.".into(),
            );
        }
        self.view.command.control_busy = false;
        self.pending.clear();
        self.import_id = None;
        self.early_imports.clear();
    }

    fn insert(&mut self, pending: Pending, call: Call) -> (String, Call) {
        let id = Uuid::new_v4().to_string();
        self.pending.insert(id.clone(), pending);
        (id, call)
    }

    fn ensure_scope(&self, scope_id: &str) -> Result<(), String> {
        if self.view.scope.id.as_deref() != Some(scope_id) {
            return Err("The active project changed. Reopen and confirm this operation".into());
        }
        Ok(())
    }

    fn ensure_no_pending(
        &self,
        predicate: impl Fn(&Pending) -> bool,
        label: &str,
    ) -> Result<(), String> {
        if self.pending.values().any(predicate) {
            Err(format!("Wait for the current {label}"))
        } else {
            Ok(())
        }
    }

    fn ensure_no_active_import(&self) -> Result<(), String> {
        if self.import_id.is_some() {
            Err(
                "Wait for the active import to complete, or reconnect and reconcile its history"
                    .into(),
            )
        } else {
            Ok(())
        }
    }

    fn ensure_command(&self, handle: &str) -> Result<(), String> {
        let command = self
            .command
            .as_ref()
            .ok_or("There is no owned App Server process")?;
        let view = self
            .view
            .command
            .process
            .as_ref()
            .ok_or("There is no current command view")?;
        if command.handle != handle || view.id != handle || view.status != "running" {
            return Err("The App Server process is no longer writable".into());
        }
        Ok(())
    }

    fn feature_page(&mut self, value: &Value) -> Result<Option<String>, String> {
        let entries = value
            .get("data")
            .and_then(Value::as_array)
            .ok_or("Feature inventory is incomplete")?;
        if self.feature_catalog.len().saturating_add(entries.len()) > MAX_ITEMS {
            return Err("Feature inventory is unexpectedly large".into());
        }
        for value in entries {
            self.feature_catalog.push(parse_feature(value)?);
        }
        let cursor = parse_cursor(value)?;
        if let Some(cursor) = cursor {
            if !self.feature_cursors.insert(cursor.clone()) {
                return Err("Feature pagination repeated a cursor".into());
            }
            return Ok(Some(cursor));
        }
        let mut names = HashSet::new();
        if self
            .feature_catalog
            .iter()
            .any(|feature| !names.insert(feature.name.clone()))
        {
            return Err("Feature inventory contains duplicate names".into());
        }
        self.view.features.entries = std::mem::take(&mut self.feature_catalog);
        self.view.features.view_id = Some(Uuid::new_v4().to_string());
        self.view.features.loading = false;
        self.view.features.loaded = true;
        self.view.features.current = true;
        Ok(None)
    }

    fn feature_failed(&mut self, error: String) {
        self.feature_catalog.clear();
        self.feature_cursors.clear();
        self.view.features.loading = false;
        self.view.features.current = false;
        self.view.features.error = Some(error);
    }

    fn detected(
        &mut self,
        value: &Value,
        include_home: bool,
        include_project: bool,
        requested_scope_id: &str,
        requested_root: Option<&str>,
    ) -> Result<(), String> {
        if self.view.scope.id.as_deref() != Some(requested_scope_id)
            || self.scope_root.as_deref() != requested_root
        {
            return Err(
                "The active project changed while detection was running. Detect again".into(),
            );
        }
        let items = value
            .get("items")
            .and_then(Value::as_array)
            .filter(|items| items.len() <= MAX_ITEMS)
            .ok_or("Detected migration inventory is invalid")?;
        let connectors = value
            .get("connectors")
            .and_then(Value::as_array)
            .filter(|items| items.len() <= 64)
            .ok_or("Detected connector inventory is invalid")?;
        let scope_id = requested_scope_id.to_owned();
        let root = requested_root;
        let view_id = Uuid::new_v4().to_string();
        let mut raw = BTreeMap::new();
        let mut views = Vec::with_capacity(items.len());
        for value in items {
            let item_type = value
                .get("itemType")
                .and_then(Value::as_str)
                .filter(|value| valid_item_type(value))
                .ok_or("Detected migration item type is invalid")?;
            let description = value
                .get("description")
                .and_then(Value::as_str)
                .filter(|value| bounded(value, MAX_TEXT))
                .ok_or("Detected migration description is invalid")?;
            let cwd = match value.get("cwd") {
                Some(Value::Null) => None,
                Some(Value::String(value)) if value.is_empty() => None,
                Some(Value::String(value))
                    if include_project && root.is_some_and(|root| same_path(root, value)) =>
                {
                    Some(value.as_str())
                }
                _ => return Err("Detected migration item escaped the requested scopes".into()),
            };
            if cwd.is_none() && !include_home {
                return Err("Detection returned a home-scoped item that was not requested".into());
            }
            let id = Uuid::new_v4().to_string();
            raw.insert(id.clone(), value.clone());
            views.push(MigrationItemView {
                id,
                item_type: item_type.into(),
                description: description.into(),
                scope: cwd.map_or("Home".into(), |_| "Current project".into()),
            });
        }
        let connector_views = connectors
            .iter()
            .map(parse_connector)
            .collect::<Result<Vec<_>, _>>()?;
        self.migration = Some(MigrationSnapshot {
            view_id: view_id.clone(),
            scope_id,
            items: raw,
        });
        self.view.migration.view_id = Some(view_id);
        self.view.migration.busy = false;
        self.view.migration.current = true;
        self.view.migration.items = views;
        self.view.migration.connectors = connector_views;
        self.view.migration.notice = Some(
            "Preview ready. Import uses these exact native items; details remain Rust-side.".into(),
        );
        Ok(())
    }

    fn migration_failed(&mut self, error: String) {
        self.view.migration.busy = false;
        self.view.migration.current = false;
        self.view.migration.error = Some(error);
    }

    fn import_failed(&mut self, error: String) {
        self.import_id = None;
        self.migration_failed(error);
        if let Some(import) = &mut self.view.migration.active_import {
            import.status = "failed".into();
        }
    }

    fn import_uncertain(&mut self, error: String) {
        self.import_id = None;
        self.view.migration.busy = false;
        self.view.migration.current = false;
        self.view.migration.error = Some(format!(
            "{error} Reconnect and refresh import history before choosing whether to try again."
        ));
        if let Some(import) = &mut self.view.migration.active_import {
            import.status = "uncertain".into();
        }
    }

    fn apply_import_notification(&mut self, completed: bool, results: Vec<ImportResultView>) {
        self.view.migration.busy = !completed;
        self.view.migration.active_import = Some(ImportView {
            status: if completed {
                "completed"
            } else {
                "in_progress"
            }
            .into(),
            results,
        });
        if completed {
            self.view.migration.notice =
                Some("Import completed. Refresh history for the durable server record.".into());
            self.import_id = None;
        }
    }

    fn reset_uncertain(&mut self, attempt_id: String, key: String, error: String) {
        self.retry_reset = Some((attempt_id.clone(), key));
        self.view.account_actions.reset_retry_id = Some(attempt_id);
        self.view.account_actions.reset_status = Some("uncertain".into());
        self.view.account_actions.error = Some(error);
    }

    fn command_finished(&mut self, result: Result<Value, CallError>) {
        let Some(view) = &mut self.view.command.process else {
            return;
        };
        match result {
            Ok(value) => {
                let Some(exit_code) = value.get("exitCode").and_then(Value::as_i64) else {
                    self.view.command.error = Some("Command response is invalid".into());
                    view.status = "failed".into();
                    return;
                };
                for stream in ["stdout", "stderr"] {
                    if let Some(text) = value
                        .get(stream)
                        .and_then(Value::as_str)
                        .filter(|text| !text.is_empty())
                    {
                        let remaining = MAX_OUTPUT.saturating_sub(
                            self.command
                                .as_ref()
                                .map_or(0, |command| command.output_bytes),
                        );
                        let (safe, truncated) = clean_output(text.as_bytes(), remaining);
                        if let Some(command) = &mut self.command {
                            command.output_bytes = command.output_bytes.saturating_add(safe.len());
                        }
                        append_output(view, stream, safe);
                        if truncated {
                            view.truncated = true;
                        }
                    }
                }
                view.exit_code = Some(exit_code);
                view.status = if exit_code == 0 {
                    "completed"
                } else {
                    "failed"
                }
                .into();
                self.view.command.notice =
                    Some(format!("App Server process exited with code {exit_code}."));
            }
            Err(error) => {
                view.status = "failed".into();
                self.view.command.error = Some(safe_call_error(error));
            }
        }
        self.view.command.control_busy = false;
    }

    fn command_control_finished(&mut self, result: Result<Value, CallError>, notice: &str) {
        self.view.command.control_busy = false;
        match result {
            Ok(value) if value.as_object().is_some_and(|value| value.is_empty()) => {
                self.view.command.notice = Some(notice.into())
            }
            Ok(_) => self.view.command.error = Some("Command control response is invalid".into()),
            Err(error) => self.view.command.error = Some(safe_call_error(error)),
        }
    }

    fn command_terminate_finished(&mut self, result: Result<Value, CallError>) {
        self.view.command.control_busy = false;
        let Some(process) = &mut self.view.command.process else {
            return;
        };
        if !matches!(
            process.status.as_str(),
            "stopping" | "termination_uncertain"
        ) {
            return;
        }
        match result {
            Ok(value) if value.as_object().is_some_and(|value| value.is_empty()) => {
                process.status = "stopping".into();
                self.view.command.notice =
                    Some("Termination requested; waiting for the final process response.".into());
            }
            Ok(_) => {
                process.status = "termination_uncertain".into();
                self.view.command.error = Some(
                    "Termination returned an invalid response. The process outcome is unknown; no control was replayed."
                        .into(),
                );
            }
            Err(
                error @ CallError::Disconnected {
                    delivery_unknown: true,
                    ..
                },
            ) => {
                process.status = "termination_uncertain".into();
                self.view.command.error = Some(format!(
                    "{} The termination request was not replayed.",
                    safe_call_error(error)
                ));
            }
            Err(error) => {
                process.status = "running".into();
                self.view.command.error = Some(safe_call_error(error));
            }
        }
    }
}

fn command_is_active(status: &str) -> bool {
    matches!(status, "running" | "stopping" | "termination_uncertain")
}

fn parse_feature(value: &Value) -> Result<FeatureView, String> {
    let name = required_string(value, "name", 512)?;
    let stage = required_string(value, "stage", 64)?;
    if !matches!(
        stage.as_str(),
        "beta" | "underDevelopment" | "stable" | "deprecated" | "removed"
    ) {
        return Err("Feature stage is invalid".into());
    }
    Ok(FeatureView {
        name,
        mutable: matches!(stage.as_str(), "beta" | "stable"),
        stage,
        display_name: optional_string(value, "displayName", 1024)?,
        description: optional_string(value, "description", MAX_TEXT)?,
        announcement: optional_string(value, "announcement", MAX_TEXT)?,
        enabled: value
            .get("enabled")
            .and_then(Value::as_bool)
            .ok_or("Feature enablement is invalid")?,
        default_enabled: value
            .get("defaultEnabled")
            .and_then(Value::as_bool)
            .ok_or("Feature default is invalid")?,
    })
}

fn parse_connector(value: &Value) -> Result<ConnectorView, String> {
    let source = required_string(value, "source", 64)?;
    if !matches!(source.as_str(), "remoteMcpServersConfig" | "sessionToolUse") {
        return Err("Detected connector source is invalid".into());
    }
    Ok(ConnectorView {
        name: required_string(value, "name", 1024)?,
        source,
        session_count: value
            .get("sessionCount")
            .and_then(Value::as_u64)
            .ok_or("Detected connector count is invalid")?,
    })
}

fn parse_import_results(value: &Value) -> Result<Vec<ImportResultView>, String> {
    let mut item_types = HashSet::new();
    value
        .as_array()
        .filter(|items| items.len() <= 64)
        .ok_or("Import results are invalid")?
        .iter()
        .map(|value| {
            let item_type = required_string(value, "itemType", 64)?;
            if !valid_item_type(&item_type) {
                return Err("Import result type is invalid".into());
            }
            if !item_types.insert(item_type.clone()) {
                return Err("Import results contain a duplicate item type".into());
            }
            let successes = value
                .get("successes")
                .and_then(Value::as_array)
                .filter(|items| items.len() <= MAX_ITEMS)
                .ok_or("Import successes are invalid")?
                .len();
            let failures = value
                .get("failures")
                .and_then(Value::as_array)
                .filter(|items| items.len() <= MAX_ITEMS)
                .ok_or("Import failures are invalid")?;
            Ok(ImportResultView {
                item_type,
                successes,
                failures: failures.len(),
            })
        })
        .collect()
}

fn parse_histories(value: &Value) -> Result<Vec<ImportHistoryView>, String> {
    value
        .get("connectors")
        .and_then(Value::as_array)
        .filter(|items| items.len() <= 128)
        .ok_or("Import-history connector inventory is invalid")?;
    value
        .get("data")
        .and_then(Value::as_array)
        .filter(|items| items.len() <= MAX_ITEMS)
        .ok_or("Import history is invalid")?
        .iter()
        .map(|history| {
            required_string(history, "importId", 512)?;
            let completed = history
                .get("completedAtMs")
                .and_then(Value::as_u64)
                .ok_or("Import history timestamp is invalid")?;
            let successes = history
                .get("successes")
                .and_then(Value::as_array)
                .filter(|items| items.len() <= MAX_ITEMS)
                .ok_or("Import history successes are invalid")?
                .len();
            let failures = history
                .get("failures")
                .and_then(Value::as_array)
                .filter(|items| items.len() <= MAX_ITEMS)
                .ok_or("Import history failures are invalid")?
                .len();
            Ok(ImportHistoryView {
                completed_at_ms: completed.to_string(),
                provider: history
                    .get("providerId")
                    .is_some_and(|value| !value.is_null()),
                successes,
                failures,
            })
        })
        .collect()
}

fn parse_cursor(value: &Value) -> Result<Option<String>, String> {
    match value.get("nextCursor") {
        Some(Value::Null) => Ok(None),
        Some(Value::String(cursor)) if bounded(cursor, 4096) => Ok(Some(cursor.clone())),
        _ => Err("Feature pagination is incomplete".into()),
    }
}

fn required_string(value: &Value, key: &str, max: usize) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| bounded(value, max))
        .map(str::to_owned)
        .ok_or_else(|| format!("Response field {key} is invalid"))
}

fn optional_string(value: &Value, key: &str, max: usize) -> Result<Option<String>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if bounded(text, max) => Ok(Some(text.clone())),
        _ => Err(format!("Response field {key} is invalid")),
    }
}

fn feedback_allowed(requirements: Option<&Value>) -> Result<bool, String> {
    let requirements = requirements.ok_or("Native managed requirements have not loaded")?;
    match requirements.get("feedback") {
        Some(Value::Null) => Ok(true),
        Some(Value::Object(feedback)) => match feedback.get("enabled") {
            None | Some(Value::Null) => Ok(true),
            Some(Value::Bool(enabled)) => Ok(*enabled),
            _ => Err("Native feedback requirements are invalid".into()),
        },
        None | Some(_) => Err("Native feedback requirements are invalid".into()),
    }
}

fn safe_call_error(error: CallError) -> String {
    match error {
        CallError::Rpc(error) => {
            format!("App Server rejected the operation (RPC {}).", error.code)
        }
        error => {
            let (safe, _) = sanitize_ui_text(&error.to_string(), 4096);
            if safe.is_empty() {
                "The App Server operation failed without a public error message.".into()
            } else {
                safe
            }
        }
    }
}

fn bounded(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.contains('\0')
}

fn valid_item_type(value: &str) -> bool {
    matches!(
        value,
        "AGENTS_MD"
            | "CONFIG"
            | "SKILLS"
            | "PLUGINS"
            | "MCP_SERVER_CONFIG"
            | "SUBAGENTS"
            | "HOOKS"
            | "COMMANDS"
            | "MEMORY"
            | "SESSIONS"
    )
}

fn clean_output(bytes: &[u8], maximum: usize) -> (String, bool) {
    let mut output: String = String::from_utf8_lossy(bytes)
        .chars()
        .filter(|character| !character.is_control() || matches!(*character, '\n' | '\r' | '\t'))
        .collect();
    let truncated = output.len() > maximum;
    if truncated {
        let mut boundary = maximum.min(output.len());
        while boundary > 0 && !output.is_char_boundary(boundary) {
            boundary -= 1;
        }
        output.truncate(boundary);
    }
    (output, truncated)
}

fn append_output(process: &mut CommandProcessView, stream: &str, text: String) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = process.output.last_mut()
        && last.stream == stream
        && last.text.len().saturating_add(text.len()) <= 64 * 1024
    {
        last.text.push_str(&text);
        return;
    }
    if process.output.len() >= 4096 {
        process.truncated = true;
        return;
    }
    process.output.push(OutputChunk {
        stream: stream.into(),
        text,
    });
}

fn same_path(left: &str, right: &str) -> bool {
    let left = std::fs::canonicalize(left).unwrap_or_else(|_| Path::new(left).to_path_buf());
    let right = std::fs::canonicalize(right).unwrap_or_else(|_| Path::new(right).to_path_buf());
    if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}

impl BrowserApp {
    pub(in crate::browser) fn sync_app_server_p2_scope(&mut self) {
        let root = self.workspace.root().map(Path::to_path_buf);
        self.app_server.p2.sync_scope(root.as_deref());
        self.app_server
            .p2
            .sync_requirements(self.app_server.requirements.as_ref());
        self.app_server.sync_profile();
    }

    pub(in crate::browser) fn app_server_p2(&mut self, action: Action) {
        self.sync_app_server_p2_scope();
        let result = (|| {
            if matches!(action, Action::SyncScope {}) {
                return Ok(None);
            }
            if !self.app_server.view.connected || self.app_server.client.is_none() {
                return Err("Connect Codex App Server first".into());
            }
            if matches!(
                &action,
                Action::ConsumeReset {} | Action::RetryReset { .. } | Action::SendNudge { .. }
            ) && !self
                .app_server
                .view
                .account
                .as_ref()
                .is_some_and(|account| account.supported)
            {
                return Err(
                    "These account actions require a connected ChatGPT subscription account".into(),
                );
            }
            if matches!(&action, Action::ConsumeReset {})
                && !self.app_server.account_state.reset_credit_available()
            {
                return Err(
                    "Refresh rate limits and confirm that a reset credit is available".into(),
                );
            }
            if matches!(
                &action,
                Action::SetFeature { .. } | Action::Import { .. } | Action::RunCommand { .. }
            ) && self.app_server.any_busy()
            {
                return Err(
                    "Finish active Codex work before this process or project mutation".into(),
                );
            }
            self.app_server
                .p2
                .action(action, self.app_server.requirements.as_ref())
        })();
        match result {
            Ok(Some((id, call))) => {
                self.app_server.view.errors.remove("p2");
                self.app_server_p2_call(id, call);
            }
            Ok(None) => {
                self.app_server.view.errors.remove("p2");
            }
            Err(error) => {
                self.app_server.view.errors.insert("p2", error);
            }
        }
        self.app_server.sync_profile();
        self.render_agent_panel();
    }

    pub(super) fn app_server_p2_call(&self, id: String, call: Call) {
        let Some(client) = self.app_server.client.clone() else {
            return;
        };
        let epoch = self.app_server.epoch;
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = call.send(&client).and_then(|ticket| ticket.wait());
            let _ = proxy.send_event(BrowserEvent::AppServer(Event::P2Reply {
                epoch,
                id,
                result,
            }));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rpc_rejection() -> CallError {
        CallError::Rpc(central_agent_codex_runtime::wire::RpcError {
            code: -32000,
            message: "private upstream detail".into(),
            data: None,
        })
    }

    fn begin_import(state: &mut State) -> String {
        state.sync_scope(Some(Path::new("C:\\project")));
        let scope = state.view.scope.id.clone().unwrap();
        let (detect, _) = state
            .action(
                Action::DetectImport {
                    scope_id: scope,
                    include_home: false,
                    include_project: true,
                },
                None,
            )
            .unwrap()
            .unwrap();
        state.reply(
            &detect,
            Ok(json!({"items":[{"itemType":"SKILLS","description":"One skill","cwd":"C:\\project","details":{}}],"connectors":[]})),
        );
        let item = state.view.migration.items[0].id.clone();
        let view = state.view.migration.view_id.clone().unwrap();
        state
            .action(
                Action::Import {
                    view_id: view,
                    item_ids: vec![item],
                },
                None,
            )
            .unwrap()
            .unwrap()
            .0
    }

    fn begin_command(state: &mut State) -> (String, String) {
        state.sync_scope(Some(Path::new("C:\\project")));
        let scope = state.view.scope.id.clone().unwrap();
        let requirements = json!({"allowedPermissionProfiles":null,"allowedSandboxModes":["read-only"],"allowedApprovalPolicies":["untrusted"]});
        let (exec, _) = state
            .action(
                Action::RunCommand {
                    scope_id: scope,
                    command: vec!["fixture".into()],
                    access: api::Access::ReadOnly,
                    rows: 24,
                    cols: 80,
                },
                Some(&requirements),
            )
            .unwrap()
            .unwrap();
        let process_id = state.view.command.process.as_ref().unwrap().id.clone();
        let (terminate, _) = state
            .action(Action::TerminateCommand { process_id }, None)
            .unwrap()
            .unwrap();
        (exec, terminate)
    }

    #[test]
    fn feature_inventory_is_atomic_and_under_development_is_read_only() {
        let mut state = State::default();
        state.sync_scope(Some(Path::new("C:\\project")));
        let (id, call) = state
            .action(Action::RefreshFeatures {}, None)
            .unwrap()
            .unwrap();
        assert_eq!(call.method, "experimentalFeature/list");
        let effect = state.reply(&id, Ok(json!({"data":[{"name":"future","stage":"underDevelopment","displayName":"Future","description":null,"announcement":null,"enabled":false,"defaultEnabled":false}],"nextCursor":null})));
        assert!(effect.follow_up.is_none());
        assert!(state.view.features.current);
        let view = state.view.features.view_id.clone().unwrap();
        assert!(
            state
                .action(
                    Action::SetFeature {
                        view_id: view,
                        name: "future".into(),
                        enabled: true
                    },
                    None
                )
                .is_err()
        );
    }

    #[test]
    fn migration_import_uses_opaque_exact_detected_items() {
        let mut state = State::default();
        state.sync_scope(Some(Path::new("C:\\project")));
        let scope = state.view.scope.id.clone().unwrap();
        let (id, _) = state
            .action(
                Action::DetectImport {
                    scope_id: scope,
                    include_home: false,
                    include_project: true,
                },
                None,
            )
            .unwrap()
            .unwrap();
        state.reply(&id, Ok(json!({"items":[{"itemType":"SKILLS","description":"One skill","cwd":"C:\\project","details":{"paths":["PRIVATE"]}}],"connectors":[]})));
        let item = state.view.migration.items[0].id.clone();
        let view = state.view.migration.view_id.clone().unwrap();
        let (import_call_id, call) = state
            .action(
                Action::Import {
                    view_id: view,
                    item_ids: vec![item],
                },
                None,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            call.params["migrationItems"][0]["details"]["paths"][0],
            "PRIVATE"
        );
        assert!(
            !serde_json::to_string(&state.view)
                .unwrap()
                .contains("PRIVATE")
        );
        state.reply(&import_call_id, Ok(json!({"importId":"import-fixture"})));
        state
            .notification(
                "externalAgentConfig/import/completed",
                &json!({"importId":"import-fixture","itemTypeResults":[{"itemType":"SKILLS","successes":[],"failures":[{"message":"PRIVATE_FAILURE","source":"C:\\private","target":"C:\\project"}]}]}),
            )
            .unwrap();
        let serialized = serde_json::to_string(&state.view).unwrap();
        assert!(!serialized.contains("PRIVATE_FAILURE"));
        assert_eq!(
            state.view.migration.active_import.as_ref().unwrap().results[0].failures,
            1
        );
        let (history_id, _) = state
            .action(Action::ReadImportHistory {}, None)
            .unwrap()
            .unwrap();
        state.reply(
            &history_id,
            Err(CallError::Rpc(
                central_agent_codex_runtime::wire::RpcError {
                    code: -32000,
                    message: "PRIVATE_SERVER_PATH".into(),
                    data: None,
                },
            )),
        );
        assert!(
            !serde_json::to_string(&state.view)
                .unwrap()
                .contains("PRIVATE_SERVER_PATH")
        );
    }

    #[test]
    fn accepted_import_stays_busy_until_completion_and_blocks_a_second_operation() {
        let mut state = State::default();
        let reply = begin_import(&mut state);
        assert!(state.busy());
        state.reply(&reply, Ok(json!({"importId":"import-active"})));
        assert!(state.busy());
        assert!(state.view.migration.busy);
        assert_eq!(
            state.view.migration.active_import.as_ref().unwrap().status,
            "accepted"
        );
        let scope = state.view.scope.id.clone().unwrap();
        assert!(
            state
                .action(
                    Action::DetectImport {
                        scope_id: scope,
                        include_home: true,
                        include_project: false,
                    },
                    None,
                )
                .is_err()
        );
        state
            .notification(
                "externalAgentConfig/import/progress",
                &json!({"importId":"import-active","itemTypeResults":[{"itemType":"SKILLS","successes":[],"failures":[]}]}),
            )
            .unwrap();
        assert!(state.busy());
        assert_eq!(
            state.view.migration.active_import.as_ref().unwrap().status,
            "in_progress"
        );
        state
            .notification(
                "externalAgentConfig/import/completed",
                &json!({"importId":"import-active","itemTypeResults":[{"itemType":"SKILLS","successes":[{}],"failures":[]}]}),
            )
            .unwrap();
        assert!(!state.busy());
        assert!(!state.view.migration.busy);
        assert_eq!(
            state.view.migration.active_import.as_ref().unwrap().status,
            "completed"
        );
    }

    #[test]
    fn early_import_completion_wins_over_late_progress_before_ack() {
        let mut state = State::default();
        let reply = begin_import(&mut state);
        state
            .notification(
                "externalAgentConfig/import/completed",
                &json!({"importId":"early","itemTypeResults":[{"itemType":"SKILLS","successes":[{}],"failures":[]}]}),
            )
            .unwrap();
        state
            .notification(
                "externalAgentConfig/import/progress",
                &json!({"importId":"early","itemTypeResults":[{"itemType":"SKILLS","successes":[],"failures":[{}]}]}),
            )
            .unwrap();
        state.reply(&reply, Ok(json!({"importId":"early"})));
        let import = state.view.migration.active_import.as_ref().unwrap();
        assert_eq!(import.status, "completed");
        assert_eq!(import.results[0].successes, 1);
        assert_eq!(import.results[0].failures, 0);
        assert!(!state.busy());
    }

    #[test]
    fn disconnect_after_import_ack_is_explicitly_uncertain() {
        let mut state = State::default();
        let reply = begin_import(&mut state);
        state.reply(&reply, Ok(json!({"importId":"accepted"})));
        state.disconnect();
        assert_eq!(
            state.view.migration.active_import.as_ref().unwrap().status,
            "uncertain"
        );
        assert!(
            state
                .view
                .migration
                .error
                .as_deref()
                .unwrap()
                .contains("refresh import history")
        );
        assert!(!state.busy());
    }

    #[test]
    fn home_only_detection_rejects_project_scoped_items() {
        let mut state = State::default();
        state.sync_scope(Some(Path::new("C:\\project")));
        let scope = state.view.scope.id.clone().unwrap();
        let (id, _) = state
            .action(
                Action::DetectImport {
                    scope_id: scope,
                    include_home: true,
                    include_project: false,
                },
                None,
            )
            .unwrap()
            .unwrap();
        state.reply(
            &id,
            Ok(json!({"items":[{"itemType":"CONFIG","description":"Unexpected project item","cwd":"C:\\project","details":null}],"connectors":[]})),
        );
        assert!(!state.view.migration.current);
        assert!(state.view.migration.items.is_empty());
        assert!(
            state
                .view
                .migration
                .error
                .as_deref()
                .unwrap()
                .contains("escaped")
        );
    }

    #[test]
    fn uncertain_reset_reuses_the_native_idempotency_key() {
        let mut state = State::default();
        let (id, first) = state
            .action(Action::ConsumeReset {}, None)
            .unwrap()
            .unwrap();
        state.reply(
            &id,
            Err(CallError::Disconnected {
                reason: "closed".into(),
                delivery_unknown: true,
            }),
        );
        let attempt = state.view.account_actions.reset_retry_id.clone().unwrap();
        let (_, retry) = state
            .action(
                Action::RetryReset {
                    attempt_id: attempt,
                },
                None,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            first.params["idempotencyKey"],
            retry.params["idempotencyKey"]
        );
    }

    #[test]
    fn malformed_reset_retains_the_attempt_and_known_rejection_is_terminal() {
        let mut uncertain = State::default();
        let (id, first) = uncertain
            .action(Action::ConsumeReset {}, None)
            .unwrap()
            .unwrap();
        uncertain.reply(&id, Ok(json!({"outcome":"future-value"})));
        assert_eq!(
            uncertain.view.account_actions.reset_status.as_deref(),
            Some("uncertain")
        );
        let attempt = uncertain
            .view
            .account_actions
            .reset_retry_id
            .clone()
            .unwrap();
        let (_, retry) = uncertain
            .action(
                Action::RetryReset {
                    attempt_id: attempt,
                },
                None,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            first.params["idempotencyKey"],
            retry.params["idempotencyKey"]
        );

        let mut failed = State::default();
        let (id, _) = failed
            .action(Action::ConsumeReset {}, None)
            .unwrap()
            .unwrap();
        failed.reply(&id, Err(rpc_rejection()));
        assert_eq!(
            failed.view.account_actions.reset_status.as_deref(),
            Some("failed")
        );
        assert!(failed.view.account_actions.reset_retry_id.is_none());
    }

    #[test]
    fn feedback_and_email_errors_never_leave_requesting_statuses() {
        let requirements = json!({"feedback":{"enabled":true}});
        let mut feedback = State::default();
        let (id, _) = feedback
            .action(
                Action::SubmitFeedback {
                    classification: "bug".into(),
                    reason: "Bounded report".into(),
                },
                Some(&requirements),
            )
            .unwrap()
            .unwrap();
        feedback.reply(&id, Err(rpc_rejection()));
        assert!(!feedback.view.feedback.busy);
        assert_eq!(feedback.view.feedback.status.as_deref(), Some("failed"));

        let mut malformed_feedback = State::default();
        let (id, _) = malformed_feedback
            .action(
                Action::SubmitFeedback {
                    classification: "bug".into(),
                    reason: "Bounded report".into(),
                },
                Some(&requirements),
            )
            .unwrap()
            .unwrap();
        malformed_feedback.reply(&id, Ok(json!({})));
        assert_eq!(
            malformed_feedback.view.feedback.status.as_deref(),
            Some("uncertain")
        );

        let mut email = State::default();
        let (id, _) = email
            .action(
                Action::SendNudge {
                    credit_type: AddCreditsNudgeCreditType::Credits,
                },
                None,
            )
            .unwrap()
            .unwrap();
        email.reply(&id, Err(rpc_rejection()));
        assert!(!email.view.account_actions.busy);
        assert_eq!(
            email.view.account_actions.nudge_status.as_deref(),
            Some("failed")
        );

        let mut malformed_email = State::default();
        let (id, _) = malformed_email
            .action(
                Action::SendNudge {
                    credit_type: AddCreditsNudgeCreditType::Credits,
                },
                None,
            )
            .unwrap()
            .unwrap();
        malformed_email.reply(&id, Ok(json!({"status":"future"})));
        assert_eq!(
            malformed_email.view.account_actions.nudge_status.as_deref(),
            Some("uncertain")
        );
    }

    #[test]
    fn command_is_argv_scoped_and_never_full_access() {
        let mut state = State::default();
        state.sync_scope(Some(Path::new("C:\\project")));
        let scope = state.view.scope.id.clone().unwrap();
        let requirements = json!({"allowedPermissionProfiles":null,"allowedSandboxModes":["read-only","workspace-write"],"allowedApprovalPolicies":["untrusted","on-request"]});
        let (_, call) = state
            .action(
                Action::RunCommand {
                    scope_id: scope.clone(),
                    command: vec!["cmd".into(), "/c".into(), "echo untouched & whoami".into()],
                    access: api::Access::ReadOnly,
                    rows: 24,
                    cols: 80,
                },
                Some(&requirements),
            )
            .unwrap()
            .unwrap();
        assert_eq!(call.params["command"][2], "echo untouched & whoami");
        assert!(call.params.get("env").is_none());
        assert!(
            state
                .action(
                    Action::RunCommand {
                        scope_id: scope,
                        command: vec!["cmd".into()],
                        access: api::Access::FullAccess,
                        rows: 24,
                        cols: 80
                    },
                    Some(&requirements)
                )
                .is_err()
        );
    }

    #[test]
    fn rejected_termination_restores_controls_without_reopening_a_completed_process() {
        let mut running = State::default();
        let (_, terminate) = begin_command(&mut running);
        running.reply(&terminate, Err(rpc_rejection()));
        assert_eq!(
            running.view.command.process.as_ref().unwrap().status,
            "running"
        );
        assert!(!running.view.command.control_busy);
        let process_id = running.view.command.process.as_ref().unwrap().id.clone();
        assert!(
            running
                .action(
                    Action::WriteCommand {
                        process_id,
                        input: "continue".into(),
                        close_stdin: false,
                    },
                    None,
                )
                .is_ok()
        );

        let mut completed = State::default();
        let (exec, terminate) = begin_command(&mut completed);
        completed.reply(&exec, Ok(json!({"exitCode":0,"stdout":"done","stderr":""})));
        completed.reply(&terminate, Err(rpc_rejection()));
        assert_eq!(
            completed.view.command.process.as_ref().unwrap().status,
            "completed"
        );
        let process_id = completed.view.command.process.as_ref().unwrap().id.clone();
        assert!(
            completed
                .action(Action::ClearCommand { process_id }, None)
                .is_ok()
        );
    }

    #[test]
    fn malformed_or_uncertain_termination_stays_active_until_completion_or_disconnect() {
        let mut malformed = State::default();
        let (exec, terminate) = begin_command(&mut malformed);
        malformed.reply(&terminate, Ok(json!({"unexpected":true})));
        assert_eq!(
            malformed.view.command.process.as_ref().unwrap().status,
            "termination_uncertain"
        );
        assert!(malformed.busy());
        let process_id = malformed.view.command.process.as_ref().unwrap().id.clone();
        assert!(
            malformed
                .action(
                    Action::ClearCommand {
                        process_id: process_id.clone(),
                    },
                    None,
                )
                .is_err()
        );
        malformed.reply(&exec, Ok(json!({"exitCode":0,"stdout":"","stderr":""})));
        assert_eq!(
            malformed.view.command.process.as_ref().unwrap().status,
            "completed"
        );

        let mut uncertain = State::default();
        let (_, terminate) = begin_command(&mut uncertain);
        uncertain.reply(
            &terminate,
            Err(CallError::Disconnected {
                reason: "closed".into(),
                delivery_unknown: true,
            }),
        );
        assert_eq!(
            uncertain.view.command.process.as_ref().unwrap().status,
            "termination_uncertain"
        );
        uncertain.disconnect();
        assert_eq!(
            uncertain.view.command.process.as_ref().unwrap().status,
            "connection_closed"
        );
    }

    #[test]
    fn project_change_invalidates_a_late_detection_reply() {
        let mut state = State::default();
        state.sync_scope(Some(Path::new("C:\\project-a")));
        let scope = state.view.scope.id.clone().unwrap();
        let (id, _) = state
            .action(
                Action::DetectImport {
                    scope_id: scope,
                    include_home: true,
                    include_project: false,
                },
                None,
            )
            .unwrap()
            .unwrap();
        state.sync_scope(Some(Path::new("C:\\project-b")));
        state.reply(
            &id,
            Ok(json!({"items":[{"itemType":"CONFIG","description":"Home config","cwd":null,"details":null}],"connectors":[]})),
        );
        assert!(!state.view.migration.current);
        assert!(state.view.migration.items.is_empty());
        assert!(
            state
                .view
                .migration
                .error
                .as_deref()
                .unwrap()
                .contains("changed")
        );
    }

    #[test]
    fn disconnect_never_replays_side_effects_and_marks_unknown_outcomes() {
        let mut state = State::default();
        assert!(
            state
                .action(
                    Action::SubmitFeedback {
                        classification: "bug".into(),
                        reason: "A bounded report".into(),
                    },
                    Some(&json!({"feedback":{"enabled":false}})),
                )
                .is_err()
        );
        let (id, _) = state
            .action(
                Action::SubmitFeedback {
                    classification: "bug".into(),
                    reason: "A bounded report".into(),
                },
                Some(&json!({"feedback":{"enabled":true}})),
            )
            .unwrap()
            .unwrap();
        assert!(state.pending.contains_key(&id));
        state.disconnect();
        assert!(state.pending.is_empty());
        assert_eq!(state.view.feedback.status.as_deref(), Some("uncertain"));
    }

    #[test]
    fn streamed_command_output_is_bounded_sanitized_and_hides_native_identity() {
        let mut state = State::default();
        state.sync_scope(Some(Path::new("C:\\project")));
        let scope = state.view.scope.id.clone().unwrap();
        let requirements = json!({"allowedPermissionProfiles":null,"allowedSandboxModes":["read-only"],"allowedApprovalPolicies":["untrusted"]});
        state
            .action(
                Action::RunCommand {
                    scope_id: scope,
                    command: vec!["fixture".into()],
                    access: api::Access::ReadOnly,
                    rows: 24,
                    cols: 80,
                },
                Some(&requirements),
            )
            .unwrap();
        let native_id = state.command.as_ref().unwrap().native_id.clone();
        state
            .notification(
                "command/exec/outputDelta",
                &json!({"processId":native_id,"stream":"stdout","deltaBase64":"QRtC","capReached":false}),
            )
            .unwrap();
        assert_eq!(
            state.view.command.process.as_ref().unwrap().output[0].text,
            "AB"
        );
        for index in 0..4100 {
            state
                .notification(
                    "command/exec/outputDelta",
                    &json!({"processId":native_id,"stream":if index % 2 == 0 {"stdout"} else {"stderr"},"deltaBase64":"eA==","capReached":false}),
                )
                .unwrap();
        }
        let process = state.view.command.process.as_ref().unwrap();
        assert!(process.output.len() <= 4096);
        assert!(process.truncated);
        assert!(
            !serde_json::to_string(&state.view)
                .unwrap()
                .contains(&native_id)
        );
    }
}
