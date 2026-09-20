use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::permissions::{ActionAuthorization, ActionEffect, CapabilityScope, PermissionMode};
use crate::provider::AgentProviderKind;
use crate::remote_desktop::RemoteDesktopProtocol;

const MAX_REQUEST_ID_LEN: usize = 96;
const MAX_ADDRESS_LEN: usize = 4_096;
const MAX_TARGET_LEN: usize = 96;
const MAX_TEXT_LEN: usize = 10_000;
pub(crate) const MAX_TERMINAL_INPUT_LEN: usize = 16_000;
const MAX_WORKSPACE_PATH_CHARS: usize = 1_024;
const MAX_WORKSPACE_QUERY_CHARS: usize = 240;
const MAX_WORKSPACE_PATCH_CHARS: usize = 512_000;
const MAX_WORKSPACE_REPLACEMENTS: usize = 16;
const MAX_WINDOW_REF_LEN: usize = 96;
const MAX_UI_ELEMENT_REF_LEN: usize = 96;
const MAX_UI_TEXT_LEN: usize = 4_000;
const MAX_REMOTE_DESKTOP_TEXT_LEN: usize = 4_000;
const MAX_REMOTE_DESKTOP_KEY_CODE_LEN: usize = 32;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TerminalDock {
    Right,
    #[default]
    Bottom,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentSubmissionDelivery {
    #[default]
    Start,
    Steer,
    Queue,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BrowserCommandRequest {
    pub request_id: String,
    pub command: BrowserCommand,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "message_type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum AgentPanelMessage {
    ComposerDraft {
        owner: String,
        request_id: String,
        action: crate::browser::composer_drafts::Action,
    },
    ProjectDiff {
        owner: String,
        view_id: String,
        request_id: String,
        action: crate::browser::project_diff::Action,
    },

    Editor {
        action: crate::file_editor::EditorAction,
    },
    CloseSettings,
    SettingsSurfaceReady,
    SettingsCoverReady,
    SettingsCloseLayoutReady,
    #[serde(rename = "open_knowledge_graph", alias = "open_agent_graph")]
    OpenAgentGraph,
    OpenProjectRegistry,
    #[serde(rename = "close_knowledge_graph", alias = "close_agent_graph")]
    CloseAgentGraph,
    #[serde(
        rename = "begin_knowledge_orb_drag",
        alias = "begin_agent_graph_launcher_drag"
    )]
    BeginAgentGraphLauncherDrag {
        screen_x: f64,
        screen_y: f64,
    },
    #[serde(
        rename = "knowledge_orb_drag_surface_ready",
        alias = "agent_graph_launcher_drag_surface_ready"
    )]
    AgentGraphLauncherDragSurfaceReady,
    #[serde(
        rename = "end_knowledge_orb_drag",
        alias = "end_agent_graph_launcher_drag"
    )]
    EndAgentGraphLauncherDrag {
        screen_x: f64,
        screen_y: f64,
        open_graph: bool,
    },
    Execute {
        request: BrowserCommandRequest,
    },
    SubmitChat {
        message: String,
        #[serde(default)]
        resume: bool,
        #[serde(default)]
        timing: Option<crate::browser::app_server::delivery::ClientTiming>,
        #[serde(default)]
        delivery: AgentSubmissionDelivery,
        #[serde(default)]
        tab_ids: Vec<u64>,
        #[serde(default)]
        terminal_session_ids: Vec<u64>,
        #[serde(default)]
        file_ids: Vec<String>,
    },
    CancelPendingAgentSubmission,
    ClearAgentSubmissionQueue,
    DiscardFailedCheckpoint {
        run_id: u64,
    },
    SelectChatFiles,
    RemoveChatFile {
        file_id: String,
    },
    OpenArtifact {
        artifact_id: String,
    },
    SaveArtifact {
        artifact_id: String,
    },
    ShowArtifactInFolder {
        artifact_id: String,
    },
    CaptureTabContext {
        tab_id: u64,
    },
    RefreshTabContext {
        tab_id: u64,
    },
    RemoveTabContext {
        tab_id: u64,
    },
    CaptureTerminalContext {
        session_id: u64,
    },
    RefreshTerminalContext {
        session_id: u64,
    },
    RemoveTerminalContext {
        session_id: u64,
    },
    SetTerminalContextFollow {
        session_id: u64,
        enabled: bool,
    },

    RetryClaudeProvider,
    AppServerAccount {
        action: crate::browser::app_server::AccountAction,
    },
    AppServerMcp {
        action: crate::browser::app_server::McpAction,
    },
    AppServerP2 {
        action: crate::browser::app_server::P2Action,
    },
    AppServerHistory {
        owner: String,
        action: crate::browser::app_server::HistoryAction,
    },
    AppServerSkills {
        owner: String,
        action: crate::browser::app_server::SkillsAction,
    },
    AppServerPreferences {
        owner: String,
        action: crate::browser::app_server::PreferencesAction,
    },
    AppServerConversation {
        owner: String,
        action: crate::browser::app_server::ConversationAction,
    },
    AppServerRequest {
        owner: String,
        action: crate::browser::app_server::RequestAction,
    },
    AppServerAccess {
        owner: String,
        action: crate::browser::app_server::AccessAction,
    },
    AppServerSteer {
        owner: String,
        input: crate::browser::app_server::SteerInput,
    },
    AppServerPrepare {
        owner: String,
    },
    ConnectClaudeProvider,
    ResetClaudeSession,
    RetrySubscriptionProvider {
        provider: AgentProviderKind,
    },
    ConnectSubscriptionProvider {
        provider: AgentProviderKind,
    },
    SetAgentProvider {
        provider: AgentProviderKind,
    },
    RetrySshClient,
    SaveSshProfile {
        #[serde(default)]
        id: Option<String>,
        name: String,
        host: String,
        port: u16,
        username: String,
        #[serde(default)]
        identity_file: Option<String>,
        agent_enabled: bool,
    },
    DeleteSshProfile {
        profile_id: String,
    },
    OpenSshTerminal {
        profile_id: String,
    },
    OpenRemoteDesktop {
        profile_id: String,
        protocol: RemoteDesktopProtocol,
        remote_port: u16,
    },
    SetSshSessionAgentReady {
        session_id: u64,
        ready: bool,
    },

    SetAgentConfiguration {
        model: String,
        effort: String,
        #[serde(default)]
        service_tier: Option<String>,
        #[serde(default)]
        context_window: Option<u64>,
        #[serde(default)]
        personality: Option<String>,
    },
    OpenTerminal,
    CloseTerminal {
        session_id: u64,
    },
    ActivateTerminal {
        session_id: u64,
    },
    TerminalWrite {
        session_id: u64,
        data: String,
    },
    ResizeTerminal {
        session_id: u64,
        rows: u16,
        cols: u16,
    },
    ReorderTerminal {
        session_id: u64,
        #[serde(default)]
        before_session_id: Option<u64>,
    },
    SetTerminalDock {
        dock: TerminalDock,
    },
    SetTerminalPanelVisible {
        visible: bool,
    },
    SetAgentPanelWidth {
        width: f64,
        #[serde(default)]
        persist: bool,
    },
    DetachAgentPanel,
    AttachAgentPanel,
    DetachTerminalPanel {
        #[serde(default)]
        screen_x: Option<i32>,
        #[serde(default)]
        screen_y: Option<i32>,
    },
    AttachTerminalPanel,
    SetTheme {
        theme: String,
    },
    SetSearchEngine {
        search_engine: String,
    },
    SetAuditRetention {
        days: u64,
    },
    SelectWorkspace,
    CreateLocalProject {
        name: String,
    },
    ProjectBoard {
        action: crate::browser::project_board::Action,
    },
    CloneRepository {
        repository: String,
    },
    ConnectSshProject {
        profile_id: String,
        directory: String,
        #[serde(default)]
        name: Option<String>,
    },
    SetRemoteProjectPinned {
        id: String,
        pinned: bool,
    },
    RemoveRemoteProject {
        id: String,
    },
    OpenProject {
        source: String,
        id: String,
    },
    OpenProjectTerminal {
        source: String,
        id: String,
        #[serde(default)]
        git: bool,
    },
    ShowProjectFolder {
        root: String,
    },
    RefreshProjectMetadata {
        source: String,
        id: String,
    },
    SetReopenLastProject {
        enabled: bool,
    },
    ActivateWorkspace {
        root: String,
    },
    CreateProjectChat {
        root: String,
    },
    RenameWorkspace {
        root: String,
        name: String,
    },
    SetWorkspacePinned {
        root: String,
        pinned: bool,
    },
    ActivateProjectChat {
        chat_id: String,
    },
    RenameProjectChat {
        chat_id: String,
        title: String,
    },
    MoveProjectChat {
        chat_id: String,
        project_root: String,
    },
    SelectProjectForChat {
        chat_id: String,
    },
    SetProjectChatPinned {
        chat_id: String,
        pinned: bool,
    },
    SetProjectChatArchived {
        chat_id: String,
        archived: bool,
    },
    SetProjectChatsArchived {
        root: String,
        archived: bool,
    },
    DeleteProjectChat {
        chat_id: String,
    },
    SetWorkspaceArchived {
        root: String,
        archived: bool,
    },
    EjectWorkspace {
        root: String,
        #[serde(default)]
        stop_active_runs: bool,
    },
    ClearWorkspace,
    RefreshWorkspaceExplorer,
    CreateWorkspaceFile {
        path: String,
    },
    CreateWorkspaceDirectory {
        path: String,
    },
    ImportWorkspaceFiles {
        #[serde(default)]
        directory: String,
    },
    CancelManagedProcess {
        process_id: u64,
    },
    OpenProcessPreview {
        process_id: u64,
        url: String,
    },
    SetWindowAccess {
        enabled: bool,
    },
    RefreshWindows,
    CaptureWindow {
        window_id: String,
    },
    SetPreviewLive {
        enabled: bool,
    },
    DetachPreview,
    AttachPreview,
    ClosePreview,
    ResumeComputerControl,
    #[serde(rename = "open_knowledge", alias = "open_agent_graph_node")]
    OpenAgentGraphNode {
        record_type: String,
        id: String,
    },
    #[serde(rename = "set_knowledge_agent", alias = "set_agent_graph_agent")]
    SetAgentGraphAgent {
        record_type: String,
        id: String,
        #[serde(default)]
        conversation_id: Option<uuid::Uuid>,
        name: String,
        mission: String,
        #[serde(default)]
        provider: Option<AgentProviderKind>,
        model: Option<String>,
        effort: Option<String>,
        service_tier: Option<String>,
        #[serde(default)]
        context_window: Option<u64>,
    },
    #[serde(rename = "set_knowledge_agent_link", alias = "set_agent_graph_link")]
    SetAgentGraphLink {
        source_record_type: String,
        source_id: String,
        #[serde(default)]
        source_conversation_id: Option<uuid::Uuid>,
        target_record_type: String,
        target_id: String,
        #[serde(default)]
        target_conversation_id: Option<uuid::Uuid>,
        linked: bool,
    },
    #[serde(rename = "remove_knowledge_agent", alias = "remove_agent_graph_agent")]
    RemoveAgentGraphAgent {
        record_type: String,
        id: String,
        #[serde(default)]
        conversation_id: Option<uuid::Uuid>,
    },
    #[serde(rename = "run_knowledge_agent", alias = "run_agent_graph_agent")]
    RunAgentGraphAgent {
        record_type: String,
        id: String,
        #[serde(default)]
        conversation_id: Option<uuid::Uuid>,
    },
    #[serde(
        rename = "continue_knowledge_agent",
        alias = "continue_agent_graph_agent"
    )]
    ContinueAgentGraphAgent {
        record_type: String,
        id: String,
        #[serde(default)]
        conversation_id: Option<uuid::Uuid>,
        message: String,
        #[serde(default)]
        timing: Option<crate::browser::app_server::delivery::ClientTiming>,
        #[serde(default)]
        delivery: AgentSubmissionDelivery,
        #[serde(default)]
        file_ids: Vec<String>,
        #[serde(default)]
        tab_ids: Vec<u64>,
        #[serde(default)]
        terminal_session_ids: Vec<u64>,
    },
    GraphContexts {
        owner: String,
        action: crate::browser::graph_contexts::Action,
    },
    GraphFiles {
        owner: String,
        action: crate::browser::graph_files::Action,
    },
    #[serde(
        rename = "clear_knowledge_agent_queue",
        alias = "clear_agent_graph_queue"
    )]
    ClearAgentGraphQueue {
        record_type: String,
        id: String,
        #[serde(default)]
        conversation_id: Option<uuid::Uuid>,
    },
    #[serde(
        rename = "load_knowledge_agent_history",
        alias = "load_agent_graph_history"
    )]
    LoadAgentGraphHistory {
        record_type: String,
        id: String,
        #[serde(default)]
        conversation_id: Option<uuid::Uuid>,
        before: usize,
    },
    #[serde(rename = "stop_knowledge_agent", alias = "stop_agent_graph_agent")]
    StopAgentGraphAgent {
        record_type: String,
        id: String,
        #[serde(default)]
        conversation_id: Option<uuid::Uuid>,
    },
    OpenCheckpoint {
        checkpoint_id: String,
        #[serde(default)]
        path: Option<String>,
    },
    SelectCheckpointFile {
        checkpoint_id: String,
        path: String,
    },
    CloseCheckpoint,
    RestoreCheckpoint {
        checkpoint_id: String,
    },
    RestoreCheckpointFile {
        checkpoint_id: String,
        path: String,
    },
    StopAgent,
    SetPermissionMode {
        mode: PermissionMode,
    },
    AuthorizeSession,
    ApproveAction {
        request_id: String,
    },
    DenyAction {
        request_id: String,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct AgentCommandRequest {
    pub request_id: String,
    pub command: AgentCommand,
}

#[derive(Clone, Debug)]
// Provider-neutral native API retained independently of provider availability.
#[allow(dead_code)]
pub(crate) enum AgentCommand {
    Browser(BrowserCommand),
    Terminal(TerminalCommand),
    Ssh(SshCommand),
    Workspace(WorkspaceCommand),
    Process(ProcessCommand),
    Window(WindowCommand),
    Capture(CaptureCommand),
    UiAutomation(UiAutomationCommand),
    RemoteDesktop(RemoteDesktopCommand),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RemoteDesktopPointerButton {
    Left,
    Middle,
    Right,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RemoteDesktopModifier {
    Control,
    Alt,
    Shift,
    Meta,
}

#[derive(Clone, Debug)]
// Provider-neutral native API retained independently of provider availability.
#[allow(dead_code)]
pub(crate) enum RemoteDesktopCommand {
    Observe,
    MovePointer {
        x: u16,
        y: u16,
    },
    Click {
        x: u16,
        y: u16,
        button: RemoteDesktopPointerButton,
    },
    Scroll {
        x: u16,
        y: u16,
        delta_y: i32,
    },
    Key {
        code: String,
        keysym: u32,
        modifiers: Vec<RemoteDesktopModifier>,
    },
    TypeText {
        text: String,
    },
}

#[derive(Clone, Debug)]
// Provider-neutral native API retained independently of provider availability.
#[allow(dead_code)]
pub(crate) enum TerminalCommand {
    Run { command: String },
}

#[derive(Clone, Debug)]
// Provider-neutral native API retained independently of provider availability.
#[allow(dead_code)]
pub(crate) enum SshCommand {
    Run {
        profile_id: String,
        profile_name: String,
        command: String,
    },
}

#[derive(Clone, Debug)]
// Provider-neutral native API retained independently of provider availability.
#[allow(dead_code)]
pub(crate) enum WorkspaceCommand {
    State,
    List {
        path: Option<String>,
        depth: u8,
        max_entries: usize,
    },
    Read {
        path: String,
        start_line: usize,
        line_count: usize,
    },
    Search {
        query: String,
        path: Option<String>,
        case_sensitive: bool,
        max_results: usize,
    },
    ApplyPatch {
        path: String,
        expected_sha256: Option<String>,
        replacements: Vec<TextReplacement>,
        create_content: Option<String>,
    },
    Run {
        command: String,
        path: Option<String>,
    },
}

#[derive(Clone, Debug)]
// Provider-neutral native API retained independently of provider availability.
#[allow(dead_code)]
pub(crate) enum ProcessCommand {
    List,
    Status {
        process_id: u64,
        stdout_offset: u64,
        stderr_offset: u64,
    },
    Start {
        command: String,
        path: Option<String>,
    },
    WriteStdin {
        process_id: u64,
        data: String,
        close: bool,
    },
    Cancel {
        process_id: u64,
    },
}

#[derive(Clone, Debug)]
// Provider-neutral native API retained independently of provider availability.
#[allow(dead_code)]
pub(crate) enum WindowCommand {
    List,
    Focus {
        window_id: String,
    },
    MoveResize {
        window_id: String,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    },
    Minimize {
        window_id: String,
    },
    Restore {
        window_id: String,
    },
    Close {
        window_id: String,
    },
}

#[derive(Clone, Debug)]
// Provider-neutral native API retained independently of provider availability.
#[allow(dead_code)]
pub(crate) enum CaptureCommand {
    Window { window_id: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UiScrollAmount {
    LargeDecrement,
    SmallDecrement,
    SmallIncrement,
    LargeIncrement,
}

#[derive(Clone, Debug)]
// Provider-neutral native API retained independently of provider availability.
#[allow(dead_code)]
pub(crate) enum UiAutomationCommand {
    Inspect {
        window_id: String,
    },
    Focus {
        element_id: String,
    },
    Invoke {
        element_id: String,
    },
    SetValue {
        element_id: String,
        value: String,
    },
    Select {
        element_id: String,
    },
    Expand {
        element_id: String,
        expanded: bool,
    },
    Scroll {
        element_id: String,
        horizontal: Option<UiScrollAmount>,
        vertical: Option<UiScrollAmount>,
    },
    TextInput {
        element_id: String,
        text: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TextReplacement {
    pub old_text: String,
    pub new_text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum BrowserCommand {
    GetState,
    InspectPage,
    Navigate {
        value: String,
    },
    Back,
    Forward,
    Reload,
    Stop,
    Home,
    NewTab {
        #[serde(default)]
        url: Option<String>,
    },
    CloseTab {
        tab_id: u64,
    },
    ActivateTab {
        tab_id: u64,
    },
    Click {
        target: String,
    },
    SetText {
        target: String,
        value: String,
    },
    Scroll {
        delta_y: i32,
    },
}

impl BrowserCommandRequest {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        let request_id = self.request_id.trim();
        if request_id.is_empty() || request_id.len() > MAX_REQUEST_ID_LEN {
            return Err("invalid request_id");
        }

        match &self.command {
            BrowserCommand::Navigate { value }
                if value.trim().is_empty() || value.len() > MAX_ADDRESS_LEN =>
            {
                Err("invalid address")
            }
            BrowserCommand::NewTab { url: Some(url) }
                if url.trim().is_empty() || url.len() > MAX_ADDRESS_LEN =>
            {
                Err("invalid new tab URL")
            }
            BrowserCommand::Click { target }
                if target.trim().is_empty() || target.len() > MAX_TARGET_LEN =>
            {
                Err("invalid element reference")
            }
            BrowserCommand::SetText { target, value }
                if target.trim().is_empty()
                    || target.len() > MAX_TARGET_LEN
                    || value.len() > MAX_TEXT_LEN =>
            {
                Err("invalid text or element reference")
            }
            BrowserCommand::Scroll { delta_y } if !(-4_000..=4_000).contains(delta_y) => {
                Err("invalid scroll distance")
            }
            _ => Ok(()),
        }
    }
}

impl From<BrowserCommandRequest> for AgentCommandRequest {
    fn from(request: BrowserCommandRequest) -> Self {
        Self {
            request_id: request.request_id,
            command: AgentCommand::Browser(request.command),
        }
    }
}

impl AgentCommandRequest {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        let request_id = self.request_id.trim();
        if request_id.is_empty() || request_id.len() > MAX_REQUEST_ID_LEN {
            return Err("invalid request_id");
        }
        self.command.validate()
    }
}

impl AgentCommand {
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::Browser(command) => BrowserCommandRequest {
                request_id: "validation".to_owned(),
                command: command.clone(),
            }
            .validate(),
            Self::Terminal(command) => command.validate(),
            Self::Ssh(command) => command.validate(),
            Self::Workspace(command) => command.validate(),
            Self::Process(command) => command.validate(),
            Self::Window(command) => command.validate(),
            Self::Capture(command) => command.validate(),
            Self::UiAutomation(command) => command.validate(),
            Self::RemoteDesktop(command) => command.validate(),
        }
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Browser(command) => command.name(),
            Self::Terminal(command) => command.name(),
            Self::Ssh(command) => command.name(),
            Self::Workspace(command) => command.name(),
            Self::Process(command) => command.name(),
            Self::Window(command) => command.name(),
            Self::Capture(command) => command.name(),
            Self::UiAutomation(command) => command.name(),
            Self::RemoteDesktop(command) => command.name(),
        }
    }

    pub(crate) fn summary(&self) -> String {
        match self {
            Self::Browser(command) => command.summary(),
            Self::Terminal(command) => command.summary(),
            Self::Ssh(command) => command.summary(),
            Self::Workspace(command) => command.summary(),
            Self::Process(command) => command.summary(),
            Self::Window(command) => command.summary(),
            Self::Capture(command) => command.summary(),
            Self::UiAutomation(command) => command.summary(),
            Self::RemoteDesktop(command) => command.summary(),
        }
    }

    pub(crate) fn requires_approval(&self) -> bool {
        !matches!(
            self,
            Self::Workspace(
                WorkspaceCommand::State
                    | WorkspaceCommand::List { .. }
                    | WorkspaceCommand::Read { .. }
                    | WorkspaceCommand::Search { .. }
            ) | Self::Process(ProcessCommand::List | ProcessCommand::Status { .. })
                | Self::Window(WindowCommand::List)
        )
    }

    pub(crate) fn authorization(&self) -> ActionAuthorization {
        match self {
            Self::Browser(command) => command.authorization(),
            Self::Terminal(TerminalCommand::Run { command }) => ActionAuthorization::new(
                CapabilityScope::Terminal,
                if shell_command_is_obviously_read_only(command) {
                    ActionEffect::ReadOnly
                } else {
                    // An arbitrary shell can delete or overwrite data through aliases,
                    // scripts, interpreters, or child processes. Treat it as potentially
                    // destructive instead of trying to prove safety with a blacklist.
                    ActionEffect::Destructive
                },
            ),
            Self::Ssh(_) => {
                ActionAuthorization::new(CapabilityScope::Ssh, ActionEffect::Destructive)
            }
            Self::Workspace(command) => command.authorization(),
            Self::Process(command) => command.authorization(),
            Self::Window(command) => command.authorization(),
            Self::Capture(command) => command.authorization(),
            Self::UiAutomation(command) => command.authorization(),
            Self::RemoteDesktop(command) => command.authorization(),
        }
    }

    pub(crate) fn may_mutate_workspace(&self) -> bool {
        match self {
            Self::Terminal(TerminalCommand::Run { command })
            | Self::Workspace(WorkspaceCommand::Run { command, .. })
            | Self::Process(ProcessCommand::Start { command, .. }) => {
                !shell_command_is_obviously_read_only(command)
            }
            Self::Workspace(WorkspaceCommand::ApplyPatch { .. })
            | Self::Process(ProcessCommand::WriteStdin { .. }) => true,
            _ => false,
        }
    }
}

pub(crate) fn shell_command_is_obviously_read_only(command: &str) -> bool {
    let command = command.trim();
    if command.is_empty()
        || command.contains('>')
        || command.contains('`')
        || command.contains("$(")
        || command.contains("${")
        || command.contains('(')
        || command.contains(')')
        || command.contains("::")
        || command.contains('{')
        || command.contains('}')
    {
        return false;
    }

    let mut saw_command = false;
    for segment in command.split([';', '\n', '\r', '|', '&']) {
        let mut segment = segment.trim();
        if segment.is_empty() || segment.starts_with('#') {
            continue;
        }
        for prefix in ["sudo ", "doas "] {
            if let Some(rest) = segment.strip_prefix(prefix) {
                segment = rest.trim_start();
            }
        }
        let tokens = segment.split_whitespace().collect::<Vec<_>>();
        let Some(program) = tokens.first() else {
            continue;
        };
        let program = program
            .trim_matches(['\'', '"'])
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(program)
            .to_ascii_lowercase();
        let arguments = &tokens[1..];
        let read_only = match program.as_str() {
            "cd" | "chdir" | "pushd" | "popd" | "set-location" | "sl" | "pwd" | "get-location"
            | "ls" | "dir" | "gci" | "get-childitem" | "cat" | "type" | "gc" | "get-content"
            | "get-item" | "gi" | "get-itemproperty" | "test-path" | "resolve-path"
            | "select-string" | "findstr" | "rg" | "grep" | "head" | "tail" | "wc" | "stat"
            | "file" | "readlink" | "realpath" | "dirname" | "basename" | "du" | "df" | "tree"
            | "cut" | "uniq" | "tr" | "select-object" | "where-object" | "sort-object"
            | "measure-object" | "format-list" | "format-table" | "format-wide" | "out-string"
            | "write-output" | "write-host" | "echo" | "printf" | "compare-object"
            | "split-path" | "join-path" | "convertto-json" | "convertfrom-json"
            | "get-filehash" | "get-command" | "get-member" | "get-process" | "get-service"
            | "whoami" | "hostname" | "uname" | "id" | "which" | "where" => true,
            "find" => find_invocation_is_read_only(arguments),
            "sort" => sort_invocation_is_read_only(arguments),
            "git" => git_invocation_is_read_only(arguments),
            _ => {
                matches!(
                    program.as_str(),
                    "cargo"
                        | "rustc"
                        | "rustup"
                        | "node"
                        | "npm"
                        | "npx"
                        | "pnpm"
                        | "yarn"
                        | "bun"
                        | "python"
                        | "python3"
                        | "py"
                        | "dotnet"
                        | "java"
                        | "javac"
                        | "go"
                        | "gcc"
                        | "g++"
                        | "clang"
                        | "clang++"
                ) && arguments.len() == 1
                    && matches!(
                        arguments[0]
                            .trim_matches(['\'', '"'])
                            .to_ascii_lowercase()
                            .as_str(),
                        "--version" | "-v" | "-version" | "version"
                    )
            }
        };
        if !read_only {
            return false;
        }
        saw_command = true;
    }
    saw_command
}

fn git_invocation_is_read_only(arguments: &[&str]) -> bool {
    let mut index = 0;
    while index < arguments.len() {
        let raw_argument = arguments[index].trim_matches(['\'', '"']);
        let argument = raw_argument.to_ascii_lowercase();
        if raw_argument == "-C" {
            index = index.saturating_add(2);
            continue;
        }
        if argument == "-c" {
            return false;
        }
        if argument == "--no-pager"
            || argument.starts_with("--git-dir=")
            || argument.starts_with("--work-tree=")
        {
            index = index.saturating_add(1);
            continue;
        }
        let read_only_subcommand = matches!(
            argument.as_str(),
            "status"
                | "diff"
                | "log"
                | "show"
                | "rev-parse"
                | "ls-files"
                | "ls-tree"
                | "cat-file"
                | "grep"
                | "describe"
                | "shortlog"
                | "blame"
        );
        return read_only_subcommand
            && !arguments[index.saturating_add(1)..].iter().any(|argument| {
                let argument = argument.trim_matches(['\'', '"']).to_ascii_lowercase();
                argument == "--output" || argument.starts_with("--output=")
            });
    }
    false
}

fn find_invocation_is_read_only(arguments: &[&str]) -> bool {
    !arguments.iter().any(|argument| {
        matches!(
            argument
                .trim_matches(['\'', '"'])
                .to_ascii_lowercase()
                .as_str(),
            "-delete" | "-exec" | "-execdir" | "-ok" | "-okdir" | "-fprint" | "-fprintf" | "-fls"
        )
    })
}

fn sort_invocation_is_read_only(arguments: &[&str]) -> bool {
    !arguments.iter().any(|argument| {
        let argument = argument.trim_matches(['\'', '"']).to_ascii_lowercase();
        argument == "-o"
            || argument == "/o"
            || argument == "--output"
            || argument.starts_with("--output=")
    })
}

impl RemoteDesktopCommand {
    fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::Observe | Self::MovePointer { .. } | Self::Click { .. } => Ok(()),
            Self::Scroll { delta_y, .. }
                if *delta_y == 0 || !(-4_000..=4_000).contains(delta_y) =>
            {
                Err("invalid remote desktop scroll distance")
            }
            Self::Scroll { .. } => Ok(()),
            Self::Key {
                code,
                keysym,
                modifiers,
            } => {
                if code.is_empty()
                    || code.len() > MAX_REMOTE_DESKTOP_KEY_CODE_LEN
                    || !code.bytes().all(|byte| byte.is_ascii_alphanumeric())
                    || *keysym == 0
                {
                    return Err("invalid remote desktop key");
                }
                if modifiers.len() > 4
                    || modifiers
                        .iter()
                        .enumerate()
                        .any(|(index, modifier)| modifiers[..index].contains(modifier))
                {
                    return Err("invalid remote desktop key modifiers");
                }
                Ok(())
            }
            Self::TypeText { text } => {
                if text.is_empty()
                    || text.chars().count() > MAX_REMOTE_DESKTOP_TEXT_LEN
                    || text.contains('\0')
                {
                    return Err("invalid remote desktop text");
                }
                Ok(())
            }
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Observe => "remote_desktop_observe",
            Self::MovePointer { .. } => "remote_desktop_move_pointer",
            Self::Click { .. } => "remote_desktop_click",
            Self::Scroll { .. } => "remote_desktop_scroll",
            Self::Key { .. } => "remote_desktop_key",
            Self::TypeText { .. } => "remote_desktop_type_text",
        }
    }

    fn summary(&self) -> String {
        match self {
            Self::Observe => "Capture the current remote Linux desktop frame".to_owned(),
            Self::MovePointer { x, y } => {
                format!("Move the remote Linux pointer to {x},{y}")
            }
            Self::Click { x, y, button } => {
                format!("Click the {button:?} button at {x},{y} on the remote Linux desktop")
            }
            Self::Scroll { x, y, delta_y } => {
                format!("Scroll the remote Linux desktop at {x},{y} by {delta_y}")
            }
            Self::Key {
                code, modifiers, ..
            } => {
                let prefix = modifiers
                    .iter()
                    .map(|modifier| format!("{modifier:?}"))
                    .collect::<Vec<_>>()
                    .join("+");
                format!(
                    "Press {}{code} on the remote Linux desktop",
                    if prefix.is_empty() {
                        String::new()
                    } else {
                        format!("{prefix}+")
                    }
                )
            }
            Self::TypeText { text } => format!(
                "Type {} character(s) on the remote Linux desktop",
                text.chars().count()
            ),
        }
    }

    fn authorization(&self) -> ActionAuthorization {
        let effect = match self {
            Self::Observe => ActionEffect::ReadOnly,
            Self::MovePointer { .. } | Self::Scroll { .. } | Self::Key { .. } => {
                ActionEffect::ExternalInteraction
            }
            Self::Click { .. } | Self::TypeText { .. } => ActionEffect::ReversibleWrite,
        };
        ActionAuthorization::new(CapabilityScope::RemoteDesktop, effect)
    }
}

impl UiAutomationCommand {
    fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::Inspect { window_id } => validate_window_id(window_id),
            Self::Focus { element_id }
            | Self::Invoke { element_id }
            | Self::Select { element_id }
            | Self::Expand { element_id, .. } => validate_ui_element_id(element_id),
            Self::SetValue { element_id, value } => {
                validate_ui_element_id(element_id)?;
                validate_ui_text(value)
            }
            Self::Scroll {
                element_id,
                horizontal,
                vertical,
            } => {
                validate_ui_element_id(element_id)?;
                if horizontal.is_none() && vertical.is_none() {
                    return Err("a UI scroll direction is required");
                }
                Ok(())
            }
            Self::TextInput { element_id, text } => {
                validate_ui_element_id(element_id)?;
                validate_ui_text(text)
            }
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Inspect { .. } => "ui_inspect",
            Self::Focus { .. } => "ui_focus",
            Self::Invoke { .. } => "ui_invoke",
            Self::SetValue { .. } => "ui_set_value",
            Self::Select { .. } => "ui_select",
            Self::Expand { .. } => "ui_expand",
            Self::Scroll { .. } => "ui_scroll",
            Self::TextInput { .. } => "ui_text_input",
        }
    }

    fn summary(&self) -> String {
        match self {
            Self::Inspect { window_id } => {
                format!("Inspect the bounded semantic UI tree for {window_id}")
            }
            Self::Focus { element_id } => format!("Focus semantic UI element {element_id}"),
            Self::Invoke { element_id } => format!("Invoke semantic UI element {element_id}"),
            Self::SetValue { element_id, value } => format!(
                "Set a {} character value on semantic UI element {element_id}",
                value.chars().count()
            ),
            Self::Select { element_id } => format!("Select semantic UI element {element_id}"),
            Self::Expand {
                element_id,
                expanded,
            } => format!(
                "{} semantic UI element {element_id}",
                if *expanded { "Expand" } else { "Collapse" }
            ),
            Self::Scroll { element_id, .. } => {
                format!("Scroll semantic UI element {element_id}")
            }
            Self::TextInput { element_id, text } => format!(
                "Enter {} character(s) into semantic UI element {element_id}",
                text.chars().count()
            ),
        }
    }

    fn authorization(&self) -> ActionAuthorization {
        let effect = match self {
            Self::Inspect { .. } => ActionEffect::ReadOnly,
            Self::SetValue { .. } | Self::TextInput { .. } => ActionEffect::ReversibleWrite,
            Self::Focus { .. }
            | Self::Invoke { .. }
            | Self::Select { .. }
            | Self::Expand { .. }
            | Self::Scroll { .. } => ActionEffect::ExternalInteraction,
        };
        ActionAuthorization::new(CapabilityScope::UiAutomation, effect)
    }
}

impl CaptureCommand {
    fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::Window { window_id } => validate_window_id(window_id),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Window { .. } => "capture_window",
        }
    }

    fn summary(&self) -> String {
        match self {
            Self::Window { window_id } => {
                format!("Capture a bounded visual snapshot of {window_id}")
            }
        }
    }

    fn authorization(&self) -> ActionAuthorization {
        ActionAuthorization::new(CapabilityScope::Capture, ActionEffect::ReadOnly)
    }
}

impl WindowCommand {
    fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::List => Ok(()),
            Self::Focus { window_id }
            | Self::Minimize { window_id }
            | Self::Restore { window_id }
            | Self::Close { window_id } => validate_window_id(window_id),
            Self::MoveResize {
                window_id,
                x,
                y,
                width,
                height,
            } => {
                validate_window_id(window_id)?;
                if !(-32_768..=32_768).contains(x)
                    || !(-32_768..=32_768).contains(y)
                    || !(160..=7_680).contains(width)
                    || !(120..=4_320).contains(height)
                {
                    return Err("invalid window bounds");
                }
                Ok(())
            }
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::List => "window_list",
            Self::Focus { .. } => "window_focus",
            Self::MoveResize { .. } => "window_move_resize",
            Self::Minimize { .. } => "window_minimize",
            Self::Restore { .. } => "window_restore",
            Self::Close { .. } => "window_close",
        }
    }

    fn summary(&self) -> String {
        match self {
            Self::List => "List current top-level Windows application windows".to_owned(),
            Self::Focus { window_id } => format!("Focus fresh window reference {window_id}"),
            Self::MoveResize {
                window_id,
                x,
                y,
                width,
                height,
            } => format!("Move {window_id} to {x},{y} at {width}×{height}"),
            Self::Minimize { window_id } => format!("Minimize {window_id}"),
            Self::Restore { window_id } => format!("Restore {window_id}"),
            Self::Close { window_id } => format!("Request {window_id} to close"),
        }
    }

    fn authorization(&self) -> ActionAuthorization {
        let effect = match self {
            Self::List => ActionEffect::ReadOnly,
            Self::Focus { .. }
            | Self::MoveResize { .. }
            | Self::Minimize { .. }
            | Self::Restore { .. } => ActionEffect::ExternalInteraction,
            Self::Close { .. } => ActionEffect::Destructive,
        };
        ActionAuthorization::new(CapabilityScope::Window, effect)
    }
}

fn validate_window_id(window_id: &str) -> Result<(), &'static str> {
    if window_id.is_empty()
        || window_id.len() > MAX_WINDOW_REF_LEN
        || !window_id.starts_with("window-")
        || !window_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        Err("invalid window reference")
    } else {
        Ok(())
    }
}

fn validate_ui_element_id(element_id: &str) -> Result<(), &'static str> {
    if element_id.is_empty()
        || element_id.len() > MAX_UI_ELEMENT_REF_LEN
        || !element_id.starts_with("ui-")
        || !element_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        Err("invalid semantic UI element reference")
    } else {
        Ok(())
    }
}

fn validate_ui_text(value: &str) -> Result<(), &'static str> {
    if value.chars().count() > MAX_UI_TEXT_LEN || value.contains('\0') {
        Err("invalid semantic UI text")
    } else {
        Ok(())
    }
}

impl ProcessCommand {
    fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::List => Ok(()),
            Self::Status { process_id, .. } | Self::Cancel { process_id } if *process_id == 0 => {
                Err("invalid managed process reference")
            }
            Self::Status { .. } | Self::Cancel { .. } => Ok(()),
            Self::Start { command, path } => {
                validate_workspace_optional_path(path)?;
                if command.trim().is_empty() || command.contains('\0') {
                    return Err("invalid managed process start request");
                }
                Ok(())
            }
            Self::WriteStdin {
                process_id, data, ..
            } => {
                if *process_id == 0 || data.contains('\0') {
                    return Err("invalid managed process input");
                }
                Ok(())
            }
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::List => "process_list",
            Self::Status { .. } => "process_status",
            Self::Start { .. } => "process_start",
            Self::WriteStdin { .. } => "process_write_stdin",
            Self::Cancel { .. } => "process_cancel",
        }
    }

    fn summary(&self) -> String {
        match self {
            Self::List => "List processes launched by Supervisor".to_owned(),
            Self::Status { process_id, .. } => {
                format!("Read background process {process_id} status and next output chunk")
            }
            Self::Start { command, path } => format!(
                "Start background agent command in {}: {}",
                path.as_deref()
                    .filter(|path| !path.is_empty())
                    .unwrap_or("the workspace"),
                truncate(command.trim(), 150)
            ),
            Self::WriteStdin {
                process_id,
                data,
                close,
            } => format!(
                "Write {} character(s) to managed process {process_id}{}",
                data.chars().count(),
                if *close { " and close stdin" } else { "" }
            ),
            Self::Cancel { process_id } => format!("Cancel managed process {process_id}"),
        }
    }

    fn authorization(&self) -> ActionAuthorization {
        let effect = match self {
            Self::List | Self::Status { .. } => ActionEffect::ReadOnly,
            Self::Start { command, .. } if shell_command_is_obviously_read_only(command) => {
                ActionEffect::ReadOnly
            }
            Self::Start { .. } | Self::WriteStdin { .. } => ActionEffect::Destructive,
            Self::Cancel { .. } => ActionEffect::Destructive,
        };
        ActionAuthorization::new(CapabilityScope::Process, effect)
    }
}

impl TerminalCommand {
    fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::Run { command } if command.trim().is_empty() || command.contains('\0') => {
                Err("invalid terminal command")
            }
            _ => Ok(()),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Run { .. } => "run_command",
        }
    }

    fn summary(&self) -> String {
        match self {
            Self::Run { command } => {
                format!("Run in the local shell: {}", truncate(command.trim(), 180))
            }
        }
    }
}

impl SshCommand {
    fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::Run {
                profile_id,
                profile_name,
                ..
            } => {
                if Uuid::parse_str(profile_id).is_err() {
                    return Err("invalid SSH profile reference");
                }
                if profile_name.trim().is_empty() || profile_name.chars().count() > 64 {
                    return Err("invalid SSH profile name");
                }
            }
        }
        let Self::Run { command, .. } = self;
        if command.trim().is_empty() || command.contains('\0') {
            return Err("invalid SSH command");
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "ssh_run"
    }

    fn summary(&self) -> String {
        match self {
            Self::Run {
                profile_name,
                command,
                ..
            } => format!(
                "Run on VPS {profile_name}: {}",
                truncate(command.trim(), 180)
            ),
        }
    }
}

impl WorkspaceCommand {
    fn validate(&self) -> Result<(), &'static str> {
        match self {
            Self::State => {}
            Self::List {
                path,
                depth,
                max_entries,
            } => {
                validate_workspace_optional_path(path)?;
                if *depth > 4 || !(1..=200).contains(max_entries) {
                    return Err("invalid workspace listing limits");
                }
            }
            Self::Read {
                path,
                start_line,
                line_count,
            } => {
                validate_workspace_path(path)?;
                if *start_line == 0 || !(1..=400).contains(line_count) {
                    return Err("invalid workspace read range");
                }
            }
            Self::Search {
                query,
                path,
                max_results,
                ..
            } => {
                validate_workspace_optional_path(path)?;
                if query.trim().is_empty()
                    || query.chars().count() > MAX_WORKSPACE_QUERY_CHARS
                    || !(1..=50).contains(max_results)
                    || query.contains('\0')
                {
                    return Err("invalid workspace search");
                }
            }
            Self::ApplyPatch {
                path,
                expected_sha256,
                replacements,
                create_content,
            } => {
                validate_workspace_path(path)?;
                let creates_file = create_content.is_some();
                if creates_file {
                    if expected_sha256.is_some() || !replacements.is_empty() {
                        return Err("new workspace files cannot also contain replacements");
                    }
                } else if expected_sha256.as_deref().is_none_or(|hash| {
                    hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit())
                }) || replacements.is_empty()
                    || replacements.len() > MAX_WORKSPACE_REPLACEMENTS
                {
                    return Err("existing workspace patches require a hash and replacements");
                }

                if create_content.as_ref().is_some_and(|content| {
                    content.len() > MAX_WORKSPACE_PATCH_CHARS || content.contains('\0')
                }) || replacements.iter().any(|replacement| {
                    replacement.old_text.is_empty()
                        || replacement.old_text.len() > MAX_WORKSPACE_PATCH_CHARS
                        || replacement.new_text.len() > MAX_WORKSPACE_PATCH_CHARS
                        || replacement.old_text.contains('\0')
                        || replacement.new_text.contains('\0')
                }) {
                    return Err("invalid workspace patch content");
                }
            }
            Self::Run { command, path } => {
                validate_workspace_optional_path(path)?;
                if command.trim().is_empty() || command.contains('\0') {
                    return Err("invalid workspace command");
                }
            }
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        match self {
            Self::State => "workspace_state",
            Self::List { .. } => "workspace_list",
            Self::Read { .. } => "workspace_read",
            Self::Search { .. } => "workspace_search",
            Self::ApplyPatch { .. } => "workspace_apply_patch",
            Self::Run { .. } => "workspace_run_command",
        }
    }

    fn summary(&self) -> String {
        match self {
            Self::State => "Read the connected workspace state".to_owned(),
            Self::List { path, .. } => format!(
                "List workspace files under {}",
                path.as_deref()
                    .filter(|path| !path.is_empty())
                    .unwrap_or(".")
            ),
            Self::Read { path, .. } => format!("Read workspace file {}", truncate(path, 160)),
            Self::Search { query, path, .. } => format!(
                "Search {} for {}",
                path.as_deref()
                    .filter(|path| !path.is_empty())
                    .unwrap_or("the workspace"),
                truncate(query.trim(), 100)
            ),
            Self::ApplyPatch {
                path,
                create_content,
                replacements,
                ..
            } => {
                if create_content.is_some() {
                    format!("Create workspace file {}", truncate(path, 160))
                } else {
                    format!(
                        "Apply {} exact edit(s) to {}",
                        replacements.len(),
                        truncate(path, 160)
                    )
                }
            }
            Self::Run { command, path } => format!(
                "Run in workspace {}: {}",
                path.as_deref()
                    .filter(|path| !path.is_empty())
                    .unwrap_or("."),
                truncate(command.trim(), 150)
            ),
        }
    }

    fn authorization(&self) -> ActionAuthorization {
        if let Self::Run { command, .. } = self {
            if shell_command_is_obviously_read_only(command) {
                return ActionAuthorization::new(
                    CapabilityScope::Workspace,
                    ActionEffect::ReadOnly,
                );
            }
            return ActionAuthorization::new(CapabilityScope::Process, ActionEffect::Destructive);
        }
        let effect = match self {
            Self::State | Self::List { .. } | Self::Read { .. } | Self::Search { .. } => {
                ActionEffect::ReadOnly
            }
            Self::ApplyPatch { .. } => ActionEffect::ReversibleWrite,
            Self::Run { .. } => unreachable!(),
        };
        ActionAuthorization::new(CapabilityScope::Workspace, effect)
    }
}

fn validate_workspace_path(path: &str) -> Result<(), &'static str> {
    if path.trim().is_empty()
        || path.chars().count() > MAX_WORKSPACE_PATH_CHARS
        || path.contains('\0')
    {
        Err("invalid workspace path")
    } else {
        Ok(())
    }
}

fn validate_workspace_optional_path(path: &Option<String>) -> Result<(), &'static str> {
    if let Some(path) = path
        && (path.chars().count() > MAX_WORKSPACE_PATH_CHARS || path.contains('\0'))
    {
        return Err("invalid workspace path");
    }
    Ok(())
}

impl BrowserCommand {
    fn authorization(&self) -> ActionAuthorization {
        let effect = match self {
            Self::GetState | Self::InspectPage => ActionEffect::ReadOnly,
            Self::Click { .. } | Self::SetText { .. } => ActionEffect::ExternalInteraction,
            Self::Navigate { .. }
            | Self::Back
            | Self::Forward
            | Self::Reload
            | Self::Stop
            | Self::Home
            | Self::NewTab { .. }
            | Self::CloseTab { .. }
            | Self::ActivateTab { .. }
            | Self::Scroll { .. } => ActionEffect::ReversibleWrite,
        };
        ActionAuthorization::new(CapabilityScope::Browser, effect)
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::GetState => "get_state",
            Self::InspectPage => "inspect_page",
            Self::Navigate { .. } => "navigate",
            Self::Back => "back",
            Self::Forward => "forward",
            Self::Reload => "reload",
            Self::Stop => "stop",
            Self::Home => "home",
            Self::NewTab { .. } => "new_tab",
            Self::CloseTab { .. } => "close_tab",
            Self::ActivateTab { .. } => "activate_tab",
            Self::Click { .. } => "click",
            Self::SetText { .. } => "set_text",
            Self::Scroll { .. } => "scroll",
        }
    }

    pub(crate) fn summary(&self) -> String {
        match self {
            Self::Navigate { value } => format!("Navigate to {}", truncate(value, 90)),
            Self::NewTab { url: Some(url) } => {
                format!("Open a tab at {}", truncate(url, 90))
            }
            Self::NewTab { url: None } => "Open a new tab".to_owned(),
            Self::CloseTab { tab_id } => format!("Close tab {tab_id}"),
            Self::ActivateTab { tab_id } => format!("Activate tab {tab_id}"),
            Self::Click { target } => format!("Click {target}"),
            Self::SetText { target, value } => {
                format!(
                    "Set text in {target} ({} characters)",
                    value.chars().count()
                )
            }
            Self::Scroll { delta_y } => format!("Scroll the page by {delta_y}px"),
            Self::GetState => "Read browser state".to_owned(),
            Self::InspectPage => "Inspect the active page".to_owned(),
            Self::Back => "Go back".to_owned(),
            Self::Forward => "Go forward".to_owned(),
            Self::Reload => "Reload the page".to_owned(),
            Self::Stop => "Stop loading".to_owned(),
            Self::Home => "Open the start page".to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CommandStatus {
    AwaitingApproval,
    Running,
    Success,
    Error,
    Denied,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActionLogEntry {
    pub request_id: String,
    pub action: String,
    pub summary: String,
    pub scope: CapabilityScope,
    pub effect: ActionEffect,
    pub status: CommandStatus,
    pub timestamp_ms: u128,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandResultView {
    pub request_id: String,
    pub action: String,
    pub ok: bool,
    pub message: String,
    pub data: Value,
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_allow_listed_command() {
        let request: BrowserCommandRequest =
            serde_json::from_str(r#"{"request_id":"req-1","command":{"type":"inspect_page"}}"#)
                .unwrap();

        assert_eq!(request.command.name(), "inspect_page");
        assert!(request.validate().is_ok());
    }

    #[test]
    fn parses_panel_execute_envelope() {
        let message: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"execute","request":{"request_id":"req-1","command":{"type":"get_state"}}}"#,
        )
        .unwrap();

        assert!(matches!(message, AgentPanelMessage::Execute { .. }));
    }

    #[test]
    fn parses_settings_close_control() {
        let message: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"close_settings"}"#).unwrap();

        assert!(matches!(message, AgentPanelMessage::CloseSettings));
    }

    #[test]
    fn parses_workspace_connection_controls() {
        let open_registry: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"open_project_registry"}"#).unwrap();
        let select: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"select_workspace"}"#).unwrap();
        let clear: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"clear_workspace"}"#).unwrap();
        let activate: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"activate_workspace","root":"C:\\code\\central-agent"}"#,
        )
        .unwrap();
        let refresh: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"refresh_workspace_explorer"}"#).unwrap();
        let create_file: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"create_workspace_file","path":"src/new.rs"}"#)
                .unwrap();
        let create_directory: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"create_workspace_directory","path":"src/generated"}"#,
        )
        .unwrap();
        let import: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"import_workspace_files","directory":"src"}"#)
                .unwrap();
        let create_chat: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"create_project_chat","root":"C:\\code\\central-agent"}"#,
        )
        .unwrap();
        let rename_project: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"rename_workspace","root":"C:\\code\\central-agent","name":"Supervisor"}"#,
        )
        .unwrap();
        let pin_project: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"set_workspace_pinned","root":"C:\\code\\central-agent","pinned":true}"#,
        )
        .unwrap();
        let rename_chat: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"rename_project_chat","chat_id":"chat-1","title":"Project navigation"}"#,
        )
        .unwrap();
        let move_chat: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"move_project_chat","chat_id":"chat-1","project_root":"C:\\code\\another-project"}"#,
        )
        .unwrap();
        let select_project_for_chat: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"select_project_for_chat","chat_id":"chat-1"}"#,
        )
        .unwrap();
        let pin_chat: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"set_project_chat_pinned","chat_id":"chat-1","pinned":true}"#,
        )
        .unwrap();
        let archive_chat: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"set_project_chat_archived","chat_id":"chat-1","archived":true}"#,
        )
        .unwrap();
        let archive_project_chats: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"set_project_chats_archived","root":"C:\\code\\central-agent","archived":true}"#,
        )
        .unwrap();
        let archive_project: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"set_workspace_archived","root":"C:\\code\\central-agent","archived":true}"#,
        )
        .unwrap();
        let eject_project: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"eject_workspace","root":"C:\\code\\central-agent"}"#,
        )
        .unwrap();
        let stop_and_eject_project: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"eject_workspace","root":"C:\\code\\central-agent","stop_active_runs":true}"#,
        )
        .unwrap();
        assert!(matches!(
            open_registry,
            AgentPanelMessage::OpenProjectRegistry
        ));
        assert!(matches!(select, AgentPanelMessage::SelectWorkspace));
        assert!(matches!(
            activate,
            AgentPanelMessage::ActivateWorkspace { root } if root == r"C:\code\central-agent"
        ));
        assert!(matches!(
            create_chat,
            AgentPanelMessage::CreateProjectChat { .. }
        ));
        assert!(matches!(
            rename_project,
            AgentPanelMessage::RenameWorkspace { name, .. } if name == "Supervisor"
        ));
        assert!(matches!(
            pin_project,
            AgentPanelMessage::SetWorkspacePinned { pinned: true, .. }
        ));
        assert!(matches!(
            rename_chat,
            AgentPanelMessage::RenameProjectChat { title, .. } if title == "Project navigation"
        ));
        assert!(matches!(
            move_chat,
            AgentPanelMessage::MoveProjectChat {
                chat_id,
                project_root,
            } if chat_id == "chat-1" && project_root == r"C:\code\another-project"
        ));
        assert!(matches!(
            select_project_for_chat,
            AgentPanelMessage::SelectProjectForChat { chat_id } if chat_id == "chat-1"
        ));
        assert!(matches!(
            pin_chat,
            AgentPanelMessage::SetProjectChatPinned { pinned: true, .. }
        ));
        assert!(matches!(
            archive_chat,
            AgentPanelMessage::SetProjectChatArchived { archived: true, .. }
        ));
        assert!(matches!(
            archive_project_chats,
            AgentPanelMessage::SetProjectChatsArchived { archived: true, .. }
        ));
        assert!(matches!(
            archive_project,
            AgentPanelMessage::SetWorkspaceArchived { archived: true, .. }
        ));
        assert!(matches!(
            eject_project,
            AgentPanelMessage::EjectWorkspace {
                stop_active_runs: false,
                ..
            }
        ));
        assert!(matches!(
            stop_and_eject_project,
            AgentPanelMessage::EjectWorkspace {
                stop_active_runs: true,
                ..
            }
        ));
        assert!(matches!(clear, AgentPanelMessage::ClearWorkspace));
        assert!(matches!(
            refresh,
            AgentPanelMessage::RefreshWorkspaceExplorer
        ));
        assert!(matches!(
            create_file,
            AgentPanelMessage::CreateWorkspaceFile { path } if path == "src/new.rs"
        ));
        assert!(matches!(
            create_directory,
            AgentPanelMessage::CreateWorkspaceDirectory { path } if path == "src/generated"
        ));
        assert!(matches!(
            import,
            AgentPanelMessage::ImportWorkspaceFiles { directory } if directory == "src"
        ));
    }

    #[test]
    fn parses_artifact_controls() {
        let open: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"open_artifact","artifact_id":"artifact-1"}"#)
                .unwrap();
        let save: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"save_artifact","artifact_id":"artifact-1"}"#)
                .unwrap();
        let folder: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"show_artifact_in_folder","artifact_id":"artifact-1"}"#,
        )
        .unwrap();

        assert!(matches!(open, AgentPanelMessage::OpenArtifact { .. }));
        assert!(matches!(save, AgentPanelMessage::SaveArtifact { .. }));
        assert!(matches!(
            folder,
            AgentPanelMessage::ShowArtifactInFolder { .. }
        ));
    }

    #[test]
    fn parses_time_machine_diff_and_restore_controls() {
        let open: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"open_checkpoint","checkpoint_id":"checkpoint-1","path":"src/main.rs"}"#,
        )
        .unwrap();
        let select: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"select_checkpoint_file","checkpoint_id":"checkpoint-1","path":"Cargo.toml"}"#,
        )
        .unwrap();
        let restore_all: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"restore_checkpoint","checkpoint_id":"checkpoint-1"}"#,
        )
        .unwrap();
        let restore_file: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"restore_checkpoint_file","checkpoint_id":"checkpoint-1","path":"src/main.rs"}"#,
        )
        .unwrap();

        assert!(
            matches!(open, AgentPanelMessage::OpenCheckpoint { path: Some(path), .. } if path == "src/main.rs")
        );
        assert!(
            matches!(select, AgentPanelMessage::SelectCheckpointFile { path, .. } if path == "Cargo.toml")
        );
        assert!(matches!(
            restore_all,
            AgentPanelMessage::RestoreCheckpoint { .. }
        ));
        assert!(
            matches!(restore_file, AgentPanelMessage::RestoreCheckpointFile { path, .. } if path == "src/main.rs")
        );
    }

    #[test]
    fn classifies_workspace_reads_and_mutations_separately() {
        let read = AgentCommand::Workspace(WorkspaceCommand::Read {
            path: "src/main.rs".to_owned(),
            start_line: 1,
            line_count: 200,
        });
        assert!(read.validate().is_ok());
        assert!(!read.requires_approval());
        assert_eq!(read.authorization().scope, CapabilityScope::Workspace);
        assert_eq!(read.authorization().effect, ActionEffect::ReadOnly);

        let patch = AgentCommand::Workspace(WorkspaceCommand::ApplyPatch {
            path: "src/main.rs".to_owned(),
            expected_sha256: Some("a".repeat(64)),
            replacements: vec![TextReplacement {
                old_text: "before".to_owned(),
                new_text: "after".to_owned(),
            }],
            create_content: None,
        });
        assert!(patch.validate().is_ok());
        assert!(patch.requires_approval());
        assert_eq!(patch.authorization().effect, ActionEffect::ReversibleWrite);
    }

    #[test]
    fn parses_settings_surface_ready_control() {
        let message: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"settings_surface_ready"}"#).unwrap();

        assert!(matches!(message, AgentPanelMessage::SettingsSurfaceReady));
    }

    #[test]
    fn parses_settings_cover_ready_control() {
        let message: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"settings_cover_ready"}"#).unwrap();

        assert!(matches!(message, AgentPanelMessage::SettingsCoverReady));
    }

    #[test]
    fn parses_settings_close_layout_ready_control() {
        let message: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"settings_close_layout_ready"}"#).unwrap();

        assert!(matches!(
            message,
            AgentPanelMessage::SettingsCloseLayoutReady
        ));
    }

    #[test]
    fn parses_permission_control_separately_from_agent_commands() {
        let message: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"set_permission_mode","mode":"initial_authorization"}"#,
        )
        .unwrap();

        assert!(matches!(
            message,
            AgentPanelMessage::SetPermissionMode {
                mode: PermissionMode::InitialAuthorization
            }
        ));
    }

    #[test]
    fn parses_legacy_agent_graph_browser_controls() {
        let open: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"open_knowledge","record_type":"entity","id":"2a568e92-25d8-4ff0-b97f-3945f3c090c2"}"#,
        )
        .unwrap();
        let assign: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"set_knowledge_agent","record_type":"entity","id":"2a568e92-25d8-4ff0-b97f-3945f3c090c2","name":"Architecture agent","mission":"Maintain the local-first architecture.","provider":"claude_code","model":"claude-sonnet-4-5","effort":"high","service_tier":"priority","context_window":128000}"#,
        )
        .unwrap();
        let link: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"set_knowledge_agent_link","source_record_type":"entity","source_id":"2a568e92-25d8-4ff0-b97f-3945f3c090c2","target_record_type":"entity","target_id":"5aab7f58-aed5-426f-8dab-34defe18fbba","linked":true}"#,
        )
        .unwrap();
        let run: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"run_knowledge_agent","record_type":"entity","id":"2a568e92-25d8-4ff0-b97f-3945f3c090c2"}"#,
        )
        .unwrap();
        let continue_run: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"continue_knowledge_agent","record_type":"entity","id":"2a568e92-25d8-4ff0-b97f-3945f3c090c2","message":"Continue from the verified repository state"}"#,
        )
        .unwrap();
        let stop_run: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"stop_knowledge_agent","record_type":"entity","id":"2a568e92-25d8-4ff0-b97f-3945f3c090c2"}"#,
        )
        .unwrap();

        assert!(matches!(
            open,
            AgentPanelMessage::OpenAgentGraphNode { record_type, .. } if record_type == "entity"
        ));
        assert!(matches!(
            assign,
            AgentPanelMessage::SetAgentGraphAgent {
                name,
                provider: Some(AgentProviderKind::ClaudeCode),
                model: Some(model),
                effort: Some(effort),
                service_tier: Some(service_tier),
                context_window: Some(128_000),
                ..
            } if name == "Architecture agent"
                && model == "claude-sonnet-4-5"
                && effort == "high"
                && service_tier == "priority"
        ));
        assert!(matches!(
            link,
            AgentPanelMessage::SetAgentGraphLink { linked: true, .. }
        ));
        assert!(matches!(run, AgentPanelMessage::RunAgentGraphAgent { .. }));
        assert!(matches!(
            continue_run,
            AgentPanelMessage::ContinueAgentGraphAgent { message, .. }
                if message == "Continue from the verified repository state"
        ));
        assert!(matches!(
            stop_run,
            AgentPanelMessage::StopAgentGraphAgent { .. }
        ));
    }

    #[test]
    fn graph_conversation_selectors_are_optional_typed_and_never_fall_back_when_invalid() {
        let conversation_id = uuid::Uuid::new_v4();
        for message_type in [
            "set_knowledge_agent",
            "remove_knowledge_agent",
            "run_knowledge_agent",
            "continue_knowledge_agent",
            "clear_knowledge_agent_queue",
            "load_knowledge_agent_history",
            "stop_knowledge_agent",
        ] {
            let mut base = serde_json::json!({"message_type":message_type,"record_type":"entity","id":"node-a"});
            match message_type {
                "set_knowledge_agent" => {
                    base["name"] = serde_json::json!("Agent");
                    base["mission"] = serde_json::json!("Inspect");
                }
                "continue_knowledge_agent" => {
                    base["message"] = serde_json::json!("Continue");
                }
                "load_knowledge_agent_history" => {
                    base["before"] = serde_json::json!(1);
                }
                _ => {}
            }
            for expected in [None, Some(conversation_id)] {
                let mut input = base.clone();
                if let Some(id) = expected {
                    input["conversation_id"] = serde_json::json!(id);
                }
                let parsed: AgentPanelMessage = serde_json::from_value(input).unwrap();
                let actual = match parsed {
                    AgentPanelMessage::SetAgentGraphAgent {
                        conversation_id, ..
                    }
                    | AgentPanelMessage::RemoveAgentGraphAgent {
                        conversation_id, ..
                    }
                    | AgentPanelMessage::RunAgentGraphAgent {
                        conversation_id, ..
                    }
                    | AgentPanelMessage::ContinueAgentGraphAgent {
                        conversation_id, ..
                    }
                    | AgentPanelMessage::ClearAgentGraphQueue {
                        conversation_id, ..
                    }
                    | AgentPanelMessage::LoadAgentGraphHistory {
                        conversation_id, ..
                    }
                    | AgentPanelMessage::StopAgentGraphAgent {
                        conversation_id, ..
                    } => conversation_id,
                    _ => panic!("Wrong graph command"),
                };
                assert_eq!(actual, expected, "{message_type}");
            }
            for invalid in [
                serde_json::json!("bad-id"),
                serde_json::json!(42),
                serde_json::json!({}),
            ] {
                let mut input = base.clone();
                input["conversation_id"] = invalid;
                assert!(
                    serde_json::from_value::<AgentPanelMessage>(input).is_err(),
                    "{message_type}"
                );
            }
        }
        let sibling = uuid::Uuid::new_v4();
        let link: AgentPanelMessage = serde_json::from_value(serde_json::json!({
            "message_type":"set_knowledge_agent_link", "source_record_type":"entity", "source_id":"node-a",
            "target_record_type":"entity", "target_id":"node-a", "linked":true,
            "source_conversation_id":conversation_id, "target_conversation_id":sibling
        })).unwrap();
        assert!(matches!(link, AgentPanelMessage::SetAgentGraphLink {
            source_conversation_id: Some(source), target_conversation_id: Some(target), ..
        } if source == conversation_id && target == sibling));
    }

    #[test]
    fn graph_prompt_delivery_defaults_are_compatible_and_choices_remain_typed() {
        for (suffix, expected) in [
            ("", AgentSubmissionDelivery::Start),
            (",\"delivery\":\"queue\"", AgentSubmissionDelivery::Queue),
            (",\"delivery\":\"steer\"", AgentSubmissionDelivery::Steer),
        ] {
            let command: AgentPanelMessage = serde_json::from_str(&format!(r#"{{"message_type":"continue_knowledge_agent","record_type":"entity","id":"node-a","message":"Continue"{suffix}}}"#)).unwrap();
            assert!(
                matches!(command, AgentPanelMessage::ContinueAgentGraphAgent { id, delivery, .. } if id == "node-a" && delivery == expected)
            );
        }
        let clear: AgentPanelMessage = serde_json::from_str(r#"{"message_type":"clear_knowledge_agent_queue","record_type":"entity","id":"node-a"}"#).unwrap();
        assert!(
            matches!(clear, AgentPanelMessage::ClearAgentGraphQueue { id, .. } if id == "node-a")
        );
        assert!(serde_json::from_str::<AgentPanelMessage>(r#"{"message_type":"continue_knowledge_agent","record_type":"entity","id":"node-a","message":"Continue","delivery":"replace_owner"}"#).is_err());
    }

    #[test]
    fn parses_chat_with_tab_context() {
        let message: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"submit_chat","message":"inspect","tab_ids":[2,4],"terminal_session_ids":[3],"file_ids":["file-1"]}"#,
        )
        .unwrap();

        assert!(matches!(
            message,
            AgentPanelMessage::SubmitChat { resume: false, delivery: AgentSubmissionDelivery::Start, tab_ids, terminal_session_ids, file_ids, .. }
                if tab_ids == vec![2, 4] && terminal_session_ids == vec![3] && file_ids == vec!["file-1"]
        ));

        let resume: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"submit_chat","message":"","resume":true}"#)
                .unwrap();
        assert!(matches!(
            resume,
            AgentPanelMessage::SubmitChat {
                message,
                resume: true,
                delivery: AgentSubmissionDelivery::Start,
                tab_ids,
                terminal_session_ids,
                file_ids,
                ..
            } if message.is_empty()
                && tab_ids.is_empty()
                && terminal_session_ids.is_empty()
                && file_ids.is_empty()
        ));

        let queued: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"submit_chat","message":"next","delivery":"queue"}"#,
        )
        .unwrap();
        assert!(matches!(
            queued,
            AgentPanelMessage::SubmitChat {
                delivery: AgentSubmissionDelivery::Queue,
                ..
            }
        ));

        let select: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"select_chat_files"}"#).unwrap();
        let remove: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"remove_chat_file","file_id":"file-1"}"#)
                .unwrap();
        assert!(matches!(select, AgentPanelMessage::SelectChatFiles));
        assert!(matches!(
            remove,
            AgentPanelMessage::RemoveChatFile { file_id } if file_id == "file-1"
        ));

        let cancel_queued: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"cancel_pending_agent_submission"}"#).unwrap();
        assert!(matches!(
            cancel_queued,
            AgentPanelMessage::CancelPendingAgentSubmission
        ));
        let clear_queue: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"clear_agent_submission_queue"}"#).unwrap();
        assert!(matches!(
            clear_queue,
            AgentPanelMessage::ClearAgentSubmissionQueue
        ));
        let discard: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"discard_failed_checkpoint","run_id":42}"#)
                .unwrap();
        assert!(matches!(
            discard,
            AgentPanelMessage::DiscardFailedCheckpoint { run_id: 42 }
        ));
    }

    #[test]
    fn parses_agent_provider_and_configuration_selection() {
        let provider: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"set_agent_provider","provider":"claude_code"}"#,
        )
        .unwrap();
        let configuration: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"set_agent_configuration","model":"sonnet","effort":"high","service_tier":null}"#,
        )
        .unwrap();

        assert!(matches!(
            provider,
            AgentPanelMessage::SetAgentProvider {
                provider: AgentProviderKind::ClaudeCode
            }
        ));
        assert!(matches!(
            configuration,
            AgentPanelMessage::SetAgentConfiguration {
                model,
                effort,
                service_tier: None,
                context_window: None,
                personality: None
            } if model == "sonnet" && effort == "high"
        ));
    }

    #[test]
    fn parses_tab_context_controls() {
        let capture: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"capture_tab_context","tab_id":7}"#).unwrap();
        let refresh: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"refresh_tab_context","tab_id":7}"#).unwrap();
        let remove: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"remove_tab_context","tab_id":7}"#).unwrap();

        assert!(matches!(
            capture,
            AgentPanelMessage::CaptureTabContext { tab_id: 7 }
        ));
        assert!(matches!(
            refresh,
            AgentPanelMessage::RefreshTabContext { tab_id: 7 }
        ));
        assert!(matches!(
            remove,
            AgentPanelMessage::RemoveTabContext { tab_id: 7 }
        ));
    }

    #[test]
    fn parses_terminal_panel_controls() {
        let resize_agent: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"set_agent_panel_width","width":920,"persist":true}"#,
        )
        .unwrap();
        let detach_agent: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"detach_agent_panel"}"#).unwrap();
        let attach_agent: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"attach_agent_panel"}"#).unwrap();
        let open: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"open_terminal"}"#).unwrap();
        let input: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"terminal_write","session_id":4,"data":"cargo test\r"}"#,
        )
        .unwrap();
        let reorder: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"reorder_terminal","session_id":4,"before_session_id":2}"#,
        )
        .unwrap();
        let dock: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"set_terminal_dock","dock":"right"}"#).unwrap();
        let detach: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"detach_terminal_panel","screen_x":2200,"screen_y":640}"#,
        )
        .unwrap();
        let attach: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"attach_terminal_panel"}"#).unwrap();
        let capture: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"capture_terminal_context","session_id":4}"#)
                .unwrap();
        let follow: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"set_terminal_context_follow","session_id":4,"enabled":true}"#,
        )
        .unwrap();

        assert!(matches!(
            resize_agent,
            AgentPanelMessage::SetAgentPanelWidth {
                width: 920.0,
                persist: true
            }
        ));
        assert!(matches!(detach_agent, AgentPanelMessage::DetachAgentPanel));
        assert!(matches!(attach_agent, AgentPanelMessage::AttachAgentPanel));
        assert!(matches!(open, AgentPanelMessage::OpenTerminal));
        assert!(matches!(
            capture,
            AgentPanelMessage::CaptureTerminalContext { session_id: 4 }
        ));
        assert!(matches!(
            follow,
            AgentPanelMessage::SetTerminalContextFollow {
                session_id: 4,
                enabled: true
            }
        ));
        assert!(matches!(
            input,
            AgentPanelMessage::TerminalWrite { session_id: 4, data } if data == "cargo test\r"
        ));
        assert!(matches!(
            reorder,
            AgentPanelMessage::ReorderTerminal {
                session_id: 4,
                before_session_id: Some(2)
            }
        ));
        assert!(matches!(
            dock,
            AgentPanelMessage::SetTerminalDock {
                dock: TerminalDock::Right
            }
        ));
        assert!(matches!(
            detach,
            AgentPanelMessage::DetachTerminalPanel {
                screen_x: Some(2200),
                screen_y: Some(640)
            }
        ));
        assert!(matches!(attach, AgentPanelMessage::AttachTerminalPanel));
    }

    #[test]
    fn parses_persistent_appearance_controls() {
        let theme: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"set_theme","theme":"central_dark"}"#).unwrap();
        let search: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"set_search_engine","search_engine":"google"}"#,
        )
        .unwrap();

        assert!(matches!(theme, AgentPanelMessage::SetTheme { theme } if theme == "central_dark"));
        assert!(
            matches!(search, AgentPanelMessage::SetSearchEngine { search_engine } if search_engine == "google")
        );
    }

    #[test]
    fn parses_legacy_floating_agent_graph_controls() {
        let open: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"open_knowledge_graph"}"#).unwrap();
        let close: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"close_knowledge_graph"}"#).unwrap();
        let begin: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"begin_knowledge_orb_drag","screen_x":412.5,"screen_y":219}"#,
        )
        .unwrap();
        let ready: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"knowledge_orb_drag_surface_ready"}"#).unwrap();
        let end: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"end_knowledge_orb_drag","screen_x":780,"screen_y":510.25,"open_graph":false}"#,
        )
        .unwrap();

        assert!(matches!(open, AgentPanelMessage::OpenAgentGraph));
        assert!(matches!(close, AgentPanelMessage::CloseAgentGraph));
        assert!(matches!(
            begin,
            AgentPanelMessage::BeginAgentGraphLauncherDrag {
                screen_x: 412.5,
                screen_y: 219.0
            }
        ));
        assert!(matches!(
            ready,
            AgentPanelMessage::AgentGraphLauncherDragSurfaceReady
        ));
        assert!(matches!(
            end,
            AgentPanelMessage::EndAgentGraphLauncherDrag {
                screen_x: 780.0,
                screen_y: 510.25,
                open_graph: false
            }
        ));

        assert!(matches!(
            serde_json::from_str::<AgentPanelMessage>(r#"{"message_type":"open_agent_graph"}"#)
                .unwrap(),
            AgentPanelMessage::OpenAgentGraph
        ));
        assert!(matches!(
            serde_json::from_str::<AgentPanelMessage>(
                r#"{"message_type":"begin_agent_graph_launcher_drag","screen_x":1,"screen_y":2}"#
            )
            .unwrap(),
            AgentPanelMessage::BeginAgentGraphLauncherDrag { .. }
        ));
    }

    #[test]
    fn parses_audit_retention_control() {
        let message: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"set_audit_retention","days":90}"#).unwrap();
        assert!(matches!(
            message,
            AgentPanelMessage::SetAuditRetention { days: 90 }
        ));
    }

    #[test]
    fn validates_terminal_commands_separately_from_browser_commands() {
        let request = AgentCommandRequest {
            request_id: "agent-1".to_owned(),
            command: AgentCommand::Terminal(TerminalCommand::Run {
                command: "cargo check".to_owned(),
            }),
        };

        assert!(request.validate().is_ok());
        assert_eq!(request.command.name(), "run_command");
        assert!(request.command.summary().contains("cargo check"));
    }

    #[test]
    fn distinguishes_obvious_shell_reads_from_workspace_mutations() {
        for command in [
            "Get-ChildItem -Force",
            "Get-Content Cargo.toml | Select-Object -First 40",
            "rg -n \"fn main\" src",
            "git -C . status --short",
            "cargo --version",
        ] {
            let action = AgentCommand::Terminal(TerminalCommand::Run {
                command: command.to_owned(),
            });
            assert_eq!(action.authorization().effect, ActionEffect::ReadOnly);
            assert!(!action.may_mutate_workspace(), "{command}");
        }

        for command in [
            "cargo test --workspace",
            "Get-Content Cargo.toml; Set-Content Cargo.toml changed",
            "rg needle src > matches.txt",
            "git checkout -- src/main.rs",
            "git diff --output=changes.patch",
            "find . -delete",
            "Get-ChildItem | Where-Object { Remove-Item $_; $true }",
            "Write-Output (Remove-Item -Recurse build)",
            "Get-Content ([IO.File]::Delete('important.txt'))",
        ] {
            let action = AgentCommand::Terminal(TerminalCommand::Run {
                command: command.to_owned(),
            });
            assert!(action.may_mutate_workspace(), "{command}");
            assert_eq!(action.authorization().effect, ActionEffect::Destructive);
        }
    }

    #[test]
    fn validates_ssh_commands_in_an_independent_scope() {
        let command = AgentCommand::Ssh(SshCommand::Run {
            profile_id: "0f98a929-c920-4673-8444-0725905ae5c3".to_owned(),
            profile_name: "Production".to_owned(),
            command: "uname -a".to_owned(),
        });

        assert!(command.validate().is_ok());
        assert_eq!(command.name(), "ssh_run");
        assert!(command.requires_approval());
        assert_eq!(command.authorization().scope, CapabilityScope::Ssh);
        assert_eq!(command.authorization().effect, ActionEffect::Destructive);
        assert!(command.summary().contains("Production"));
    }

    #[test]
    fn remote_desktop_observation_and_input_use_their_own_permission_scope() {
        let observe = AgentCommand::RemoteDesktop(RemoteDesktopCommand::Observe);
        assert!(observe.validate().is_ok());
        assert!(observe.requires_approval());
        assert_eq!(
            observe.authorization().scope,
            CapabilityScope::RemoteDesktop
        );
        assert_eq!(observe.authorization().effect, ActionEffect::ReadOnly);

        let click = AgentCommand::RemoteDesktop(RemoteDesktopCommand::Click {
            x: 640,
            y: 360,
            button: RemoteDesktopPointerButton::Left,
        });
        assert!(click.validate().is_ok());
        assert_eq!(click.authorization().effect, ActionEffect::ReversibleWrite);

        let invalid_scroll = AgentCommand::RemoteDesktop(RemoteDesktopCommand::Scroll {
            x: 1,
            y: 1,
            delta_y: 0,
        });
        assert!(invalid_scroll.validate().is_err());
    }

    #[test]
    fn parses_ssh_profile_and_live_session_controls() {
        let save: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"save_ssh_profile","id":null,"name":"Production","host":"vps.example.com","port":22,"username":"deploy","identity_file":null,"agent_enabled":true}"#,
        )
        .unwrap();
        let open: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"open_ssh_terminal","profile_id":"0f98a929-c920-4673-8444-0725905ae5c3"}"#,
        )
        .unwrap();
        let desktop: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"open_remote_desktop","profile_id":"0f98a929-c920-4673-8444-0725905ae5c3","protocol":"vnc","remote_port":5901}"#,
        )
        .unwrap();
        let arm: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"set_ssh_session_agent_ready","session_id":8,"ready":true}"#,
        )
        .unwrap();
        assert!(matches!(
            save,
            AgentPanelMessage::SaveSshProfile {
                port: 22,
                agent_enabled: true,
                ..
            }
        ));
        assert!(matches!(open, AgentPanelMessage::OpenSshTerminal { .. }));
        assert!(matches!(
            desktop,
            AgentPanelMessage::OpenRemoteDesktop {
                protocol: RemoteDesktopProtocol::Vnc,
                remote_port: 5901,
                ..
            }
        ));
        assert!(matches!(
            arm,
            AgentPanelMessage::SetSshSessionAgentReady {
                session_id: 8,
                ready: true
            }
        ));
    }

    #[test]
    fn managed_process_reads_are_safe_but_mutations_are_gated() {
        let status = AgentCommand::Process(ProcessCommand::Status {
            process_id: 8,
            stdout_offset: 0,
            stderr_offset: 0,
        });
        assert!(status.validate().is_ok());
        assert!(!status.requires_approval());
        assert_eq!(status.authorization().scope, CapabilityScope::Process);
        assert_eq!(status.authorization().effect, ActionEffect::ReadOnly);

        let start = AgentCommand::Process(ProcessCommand::Start {
            command: "cargo test".to_owned(),
            path: None,
        });
        assert!(start.validate().is_ok());
        assert!(start.requires_approval());
        assert_eq!(start.authorization().effect, ActionEffect::Destructive);

        let cancel = AgentCommand::Process(ProcessCommand::Cancel { process_id: 8 });
        assert_eq!(cancel.authorization().effect, ActionEffect::Destructive);
    }

    #[test]
    fn parses_direct_managed_process_cancellation() {
        let message: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"cancel_managed_process","process_id":4}"#)
                .unwrap();
        assert!(matches!(
            message,
            AgentPanelMessage::CancelManagedProcess { process_id: 4 }
        ));
    }

    #[test]
    fn parses_a_managed_process_preview_request() {
        let message: AgentPanelMessage = serde_json::from_str(
            r#"{"message_type":"open_process_preview","process_id":4,"url":"http://127.0.0.1:8080/"}"#,
        )
        .unwrap();
        assert!(matches!(
            message,
            AgentPanelMessage::OpenProcessPreview { process_id: 4, url }
                if url == "http://127.0.0.1:8080/"
        ));
    }

    #[test]
    fn classifies_window_inventory_control_and_close_separately() {
        let list = AgentCommand::Window(WindowCommand::List);
        assert!(!list.requires_approval());
        assert_eq!(list.authorization().effect, ActionEffect::ReadOnly);

        let focus = AgentCommand::Window(WindowCommand::Focus {
            window_id: "window-3-1".to_owned(),
        });
        assert!(focus.validate().is_ok());
        assert_eq!(
            focus.authorization().effect,
            ActionEffect::ExternalInteraction
        );

        let close = AgentCommand::Window(WindowCommand::Close {
            window_id: "window-3-1".to_owned(),
        });
        assert_eq!(close.authorization().effect, ActionEffect::Destructive);
        assert!(
            AgentCommand::Window(WindowCommand::Focus {
                window_id: "1234".to_owned()
            })
            .validate()
            .is_err()
        );
    }

    #[test]
    fn parses_window_access_controls() {
        let toggle: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"set_window_access","enabled":true}"#).unwrap();
        let refresh: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"refresh_windows"}"#).unwrap();
        assert!(matches!(
            toggle,
            AgentPanelMessage::SetWindowAccess { enabled: true }
        ));
        assert!(matches!(refresh, AgentPanelMessage::RefreshWindows));
    }

    #[test]
    fn parses_live_preview_controls() {
        let pause: AgentPanelMessage =
            serde_json::from_str(r#"{"message_type":"set_preview_live","enabled":false}"#).unwrap();
        assert!(matches!(
            pause,
            AgentPanelMessage::SetPreviewLive { enabled: false }
        ));
    }

    #[test]
    fn rejects_unknown_command() {
        let result = serde_json::from_str::<BrowserCommandRequest>(
            r#"{"request_id":"req-2","command":{"type":"run_shell"}}"#,
        );

        assert!(result.is_err());
    }

    #[test]
    fn rejects_oversized_text() {
        let request = BrowserCommandRequest {
            request_id: "req-3".to_owned(),
            command: BrowserCommand::SetText {
                target: "el-1".to_owned(),
                value: "x".repeat(MAX_TEXT_LEN + 1),
            },
        };

        assert!(request.validate().is_err());
    }

    #[test]
    fn text_summary_does_not_log_text_contents() {
        let command = BrowserCommand::SetText {
            target: "el-2".to_owned(),
            value: "dato privato".to_owned(),
        };

        let summary = command.summary();
        assert!(!summary.contains("dato privato"));
        assert!(summary.contains("12 characters"));
    }
}
