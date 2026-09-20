//! Bounded, read-only account metadata. Raw account, limits, configuration and
//! notification payloads never cross into the WebView or local persistence.
use super::*;
use central_agent_codex_runtime::model_notices::{GlobalNotice, parse_global, push_global};

const MAX_INVENTORY: usize = 512;
const MAX_LABEL_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct View {
    pub(super) permission_profiles: PermissionProfilesView,
    pub(super) rate_limits: RateLimitsView,
    pub(super) provider_capabilities: ProviderCapabilitiesView,
    pub(super) account_usage: AccountUsageView,
    pub(super) workspace_messages: WorkspaceMessagesView,
    pub(super) runtime_notices: Vec<GlobalNotice>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProviderCapabilitiesView {
    pub(super) loaded: bool,
    pub(super) current: bool,
    pub(super) namespace_tools: bool,
    pub(super) image_generation: bool,
    pub(super) web_search: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AccountUsageView {
    pub(super) refreshing: bool,
    pub(super) loaded: bool,
    pub(super) current: bool,
    pub(super) summary: Option<AccountUsageSummary>,
    pub(super) daily_usage_buckets: Option<Vec<AccountUsageDay>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AccountUsageSummary {
    lifetime_tokens: Option<String>,
    peak_daily_tokens: Option<String>,
    longest_running_turn_sec: Option<String>,
    current_streak_days: Option<String>,
    longest_streak_days: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AccountUsageDay {
    start_date: String,
    tokens: String,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkspaceMessagesView {
    pub(super) refreshing: bool,
    pub(super) loaded: bool,
    pub(super) current: bool,
    pub(super) feature_enabled: bool,
    pub(super) messages: Vec<WorkspaceMessage>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkspaceMessage {
    message_id: String,
    message_type: String,
    message_body: String,
    created_at: Option<i64>,
    archived_at: Option<i64>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PermissionProfilesView {
    pub(super) loading: bool,
    pub(super) loaded: bool,
    pub(super) current: bool,
    pub(super) requires_named_profile: bool,
    pub(super) entries: Vec<PermissionProfile>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct PermissionProfile {
    id: String,
    description: Option<String>,
    allowed: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RateLimitsView {
    pub(super) refreshing: bool,
    pub(super) loaded: bool,
    pub(super) current: bool,
    pub(super) buckets: Vec<RateLimitBucket>,
    /// Decimal text preserves the generated bigint without exposing credit IDs.
    pub(super) available_reset_credits: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(super) struct RateLimitBucket {
    id: String,
    name: Option<String>,
    primary: Option<RateLimitWindow>,
    secondary: Option<RateLimitWindow>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(super) struct RateLimitWindow {
    used_percent: u8,
    window_duration_mins: Option<u64>,
    resets_at: Option<i64>,
}

#[derive(Default)]
pub(super) struct State {
    pub(super) view: View,
    permission_catalog: Vec<PermissionProfile>,
    permission_cursors: HashSet<String>,
    rate_refresh_pending: bool,
    notice_sequence: u64,
}

impl State {
    pub(super) fn reset_credit_available(&self) -> bool {
        self.view.rate_limits.current
            && self
                .view
                .rate_limits
                .available_reset_credits
                .as_deref()
                .and_then(|value| value.parse::<u64>().ok())
                .is_some_and(|count| count > 0)
    }

    pub(super) fn begin_refresh(&mut self) {
        self.view.permission_profiles.loading = true;
        self.view.permission_profiles.current = false;
        self.permission_catalog.clear();
        self.permission_cursors.clear();
        // A newer account refresh invalidates any in-flight rate read. Its host
        // reply carries the old revision and will be ignored; allow the new
        // authoritative account response to schedule a replacement read.
        self.rate_refresh_pending = false;
        self.view.rate_limits.refreshing = false;
        self.view.rate_limits.current = false;
        self.view.provider_capabilities.current = false;
        self.view.account_usage.refreshing = false;
        self.view.account_usage.current = false;
        self.view.workspace_messages.refreshing = false;
        self.view.workspace_messages.current = false;
    }

    pub(super) fn provider_capabilities(&mut self, value: &Value) -> Result<(), String> {
        let boolean = |key| {
            value
                .get(key)
                .and_then(Value::as_bool)
                .ok_or("Provider-capabilities response is incomplete")
        };
        self.view.provider_capabilities = ProviderCapabilitiesView {
            loaded: true,
            current: true,
            namespace_tools: boolean("namespaceTools")?,
            image_generation: boolean("imageGeneration")?,
            web_search: boolean("webSearch")?,
        };
        Ok(())
    }

    pub(super) fn provider_capabilities_failed(&mut self) {
        self.view.provider_capabilities.current = false;
    }

    pub(super) fn begin_account_extras(&mut self) {
        self.view.account_usage.refreshing = true;
        self.view.account_usage.current = false;
        self.view.workspace_messages.refreshing = true;
        self.view.workspace_messages.current = false;
    }

    pub(super) fn account_usage(&mut self, value: &Value) -> Result<(), String> {
        let summary = parse_usage_summary(
            value
                .get("summary")
                .ok_or("Account-usage response is incomplete")?,
        )?;
        let daily_usage_buckets = match value.get("dailyUsageBuckets") {
            Some(Value::Null) => None,
            Some(Value::Array(days)) if days.len() <= 400 => Some(
                days.iter()
                    .map(parse_usage_day)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            _ => return Err("Account daily-usage response is invalid".into()),
        };
        self.view.account_usage = AccountUsageView {
            refreshing: false,
            loaded: true,
            current: true,
            summary: Some(summary),
            daily_usage_buckets,
        };
        Ok(())
    }

    pub(super) fn account_usage_failed(&mut self) {
        self.view.account_usage.refreshing = false;
        self.view.account_usage.current = false;
    }

    pub(super) fn workspace_messages(&mut self, value: &Value) -> Result<(), String> {
        let feature_enabled = value
            .get("featureEnabled")
            .and_then(Value::as_bool)
            .ok_or("Workspace-message response is incomplete")?;
        let messages = value
            .get("messages")
            .and_then(Value::as_array)
            .filter(|messages| messages.len() <= 64)
            .ok_or("Workspace-message response is invalid")?
            .iter()
            .map(parse_workspace_message)
            .collect::<Result<Vec<_>, _>>()?;
        self.view.workspace_messages = WorkspaceMessagesView {
            refreshing: false,
            loaded: true,
            current: true,
            feature_enabled,
            messages,
        };
        Ok(())
    }

    pub(super) fn workspace_messages_failed(&mut self) {
        self.view.workspace_messages.refreshing = false;
        self.view.workspace_messages.current = false;
    }

    pub(super) fn clear_account_extras(&mut self) {
        self.view.account_usage = AccountUsageView::default();
        self.view.workspace_messages = WorkspaceMessagesView::default();
    }

    pub(super) fn requirements(&mut self, requirements: &Value) {
        self.view.permission_profiles.requires_named_profile = requirements
            .get("allowedPermissionProfiles")
            .is_some_and(|value| !value.is_null());
    }

    pub(super) fn permission_page(&mut self, value: Value) -> Result<Option<Call>, String> {
        let entries = value
            .get("data")
            .and_then(Value::as_array)
            .ok_or("Permission-profile response is incomplete")?;
        if self.permission_catalog.len().saturating_add(entries.len()) > MAX_INVENTORY {
            return Err("Permission-profile inventory is unexpectedly large".into());
        }
        for value in entries {
            let profile: PermissionProfile = serde_json::from_value(value.clone())
                .map_err(|_| "Invalid permission-profile entry")?;
            if !bounded(&profile.id)
                || profile
                    .description
                    .as_ref()
                    .is_some_and(|value| !bounded(value))
            {
                return Err("Invalid permission-profile entry".into());
            }
            self.permission_catalog.push(profile);
        }
        let cursor = next_cursor(&value)?;
        if let Some(cursor) = cursor {
            if !self.permission_cursors.insert(cursor.clone()) {
                self.permission_catalog.clear();
                return Err("Permission-profile pagination repeated a cursor".into());
            }
            return Ok(Some(api::permission_profiles(Some(&cursor))));
        }
        let mut ids = HashSet::new();
        if self
            .permission_catalog
            .iter()
            .any(|profile| !ids.insert(profile.id.clone()))
        {
            self.permission_catalog.clear();
            return Err("Permission-profile inventory contains a duplicated ID".into());
        }
        self.view.permission_profiles.entries = std::mem::take(&mut self.permission_catalog);
        self.view.permission_profiles.loading = false;
        self.view.permission_profiles.loaded = true;
        self.view.permission_profiles.current = true;
        Ok(None)
    }

    pub(super) fn permission_failed(&mut self) {
        self.permission_catalog.clear();
        self.permission_cursors.clear();
        self.view.permission_profiles.loading = false;
        self.view.permission_profiles.current = false;
    }

    pub(super) fn begin_rate_limits(&mut self) -> bool {
        if self.view.rate_limits.refreshing {
            return false;
        }
        self.view.rate_limits.refreshing = true;
        self.view.rate_limits.current = false;
        true
    }

    /// Mark a sparse notification as invalidation. When a read is already in
    /// flight, coalesce notifications into exactly one follow-up read so an
    /// older response cannot become the final authoritative snapshot.
    pub(super) fn invalidate_rate_limits(&mut self) -> bool {
        self.view.rate_limits.current = false;
        if self.view.rate_limits.refreshing {
            self.rate_refresh_pending = true;
            false
        } else {
            self.view.rate_limits.refreshing = true;
            true
        }
    }

    /// Returns whether an invalidation arrived during this read and therefore
    /// requires one more authoritative request.
    pub(super) fn rate_limits(&mut self, value: &Value) -> Result<bool, String> {
        self.view.rate_limits.buckets = parse_rate_limits(value)?;
        self.view.rate_limits.available_reset_credits = parse_reset_credit_count(value)?;
        self.view.rate_limits.loaded = true;
        if std::mem::take(&mut self.rate_refresh_pending) {
            self.view.rate_limits.refreshing = true;
            self.view.rate_limits.current = false;
            Ok(true)
        } else {
            self.view.rate_limits.refreshing = false;
            self.view.rate_limits.current = true;
            Ok(false)
        }
    }

    pub(super) fn rate_limits_failed(&mut self) -> bool {
        self.view.rate_limits.current = false;
        if std::mem::take(&mut self.rate_refresh_pending) {
            self.view.rate_limits.refreshing = true;
            true
        } else {
            self.view.rate_limits.refreshing = false;
            false
        }
    }

    pub(super) fn clear_rate_limits(&mut self) {
        self.rate_refresh_pending = false;
        self.view.rate_limits = RateLimitsView::default();
    }

    pub(super) fn notice(&mut self, method: &str, params: &Value) -> Result<bool, String> {
        let Some(value) = parse_global(method, params)? else {
            return Ok(false);
        };
        self.notice_sequence = self.notice_sequence.saturating_add(1);
        push_global(&mut self.view.runtime_notices, value, self.notice_sequence);
        Ok(true)
    }

    pub(super) fn disconnect(&mut self) {
        self.view.permission_profiles.loading = false;
        self.view.permission_profiles.current = false;
        self.rate_refresh_pending = false;
        self.view.rate_limits.refreshing = false;
        self.view.rate_limits.current = false;
        self.view.provider_capabilities.current = false;
        self.view.account_usage.refreshing = false;
        self.view.account_usage.current = false;
        self.view.workspace_messages.refreshing = false;
        self.view.workspace_messages.current = false;
        for notice in &mut self.view.runtime_notices {
            notice.current = false;
        }
    }
}

fn decimal(value: Option<&Value>, field: &str) -> Result<Option<String>, String> {
    match value {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value
            .as_u64()
            .map(|value| Some(value.to_string()))
            .ok_or_else(|| format!("Account-usage field {field} is invalid")),
    }
}

fn parse_usage_summary(value: &Value) -> Result<AccountUsageSummary, String> {
    if !value.is_object() {
        return Err("Account-usage summary is invalid".into());
    }
    Ok(AccountUsageSummary {
        lifetime_tokens: decimal(value.get("lifetimeTokens"), "lifetimeTokens")?,
        peak_daily_tokens: decimal(value.get("peakDailyTokens"), "peakDailyTokens")?,
        longest_running_turn_sec: decimal(
            value.get("longestRunningTurnSec"),
            "longestRunningTurnSec",
        )?,
        current_streak_days: decimal(value.get("currentStreakDays"), "currentStreakDays")?,
        longest_streak_days: decimal(value.get("longestStreakDays"), "longestStreakDays")?,
    })
}

fn parse_usage_day(value: &Value) -> Result<AccountUsageDay, String> {
    let start_date = value
        .get("startDate")
        .and_then(Value::as_str)
        .filter(|value| bounded(value) && value.len() <= 32)
        .ok_or("Account usage date is invalid")?;
    let tokens = decimal(value.get("tokens"), "tokens")?.ok_or("Account usage tokens missing")?;
    Ok(AccountUsageDay {
        start_date: start_date.into(),
        tokens,
    })
}

fn parse_workspace_message(value: &Value) -> Result<WorkspaceMessage, String> {
    let string = |key| {
        value
            .get(key)
            .and_then(Value::as_str)
            .filter(|text| bounded(text))
            .map(str::to_owned)
            .ok_or("Workspace message is invalid")
    };
    let message_type = string("messageType")?;
    if !matches!(
        message_type.as_str(),
        "headline" | "announcement" | "unknown"
    ) {
        return Err("Workspace message type is invalid".into());
    }
    let timestamp = |key| match value.get(key) {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value
            .as_i64()
            .map(Some)
            .ok_or("Workspace message timestamp is invalid"),
    };
    Ok(WorkspaceMessage {
        message_id: string("messageId")?,
        message_type,
        message_body: string("messageBody")?,
        created_at: timestamp("createdAt")?,
        archived_at: timestamp("archivedAt")?,
    })
}

fn next_cursor(value: &Value) -> Result<Option<String>, String> {
    match value.get("nextCursor") {
        Some(Value::Null) => Ok(None),
        Some(Value::String(cursor)) if bounded(cursor) => Ok(Some(cursor.clone())),
        _ => Err("Inventory pagination is incomplete".into()),
    }
}

fn parse_rate_limits(value: &Value) -> Result<Vec<RateLimitBucket>, String> {
    let mut buckets = Vec::new();
    match value.get("rateLimitsByLimitId") {
        Some(Value::Object(values)) if !values.is_empty() => {
            if values.len() > 64 {
                return Err("Rate-limit inventory is unexpectedly large".into());
            }
            for (id, snapshot) in values {
                buckets.push(parse_rate_limit_snapshot(snapshot, Some(id))?);
            }
        }
        Some(Value::Null) | None | Some(Value::Object(_)) => {
            buckets.push(parse_rate_limit_snapshot(
                value
                    .get("rateLimits")
                    .ok_or("Rate-limit response is incomplete")?,
                None,
            )?);
        }
        _ => return Err("Rate-limit response is invalid".into()),
    }
    Ok(buckets)
}

fn parse_reset_credit_count(value: &Value) -> Result<Option<String>, String> {
    match value.get("rateLimitResetCredits") {
        None | Some(Value::Null) => Ok(None),
        Some(summary) if summary.is_object() => summary
            .get("availableCount")
            .and_then(Value::as_u64)
            .map(|count| Some(count.to_string()))
            .ok_or_else(|| "Rate-limit reset-credit summary is invalid".into()),
        _ => Err("Rate-limit reset-credit summary is invalid".into()),
    }
}

fn parse_rate_limit_snapshot(
    snapshot: &Value,
    map_id: Option<&str>,
) -> Result<RateLimitBucket, String> {
    if !snapshot.is_object() {
        return Err("Rate-limit snapshot is invalid".into());
    }
    let id = map_id
        .map(str::to_owned)
        .or_else(|| {
            snapshot
                .get("limitId")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "default".into());
    if !bounded(&id) {
        return Err("Rate-limit identifier is invalid".into());
    }
    let name = optional_bounded(snapshot, "limitName")?;
    Ok(RateLimitBucket {
        id,
        name,
        primary: parse_window(snapshot.get("primary"))?,
        secondary: parse_window(snapshot.get("secondary"))?,
    })
}

fn parse_window(value: Option<&Value>) -> Result<Option<RateLimitWindow>, String> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let used_percent = value["usedPercent"]
        .as_u64()
        .filter(|value| *value <= 100)
        .map(|value| value as u8)
        .ok_or("Rate-limit percentage is invalid")?;
    let window_duration_mins = optional_u64(value, "windowDurationMins")?;
    let resets_at = match value.get("resetsAt") {
        Some(Value::Null) | None => None,
        Some(value) => Some(value.as_i64().ok_or("Rate-limit reset time is invalid")?),
    };
    Ok(Some(RateLimitWindow {
        used_percent,
        window_duration_mins,
        resets_at,
    }))
}

fn optional_u64(value: &Value, key: &str) -> Result<Option<u64>, String> {
    match value.get(key) {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("Rate-limit field {key} is invalid")),
    }
}

fn optional_bounded(value: &Value, key: &str) -> Result<Option<String>, String> {
    match value.get(key) {
        Some(Value::Null) | None => Ok(None),
        Some(Value::String(value)) if bounded(value) => Ok(Some(value.clone())),
        _ => Err(format!("Account field {key} is invalid")),
    }
}

fn bounded(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_LABEL_BYTES && !value.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_inventory_is_atomic_paginated_and_bounded() {
        let mut state = State::default();
        state.begin_refresh();
        let next = state
            .permission_page(json!({"data":[{"id":":read-only","description":"Read only","allowed":true,"private":"DROP"}],"nextCursor":"two"}))
            .unwrap()
            .unwrap();
        assert_eq!(next.method, "permissionProfile/list");
        assert!(state.view.permission_profiles.entries.is_empty());
        state
            .permission_page(json!({"data":[{"id":"managed","description":null,"allowed":false}],"nextCursor":null}))
            .unwrap();
        assert_eq!(state.view.permission_profiles.entries.len(), 2);
        assert!(state.view.permission_profiles.current);
        assert!(!serde_json::to_string(&state.view).unwrap().contains("DROP"));

        state.begin_refresh();
        assert!(
            state
                .permission_page(json!({"data":[
                {"id":"same","description":null,"allowed":true},
                {"id":"same","description":null,"allowed":false}
            ],"nextCursor":null}))
                .is_err()
        );
        assert!(state.view.permission_profiles.entries.len() == 2);
        assert!(!state.view.permission_profiles.current);
    }

    #[test]
    fn rate_limits_project_only_public_buckets_and_preserve_missing_values() {
        let mut state = State::default();
        assert!(state.begin_rate_limits());
        assert!(!state.begin_rate_limits());
        assert!(!state.invalidate_rate_limits());
        assert!(!state.invalidate_rate_limits());
        state.begin_refresh();
        assert!(!state.view.rate_limits.refreshing);
        assert!(state.begin_rate_limits());
        assert!(!state.begin_rate_limits());
        assert!(!state.invalidate_rate_limits());
        assert!(
            state
            .rate_limits(&json!({
                "rateLimits":{"limitId":null,"limitName":null,"primary":null,"secondary":null},
                "rateLimitsByLimitId":{"codex":{"limitId":"ignored-map-wins","limitName":"Codex","primary":{"usedPercent":31,"windowDurationMins":15,"resetsAt":1730948100},"secondary":null,"credits":{"balance":"PRIVATE"}}},
                "accountId":"PRIVATE_ACCOUNT","rateLimitUpsell":{"secret":"PRIVATE_BANNER"}
            }))
            .unwrap()
        );
        assert!(state.view.rate_limits.refreshing);
        assert!(!state.view.rate_limits.current);
        assert!(
            !state
                .rate_limits(&json!({
                    "rateLimits":{"limitId":null,"limitName":null,"primary":null,"secondary":null},
                    "rateLimitsByLimitId":{"codex":{"limitName":"Codex","primary":{"usedPercent":31,"windowDurationMins":15,"resetsAt":1730948100},"secondary":null}},
                    "rateLimitResetCredits":{"availableCount":2,"credits":[{"id":"PRIVATE_CREDIT"}]}
                }))
                .unwrap()
        );
        let value = serde_json::to_value(&state.view.rate_limits).unwrap();
        assert_eq!(value["buckets"][0]["id"], "codex");
        assert_eq!(value["buckets"][0]["primary"]["usedPercent"], 31);
        assert_eq!(value["availableResetCredits"], "2");
        assert!(!value.to_string().contains("PRIVATE"));
        assert!(state.view.rate_limits.current);
        assert!(!state.view.rate_limits.refreshing);
        assert!(state.reset_credit_available());
        assert!(state.invalidate_rate_limits());
        assert!(!state.reset_credit_available());
        assert!(!state.invalidate_rate_limits());
        assert!(state.rate_limits_failed());
        assert!(!state.rate_limits_failed());
    }

    #[test]
    fn global_notices_are_safe_bounded_and_connection_scoped() {
        let mut state = State::default();
        assert!(state
            .notice(
                "windows/worldWritableWarning",
                &json!({"samplePaths":["C:\\unsafe"],"extraCount":2,"failedScan":false,"token":"PRIVATE"})
            )
            .unwrap());
        assert!(state
            .notice(
                "configWarning",
                &json!({"summary":"Invalid setting","details":null,"path":"C:\\config.toml","range":{"start":{"line":3,"column":4},"end":{"line":3,"column":8}}})
            )
            .unwrap());
        assert_eq!(state.view.runtime_notices.len(), 2);
        assert!(
            !serde_json::to_string(&state.view)
                .unwrap()
                .contains("PRIVATE")
        );
        state.disconnect();
        assert!(
            state
                .view
                .runtime_notices
                .iter()
                .all(|notice| !notice.current)
        );
    }

    #[test]
    fn account_extras_are_atomic_bounded_and_drop_private_fields() {
        let mut state = State::default();
        state.begin_account_extras();
        state
            .account_usage(&json!({
                "summary":{
                    "lifetimeTokens":123456789012345_u64,
                    "peakDailyTokens":99,
                    "longestRunningTurnSec":null,
                    "currentStreakDays":4,
                    "longestStreakDays":8,
                    "privateSpend":"DROP"
                },
                "dailyUsageBuckets":[{"startDate":"2026-09-08","tokens":12,"billing":"DROP"}],
                "threadUsage":{"private":"DROP"}
            }))
            .unwrap();
        state
            .workspace_messages(&json!({
                "featureEnabled":true,
                "messages":[{
                    "messageId":"notice-a","messageType":"announcement",
                    "messageBody":"Maintenance window","createdAt":1788825600,
                    "archivedAt":null,"privateLink":"DROP"
                }]
            }))
            .unwrap();
        state
            .provider_capabilities(&json!({
                "namespaceTools":true,"imageGeneration":false,"webSearch":true,
                "privateProviderConfig":"DROP"
            }))
            .unwrap();
        let encoded = serde_json::to_string(&state.view).unwrap();
        assert!(!encoded.contains("DROP"));
        assert!(encoded.contains("123456789012345"));
        assert!(state.view.account_usage.current);
        assert!(state.view.workspace_messages.current);
        assert!(state.view.provider_capabilities.current);

        let previous = serde_json::to_string(&state.view.account_usage).unwrap();
        assert!(
            state
                .account_usage(&json!({
                    "summary":{
                        "lifetimeTokens":1,"peakDailyTokens":1,
                        "longestRunningTurnSec":1,"currentStreakDays":1,
                        "longestStreakDays":1
                    },
                    "dailyUsageBuckets":[{"startDate":"bad","tokens":"not-a-number"}]
                }))
                .is_err()
        );
        assert_eq!(
            serde_json::to_string(&state.view.account_usage).unwrap(),
            previous
        );
    }
}
