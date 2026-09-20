//! Small outbound adapters for the versioned generated contract. These do not
//! implement CLI commands or model behavior. `verify-app-server-contract.mjs`
//! validates the actual serialized calls against the official generated schema.
use crate::transport::{CallError, Client, Ticket};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize)]
pub struct Call {
    pub method: &'static str,
    pub params: Value,
}
impl Call {
    pub fn send(self, client: &Client) -> Result<Ticket, CallError> {
        client.request(self.method, self.params)
    }
}
fn call(method: &'static str, params: Value) -> Call {
    Call { method, params }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub service_tier: Option<String>,
    pub personality: Option<Personality>,
    #[serde(default)]
    pub summary: Option<ReasoningSummary>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Personality {
    None,
    Friendly,
    Pragmatic,
}

/// Optional fields supported by the stable thread/start contract. Instruction
/// and config values are internal host inputs; no untrusted WebView payload is
/// forwarded into this type.
#[derive(Clone, Debug, Default)]
pub struct ThreadStartOptions {
    pub model_provider: Option<String>,
    pub personality: Option<Personality>,
    pub ephemeral: Option<bool>,
    pub session_start_source: Option<ThreadStartSource>,
    pub thread_source: Option<String>,
    pub base_instructions: Option<String>,
    pub developer_instructions: Option<String>,
    pub config: Option<BTreeMap<String, Value>>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThreadStartSource {
    Startup,
    Clear,
}

#[derive(Clone, Debug, Default)]
pub struct ThreadResumeOptions {
    pub cwd: Option<String>,
    pub profile: Option<Profile>,
    pub access: Option<Access>,
    pub model_provider: Option<String>,
    pub personality: Option<Personality>,
    pub base_instructions: Option<String>,
    pub developer_instructions: Option<String>,
    pub config: Option<BTreeMap<String, Value>>,
    pub exclude_turns: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ThreadForkOptions {
    pub last_turn_id: Option<String>,
    pub cwd: Option<String>,
    pub profile: Option<Profile>,
    pub access: Option<Access>,
    pub model_provider: Option<String>,
    pub ephemeral: bool,
    pub thread_source: Option<String>,
    pub base_instructions: Option<String>,
    pub developer_instructions: Option<String>,
    pub config: Option<BTreeMap<String, Value>>,
    pub exclude_turns: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ThreadListOptions<'a> {
    pub cursor: Option<&'a str>,
    pub archived: bool,
    pub search: Option<&'a str>,
    pub model_providers: Vec<String>,
    pub cwd: Vec<String>,
    pub section_id: Option<Option<String>>,
    pub use_state_db_only: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnToolOutput {
    pub name: String,
    pub namespace: Option<String>,
    pub output: Value,
}

#[derive(Clone, Debug, Default)]
pub struct TurnOptions {
    pub turn_trigger: Option<String>,
    pub tool_output: Option<TurnToolOutput>,
    pub service_tier_for_turn: Option<String>,
    pub personality: Option<Personality>,
    pub output_schema: Option<Value>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitInfoUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<Option<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_url: Option<Option<String>>,
}

/// Public App Server summary preference, never private reasoning content.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningSummary {
    Auto,
    Concise,
    Detailed,
    None,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Access {
    #[default]
    ReadOnly,
    WorkspaceWrite,
    /// This can only follow explicit user consent in the host UI.
    FullAccess,
}
impl Access {
    /// The server remains authoritative; this only filters this client's presets.
    pub fn validate_requirements(self, requirements: Option<&Value>) -> Result<(), String> {
        let requirements = requirements.ok_or("Native managed requirements have not loaded")?;
        if !requirements["allowedPermissionProfiles"].is_null() {
            return Err("This installation requires named permission profiles; preset access modes cannot override them".into());
        }
        for (key, choice) in [
            ("allowedSandboxModes", self.sandbox()),
            ("allowedApprovalPolicies", self.approval()),
        ] {
            if let Some(allowed) = requirements.get(key).filter(|v| !v.is_null())
                && !allowed
                    .as_array()
                    .is_some_and(|items| items.iter().any(|v| v.as_str() == Some(choice)))
            {
                return Err(format!("Managed configuration does not allow {choice}"));
            }
        }
        Ok(())
    }
    fn sandbox(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::FullAccess => "danger-full-access",
        }
    }
    fn approval(self) -> &'static str {
        match self {
            Self::ReadOnly => "untrusted",
            Self::WorkspaceWrite => "on-request",
            Self::FullAccess => "never",
        }
    }
    fn turn_policy(self, cwd: &str) -> Value {
        match self {
            Self::ReadOnly => json!({"type":"readOnly","networkAccess":false}),
            Self::WorkspaceWrite => {
                json!({"type":"workspaceWrite","writableRoots":[cwd],"networkAccess":false,"excludeTmpdirEnvVar":true,"excludeSlashTmp":true})
            }
            Self::FullAccess => json!({"type":"dangerFullAccess"}),
        }
    }
}

pub fn account_read() -> Call {
    call("account/read", json!({"refreshToken":false}))
}

pub fn account_usage(thread_id: Option<&str>) -> Call {
    call(
        "account/usage/read",
        thread_id.map_or(Value::Null, |thread_id| json!({"threadId":thread_id})),
    )
}

pub fn workspace_messages() -> Call {
    call("account/workspaceMessages/read", Value::Null)
}

pub fn model_provider_capabilities() -> Call {
    call("modelProvider/capabilities/read", json!({}))
}

pub fn apps_installed(thread_id: Option<&str>, force_refresh: bool) -> Call {
    call(
        "app/installed",
        json!({"threadId":thread_id,"forceRefresh":force_refresh}),
    )
}

pub fn apps_list(cursor: Option<&str>, thread_id: Option<&str>, force_refetch: bool) -> Call {
    call(
        "app/list",
        json!({"cursor":cursor,"limit":50,"threadId":thread_id,"forceRefetch":force_refetch}),
    )
}

pub fn apps_read(
    app_ids: &[String],
    thread_id: Option<&str>,
    include_tools: bool,
) -> Result<Call, String> {
    if app_ids.is_empty() || app_ids.len() > 100 || app_ids.iter().any(|id| id.is_empty()) {
        return Err("Read between one and 100 nonempty app IDs".into());
    }
    Ok(call(
        "app/read",
        json!({"appIds":app_ids,"threadId":thread_id,"includeTools":include_tools}),
    ))
}

/// Read the installed Codex plugins used by composer mention surfaces.
///
/// A working directory includes repository-scoped marketplaces when the
/// current conversation belongs to a project. Installation and authentication
/// remain owned by Codex.
pub fn plugins_installed(cwd: Option<&str>) -> Call {
    call(
        "plugin/installed",
        json!({"cwds":cwd.map(|cwd| vec![cwd]),"installSuggestionPluginNames":Value::Null}),
    )
}

pub fn skills_list(cwd: &str) -> Call {
    call("skills/list", json!({"cwds":[cwd],"forceReload":true}))
}

pub fn skills_extra_roots(extra_roots: &[String]) -> Call {
    call("skills/extraRoots/set", json!({"extraRoots":extra_roots}))
}

pub fn hooks_list(cwds: &[String]) -> Call {
    call("hooks/list", json!({"cwds":cwds}))
}

pub fn skill_enabled(path: &str, enabled: bool) -> Call {
    call(
        "skills/config/write",
        json!({"path":path,"enabled":enabled}),
    )
}
pub fn mcp_status(cursor: Option<&str>) -> Call {
    call(
        "mcpServerStatus/list",
        json!({"cursor":cursor,"detail":"full"}),
    )
}
pub fn thread_mcp_status(thread_id: &str, cursor: Option<&str>) -> Call {
    call(
        "mcpServerStatus/list",
        json!({"threadId":thread_id,"cursor":cursor,"detail":"full"}),
    )
}
pub fn mcp_reload() -> Call {
    call("config/mcpServer/reload", Value::Null)
}
pub fn mcp_login(name: &str) -> Call {
    call("mcpServer/oauth/login", json!({"name":name}))
}
pub fn thread_mcp_login(thread: &str, name: &str) -> Call {
    call(
        "mcpServer/oauth/login",
        json!({"threadId":thread,"name":name}),
    )
}
pub fn mcp_resource_read(thread: Option<&str>, server: &str, uri: &str) -> Call {
    call(
        "mcpServer/resource/read",
        json!({"threadId":thread,"server":server,"uri":uri}),
    )
}
pub fn mcp_tool_call(thread: &str, server: &str, tool: &str, arguments: Value) -> Call {
    call(
        "mcpServer/tool/call",
        json!({"threadId":thread,"server":server,"tool":tool,"arguments":arguments}),
    )
}
pub fn login(device_code: bool) -> Call {
    call(
        "account/login/start",
        json!({"type":if device_code {"chatgptDeviceCode"} else {"chatgpt"}}),
    )
}
pub fn cancel_login(id: &str) -> Call {
    call("account/login/cancel", json!({"loginId":id}))
}
pub fn logout() -> Call {
    call("account/logout", Value::Null)
}
pub fn model_list(cursor: Option<&str>) -> Call {
    call(
        "model/list",
        json!({"cursor":cursor,"limit":50,"includeHidden":false}),
    )
}
pub fn requirements() -> Call {
    call("configRequirements/read", Value::Null)
}

/// Stable read-only inventory. Selecting a named profile on thread/start is a
/// separate experimental capability and is intentionally not constructed here.
pub fn permission_profiles(cursor: Option<&str>) -> Call {
    call(
        "permissionProfile/list",
        json!({"cursor":cursor,"limit":50}),
    )
}

pub fn config_read(cwd: &str) -> Call {
    call("config/read", json!({"cwd":cwd,"includeLayers":true}))
}

/// The target and opaque revision must come from a fresh native config/read.
/// Writes do not silently reload settings into existing conversations.
pub fn config_write(file: &str, version: &str, key: &str, value: Value) -> Call {
    call(
        "config/batchWrite",
        json!({"filePath":file,"expectedVersion":version,"reloadUserConfig":false,
            "edits":[{"keyPath":key,"value":value,"mergeStrategy":"upsert"}]}),
    )
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ReviewTarget {
    UncommittedChanges,
    BaseBranch { branch: String },
    Commit { sha: String, title: Option<String> },
    Custom { instructions: String },
}
impl ReviewTarget {
    pub fn validate(&self) -> Result<(), String> {
        let value = match self {
            Self::UncommittedChanges => return Ok(()),
            Self::BaseBranch { branch } => branch,
            Self::Commit { sha, .. } => sha,
            Self::Custom { instructions } => instructions,
        };
        if value.trim().is_empty() || value.contains('\0') {
            return Err("Specify a nonempty review target".into());
        }
        Ok(())
    }
}

/// Review has no cwd/policy overrides of its own. Prepare its native session
/// explicitly, without reconstructing history or changing global configuration.
pub fn prepare_review(thread: &str, cwd: &str) -> Call {
    call(
        "thread/resume",
        json!({"threadId":thread,"cwd":cwd,"sandbox":"read-only","approvalPolicy":"untrusted","approvalsReviewer":"user","excludeTurns":true}),
    )
}
pub fn start_review(thread: &str, target: &ReviewTarget) -> Call {
    start_review_with_delivery(thread, target, ReviewDelivery::Inline)
}

/// The runtime forks and starts a native review. The returned reviewThreadId,
/// not the source thread ID or any local chat ID, owns subsequent review work.
pub fn start_detached_review(thread: &str, target: &ReviewTarget) -> Call {
    start_review_with_delivery(thread, target, ReviewDelivery::Detached)
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReviewDelivery {
    #[default]
    Inline,
    Detached,
}

pub fn start_review_with_delivery(
    thread: &str,
    target: &ReviewTarget,
    delivery: ReviewDelivery,
) -> Call {
    call(
        "review/start",
        json!({"threadId":thread,"delivery":delivery,"target":target}),
    )
}

pub fn image_input(url: &str) -> Value {
    json!({"type":"image","url":url})
}
pub fn audio_input(url: &str) -> Value {
    json!({"type":"audio","url":url})
}
pub fn local_audio_input(path: &str) -> Value {
    json!({"type":"localAudio","path":path})
}
pub fn mention_input(name: &str, app_id: &str) -> Value {
    json!({"type":"mention","name":name,"path":format!("app://{app_id}")})
}
pub fn plugin_mention_input(name: &str, plugin_id: &str) -> Value {
    json!({"type":"mention","name":name,"path":format!("plugin://{plugin_id}")})
}
pub fn skill_input(name: &str, path: &str) -> Value {
    json!({"type":"skill","name":name,"path":path})
}
pub fn rate_limits() -> Call {
    call("account/rateLimits/read", Value::Null)
}

/// Stable process-wide feature inventory. This does not opt the connection in
/// to the experimental protocol surface.
pub fn experimental_features(cursor: Option<&str>) -> Call {
    call(
        "experimentalFeature/list",
        json!({"cursor":cursor,"limit":50,"threadId":Value::Null}),
    )
}

/// Update one named runtime feature after the host has shown its native
/// inventory entry and obtained explicit confirmation.
pub fn set_experimental_feature(name: &str, enabled: bool) -> Call {
    call(
        "experimentalFeature/enablement/set",
        json!({"enablement":{name:enabled}}),
    )
}

pub fn external_agent_detect(include_home: bool, cwds: &[String]) -> Call {
    call(
        "externalAgentConfig/detect",
        json!({"includeHome":include_home,"cwds":cwds,"maxSessionAgeDays":90,"maxSessions":200}),
    )
}

/// Migration items must be the exact opaque values returned by detection. The
/// WebView never constructs or edits them.
pub fn external_agent_import(migration_items: &[Value]) -> Call {
    call(
        "externalAgentConfig/import",
        json!({"migrationItems":migration_items,"source":"central-agent"}),
    )
}

pub fn external_agent_import_histories() -> Call {
    call("externalAgentConfig/import/readHistories", Value::Null)
}

pub fn consume_rate_limit_reset_credit(idempotency_key: &str) -> Call {
    call(
        "account/rateLimitResetCredit/consume",
        json!({"idempotencyKey":idempotency_key}),
    )
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AddCreditsNudgeCreditType {
    Credits,
    UsageLimit,
}

pub fn send_add_credits_nudge_email(credit_type: AddCreditsNudgeCreditType) -> Call {
    call(
        "account/sendAddCreditsNudgeEmail",
        json!({"creditType":credit_type}),
    )
}

/// P2 feedback is deliberately text-only. Logs, arbitrary files, tags and
/// conversation content are never attached implicitly.
pub fn feedback_upload(classification: &str, reason: Option<&str>) -> Call {
    call(
        "feedback/upload",
        json!({"classification":classification,"reason":reason,"threadId":Value::Null,
            "includeLogs":false,"extraLogFiles":Value::Null,"tags":Value::Null}),
    )
}

/// Execute an argv vector under the App Server sandbox. Full access is not a
/// supported preset for this separate utility and environment overrides are
/// intentionally absent.
pub fn command_exec(
    command: &[String],
    process_id: &str,
    cwd: &str,
    access: Access,
    rows: u16,
    cols: u16,
) -> Result<Call, String> {
    if command.is_empty() || command.len() > 64 {
        return Err("Provide between one and 64 command arguments".into());
    }
    if command
        .iter()
        .any(|part| part.is_empty() || part.len() > 4096 || part.contains('\0'))
        || command.iter().map(String::len).sum::<usize>() > 32 * 1024
    {
        return Err("Command arguments exceed the native utility limits".into());
    }
    if access == Access::FullAccess {
        return Err("The App Server command utility does not expose full access".into());
    }
    if !(5..=200).contains(&rows) || !(20..=500).contains(&cols) {
        return Err("Terminal size is outside the supported range".into());
    }
    Ok(call(
        "command/exec",
        json!({"command":command,"processId":process_id,"tty":true,"streamStdin":true,
            "streamStdoutStderr":true,"outputBytesCap":1048576,"timeoutMs":900000,
            "cwd":cwd,"size":{"rows":rows,"cols":cols},"sandboxPolicy":access.turn_policy(cwd)}),
    ))
}

pub fn command_write(process_id: &str, delta_base64: Option<&str>, close_stdin: bool) -> Call {
    call(
        "command/exec/write",
        json!({"processId":process_id,"deltaBase64":delta_base64,"closeStdin":close_stdin}),
    )
}

pub fn command_resize(process_id: &str, rows: u16, cols: u16) -> Call {
    call(
        "command/exec/resize",
        json!({"processId":process_id,"size":{"rows":rows,"cols":cols}}),
    )
}

pub fn command_terminate(process_id: &str) -> Call {
    call("command/exec/terminate", json!({"processId":process_id}))
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WindowsSandboxMode {
    Elevated,
    Unelevated,
}
pub fn setup_windows_sandbox(mode: WindowsSandboxMode) -> Call {
    call("windowsSandbox/setupStart", json!({"mode":mode}))
}

pub fn start_thread(cwd: &str, profile: &Profile, access: Access) -> Call {
    start_thread_with_options(cwd, profile, access, &ThreadStartOptions::default())
}
pub fn start_thread_with_options(
    cwd: &str,
    profile: &Profile,
    access: Access,
    options: &ThreadStartOptions,
) -> Call {
    let mut params = json!({"cwd":cwd,"model":profile.model,"serviceTier":profile.service_tier,
        "sandbox":access.sandbox(),"approvalPolicy":access.approval(),"approvalsReviewer":"user","serviceName":crate::CLIENT_NAME});
    optional(
        &mut params,
        "modelProvider",
        options.model_provider.as_ref(),
    );
    optional(
        &mut params,
        "personality",
        options
            .personality
            .as_ref()
            .or(profile.personality.as_ref()),
    );
    optional(&mut params, "ephemeral", options.ephemeral.as_ref());
    optional(
        &mut params,
        "sessionStartSource",
        options.session_start_source.as_ref(),
    );
    optional(&mut params, "threadSource", options.thread_source.as_ref());
    optional(
        &mut params,
        "baseInstructions",
        options.base_instructions.as_ref(),
    );
    optional(
        &mut params,
        "developerInstructions",
        options.developer_instructions.as_ref(),
    );
    optional(&mut params, "config", options.config.as_ref());
    call("thread/start", params)
}
pub fn resume_thread(thread_id: &str) -> Call {
    resume_thread_with_options(thread_id, &ThreadResumeOptions::default())
}
pub fn resume_thread_with_options(thread_id: &str, options: &ThreadResumeOptions) -> Call {
    let mut params = json!({"threadId":thread_id});
    optional(&mut params, "cwd", options.cwd.as_ref());
    if let Some(profile) = &options.profile {
        optional(&mut params, "model", profile.model.as_ref());
        optional(&mut params, "serviceTier", profile.service_tier.as_ref());
        optional(&mut params, "personality", profile.personality.as_ref());
    }
    if let Some(access) = options.access {
        params["sandbox"] = json!(access.sandbox());
        params["approvalPolicy"] = json!(access.approval());
        params["approvalsReviewer"] = json!("user");
    }
    optional(
        &mut params,
        "modelProvider",
        options.model_provider.as_ref(),
    );
    optional(&mut params, "personality", options.personality.as_ref());
    optional(
        &mut params,
        "baseInstructions",
        options.base_instructions.as_ref(),
    );
    optional(
        &mut params,
        "developerInstructions",
        options.developer_instructions.as_ref(),
    );
    optional(&mut params, "config", options.config.as_ref());
    if options.exclude_turns {
        params["excludeTurns"] = json!(true);
    }
    call("thread/resume", params)
}
pub fn read_thread(thread_id: &str) -> Call {
    call(
        "thread/read",
        json!({"threadId":thread_id,"includeTurns":true}),
    )
}
pub fn read_thread_metadata(thread_id: &str) -> Call {
    call(
        "thread/read",
        json!({"threadId":thread_id,"includeTurns":false}),
    )
}
pub fn list_thread_turns(thread_id: &str, cursor: Option<&str>) -> Call {
    list_thread_turns_view(thread_id, cursor, 16, false)
}
/// Full items are requested only for a single disclosed turn. Cursors always
/// come from native pagination, never from decoded or synthesized turn IDs.
pub fn list_thread_turns_view(
    thread_id: &str,
    cursor: Option<&str>,
    limit: u32,
    full: bool,
) -> Call {
    call(
        "thread/turns/list",
        json!({
            "threadId":thread_id,
            "cursor":cursor,
            "limit":limit,
            "sortDirection":"asc",
            "itemsView":if full { "full" } else { "summary" }
        }),
    )
}
pub fn loaded_threads(cursor: Option<&str>) -> Call {
    call("thread/loaded/list", json!({"cursor":cursor,"limit":200}))
}
pub fn unsubscribe_thread(thread_id: &str) -> Call {
    call("thread/unsubscribe", json!({"threadId":thread_id}))
}
pub fn list_threads(cursor: Option<&str>, archived: bool) -> Call {
    search_threads(cursor, archived, None)
}
pub fn search_threads(cursor: Option<&str>, archived: bool, search: Option<&str>) -> Call {
    search_threads_with_options(&ThreadListOptions {
        cursor,
        archived,
        search,
        ..ThreadListOptions::default()
    })
}
pub fn search_threads_with_options(options: &ThreadListOptions<'_>) -> Call {
    let mut params = json!({"cursor":options.cursor,"limit":50,"archived":options.archived,"searchTerm":options.search,
        "sourceKinds":["cli","vscode","appServer","exec","subAgent","subAgentReview","subAgentCompact","subAgentThreadSpawn","subAgentOther","unknown"],"sortKey":"updated_at"});
    if !options.model_providers.is_empty() {
        params["modelProviders"] = json!(options.model_providers);
    }
    if !options.cwd.is_empty() {
        params["cwd"] = json!(options.cwd);
    }
    if let Some(section_id) = &options.section_id {
        params["sectionId"] = json!(section_id);
    }
    if options.use_state_db_only {
        params["useStateDbOnly"] = json!(true);
    }
    call("thread/list", params)
}
pub fn rename_thread(thread_id: &str, name: &str) -> Call {
    call("thread/name/set", json!({"threadId":thread_id,"name":name}))
}
pub fn archive_thread(thread_id: &str) -> Call {
    call("thread/archive", json!({"threadId":thread_id}))
}
pub fn unarchive_thread(thread_id: &str) -> Call {
    call("thread/unarchive", json!({"threadId":thread_id}))
}
pub fn delete_thread(thread_id: &str) -> Call {
    call("thread/delete", json!({"threadId":thread_id}))
}
pub fn update_thread_git_info(thread_id: &str, git_info: &GitInfoUpdate) -> Call {
    call(
        "thread/metadata/update",
        json!({"threadId":thread_id,"gitInfo":git_info}),
    )
}
pub fn list_thread_sections(cursor: Option<&str>) -> Call {
    call("threadSection/list", json!({"cursor":cursor,"limit":64}))
}
pub fn create_thread_section(name: &str) -> Call {
    call("threadSection/create", json!({"name":name}))
}
pub fn rename_thread_section(section_id: &str, name: &str) -> Call {
    call(
        "threadSection/update",
        json!({"sectionId":section_id,"name":name}),
    )
}
pub fn delete_thread_section(section_id: &str) -> Call {
    call("threadSection/delete", json!({"sectionId":section_id}))
}
pub fn move_thread_section(
    thread_id: &str,
    section_id: Option<&str>,
    before_thread_id: Option<&str>,
) -> Call {
    call(
        "thread/section/move",
        json!({"threadId":thread_id,"sectionId":section_id,"beforeThreadId":before_thread_id}),
    )
}
pub fn revert_thread(thread_id: &str, before_turn_id: &str) -> Call {
    call(
        "thread/revert",
        json!({"threadId":thread_id,"beforeTurnId":before_turn_id}),
    )
}
/// Prepared only. Client::request refuses this method before writing any bytes.
pub fn list_thread_items(thread_id: &str, turn_id: Option<&str>, cursor: Option<&str>) -> Call {
    call(
        "thread/items/list",
        json!({"threadId":thread_id,"turnId":turn_id,"cursor":cursor,"limit":64,"sortDirection":"asc"}),
    )
}
pub fn fork_thread(thread_id: &str) -> Call {
    fork_thread_with_options(thread_id, &ThreadForkOptions::default())
}
pub fn fork_thread_with_options(thread_id: &str, options: &ThreadForkOptions) -> Call {
    let mut params = json!({"threadId":thread_id});
    optional(&mut params, "lastTurnId", options.last_turn_id.as_ref());
    optional(&mut params, "cwd", options.cwd.as_ref());
    if let Some(profile) = &options.profile {
        optional(&mut params, "model", profile.model.as_ref());
        optional(&mut params, "serviceTier", profile.service_tier.as_ref());
    }
    if let Some(access) = options.access {
        params["sandbox"] = json!(access.sandbox());
        params["approvalPolicy"] = json!(access.approval());
        params["approvalsReviewer"] = json!("user");
    }
    optional(
        &mut params,
        "modelProvider",
        options.model_provider.as_ref(),
    );
    optional(&mut params, "threadSource", options.thread_source.as_ref());
    optional(
        &mut params,
        "baseInstructions",
        options.base_instructions.as_ref(),
    );
    optional(
        &mut params,
        "developerInstructions",
        options.developer_instructions.as_ref(),
    );
    optional(&mut params, "config", options.config.as_ref());
    if options.ephemeral {
        params["ephemeral"] = json!(true);
    }
    if options.exclude_turns {
        params["excludeTurns"] = json!(true);
    }
    call("thread/fork", params)
}
pub fn compact_thread(thread_id: &str) -> Call {
    call("thread/compact/start", json!({"threadId":thread_id}))
}

pub fn text_input(text: &str) -> Value {
    json!({"type":"text","text":text,"text_elements":[]})
}
pub fn local_image_input(path: &str) -> Value {
    json!({"type":"localImage","path":path})
}
pub fn start_turn(
    thread_id: &str,
    message_id: &str,
    input: Vec<Value>,
    cwd: &str,
    profile: &Profile,
    access: Access,
) -> Call {
    start_turn_with_options(
        thread_id,
        message_id,
        input,
        cwd,
        profile,
        access,
        &TurnOptions::default(),
    )
}
pub fn start_turn_with_options(
    thread_id: &str,
    message_id: &str,
    input: Vec<Value>,
    cwd: &str,
    profile: &Profile,
    access: Access,
    options: &TurnOptions,
) -> Call {
    let mut request = call(
        "turn/start",
        json!({"threadId":thread_id,"clientUserMessageId":message_id,"input":input,
        "cwd":cwd,"model":profile.model,"effort":profile.effort,"serviceTier":profile.service_tier,
        "approvalPolicy":access.approval(),"approvalsReviewer":"user","sandboxPolicy":access.turn_policy(cwd)}),
    );
    if let Some(summary) = profile.summary {
        request.params["summary"] = json!(summary);
    }
    optional(
        &mut request.params,
        "turnTrigger",
        options.turn_trigger.as_ref(),
    );
    optional(
        &mut request.params,
        "toolOutput",
        options.tool_output.as_ref(),
    );
    optional(
        &mut request.params,
        "serviceTierForTurn",
        options.service_tier_for_turn.as_ref(),
    );
    optional(
        &mut request.params,
        "personality",
        options
            .personality
            .as_ref()
            .or(profile.personality.as_ref()),
    );
    optional(
        &mut request.params,
        "outputSchema",
        options.output_schema.as_ref(),
    );
    request
}

fn optional<T: Serialize>(params: &mut Value, key: &str, value: Option<&T>) {
    if let Some(value) = value {
        params[key] = json!(value);
    }
}
pub fn steer_turn(thread_id: &str, turn_id: &str, message_id: &str, input: Vec<Value>) -> Call {
    call(
        "turn/steer",
        json!({"threadId":thread_id,"expectedTurnId":turn_id,"clientUserMessageId":message_id,"input":input}),
    )
}
pub fn interrupt_turn(thread_id: &str, turn_id: &str) -> Call {
    call(
        "turn/interrupt",
        json!({"threadId":thread_id,"turnId":turn_id}),
    )
}

/// Offline samples generated by the same constructors used by the application.
pub fn contract_calls() -> Vec<Call> {
    let p = Profile {
        model: Some("fixture-model".into()),
        effort: Some("high".into()),
        service_tier: None,
        personality: Some(Personality::Pragmatic),
        summary: Some(ReasoningSummary::Auto),
    };
    let cwd = if cfg!(windows) {
        "C:\\fixture"
    } else {
        "/fixture"
    };
    let mut calls = vec![
        account_read(),
        account_usage(None),
        account_usage(Some("thread-a")),
        workspace_messages(),
        login(false),
        login(true),
        cancel_login("login-a"),
        logout(),
        model_list(None),
        model_list(Some("cursor")),
        model_provider_capabilities(),
        apps_installed(None, false),
        apps_installed(Some("thread-a"), true),
        apps_list(None, None, false),
        apps_list(Some("apps-page"), Some("thread-a"), true),
        apps_read(
            &["fixture-app".into(), "fixture-app-two".into()],
            Some("thread-a"),
            true,
        )
        .unwrap(),
        plugins_installed(None),
        plugins_installed(Some(cwd)),
        requirements(),
        permission_profiles(None),
        permission_profiles(Some("permission-page")),
        config_read(cwd),
        config_write(
            &format!("{cwd}/config.toml"),
            "opaque-version",
            "model_verbosity",
            json!("high"),
        ),
        config_write(
            &format!("{cwd}/config.toml"),
            "opaque-version",
            "model_verbosity",
            Value::Null,
        ),
        skills_list(cwd),
        skills_extra_roots(&[format!("{cwd}/extra-skills")]),
        hooks_list(&[cwd.into()]),
        config_write(
            &format!("{cwd}/config.toml"),
            "opaque-version",
            "model_auto_compact_token_limit_scope",
            json!("body_after_prefix"),
        ),
        skill_enabled(&format!("{cwd}/.agents/skills/example/SKILL.md"), false),
        mcp_status(None),
        mcp_status(Some("mcp-page")),
        thread_mcp_status("thread-fixture", None),
        thread_mcp_status("thread-fixture", Some("mcp-page")),
        mcp_reload(),
        mcp_login("fixture-server"),
        thread_mcp_login("thread-fixture", "fixture-server"),
        mcp_resource_read(
            Some("thread-fixture"),
            "fixture-server",
            "fixture://resource",
        ),
        mcp_tool_call(
            "thread-fixture",
            "fixture-server",
            "lookup",
            json!({"query":"fixture"}),
        ),
        rate_limits(),
        experimental_features(None),
        experimental_features(Some("feature-page")),
        set_experimental_feature("fixture_feature", true),
        external_agent_detect(true, &[cwd.into()]),
        external_agent_import(&[
            json!({"itemType":"SKILLS","description":"Fixture skill","cwd":cwd,"details":null}),
        ]),
        external_agent_import_histories(),
        consume_rate_limit_reset_credit("00000000-0000-4000-8000-000000000001"),
        send_add_credits_nudge_email(AddCreditsNudgeCreditType::Credits),
        send_add_credits_nudge_email(AddCreditsNudgeCreditType::UsageLimit),
        feedback_upload("bug", Some("Fixture feedback")),
        command_exec(
            &["fixture".into(), "--version".into()],
            "00000000-0000-4000-8000-000000000002",
            cwd,
            Access::ReadOnly,
            24,
            80,
        )
        .unwrap(),
        command_write(
            "00000000-0000-4000-8000-000000000002",
            Some("Zml4dHVyZQo="),
            false,
        ),
        command_resize("00000000-0000-4000-8000-000000000002", 30, 100),
        command_terminate("00000000-0000-4000-8000-000000000002"),
        setup_windows_sandbox(WindowsSandboxMode::Elevated),
        setup_windows_sandbox(WindowsSandboxMode::Unelevated),
        resume_thread("thread-a"),
        prepare_review("thread-a", cwd),
        start_review("thread-a", &ReviewTarget::UncommittedChanges),
        start_detached_review("thread-a", &ReviewTarget::UncommittedChanges),
        start_review(
            "thread-a",
            &ReviewTarget::BaseBranch {
                branch: "main".into(),
            },
        ),
        start_review(
            "thread-a",
            &ReviewTarget::Commit {
                sha: "abc123".into(),
                title: None,
            },
        ),
        start_review(
            "thread-a",
            &ReviewTarget::Custom {
                instructions: "Check error handling".into(),
            },
        ),
        read_thread("thread-a"),
        read_thread_metadata("thread-a"),
        list_thread_turns("thread-a", None),
        list_thread_turns("thread-a", Some("turn-page")),
        loaded_threads(None),
        loaded_threads(Some("loaded-page")),
        unsubscribe_thread("thread-a"),
        list_threads(None, false),
        search_threads(Some("older-history"), true, Some("Project")),
        rename_thread("thread-a", "Example"),
        archive_thread("thread-a"),
        unarchive_thread("thread-a"),
        delete_thread("thread-a"),
        list_thread_sections(None),
        list_thread_sections(Some("next-section")),
        create_thread_section("Work"),
        rename_thread_section("section-a", "Review"),
        delete_thread_section("section-a"),
        move_thread_section("thread-a", Some("section-a"), Some("thread-b")),
        move_thread_section("thread-a", None, None),
        revert_thread("thread-a", "turn-a"),
        list_thread_items("thread-a", None, None),
        list_thread_items("thread-a", Some("turn-a"), Some("next-item")),
        update_thread_git_info(
            "thread-a",
            &GitInfoUpdate {
                branch: Some(Some("main".into())),
                sha: Some(None),
                origin_url: None,
            },
        ),
        fork_thread("thread-a"),
        fork_thread_with_options(
            "thread-a",
            &ThreadForkOptions {
                last_turn_id: Some("turn-a".into()),
                cwd: Some(cwd.into()),
                profile: Some(p.clone()),
                access: Some(Access::ReadOnly),
                model_provider: Some("openai".into()),
                ephemeral: true,
                thread_source: Some("central-agent-fork".into()),
                base_instructions: Some("Fixture base".into()),
                developer_instructions: Some("Fixture developer".into()),
                config: Some(BTreeMap::from([("model_verbosity".into(), json!("high"))])),
                exclude_turns: true,
            },
        ),
        compact_thread("thread-a"),
        crate::goals::get("thread-a"),
        crate::goals::clear("thread-a"),
        crate::goals::Edit::Replace {
            objective: "Native example goal".into(),
            status: crate::goals::UserStatus::Paused,
            token_budget: None,
        }
        .call("thread-a")
        .unwrap(),
        crate::goals::Edit::Status {
            status: crate::goals::UserStatus::Active,
        }
        .call("thread-a")
        .unwrap(),
        crate::goals::Edit::Budget {
            token_budget: Some(1000),
        }
        .call("thread-a")
        .unwrap(),
        crate::goals::Edit::Budget { token_budget: None }
            .call("thread-a")
            .unwrap(),
        steer_turn(
            "thread-a",
            "turn-a",
            "followup-a",
            vec![
                text_input("follow-up"),
                text_input("$fixture-skill"),
                skill_input(
                    "fixture-skill",
                    &format!("{cwd}/.agents/skills/fixture/SKILL.md"),
                ),
                image_input("data:image/png;base64,iVBORw0KGgo="),
            ],
        ),
        interrupt_turn("thread-a", "turn-a"),
        resume_thread_with_options(
            "thread-a",
            &ThreadResumeOptions {
                cwd: Some(cwd.into()),
                profile: Some(p.clone()),
                access: Some(Access::ReadOnly),
                model_provider: Some("openai".into()),
                personality: Some(Personality::Pragmatic),
                base_instructions: Some("Fixture base".into()),
                developer_instructions: Some("Fixture developer".into()),
                config: Some(BTreeMap::from([("model_verbosity".into(), json!("high"))])),
                exclude_turns: true,
            },
        ),
        search_threads_with_options(&ThreadListOptions {
            cursor: None,
            archived: false,
            search: Some("fixture"),
            model_providers: vec!["openai".into()],
            cwd: vec![cwd.into()],
            section_id: Some(None),
            use_state_db_only: true,
        }),
    ];
    for access in [Access::ReadOnly, Access::WorkspaceWrite, Access::FullAccess] {
        calls.push(start_thread(cwd, &p, access));
        calls.push(start_thread_with_options(
            cwd,
            &p,
            access,
            &ThreadStartOptions {
                model_provider: Some("openai".into()),
                personality: Some(Personality::Friendly),
                ephemeral: Some(true),
                session_start_source: Some(ThreadStartSource::Startup),
                thread_source: Some("central-agent".into()),
                base_instructions: Some("Fixture base".into()),
                developer_instructions: Some("Fixture developer".into()),
                config: Some(BTreeMap::from([("model_verbosity".into(), json!("high"))])),
            },
        ));
        calls.push(start_turn(
            "thread-a",
            "message-a",
            vec![
                text_input("fixture"),
                text_input("$fixture-skill"),
                skill_input(
                    "fixture-skill",
                    &format!("{cwd}/.agents/skills/fixture/SKILL.md"),
                ),
                local_image_input(&format!("{cwd}/image.png")),
                image_input("data:image/jpeg;base64,/9j/"),
            ],
            cwd,
            &p,
            access,
        ));
        calls.push(start_turn_with_options(
            "thread-a",
            "message-structured",
            vec![
                text_input("$fixture-app summarize"),
                mention_input("fixture-app", "fixture-app"),
                audio_input("data:audio/wav;base64,UklGRg=="),
                local_audio_input(&format!("{cwd}/audio.wav")),
            ],
            cwd,
            &p,
            access,
            &TurnOptions {
                turn_trigger: Some("central-agent-composer".into()),
                service_tier_for_turn: Some("default".into()),
                personality: Some(Personality::Pragmatic),
                output_schema: Some(json!({"type":"object","properties":{"answer":{"type":"string"}},"required":["answer"],"additionalProperties":false})),
                tool_output: None,
            },
        ));
        calls.push(start_turn_with_options(
            "thread-a",
            "message-tool-output",
            vec![],
            cwd,
            &p,
            access,
            &TurnOptions {
                turn_trigger: Some("central-agent-host-tool".into()),
                tool_output: Some(TurnToolOutput {
                    name: "fixture_tool".into(),
                    namespace: Some("central_agent".into()),
                    output: json!("fixture result"),
                }),
                ..TurnOptions::default()
            },
        ));
    }
    // Exercise real validated MCP edit constructors, not hand-written copies
    // of their outbound messages, against the generated stable request schema.
    use crate::mcp_configuration::{Edit, OptionKey, Snapshot, Transport};
    let snapshot = Snapshot::read(&json!({
        "config":{"mcp_servers":{"existing":{"command":"fixture","enabled":false}}},
        "origins":{},"layers":[{"name":{"type":"user","profile":null,"file":format!("{cwd}/config.toml")},
        "version":"opaque-version","disabledReason":null,
        "config":{"mcp_servers":{"existing":{"command":"fixture","enabled":false,"args":[]},"http":{"url":"https://example.invalid","env_http_headers":{"X-Old":"OLD_TOKEN"}}}}}]
    })).unwrap();
    for edit in [
        Edit::Add {
            name: "stdio-fixture".into(),
            transport: Transport::Stdio {
                command: "fixture".into(),
                args: vec!["--stdio".into()],
                cwd: Some(cwd.into()),
                env_vars: vec!["MCP_TOKEN".into()],
            },
        },
        Edit::Add {
            name: "http-fixture".into(),
            transport: Transport::Http {
                url: "https://example.invalid/mcp".into(),
                bearer_token_env_var: Some("MCP_TOKEN".into()),
            },
        },
        Edit::SetEnabled {
            name: "existing".into(),
            enabled: true,
        },
        Edit::SetEnabled {
            name: "existing".into(),
            enabled: false,
        },
        Edit::Remove {
            name: "existing".into(),
        },
        Edit::SetOption {
            name: "existing".into(),
            key: OptionKey::Args,
            value: json!(["--changed"]),
        },
        Edit::ClearOption {
            name: "existing".into(),
            key: OptionKey::Args,
        },
        Edit::SetOption {
            name: "http".into(),
            key: OptionKey::EnvHttpHeaders,
            value: json!({"X-New":"NEW_TOKEN"}),
        },
    ] {
        calls.push(snapshot.edit(&edit).unwrap());
    }
    calls
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn review_targets_are_native_values_not_commands_or_synthetic_prompts() {
        for target in [
            ReviewTarget::BaseBranch { branch: " ".into() },
            ReviewTarget::Commit {
                sha: "".into(),
                title: None,
            },
            ReviewTarget::Custom {
                instructions: "a\0b".into(),
            },
        ] {
            assert!(target.validate().is_err());
        }
        let target = ReviewTarget::BaseBranch {
            branch: "branch with spaces; echo untouched".into(),
        };
        target.validate().unwrap();
        let call = start_review("native", &target);
        assert_eq!(call.method, "review/start");
        assert_eq!(
            call.params["target"]["branch"],
            "branch with spaces; echo untouched"
        );
        assert!(call.params.get("input").is_none());
        let detached = start_detached_review("native", &target);
        assert_eq!(detached.method, "review/start");
        assert_eq!(detached.params["delivery"], "detached");
        assert_eq!(detached.params["target"], call.params["target"]);
        assert!(detached.params.get("input").is_none());
        assert!(
            serde_json::from_value::<ReviewTarget>(
                json!({"type":"custom","instructions":"review","command":"ignored"})
            )
            .is_err()
        );
    }
    #[test]
    fn presets_respect_generated_managed_allowlists_without_fallback() {
        assert!(Access::ReadOnly.validate_requirements(None).is_err());
        assert!(
            Access::FullAccess
                .validate_requirements(Some(&Value::Null))
                .is_ok()
        );
        let requirements = json!({"allowedSandboxModes":["read-only","workspace-write"],"allowedApprovalPolicies":["on-request"]});
        assert!(
            Access::WorkspaceWrite
                .validate_requirements(Some(&requirements))
                .is_ok()
        );
        assert!(
            Access::ReadOnly
                .validate_requirements(Some(&requirements))
                .is_err()
        );
        assert!(
            Access::FullAccess
                .validate_requirements(Some(&requirements))
                .is_err()
        );
        assert!(
            Access::WorkspaceWrite
                .validate_requirements(Some(&json!({"allowedPermissionProfiles":{"managed":true}})))
                .is_err()
        );
        assert!(
            Access::WorkspaceWrite
                .validate_requirements(Some(&json!({"allowedSandboxModes":"invalid"})))
                .is_err()
        );
    }
    #[test]
    fn resume_preserves_official_history_and_steer_does_not_override_profile() {
        assert_eq!(resume_thread("t").params, json!({"threadId":"t"}));
        assert_eq!(
            prepare_review("t", "C:\\fixture").params["excludeTurns"],
            true
        );
        assert_eq!(
            read_thread_metadata("t").params,
            json!({"threadId":"t","includeTurns":false})
        );
        assert_eq!(
            list_thread_turns("t", Some("next")).params,
            json!({
                "threadId":"t",
                "cursor":"next",
                "limit":16,
                "sortDirection":"asc",
                "itemsView":"summary"
            })
        );
        let steer = steer_turn("t", "active-turn", "followup", vec![text_input("next")]);
        assert_eq!(steer.params["expectedTurnId"], "active-turn");
        assert_eq!(steer.params["clientUserMessageId"], "followup");
        assert!(steer.params.get("model").is_none());
        assert!(steer.params.get("cwd").is_none());
    }
    #[test]
    fn default_access_is_not_an_unsandboxed_fallback() {
        let c = start_thread("C:\\fixture", &Profile::default(), Access::default());
        assert_eq!(c.params["sandbox"], "read-only");
        assert_eq!(c.params["approvalPolicy"], "untrusted");
        assert!(c.params.get("baseInstructions").is_none());
        assert!(c.params.get("developerInstructions").is_none());
        assert!(c.params.get("dynamicTools").is_none());
        assert_eq!(c.params["serviceName"], crate::CLIENT_NAME);
    }

    #[test]
    fn stable_p0_inventory_and_unsubscribe_calls_do_not_enable_profile_selection() {
        assert_eq!(
            permission_profiles(Some("next")).params,
            json!({"cursor":"next","limit":50})
        );
        assert_eq!(
            loaded_threads(Some("next")).params,
            json!({"cursor":"next","limit":200})
        );
        assert_eq!(
            unsubscribe_thread("thread").params,
            json!({"threadId":"thread"})
        );
        let start = start_thread("C:\\fixture", &Profile::default(), Access::ReadOnly);
        assert!(start.params.get("permissions").is_none());
    }

    #[test]
    fn turns_inherit_native_reasoning_summary_without_an_implicit_override() {
        for access in [Access::ReadOnly, Access::WorkspaceWrite, Access::FullAccess] {
            let call = start_turn(
                "thread",
                "message",
                vec![text_input("input")],
                "C:\\fixture",
                &Profile::default(),
                access,
            );
            assert!(call.params.get("summary").is_none());
            assert_eq!(call.params["approvalPolicy"], access.approval());
            assert_eq!(
                call.params["sandboxPolicy"],
                access.turn_policy("C:\\fixture")
            );
        }
    }

    #[test]
    fn explicit_summary_is_only_a_turn_setting_and_never_changes_authority() {
        for summary in [
            ReasoningSummary::Auto,
            ReasoningSummary::Concise,
            ReasoningSummary::Detailed,
            ReasoningSummary::None,
        ] {
            let profile = Profile {
                summary: Some(summary),
                ..Default::default()
            };
            let request = start_turn(
                "thread",
                "message",
                vec![text_input("input")],
                "C:\\fixture",
                &profile,
                Access::ReadOnly,
            );
            assert_eq!(request.params["summary"], json!(summary));
            assert_eq!(request.params["approvalPolicy"], "untrusted");
            assert!(
                start_thread("C:\\fixture", &profile, Access::ReadOnly)
                    .params
                    .get("summary")
                    .is_none()
            );
            assert!(
                steer_turn("thread", "turn", "followup", vec![text_input("input")])
                    .params
                    .get("summary")
                    .is_none()
            );
        }
        assert!(serde_json::from_value::<ReasoningSummary>(json!("private")).is_err());
    }
}
