//! Official App Server connection and account control. No inference loop, CLI
//! emulation, credential file access, or transcript reconstruction belongs here.
use super::*;
use central_agent_codex_runtime::{
    api::{self, Call},
    cloud::TaskPage as CloudTaskPage,
    conversations::{
        Action as ThreadAction, Conversations, Outcome, Request as ThreadRequest, Saved,
    },
    runtime::Runtime,
    transport::{CallError, Client, Event as TransportEvent},
    wire::RpcError,
};
use std::collections::BTreeMap;

fn display_local_path(value: &str) -> String {
    #[cfg(windows)]
    {
        if let Some(rest) = value.strip_prefix("\\\\?\\UNC\\") {
            return format!("\\\\{rest}");
        }
        if let Some(rest) = value.strip_prefix("\\\\?\\") {
            return rest.to_owned();
        }
    }
    value.to_owned()
}

fn display_path(path: &Path) -> String {
    display_local_path(&path.display().to_string())
}

fn same_local_path(a: &Path, b: &Path) -> bool {
    a == b
        || fs::canonicalize(a)
            .ok()
            .zip(fs::canonicalize(b).ok())
            .is_some_and(|(a, b)| a == b)
}
mod access;
mod account_state;
mod apps;
pub(crate) use apps::{AppsAction, SelectedApp};
mod attachments;
mod branches;
mod chat_reload;
mod computer_use;
mod connection;
mod delegation;
pub(crate) mod delivery;
mod diagnostics;
mod event_log;
mod goals;
mod history;
mod history_pages;
mod hooks;
mod native_profile;
mod pending_input;
mod preparation;
mod skills;
mod work_history;
pub(crate) use skills::{SelectedSkill, SkillsAction};
mod preferences;
pub(crate) use preferences::PreferencesAction;
mod lifecycle;
pub(crate) use history::HistoryAction;
mod mcp;
mod thread_mcp;
pub(crate) use mcp::McpAction;
mod p2;
pub(crate) use p2::Action as P2Action;
mod steering;
mod usage;
pub(crate) use steering::SteerInput;
mod presentation;

pub(super) fn work_reference_messages() -> Vec<Value> {
    presentation::work_reference_messages()
}
pub(crate) use access::AccessAction;
pub(crate) mod acceptance;
mod profile;
mod requests;
mod review;
mod turns;
pub(crate) use requests::RequestAction;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AccountAction {
    Connect,
    Refresh,
    ReloadChats,
    LoginBrowser,
    LoginDevice,
    OpenLogin,
    CancelLogin,
    LogoutConfirmed,
    SetupSandboxElevated,
    SetupSandboxUnelevated,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ConversationAction {
    HooksStatus {
        expected_thread_id: String,
        expected_directory: String,
    },
    AppsControl {
        expected_thread_id: Option<String>,
        action: AppsAction,
    },
    Goal {
        expected_thread_id: String,
        expected_directory: String,
        action: goals::Action,
    },
    McpControl {
        expected_thread_id: String,
        action: McpAction,
    },
    McpStatus {
        expected_thread_id: String,
    },
    Compact {
        expected_thread_id: String,
    },
    ResetUnused {
        expected_thread_id: String,
    },
    StartReview {
        expected_thread_id: String,
        expected_directory: String,
        target: api::ReviewTarget,
        #[serde(default)]
        delivery: api::ReviewDelivery,
        #[serde(default)]
        destination_name: Option<String>,
    },
    Open {},
    Read {},
    ReadWork {
        expected_thread_id: String,
        turn_id: String,
    },
    Rename {
        name: String,
        expected_thread_id: String,
    },
    Archive {
        expected_thread_id: String,
    },
    Unarchive {
        expected_thread_id: String,
    },
    Delete {
        expected_thread_id: String,
    },
    NewFork {
        name: String,
        expected_thread_id: String,
        expected_directory: String,
        #[serde(default)]
        last_turn_id: Option<String>,
    },
    OpenBranch {
        destination: String,
    },
    OpenDelegate {
        expected_thread_id: String,
        destination: String,
    },
    EventLog {
        expected_thread_id: String,
    },
    ReviewDelivery {
        message_id: String,
    },
}

pub(crate) enum Event {
    WorkHistoryReply {
        epoch: u64,
        owner: String,
        thread: String,
        turn: String,
        request: u64,
        result: Result<Value, String>,
    },
    UnsubscribeReply {
        epoch: u64,
        thread: String,
        result: Result<Value, CallError>,
    },
    GoalReply {
        epoch: u64,
        owner: String,
        thread: String,
        id: String,
        result: Result<Value, CallError>,
    },
    ThreadMcpReply {
        epoch: u64,
        owner: String,
        thread: String,
        id: String,
        result: Result<Value, CallError>,
    },
    AppsReply {
        epoch: u64,
        owner: String,
        thread: String,
        id: String,
        result: Result<Value, CallError>,
    },
    HooksReply {
        epoch: u64,
        owner: String,
        thread: String,
        id: String,
        result: Result<Value, CallError>,
    },
    PreferencesReply {
        epoch: u64,
        owner: String,
        id: String,
        result: Result<Value, CallError>,
    },
    SkillsReply {
        epoch: u64,
        owner: String,
        id: String,
        result: Result<Value, CallError>,
    },
    HistoryReply {
        epoch: u64,
        owner: String,
        id: String,
        result: Result<Value, CallError>,
    },
    CloudHistoryReply {
        epoch: u64,
        owner: String,
        id: String,
        result: Result<CloudTaskPage, String>,
    },
    CloudDiffReply {
        epoch: u64,
        owner: String,
        id: String,
        result: Result<String, String>,
    },
    HistoryPageReply {
        epoch: u64,
        owner: String,
        thread: String,
        sequence: u64,
        cursor: Option<String>,
        result: Result<Value, CallError>,
    },
    McpReply {
        epoch: u64,
        id: String,
        result: Result<Value, CallError>,
    },
    P2Reply {
        epoch: u64,
        id: String,
        result: Result<Value, CallError>,
    },
    Answered {
        epoch: u64,
        owner: String,
        key: central_agent_codex_runtime::wire::ServerRequestKey,
        result: Result<(), CallError>,
    },
    ConversationReply {
        epoch: u64,
        request: ThreadRequest,
        result: Result<Value, CallError>,
    },
    Connected {
        epoch: u64,
        client: Client,
        runtime: Runtime,
        version: String,
        computer_use_root: Result<Option<computer_use::Attachment>, String>,
    },
    Failed {
        epoch: u64,
        reason: String,
    },
    Transport {
        epoch: u64,
        event: TransportEvent,
        received: std::time::Instant,
    },
    Reply {
        epoch: u64,
        revision: u64,
        query: Query,
        result: Result<Value, CallError>,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum Query {
    Account,
    Models,
    ProviderCapabilities,
    Requirements,
    PermissionProfiles,
    LoadedThreads,
    RateLimits,
    AccountUsage,
    WorkspaceMessages,
    Login,
    Cancel,
    Logout,
    SandboxSetup(api::WindowsSandboxMode),
    ComputerUseSkills,
    ComputerUseMcp,
}
impl Query {
    fn key(self) -> &'static str {
        match self {
            Self::Account => "account",
            Self::Models => "models",
            Self::ProviderCapabilities => "provider-capabilities",
            Self::Requirements => "requirements",
            Self::PermissionProfiles => "permission-profiles",
            Self::LoadedThreads => "loaded-threads",
            Self::RateLimits => "rate-limits",
            Self::AccountUsage => "account-usage",
            Self::WorkspaceMessages => "workspace-messages",
            Self::Login => "login",
            Self::Cancel => "cancel",
            Self::Logout => "logout",
            Self::SandboxSetup(_) => "windows-sandbox",
            Self::ComputerUseSkills | Self::ComputerUseMcp => "computer-use",
        }
    }
    fn is_read(self) -> bool {
        matches!(
            self,
            Self::Account
                | Self::Models
                | Self::ProviderCapabilities
                | Self::Requirements
                | Self::PermissionProfiles
                | Self::LoadedThreads
                | Self::RateLimits
                | Self::AccountUsage
                | Self::WorkspaceMessages
        )
    }
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct View {
    connected: bool,
    connecting: bool,
    refreshing: bool,
    auth_busy: bool,
    version: Option<String>,
    account: Option<Account>,
    login: Option<Login>,
    models: Vec<AgentModelOption>,
    requirements_loaded: bool,
    errors: BTreeMap<&'static str, String>,
    chat_reload: chat_reload::View,
    sandbox_setup: access::Setup,
    mcp: mcp::View,
    p2: p2::View,
    diagnostics: diagnostics::Diagnostics,
    computer_use: &'static str,
    browser_use: &'static str,
    computer_use_connection: computer_use::health::Status,
    #[serde(flatten)]
    account_state: account_state::View,
}

// Only public account information crosses into the UI. No raw auth response,
// access token or user-global configuration is persisted or logged here.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Account {
    #[serde(rename = "type")]
    kind: String,
    email: Option<String>,
    plan_type: Option<String>,
    supported: bool,
    policy: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Login {
    #[serde(skip_serializing)]
    id: String,
    url: String,
    user_code: Option<String>,
}

#[derive(Default)]
pub(super) struct State {
    computer_use_health: computer_use::health::Health,
    event_log: event_log::Journal,
    pub(super) view: View,
    pub(super) conversations: Conversations,
    requests: central_agent_codex_runtime::requests::Requests,
    profile: profile::Profile,
    submissions: BTreeMap<String, turns::Submission>,
    pending_inputs: pending_input::PendingInputs,
    preparation: preparation::Preparation,
    timings: delivery::Timings,
    reviews: BTreeMap<String, review::Pending>,
    steering: BTreeMap<String, steering::Steering>,
    roots: BTreeMap<String, PathBuf>,
    access: api::Access,
    summaries: BTreeMap<String, Option<api::ReasoningSummary>>,
    binding_lock: Option<fs::File>,
    binding_store_error: Option<String>,
    // Only an explicit disposable acceptance run can supply its native fixture.
    pub(super) acceptance_home: Option<PathBuf>,
    connection: connection::Connection,
    client: Option<Client>,
    runtime: Option<Runtime>,
    epoch: u64,
    revision: u64,
    initialized: bool,
    bootstrapped: bool,
    completed_logins: HashSet<String>,
    catalog: Vec<AgentModelOption>,
    cursors: HashSet<String>,
    reads: HashSet<&'static str>,
    // Kept Rust-side for native permission/profile validation during turn start.
    requirements: Option<Value>,
    mcp: mcp::McpState,
    p2: p2::State,
    thread_mcp: thread_mcp::Inventories,
    apps: apps::Apps,
    hooks: hooks::Hooks,
    goals: goals::Goals,
    history: BTreeMap<String, history::History>,
    history_pages: history_pages::Pages,
    work_history: work_history::History,
    chat_reload: chat_reload::Reload,
    skills: skills::Skills,
    computer_use_root: Option<PathBuf>,
    browser_use_root: Option<PathBuf>,
    preferences: preferences::Preferences,
    last_branches: BTreeMap<String, branches::BranchTarget>,
    account_state: account_state::State,
    loaded_catalog: HashSet<String>,
    loaded_cursors: HashSet<String>,
}

impl BrowserApp {
    pub(super) fn app_server_conversation(&mut self, owner: String, action: ConversationAction) {
        if let Err(error) = self.prepare_app_server_conversation(&owner, action) {
            self.conversation_event(
                "central-agent:app-server-conversation",
                json!({"owner":owner,"error":error}),
            );
        }
        self.render_agent_panel();
    }

    fn prepare_app_server_conversation(
        &mut self,
        owner: &str,
        action: ConversationAction,
    ) -> Result<(), String> {
        if let ConversationAction::EventLog { expected_thread_id } = &action {
            self.conversation_target(owner)?;
            action.validate_target(self.app_server.conversations.binding(owner))?;
            let mut log = self
                .app_server
                .event_log
                .view(owner, Some(expected_thread_id));
            log["deliveryTimings"] = json!(self.app_server.timings.view(owner, expected_thread_id));
            self.conversation_event(
                "central-agent:native-event-log",
                json!({"owner":owner,"threadId":expected_thread_id,"log":log}),
            );
            return Ok(());
        }
        if let ConversationAction::OpenDelegate {
            expected_thread_id,
            destination,
        } = &action
        {
            action.validate_target(self.app_server.conversations.binding(owner))?;
            return self.open_native_delegate(owner, expected_thread_id, destination);
        }
        if let ConversationAction::ReadWork {
            expected_thread_id,
            turn_id,
        } = &action
        {
            action.validate_target(self.app_server.conversations.binding(owner))?;
            self.conversation_target(owner)?;
            return self.read_native_work_history(owner, expected_thread_id, turn_id);
        }
        if !self.app_server.view.connected {
            return Err("Connect Codex in AI accounts first".into());
        }
        if let Some(error) = &self.app_server.binding_store_error {
            return Err(error.clone());
        }
        if self.app_server.history_pages.busy(owner) {
            return Err("Wait for native Codex history to finish loading".into());
        }
        let target = self.conversation_target(owner)?;
        action.validate_target(self.app_server.conversations.binding(owner))?;
        if let ConversationAction::Goal {
            expected_thread_id,
            expected_directory,
            action,
        } = action
        {
            return self.control_native_goal(
                owner,
                &expected_thread_id,
                &expected_directory,
                action,
            );
        }
        if let ConversationAction::AppsControl {
            expected_thread_id,
            action,
        } = action
        {
            if target.remote || !self.native_access_selected(owner) {
                return Err("Select Codex in a local conversation to use native Apps".into());
            }
            return self.control_native_apps(owner, expected_thread_id.as_deref(), action);
        }
        if let ConversationAction::HooksStatus {
            expected_thread_id,
            expected_directory,
        } = action
        {
            if target.remote || !self.native_access_selected(owner) {
                return Err("Select Codex in a local conversation to inspect native hooks".into());
            }
            return self.refresh_native_hooks(owner, &expected_thread_id, &expected_directory);
        }
        if let ConversationAction::McpControl {
            expected_thread_id,
            action,
        } = action
        {
            if target.remote || !self.native_access_selected(owner) {
                return Err(
                    "Select Codex in a local conversation to authorize its MCP service.".into(),
                );
            }
            return self.control_thread_mcp(owner, &expected_thread_id, action);
        }
        if let ConversationAction::McpStatus { expected_thread_id } = action {
            if target.remote || !self.native_access_selected(owner) {
                return Err(
                    "Select Codex in a local conversation to inspect its MCP tools.".into(),
                );
            }
            return self.refresh_thread_mcp(owner, &expected_thread_id);
        }
        if let ConversationAction::OpenBranch { destination } = &action {
            return self.open_native_branch(owner, destination);
        }
        if target.remote {
            return Err("Remote filesystem agents require the native SSH/MCP integration; a remote path is never a Windows working directory".into());
        }
        if !matches!(
            action,
            ConversationAction::Read { .. } | ConversationAction::ReviewDelivery { .. }
        ) && (self.owner_has_unfinished_work(owner) || self.conversation_busy(owner))
        {
            return Err("Finish this conversation's current provider work before changing its native thread".into());
        }
        if let ConversationAction::ReviewDelivery { message_id } = action {
            self.app_server
                .conversations
                .resolve_delivery_after_user_review(owner, &message_id)?;
            self.save_app_server_bindings()?;
            self.emit_app_server_conversation(owner, "delivery_reviewed");
            return Ok(());
        }
        if matches!(action, ConversationAction::ResetUnused { .. }) {
            if !self.native_access_selected(owner) {
                return Err("Select Codex before resetting its local link".into());
            }
            self.app_server.conversations.reset_unused_link(owner)?;
            self.save_app_server_bindings()?;
            self.emit_app_server_conversation(owner, "unused_link_reset");
            return Ok(());
        }
        if let ConversationAction::StartReview {
            expected_directory,
            target,
            delivery,
            destination_name,
            ..
        } = action
        {
            return self.prepare_native_review(
                owner,
                &expected_directory,
                target,
                delivery,
                destination_name,
            );
        }
        if let ConversationAction::NewFork {
            name,
            expected_directory,
            last_turn_id,
            ..
        } = action
        {
            if !self.app_server.conversations.observed(owner) {
                return Err("Read native history after reconnecting before forking".into());
            }
            return self.create_native_branch(
                owner,
                &name,
                &expected_directory,
                last_turn_id.as_deref(),
            );
        }
        let request = match action {
            ConversationAction::Open { .. } => {
                if !self.app_server.view.requirements_loaded || self.app_server.view.refreshing {
                    return Err(
                        "Wait for Codex account and managed configuration to finish loading".into(),
                    );
                }
                let cwd = match target.root {
                    Some(root) => root,
                    None if !owner.starts_with("graph:") => {
                        let root = self.data_dir.join("codex-workspace");
                        fs::create_dir_all(&root).map_err(|error| {
                            format!("The local Codex workspace could not be prepared: {error}")
                        })?;
                        root
                    }
                    None => return Err("Select a graph node with a local working directory".into()),
                };
                let access = self.app_server.access();
                access.validate_requirements(self.app_server.requirements.as_ref())?;
                self.app_server
                    .conversations
                    .open(owner, &cwd, &api::Profile::default(), access)?
            }
            action => {
                let action = match action {
                    ConversationAction::Read { .. } => ThreadAction::Read,
                    ConversationAction::Rename { name, .. } => ThreadAction::Rename(name),
                    ConversationAction::Archive { .. } => ThreadAction::Archive,
                    ConversationAction::Unarchive { .. } => ThreadAction::Unarchive,
                    ConversationAction::Delete { .. } => ThreadAction::Delete,
                    ConversationAction::Compact { .. } => {
                        if !self.native_access_selected(owner)
                            || self.app_server.native_config_pending()
                        {
                            return Err("Select Codex and finish shared configuration changes before compacting".into());
                        }
                        ThreadAction::Compact {
                            operation_id: Uuid::new_v4().to_string(),
                        }
                    }
                    _ => unreachable!(),
                };
                self.app_server.conversations.action(owner, action)?
            }
        };
        self.dispatch_app_server_thread(request);
        Ok(())
    }

    fn save_app_server_bindings(&mut self) -> Result<(), String> {
        let saved = self.app_server.conversations.saved().clone();
        self.save_app_server_binding_snapshot(&saved)
    }

    fn save_app_server_binding_snapshot(&mut self, saved: &Saved) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(saved).map_err(|e| e.to_string())?;
        if let Err(error) =
            write_file_atomically::<Saved>(&self.data_dir.join("app-server-threads.json"), &bytes)
        {
            let message = format!(
                "Native conversation IDs could not be saved: {error}. No turn will start until persistence is recovered."
            );
            self.app_server.binding_store_error = Some(message.clone());
            self.app_server
                .view
                .errors
                .insert("conversations", message.clone());
            return Err(message);
        }
        Ok(())
    }

    /// Remove local ownership atomically, then release any connection-local
    /// subscriptions. Native history and project files are never deleted here.
    pub(super) fn validate_app_server_unlink_owners(
        &self,
        owners: &[String],
    ) -> Result<(), String> {
        for owner in owners {
            if self.app_server.busy(owner) || !self.app_server.requests.views(owner).is_empty() {
                return Err(format!(
                    "Stop or reconcile {owner}'s native Codex work before removing its local surface"
                ));
            }
            self.app_server.conversations.prepare_unlink(owner)?;
        }
        Ok(())
    }

    pub(super) fn unlink_app_server_owners(&mut self, owners: &[String]) -> Result<(), String> {
        self.validate_app_server_unlink_owners(owners)?;
        let mut unlinks = Vec::new();
        for owner in owners {
            if let Some(unlink) = self.app_server.conversations.prepare_unlink(owner)? {
                unlinks.push(unlink);
            }
        }
        if unlinks.is_empty() {
            return Ok(());
        }
        let mut saved = self.app_server.conversations.saved().clone();
        for unlink in &unlinks {
            saved.bindings.remove(&unlink.owner);
            saved.lineage.remove(&unlink.thread_id);
            saved.delegations.remove(&unlink.thread_id);
            saved.unresolved.remove(&unlink.owner);
            saved.fork_origins.remove(&unlink.owner);
        }
        saved.validate()?;
        self.save_app_server_binding_snapshot(&saved)?;
        let mut calls = Vec::new();
        for unlink in unlinks {
            if let Some(call) = self.app_server.conversations.commit_unlink(&unlink)? {
                calls.push((unlink.thread_id.clone(), call));
            }
            self.app_server.roots.remove(&unlink.owner);
            self.app_server.summaries.remove(&unlink.owner);
            self.app_server.submissions.remove(&unlink.owner);
            self.app_server.pending_inputs.forget(&unlink.owner);
            self.app_server.preparation.forget(&unlink.owner);
            self.app_server.reviews.remove(&unlink.owner);
            self.app_server.steering.remove(&unlink.owner);
            self.app_server.history.remove(&unlink.owner);
            self.app_server.last_branches.remove(&unlink.owner);
            self.app_server.apps.remove_owner(&unlink.owner);
            self.app_server.hooks.remove_owner(&unlink.owner);
        }
        for (thread, call) in calls {
            self.dispatch_app_server_unsubscribe(thread, call);
        }
        Ok(())
    }

    fn dispatch_app_server_unsubscribe(&mut self, thread: String, call: Call) {
        let Some(client) = self.app_server.client.clone() else {
            let _ = self.app_server.conversations.complete_unsubscribe(
                &thread,
                Err(CallError::Rejected(
                    "The connection closed before unsubscribe dispatch".into(),
                )),
            );
            return;
        };
        let epoch = self.app_server.epoch;
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = call.send(&client).and_then(|ticket| ticket.wait());
            let _ = proxy.send_event(BrowserEvent::AppServer(Event::UnsubscribeReply {
                epoch,
                thread,
                result,
            }));
        });
    }

    pub(in crate::browser) fn emit_app_server_conversation(&self, owner: &str, change: &str) {
        let supervision_state = self
            .is_supervised_project_chat_owner(owner)
            .then(|| self.conversation_state(owner));
        let binding = self.app_server.conversations.binding(owner);
        let thread = binding.and_then(|binding| {
            self.app_server
                .conversations
                .mirror
                .thread(&binding.thread_id)
        });
        self.conversation_event(
            "central-agent:app-server-conversation",
            json!({"owner":owner,"change":change,
            "binding":binding,"thread":thread.map(|t|json!({"id":t.id})),"busy":self.app_server.conversations.busy(owner),
            "liveDiff":self.live_diff_for_owner(owner),
            "conversationState":supervision_state,
            "nativeConversation":self.app_server_conversation_view(owner),
            "unresolved":self.app_server.delivery_warning(owner),
            "nativeRequests":self.app_server_request_views(owner)}),
        );
    }
    pub(super) fn app_server_account(&mut self, action: AccountAction) {
        if matches!(action, AccountAction::ReloadChats) {
            if self.app_server.view.connecting || self.app_server.chat_reload.view.loading {
                return;
            }
            if !self.app_server.view.connected {
                self.app_server_account(AccountAction::Connect);
            } else {
                self.begin_native_chat_reload();
                self.advance_native_chat_reload();
                self.app_server.sync_profile();
                self.render_agent_panel();
            }
            return;
        }
        if matches!(action, AccountAction::Connect)
            && (self.app_server.binding_lock.is_none()
                || self.app_server.binding_store_error.is_some())
        {
            self.app_server.view.errors.insert("connection", "Supervisor cannot connect until its conversation archive is available exclusively and can be read safely.".into());
            self.render_agent_panel();
            return;
        }
        if self.app_server.view.connected
            && (self.app_server.any_busy() || self.app_server.view.sandbox_setup.busy())
            && matches!(
                action,
                AccountAction::Connect
                    | AccountAction::LogoutConfirmed
                    | AccountAction::LoginBrowser
                    | AccountAction::LoginDevice
            )
        {
            self.app_server.view.errors.insert("connection", "Stop or reconcile active Codex conversations before reconnecting or changing the account.".into());
            self.render_agent_panel();
            return;
        }
        match action {
            AccountAction::Connect
                if !self.app_server.view.connecting
                    && !self.app_server.view.auth_busy
                    && self.app_server.view.login.is_none() =>
            {
                // Explicit reconnect or one remembered startup connection.
                // Dropping the old process never replays calls or signs in.
                self.computer_use_frame.clear();
                self.app_server.connection.begin_connect();
                let epoch = self.app_server.epoch + 1;
                let mut old = std::mem::replace(
                    &mut self.app_server,
                    State {
                        epoch,
                        ..State::default()
                    },
                );
                old.conversations.disconnect();
                old.preparation.disconnect();
                old.p2.disconnect();
                for owner in old.requests.disconnect() {
                    self.emit_app_server_requests(&owner, None);
                }
                self.app_server.conversations = old.conversations;
                self.app_server.event_log = old.event_log;
                self.app_server.preparation = old.preparation;
                self.app_server.roots = old.roots;
                self.app_server.access = old.access;
                self.app_server.p2 = old.p2;
                self.app_server.profile.selection = old.profile.selection;
                self.app_server.binding_store_error = old.binding_store_error;
                self.app_server.binding_lock = old.binding_lock;
                self.app_server.acceptance_home = old.acceptance_home;
                self.app_server.connection = old.connection;
                for key in ["connection-preference", "permissions"] {
                    if let Some(error) = old.view.errors.remove(key) {
                        self.app_server.view.errors.insert(key, error);
                    }
                }
                if let Some(error) = &self.app_server.binding_store_error {
                    self.app_server
                        .view
                        .errors
                        .insert("conversations", error.clone());
                }
                if let Some(client) = old.client {
                    std::thread::spawn(move || client.shutdown());
                }
                self.app_server.view.connecting = true;
                let cwd = self.data_dir.clone();
                let saved = self.app_server.conversations.saved().clone();
                let acceptance_home = self.app_server.acceptance_home.clone();
                let acceptance = acceptance_home.is_some();
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let result = acceptance_home
                        .map_or_else(|| native_profile::prepare(&cwd, &saved), Ok)
                        .and_then(|home| Runtime::discover(&cwd)?.with_home(&home))
                        .and_then(|runtime| {
                            if !acceptance {
                                native_profile::finish(&runtime)?;
                            }
                            let computer_use_root = if acceptance {
                                Ok(None)
                            } else {
                                runtime
                                    .home()
                                    .map(computer_use::prepare)
                                    .unwrap_or(Ok(None))
                            };
                            let events = proxy.clone();
                            Client::spawn(&runtime, env!("CARGO_PKG_VERSION"), move |event| {
                                let _ =
                                    events.send_event(BrowserEvent::AppServer(Event::Transport {
                                        epoch,
                                        event,
                                        received: std::time::Instant::now(),
                                    }));
                            })
                            .map(|client| {
                                let version = runtime.version().to_owned();
                                (client, runtime, version, computer_use_root)
                            })
                            .map_err(|e| e.to_string())
                        });
                    let event = match result {
                        Ok((client, runtime, version, computer_use_root)) => Event::Connected {
                            epoch,
                            client,
                            runtime,
                            version,
                            computer_use_root,
                        },
                        Err(reason) => Event::Failed { epoch, reason },
                    };
                    let _ = proxy.send_event(BrowserEvent::AppServer(event));
                });
            }
            AccountAction::Connect => {}
            AccountAction::ReloadChats => unreachable!("handled before account actions"),
            _ if !self.app_server.view.connected => {
                self.app_server.view.errors.insert(
                    "connection",
                    "Connect the official App Server first.".into(),
                );
            }
            AccountAction::OpenLogin => {
                if let Some(login) = &self.app_server.view.login
                    && let Err(error) = crate::external_browser::open(&login.url)
                {
                    self.app_server.view.errors.insert("login", error);
                }
            }
            AccountAction::SetupSandboxElevated => {
                self.start_native_sandbox_setup(api::WindowsSandboxMode::Elevated)
            }
            AccountAction::SetupSandboxUnelevated => {
                self.start_native_sandbox_setup(api::WindowsSandboxMode::Unelevated)
            }
            AccountAction::Refresh => self.refresh_app_server_account(),
            _ if self.app_server.view.auth_busy => {}
            AccountAction::LoginBrowser | AccountAction::LoginDevice => {
                if self.app_server.view.login.is_none() && !self.app_server.view.refreshing {
                    self.app_server.connection.begin_login();
                    self.app_server.view.auth_busy = true;
                    self.app_server.view.errors.remove("login");
                    self.app_server.view.errors.remove("sign-in");
                    self.app_server_call(
                        Query::Login,
                        api::login(matches!(action, AccountAction::LoginDevice)),
                    );
                }
            }
            AccountAction::CancelLogin => {
                if let Some(id) = self
                    .app_server
                    .view
                    .login
                    .as_ref()
                    .map(|login| login.id.clone())
                {
                    self.app_server.view.auth_busy = true;
                    self.app_server_call(Query::Cancel, api::cancel_login(&id));
                }
            }
            AccountAction::LogoutConfirmed => {
                if self.app_server.view.login.is_none()
                    && self.app_server.forget_connection_for_logout(&self.data_dir)
                {
                    self.app_server.view.auth_busy = true;
                    self.app_server_call(Query::Logout, api::logout());
                }
            }
        }
        self.app_server.sync_profile();
        self.render_agent_panel();
    }

    fn app_server_call(&self, query: Query, call: Call) {
        let Some(client) = self.app_server.client.clone() else {
            return;
        };
        let epoch = self.app_server.epoch;
        let revision = self.app_server.revision;
        let proxy = self.proxy.clone();
        // Includes the write: even a backpressured stdin cannot block winit.
        std::thread::spawn(move || {
            let result = call.send(&client).and_then(|ticket| ticket.wait());
            let _ = proxy.send_event(BrowserEvent::AppServer(Event::Reply {
                epoch,
                revision,
                query,
                result,
            }));
        });
    }

    fn refresh_app_server_account(&mut self) {
        self.app_server.begin_refresh();
        for (query, call) in [
            (Query::Account, api::account_read()),
            (Query::Models, api::model_list(None)),
            (
                Query::ProviderCapabilities,
                api::model_provider_capabilities(),
            ),
            (Query::Requirements, api::requirements()),
            (Query::PermissionProfiles, api::permission_profiles(None)),
        ] {
            self.app_server.view.errors.remove(query.key());
            self.app_server_call(query, call);
        }
        self.reconcile_app_server_loaded_threads();
    }

    fn refresh_app_server_account_extras(&mut self) {
        self.app_server.account_state.begin_account_extras();
        for (query, call) in [
            (Query::AccountUsage, api::account_usage(None)),
            (Query::WorkspaceMessages, api::workspace_messages()),
        ] {
            self.app_server.reads.insert(query.key());
            self.app_server.view.errors.remove(query.key());
            self.app_server_call(query, call);
        }
    }

    fn refresh_app_server_rate_limits(&mut self) {
        if self
            .app_server
            .view
            .account
            .as_ref()
            .is_some_and(|account| account.supported)
            && self.app_server.account_state.begin_rate_limits()
        {
            self.dispatch_app_server_rate_limits();
        }
    }

    fn invalidate_app_server_rate_limits(&mut self) {
        if self
            .app_server
            .view
            .account
            .as_ref()
            .is_some_and(|account| account.supported)
            && self.app_server.account_state.invalidate_rate_limits()
        {
            self.dispatch_app_server_rate_limits();
        }
    }

    fn dispatch_app_server_rate_limits(&mut self) {
        self.app_server.reads.insert("rate-limits");
        self.app_server.view.refreshing = true;
        self.app_server.view.errors.remove("rate-limits");
        self.app_server_call(Query::RateLimits, api::rate_limits());
    }

    fn reconcile_app_server_loaded_threads(&mut self) {
        self.app_server.loaded_catalog.clear();
        self.app_server.loaded_cursors.clear();
        self.app_server.reads.insert("loaded-threads");
        self.app_server.view.refreshing = true;
        self.app_server.view.errors.remove("loaded-threads");
        self.app_server_call(Query::LoadedThreads, api::loaded_threads(None));
    }

    pub(super) fn handle_app_server_event(&mut self, event_loop: &ActiveEventLoop, event: Event) {
        let epoch = match &event {
            Event::UnsubscribeReply { epoch, .. }
            | Event::WorkHistoryReply { epoch, .. }
            | Event::GoalReply { epoch, .. }
            | Event::ThreadMcpReply { epoch, .. }
            | Event::AppsReply { epoch, .. }
            | Event::HooksReply { epoch, .. }
            | Event::PreferencesReply { epoch, .. }
            | Event::SkillsReply { epoch, .. }
            | Event::HistoryReply { epoch, .. }
            | Event::CloudHistoryReply { epoch, .. }
            | Event::CloudDiffReply { epoch, .. }
            | Event::HistoryPageReply { epoch, .. }
            | Event::McpReply { epoch, .. }
            | Event::P2Reply { epoch, .. }
            | Event::Answered { epoch, .. }
            | Event::ConversationReply { epoch, .. }
            | Event::Connected { epoch, .. }
            | Event::Failed { epoch, .. }
            | Event::Transport { epoch, .. }
            | Event::Reply { epoch, .. } => *epoch,
        };
        if epoch != self.app_server.epoch {
            if let Event::Connected { client, .. } = event {
                std::thread::spawn(move || client.shutdown());
            }
            return;
        }
        match event {
            Event::WorkHistoryReply {
                owner,
                thread,
                turn,
                request,
                result,
                ..
            } => {
                self.native_work_history_reply(&owner, &thread, &turn, request, result);
            }
            Event::UnsubscribeReply { thread, result, .. } => {
                match self
                    .app_server
                    .conversations
                    .complete_unsubscribe(&thread, result)
                {
                    Ok(()) => {
                        self.app_server.view.errors.remove("subscription");
                    }
                    Err(error) => {
                        self.app_server.view.errors.insert("subscription", error);
                    }
                }
            }
            Event::GoalReply {
                owner,
                thread,
                id,
                result,
                ..
            } => {
                self.native_goal_reply(&owner, &thread, &id, result);
            }
            Event::ThreadMcpReply {
                owner,
                thread,
                id,
                result,
                ..
            } => {
                self.thread_mcp_reply(&owner, &thread, &id, result);
            }
            Event::AppsReply {
                owner,
                thread,
                id,
                result,
                ..
            } => self.native_apps_reply(&owner, &thread, &id, result),
            Event::HooksReply {
                owner,
                thread,
                id,
                result,
                ..
            } => self.native_hooks_reply(&owner, &thread, &id, result),
            Event::PreferencesReply {
                owner, id, result, ..
            } => self.app_server_preferences_reply(&owner, &id, result),
            Event::SkillsReply {
                owner, id, result, ..
            } => self.app_server_skills_reply(&owner, &id, result),
            Event::HistoryReply {
                owner, id, result, ..
            } => self.app_server_history_reply(&owner, &id, result),
            Event::CloudHistoryReply {
                owner, id, result, ..
            } => self.app_server_cloud_history_reply(&owner, &id, result),
            Event::CloudDiffReply {
                owner, id, result, ..
            } => self.app_server_cloud_diff_reply(&owner, &id, result),
            Event::HistoryPageReply {
                owner,
                thread,
                sequence,
                cursor,
                result,
                ..
            } => {
                self.native_history_page_reply(&owner, &thread, sequence, cursor.as_deref(), result)
            }
            Event::McpReply { id, result, .. } => {
                if let Some((id, call)) = self.app_server.mcp.reply(&id, result) {
                    self.app_server_mcp_call(id, call);
                }
            }
            Event::P2Reply { id, result, .. } => {
                let effect = self.app_server.p2.reply(&id, result);
                if let Some((next_id, call)) = effect.follow_up {
                    self.app_server_p2_call(next_id, call);
                }
                if effect.refresh_rate_limits {
                    self.refresh_app_server_rate_limits();
                }
            }
            Event::Answered {
                owner, key, result, ..
            } => {
                let thread = self
                    .app_server
                    .conversations
                    .binding(&owner)
                    .map(|b| b.thread_id.clone());
                self.record_native_event(Some(&owner), "response", "request/answer", &json!({"threadId":thread,"outcome":if result.is_ok() {"sent"}else{"transport_error"}}), Some(match &key.id { central_agent_codex_runtime::wire::RequestId::Text(id)=>id.clone(), central_agent_codex_runtime::wire::RequestId::Integer(id)=>id.to_string() }));
                self.app_server.requests.sent(&key);
                self.emit_app_server_requests(&owner, result.err().map(|e| e.to_string()));
                if let Some(thread) = thread {
                    self.emit_native_delegate_relatives(&thread);
                }
            }
            Event::ConversationReply {
                request, result, ..
            } => {
                let was_preparation = self.app_server.preparation.finish(&request);
                let owner = request.owner.clone();
                let mut reload_ok = false;
                let value = result.as_ref().ok();
                let metadata = json!({"threadId":value.and_then(|v|v["thread"]["id"].as_str()).or_else(||request.call.params["threadId"].as_str()),"turnId":value.and_then(|v|v["turn"]["id"].as_str()),"outcome":event_log::call_outcome(&result)});
                self.record_native_event(
                    Some(&owner),
                    "response",
                    request.call.method,
                    &metadata,
                    Some(request.diagnostic_id()),
                );
                match self.app_server.conversations.complete(&request, result) {
                    Ok(outcome) => match self.save_app_server_bindings() {
                        Ok(()) => {
                            let (target, change) = match &outcome {
                                Outcome::ReviewReady { owner } => {
                                    (owner.as_str(), "review_prepared")
                                }
                                Outcome::DetachedReview { owner, .. } => {
                                    (owner.as_str(), "detached_review_started")
                                }
                                Outcome::Forked { owner, .. } => (owner.as_str(), "forked"),
                                Outcome::Opened { owner, .. } => (owner.as_str(), "opened"),
                                Outcome::Deleted { owner } => (owner.as_str(), "deleted"),
                                Outcome::Updated { owner }
                                    if request.call.method == "thread/compact/start" =>
                                {
                                    (owner.as_str(), "compaction_accepted")
                                }
                                Outcome::Updated { owner } => (owner.as_str(), "updated"),
                                Outcome::Accepted { owner, .. } => (owner.as_str(), "accepted"),
                            };
                            // Dispatch a waiting prompt before history pagination,
                            // delegate projection or UI refresh can delay it.
                            if let Outcome::Opened { owner, .. } = &outcome
                                && let Err(error) = self.start_opened_app_server_turn(owner)
                            {
                                self.fail_app_server_submission(owner, error);
                            }
                            let history_error = self
                                .schedule_requested_native_history(&request, target)
                                .err();
                            reload_ok = history_error.is_none();
                            self.reconcile_native_delegates(target);
                            self.emit_app_server_conversation(target, change);
                            if matches!(outcome, Outcome::Forked { .. }) && target != owner {
                                self.emit_app_server_conversation(&owner, "branched");
                            }
                            if let Some(error) = history_error {
                                self.conversation_event(
                                    "central-agent:app-server-conversation",
                                    json!({
                                        "owner":target,
                                        "error":error,
                                        "nativeConversation":self.app_server_conversation_view(target)
                                    }),
                                );
                            }
                            match outcome {
                                Outcome::ReviewReady { owner } => {
                                    if let Err(error) = self.start_prepared_native_review(&owner) {
                                        self.fail_native_review(&owner, error);
                                    }
                                }
                                Outcome::Updated { owner }
                                    if request.call.method == "review/start" =>
                                {
                                    self.accepted_native_review(&owner)
                                }
                                Outcome::DetachedReview {
                                    source_owner,
                                    owner,
                                    ..
                                } => self.accepted_detached_native_review(&source_owner, &owner),
                                Outcome::Accepted {
                                    owner,
                                    message_id,
                                    turn_id,
                                } => {
                                    self.accept_app_server_submission(&owner, &message_id, &turn_id)
                                }
                                _ => {}
                            }
                        }
                        Err(error) if self.app_server.reviews.contains_key(&owner) => {
                            self.fail_native_review(&owner, error)
                        }
                        Err(error) => self.fail_app_server_submission(&owner, error),
                    },
                    Err(error) => {
                        // Definitive rejection clears its receipt too.
                        let error = self.save_app_server_bindings().err().unwrap_or(error);
                        let error = if request.call.method == "thread/fork" {
                            format!(
                                "{error} A native fork may already exist. Browse Codex history before retrying; it will not be replayed automatically."
                            )
                        } else {
                            error
                        };
                        if self.app_server.reviews.contains_key(&owner)
                            && matches!(request.call.method, "thread/resume" | "review/start")
                        {
                            self.fail_native_review(&owner, error);
                        } else if conversation_failure_belongs_to_submission(
                            request.call.method,
                            self.app_server.submissions.contains_key(&owner),
                        ) {
                            self.fail_app_server_submission(&owner, error);
                        } else if !was_preparation {
                            // A concurrent read/interrupt failure does not resolve
                            // the input awaiting its own turn/start acknowledgement.
                            self.conversation_event(
                                "central-agent:app-server-conversation",
                                json!({"owner":owner,"error":error}),
                            );
                        }
                    }
                }
                self.app_server.chat_reload.read_reply(
                    &request,
                    reload_ok,
                    self.app_server.history_pages.sequence(&owner),
                );
                self.refresh_completed_native_review(&owner);
            }
            Event::Connected {
                client,
                runtime,
                version,
                computer_use_root,
                ..
            } => {
                self.computer_use_frame.clear();
                self.app_server.client = Some(client);
                self.app_server.runtime = Some(runtime);
                self.app_server.view.version = Some(version);
                let computer_use_root = match computer_use_root {
                    Ok(root) => root,
                    Err(error) => {
                        self.app_server.view.errors.insert("computer-use", error);
                        None
                    }
                };
                let computer_use_enabled = computer_use_root
                    .as_ref()
                    .is_some_and(|attachment| attachment.enabled && attachment.root.is_some());
                let browser_use_enabled = computer_use_root.as_ref().is_some_and(|attachment| {
                    attachment.browser_enabled && attachment.browser_root.is_some()
                });
                self.app_server.view.computer_use = if computer_use_enabled {
                    "loading"
                } else {
                    "unavailable"
                };
                self.app_server.view.browser_use = if browser_use_enabled {
                    "loading"
                } else {
                    "unavailable"
                };
                self.app_server
                    .skills
                    .set_computer_use(computer_use_root.as_ref());
                self.app_server.browser_use_root =
                    computer_use_root.as_ref().and_then(|attachment| {
                        attachment
                            .browser_enabled
                            .then(|| attachment.browser_root.clone())
                            .flatten()
                    });
                self.app_server.computer_use_root = computer_use_root
                    .and_then(|attachment| attachment.enabled.then_some(attachment.root).flatten());
                self.sync_app_server_p2_scope();
            }
            Event::Failed { reason, .. }
            | Event::Transport {
                event: TransportEvent::Closed { reason },
                ..
            } => {
                self.computer_use_frame.clear();
                let owners = self
                    .app_server
                    .conversations
                    .saved()
                    .bindings
                    .iter()
                    .map(|(owner, b)| (owner.clone(), b.thread_id.clone()))
                    .collect::<Vec<_>>();
                for (owner, thread) in owners {
                    self.record_native_event(
                        Some(&owner),
                        "connection",
                        "connection/closed",
                        &json!({"threadId":thread,"outcome":"closed"}),
                        None,
                    );
                }
                self.app_server.view.errors.insert("connection", reason);
                self.app_server.chat_reload.disconnect();
                self.app_server.work_history.disconnect();
                self.app_server.view.connected = false;
                self.app_server.view.computer_use = "unavailable";
                self.app_server.view.browser_use = "unavailable";
                self.app_server.computer_use_health = Default::default();
                self.app_server.view.computer_use_connection = Default::default();
                self.app_server.view.connecting = false;
                self.app_server.view.refreshing = false;
                self.app_server.view.auth_busy = false;
                self.app_server.view.login = None;
                self.app_server.view.account = None;
                self.app_server.runtime = None;
                self.app_server.view.models.clear();
                self.app_server.view.requirements_loaded = false;
                self.app_server.requirements = None;
                self.app_server.conversations.disconnect();
                self.supervision_disconnected();
                self.app_server.pending_inputs.clear();
                self.app_server.preparation.disconnect();
                self.app_server.timings.disconnect();
                self.app_server.history_pages.clear();
                self.app_server.reviews.clear();
                self.app_server.view.sandbox_setup.disconnected();
                self.app_server.mcp.disconnect();
                self.app_server.p2.disconnect();
                for owner in self.app_server.goals.disconnect() {
                    self.emit_app_server_conversation(&owner, "stream");
                }
                for owner in self.app_server.thread_mcp.invalidate_all("Disconnected. Refresh after resuming the native conversation; no request was replayed.") {
                    self.emit_app_server_conversation(&owner, "stream");
                }
                for owner in self.app_server.apps.invalidate_all("Disconnected. Refresh Plugins after resuming the native conversation; no request was replayed.") {
                    self.emit_app_server_conversation(&owner, "stream");
                }
                for owner in self.app_server.hooks.invalidate_all("Disconnected. Refresh hooks after reconnecting; no hook was run or replayed by Supervisor.") {
                    self.emit_app_server_conversation(&owner, "stream");
                }
                self.app_server.skills.disconnect();
                self.app_server.preferences.disconnect();
                self.app_server.account_state.disconnect();
                self.invalidate_app_server_preferences();
                self.invalidate_app_server_skills();
                let history_owners: Vec<_> = self.app_server.history.keys().cloned().collect();
                self.app_server.history.clear();
                for owner in history_owners {
                    self.emit_app_server_history(&owner,Some("Codex disconnected. Reload native history; no import or fork was replayed.".into()));
                }
                let owners: Vec<_> = self
                    .app_server
                    .submissions
                    .keys()
                    .chain(self.app_server.steering.keys())
                    .cloned()
                    .collect();
                for owner in owners {
                    self.fail_app_server_submission(&owner, "Codex disconnected. The draft was not cleared. Check native history before retrying an unacknowledged request; it will not be resent automatically.".into());
                }
                for owner in self.app_server.requests.disconnect() {
                    self.emit_app_server_requests(
                        &owner,
                        Some(
                            "Codex disconnected. Pending decisions were discarded, not replayed."
                                .into(),
                        ),
                    );
                }
                self.app_server.epoch += 1; // Invalidates late replies and a racing Connected.
                if let Some(client) = self.app_server.client.take() {
                    std::thread::spawn(move || client.shutdown());
                }
            }
            Event::Transport {
                event: TransportEvent::Ready { .. },
                ..
            } => self.app_server.initialized = true,
            Event::Transport {
                event: TransportEvent::Notification { method, params },
                received,
                ..
            } => {
                if self.app_server.computer_use_root.is_some()
                    && self.app_server.computer_use_health.notice(&method, &params)
                {
                    self.app_server.view.computer_use_connection =
                        self.app_server.computer_use_health.status;
                    self.emit_other_skill_views("");
                }
                self.computer_use_frame.notice(
                    &method,
                    &params,
                    self.app_server.view.computer_use == "available",
                );
                if let Err(error) = self
                    .computer_use_frame
                    .sync(event_loop.available_monitors())
                {
                    warn!(%error, "Computer Use desktop frame could not be displayed");
                }
                self.app_server
                    .timings
                    .notification(&method, &params, received);
                let mut projected = false;
                let children = self
                    .app_server
                    .conversations
                    .discover_delegations(&method, &params);
                self.adopt_native_delegates(children);
                let thread = params
                    .get("threadId")
                    .or_else(|| params["thread"].get("id"))
                    .and_then(Value::as_str);
                let owner = thread
                    .and_then(|id| self.app_server.conversations.owner_for_thread(id))
                    .map(str::to_owned);
                if thread.is_some() {
                    self.record_native_event(
                        owner.as_deref(),
                        "notification",
                        &method,
                        &params,
                        None,
                    );
                }
                match self.app_server.p2.notification(&method, &params) {
                    Ok(true) => {
                        projected = true;
                        self.app_server.view.errors.remove("p2-event");
                    }
                    Ok(false) => {}
                    Err(error) => {
                        projected = true;
                        self.app_server.view.errors.insert("p2-event", error);
                    }
                }
                match self.app_server.account_state.notice(&method, &params) {
                    Ok(true) => {
                        projected = true;
                        self.app_server.view.errors.remove("runtime-notice");
                    }
                    Ok(false) => {}
                    Err(error) => {
                        projected = true;
                        self.app_server.view.errors.insert("runtime-notice", error);
                    }
                }
                for owner in self.app_server.goals.notify(&method, &params) {
                    projected = true;
                    self.emit_app_server_conversation(&owner, "stream");
                }
                for owner in self.app_server.thread_mcp.notify(&method, &params) {
                    projected = true;
                    self.emit_app_server_conversation(&owner, "stream");
                }
                for owner in self.app_server.requests.notify(&method, &params) {
                    projected = true;
                    self.emit_app_server_requests(&owner, None);
                }
                for owner in self.app_server.hooks.notify(&method, &params) {
                    projected = true;
                    self.emit_app_server_conversation(&owner, "stream");
                }
                if method == "skills/changed" {
                    projected = true;
                    self.invalidate_app_server_skills();
                    self.invalidate_app_server_preferences();
                    for owner in self.app_server.apps.list_updated(
                        "Installed Codex plugins changed. Refresh before selecting a plugin.",
                    ) {
                        self.emit_app_server_conversation(&owner, "stream");
                    }
                }
                if (matches!(
                    method.as_str(),
                    "thread/closed" | "thread/deleted" | "thread/archived"
                ) || method == "thread/status/changed"
                    && params["status"]["type"] == "notLoaded")
                    && let Some(thread) = params.get("threadId").and_then(Value::as_str)
                {
                    for owner in self.app_server.apps.invalidate_thread(
                        thread,
                        "The native conversation changed. Resume it and refresh Plugins.",
                    ) {
                        self.emit_app_server_conversation(&owner, "stream");
                    }
                    for owner in self.app_server.hooks.invalidate_thread(
                        thread,
                        "The native conversation changed. Resume it and refresh hooks.",
                    ) {
                        self.emit_app_server_conversation(&owner, "stream");
                    }
                }
                match self.app_server.conversations.notification(&method, &params) {
                    Ok(true) => {
                        projected = true;
                        if matches!(
                            method.as_str(),
                            "thread/archived"
                                | "thread/unarchived"
                                | "thread/deleted"
                                | "turn/started"
                                | "turn/completed"
                        ) || method == "thread/status/changed"
                            && params["status"]["type"] == "active"
                        {
                            // Persist only binding metadata, never event content.
                            // Failure gates new native mutations until recovered.
                            let _ = self.save_app_server_bindings();
                        }
                        if let Some(owner) = params
                            .get("threadId")
                            .and_then(Value::as_str)
                            .and_then(|id| self.app_server.conversations.owner_for_thread(id))
                            .map(str::to_owned)
                        {
                            if method == "thread/reverted" {
                                self.app_server.history_pages.cancel(&owner);
                                if !self.app_server.conversations.revert_pending(&owner) {
                                    match self
                                        .app_server
                                        .conversations
                                        .action(&owner, ThreadAction::Read)
                                    {
                                        Ok(read) => self.dispatch_app_server_thread(read),
                                        Err(error) => {
                                            self.app_server
                                                .view
                                                .errors
                                                .insert("conversation-event", error);
                                        }
                                    }
                                }
                            }
                            self.emit_app_server_conversation(&owner, "stream");
                            if self.app_server.conversations.take_compaction_stop(&owner) {
                                self.stop_app_server(&owner);
                            }
                            self.refresh_completed_native_review(&owner);
                            if matches!(
                                method.as_str(),
                                "turn/started"
                                    | "turn/completed"
                                    | "thread/status/changed"
                                    | "thread/closed"
                                    | "serverRequest/resolved"
                            ) && let Some(thread) = params["threadId"].as_str()
                            {
                                self.emit_native_delegate_relatives(thread);
                            }
                        }
                    }
                    Ok(false) => {}
                    Err(error) => {
                        projected = true;
                        self.app_server
                            .view
                            .errors
                            .insert("conversation-event", error);
                        if method == "thread/settings/updated"
                            && let Some(owner) = params
                                .get("threadId")
                                .and_then(Value::as_str)
                                .and_then(|id| self.app_server.conversations.owner_for_thread(id))
                                .map(str::to_owned)
                        {
                            // Invalid settings invalidate the previous report,
                            // not just an error hidden in account Settings.
                            self.emit_app_server_conversation(&owner, "stream");
                        }
                    }
                }
                match method.as_str() {
                    "mcpServer/oauthLogin/completed" => {
                        projected = true;
                        self.app_server.mcp.invalidate_inventory("Native service authentication changed. Refresh the configured-server inventory.");
                        self.app_server.mcp.completed(&params);
                    }
                    "windowsSandbox/setupCompleted" => {
                        projected = true;
                        self.app_server.view.sandbox_setup.completed(&params)
                    }
                    "account/updated" => {
                        projected = true;
                        self.refresh_app_server_account();
                    }
                    "account/rateLimits/updated" => {
                        projected = true;
                        if !params.get("rateLimits").is_some_and(Value::is_object) {
                            self.app_server.view.errors.insert(
                                "rate-limits",
                                "Codex sent an invalid rate-limit update".into(),
                            );
                        } else {
                            self.invalidate_app_server_rate_limits();
                        }
                    }
                    "account/login/completed" => {
                        projected = true;
                        if let Some(id) = params.get("loginId").and_then(Value::as_str) {
                            self.app_server.completed_logins.insert(id.to_owned());
                            if self
                                .app_server
                                .view
                                .login
                                .as_ref()
                                .is_some_and(|login| login.id == id)
                            {
                                self.app_server.view.login = None;
                            }
                            if params.get("success").and_then(Value::as_bool) == Some(false) {
                                self.app_server.view.errors.insert(
                                    "sign-in",
                                    params
                                        .get("error")
                                        .and_then(Value::as_str)
                                        .unwrap_or("Sign-in did not complete.")
                                        .to_owned(),
                                );
                            }
                        }
                        self.refresh_app_server_account();
                    }
                    _ => {}
                }
                if let Some(owner) = owner.as_deref() {
                    if method == "turn/completed"
                        && let Some(turn_id) = params
                            .get("turnId")
                            .and_then(Value::as_str)
                            .or_else(|| params["turn"].get("id").and_then(Value::as_str))
                    {
                        self.complete_supervisor_review(owner, turn_id);
                    }
                    self.note_supervision_notification(owner, &method, &params);
                }
                if !projected {
                    self.app_server.view.diagnostics.record(&method);
                }
            }
            Event::Transport {
                event:
                    TransportEvent::ServerRequest {
                        key,
                        method,
                        params,
                    },
                ..
            } => {
                self.receive_app_server_request(key, &method, params);
            }
            Event::Reply {
                revision,
                query,
                result,
                ..
            } => {
                if let Query::SandboxSetup(mode) = query {
                    self.app_server.view.sandbox_setup.replied(mode, &result);
                    self.app_server.sync_profile();
                    self.start_next_agent_submission_if_idle();
                    self.render_agent_panel();
                    return;
                }
                if query.is_read() && revision != self.app_server.revision {
                    return;
                }
                if matches!(query, Query::ComputerUseSkills) {
                    self.app_server.skills.official_bootstrap_pending = false;
                }
                if !query.is_read()
                    && !matches!(query, Query::ComputerUseSkills | Query::ComputerUseMcp)
                {
                    self.app_server.view.auth_busy = false;
                }
                match result.and_then(|value| {
                    self.apply_app_server_reply(query, value)
                        .map_err(CallError::Rejected)
                }) {
                    Ok(()) => {
                        self.app_server.view.errors.remove(query.key());
                    }
                    Err(error) => {
                        if matches!(query, Query::ComputerUseSkills | Query::ComputerUseMcp) {
                            self.app_server.view.computer_use = "unavailable";
                            self.app_server.view.browser_use = "unavailable";
                        }
                        self.app_server.reads.remove(query.key());
                        let mut retry_rate_limits = false;
                        match query {
                            Query::PermissionProfiles => {
                                self.app_server.account_state.permission_failed()
                            }
                            Query::RateLimits => {
                                retry_rate_limits =
                                    self.app_server.account_state.rate_limits_failed()
                            }
                            Query::ProviderCapabilities => {
                                self.app_server.account_state.provider_capabilities_failed()
                            }
                            Query::AccountUsage => {
                                self.app_server.account_state.account_usage_failed()
                            }
                            Query::WorkspaceMessages => {
                                self.app_server.account_state.workspace_messages_failed()
                            }
                            Query::LoadedThreads => {
                                self.app_server.loaded_catalog.clear();
                                self.app_server.loaded_cursors.clear();
                            }
                            _ => {}
                        }
                        self.app_server
                            .view
                            .errors
                            .insert(query.key(), error.to_string());
                        if retry_rate_limits {
                            self.dispatch_app_server_rate_limits();
                        }
                    }
                }
                self.app_server.view.refreshing = !self.app_server.reads.is_empty();
                if matches!(query, Query::ComputerUseSkills | Query::ComputerUseMcp) {
                    self.emit_other_skill_views("");
                }
            }
        }
        // initialize may finish before the worker publishes the Client handle.
        if self.app_server.initialized
            && self.app_server.client.is_some()
            && !self.app_server.bootstrapped
        {
            self.app_server.bootstrapped = true;
            self.app_server.view.connected = true;
            self.app_server.view.connecting = false;
            self.refresh_app_server_account();
            let official_roots = self
                .app_server
                .computer_use_root
                .iter()
                .chain(self.app_server.browser_use_root.iter())
                .map(|root| root.to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            if !official_roots.is_empty() {
                self.app_server.skills.official_bootstrap_pending = true;
                self.app_server_call(
                    Query::ComputerUseSkills,
                    api::skills_extra_roots(&official_roots),
                );
            }
            self.begin_native_chat_reload();
            let owner = self
                .app_server
                .preparation
                .selected_owner()
                .map(str::to_owned)
                .unwrap_or_else(|| self.composer_owner());
            self.select_native_preparation(owner);
        }
        self.advance_native_chat_reload();
        self.app_server
            .pending_inputs
            .reconcile(&self.app_server.conversations);
        self.app_server.sync_profile();
        self.start_next_agent_submission_if_idle();
        if self.app_server.configuration().0.can_run() {
            self.resume_supervision_loops();
        }
        self.advance_native_preparation();
        self.render_agent_panel();
    }

    fn apply_app_server_reply(&mut self, query: Query, value: Value) -> Result<(), String> {
        match query {
            Query::SandboxSetup(_) => unreachable!("handled by native setup lifecycle"),
            Query::ComputerUseSkills => {
                if !value.as_object().is_some_and(|object| object.is_empty()) {
                    return Err("Unexpected native Computer Use skill acknowledgement".into());
                }
                self.app_server_call(Query::ComputerUseMcp, api::mcp_status(None));
            }
            Query::ComputerUseMcp => {
                let servers = value["data"]
                    .as_array()
                    .ok_or("Invalid native Computer Use server inventory")?;
                if servers.iter().any(|server| {
                    server["name"] == "node_repl" && server["tools"]["js"].is_object()
                }) {
                    if self.app_server.computer_use_root.is_some() {
                        self.app_server.view.computer_use = "available";
                    }
                    if self.app_server.browser_use_root.is_some() {
                        self.app_server.view.browser_use = "available";
                    }
                } else {
                    return Err(
                        "The official Computer Use server is not available to this App Server"
                            .into(),
                    );
                }
            }
            Query::Account => {
                self.app_server.accept_account(&self.data_dir, &value)?;
                self.app_server.reads.remove("account");
                if self
                    .app_server
                    .view
                    .account
                    .as_ref()
                    .is_some_and(|account| account.supported)
                {
                    self.refresh_app_server_rate_limits();
                    self.refresh_app_server_account_extras();
                } else {
                    self.app_server.account_state.clear_rate_limits();
                    self.app_server.account_state.clear_account_extras();
                }
            }
            Query::Models => {
                if let Some(next) = self.app_server.accept_catalog(value)? {
                    self.app_server_call(Query::Models, next);
                }
            }
            Query::ProviderCapabilities => {
                self.app_server
                    .account_state
                    .provider_capabilities(&value)?;
                self.app_server.reads.remove("provider-capabilities");
            }
            Query::Requirements => {
                let requirements = value
                    .get("requirements")
                    .ok_or("Configuration requirements response is incomplete")?;
                if !(requirements.is_null() || requirements.is_object()) {
                    return Err("Invalid configuration requirements".into());
                }
                self.app_server.requirements = Some(requirements.clone());
                self.app_server.account_state.requirements(requirements);
                self.app_server.p2.sync_requirements(Some(requirements));
                self.app_server.view.requirements_loaded = true;
                self.app_server.reads.remove("requirements");
            }
            Query::PermissionProfiles => {
                if let Some(next) = self.app_server.account_state.permission_page(value)? {
                    self.app_server_call(Query::PermissionProfiles, next);
                } else {
                    self.app_server.reads.remove("permission-profiles");
                }
            }
            Query::LoadedThreads => {
                if let Some(next) = self.app_server.accept_loaded_threads(value)? {
                    self.app_server_call(Query::LoadedThreads, next);
                } else {
                    self.app_server.reads.remove("loaded-threads");
                }
            }
            Query::RateLimits => {
                if self.app_server.account_state.rate_limits(&value)? {
                    self.dispatch_app_server_rate_limits();
                } else {
                    self.app_server.reads.remove("rate-limits");
                }
            }
            Query::AccountUsage => {
                self.app_server.account_state.account_usage(&value)?;
                self.app_server.reads.remove("account-usage");
            }
            Query::WorkspaceMessages => {
                self.app_server.account_state.workspace_messages(&value)?;
                self.app_server.reads.remove("workspace-messages");
            }
            Query::Login => {
                let login = match parse_login(&value) {
                    Ok(login) => login,
                    Err(error) => {
                        // Stop an official pending login if its returned URL cannot
                        // safely be offered by this client. Never leave an invisible
                        // sign-in listener as a fallback to invalid configuration.
                        if let Some(id) = value.get("loginId").and_then(Value::as_str) {
                            self.app_server_call(Query::Cancel, api::cancel_login(id));
                            self.app_server.view.auth_busy = true;
                        }
                        return Err(error);
                    }
                };
                if !self.app_server.completed_logins.contains(&login.id) {
                    self.app_server.view.login = Some(login);
                }
            }
            Query::Cancel => {
                self.app_server.view.login = None;
            }
            Query::Logout => {
                self.app_server.view.account = None;
                self.refresh_app_server_account();
            }
        }
        Ok(())
    }
}

fn conversation_failure_belongs_to_submission(method: &str, has_submission: bool) -> bool {
    matches!(method, "thread/start" | "turn/start" | "turn/steer")
        || method == "thread/resume" && has_submission
}

impl State {
    pub(super) fn is_connected(&self) -> bool {
        self.view.connected && self.client.is_some()
    }

    pub(super) fn shutdown_owned_process(&mut self) {
        self.epoch = self.epoch.saturating_add(1);
        if let Some(client) = self.client.take() {
            client.shutdown();
        }
    }

    pub(super) fn load(data_dir: &Path) -> Self {
        let path = data_dir.join("app-server-threads.json");
        // An OS-owned lock, not a PID or persistent sentinel file. Crashes release
        // it automatically. Other providers remain usable in a second instance.
        let lock = (|| -> Result<fs::File, String> {
            let file = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(data_dir.join("app-server-threads.lock"))
                .map_err(|e| e.to_string())?;
            file.try_lock().map_err(|e| format!("Native conversation storage is held by another application instance or cannot be locked: {e}. Close the other instance and reopen Supervisor."))?;
            Ok(file)
        })();
        let lock = match lock {
            Ok(lock) => lock,
            Err(error) => {
                let mut state = Self {
                    binding_store_error: Some(error.clone()),
                    ..Self::default()
                };
                state.view.errors.insert("conversations", error);
                return state;
            }
        };
        let mut state = match read_bindings(&path) {
            Ok(conversations) => Self {
                conversations,
                binding_lock: Some(lock),
                ..Self::default()
            },
            Err(error) => {
                let mut state = Self {
                    binding_store_error: Some(error.clone()),
                    binding_lock: Some(lock),
                    ..Self::default()
                };
                state.view.errors.insert("conversations", error);
                state
            }
        };
        state.event_log = event_log::Journal::load(data_dir);
        state.epoch = state.event_log.next_epoch();
        match fs::read(data_dir.join("app-server-profile.json")) {
            Ok(bytes) => match serde_json::from_slice::<AgentSelection>(&bytes) {
                Ok(selection) => state.profile.selection = selection,
                Err(_) => {
                    state.view.errors.insert("profile", "Saved Codex profile could not be read. Its file was preserved; select a valid model profile to replace it.".into());
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                state.view.errors.insert(
                    "profile",
                    format!("Saved Codex profile could not be read: {error}"),
                );
            }
        }
        state.load_access_preference(data_dir);
        state.load_connection_preference(data_dir);
        state
    }
    fn begin_refresh(&mut self) {
        self.revision += 1;
        self.view.refreshing = true;
        self.view.models.clear();
        self.view.account = None;
        self.view.requirements_loaded = false;
        self.requirements = None;
        self.p2.sync_requirements(None);
        self.account_state.begin_refresh();
        self.catalog.clear();
        self.cursors.clear();
        self.reads = HashSet::from([
            "account",
            "models",
            "provider-capabilities",
            "requirements",
            "permission-profiles",
        ]);
    }

    fn accept_loaded_threads(&mut self, value: Value) -> Result<Option<Call>, String> {
        let data = value
            .get("data")
            .and_then(Value::as_array)
            .ok_or("Loaded-thread response is incomplete")?;
        if self.loaded_catalog.len().saturating_add(data.len()) > 4096 {
            return Err("Loaded-thread inventory is unexpectedly large".into());
        }
        for value in data {
            let id = value
                .as_str()
                .filter(|id| !id.is_empty() && id.len() <= 1024 && !id.contains('\0'))
                .ok_or("Loaded-thread inventory contains an invalid ID")?;
            self.loaded_catalog.insert(id.into());
        }
        let cursor = match value.get("nextCursor") {
            Some(Value::Null) => None,
            Some(Value::String(cursor))
                if !cursor.is_empty() && cursor.len() <= 16 * 1024 && !cursor.contains('\0') =>
            {
                Some(cursor.clone())
            }
            _ => return Err("Loaded-thread pagination is incomplete".into()),
        };
        if let Some(cursor) = cursor {
            if !self.loaded_cursors.insert(cursor.clone()) {
                self.loaded_catalog.clear();
                return Err("Loaded-thread pagination repeated a cursor".into());
            }
            return Ok(Some(api::loaded_threads(Some(&cursor))));
        }
        let loaded = std::mem::take(&mut self.loaded_catalog);
        let owners = self.conversations.reconcile_loaded(&loaded);
        self.loaded_cursors.clear();
        // Reconciliation runs before the provider becomes ready. The next
        // normal panel render publishes the changed owner state atomically.
        let _ = owners;
        Ok(None)
    }

    fn accept_catalog(&mut self, value: Value) -> Result<Option<Call>, String> {
        let (models, cursor) = catalog_page(value)?;
        if self.catalog.len().saturating_add(models.len()) > MAX_MODEL_CATALOG_ENTRIES {
            self.catalog.clear();
            self.cursors.clear();
            return Err("Model catalog is unexpectedly large. Refresh to retry.".into());
        }
        self.catalog.extend(models);
        if let Some(cursor) = cursor {
            if !self.cursors.insert(cursor.clone()) {
                self.catalog.clear();
                return Err("Model catalog repeated a pagination cursor. Refresh to retry.".into());
            }
            Ok(Some(api::model_list(Some(&cursor))))
        } else {
            let mut ids = HashSet::new();
            self.catalog.retain(|model| ids.insert(model.id.clone()));
            self.view.models = std::mem::take(&mut self.catalog);
            self.reads.remove("models");
            Ok(None)
        }
    }
}

fn read_bindings(path: &Path) -> Result<Conversations, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(Conversations::default());
        }
        Err(error) => {
            return Err(format!(
                "The App Server conversation bindings could not be read: {error}. The file was not modified."
            ));
        }
    };
    let saved: Saved = serde_json::from_slice(&bytes).map_err(|_| "The App Server conversation bindings are invalid. The original file was kept; restore a valid backup before starting conversations.".to_owned())?;
    Conversations::restore(saved)
}

fn parse_account(value: &Value) -> Result<Account, String> {
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or("Account response is invalid")?;
    let bounded =
        |value: &str| !value.is_empty() && value.len() <= 16 * 1024 && !value.contains('\0');
    match kind {
        "chatgpt" => {
            let email = match value.get("email") {
                Some(Value::Null) => None,
                Some(Value::String(email)) if bounded(email) => Some(email.clone()),
                _ => return Err("ChatGPT account response is invalid".into()),
            };
            let plan_type = value
                .get("planType")
                .and_then(Value::as_str)
                .filter(|plan| bounded(plan))
                .map(str::to_owned)
                .ok_or("ChatGPT account response is invalid")?;
            Ok(Account {
                kind: kind.into(),
                email,
                plan_type: Some(plan_type),
                supported: true,
                policy: None,
            })
        }
        "apiKey" => Ok(Account {
            kind: kind.into(),
            email: None,
            plan_type: None,
            supported: false,
            policy: Some("Supervisor intentionally supports ChatGPT subscription accounts only. It never reads, stores or forwards a Codex API key; sign in with ChatGPT to run native conversations here.".into()),
        }),
        "amazonBedrock" => {
            if !value
                .get("usesCodexManagedCredentials")
                .is_some_and(Value::is_boolean)
            {
                return Err("Amazon Bedrock account response is invalid".into());
            }
            Ok(Account {
                kind: kind.into(),
                email: None,
                plan_type: None,
                supported: false,
                policy: Some("Supervisor intentionally supports ChatGPT subscription accounts only. It does not access the Amazon Bedrock credential chain; sign in with ChatGPT to run native conversations here.".into()),
            })
        }
        _ => Err("Codex reported an unsupported account type".into()),
    }
}

fn parse_login(value: &Value) -> Result<Login, String> {
    let required = |field| {
        value
            .get(field)
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
            .ok_or("Sign-in response is incomplete")
    };
    let (url, user_code) = match required("type")? {
        "chatgpt" => (required("authUrl")?, None),
        "chatgptDeviceCode" => (
            required("verificationUrl")?,
            Some(required("userCode")?.to_owned()),
        ),
        _ => return Err("Only Codex-managed ChatGPT sign-in is supported".into()),
    };
    let parsed = Url::parse(url).map_err(|_| "Invalid sign-in URL")?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port_or_known_default() != Some(443)
        || !matches!(
            parsed.host_str(),
            Some("auth.openai.com" | "auth0.openai.com" | "chatgpt.com")
        )
    {
        return Err("Codex returned an unexpected sign-in URL; it was not opened.".into());
    }
    Ok(Login {
        id: required("loginId")?.to_owned(),
        url: url.to_owned(),
        user_code,
    })
}

const MAX_MODEL_CATALOG_ENTRIES: usize = 512;
const MAX_MODEL_OPTIONS: usize = 64;
const MAX_MODEL_ID_BYTES: usize = 1024;
const MAX_MODEL_TEXT_BYTES: usize = 16 * 1024;

fn catalog_text(value: &str, maximum: usize, required: bool) -> bool {
    (!required || !value.is_empty()) && value.len() <= maximum && !value.contains('\0')
}

fn catalog_page(value: Value) -> Result<(Vec<AgentModelOption>, Option<String>), String> {
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or("Model catalog response is incomplete")?;
    if data.len() > MAX_MODEL_CATALOG_ENTRIES {
        return Err("Model catalog page is unexpectedly large".into());
    }
    let cursor = match value.get("nextCursor") {
        Some(Value::Null) => None,
        Some(Value::String(cursor)) if catalog_text(cursor, MAX_MODEL_TEXT_BYTES, true) => {
            Some(cursor.clone())
        }
        _ => return Err("Model catalog pagination is incomplete".into()),
    };
    let mut models = Vec::new();
    for raw in data {
        if raw.get("hidden").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let mut model: AgentModelOption =
            serde_json::from_value(raw.clone()).map_err(|_| "Invalid model catalog entry")?;
        let nested_shape_valid = model.supported_reasoning_efforts.len() <= MAX_MODEL_OPTIONS
            && model.service_tiers.len() <= MAX_MODEL_OPTIONS
            && model.input_modalities.len() <= 3
            && model.additional_speed_tiers.len() <= MAX_MODEL_OPTIONS
            && model.supported_reasoning_efforts.iter().all(|option| {
                catalog_text(&option.reasoning_effort, MAX_MODEL_ID_BYTES, true)
                    && catalog_text(&option.description, MAX_MODEL_TEXT_BYTES, false)
            })
            && model.service_tiers.iter().all(|option| {
                catalog_text(&option.id, MAX_MODEL_ID_BYTES, true)
                    && catalog_text(&option.name, MAX_MODEL_TEXT_BYTES, true)
                    && catalog_text(&option.description, MAX_MODEL_TEXT_BYTES, false)
            })
            && model
                .input_modalities
                .iter()
                .all(|modality| matches!(modality.as_str(), "text" | "image" | "audio"))
            && model
                .additional_speed_tiers
                .iter()
                .all(|tier| catalog_text(tier, MAX_MODEL_ID_BYTES, true));
        let scalar_shape_valid = catalog_text(&model.id, MAX_MODEL_ID_BYTES, true)
            && catalog_text(&model.model, MAX_MODEL_ID_BYTES, true)
            && catalog_text(&model.display_name, MAX_MODEL_TEXT_BYTES, true)
            && catalog_text(&model.description, MAX_MODEL_TEXT_BYTES, false)
            && catalog_text(&model.default_reasoning_effort, MAX_MODEL_ID_BYTES, false)
            && model
                .upgrade
                .as_deref()
                .is_none_or(|value| catalog_text(value, MAX_MODEL_ID_BYTES, true))
            && model
                .default_service_tier
                .as_deref()
                .is_none_or(|value| catalog_text(value, MAX_MODEL_ID_BYTES, true));
        let upgrade_shape_valid = model.upgrade_info.as_ref().is_none_or(|upgrade| {
            catalog_text(&upgrade.model, MAX_MODEL_ID_BYTES, true)
                && upgrade
                    .upgrade_copy
                    .as_deref()
                    .is_none_or(|value| catalog_text(value, MAX_MODEL_TEXT_BYTES, false))
        });
        if !scalar_shape_valid
            || !nested_shape_valid
            || !upgrade_shape_valid
            || !raw
                .get("supportedReasoningEfforts")
                .is_some_and(Value::is_array)
            || !raw.get("serviceTiers").is_some_and(Value::is_array)
        {
            return Err("Model catalog entry lacks required profile fields".into());
        }
        if let Some(upgrade) = &mut model.upgrade_info {
            // These official fields are presentation links/rich prose, not native
            // execution authority. Supervisor publishes only the bounded target
            // and plain upgrade copy used by its explicit model choice surface.
            upgrade.model_link = None;
            upgrade.migration_markdown = None;
        }
        models.push(model);
    }
    Ok((models, cursor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_resume_rejection_is_not_treated_as_a_failed_prompt_submission() {
        assert!(!conversation_failure_belongs_to_submission(
            "thread/resume",
            false
        ));
        assert!(conversation_failure_belongs_to_submission(
            "thread/resume",
            true
        ));
        assert!(conversation_failure_belongs_to_submission(
            "thread/start",
            false
        ));
        assert!(conversation_failure_belongs_to_submission(
            "turn/steer",
            false
        ));
    }

    #[test]
    fn displayed_and_canonical_local_paths_have_the_same_identity() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = fs::canonicalize(temp.path()).unwrap();
        let displayed = PathBuf::from(display_path(&canonical));
        assert!(same_local_path(&canonical, &displayed));
    }

    #[test]
    fn native_storage_lock_is_process_owned_not_a_permanent_sentinel() {
        let temp = tempfile::tempdir().unwrap();
        let first = State::load(temp.path());
        assert!(first.binding_store_error.is_none());
        let second = State::load(temp.path());
        assert!(second.binding_store_error.is_some());
        drop(first);
        let third = State::load(temp.path());
        assert!(third.binding_store_error.is_none());
        assert!(temp.path().join("app-server-threads.lock").exists());
    }
    #[test]
    fn binding_load_retains_native_ids_and_never_overwrites_corruption() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("app-server-threads.json");
        assert!(read_bindings(&path).unwrap().saved().bindings.is_empty());
        fs::write(&path, b"not valid JSON").unwrap();
        assert!(State::load(temp.path()).binding_store_error.is_some());
        assert_eq!(fs::read(&path).unwrap(), b"not valid JSON");
        let json = json!({"version":1,"bindings":{"graph:n":{"threadId":"native-id","sessionId":"native-session","archived":false}},"unresolved":{}});
        write_file_atomically::<Saved>(&path, &serde_json::to_vec(&json).unwrap()).unwrap();
        let state = State::load(temp.path());
        assert_eq!(
            state
                .conversations
                .binding("graph:n")
                .unwrap()
                .session_id
                .as_deref(),
            Some("native-session")
        );
        assert!(state.binding_store_error.is_none());
    }
    #[test]
    fn account_ipc_does_not_accept_raw_rpc_or_authentication_tokens() {
        let valid = r#"{"message_type":"app_server_account","action":"login_device"}"#;
        assert!(serde_json::from_str::<AgentPanelMessage>(valid).is_ok());
        for raw in [
            r#"{"message_type":"app_server_account","action":"account/logout"}"#,
            r#"{"message_type":"app_server_account","action":"login_device","accessToken":"not-accepted"}"#,
            r#"{"message_type":"app_server_account","action":"open_login","url":"https://example.com"}"#,
        ] {
            assert!(serde_json::from_str::<AgentPanelMessage>(raw).is_err());
        }
    }
    #[test]
    fn conversation_ipc_accepts_local_owners_not_native_ids_or_arbitrary_rpc() {
        assert!(serde_json::from_str::<AgentPanelMessage>(r#"{"message_type":"app_server_conversation","owner":"graph:n","action":{"kind":"read"}}"#).is_ok());
        for raw in [
            r#"{"message_type":"app_server_conversation","owner":"chat:a","action":{"kind":"read","threadId":"personal-native-thread"}}"#,
            r#"{"message_type":"app_server_conversation","owner":"chat:a","action":{"kind":"open","cwd":"C:\\private"}}"#,
            r#"{"message_type":"app_server_conversation","owner":"chat:a","action":{"kind":"turn/start","input":[]}}"#,
        ] {
            assert!(serde_json::from_str::<AgentPanelMessage>(raw).is_err());
        }
    }
    fn model(id: &str) -> Value {
        json!({"id":id,"model":id,"displayName":id,"supportedReasoningEfforts":[],"serviceTiers":[]})
    }
    #[test]
    fn catalog_publishes_all_pages_atomically_and_deduplicates() {
        let mut state = State::default();
        state.begin_refresh();
        let next = state
            .accept_catalog(json!({"data":[model("one")],"nextCursor":"two"}))
            .unwrap()
            .unwrap();
        assert_eq!(next.params["cursor"], "two");
        assert!(state.view.models.is_empty());
        state
            .accept_catalog(json!({"data":[model("one"),model("two")],"nextCursor":null}))
            .unwrap();
        assert_eq!(state.view.models.len(), 2);
        assert!(!state.reads.contains("models"));
        assert!(state.reads.contains("requirements"));
    }
    #[test]
    fn refresh_invalidates_old_catalog_and_permission_state() {
        let mut state = State::default();
        state.begin_refresh();
        let old_revision = state.revision;
        state.view.requirements_loaded = true;
        state.requirements = Some(json!({"allowedSandboxModes":["read-only"]}));
        state
            .accept_catalog(json!({"data":[model("one")],"nextCursor":null}))
            .unwrap();
        state.begin_refresh();
        assert_ne!(state.revision, old_revision);
        assert!(state.view.models.is_empty());
        assert!(!state.view.requirements_loaded);
        assert!(state.requirements.is_none());
        state
            .accept_catalog(json!({"data":[model("one")],"nextCursor":"same"}))
            .unwrap();
        assert!(
            state
                .accept_catalog(json!({"data":[model("two")],"nextCursor":"same"}))
                .is_err()
        );
        assert!(state.view.models.is_empty());
        assert!(state.catalog.is_empty());
    }
    #[test]
    fn loaded_thread_pages_publish_atomically_and_only_reconcile_owned_ids() {
        let saved: Saved = serde_json::from_value(json!({
            "version":1,
            "bindings":{"chat:a":{"threadId":"owned","sessionId":null,"archived":false,"deleted":false,"neverSubmitted":false}},
            "unresolved":{},
            "forkOrigins":{}
        }))
        .unwrap();
        let mut state = State {
            conversations: Conversations::restore(saved).unwrap(),
            ..State::default()
        };
        let next = state
            .accept_loaded_threads(json!({"data":["owned"],"nextCursor":"page-two"}))
            .unwrap()
            .unwrap();
        assert_eq!(next.method, "thread/loaded/list");
        assert_eq!(next.params["cursor"], "page-two");
        assert!(!state.conversations.is_thread_loaded("chat:a"));

        state
            .accept_loaded_threads(json!({"data":["unowned"],"nextCursor":null}))
            .unwrap();
        assert!(state.conversations.is_thread_loaded("chat:a"));
        assert!(state.conversations.owner_for_thread("unowned").is_none());
        state
            .accept_loaded_threads(json!({"data":[],"nextCursor":null}))
            .unwrap();
        assert!(!state.conversations.is_thread_loaded("chat:a"));

        state
            .accept_loaded_threads(json!({"data":[],"nextCursor":"repeat"}))
            .unwrap();
        assert!(
            state
                .accept_loaded_threads(json!({"data":[],"nextCursor":"repeat"}))
                .is_err()
        );
        assert!(state.loaded_catalog.is_empty());
    }
    #[test]
    fn login_only_accepts_managed_https_identity_origins() {
        for url in [
            "file:///C:/anything",
            "https://auth.openai.com.evil.example/",
            "http://auth.openai.com/",
            "https://user@auth.openai.com/",
            "https://auth.openai.com:444/",
        ] {
            assert!(parse_login(&json!({"type":"chatgpt","loginId":"l1","authUrl":url})).is_err());
        }
        let login = parse_login(&json!({"type":"chatgptDeviceCode","loginId":"l1","verificationUrl":"https://auth.openai.com/codex/device","userCode":"TEST-CODE"})).unwrap();
        assert_eq!(login.user_code.as_deref(), Some("TEST-CODE"));
        assert!(serde_json::to_value(login).unwrap().get("id").is_none());
        assert!(parse_login(&json!({"type":"apiKey"})).is_err());
    }
    #[test]
    fn catalog_never_invents_models_or_accepts_incomplete_pagination() {
        assert!(catalog_page(json!({"data":[]})).is_err());
        assert!(catalog_page(json!({"data":[{"model":"fake"}],"nextCursor":null})).is_err());
        let page = catalog_page(json!({"data":[{"id":"m1","model":"model-1","displayName":"Model 1",
            "supportedReasoningEfforts":[{"reasoningEffort":"high","description":"High"}],
            "serviceTiers":[{"id":"fast","name":"Fast","description":"Catalog fast"}]}],"nextCursor":"next"})).unwrap();
        assert_eq!(page.0[0].model, "model-1");
        assert_eq!(page.0[0].service_tiers[0].id, "fast");
        assert_eq!(page.1.as_deref(), Some("next"));

        let page = catalog_page(json!({"data":[{
            "id":"m2","model":"model-2","displayName":"Model 2",
            "supportsPersonality":true,"inputModalities":["text","audio"],
            "supportedReasoningEfforts":[],"serviceTiers":[],
            "upgradeInfo":{"model":"model-3","upgradeCopy":"Upgrade explicitly",
                "modelLink":"https://private.example/model","migrationMarkdown":"PRIVATE"}
        }],"nextCursor":null}))
        .unwrap();
        assert!(page.0[0].supports_personality);
        assert!(page.0[0].supports_audio_input());
        let encoded = serde_json::to_string(&page.0).unwrap();
        assert!(encoded.contains("Upgrade explicitly"));
        assert!(!encoded.contains("private.example"));
        assert!(!encoded.contains("PRIVATE"));

        assert!(
            catalog_page(json!({"data":[{
            "id":"bad","model":"bad","displayName":"Bad",
            "inputModalities":["executable"],
            "supportedReasoningEfforts":[],"serviceTiers":[]
        }],"nextCursor":null}))
            .is_err()
        );
        assert!(
            catalog_page(json!({"data":[],"nextCursor":"x".repeat(MAX_MODEL_TEXT_BYTES + 1)}))
                .is_err()
        );
    }
    #[test]
    fn account_view_does_not_include_unexpected_secrets() {
        let account = parse_account(&json!({
            "type":"chatgpt",
            "email":null,
            "planType":"pro",
            "accessToken":"not-for-ui"
        }))
        .unwrap();
        assert!(
            !serde_json::to_string(&account)
                .unwrap()
                .contains("not-for-ui")
        );
        assert!(View::default().account.is_none());

        let api_key = parse_account(&json!({"type":"apiKey","apiKey":"PRIVATE"})).unwrap();
        assert!(!api_key.supported);
        assert!(api_key.policy.as_deref().unwrap().contains("subscription"));
        assert!(!serde_json::to_string(&api_key).unwrap().contains("PRIVATE"));

        let bedrock = parse_account(&json!({
            "type":"amazonBedrock",
            "usesCodexManagedCredentials":false,
            "credentials":"PRIVATE"
        }))
        .unwrap();
        assert!(!bedrock.supported);
        assert!(bedrock.policy.as_deref().unwrap().contains("Bedrock"));
        assert!(!serde_json::to_string(&bedrock).unwrap().contains("PRIVATE"));
        assert!(parse_account(&json!({"type":"chatgpt","email":null})).is_err());
    }
}
