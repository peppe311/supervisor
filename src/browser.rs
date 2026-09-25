use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

mod agent_graph;
pub(crate) mod app_server;
mod board_operations;
#[cfg(windows)]
mod browser_use_backend;
#[cfg(windows)]
pub(crate) mod browser_use_bridge;
mod chat_ownership;
pub(crate) mod composer_drafts;
mod conversation_controls;
pub(crate) mod graph_contexts;
mod graph_delivery;
pub(crate) mod graph_files;
mod main_drafts;
mod main_runs;
mod native_targets;
mod project_activity;
pub(crate) mod project_board;
pub(crate) mod project_diff;
mod run_context;
mod snapshot_delivery;
mod submission_scope;
mod supervision;
mod work_results;
use agent_graph::*;
use anyhow::{Context, anyhow};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use submission_scope::SubmissionSnapshots;
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentConfigurationView<'a> {
    models: &'a [AgentModelOption],
    selection: &'a AgentSelection,
}
use directories::ProjectDirs;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use tracing::{error, info, warn};
use url::Url;
use uuid::Uuid;
#[cfg(windows)]
use webview2_com::{
    AcceleratorKeyPressedEventHandler, CallDevToolsProtocolMethodCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN, COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN,
    },
};
#[cfg(windows)]
use windows::{
    Win32::{
        Graphics::Dwm::{
            DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
            DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute,
        },
        Storage::FileSystem::{REPLACEFILE_WRITE_THROUGH, ReplaceFileW},
        UI::Input::KeyboardAndMouse::{GetAsyncKeyState, GetKeyState, VK_CONTROL, VK_MENU},
    },
    core::{HSTRING, PCWSTR},
};
#[cfg(windows)]
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy},
    monitor::MonitorHandle,
    window::{Theme, Window, WindowId},
};
use wry::{
    DragDropEvent, NewWindowResponse, PageLoadEvent, Rect, WebContext, WebView, WebViewBuilder,
    dpi::{PhysicalPosition, PhysicalSize},
};
#[cfg(windows)]
use wry::{WebViewBuilderExtWindows, WebViewExtWindows};

use crate::{
    agent_runtime::{
        AgentPhase, AgentPlanStepView, AgentRun, AgentRuntimeView, AgentStepKind, AgentStepStatus,
        MAX_CHAT_ATTACHMENTS,
    },
    agent_scripts,
    artifact::{
        ArtifactStore, ChatArtifact, ChatArtifactView, extract_citation_artifacts,
        normalize_display_path, open_with_default, show_in_folder,
    },
    audit_runtime::{AuditRuntime, AuditRuntimeView, DEFAULT_AUDIT_RETENTION_DAYS},
    brand,
    capture_runtime::{self, CapturedWindowImage},
    claude_provider::{
        self, ClaudeFailureKind, ClaudePermissionRequest, ClaudePermissionResult,
        ClaudeProbeResult, ClaudeProviderEvent, ClaudeRunHandle, ClaudeRunRequest, ClaudeRunResult,
        ClaudeStreamDelta, ClaudeStreamKind, ClaudeToolFinished, ClaudeToolStarted,
        ClaudeUsageLimit,
    },
    commands::{
        ActionLogEntry, AgentCommand, AgentCommandRequest, AgentPanelMessage,
        AgentSubmissionDelivery, BrowserCommand, CaptureCommand, CommandResultView, CommandStatus,
        MAX_TERMINAL_INPUT_LEN, ProcessCommand, RemoteDesktopCommand, RemoteDesktopModifier,
        RemoteDesktopPointerButton, SshCommand, TerminalCommand, TerminalDock, UiAutomationCommand,
        WindowCommand, WorkspaceCommand,
    },
    computer_use_frame::ComputerUseFrame,
    file_attachment::{
        AGENT_FILE_DIALOG_EXTENSIONS, FileAttachmentKind, FileAttachmentView, MAX_FILE_ATTACHMENTS,
        PendingFileAttachment,
    },
    markdown::render_safe_markdown,
    model_catalog::{self, ModelCatalogRefresh},
    navigation::{
        address_for_display, is_allowed_webview_navigation, is_supported_url,
        normalize_address_input,
    },
    permissions::{
        ActionAuthorization, ActionEffect, CapabilityScope, PermissionDecision, PermissionMode,
        PermissionPolicy,
    },
    preferences::{
        DEFAULT_SEARCH_ENGINE, DEFAULT_THEME, PreferenceOption, normalize_theme, search_engine,
        search_engine_options, theme_surface_color,
    },
    process_runtime::{ManagedProcessEvent, ManagedProcessRuntime},
    project_registry::{
        ProjectImportView, ProjectMetadata, RemoteProject, clone_repository, inspect_local_project,
        normalize_remote_directory, parse_remote_inspection, remote_inspection_script,
        remote_project_name,
    },
    provider::AgentProviderKind,
    provider_types::{
        self, AgentBrowserTab, AgentContextElement, AgentContextImage, AgentContextLink,
        AgentConversationMessage, AgentModelOption, AgentProviderView, AgentRemoteDesktop,
        AgentRunRequest, AgentSelection, AgentSshProfile, AgentTabContext, AgentTerminalContext,
    },
    remote_desktop::{
        RemoteDesktopEvent, RemoteDesktopMessage, RemoteDesktopPreference, RemoteDesktopProtocol,
        RemoteDesktopRuntime, RemoteDesktopView, reserve_loopback_port,
    },
    run_start_gate::{RunStartGate, RunStartGateRegistry},
    safety_runtime::{self, SafetyEvent, SafetyRuntime, SafetyView},
    ssh_runtime::{
        AUTOMATIC_SSH_CONFIG_FILE, SshProfile, SshProfileInput, SshRuntime, SshRuntimeView,
        SshUploadResult, load_automatic_ssh_profiles, run_batch_ssh_command, run_ssh_upload,
    },
    subscription_provider::{
        self, SubscriptionPermissionRequest, SubscriptionPermissionResult, SubscriptionProbeResult,
        SubscriptionProviderEvent, SubscriptionRunHandle, SubscriptionRunRequest,
        SubscriptionRunResult, SubscriptionStreamDelta, SubscriptionStreamKind,
        SubscriptionToolFinished, SubscriptionToolStarted,
    },
    tab_context::{
        TabContextSnapshot, TabContextView, decode_visual_capture, sanitize_context_title,
        sanitize_context_url,
    },
    terminal::{
        SSH_RUNTIME_DISCOVERY_ACTION, TerminalContextSnapshot, TerminalEvent, TerminalKind,
        TerminalPhase, TerminalRuntime, TerminalView, extract_ssh_capability_probe,
        parse_ssh_capability_probe, strip_ssh_capability_probe,
    },
    time_machine::{CheckpointSummary, LiveDiffStats, TimeMachineRuntime, TimeMachineView},
    ui_automation::{self, UiAutomationRuntime, UiAutomationTaskResult, UiAutomationView},
    ui_development,
    window_runtime::{WindowRuntime, WindowRuntimeView},
    workspace::{WorkspaceRuntime, WorkspaceView},
};

const INITIAL_WINDOW_WIDTH_LOGICAL: f64 = 1_920.0;
const INITIAL_WINDOW_HEIGHT_LOGICAL: f64 = 1_080.0;
const WINDOW_CHROME_WIDTH_ALLOWANCE_LOGICAL: f64 = 32.0;
const WINDOW_CHROME_HEIGHT_ALLOWANCE_LOGICAL: f64 = 48.0;
const TOOLBAR_HEIGHT_FULL_HD_LOGICAL: f64 = 104.0;
const TOOLBAR_HEIGHT_2K_LOGICAL: f64 = 116.0;
const TOOLBAR_HEIGHT_4K_LOGICAL: f64 = 128.0;
const TOOLBAR_NAVIGATION_HEIGHT_LOGICAL: f64 = 60.0;
const PROJECT_TOOLBAR_HEIGHT_LOGICAL: f64 = 40.0;
const AGENT_PANEL_WIDTH_FULL_HD_LOGICAL: f64 = 800.0;
const AGENT_PANEL_WIDTH_2K_LOGICAL: f64 = 1_000.0;
const AGENT_PANEL_WIDTH_4K_LOGICAL: f64 = 1_280.0;
const AGENT_PANEL_MIN_WIDTH_LOGICAL: f64 = 560.0;
const RESUME_INTERRUPTED_PROMPT: &str = "Riprendi il lavoro interrotto dal punto in cui ti sei fermato. Verifica lo stato attuale e completa la richiesta precedente.";
const BROWSER_PANE_MIN_WIDTH_LOGICAL: f64 = 420.0;
const TERMINAL_WIDTH_FULL_HD_LOGICAL: f64 = 520.0;
const TERMINAL_WIDTH_2K_LOGICAL: f64 = 620.0;
const TERMINAL_WIDTH_4K_LOGICAL: f64 = 760.0;
const TERMINAL_HEIGHT_FULL_HD_LOGICAL: f64 = 320.0;
const TERMINAL_HEIGHT_2K_LOGICAL: f64 = 390.0;
const TERMINAL_HEIGHT_4K_LOGICAL: f64 = 520.0;
const DETACHED_TAB_WIDTH_LOGICAL: f64 = 1_280.0;
const DETACHED_TAB_HEIGHT_LOGICAL: f64 = 800.0;
const DETACHED_AGENT_PANEL_WIDTH_LOGICAL: f64 = 1_180.0;
const DETACHED_AGENT_PANEL_HEIGHT_LOGICAL: f64 = 850.0;
const DETACHED_TERMINAL_WIDTH_LOGICAL: f64 = 1_080.0;
const DETACHED_TERMINAL_HEIGHT_LOGICAL: f64 = 720.0;
const DETACHED_PREVIEW_WIDTH_LOGICAL: f64 = 1_100.0;
const DETACHED_PREVIEW_HEIGHT_LOGICAL: f64 = 720.0;
const MAX_TERMINAL_CHAT_ATTACHMENTS: usize = 4;
const TERMINAL_LIVE_UI_INTERVAL_MS: u128 = 300;
const MAX_AGENT_GRAPH_BINDINGS: usize = 240;
const MAX_AGENT_GRAPH_NAME_CHARS: usize = 80;
const MAX_AGENT_GRAPH_MISSION_CHARS: usize = 2_000;
const LINKED_AGENT_CONTEXT_RECENT_TURNS: usize = 1;
const MAX_LINKED_AGENT_CONTEXTS: usize = 8;
const MAX_AGENT_GRAPH_CONTEXT_MESSAGES_PER_TURN: usize = 4;
const MAX_AGENT_GRAPH_CONTEXT_STEPS_PER_TURN: usize = 8;
const MAX_AGENT_GRAPH_CONTEXT_TEXT_CHARS: usize = 4_000;
const AGENT_GRAPH_HISTORY_PAGE_SIZE: usize = 20;
const AGENT_GRAPH_HISTORY_FILE: &str = "knowledge-agent-history.json";
const PROJECT_CHAT_HISTORY_FILE: &str = "project-chat-history.json";
const MAX_PROJECT_LABEL_CHARS: usize = 80;
const PROJECT_METADATA_FRESH_MS: u64 = 5 * 60 * 1_000;
const MAX_PROJECT_CHAT_TITLE_CHARS: usize = 120;
const AGENT_GRAPH_LAUNCHER_SIZE_FULL_HD_LOGICAL: f64 = 112.0;
const AGENT_GRAPH_LAUNCHER_SIZE_2K_LOGICAL: f64 = 128.0;
const AGENT_GRAPH_LAUNCHER_SIZE_4K_LOGICAL: f64 = 136.0;
const START_PAGE_MAIN_HEIGHT_FULL_HD_LOGICAL: f64 = 295.8;
const START_PAGE_MAIN_HEIGHT_2K_LOGICAL: f64 = 337.24;
const START_PAGE_MAIN_HEIGHT_4K_LOGICAL: f64 = 383.4;
const DEFAULT_TITLE: &str = "New tab";
const TOOLBAR_HTML: &str = include_str!("../assets/toolbar.html");
const AGENT_PANEL_HTML: &str = include_str!("../assets/agent-panel.html");
const AGENT_GRAPH_HTML: &str = include_str!("../assets/agent-graph.html");
const PREVIEW_HTML: &str = include_str!("../assets/preview.html");
const REMOTE_DESKTOP_HTML: &str = include_str!("../assets/remote-desktop.html");
const START_PAGE_HTML: &str = include_str!("../assets/start-page.html");
const SUPERVISOR_WORDMARK_SVG: &str = include_str!("../assets/supervisor-wordmark.svg");
const BACKDROP_HTML: &str = include_str!("../assets/backdrop.html");
const THEMES_CSS: &str = include_str!("../assets/themes.css");
const SUPERVISOR_UI_FONT: &[u8] = include_bytes!("../assets/fonts/inter/Inter-Variable.woff2");
const FILE_ICONS_JS: &str = include_str!("../assets/file-icons.js");
const SVELTE_UI_CSS: &str = include_str!("../ui/dist/central-agent-ui.css");
const SVELTE_UI_JS: &str = include_str!("../ui/dist/central-agent-ui.js");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UiLayoutProfile {
    FullHd,
    TwoK,
    FourK,
}

fn visual_capture_params(page_width: u32, page_height: u32) -> (String, bool) {
    const MAX_SOURCE_WIDTH: u32 = 12_000;
    const MAX_SOURCE_HEIGHT: u32 = 25_000;
    const MAX_OUTPUT_WIDTH: f64 = 1_280.0;
    const MAX_OUTPUT_HEIGHT: f64 = 6_000.0;
    const MAX_OUTPUT_PIXELS: f64 = 3_000_000.0;

    let source_width = if page_width == 0 { 1_280 } else { page_width };
    let source_height = if page_height == 0 { 800 } else { page_height };
    let clip_width = source_width.min(MAX_SOURCE_WIDTH);
    let clip_height = source_height.min(MAX_SOURCE_HEIGHT);
    let width = f64::from(clip_width);
    let height = f64::from(clip_height);
    let scale = 1.0_f64
        .min(MAX_OUTPUT_WIDTH / width)
        .min(MAX_OUTPUT_HEIGHT / height)
        .min((MAX_OUTPUT_PIXELS / (width * height)).sqrt())
        .max(0.1);
    let truncated = source_width > clip_width || source_height > clip_height;
    (
        json!({
            "format": "jpeg",
            "quality": 42,
            "fromSurface": true,
            "captureBeyondViewport": true,
            "optimizeForSpeed": true,
            "clip": {
                "x": 0,
                "y": 0,
                "width": clip_width,
                "height": clip_height,
                "scale": scale,
            }
        })
        .to_string(),
        truncated,
    )
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TabDockSide {
    Left,
    #[default]
    Right,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ToolbarCommand {
    Navigate {
        value: String,
    },
    Back,
    Forward,
    Reload,
    Stop,
    Home,
    NewTab,
    CloseTab {
        tab_id: u64,
    },
    ActivateTab {
        tab_id: u64,
    },
    ReorderTab {
        tab_id: u64,
        #[serde(default)]
        before_tab_id: Option<u64>,
    },
    DockTab {
        tab_id: u64,
        side: TabDockSide,
    },
    UndockTab {
        tab_id: u64,
    },
    DetachTab {
        tab_id: u64,
        #[serde(default)]
        screen_x: Option<i32>,
        #[serde(default)]
        screen_y: Option<i32>,
    },
    AttachTab {
        tab_id: u64,
    },
    OpenSettings,
    ToggleAgentPanel,
    ToggleBrowserPanel,
    AgentPanelCloseLayoutReady,
    ToggleTerminalPanel,
    CloseProjectBoard,
}

impl ToolbarCommand {
    fn targets_browser(&self) -> bool {
        matches!(
            self,
            Self::Navigate { .. }
                | Self::Back
                | Self::Forward
                | Self::Reload
                | Self::Stop
                | Self::Home
                | Self::NewTab
                | Self::CloseTab { .. }
                | Self::ActivateTab { .. }
                | Self::ReorderTab { .. }
                | Self::DockTab { .. }
                | Self::UndockTab { .. }
                | Self::DetachTab { .. }
                | Self::AttachTab { .. }
                | Self::ToggleBrowserPanel
        )
    }
}

pub(crate) enum BrowserEvent {
    #[cfg(windows)]
    SystemTray(crate::system_tray::Action),
    #[cfg(windows)]
    BrowserUseBridge(browser_use_bridge::Request),
    Toolbar(ToolbarCommand),
    ToolbarReady,
    AgentPanel(AgentPanelMessage),
    ScopedAgentPanel {
        owner: String,
        message: AgentPanelMessage,
    },
    AgentPanelReady,
    ChatZoom(ChatZoomAction),
    AgentGraphSurfaceReady,
    TerminalPanel(AgentPanelMessage),
    TerminalPanelReady,
    PreviewPanelReady,
    RemoteDesktopPanel {
        tab_id: u64,
        message: RemoteDesktopMessage,
    },
    RemoteDesktop(RemoteDesktopEvent),
    RemoteDesktopDrag {
        tab_id: u64,
        event: RemoteDesktopDragEvent,
    },
    RemoteDesktopUploadCompleted {
        profile_id: String,
        result: Result<SshUploadResult, String>,
    },
    AgentGraphProjectDrop(ProjectDropEvent),
    AgentGraphFileDropResolved {
        paths: Vec<PathBuf>,
        owner: Option<String>,
    },
    ProjectInspectionCompleted {
        key: String,
        result: ProjectMetadata,
    },
    ProjectCloneCompleted {
        operation_id: String,
        result: Result<PathBuf, String>,
    },
    PageLoad {
        tab_id: u64,
        loading: bool,
        url: String,
    },
    TitleChanged {
        tab_id: u64,
        title: String,
    },
    #[cfg(windows)]
    BrowserPopup {
        opener_tab_id: u64,
        url: String,
    },
    OpenInNewTab(String),
    ScriptResult {
        request_id: String,
        action: String,
        raw_result: String,
    },
    TabContextResult {
        tab_id: u64,
        capture_id: u64,
        raw_result: String,
    },
    TabVisualResult {
        tab_id: u64,
        capture_id: u64,
        raw_result: String,
        truncated: bool,
    },

    ProjectDiff(project_diff::Reply),
    BoardWorktreeCompleted {
        operation_id: String,
        name: String,
        result: Result<PathBuf, String>,
    },
    AppServer(app_server::Event),

    TimeMachineStarted {
        run_id: u64,
        result: Result<(), String>,
    },
    TimeMachineFinished {
        run_id: u64,
        result: Result<Option<CheckpointSummary>, String>,
    },
    TimeMachineLiveDiffTick {
        run_id: u64,
    },
    TimeMachineLiveDiffUpdated {
        run_id: u64,
        result: Result<Option<LiveDiffStats>, String>,
    },
    SupervisorReviewTick {
        node_key: String,
        generation: u64,
    },
    ClaudeProviderProbed(ClaudeProbeResult),
    ClaudeLoginCompleted(Result<(), String>),
    ClaudeProvider(ClaudeProviderEvent),
    SubscriptionProviderProbed {
        provider: AgentProviderKind,
        result: SubscriptionProbeResult,
    },
    SubscriptionLoginCompleted {
        provider: AgentProviderKind,
        result: Result<(), String>,
    },
    SubscriptionProvider {
        provider: AgentProviderKind,
        event: SubscriptionProviderEvent,
    },
    Terminal(TerminalEvent),
    TerminalContextRefresh {
        session_id: u64,
    },
    ManagedProcess(ManagedProcessEvent),
    WindowCapture {
        request_id: Option<String>,
        action: String,
        generation: u64,
        result: Result<CapturedWindowImage, String>,
    },
    WindowCaptureTick {
        generation: u64,
    },
    UiAutomation {
        request_id: String,
        action: String,
        result: Result<UiAutomationTaskResult, String>,
    },
    Safety(SafetyEvent),
    DevelopmentUiChanged(Vec<String>),
}

#[derive(Debug)]
pub(crate) enum ProjectDropEvent {
    Enter {
        directory_count: usize,
        file_count: usize,
        position: (i32, i32),
    },
    Over {
        position: (i32, i32),
    },
    Drop {
        paths: Vec<PathBuf>,
        position: (i32, i32),
    },
    Leave,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChatZoomAction {
    Increase,
    Decrease,
    Reset,
}

impl ChatZoomAction {
    fn javascript_argument(self) -> &'static str {
        match self {
            Self::Increase => "\"increase\"",
            Self::Decrease => "\"decrease\"",
            Self::Reset => "\"reset\"",
        }
    }
}

const CHAT_ZOOM_VK_ZERO: u32 = 0x30;
const CHAT_ZOOM_VK_NUMPAD_ZERO: u32 = 0x60;
const CHAT_ZOOM_VK_ADD: u32 = 0x6b;
const CHAT_ZOOM_VK_SUBTRACT: u32 = 0x6d;
const CHAT_ZOOM_VK_OEM_PLUS: u32 = 0xbb;
const CHAT_ZOOM_VK_OEM_MINUS: u32 = 0xbd;

fn chat_zoom_action_for_accelerator(
    key_down: bool,
    control_down: bool,
    alt_down: bool,
    virtual_key: u32,
) -> Option<ChatZoomAction> {
    if !key_down || !control_down || alt_down {
        return None;
    }
    match virtual_key {
        CHAT_ZOOM_VK_ADD | CHAT_ZOOM_VK_OEM_PLUS => Some(ChatZoomAction::Increase),
        CHAT_ZOOM_VK_SUBTRACT | CHAT_ZOOM_VK_OEM_MINUS => Some(ChatZoomAction::Decrease),
        CHAT_ZOOM_VK_ZERO | CHAT_ZOOM_VK_NUMPAD_ZERO => Some(ChatZoomAction::Reset),
        _ => None,
    }
}

#[cfg(windows)]
fn virtual_key_is_down(virtual_key: u16) -> bool {
    let virtual_key = i32::from(virtual_key);
    // `GetKeyState` reflects the queue that delivered the WebView2 event, while
    // `GetAsyncKeyState` reflects the physical state. Accept either so the
    // modifier remains reliable across child HWND focus and keyboard layouts.
    unsafe { GetKeyState(virtual_key) < 0 || GetAsyncKeyState(virtual_key) < 0 }
}

#[cfg(windows)]
fn install_chat_zoom_accelerator(
    webview: &WebView,
    proxy: EventLoopProxy<BrowserEvent>,
) -> anyhow::Result<()> {
    let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_sender, arguments| {
        let Some(arguments) = arguments else {
            return Ok(());
        };
        let mut key_event_kind = COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN;
        let mut virtual_key = 0;
        unsafe {
            arguments.KeyEventKind(&mut key_event_kind)?;
            arguments.VirtualKey(&mut virtual_key)?;
        }
        let key_down = matches!(
            key_event_kind,
            COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN | COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
        );
        let control_down = virtual_key_is_down(VK_CONTROL.0);
        let alt_down = virtual_key_is_down(VK_MENU.0);
        let Some(action) =
            chat_zoom_action_for_accelerator(key_down, control_down, alt_down, virtual_key)
        else {
            return Ok(());
        };

        // Consume only the three chat zoom accelerators. All other browser shortcuts keep
        // their native WebView2 behavior.
        unsafe { arguments.SetHandled(true)? };
        let _ = proxy.send_event(BrowserEvent::ChatZoom(action));
        Ok(())
    }));
    let mut token = 0;
    unsafe {
        webview
            .controller()
            .add_AcceleratorKeyPressed(&handler, &mut token)
    }
    .context("failed to install the application chat zoom accelerator")
}

#[cfg(not(windows))]
fn install_chat_zoom_accelerator(
    _webview: &WebView,
    _proxy: EventLoopProxy<BrowserEvent>,
) -> anyhow::Result<()> {
    Ok(())
}

pub(crate) enum RemoteDesktopDragEvent {
    Enter { file_count: usize },
    Leave,
    Drop { paths: Vec<PathBuf> },
}

struct BrowserTab {
    id: u64,
    title: String,
    url: String,
    loading: bool,
    ready_to_show: bool,
    content_revision: u64,
    is_start_page: bool,
    web_preview: Option<WebPreviewMetadata>,
    webview: WebView,
    detached_window: Option<Window>,
}

struct WebPreviewMetadata {
    owner_process_id: u64,
    origin: String,
    process_running: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolbarState {
    browser_panel_minimized: bool,
    browser_panel_toggle_enabled: bool,
    browser_tabs_visible: bool,
    project_board_open: bool,
    tabs: Vec<ToolbarTab>,
    active_tab_id: Option<u64>,
    primary_tab_id: Option<u64>,
    address: String,
    can_go_back: bool,
    can_go_forward: bool,
    loading: bool,
    settings_open: bool,
    agent_panel_visible: bool,
    terminal_panel_visible: bool,
    terminal_detached: bool,
    terminal_session_count: usize,
    terminal_dock: TerminalDock,
    theme: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolbarTab {
    id: u64,
    title: String,
    loading: bool,
    primary: bool,
    docked: bool,
    dock_side: Option<TabDockSide>,
    detached: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentPanelState<'a> {
    app_server: &'a app_server::View,
    conversation_state: Value,
    runtime_busy: bool,

    browser_panel_minimized: bool,
    settings_open: bool,
    agent_panel_detached: bool,
    active_page: Option<AgentPanelPage>,
    tabs: Vec<AgentPanelTab>,
    action_log: Vec<ActionLogEntry>,
    last_result: Option<CommandResultView>,
    capabilities: Vec<&'static str>,
    permission: PermissionPanelState,
    agent: AgentRuntimeView,
    agent_can_resume: bool,
    queued_agent_request: bool,
    agent_submission_queue_count: usize,
    discardable_checkpoint_run_id: Option<u64>,
    chat_messages: Vec<RenderedChatMessage<'a>>,
    tab_contexts: Vec<TabContextView>,
    terminal_contexts: Vec<TerminalContextDraftView>,
    file_attachments: Vec<FileAttachmentView>,
    provider: &'a AgentProviderView,
    providers: AgentProvidersView<'a>,
    agent_configuration: AgentConfigurationView<'a>,

    terminal: TerminalView<'a>,
    ssh: SshRuntimeView<'a>,
    remote_desktop: RemoteDesktopView<'a>,
    terminal_panel_visible: bool,
    terminal_detached: bool,
    terminal_dock: TerminalDock,
    system: SystemPanelState,
    hardening: HardeningPanelState,
    preview: PreviewPanelState,
    appearance: AppearancePanelState,
    workspace: WorkspaceView,
    project_chats: Vec<ProjectChatSummaryView>,
    active_project_chat_id: Option<&'a str>,
    #[serde(rename = "knowledgeAgents")]
    agent_graph: AgentGraphAgentsView<'a>,
    time_machine: TimeMachineView,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentProvidersView<'a> {
    selected: AgentProviderKind,
    codex_app_server: &'a AgentProviderView,

    claude_code: &'a AgentProviderView,
    cursor: &'a AgentProviderView,
    github_copilot: &'a AgentProviderView,
    google_antigravity: &'a AgentProviderView,
    opencode_go: &'a AgentProviderView,
    claude_session_id: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentGraphAgentsView<'a> {
    bindings: &'a [AgentGraphBinding],
    active_node_keys: Vec<&'a str>,
    links: &'a [AgentGraphLink],
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentGraphSurfaceState<'a> {
    expanded: bool,
    dragging: bool,
    theme: &'a str,
    orb: AgentGraphLauncherView,
    graph: AgentGraphView,
    project_registry: ProjectRegistryView,
    project_chats: Vec<ProjectChatSummaryView>,
    project_chat_previews: Vec<ProjectChatPreviewView<'a>>,
    active_chat_id: Option<&'a str>,
    workspace: WorkspaceView,
    #[serde(rename = "knowledgeAgents")]
    agent_graph: AgentGraphAgentsView<'a>,
    providers: Vec<AgentGraphProviderConfigurationView<'a>>,
    #[serde(rename = "knowledgeRuns")]
    agent_graph_runs: Vec<AgentGraphRunView<'a>>,
    pending_approvals: Vec<AgentGraphPendingApprovalView>,
    permission_mode: PermissionMode,
    session_authorized: bool,
    queued_agent_request: bool,
    discardable_checkpoint_run_id: Option<u64>,
    time_machine: TimeMachineView,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectRegistryView {
    projects: Vec<ProjectCardView>,
    ssh_profiles: Vec<ProjectSshProfileView>,
    import: ProjectImportView,
    drop_active: bool,
    add_request_id: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectCardView {
    id: String,
    node_key: String,
    source: &'static str,
    name: String,
    path: String,
    active: bool,
    pinned: bool,
    last_opened_at_ms: u64,
    agent_count: usize,
    access: &'static str,
    ssh_profile_id: Option<String>,
    ssh_profile_name: Option<String>,
    metadata: Option<ProjectMetadata>,
    activity: Vec<project_activity::Work>,
    scanning: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectSshProfileView {
    id: String,
    name: String,
    target: String,
    agent_enabled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentGraphProviderConfigurationView<'a> {
    id: &'static str,
    label: &'static str,
    provider: &'a AgentProviderView,
    models: &'a [AgentModelOption],
    selection: &'a AgentSelection,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentGraphPendingApprovalView {
    node_key: String,
    approval: PendingCommandView,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentGraphRunView<'a> {
    conversation_state: Value,
    supervised_chat: Option<ProjectChatPreviewView<'a>>,
    supervision_automatic: bool,
    verification: Option<supervision::DeliveryView>,

    node_key: String,
    graph_node_key: String,
    conversation_id: Option<Uuid>,
    agent_name: &'a str,
    project_directory: String,
    provider: AgentProviderKind,
    selection: &'a AgentSelection,
    agent: AgentRuntimeView,
    messages: Vec<RenderedChatMessage<'a>>,
    history: Vec<AgentGraphTurnView<'a>>,
    history_total: usize,
    history_offset: usize,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentGraphLauncherView {
    x: f64,
    y: f64,
    size: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppearancePanelState {
    theme: String,
    search_engine: String,
    search_engines: Vec<PreferenceOption>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemPanelState {
    window_access_enabled: bool,
    windows: WindowRuntimeView,
    ui_automation: UiAutomationView,
    safety: SafetyView,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HardeningPanelState {
    audit: AuditRuntimeView,
    isolation_backend: &'static str,
    process_sandboxed: bool,
    release_ready: bool,
    status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreviewPanelState {
    visible: bool,
    capturing: bool,
    title: Option<String>,
    source_type: Option<&'static str>,
    pid: Option<u32>,
    owned: bool,
    width: Option<u32>,
    height: Option<u32>,
    byte_count: Option<usize>,
    owner_process_id: Option<u64>,
    live: bool,
    detached: bool,
    status: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativePreviewSource {
    NativeWindow,
    GameWindow,
}

impl NativePreviewSource {
    fn id(self) -> &'static str {
        match self {
            Self::NativeWindow => "native_window",
            Self::GameWindow => "game_window",
        }
    }
}

#[derive(Default)]
struct PreviewRuntime {
    image: Option<CapturedWindowImage>,
    target: Option<crate::window_runtime::WindowCaptureTarget>,
    owner_process_id: Option<u64>,
    source_type: Option<NativePreviewSource>,
    visible: bool,
    capturing: bool,
    live: bool,
    generation: u64,
    status: String,
}

impl PreviewRuntime {
    fn view(&self, detached: bool) -> PreviewPanelState {
        let image = self.image.as_ref();
        PreviewPanelState {
            visible: self.visible && !detached,
            capturing: self.capturing,
            title: image
                .map(|image| image.title.clone())
                .or_else(|| self.target.as_ref().map(|target| target.title.clone())),
            source_type: self.source_type.map(NativePreviewSource::id),
            pid: image
                .map(|image| image.pid)
                .or_else(|| self.target.as_ref().map(|target| target.pid)),
            owned: image.is_some_and(|image| image.owned)
                || self.target.as_ref().is_some_and(|target| target.owned),
            width: image.map(|image| image.width),
            height: image.map(|image| image.height),
            byte_count: image.map(|image| image.jpeg.len()),
            owner_process_id: self.owner_process_id,
            live: self.live,
            detached,
            status: self.status.clone(),
        }
    }

    fn begin(
        &mut self,
        target: crate::window_runtime::WindowCaptureTarget,
        owner_process_id: Option<u64>,
        source_type: NativePreviewSource,
        live: bool,
    ) -> u64 {
        self.generation = self.generation.saturating_add(1);
        self.image = None;
        self.target = Some(target);
        self.owner_process_id = owner_process_id;
        self.source_type = Some(source_type);
        self.visible = true;
        self.capturing = true;
        self.live = live;
        self.status = if live {
            "Starting bounded live preview…".to_owned()
        } else {
            "Capturing application window…".to_owned()
        };
        self.generation
    }

    fn stop_live(&mut self, status: impl Into<String>) {
        self.generation = self.generation.saturating_add(1);
        self.capturing = false;
        self.live = false;
        self.status = status.into();
    }

    fn resume_live(&mut self) -> Result<(u64, crate::window_runtime::WindowCaptureTarget), String> {
        let target = self
            .target
            .clone()
            .ok_or_else(|| "Open a native application preview first".to_owned())?;
        self.generation = self.generation.saturating_add(1);
        self.visible = true;
        self.capturing = true;
        self.live = true;
        self.status = "Resuming bounded live preview…".to_owned();
        Ok((self.generation, target))
    }

    fn close(&mut self) {
        self.generation = self.generation.saturating_add(1);
        self.image = None;
        self.target = None;
        self.owner_process_id = None;
        self.source_type = None;
        self.visible = false;
        self.capturing = false;
        self.live = false;
        self.status.clear();
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentPanelPage {
    tab_id: u64,
    title: String,
    url: String,
    loading: bool,
    preview_source_type: Option<&'static str>,
    preview_owner_process_id: Option<u64>,
    preview_process_running: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentPanelTab {
    id: u64,
    title: String,
    url: String,
    active: bool,
    preview_source_type: Option<&'static str>,
    preview_owner_process_id: Option<u64>,
    preview_process_running: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatMessage {
    id: u64,
    role: ChatRole,
    kind: ChatMessageKind,
    text: String,
    streaming: bool,
    timestamp_ms: u128,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    native_turn_id: Option<String>,

    attachments: Vec<ChatTabAttachment>,
    terminal_attachments: Vec<ChatTerminalAttachment>,
    file_attachments: Vec<FileAttachmentView>,
    #[serde(default)]
    artifacts: Vec<ChatArtifact>,
    provider: Option<AgentProviderKind>,
    checkpoint: Option<CheckpointSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    activity_status: Option<AgentStepStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    activity_category: Option<AgentActivityCategory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    activity_detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    activity_context: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    activity_additions: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    activity_deletions: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    activity_diff: Option<String>,
    #[serde(default)]
    run_id: Option<u64>,
    #[serde(default)]
    provider_item_id: Option<String>,
    #[serde(default)]
    reasoning_summary_index: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectChat {
    id: String,
    project_root: String,
    title: String,
    #[serde(default)]
    title_custom: bool,
    #[serde(default)]
    pinned: bool,
    archived: bool,
    created_at_ms: u128,
    updated_at_ms: u128,
    #[serde(default)]
    messages: Vec<ChatMessage>,
    #[serde(default)]
    claude_session_id: Option<String>,
    #[serde(default)]
    claude_session_cwd: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectChatHistory {
    #[serde(default)]
    chats: Vec<ProjectChat>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectChatSummaryView {
    lineage: Option<chat_ownership::ChatLineageView>,
    agent_active: bool,
    needs_attention: bool,
    mutation_locked: bool,
    verification: Option<supervision::DeliveryView>,
    id: String,
    project_root: String,
    title: String,
    pinned: bool,
    archived: bool,
    active: bool,
    message_count: usize,
    updated_at_ms: u128,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectChatPreviewView<'a> {
    id: &'a str,
    title: String,
    provider: AgentProviderKind,
    selection: AgentSelection,
    agent: AgentRuntimeView,
    can_resume: bool,
    pending_approval: Option<PendingCommandView>,
    conversation_state: Value,
    messages: Vec<RenderedChatMessage<'a>>,
}

#[derive(Clone, Debug)]
struct ProjectChatCardProfile {
    provider: AgentProviderKind,
    selection: AgentSelection,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RenderedChatMessage<'a> {
    #[serde(flatten)]
    message: &'a ChatMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    rendered_html: Option<String>,
    artifacts: Vec<ChatArtifactView<'a>>,
}

impl<'a> RenderedChatMessage<'a> {
    fn new(message: &'a ChatMessage, artifact_store: &'a ArtifactStore) -> Self {
        Self {
            message,
            rendered_html: rendered_message_html(message.role, message.kind, &message.text),
            artifacts: message
                .artifacts
                .iter()
                .map(|artifact| artifact_store.view(artifact))
                .collect(),
        }
    }
}

fn rendered_message_html(role: ChatRole, kind: ChatMessageKind, text: &str) -> Option<String> {
    (role == ChatRole::Assistant
        && matches!(kind, ChatMessageKind::Message | ChatMessageKind::Reasoning))
    .then(|| render_safe_markdown(text))
}

fn extend_unique_artifacts(
    target: &mut Vec<ChatArtifact>,
    artifacts: impl IntoIterator<Item = ChatArtifact>,
) {
    let mut ids = target
        .iter()
        .map(|artifact| artifact.id.clone())
        .collect::<HashSet<_>>();
    for artifact in artifacts {
        if ids.insert(artifact.id.clone()) {
            target.push(artifact);
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum AgentActivityCategory {
    Command,
    File,
    Read,
    Browser,
    Tool,
}

#[derive(Clone, Debug)]
struct AgentActivityMetadata {
    category: AgentActivityCategory,
    detail: Option<String>,
    context: Option<String>,
    additions: Option<usize>,
    deletions: Option<usize>,
    diff: Option<String>,
}

impl AgentActivityMetadata {
    fn reported_files(files: Option<&crate::provider_types::ToolFileActivity>) -> Self {
        let Some(files) = files else {
            return Self::generic();
        };
        Self {
            category: if files.editing {
                AgentActivityCategory::File
            } else {
                AgentActivityCategory::Read
            },
            context: Some(files.paths.join("\n")),
            ..Self::generic()
        }
    }

    fn generic() -> Self {
        Self {
            category: AgentActivityCategory::Tool,
            detail: None,
            context: None,
            additions: None,
            deletions: None,
            diff: None,
        }
    }
}

impl ChatMessage {
    fn reasoning(id: u64, run_id: u64, item_id: String, summary_index: usize, text: &str) -> Self {
        Self {
            id,
            role: ChatRole::Assistant,
            kind: ChatMessageKind::Reasoning,
            text: text.to_owned(),
            streaming: true,
            timestamp_ms: unix_time_ms(),
            attachments: Vec::new(),
            terminal_attachments: Vec::new(),
            file_attachments: Vec::new(),
            artifacts: Vec::new(),
            provider: None,
            checkpoint: None,
            activity_status: None,
            activity_category: None,
            activity_detail: None,
            activity_context: None,
            activity_additions: None,
            activity_deletions: None,
            activity_diff: None,
            run_id: Some(run_id),
            provider_item_id: Some(item_id),
            reasoning_summary_index: Some(summary_index),

            native_turn_id: None,
        }
    }

    fn activity(
        id: u64,
        run_id: u64,
        item_id: String,
        provider: AgentProviderKind,
        label: &str,
        metadata: AgentActivityMetadata,
    ) -> Self {
        Self {
            id,
            role: ChatRole::Assistant,
            kind: ChatMessageKind::Activity,
            text: label.trim().to_owned(),
            streaming: true,
            timestamp_ms: unix_time_ms(),
            attachments: Vec::new(),
            terminal_attachments: Vec::new(),
            file_attachments: Vec::new(),
            artifacts: Vec::new(),
            provider: Some(provider),
            checkpoint: None,
            activity_status: Some(AgentStepStatus::Running),
            activity_category: Some(metadata.category),
            activity_detail: metadata.detail,
            activity_context: metadata.context,
            activity_additions: metadata.additions,
            activity_deletions: metadata.deletions,
            activity_diff: metadata.diff,
            run_id: Some(run_id),
            provider_item_id: Some(item_id),
            reasoning_summary_index: None,

            native_turn_id: None,
        }
    }

    fn is_activity(&self, run_id: u64, item_id: &str) -> bool {
        self.kind == ChatMessageKind::Activity
            && self.run_id == Some(run_id)
            && self.provider_item_id.as_deref() == Some(item_id)
    }

    fn set_activity_status(&mut self, status: AgentStepStatus, detail: Option<&str>) {
        self.activity_status = Some(status);
        self.streaming = matches!(
            status,
            AgentStepStatus::Queued | AgentStepStatus::AwaitingApproval | AgentStepStatus::Running
        );
        let headline = self.text.lines().next().unwrap_or_default().to_owned();
        self.text = detail
            .map(str::trim)
            .filter(|detail| !detail.is_empty())
            .map_or(headline.clone(), |detail| format!("{headline}\n{detail}"));
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ChatMessageKind {
    Message,
    Reasoning,
    Activity,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ChatRole {
    User,
    Assistant,
    System,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatTabAttachment {
    tab_id: u64,
    title: String,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    preview_data_url: Option<String>,
    captured_at_ms: Option<u128>,
    text_char_count: usize,
    element_count: usize,
    link_count: usize,
    image_count: usize,
    visual_included: bool,
    estimated_token_count: usize,
    redaction_count: usize,
    truncated: bool,
    stale: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatTerminalAttachment {
    session_id: u64,
    label: String,
    kind: TerminalKind,
    profile_id: Option<String>,
    remote_target: Option<String>,
    shell: String,
    cwd: String,
    status: String,
    phase: TerminalPhase,
    busy: bool,
    output_preview: String,
    last_exit_code: Option<i32>,
    captured_at_ms: u128,
    estimated_token_count: usize,
    redaction_count: usize,
    truncated: bool,
    followed_live: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DraftTerminalContext {
    snapshot: TerminalContextSnapshot,
    follow_live: bool,
    last_live_render_ms: u128,
    live_update_scheduled: bool,
}

impl DraftTerminalContext {
    fn attached(snapshot: TerminalContextSnapshot, now_ms: u128) -> Self {
        Self {
            snapshot,
            follow_live: true,
            last_live_render_ms: now_ms,
            live_update_scheduled: false,
        }
    }

    fn view(&self, available: bool) -> TerminalContextDraftView {
        TerminalContextDraftView {
            session_id: self.snapshot.session_id,
            label: self.snapshot.label.clone(),
            kind: self.snapshot.kind,
            profile_id: self.snapshot.profile_id.clone(),
            remote_target: self.snapshot.remote_target.clone(),
            shell: self.snapshot.shell.clone(),
            cwd: self.snapshot.cwd.clone(),
            status: self.snapshot.status.clone(),
            phase: self.snapshot.phase,
            busy: self.snapshot.busy,
            output_preview: self.snapshot.output_preview(),
            source_output_char_count: self.snapshot.source_output_char_count,
            output_revision: self.snapshot.output_revision,
            last_exit_code: self.snapshot.last_exit_code,
            captured_at_ms: self.snapshot.captured_at_ms,
            estimated_token_count: self.snapshot.estimated_token_count,
            redaction_count: self.snapshot.redaction_count,
            truncated: self.snapshot.truncated,
            follow_live: self.follow_live,
            available,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalContextDraftView {
    session_id: u64,
    label: String,
    kind: TerminalKind,
    profile_id: Option<String>,
    remote_target: Option<String>,
    shell: String,
    cwd: String,
    status: String,
    phase: TerminalPhase,
    busy: bool,
    output_preview: String,
    source_output_char_count: usize,
    output_revision: u64,
    last_exit_code: Option<i32>,
    captured_at_ms: u128,
    estimated_token_count: usize,
    redaction_count: usize,
    truncated: bool,
    follow_live: bool,
    available: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PermissionPanelState {
    mode: PermissionMode,
    session_authorized: bool,
    authorized_scopes: Vec<CapabilityScope>,
    pending: Option<PendingCommandView>,
    scope: &'static str,
    resets_on_restart: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingCommandView {
    request_id: String,
    action: String,
    summary: String,
    scope: CapabilityScope,
    effect: crate::permissions::ActionEffect,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentGraphView {
    visual_graph: AgentGraphData,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentGraphData {
    nodes: Vec<AgentGraphNode>,
    edges: Vec<AgentGraphEdge>,
    truncated: bool,
    record_limit: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentGraphNode {
    key: String,
    record_type: String,
    record_id: String,
    label: String,
    detail: String,
    entity_kind: Option<String>,
    source_uri: Option<String>,
    icon_key: String,
    filesystem_path: Option<String>,
    filesystem_root: bool,
    archived: bool,
    placeholder: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentGraphEdge {
    key: String,
    source: String,
    target: String,
    label: String,
    kind: String,
    directed: bool,
}

#[derive(Clone, Debug)]
struct AgentGraphNodeContext {
    record_type: String,
    record_id: String,
    label: String,
    project_directory: Option<String>,
    ssh_profile_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ScriptBridgeResult {
    ok: bool,
    #[serde(default)]
    data: Value,
    error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct BrowserSession {
    #[serde(default)]
    browser_panel_minimized: bool,
    tabs: Vec<SessionTab>,
    active_index: usize,
    #[serde(default)]
    primary_index: Option<usize>,
    #[serde(default)]
    docked_index: Option<usize>,
    #[serde(default)]
    tab_dock_side: TabDockSide,
    #[serde(default)]
    terminal_dock: TerminalDock,
    #[serde(default = "default_agent_panel_visible")]
    agent_panel_visible: bool,
    #[serde(default)]
    agent_panel_detached: bool,
    #[serde(default)]
    agent_panel_width_logical: Option<f64>,

    #[serde(default)]
    agent_provider: AgentProviderKind,
    #[serde(default)]
    claude_selection: AgentSelection,
    #[serde(default)]
    cursor_selection: AgentSelection,
    #[serde(default)]
    github_copilot_selection: AgentSelection,
    #[serde(default)]
    google_antigravity_selection: AgentSelection,
    #[serde(default)]
    opencode_go_selection: AgentSelection,
    #[serde(default)]
    claude_session_id: Option<String>,
    #[serde(default)]
    claude_session_cwd: Option<String>,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_search_engine")]
    search_engine: String,
    #[serde(default)]
    workspace_root: Option<String>,
    #[serde(default)]
    workspace_roots: Vec<String>,
    #[serde(default)]
    archived_workspace_roots: Vec<String>,
    #[serde(default)]
    workspace_project_labels: HashMap<String, String>,
    #[serde(default)]
    pinned_workspace_roots: Vec<String>,
    #[serde(default)]
    workspace_last_opened_ms: HashMap<String, u64>,
    #[serde(default = "default_reopen_last_project")]
    reopen_last_project: bool,
    #[serde(default)]
    project_metadata: HashMap<String, ProjectMetadata>,
    #[serde(default)]
    remote_projects: Vec<RemoteProject>,
    #[serde(default)]
    active_project_chat_id: Option<String>,
    #[serde(default)]
    window_access_enabled: bool,
    #[serde(default = "default_audit_retention_days")]
    audit_retention_days: u64,
    #[serde(default, rename = "knowledgeAgentBindings")]
    agent_graph_bindings: Vec<AgentGraphBinding>,
    // Read legacy inline histories for migration, but keep the frequently written session file
    // small. New builds persist agent history separately and only when that history changes.
    #[serde(default, skip_serializing, rename = "knowledgeAgentSessions")]
    agent_graph_sessions: Vec<AgentGraphSession>,
    #[serde(default, rename = "knowledgeAgentLinks")]
    agent_graph_links: Vec<AgentGraphLink>,
    #[serde(default, rename = "knowledgeOrbPosition")]
    agent_graph_launcher_position: AgentGraphLauncherPosition,
    #[serde(default, rename = "knowledgeLogoControlV1")]
    agent_graph_launcher_v1: bool,
    #[serde(default)]
    ssh_profiles: Vec<SshProfile>,
    #[serde(default)]
    remote_desktop: RemoteDesktopPreference,
}

impl Default for BrowserSession {
    fn default() -> Self {
        Self {
            browser_panel_minimized: false,
            tabs: Vec::new(),
            active_index: 0,
            primary_index: None,
            docked_index: None,
            tab_dock_side: TabDockSide::default(),
            terminal_dock: TerminalDock::default(),
            agent_panel_visible: default_agent_panel_visible(),
            agent_panel_detached: false,
            agent_panel_width_logical: None,

            agent_provider: AgentProviderKind::default(),
            claude_selection: AgentSelection::default(),
            cursor_selection: AgentSelection::default(),
            github_copilot_selection: AgentSelection::default(),
            google_antigravity_selection: AgentSelection::default(),
            opencode_go_selection: AgentSelection::default(),
            claude_session_id: None,
            claude_session_cwd: None,
            theme: default_theme(),
            search_engine: default_search_engine(),
            workspace_root: None,
            workspace_roots: Vec::new(),
            archived_workspace_roots: Vec::new(),
            workspace_project_labels: HashMap::new(),
            pinned_workspace_roots: Vec::new(),
            workspace_last_opened_ms: HashMap::new(),
            reopen_last_project: default_reopen_last_project(),
            project_metadata: HashMap::new(),
            remote_projects: Vec::new(),
            active_project_chat_id: None,
            window_access_enabled: false,
            audit_retention_days: default_audit_retention_days(),
            agent_graph_bindings: Vec::new(),
            agent_graph_sessions: Vec::new(),
            agent_graph_links: Vec::new(),
            agent_graph_launcher_position: AgentGraphLauncherPosition::default(),
            agent_graph_launcher_v1: false,
            ssh_profiles: Vec::new(),
            remote_desktop: RemoteDesktopPreference::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct AgentGraphLauncherPosition {
    x_ratio: f64,
    y_ratio: f64,
}

impl Default for AgentGraphLauncherPosition {
    fn default() -> Self {
        Self {
            x_ratio: 0.38,
            y_ratio: 0.41,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct AgentGraphLauncherDrag {
    start_screen_x: f64,
    start_screen_y: f64,
    origin_x: i32,
    origin_y: i32,
    surface_ready: bool,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct SessionTab {
    url: Option<String>,
    #[serde(default)]
    detached: bool,
}

struct ClaudeJob {
    run_id: u64,
    cwd: String,
    handle: ClaudeRunHandle,
}

struct SubscriptionJob {
    run_id: u64,
    provider: AgentProviderKind,
    handle: SubscriptionRunHandle,
}

#[derive(Clone, Debug)]
struct PendingRunOutcome {
    phase: AgentPhase,
    status: String,
}

struct ChatSubmissionPrompt {
    message: String,
    resume: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct PendingAgentSubmission {
    #[serde(skip)]
    delivery_trace: Option<app_server::delivery::Trace>,
    #[serde(default)]
    native_skills: Vec<app_server::SelectedSkill>,
    #[serde(default)]
    native_apps: Vec<app_server::SelectedApp>,
    #[serde(default)]
    scope: Option<submission_scope::SubmissionScope>,

    #[serde(default)]
    snapshots: SubmissionSnapshots,
    provider: AgentProviderKind,
    message: String,
    tab_ids: Vec<u64>,
    terminal_session_ids: Vec<u64>,
    file_ids: Vec<String>,
    selection_override: Option<AgentSelection>,
    #[serde(default, rename = "knowledge_launch")]
    agent_graph_launch: Option<AgentGraphLaunch>,
    /// The request was composed in a project-board agent card. Its attachment
    /// draft is isolated from the same chat opened in the main Agent panel.
    #[serde(default)]
    card_draft: bool,
    /// Host-only authority for one structured Supervisor review. This is never
    /// persisted with a queued prompt or accepted from IPC.
    #[serde(skip)]
    supervision_review: Option<supervision::ReviewDispatch>,
}

#[derive(Clone, Debug)]
struct PendingClaudePermission {
    run_id: u64,
    request_id: String,
    tool_use_id: Option<String>,
    tool_name: String,
    summary: String,
    authorization: ActionAuthorization,
}

#[derive(Clone, Debug)]
struct PendingSubscriptionPermission {
    run_id: u64,
    provider: AgentProviderKind,
    request_id: String,
    tool_use_id: Option<String>,
    tool_name: String,
    summary: String,
    authorization: ActionAuthorization,
}

#[derive(Clone, Debug)]
struct TimeMachineRunPlan {
    root: PathBuf,
    provider: String,
    context: String,
}

#[derive(Clone, Debug)]
enum PendingTimeMachineAction {
    AgentCommand {
        run_id: u64,
        request: AgentCommandRequest,
    },

    ClaudePermission {
        request: PendingClaudePermission,
        approval_message: String,
    },
    SubscriptionPermission {
        request: PendingSubscriptionPermission,
        approval_message: String,
    },
}

impl PendingTimeMachineAction {
    fn run_id(&self) -> u64 {
        match self {
            Self::AgentCommand { run_id, .. } => *run_id,

            Self::ClaudePermission { request, .. } => request.run_id,
            Self::SubscriptionPermission { request, .. } => request.run_id,
        }
    }

    fn may_mutate_workspace(&self) -> bool {
        match self {
            Self::AgentCommand { request, .. } => request.command.may_mutate_workspace(),

            Self::ClaudePermission { request, .. } => {
                authorization_may_mutate_workspace(request.authorization)
            }
            Self::SubscriptionPermission { request, .. } => {
                authorization_may_mutate_workspace(request.authorization)
            }
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
struct AgentGraphBinding {
    record_type: String,
    record_id: String,
    #[serde(default)]
    conversation_id: Option<Uuid>,
    #[serde(default)]
    project_chat_id: Option<String>,
    #[serde(default)]
    project_directory: Option<String>,
    #[serde(default)]
    ssh_profile_id: Option<String>,
    name: String,
    mission: String,
    #[serde(default)]
    provider: AgentProviderKind,
    selection: AgentSelection,
}

impl AgentGraphBinding {
    fn node_key(&self) -> String {
        agent_graph_conversation_key(&self.record_type, &self.record_id, self.conversation_id)
    }

    fn matches_target(
        &self,
        record_type: &str,
        record_id: &str,
        conversation_id: Option<Uuid>,
    ) -> bool {
        self.record_type == record_type
            && self.record_id == record_id
            && self.conversation_id == conversation_id
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct AgentGraphLaunch {
    node_key: String,
    agent_name: String,
    project_directory: String,
    ssh_profile_id: Option<String>,
    user_request: String,
}

#[derive(Clone, Debug)]
struct AgentGraphAssignment {
    record_type: String,
    record_id: String,
    conversation_id: Option<Uuid>,
    name: String,
    mission: String,
    provider: Option<AgentProviderKind>,
    model: Option<String>,
    effort: Option<String>,
    service_tier: Option<String>,
    context_window: Option<u64>,
}

struct AgentGraphRunPresentation {
    node_key: String,
    run_id: u64,
    agent_name: String,
    project_directory: String,
    provider: AgentProviderKind,
    selection: AgentSelection,
    runtime: AgentRun,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentGraphHistoryMessage {
    id: u64,
    role: ChatRole,
    kind: ChatMessageKind,
    text: String,
    timestamp_ms: u128,
    #[serde(default)]
    artifacts: Vec<ChatArtifact>,
    #[serde(default)]
    native: Option<ChatMessage>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RenderedAgentGraphHistoryMessage<'a> {
    id: u64,
    role: ChatRole,
    kind: ChatMessageKind,
    text: &'a str,
    timestamp_ms: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    rendered_html: Option<String>,
    artifacts: Vec<ChatArtifactView<'a>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentGraphHistoryStep {
    label: String,
    kind: AgentStepKind,
    status: AgentStepStatus,
    detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentGraphTurn {
    run_id: u64,
    request: String,
    #[serde(default)]
    provider: AgentProviderKind,
    #[serde(default)]
    selection: AgentSelection,
    started_at_ms: u128,
    finished_at_ms: Option<u128>,
    phase: AgentPhase,
    status: String,
    messages: Vec<AgentGraphHistoryMessage>,
    steps: Vec<AgentGraphHistoryStep>,
    #[serde(default)]
    checkpoint: Option<CheckpointSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentGraphTurnView<'a> {
    run_id: u64,
    request: &'a str,
    provider: AgentProviderKind,
    selection: &'a AgentSelection,
    started_at_ms: u128,
    finished_at_ms: Option<u128>,
    phase: AgentPhase,
    status: &'a str,
    messages: Vec<Value>,
    steps: &'a [AgentGraphHistoryStep],
    #[serde(skip_serializing_if = "Option::is_none")]
    checkpoint: Option<&'a CheckpointSummary>,
}

impl<'a> AgentGraphTurnView<'a> {
    fn new(turn: &'a AgentGraphTurn, artifact_store: &'a ArtifactStore) -> Self {
        Self {
            run_id: turn.run_id,
            request: &turn.request,
            provider: turn.provider,
            selection: &turn.selection,
            started_at_ms: turn.started_at_ms,
            finished_at_ms: turn.finished_at_ms,
            phase: turn.phase,
            status: &turn.status,
            messages: turn
                .messages
                .iter()
                .map(|message| {
                    if let Some(native) = &message.native {
                        return serde_json::to_value(RenderedChatMessage::new(
                            native,
                            artifact_store,
                        ))
                        .unwrap_or(Value::Null);
                    }
                    serde_json::to_value(RenderedAgentGraphHistoryMessage {
                        id: message.id,
                        role: message.role,
                        kind: message.kind,
                        text: &message.text,
                        timestamp_ms: message.timestamp_ms,
                        rendered_html: rendered_message_html(
                            message.role,
                            message.kind,
                            &message.text,
                        ),
                        artifacts: message
                            .artifacts
                            .iter()
                            .map(|artifact| artifact_store.view(artifact))
                            .collect(),
                    })
                    .unwrap_or(Value::Null)
                })
                .collect(),
            steps: &turn.steps,
            checkpoint: turn.checkpoint.as_ref(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentGraphSession {
    node_key: String,
    record_type: String,
    record_id: String,
    agent_name: String,
    project_directory: String,
    #[serde(default)]
    provider: AgentProviderKind,
    selection: AgentSelection,
    turns: Vec<AgentGraphTurn>,
    updated_at_ms: u128,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentGraphLink {
    source_node_key: String,
    target_node_key: String,
}

#[derive(Clone, Debug)]
struct PendingSshRuntimeCommand {
    request_id: String,
    action: String,
    profile_id: String,
    profile_name: String,
    command: String,
}

struct AgentRunTiming {
    accepted_at_ms: u128,
    first_provider_event_at_ms: Option<u128>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct LiveDiffView {
    additions: usize,
    deletions: usize,
    file_count: usize,
    active: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrackedLiveDiff {
    run_id: u64,
    view: LiveDiffView,
}

impl From<LiveDiffStats> for TrackedLiveDiff {
    fn from(stats: LiveDiffStats) -> Self {
        Self {
            run_id: stats.run_id,
            view: LiveDiffView {
                additions: stats.additions,
                deletions: stats.deletions,
                file_count: stats.file_count,
                active: stats.active,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DevelopmentUiReloadTargets {
    backdrop: bool,
    toolbar: bool,
    agent_panel: bool,
    agent_graph_surface: bool,
    terminal_panel: bool,
    preview_panel: bool,
    start_pages: bool,
    remote_desktop: bool,
}

fn development_ui_reload_targets(changed: &[String]) -> DevelopmentUiReloadTargets {
    let contains = |path: &str| changed.iter().any(|changed| changed == path);
    let themes =
        contains("assets/themes.css") || contains("assets/fonts/inter/Inter-Variable.woff2");
    let svelte_bundle =
        contains("ui/dist/central-agent-ui.css") || contains("ui/dist/central-agent-ui.js");
    let file_icons = contains("assets/file-icons.js");
    let brand_mark = contains("assets/supervisor-mark.svg");

    DevelopmentUiReloadTargets {
        backdrop: themes || contains("assets/backdrop.html"),
        toolbar: themes || svelte_bundle || brand_mark || contains("assets/toolbar.html"),
        agent_panel: themes
            || svelte_bundle
            || file_icons
            || brand_mark
            || contains("assets/agent-panel.html"),
        agent_graph_surface: themes
            || svelte_bundle
            || file_icons
            || brand_mark
            || contains("assets/agent-graph.html"),
        terminal_panel: themes
            || svelte_bundle
            || file_icons
            || brand_mark
            || contains("assets/agent-panel.html"),
        preview_panel: themes || contains("assets/preview.html"),
        start_pages: themes
            || brand_mark
            || contains("assets/supervisor-wordmark.svg")
            || contains("assets/start-page.html"),
        remote_desktop: themes || contains("assets/remote-desktop.html"),
    }
}

pub(crate) struct BrowserApp {
    startup_error: Option<anyhow::Error>,
    file_editor: crate::file_editor::FileEditor,
    browser_panel_minimized: bool,
    proxy: EventLoopProxy<BrowserEvent>,
    #[cfg(windows)]
    system_tray: Option<crate::system_tray::SystemTray>,
    background_mode_enabled: bool,
    tray_quit_deadline: Option<Instant>,
    window: Option<Window>,
    backdrop: Option<WebView>,
    toolbar: Option<WebView>,
    agent_panel: Option<WebView>,
    agent_panel_window: Option<Window>,
    agent_graph_surface: Option<WebView>,
    terminal_panel: Option<WebView>,
    terminal_window: Option<Window>,
    preview_panel: Option<WebView>,
    preview_window: Option<Window>,
    remote_desktop_tab_id: Option<u64>,
    remote_desktop_ready: bool,
    ui_context: Option<WebContext>,
    content_context: Option<WebContext>,
    tabs: Vec<BrowserTab>,
    active_tab_id: Option<u64>,
    primary_tab_id: Option<u64>,
    docked_tab_id: Option<u64>,
    pending_tab_activation: Option<u64>,
    tab_dock_side: TabDockSide,
    next_tab_id: u64,
    toolbar_ready: bool,
    agent_panel_ready: bool,
    agent_graph_surface_ready: bool,
    agent_panel_visible: bool,
    agent_panel_width_logical: Option<f64>,
    agent_panel_close_pending: bool,
    settings_open: bool,
    terminal_panel_ready: bool,
    preview_panel_ready: bool,
    terminal_panel_visible: bool,
    terminal_dock: TerminalDock,
    action_log: VecDeque<ActionLogEntry>,
    audit: AuditRuntime,
    last_result: Option<CommandResultView>,
    permission_policy: PermissionPolicy,
    pending_commands: HashMap<String, AgentCommandRequest>,
    chat_messages: VecDeque<ChatMessage>,
    chat_ownership: chat_ownership::ChatOwnership,
    draft_contexts: Vec<TabContextSnapshot>,
    draft_terminal_contexts: Vec<DraftTerminalContext>,
    draft_file_attachments: Vec<PendingFileAttachment>,
    graph_files: graph_files::State,
    graph_contexts: graph_contexts::State,
    main_draft_owner: String,
    main_drafts: HashMap<String, main_drafts::MainDraft>,
    main_runs: main_runs::MainRuns,
    native_session_id: String,
    local_agent_terminals: HashMap<String, u64>,

    project_diff: project_diff::State,
    app_server: app_server::State,
    computer_use_frame: ComputerUseFrame,
    #[cfg(windows)]
    browser_use_bridge: Option<browser_use_bridge::Handle>,
    #[cfg(windows)]
    browser_use_owned_tabs: HashMap<u64, browser_use_backend::OwnedTab>,
    #[cfg(windows)]
    browser_use_activity: HashMap<u64, browser_use_backend::TabActivity>,

    agent_provider: AgentProviderKind,
    claude_provider: AgentProviderView,
    claude_models: Vec<AgentModelOption>,
    claude_catalog_refresh: ModelCatalogRefresh,
    claude_login_in_progress: bool,
    claude_selection: AgentSelection,
    claude_session_id: Option<String>,
    claude_session_cwd: Option<String>,
    /// Latest subscription usage window reported by Claude Code; transient.
    claude_usage_limit: Option<ClaudeUsageLimit>,
    /// Notice currently appended to the Claude provider detail.
    claude_usage_notice: Option<String>,
    claude_jobs: HashMap<u64, ClaudeJob>,
    pending_claude_permissions: HashMap<String, PendingClaudePermission>,
    claude_tool_steps: HashMap<String, usize>,
    claude_tool_run_ids: HashMap<String, u64>,
    claude_tool_request_ids: HashMap<String, String>,
    cursor_provider: AgentProviderView,
    cursor_models: Vec<AgentModelOption>,
    cursor_selection: AgentSelection,
    github_copilot_provider: AgentProviderView,
    github_copilot_models: Vec<AgentModelOption>,
    github_copilot_selection: AgentSelection,
    google_antigravity_provider: AgentProviderView,
    google_antigravity_models: Vec<AgentModelOption>,
    google_antigravity_selection: AgentSelection,
    opencode_go_provider: AgentProviderView,
    opencode_go_models: Vec<AgentModelOption>,
    opencode_go_selection: AgentSelection,
    subscription_jobs: HashMap<u64, SubscriptionJob>,
    pending_subscription_permissions: HashMap<String, PendingSubscriptionPermission>,
    subscription_tool_steps: HashMap<String, usize>,
    subscription_tool_run_ids: HashMap<String, u64>,
    subscription_tool_request_ids: HashMap<String, String>,
    terminal: TerminalRuntime,
    ssh: SshRuntime,
    remote_desktop: RemoteDesktopRuntime,
    pending_ssh_runtime_commands: HashMap<u64, VecDeque<PendingSshRuntimeCommand>>,
    processes: ManagedProcessRuntime,
    windows: WindowRuntime,
    ui_automation: UiAutomationRuntime,
    safety: SafetyRuntime,
    window_access_enabled: bool,
    preview: PreviewRuntime,
    provider_tool_images: HashMap<String, String>,
    artifact_store: ArtifactStore,
    pending_artifacts: HashMap<u64, Vec<ChatArtifact>>,
    workspace: WorkspaceRuntime,
    workspace_project_labels: HashMap<String, String>,
    pinned_workspace_roots: HashSet<String>,
    workspace_last_opened_ms: HashMap<String, u64>,
    reopen_last_project: bool,
    project_metadata: HashMap<String, ProjectMetadata>,
    remote_projects: Vec<RemoteProject>,
    project_scans_in_flight: HashSet<String>,
    project_import: ProjectImportView,
    project_import_inspection_key: Option<String>,
    project_import_operation_id: Option<String>,
    project_drop_active: bool,
    project_registry_add_request_id: u64,
    project_chats: Vec<ProjectChat>,
    active_project_chat_id: Option<String>,
    project_board_open_chat_ids: Vec<String>,
    project_chat_card_profiles: HashMap<String, ProjectChatCardProfile>,
    run_execution_contexts: HashMap<u64, run_context::RunExecutionContext>,
    time_machine: Arc<Mutex<TimeMachineRuntime>>,
    time_machine_start_gates: RunStartGateRegistry,
    time_machine_run_plans: HashMap<u64, TimeMachineRunPlan>,
    time_machine_run_roots: HashMap<u64, PathBuf>,
    live_diff_by_owner: HashMap<String, TrackedLiveDiff>,
    time_machine_live_diff_scans_in_flight: HashSet<u64>,
    pending_time_machine_actions: HashMap<u64, VecDeque<PendingTimeMachineAction>>,
    pending_run_finalizations: HashSet<u64>,
    pending_run_outcomes: HashMap<u64, PendingRunOutcome>,
    retryable_time_machine_finalizations: HashSet<u64>,
    time_machine_finalizations_in_flight: HashSet<u64>,
    pending_workspace_ejections: HashSet<String>,
    pending_agent_submissions: VecDeque<PendingAgentSubmission>,
    agent_submission_queue: VecDeque<PendingAgentSubmission>,
    agent_graph_bindings: Vec<AgentGraphBinding>,
    agent_graph_sessions: Vec<AgentGraphSession>,
    agent_graph_links: Vec<AgentGraphLink>,
    agent_graph_runs: HashMap<String, AgentGraphRunPresentation>,
    supervision_loops: HashMap<String, supervision::LoopState>,
    active_supervisor_reviews: HashMap<String, supervision::ReviewDispatch>,
    agent_graph_launcher_position: AgentGraphLauncherPosition,
    agent_graph_launcher_drag: Option<AgentGraphLauncherDrag>,
    agent_graph_open: bool,
    theme: String,
    search_engine: String,
    next_chat_message_id: u64,
    next_agent_run_id: u64,

    next_context_capture_id: u64,
    data_dir: PathBuf,
    pending_session: Option<BrowserSession>,

    run_timings: HashMap<u64, AgentRunTiming>,
    ui_development_root: Option<PathBuf>,
    ui_development_watcher_started: bool,
    agent_graph_history_offsets: HashMap<String, usize>,
}

impl BrowserApp {
    pub(crate) fn new(proxy: EventLoopProxy<BrowserEvent>) -> anyhow::Result<Self> {
        let data_dir = if let Some(path) =
            std::env::var_os("CENTRAL_AGENT_DATA_DIR").filter(|path| !path.is_empty())
        {
            PathBuf::from(path)
        } else {
            ProjectDirs::from("dev", "CentralAgent", "CentralAgent")
                .ok_or_else(|| anyhow!("user data directory is unavailable"))?
                .data_local_dir()
                .to_path_buf()
        };
        Self::with_data_dir(proxy, data_dir, true)
    }

    // Acceptance owns a disposable store and does not start OS control hooks.
    fn with_data_dir(
        proxy: EventLoopProxy<BrowserEvent>,
        data_dir: PathBuf,
        start_safety: bool,
    ) -> anyhow::Result<Self> {
        if start_safety {
            let safety_proxy = proxy.clone();
            safety_runtime::start(move |event| {
                let _ = safety_proxy.send_event(BrowserEvent::Safety(event));
            });
        }
        #[cfg(windows)]
        let browser_use_bridge = if start_safety {
            match browser_use_bridge::start(proxy.clone()) {
                Ok(bridge) => Some(bridge),
                Err(error) => {
                    warn!(%error, "official Browser Use could not attach to Supervisor's browser");
                    None
                }
            }
        } else {
            None
        };
        let ui_development_root = if start_safety {
            ui_development::configured_root()?
        } else {
            None
        };
        fs::create_dir_all(&data_dir).with_context(|| {
            format!("failed to create the data directory {}", data_dir.display())
        })?;

        let pending_session = load_session(&data_dir.join("session.json"));
        let mut project_chats =
            load_project_chat_history(&data_dir.join(PROJECT_CHAT_HISTORY_FILE)).chats;
        let project_artifact_duplicates_removed =
            normalize_loaded_project_chats(&mut project_chats);
        let workspace_project_labels = pending_session
            .workspace_project_labels
            .iter()
            .filter_map(|(root, label)| {
                normalized_user_title(label, MAX_PROJECT_LABEL_CHARS)
                    .map(|label| (root.clone(), label))
            })
            .collect::<HashMap<_, _>>();
        let pinned_workspace_roots = pending_session
            .pinned_workspace_roots
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let workspace_last_opened_ms = pending_session.workspace_last_opened_ms.clone();
        // Reopening is the fixed default now that the project registry no longer exposes a toggle.
        // Ignore a retired opt-out so hidden legacy state cannot leave the app deactivated.
        let reopen_last_project = default_reopen_last_project();
        let project_metadata = pending_session.project_metadata.clone();

        let agent_provider = pending_session.agent_provider;
        let claude_selection = pending_session.claude_selection.clone();
        let cursor_selection = pending_session.cursor_selection.clone();
        let github_copilot_selection = pending_session.github_copilot_selection.clone();
        let google_antigravity_selection = pending_session.google_antigravity_selection.clone();
        let opencode_go_selection = pending_session.opencode_go_selection.clone();
        let legacy_claude_session_id = pending_session.claude_session_id.clone();
        let legacy_claude_session_cwd = pending_session.claude_session_cwd.clone();
        let theme = normalize_theme(&pending_session.theme).to_owned();
        let search_engine = search_engine(&pending_session.search_engine).id.to_owned();
        let mut ssh = SshRuntime::new(pending_session.ssh_profiles.clone());
        let mut remote_desktop = RemoteDesktopRuntime::new(pending_session.remote_desktop.clone());
        let configured_remote_desktop_profile =
            remote_desktop.configured_profile_id().map(str::to_owned);
        if let Some(profile_id) = configured_remote_desktop_profile
            && let Some(profile) = ssh.profile(&profile_id)
        {
            remote_desktop.configure(
                profile.id.clone(),
                profile.name.clone(),
                profile.username.clone(),
                format!("{}@{}:{}", profile.username, profile.host, profile.port),
                pending_session.remote_desktop.protocol,
                pending_session.remote_desktop.remote_port.max(1),
            );
        }
        let mut automatic_ssh_config_paths = vec![data_dir.join(AUTOMATIC_SSH_CONFIG_FILE)];
        // User-specific SSH metadata belongs in the per-user data directory. Reading a config next
        // to the executable is kept only as an explicit development escape hatch so a distributable
        // folder cannot accidentally become coupled to one developer's host and key path.
        if std::env::var("CENTRAL_AGENT_ALLOW_ADJACENT_SSH_CONFIG").as_deref() == Ok("1")
            && let Ok(executable) = std::env::current_exe()
            && let Some(directory) = executable.parent()
        {
            let adjacent = directory.join(AUTOMATIC_SSH_CONFIG_FILE);
            if !automatic_ssh_config_paths.contains(&adjacent) {
                automatic_ssh_config_paths.push(adjacent);
            }
        }
        for config_path in automatic_ssh_config_paths {
            match load_automatic_ssh_profiles(&config_path) {
                Ok(profiles) => {
                    for profile in profiles {
                        match ssh.upsert(profile.input) {
                            Ok(_) => {}
                            Err(message) => {
                                warn!(%message, path = %config_path.display(), "automatic SSH profile was rejected")
                            }
                        }
                    }
                }
                Err(message) => {
                    warn!(%message, path = %config_path.display(), "automatic SSH configuration was ignored")
                }
            }
        }
        let mut remote_projects = Vec::new();
        for mut project in pending_session.remote_projects.clone() {
            let Ok(directory) = normalize_remote_directory(&project.directory) else {
                continue;
            };
            if ssh.profile(&project.ssh_profile_id).is_none()
                || remote_projects.iter().any(|existing: &RemoteProject| {
                    existing.ssh_profile_id == project.ssh_profile_id
                        && existing.directory == directory
                })
            {
                continue;
            }
            project.directory = directory;
            if project.name.trim().is_empty() {
                project.name = remote_project_name(&project.directory);
            }
            remote_projects.push(project);
        }
        let agent_graph_launcher_position =
            normalize_agent_graph_launcher_position(pending_session.agent_graph_launcher_position);
        let mut agent_graph_bindings = Vec::new();
        for binding in pending_session
            .agent_graph_bindings
            .iter()
            .cloned()
            .filter_map(normalize_loaded_agent_graph_binding)
        {
            let node_key = binding.node_key();
            if !agent_graph_bindings
                .iter()
                .any(|existing: &AgentGraphBinding| existing.node_key() == node_key)
            {
                agent_graph_bindings.push(binding);
            }
            if agent_graph_bindings.len() >= MAX_AGENT_GRAPH_BINDINGS {
                break;
            }
        }
        let history_path = data_dir.join(AGENT_GRAPH_HISTORY_FILE);
        let mut agent_graph_sessions = load_agent_graph_sessions(&history_path)
            .unwrap_or_else(|| pending_session.agent_graph_sessions.clone());
        let restored_at_ms = unix_time_ms();
        for session in &mut agent_graph_sessions {
            for turn in &mut session.turns {
                let checkpoint = turn.checkpoint.clone();
                for message in &mut turn.messages {
                    remove_checkpoint_artifact_duplicates(
                        &mut message.artifacts,
                        checkpoint.as_ref(),
                    );
                }
                if turn.selection.model.is_empty() || turn.selection.effort.is_empty() {
                    turn.selection = session.selection.clone();
                    turn.provider = session.provider;
                }
                if turn.finished_at_ms.is_none() {
                    turn.finished_at_ms = Some(restored_at_ms);
                    turn.phase = AgentPhase::Stopped;
                    turn.status =
                        "The previous app session ended before this run completed".to_owned();
                }
            }
        }
        if !agent_graph_sessions.is_empty() {
            match serde_json::to_vec_pretty(&agent_graph_sessions) {
                Ok(json) => {
                    if let Err(error) =
                        write_file_atomically::<Vec<AgentGraphSession>>(&history_path, &json)
                    {
                        warn!(%error, path = %history_path.display(), "normalized graph-agent history was not saved");
                    }
                }
                Err(error) => {
                    warn!(%error, "normalized graph-agent history was not serialized")
                }
            }
        }
        let assigned_node_keys = agent_graph_bindings
            .iter()
            .map(AgentGraphBinding::node_key)
            .collect::<HashSet<_>>();
        let mut agent_graph_links = Vec::new();
        for link in pending_session.agent_graph_links.iter().cloned() {
            if link.source_node_key == link.target_node_key
                || !assigned_node_keys.contains(&link.source_node_key)
                || !assigned_node_keys.contains(&link.target_node_key)
            {
                continue;
            }
            let duplicate = agent_graph_links.iter().any(|existing: &AgentGraphLink| {
                (existing.source_node_key == link.source_node_key
                    && existing.target_node_key == link.target_node_key)
                    || (existing.source_node_key == link.target_node_key
                        && existing.target_node_key == link.source_node_key)
            });
            if !duplicate {
                agent_graph_links.push(link);
            }
        }
        let mut time_machine_runtime = TimeMachineRuntime::open(data_dir.join("time-machine"));
        let (project_checkpoint_metadata_changed, graph_checkpoint_metadata_changed) =
            refresh_persisted_checkpoint_metadata(
                &mut project_chats,
                &mut agent_graph_sessions,
                &time_machine_runtime,
            );
        if project_checkpoint_metadata_changed || project_artifact_duplicates_removed {
            let path = data_dir.join(PROJECT_CHAT_HISTORY_FILE);
            match serde_json::to_vec_pretty(&ProjectChatHistory {
                chats: project_chats.clone(),
            }) {
                Ok(json) => {
                    if let Err(error) = write_file_atomically::<ProjectChatHistory>(&path, &json) {
                        warn!(%error, path = %path.display(), "normalized checkpoint chat metadata was not saved");
                    }
                }
                Err(error) => {
                    warn!(%error, "normalized checkpoint chat metadata was not serialized")
                }
            }
        }
        let time_machine_next_run_id = time_machine_runtime.next_run_id_hint();
        let recovered_checkpoint_summaries = time_machine_runtime.take_recovered_summaries();
        let (time_machine_run_roots, retryable_time_machine_finalizations, recovery_next_run_id) =
            index_unfinished_time_machine_runs(time_machine_runtime.unfinished_runs());
        let project_chat_message_id = project_chats
            .iter()
            .flat_map(|chat| chat.messages.iter().map(|message| message.id))
            .max()
            .map_or(1, |id| id.saturating_add(1));
        let mut next_chat_message_id =
            next_agent_graph_history_message_id(&agent_graph_sessions).max(project_chat_message_id);
        let (recovered_chat_messages, recovered_graph_history_changed) =
            attach_recovered_checkpoint_summaries(
                &mut agent_graph_sessions,
                recovered_checkpoint_summaries,
                &mut next_chat_message_id,
            );
        if recovered_graph_history_changed || graph_checkpoint_metadata_changed {
            match serde_json::to_vec_pretty(&agent_graph_sessions) {
                Ok(json) => {
                    if let Err(error) =
                        write_file_atomically::<Vec<AgentGraphSession>>(&history_path, &json)
                    {
                        warn!(%error, path = %history_path.display(), "recovered graph checkpoint history was not saved");
                    }
                }
                Err(error) => {
                    warn!(%error, "recovered graph checkpoint history was not serialized")
                }
            }
        }
        let next_agent_run_id = agent_graph_sessions
            .iter()
            .flat_map(|session| session.turns.iter().map(|turn| turn.run_id))
            .max()
            .map_or(
                time_machine_next_run_id.max(recovery_next_run_id),
                |run_id| {
                    time_machine_next_run_id
                        .max(recovery_next_run_id)
                        .max(run_id.saturating_add(1))
                },
            );
        let next_agent_run_id = next_agent_run_id.max(
            project_chats
                .iter()
                .flat_map(|chat| chat.messages.iter().filter_map(|message| message.run_id))
                .max()
                .map_or(1, |id| id.saturating_add(1)),
        );
        let time_machine = Arc::new(Mutex::new(time_machine_runtime));

        let artifact_store =
            ArtifactStore::open(data_dir.join("artifacts")).map_err(anyhow::Error::msg)?;
        let audit = AuditRuntime::open(&data_dir, pending_session.audit_retention_days);
        let mut workspace = WorkspaceRuntime::restore_projects(
            pending_session
                .workspace_roots
                .iter()
                .map(PathBuf::from)
                .collect(),
            pending_session
                .archived_workspace_roots
                .iter()
                .map(PathBuf::from)
                .collect(),
            pending_session.workspace_root.as_deref().map(PathBuf::from),
        );
        if !reopen_last_project {
            workspace.deactivate();
        }
        let active_project_root = workspace.root().map(|root| root.display().to_string());
        let mut active_project_chat_id = pending_session
            .active_project_chat_id
            .as_ref()
            .filter(|chat_id| {
                project_chats.iter().any(|chat| {
                    chat.id.as_str() == chat_id.as_str()
                        && !chat.archived
                        && active_project_root.as_deref().unwrap_or_default() == chat.project_root
                })
            })
            .cloned()
            .or_else(|| {
                active_project_root.as_deref().and_then(|root| {
                    project_chats
                        .iter()
                        .filter(|chat| chat.project_root == root && !chat.archived)
                        .max_by_key(|chat| chat.updated_at_ms)
                        .map(|chat| chat.id.clone())
                })
            });
        let mut chat_messages = active_project_chat_id
            .as_ref()
            .and_then(|chat_id| project_chats.iter().find(|chat| chat.id == *chat_id))
            .map(|chat| chat.messages.iter().cloned().collect::<VecDeque<_>>())
            .unwrap_or_default();
        chat_messages.extend(recovered_chat_messages);
        if !chat_messages.is_empty()
            && active_project_chat_id.is_none()
            && let Some(project_root) = active_project_root.clone()
        {
            let now_ms = unix_time_ms();
            let chat = ProjectChat {
                id: Uuid::new_v4().to_string(),
                project_root,
                title: project_chat_title(&chat_messages),
                title_custom: false,
                pinned: false,
                archived: false,
                created_at_ms: now_ms,
                updated_at_ms: now_ms,
                messages: chat_messages.iter().cloned().collect(),
                claude_session_id: legacy_claude_session_id.clone(),
                claude_session_cwd: legacy_claude_session_cwd.clone(),
            };
            let chat_id = chat.id.clone();
            project_chats.push(chat);
            // Recovery messages now belong to a normal project chat and remain selectable.
            active_project_chat_id = Some(chat_id);
        }
        let chat_ownership = chat_ownership::ChatOwnership::restored(
            active_project_chat_id.as_deref(),
            &chat_messages,
        );
        let (claude_session_id, claude_session_cwd) = active_project_chat_id
            .as_ref()
            .and_then(|chat_id| project_chats.iter().find(|chat| chat.id == *chat_id))
            .map(|chat| {
                (
                    chat.claude_session_id.clone(),
                    chat.claude_session_cwd.clone(),
                )
            })
            .unwrap_or((legacy_claude_session_id, legacy_claude_session_cwd));
        let terminal = TerminalRuntime::default();

        Ok(Self {
            startup_error: None,
            file_editor: crate::file_editor::FileEditor::load(data_dir.join("editor-drafts.json")),
            proxy,
            #[cfg(windows)]
            system_tray: None,
            background_mode_enabled: start_safety && cfg!(windows),
            tray_quit_deadline: None,
            window: None,
            backdrop: None,
            toolbar: None,
            agent_panel: None,
            agent_panel_window: None,
            agent_graph_surface: None,
            terminal_panel: None,
            terminal_window: None,
            preview_panel: None,
            preview_window: None,
            remote_desktop_tab_id: None,
            remote_desktop_ready: false,
            ui_context: None,
            content_context: None,
            tabs: Vec::new(),
            active_tab_id: None,
            primary_tab_id: None,
            docked_tab_id: None,
            pending_tab_activation: None,
            tab_dock_side: TabDockSide::default(),
            next_tab_id: 1,
            toolbar_ready: false,
            agent_panel_ready: false,
            agent_graph_surface_ready: false,
            agent_panel_visible: true,
            browser_panel_minimized: false,
            agent_panel_width_logical: normalize_agent_panel_width_logical(
                pending_session.agent_panel_width_logical,
            ),
            agent_panel_close_pending: false,
            settings_open: false,
            terminal_panel_ready: false,
            preview_panel_ready: false,
            terminal_panel_visible: false,
            terminal_dock: pending_session.terminal_dock,
            action_log: VecDeque::new(),
            audit,
            last_result: None,
            permission_policy: PermissionPolicy::default(),
            pending_commands: HashMap::new(),
            chat_messages,
            chat_ownership,
            draft_contexts: Vec::new(),
            draft_terminal_contexts: Vec::new(),
            draft_file_attachments: Vec::new(),
            graph_files: graph_files::State::default(),
            graph_contexts: graph_contexts::State::default(),
            main_draft_owner: active_project_chat_id.as_ref().map_or_else(
                || {
                    format!(
                        "draft:{}",
                        workspace
                            .root()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default()
                    )
                },
                |id| format!("chat:{id}"),
            ),
            main_drafts: HashMap::new(),
            main_runs: main_runs::MainRuns::default(),
            native_session_id: Uuid::new_v4().to_string(),
            local_agent_terminals: HashMap::new(),

            project_diff: project_diff::State::default(),
            app_server: {
                let mut state = app_server::State::load(&data_dir);
                if !start_safety {
                    state.acceptance_home = std::env::var_os("CODEX_HOME")
                        .filter(|value| !value.is_empty())
                        .map(PathBuf::from);
                }
                state
            },
            computer_use_frame: ComputerUseFrame::default(),
            #[cfg(windows)]
            browser_use_bridge,
            #[cfg(windows)]
            browser_use_owned_tabs: HashMap::new(),
            #[cfg(windows)]
            browser_use_activity: HashMap::new(),

            agent_provider,
            claude_provider: claude_provider::checking_view(),
            claude_models: Vec::new(),
            claude_catalog_refresh: ModelCatalogRefresh::default(),
            claude_login_in_progress: false,
            claude_selection,
            claude_session_id,
            claude_session_cwd,
            claude_usage_limit: None,
            claude_usage_notice: None,
            claude_jobs: HashMap::new(),
            pending_claude_permissions: HashMap::new(),
            claude_tool_steps: HashMap::new(),
            claude_tool_run_ids: HashMap::new(),
            claude_tool_request_ids: HashMap::new(),
            cursor_provider: subscription_provider::checking_view(AgentProviderKind::Cursor),
            cursor_models: Vec::new(),
            cursor_selection,
            github_copilot_provider: subscription_provider::checking_view(
                AgentProviderKind::GithubCopilot,
            ),
            github_copilot_models: Vec::new(),
            github_copilot_selection,
            google_antigravity_provider: subscription_provider::checking_view(
                AgentProviderKind::GoogleAntigravity,
            ),
            google_antigravity_models: Vec::new(),
            google_antigravity_selection,
            opencode_go_provider: subscription_provider::checking_view(
                AgentProviderKind::OpencodeGo,
            ),
            opencode_go_models: Vec::new(),
            opencode_go_selection,
            subscription_jobs: HashMap::new(),
            pending_subscription_permissions: HashMap::new(),
            subscription_tool_steps: HashMap::new(),
            subscription_tool_run_ids: HashMap::new(),
            subscription_tool_request_ids: HashMap::new(),
            terminal,
            ssh,
            remote_desktop,
            pending_ssh_runtime_commands: HashMap::new(),
            processes: ManagedProcessRuntime::default(),
            windows: WindowRuntime::default(),
            ui_automation: UiAutomationRuntime::default(),
            safety: SafetyRuntime::default(),
            window_access_enabled: pending_session.window_access_enabled,
            preview: PreviewRuntime::default(),
            provider_tool_images: HashMap::new(),
            artifact_store,
            pending_artifacts: HashMap::new(),
            workspace,
            workspace_project_labels,
            pinned_workspace_roots,
            workspace_last_opened_ms,
            reopen_last_project,
            project_metadata,
            remote_projects,
            project_scans_in_flight: HashSet::new(),
            project_import: ProjectImportView::default(),
            project_import_inspection_key: None,
            project_import_operation_id: None,
            project_drop_active: false,
            project_registry_add_request_id: 0,
            project_chats,
            active_project_chat_id,
            project_board_open_chat_ids: Vec::new(),
            project_chat_card_profiles: HashMap::new(),
            run_execution_contexts: HashMap::new(),
            time_machine,
            time_machine_start_gates: RunStartGateRegistry::default(),
            time_machine_run_plans: HashMap::new(),
            time_machine_run_roots,
            live_diff_by_owner: HashMap::new(),
            time_machine_live_diff_scans_in_flight: HashSet::new(),
            pending_time_machine_actions: HashMap::new(),
            pending_run_finalizations: HashSet::new(),
            pending_run_outcomes: HashMap::new(),
            retryable_time_machine_finalizations,
            time_machine_finalizations_in_flight: HashSet::new(),
            pending_workspace_ejections: HashSet::new(),
            pending_agent_submissions: VecDeque::new(),
            agent_submission_queue: VecDeque::new(),
            agent_graph_bindings,
            agent_graph_sessions,
            agent_graph_history_offsets: HashMap::new(),
            agent_graph_links,
            agent_graph_runs: HashMap::new(),
            supervision_loops: HashMap::new(),
            active_supervisor_reviews: HashMap::new(),
            agent_graph_launcher_position,
            agent_graph_launcher_drag: None,
            agent_graph_open: false,
            theme,
            search_engine,
            next_chat_message_id,
            next_agent_run_id,

            next_context_capture_id: 1,
            data_dir,
            pending_session: Some(pending_session),

            run_timings: HashMap::new(),
            ui_development_root,
            ui_development_watcher_started: false,
        })
    }

    fn initialize(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        if self.window.is_some() {
            return Ok(());
        }

        let primary_monitor = event_loop.primary_monitor();
        let start_maximized = primary_monitor.as_ref().is_some_and(should_start_maximized);
        let window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Supervisor")
                    .with_window_icon(Some(brand::window_icon()))
                    .with_inner_size(LogicalSize::new(
                        INITIAL_WINDOW_WIDTH_LOGICAL,
                        INITIAL_WINDOW_HEIGHT_LOGICAL,
                    ))
                    .with_min_inner_size(LogicalSize::new(1_024.0, 640.0))
                    .with_resizable(true)
                    .with_maximized(start_maximized)
                    .with_visible(false),
            )
            .context("failed to create the main window")?;
        if !start_maximized && let Some(monitor) = primary_monitor.as_ref() {
            center_window_on_monitor(&window, monitor);
        }
        apply_native_window_theme(&window, &self.theme);

        self.window = Some(window);
        #[cfg(windows)]
        if self.background_mode_enabled {
            match crate::system_tray::SystemTray::new(self.proxy.clone()) {
                Ok(tray) => self.system_tray = Some(tray),
                Err(error) => {
                    self.background_mode_enabled = false;
                    warn!(%error, "system tray is unavailable; window close will exit Supervisor");
                }
            }
        }
        self.ui_context = Some(WebContext::new(Some(self.data_dir.join("ui-profile"))));
        self.content_context = Some(WebContext::new(Some(self.data_dir.join("browser-profile"))));

        let session = self.pending_session.take().unwrap_or_default();
        self.browser_panel_minimized =
            session.browser_panel_minimized && !session.agent_panel_detached;
        self.agent_panel_visible = session.agent_panel_visible || self.browser_panel_minimized;
        self.tab_dock_side = session.tab_dock_side;
        self.terminal_dock = session.terminal_dock;
        if !session.agent_graph_launcher_v1
            && let Some(window) = self.window.as_ref()
        {
            self.agent_graph_launcher_position = agent_graph_launcher_home_position(
                window,
                self.agent_panel_visible && !session.agent_panel_detached,
            );
        }
        self.build_backdrop()?;
        self.build_toolbar()?;
        self.build_agent_panel()?;
        self.build_terminal_panel()?;

        let detached_indices = session
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(index, tab)| tab.detached.then_some(index))
            .collect::<Vec<_>>();
        let mut restored_active_tab_id = None;
        if session.tabs.is_empty() {
            self.create_tab(None, true)?;
        } else {
            for tab in &session.tabs {
                self.create_tab(tab.url.clone(), false)?;
            }
            let active_index = session.active_index.min(self.tabs.len().saturating_sub(1));
            restored_active_tab_id = self.tabs.get(active_index).map(|tab| tab.id);
            let primary_index = session
                .primary_index
                .unwrap_or(active_index)
                .min(self.tabs.len().saturating_sub(1));
            self.primary_tab_id = self.tabs.get(primary_index).map(|tab| tab.id);
            self.docked_tab_id = session
                .docked_index
                .filter(|index| *index < self.tabs.len() && *index != primary_index)
                .and_then(|index| self.tabs.get(index).map(|tab| tab.id));
            self.active_tab_id = self
                .tabs
                .get(active_index)
                .map(|tab| tab.id)
                .filter(|id| Some(*id) == self.primary_tab_id || Some(*id) == self.docked_tab_id)
                .or(self.primary_tab_id);
            self.sync_tab_webviews();
        }

        for index in detached_indices {
            let Some(tab_id) = self.tabs.get(index).map(|tab| tab.id) else {
                continue;
            };
            if let Err(error) = self.detach_tab(event_loop, tab_id) {
                warn!(%error, tab_id, "detached tab could not be restored");
            }
        }
        if let Some(tab_id) = restored_active_tab_id {
            self.activate_tab(tab_id);
        }

        self.build_agent_graph_surface()?;
        self.start_project_inspections();
        if session.agent_panel_detached
            && self.agent_panel_visible
            && let Err(error) = self.detach_agent_panel(event_loop)
        {
            warn!(%error, "detached agent panel could not be restored");
        }
        self.layout_webviews();

        self.start_claude_probe();
        for provider in [
            AgentProviderKind::Cursor,
            AgentProviderKind::GithubCopilot,
            AgentProviderKind::GoogleAntigravity,
            AgentProviderKind::OpencodeGo,
        ] {
            self.start_subscription_probe(provider);
        }
        self.render_toolbar();
        self.render_agent_panel();
        self.start_ui_development_watcher();
        if let Some(window) = &self.window {
            window.set_visible(true);
            window.focus_window();
        }
        info!(data_dir = %self.data_dir.display(), "browser initialized");
        Ok(())
    }

    fn build_backdrop(&mut self) -> anyhow::Result<()> {
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| anyhow!("main window is missing"))?;
        let background = theme_surface_color(&self.theme);
        let context = self
            .ui_context
            .as_mut()
            .ok_or_else(|| anyhow!("UI context is missing"))?;

        let backdrop = WebViewBuilder::new_with_web_context(context)
            .with_html(themed_ui_asset_html(
                self.ui_development_root.as_deref(),
                "assets/backdrop.html",
                BACKDROP_HTML,
                &self.theme,
            ))
            .with_bounds(full_window_bounds(window))
            .with_background_color(background)
            .with_visible(true)
            .with_devtools(false)
            .build_as_child(window)
            .context("failed to create the themed window backdrop")?;

        self.backdrop = Some(backdrop);
        Ok(())
    }

    fn build_toolbar(&mut self) -> anyhow::Result<()> {
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| anyhow!("main window is missing"))?;
        let bounds = self.active_toolbar_bounds(window);
        let background = theme_surface_color(&self.theme);
        let ipc_proxy = self.proxy.clone();
        let ready_proxy = self.proxy.clone();
        let zoom_proxy = self.proxy.clone();
        let context = self
            .ui_context
            .as_mut()
            .ok_or_else(|| anyhow!("UI context is missing"))?;

        let toolbar = trusted_ui_builder(context, self.ui_development_root.as_deref())
            .with_html(themed_ui_asset_html(
                self.ui_development_root.as_deref(),
                "assets/toolbar.html",
                TOOLBAR_HTML,
                &self.theme,
            ))
            .with_bounds(bounds)
            .with_background_color(background)
            .with_clipboard(true)
            .with_devtools(cfg!(debug_assertions))
            .with_ipc_handler(move |request| {
                match serde_json::from_str::<ToolbarCommand>(request.body()) {
                    Ok(command) => {
                        let _ = ipc_proxy.send_event(BrowserEvent::Toolbar(command));
                    }
                    Err(error) => warn!(%error, "toolbar message rejected"),
                }
            })
            .with_on_page_load_handler(move |event, _| {
                if matches!(event, PageLoadEvent::Finished) {
                    let _ = ready_proxy.send_event(BrowserEvent::ToolbarReady);
                }
            })
            .build_as_child(window)
            .context("failed to create the WebView2 toolbar")?;
        install_chat_zoom_accelerator(&toolbar, zoom_proxy)?;

        self.toolbar = Some(toolbar);
        Ok(())
    }

    fn build_agent_panel(&mut self) -> anyhow::Result<()> {
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| anyhow!("main window is missing"))?;
        let toolbar_height = self.active_toolbar_height(window);
        let bounds = agent_panel_bounds_with_width(
            window,
            self.effective_agent_panel_width(window),
            if self.browser_tabs_visible() {
                0
            } else {
                toolbar_height
            },
        );
        let background = theme_surface_color(&self.theme);
        let ipc_proxy = self.proxy.clone();
        let ready_proxy = self.proxy.clone();
        let zoom_proxy = self.proxy.clone();
        let context = self
            .ui_context
            .as_mut()
            .ok_or_else(|| anyhow!("UI context is missing"))?;

        let panel_builder = trusted_ui_builder(context, self.ui_development_root.as_deref())
            .with_html(themed_ui_asset_html(
                self.ui_development_root.as_deref(),
                "assets/agent-panel.html",
                AGENT_PANEL_HTML,
                &self.theme,
            ))
            .with_bounds(bounds)
            .with_background_color(background)
            .with_visible(self.agent_panel_visible)
            .with_clipboard(true)
            .with_devtools(cfg!(debug_assertions));
        // WebView2 consumes Ctrl+Plus/Minus as browser accelerators even when native zoom is
        // disabled, preventing the chat-specific shortcut from reaching the document.
        #[cfg(windows)]
        let panel_builder = panel_builder.with_browser_accelerator_keys(false);

        let panel = panel_builder
            // Do not install Wry's native file-drop handler on this WebView. On Windows it
            // replaces WebView2's drop target and disables the HTML Drag and Drop API, which
            // the toolbar-tab and terminal attachment protocols use. Files remain available
            // through the validated attachment picker in the composer.
            .with_ipc_handler(
                move |request| match main_runs::parse_control(request.body()) {
                    Ok((owner, message)) => {
                        let _ =
                            ipc_proxy.send_event(BrowserEvent::ScopedAgentPanel { owner, message });
                    }
                    Err(error) => warn!(%error, "agent panel command rejected"),
                },
            )
            .with_on_page_load_handler(move |event, _| {
                if matches!(event, PageLoadEvent::Finished) {
                    let _ = ready_proxy.send_event(BrowserEvent::AgentPanelReady);
                }
            })
            .build_as_child(window)
            .context("failed to create the WebView2 agent panel")?;
        install_chat_zoom_accelerator(&panel, zoom_proxy)?;

        self.agent_panel = Some(panel);
        Ok(())
    }

    fn build_terminal_panel(&mut self) -> anyhow::Result<()> {
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| anyhow!("main window is missing"))?;
        let bounds = terminal_panel_bounds_with_panel_width(
            window,
            self.active_toolbar_height(window),
            if self.browser_panel_minimized {
                None
            } else {
                self.docked_agent_panel_width(window)
            },
            self.terminal_dock,
        );
        let background = theme_surface_color(&self.theme);
        let ipc_proxy = self.proxy.clone();
        let ready_proxy = self.proxy.clone();
        let context = self
            .ui_context
            .as_mut()
            .ok_or_else(|| anyhow!("UI context is missing"))?;

        let panel = trusted_ui_builder(context, self.ui_development_root.as_deref())
            .with_html(terminal_panel_asset_html(
                self.ui_development_root.as_deref(),
                &self.theme,
            ))
            .with_bounds(bounds)
            .with_visible(self.terminal_panel_visible)
            .with_background_color(background)
            .with_clipboard(true)
            .with_devtools(cfg!(debug_assertions))
            .with_ipc_handler(
                move |request| match main_runs::parse_control(request.body()) {
                    Ok((_, message)) => {
                        let _ = ipc_proxy.send_event(BrowserEvent::TerminalPanel(message));
                    }
                    Err(error) => warn!(%error, "terminal panel command rejected"),
                },
            )
            .with_on_page_load_handler(move |event, _| {
                if matches!(event, PageLoadEvent::Finished) {
                    let _ = ready_proxy.send_event(BrowserEvent::TerminalPanelReady);
                }
            })
            .build_as_child(window)
            .context("failed to create the terminal panel")?;

        self.terminal_panel = Some(panel);
        Ok(())
    }

    fn create_tab(&mut self, initial_url: Option<String>, activate: bool) -> anyhow::Result<u64> {
        let tab_id = self.next_tab_id;
        self.next_tab_id += 1;

        let window = self
            .window
            .as_ref()
            .ok_or_else(|| anyhow!("main window is missing"))?;
        let bounds = browser_content_bounds_with_panel_width(
            window,
            self.active_toolbar_height(window),
            self.docked_agent_panel_width(window),
            self.terminal_panel_visible && self.terminal_window.is_none(),
            self.terminal_dock,
        );
        let load_proxy = self.proxy.clone();
        let title_proxy = self.proxy.clone();
        let popup_proxy = self.proxy.clone();
        let zoom_proxy = self.proxy.clone();
        let visible = activate;
        let defer_activation =
            should_defer_new_tab_activation(activate, self.primary_tab_id, self.docked_tab_id);
        let start_page = self.start_page_html();
        let background = theme_surface_color(&self.theme);
        let url = initial_url.filter(|url| is_supported_url(url));
        let is_start_page = url.is_none();
        let context = self
            .content_context
            .as_mut()
            .ok_or_else(|| anyhow!("content context is missing"))?;

        let mut builder = WebViewBuilder::new_with_web_context(context)
            .with_bounds(bounds)
            // WebView2 initially composites a white document surface. Keep the
            // controller hidden until its theme background has been applied,
            // then reveal it through `sync_tab_webviews`.
            .with_visible(false)
            .with_background_color(background)
            .with_clipboard(true)
            // Ctrl+Plus/Minus belongs to the Agent conversation, even while a
            // browser tab owns focus. Never zoom arbitrary page content here.
            .with_hotkeys_zoom(false)
            .with_back_forward_navigation_gestures(true)
            .with_general_autofill_enabled(true)
            .with_devtools(cfg!(debug_assertions))
            .with_navigation_handler(|url| is_allowed_webview_navigation(&url))
            .with_on_page_load_handler(move |event, url| {
                let loading = matches!(event, PageLoadEvent::Started);
                let _ = load_proxy.send_event(BrowserEvent::PageLoad {
                    tab_id,
                    loading,
                    url,
                });
            })
            .with_document_title_changed_handler(move |title| {
                let _ = title_proxy.send_event(BrowserEvent::TitleChanged { tab_id, title });
            })
            .with_new_window_req_handler(move |url, _| {
                if is_supported_url(&url) {
                    let _ = popup_proxy.send_event(BrowserEvent::BrowserPopup {
                        opener_tab_id: tab_id,
                        url,
                    });
                }
                NewWindowResponse::Deny
            });

        builder = match &url {
            Some(url) => builder.with_url(url),
            None => builder.with_html(start_page),
        };

        let webview = builder
            .build_as_child(window)
            .with_context(|| format!("failed to create tab {tab_id}"))?;
        install_chat_zoom_accelerator(&webview, zoom_proxy)?;
        #[cfg(windows)]
        if let Some(bridge) = self.browser_use_bridge.clone()
            && let Err(error) =
                browser_use_backend::install_devtools_events(&webview, tab_id, bridge)
        {
            warn!(%error, tab_id, "Browser Use event forwarding is unavailable for this tab");
        }

        self.tabs.push(BrowserTab {
            id: tab_id,
            title: DEFAULT_TITLE.to_owned(),
            url: url.unwrap_or_default(),
            loading: true,
            ready_to_show: false,
            content_revision: 1,
            is_start_page,
            web_preview: None,
            webview,
            detached_window: None,
        });

        if visible {
            if defer_activation {
                self.pending_tab_activation = Some(tab_id);
                self.render_toolbar();
                self.render_agent_panel();
            } else {
                self.primary_tab_id = Some(tab_id);
                self.active_tab_id = Some(tab_id);
                self.sync_tab_webviews();
                self.update_window_title();
                self.render_toolbar();
                self.render_agent_panel();
            }
        } else {
            self.render_toolbar();
            self.render_agent_panel();
        }

        self.save_session();
        Ok(tab_id)
    }

    fn create_remote_desktop_tab(&mut self) -> anyhow::Result<u64> {
        if let Some(tab_id) = self.remote_desktop_tab_id
            && self.tabs.iter().any(|tab| tab.id == tab_id)
        {
            self.activate_tab(tab_id);
            return Ok(tab_id);
        }

        let tab_id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.saturating_add(1);
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| anyhow!("main window is missing"))?;
        let bounds = browser_content_bounds_with_panel_width(
            window,
            self.active_toolbar_height(window),
            self.docked_agent_panel_width(window),
            self.terminal_panel_visible && self.terminal_window.is_none(),
            self.terminal_dock,
        );
        let background = theme_surface_color(&self.theme);
        let ipc_proxy = self.proxy.clone();
        let drag_proxy = self.proxy.clone();
        let context = self
            .ui_context
            .as_mut()
            .ok_or_else(|| anyhow!("UI context is missing"))?;
        let webview = WebViewBuilder::new_with_web_context(context)
            .with_html(themed_ui_asset_html(
                self.ui_development_root.as_deref(),
                "assets/remote-desktop.html",
                REMOTE_DESKTOP_HTML,
                &self.theme,
            ))
            .with_bounds(bounds)
            .with_visible(false)
            .with_background_color(background)
            .with_clipboard(true)
            .with_devtools(cfg!(debug_assertions))
            .with_ipc_handler(move |request| {
                match serde_json::from_str::<RemoteDesktopMessage>(request.body()) {
                    Ok(message) => {
                        let _ = ipc_proxy
                            .send_event(BrowserEvent::RemoteDesktopPanel { tab_id, message });
                    }
                    Err(error) => warn!(%error, "remote desktop command rejected"),
                }
            })
            .with_drag_drop_handler(move |event| {
                let event = match event {
                    DragDropEvent::Enter { paths, .. } => RemoteDesktopDragEvent::Enter {
                        file_count: paths.len(),
                    },
                    DragDropEvent::Over { .. } => return true,
                    DragDropEvent::Drop { paths, .. } => RemoteDesktopDragEvent::Drop { paths },
                    DragDropEvent::Leave => RemoteDesktopDragEvent::Leave,
                    _ => return true,
                };
                let _ = drag_proxy.send_event(BrowserEvent::RemoteDesktopDrag { tab_id, event });
                true
            })
            .build_as_child(window)
            .context("failed to create the Remote Linux Desktop tab")?;

        self.tabs.push(BrowserTab {
            id: tab_id,
            title: "Linux Desktop".to_owned(),
            url: String::new(),
            loading: true,
            ready_to_show: false,
            content_revision: 1,
            is_start_page: false,
            web_preview: None,
            webview,
            detached_window: None,
        });
        self.remote_desktop_tab_id = Some(tab_id);
        self.remote_desktop_ready = false;
        self.pending_tab_activation = Some(tab_id);
        self.render_toolbar();
        self.render_agent_panel();
        self.save_session();
        Ok(tab_id)
    }

    fn open_remote_desktop(
        &mut self,
        profile_id: &str,
        protocol: RemoteDesktopProtocol,
        remote_port: u16,
    ) {
        if remote_port == 0 {
            self.ssh
                .set_message("Remote desktop port must be between 1 and 65535.", true);
            self.render_agent_panel();
            return;
        }
        let Some(profile) = self.ssh.profile(profile_id).cloned() else {
            self.ssh
                .set_message("The selected VPS profile no longer exists.", true);
            self.render_agent_panel();
            return;
        };
        let remote_target = format!("{}@{}:{}", profile.username, profile.host, profile.port);
        self.remote_desktop.configure(
            profile.id,
            profile.name,
            profile.username,
            remote_target,
            protocol,
            remote_port,
        );
        match self.create_remote_desktop_tab() {
            Ok(_) => {
                self.close_settings();
                self.render_remote_desktop();
                self.save_session();
            }
            Err(error) => {
                self.remote_desktop
                    .report_error(format!("The Linux desktop tab could not open: {error}"));
                self.render_agent_panel();
            }
        }
    }

    fn handle_remote_desktop_message(&mut self, tab_id: u64, message: RemoteDesktopMessage) {
        if self.remote_desktop_tab_id != Some(tab_id) {
            return;
        }
        match message {
            RemoteDesktopMessage::Ready => {
                self.remote_desktop_ready = true;
                if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) {
                    tab.loading = false;
                    tab.ready_to_show = true;
                }
                if self.pending_tab_activation == Some(tab_id) {
                    self.activate_tab(tab_id);
                } else {
                    self.render_toolbar();
                }
                self.render_remote_desktop();
                self.remote_desktop.refresh();
            }
            RemoteDesktopMessage::Connect { username, password } => {
                let Some(profile_id) = self
                    .remote_desktop
                    .configured_profile_id()
                    .map(str::to_owned)
                else {
                    self.remote_desktop
                        .report_error("Select an SSH profile before connecting the Linux desktop");
                    self.render_remote_desktop();
                    self.render_agent_panel();
                    return;
                };
                let preference = self.remote_desktop.preference();
                let username = if preference.protocol == RemoteDesktopProtocol::Rdp {
                    let username = username.trim();
                    if username.is_empty() {
                        self.ssh
                            .profile(&profile_id)
                            .map(|profile| profile.username.clone())
                            .unwrap_or_default()
                    } else {
                        username.to_owned()
                    }
                } else {
                    String::new()
                };
                if username.len() > 128 || password.len() > 256 {
                    self.remote_desktop
                        .report_error("Remote desktop credentials exceeded the supported length");
                    self.render_remote_desktop();
                    self.render_agent_panel();
                    return;
                }
                let remote_port = preference.remote_port;
                let result = reserve_loopback_port().and_then(|local_port| {
                    self.ssh
                        .desktop_tunnel_spec(&profile_id, local_port, remote_port)
                });
                let spec = match result {
                    Ok(spec) => spec,
                    Err(message) => {
                        self.remote_desktop.report_error(message);
                        self.render_remote_desktop();
                        self.render_agent_panel();
                        return;
                    }
                };
                let proxy = self.proxy.clone();
                if let Err(message) =
                    self.remote_desktop
                        .start(spec, username, password, move |event| {
                            let _ = proxy.send_event(BrowserEvent::RemoteDesktop(event));
                        })
                {
                    self.remote_desktop.report_error(message);
                }
                self.render_remote_desktop();
                self.render_agent_panel();
            }
            RemoteDesktopMessage::Disconnect => {
                self.remote_desktop.disconnect();
                self.render_remote_desktop();
                self.render_agent_panel();
            }
            RemoteDesktopMessage::Refresh => self.remote_desktop.refresh(),
            RemoteDesktopMessage::Pointer { x, y, buttons } => {
                self.remote_desktop.pointer(x, y, buttons)
            }
            RemoteDesktopMessage::Key { code, keysym, down } => {
                self.remote_desktop.key(code, keysym, down)
            }
            RemoteDesktopMessage::Clipboard { text } => self.remote_desktop.clipboard(text),
            RemoteDesktopMessage::SetAgentControl { enabled } => {
                self.set_remote_desktop_agent_control(enabled)
            }
            RemoteDesktopMessage::Snapshot {
                request_id,
                width,
                height,
                data_url,
                error,
            } => self.complete_remote_desktop_snapshot(request_id, width, height, data_url, error),
        }
    }

    fn set_remote_desktop_agent_control(&mut self, enabled: bool) {
        let changed = self.remote_desktop.set_agent_control(enabled);
        if !enabled {
            self.permission_policy
                .revoke_scope(CapabilityScope::RemoteDesktop);
            let pending = self
                .pending_commands
                .iter()
                .filter(|(_, request)| matches!(request.command, AgentCommand::RemoteDesktop(_)))
                .map(|(request_id, _)| request_id.clone())
                .collect::<Vec<_>>();
            for request_id in pending {
                self.deny_pending_action(
                    &request_id,
                    "Remote Linux desktop control was stopped by the user",
                );
            }
            self.fail_active_remote_desktop_observations(
                "Remote Linux desktop control was stopped by the user",
            );
        }
        if changed {
            self.audit.record(
                if enabled {
                    "remote_desktop_agent_enabled"
                } else {
                    "remote_desktop_agent_disabled"
                },
                "",
                "remote_desktop",
                self.remote_desktop
                    .configured_profile_id()
                    .unwrap_or("unconfigured"),
                "external_interaction",
                if enabled { "running" } else { "stopped" },
                if enabled {
                    "Remote Linux desktop agent control enabled by the trusted local UI"
                } else {
                    "Remote Linux desktop agent control stopped by the trusted local UI"
                },
            );
        }
        self.render_remote_desktop();
        self.render_agent_panel();
    }

    fn fail_active_remote_desktop_observations(&mut self, message: &str) {
        let request_ids = self
            .action_log
            .iter()
            .filter(|entry| {
                entry.action == "remote_desktop_observe" && entry.status == CommandStatus::Running
            })
            .map(|entry| entry.request_id.clone())
            .collect::<Vec<_>>();
        for request_id in request_ids {
            self.finish_command(
                request_id,
                "remote_desktop_observe".to_owned(),
                false,
                message.to_owned(),
                Value::Null,
            );
        }
    }

    fn complete_remote_desktop_snapshot(
        &mut self,
        request_id: String,
        width: u16,
        height: u16,
        data_url: String,
        error: Option<String>,
    ) {
        let active = self.action_log.iter().any(|entry| {
            entry.request_id == request_id
                && entry.action == "remote_desktop_observe"
                && entry.status == CommandStatus::Running
        });
        if !active {
            return;
        }
        if let Err(message) = self.validate_remote_target(self.run_id_for_request(&request_id)) {
            self.finish_command(
                request_id,
                "remote_desktop_observe".to_owned(),
                false,
                message,
                Value::Null,
            );
            return;
        }
        if !self.remote_desktop.agent_control_enabled() || !self.remote_desktop.is_connected() {
            self.finish_command(
                request_id,
                "remote_desktop_observe".to_owned(),
                false,
                "Remote Linux desktop control is no longer enabled".to_owned(),
                Value::Null,
            );
            return;
        }
        if let Some(error) = error.filter(|error| !error.trim().is_empty()) {
            self.finish_command(
                request_id,
                "remote_desktop_observe".to_owned(),
                false,
                format!("Remote Linux desktop frame capture failed: {error}"),
                Value::Null,
            );
            return;
        }
        const JPEG_DATA_URL_PREFIX: &str = "data:image/jpeg;base64,";
        const MAX_REMOTE_DESKTOP_SNAPSHOT_CHARS: usize = 6 * 1_024 * 1_024;
        let valid_image = data_url.starts_with(JPEG_DATA_URL_PREFIX)
            && data_url.len() <= MAX_REMOTE_DESKTOP_SNAPSHOT_CHARS
            && BASE64_STANDARD
                .decode(&data_url[JPEG_DATA_URL_PREFIX.len()..])
                .is_ok_and(|jpeg| jpeg.starts_with(&[0xff, 0xd8]) && jpeg.ends_with(&[0xff, 0xd9]));
        if !valid_image || width == 0 || height == 0 {
            self.finish_command(
                request_id,
                "remote_desktop_observe".to_owned(),
                false,
                "Remote Linux desktop returned an invalid visual frame".to_owned(),
                Value::Null,
            );
            return;
        }
        self.provider_tool_images
            .insert(request_id.clone(), data_url);
        let view = self.remote_desktop.view();
        self.finish_command(
            request_id,
            "remote_desktop_observe".to_owned(),
            true,
            "Remote Linux desktop frame captured".to_owned(),
            json!({
                "profileId": view.profile_id,
                "profileName": view.profile_name,
                "protocol": view.protocol,
                "width": width,
                "height": height,
                "format": "jpeg",
            }),
        );
    }

    fn handle_remote_desktop_drag(&mut self, tab_id: u64, event: RemoteDesktopDragEvent) {
        if self.remote_desktop_tab_id != Some(tab_id) {
            return;
        }
        match event {
            RemoteDesktopDragEvent::Enter { file_count } => {
                self.stream_remote_desktop_transfer(json!({
                    "dragging": true,
                    "fileCount": file_count,
                    "busy": self.remote_desktop.view().upload_busy,
                }));
            }
            RemoteDesktopDragEvent::Leave => {
                self.stream_remote_desktop_transfer(json!({
                    "dragging": false,
                    "busy": self.remote_desktop.view().upload_busy,
                }));
            }
            RemoteDesktopDragEvent::Drop { paths } => {
                self.stream_remote_desktop_transfer(json!({
                    "dragging": false,
                    "busy": self.remote_desktop.view().upload_busy,
                }));
                self.start_remote_desktop_upload(paths);
            }
        }
    }

    fn start_remote_desktop_upload(&mut self, paths: Vec<PathBuf>) {
        let Some(profile_id) = self
            .remote_desktop
            .configured_profile_id()
            .map(str::to_owned)
        else {
            self.remote_desktop
                .finish_upload("Select an SSH profile before uploading files");
            self.render_remote_desktop();
            return;
        };
        if let Err(message) = self.remote_desktop.begin_upload(paths.len()) {
            self.remote_desktop.finish_upload(message);
            self.render_remote_desktop();
            return;
        }
        let spec = match self.ssh.upload_spec(&profile_id, &paths) {
            Ok(spec) => spec,
            Err(message) => {
                self.remote_desktop.finish_upload(message);
                self.render_remote_desktop();
                return;
            }
        };
        self.audit.record(
            "remote_desktop_file_upload_started",
            "",
            "ssh_profile",
            &profile_id,
            "external_interaction",
            "running",
            &format!(
                "{} local file(s) queued for authenticated VPS upload",
                paths.len()
            ),
        );
        self.render_remote_desktop();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = run_ssh_upload(spec);
            let _ =
                proxy.send_event(BrowserEvent::RemoteDesktopUploadCompleted { profile_id, result });
        });
    }

    fn complete_remote_desktop_upload(
        &mut self,
        profile_id: String,
        result: Result<SshUploadResult, String>,
    ) {
        if self.remote_desktop.configured_profile_id() != Some(profile_id.as_str()) {
            return;
        }
        match result {
            Ok(result) => {
                let count = result.file_names.len();
                let message = format!(
                    "Uploaded {count} file{} ({} KB) to {}",
                    if count == 1 { "" } else { "s" },
                    result.total_bytes.div_ceil(1_024),
                    result.remote_directory,
                );
                self.remote_desktop.finish_upload(message.clone());
                self.audit.record(
                    "remote_desktop_file_upload_completed",
                    "",
                    "ssh_profile",
                    &profile_id,
                    "external_interaction",
                    "success",
                    &message,
                );
            }
            Err(message) => {
                self.remote_desktop
                    .finish_upload(format!("Upload failed: {message}"));
                self.audit.record(
                    "remote_desktop_file_upload_failed",
                    "",
                    "ssh_profile",
                    &profile_id,
                    "external_interaction",
                    "error",
                    &message,
                );
            }
        }
        self.render_remote_desktop();
        self.render_agent_panel();
    }

    fn handle_remote_desktop_event(&mut self, event: RemoteDesktopEvent) {
        if !self.remote_desktop.apply_event(&event) {
            return;
        }
        if matches!(event, RemoteDesktopEvent::Stopped { .. }) {
            self.permission_policy
                .revoke_scope(CapabilityScope::RemoteDesktop);
            self.fail_active_remote_desktop_observations(
                "The remote Linux desktop connection stopped",
            );
        }
        match &event {
            RemoteDesktopEvent::Draw {
                generation,
                frame_id,
                operations,
            } => self.stream_remote_desktop_draw(*generation, *frame_id, operations),
            RemoteDesktopEvent::Cursor { cursor, .. } => self.stream_remote_desktop_cursor(cursor),
            RemoteDesktopEvent::Clipboard { text, .. } => {
                self.stream_remote_desktop_clipboard(text)
            }
            RemoteDesktopEvent::Bell { .. } => self.signal_remote_desktop_bell(),
            RemoteDesktopEvent::Connected { .. }
            | RemoteDesktopEvent::Resolution { .. }
            | RemoteDesktopEvent::Stopped { .. } => {
                self.render_remote_desktop();
                self.render_agent_panel();
            }
        }
    }

    fn remote_desktop_webview(&self) -> Option<&WebView> {
        let tab_id = self.remote_desktop_tab_id?;
        self.tabs
            .iter()
            .find(|tab| tab.id == tab_id)
            .map(|tab| &tab.webview)
    }

    fn render_remote_desktop(&self) {
        if !self.remote_desktop_ready {
            return;
        }
        let Some(webview) = self.remote_desktop_webview() else {
            return;
        };
        let Ok(state) = serde_json::to_string(&self.remote_desktop.view()) else {
            return;
        };
        let Ok(theme) = serde_json::to_string(&self.theme) else {
            return;
        };
        let script = format!(
            "window.renderRemoteDesktopState && window.renderRemoteDesktopState({state}, {theme});"
        );
        if let Err(error) = webview.evaluate_script(&script) {
            warn!(%error, "remote desktop state update failed");
        }
    }

    fn stream_remote_desktop_draw(
        &self,
        generation: u64,
        frame_id: u64,
        operations: &[crate::remote_desktop::RemoteDesktopDrawOp],
    ) {
        if !self.remote_desktop_ready || operations.is_empty() {
            return;
        }
        let Some(webview) = self.remote_desktop_webview() else {
            return;
        };
        let Ok(operations) = serde_json::to_string(operations) else {
            return;
        };
        let script = if self.remote_desktop.preference().protocol
            == crate::remote_desktop::RemoteDesktopProtocol::Vnc
        {
            format!("window.receiveVncDesktopDraw && window.receiveVncDesktopDraw({operations});")
        } else {
            format!(
                "window.receiveRemoteDesktopDraw && window.receiveRemoteDesktopDraw({generation}, {frame_id}, false, {operations});"
            )
        };
        if let Err(error) = webview.evaluate_script(&script) {
            warn!(%error, "remote desktop frame update failed");
        }
    }

    fn stream_remote_desktop_clipboard(&self, text: &str) {
        if !self.remote_desktop_ready {
            return;
        }
        let Some(webview) = self.remote_desktop_webview() else {
            return;
        };
        let Ok(text) = serde_json::to_string(text) else {
            return;
        };
        let script = format!(
            "window.receiveRemoteDesktopClipboard && window.receiveRemoteDesktopClipboard({text});"
        );
        if let Err(error) = webview.evaluate_script(&script) {
            warn!(%error, "remote desktop clipboard update failed");
        }
    }

    fn stream_remote_desktop_cursor(&self, cursor: &crate::remote_desktop::RemoteDesktopCursor) {
        if !self.remote_desktop_ready {
            return;
        }
        let Some(webview) = self.remote_desktop_webview() else {
            return;
        };
        let Ok(cursor) = serde_json::to_string(cursor) else {
            return;
        };
        let script = format!(
            "window.receiveRemoteDesktopCursor && window.receiveRemoteDesktopCursor({cursor});"
        );
        if let Err(error) = webview.evaluate_script(&script) {
            warn!(%error, "remote desktop cursor update failed");
        }
    }

    fn stream_remote_desktop_agent_action(&self, action: Value) {
        if !self.remote_desktop_ready {
            return;
        }
        let Some(webview) = self.remote_desktop_webview() else {
            return;
        };
        let script = format!(
            "window.receiveRemoteDesktopAgentAction && window.receiveRemoteDesktopAgentAction({action});"
        );
        if let Err(error) = webview.evaluate_script(&script) {
            warn!(%error, "remote desktop agent activity update failed");
        }
    }

    fn stream_remote_desktop_transfer(&self, transfer: Value) {
        if !self.remote_desktop_ready {
            return;
        }
        let Some(webview) = self.remote_desktop_webview() else {
            return;
        };
        let script = format!(
            "window.renderRemoteDesktopTransfer && window.renderRemoteDesktopTransfer({transfer});"
        );
        if let Err(error) = webview.evaluate_script(&script) {
            warn!(%error, "remote desktop upload UI update failed");
        }
    }

    fn signal_remote_desktop_bell(&self) {
        if let Some(webview) = self.remote_desktop_webview()
            && let Err(error) = webview.evaluate_script(
                "window.receiveRemoteDesktopBell && window.receiveRemoteDesktopBell();",
            )
        {
            warn!(%error, "remote desktop bell update failed");
        }
    }

    fn activate_tab(&mut self, tab_id: u64) {
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == tab_id) else {
            return;
        };

        // A freshly-created WebView2 controller has a black composition
        // surface until its first document finishes loading. Keep the current
        // pane visible and complete the switch from `handle_page_load`.
        if !tab.ready_to_show {
            self.pending_tab_activation = Some(tab_id);
            self.render_toolbar();
            self.render_agent_panel();
            return;
        }
        self.pending_tab_activation = None;

        let detached = tab.detached_window.is_some();
        if !detached && self.docked_tab_id != Some(tab_id) {
            self.primary_tab_id = Some(tab_id);
        }
        self.active_tab_id = Some(tab_id);
        self.sync_tab_webviews();
        if detached
            && let Some(window) = self
                .tabs
                .iter()
                .find(|tab| tab.id == tab_id)
                .and_then(|tab| tab.detached_window.as_ref())
        {
            window.set_minimized(false);
            window.focus_window();
        }
        self.update_window_title();
        self.render_toolbar();
        self.render_agent_panel();
        if self.remote_desktop_tab_id == Some(tab_id) {
            self.render_remote_desktop();
            self.remote_desktop.refresh();
        }
        self.save_session();
    }

    fn reorder_tab(&mut self, tab_id: u64, before_tab_id: Option<u64>) {
        let Some(source_index) = self.tabs.iter().position(|tab| tab.id == tab_id) else {
            return;
        };
        if before_tab_id == Some(tab_id) {
            return;
        }
        let tab = self.tabs.remove(source_index);
        let target_index = before_tab_id
            .and_then(|target_id| self.tabs.iter().position(|tab| tab.id == target_id))
            .unwrap_or(self.tabs.len());
        self.tabs.insert(target_index, tab);
        self.render_toolbar();
        self.render_agent_panel();
        self.save_session();
    }

    fn dock_tab(&mut self, tab_id: u64, side: TabDockSide) {
        if !self.tabs.iter().any(|tab| tab.id == tab_id) {
            return;
        }

        if self.docked_tab_id == Some(tab_id) {
            self.tab_dock_side = side;
            self.active_tab_id = Some(tab_id);
            self.sync_tab_webviews();
            self.render_toolbar();
            self.render_agent_panel();
            self.save_session();
            return;
        }

        if self.primary_tab_id == Some(tab_id) {
            let replacement = self
                .docked_tab_id
                .filter(|id| *id != tab_id)
                .or_else(|| {
                    self.tabs
                        .iter()
                        .find(|tab| tab.id != tab_id)
                        .map(|tab| tab.id)
                })
                .or_else(|| self.create_tab(None, false).ok());
            let Some(replacement) = replacement else {
                return;
            };
            self.primary_tab_id = Some(replacement);
        } else if self.primary_tab_id.is_none() {
            self.primary_tab_id = self
                .tabs
                .iter()
                .find(|tab| tab.id != tab_id)
                .map(|tab| tab.id);
        }

        self.docked_tab_id = Some(tab_id);
        self.tab_dock_side = side;
        self.active_tab_id = Some(tab_id);
        self.sync_tab_webviews();
        self.render_toolbar();
        self.render_agent_panel();
        self.save_session();
    }

    fn undock_tab(&mut self, tab_id: u64) {
        if self.docked_tab_id != Some(tab_id) {
            return;
        }
        self.docked_tab_id = None;
        self.primary_tab_id = Some(tab_id);
        self.active_tab_id = Some(tab_id);
        self.sync_tab_webviews();
        self.render_toolbar();
        self.render_agent_panel();
        self.save_session();
    }

    fn detach_tab(&mut self, event_loop: &ActiveEventLoop, tab_id: u64) -> anyhow::Result<()> {
        if let Some(window) = self
            .tabs
            .iter()
            .find(|tab| tab.id == tab_id)
            .and_then(|tab| tab.detached_window.as_ref())
        {
            window.set_minimized(false);
            window.focus_window();
            return Ok(());
        }
        if !self.tabs.iter().any(|tab| tab.id == tab_id) {
            return Ok(());
        }

        let monitor = self
            .window
            .as_ref()
            .and_then(Window::current_monitor)
            .or_else(|| event_loop.primary_monitor());
        let detached_window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Supervisor")
                    .with_window_icon(Some(brand::window_icon()))
                    .with_inner_size(LogicalSize::new(
                        DETACHED_TAB_WIDTH_LOGICAL,
                        DETACHED_TAB_HEIGHT_LOGICAL,
                    ))
                    .with_min_inner_size(LogicalSize::new(640.0, 420.0))
                    .with_resizable(true)
                    .with_visible(false),
            )
            .context("failed to create the detached tab window")?;
        if let Some(monitor) = monitor.as_ref() {
            center_window_on_monitor(&detached_window, monitor);
        }
        apply_native_window_theme(&detached_window, &self.theme);

        {
            let tab = self
                .tabs
                .iter_mut()
                .find(|tab| tab.id == tab_id)
                .ok_or_else(|| anyhow!("tab {tab_id} no longer exists"))?;
            reparent_webview(&tab.webview, &detached_window)
                .with_context(|| format!("failed to detach tab {tab_id}"))?;
            tab.detached_window = Some(detached_window);
            if let Some(window) = tab.detached_window.as_ref() {
                if let Err(error) = tab.webview.set_bounds(full_window_bounds(window)) {
                    warn!(%error, tab_id, "detached tab bounds were not updated");
                }
                if let Err(error) = tab.webview.set_visible(true) {
                    warn!(%error, tab_id, "detached tab was not made visible");
                }
                window.set_visible(true);
            }
        }

        if self.docked_tab_id == Some(tab_id) {
            self.docked_tab_id = None;
        }
        if self.primary_tab_id == Some(tab_id) {
            let replacement = self
                .docked_tab_id
                .take()
                .filter(|id| {
                    self.tabs
                        .iter()
                        .find(|tab| tab.id == *id)
                        .is_some_and(|tab| tab.detached_window.is_none())
                })
                .or_else(|| {
                    self.tabs
                        .iter()
                        .find(|tab| tab.id != tab_id && tab.detached_window.is_none())
                        .map(|tab| tab.id)
                });
            self.primary_tab_id = replacement;
            if self.primary_tab_id.is_none() {
                self.primary_tab_id = Some(self.create_tab(None, false)?);
            }
        }
        if self.primary_tab_id.is_none() {
            self.primary_tab_id = self
                .tabs
                .iter()
                .find(|tab| tab.detached_window.is_none())
                .map(|tab| tab.id);
            if self.primary_tab_id.is_none() {
                self.primary_tab_id = Some(self.create_tab(None, false)?);
            }
        }

        self.active_tab_id = Some(tab_id);
        self.sync_tab_webviews();
        if let Some(window) = self
            .tabs
            .iter()
            .find(|tab| tab.id == tab_id)
            .and_then(|tab| tab.detached_window.as_ref())
        {
            window.focus_window();
        }
        self.render_toolbar();
        self.render_agent_panel();
        self.save_session();
        Ok(())
    }

    fn attach_tab(&mut self, tab_id: u64) -> anyhow::Result<()> {
        let main_window = self
            .window
            .as_ref()
            .ok_or_else(|| anyhow!("main window is missing"))?;
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) else {
            return Ok(());
        };
        if tab.detached_window.is_none() {
            return Ok(());
        }

        reparent_webview(&tab.webview, main_window)
            .with_context(|| format!("failed to reattach tab {tab_id}"))?;
        tab.detached_window.take();
        self.docked_tab_id = self.docked_tab_id.filter(|id| *id != tab_id);
        self.primary_tab_id = Some(tab_id);
        self.active_tab_id = Some(tab_id);
        self.sync_tab_webviews();
        main_window.set_minimized(false);
        main_window.focus_window();
        self.render_toolbar();
        self.render_agent_panel();
        self.save_session();
        Ok(())
    }

    fn update_tab_webview_visibility(&self) {
        let background = theme_surface_color(&self.theme);
        let tab_states = self
            .tabs
            .iter()
            .map(|tab| (tab.id, tab.detached_window.is_some()))
            .collect::<Vec<_>>();
        let (primary_tab_id, docked_tab_id) = if self.settings_covering_main()
            || self.agent_graph_open
            || self.browser_panel_minimized
        {
            (None, None)
        } else {
            (self.primary_tab_id, self.docked_tab_id)
        };
        for (index, visible) in
            ordered_tab_visibility_updates(&tab_states, primary_tab_id, docked_tab_id)
        {
            let tab = &self.tabs[index];
            if visible && let Err(error) = tab.webview.set_background_color(background) {
                warn!(%error, tab_id = tab.id, "tab surface was not prepared before reveal");
            }
            if let Err(error) = tab.webview.set_visible(visible) {
                warn!(%error, tab_id = tab.id, "tab visibility was not updated");
            }
        }
    }

    fn focus_current_surface(&self) {
        if self.agent_graph_open && !self.settings_open {
            if let Some(surface) = &self.agent_graph_surface {
                let _ = surface.focus();
            }
        } else if self.settings_open || self.browser_panel_minimized {
            if let Some(panel) = &self.agent_panel {
                let _ = panel.focus();
            }
        } else if let Some(tab) = self.active_tab() {
            let _ = tab.webview.focus();
        }
    }

    fn sync_tab_webviews(&self) {
        self.update_tab_webview_visibility();
        self.layout_webviews();
        self.focus_current_surface();
    }

    fn close_tab(&mut self, tab_id: u64) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == tab_id) else {
            return;
        };
        #[cfg(windows)]
        {
            self.browser_use_owned_tabs.remove(&tab_id);
            self.browser_use_activity.remove(&tab_id);
            if let Some(bridge) = &self.browser_use_bridge {
                bridge.notify_cdp_detach(tab_id);
            }
        }
        let closing_remote_desktop = self.remote_desktop_tab_id == Some(tab_id);
        if closing_remote_desktop && self.tabs.len() == 1 {
            let _ = self.create_tab(None, false);
        }
        if closing_remote_desktop {
            self.set_remote_desktop_agent_control(false);
            self.remote_desktop.disconnect();
            self.remote_desktop_tab_id = None;
            self.remote_desktop_ready = false;
        }
        if self.pending_tab_activation == Some(tab_id) {
            self.pending_tab_activation = None;
        }
        self.forget_tab_drafts(tab_id);
        for context in self.run_execution_contexts.values_mut() {
            if context.native_targets.browser_tab_id == Some(tab_id) {
                context.native_targets.browser_tab_id = None;
            }
        }
        for submission in self
            .agent_submission_queue
            .iter_mut()
            .chain(&mut self.pending_agent_submissions)
        {
            if let Some(scope) = &mut submission.scope
                && scope.native_targets.browser_tab_id == Some(tab_id)
            {
                scope.native_targets.browser_tab_id = None;
            }
        }

        if self.tabs.len() == 1 {
            let html = self.start_page_html();
            let background = theme_surface_color(&self.theme);
            let tab = &mut self.tabs[index];
            if let Err(error) = tab.webview.set_background_color(background) {
                warn!(%error, tab_id, "tab background was not reset before closing");
            }
            if let Err(error) = tab.webview.load_html(&html) {
                warn!(%error, tab_id, "the final tab could not be reset");
                return;
            }
            tab.title = DEFAULT_TITLE.to_owned();
            tab.url.clear();
            tab.loading = true;
            // The existing controller remains on screen while it navigates;
            // it has already produced a valid composition frame.
            tab.ready_to_show = true;
            tab.content_revision = tab.content_revision.saturating_add(1);
            tab.is_start_page = true;
            self.active_tab_id = Some(tab_id);
            self.primary_tab_id = Some(tab_id);
            self.docked_tab_id = None;
            self.sync_tab_webviews();
            self.update_window_title();
            self.render_toolbar();
            self.render_agent_panel();
            self.save_session();
            return;
        }

        let was_active = self.active_tab_id == Some(tab_id);
        let was_primary = self.primary_tab_id == Some(tab_id);
        let was_docked = self.docked_tab_id == Some(tab_id);
        let remaining_ids = self
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(candidate_index, tab)| (candidate_index != index).then_some(tab.id))
            .collect::<Vec<_>>();

        if was_docked {
            self.docked_tab_id = None;
        }
        if was_primary {
            let neighbor_index = index.saturating_sub(1).min(remaining_ids.len() - 1);
            self.primary_tab_id = remaining_ids
                .iter()
                .cycle()
                .skip(neighbor_index)
                .take(remaining_ids.len())
                .copied()
                .find(|candidate_id| Some(*candidate_id) != self.docked_tab_id);
            if self.primary_tab_id.is_none() {
                self.primary_tab_id = self.docked_tab_id.take();
            }
        }
        if self.primary_tab_id.is_none() {
            self.primary_tab_id = remaining_ids.first().copied();
        }
        if was_active
            || !self
                .active_tab_id
                .is_some_and(|id| id != tab_id && remaining_ids.contains(&id))
        {
            self.active_tab_id = self.primary_tab_id.or(self.docked_tab_id);
        }

        // Reveal the successor before the outgoing WebView is hidden and
        // destroyed, so the native parent never exposes its default surface.
        self.sync_tab_webviews();
        self.tabs.remove(index);
        self.sync_tab_webviews();
        self.render_toolbar();
        self.render_agent_panel();
        self.save_session();
    }

    fn handle_toolbar_command(&mut self, event_loop: &ActiveEventLoop, command: ToolbarCommand) {
        // Projects is an application workspace, not a browser overlay. Ignore
        // stale toolbar events and shortcuts after its browser controls leave
        // the DOM so a hidden tab can never navigate underneath the board.
        if self.agent_graph_open && command.targets_browser() {
            return;
        }
        // Explicit browser navigation restores the pane. Provider-driven tab
        // operations do not override the user's minimized workspace.
        if self.browser_panel_minimized
            && matches!(
                &command,
                ToolbarCommand::Navigate { .. }
                    | ToolbarCommand::Back
                    | ToolbarCommand::Forward
                    | ToolbarCommand::Reload
                    | ToolbarCommand::Home
                    | ToolbarCommand::NewTab
                    | ToolbarCommand::ActivateTab { .. }
                    | ToolbarCommand::AttachTab { .. }
                    | ToolbarCommand::DockTab { .. }
                    | ToolbarCommand::UndockTab { .. }
            )
        {
            self.set_browser_panel_minimized(false);
        }
        match command {
            ToolbarCommand::Navigate { value } => {
                if let Some(url) =
                    normalize_address_input(&value, search_engine(&self.search_engine))
                {
                    if self.active_tab_id == self.remote_desktop_tab_id {
                        if let Err(error) = self.create_tab(Some(url.clone()), true) {
                            warn!(%error, %url, "navigation tab could not be created");
                        }
                    } else if let Some(active_id) = self.active_tab_id
                        && let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == active_id)
                    {
                        if let Err(error) = tab.webview.load_url(&url) {
                            warn!(%error, %url, "navigation failed");
                        } else {
                            tab.is_start_page = false;
                        }
                        self.layout_agent_graph_surface();
                    }
                }
            }
            ToolbarCommand::Back => {
                if self.active_tab_id != self.remote_desktop_tab_id
                    && let Some(tab) = self.active_tab()
                    && let Err(error) = tab.webview.go_back()
                {
                    warn!(%error, "back navigation failed");
                }
            }
            ToolbarCommand::Forward => {
                if self.active_tab_id != self.remote_desktop_tab_id
                    && let Some(tab) = self.active_tab()
                    && let Err(error) = tab.webview.go_forward()
                {
                    warn!(%error, "forward navigation failed");
                }
            }
            ToolbarCommand::Reload => {
                if self.active_tab_id == self.remote_desktop_tab_id {
                    self.remote_desktop.refresh();
                } else if let Some(tab) = self.active_tab()
                    && let Err(error) = tab.webview.reload()
                {
                    warn!(%error, "reload failed");
                }
            }
            ToolbarCommand::Stop => {
                if self.active_tab_id != self.remote_desktop_tab_id
                    && let Some(tab) = self.active_tab()
                    && let Err(error) = tab.webview.evaluate_script("window.stop();")
                {
                    warn!(%error, "stop failed");
                }
            }
            ToolbarCommand::Home => {
                if self.active_tab_id == self.remote_desktop_tab_id {
                    if let Err(error) = self.create_tab(None, true) {
                        warn!(%error, "failed to open a new start page");
                    }
                } else {
                    let html = self.start_page_html();
                    if let Some(active_id) = self.active_tab_id
                        && let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == active_id)
                    {
                        if let Err(error) = tab.webview.load_html(&html) {
                            warn!(%error, "failed to open the start page");
                        } else {
                            tab.is_start_page = true;
                            tab.loading = true;
                        }
                        self.layout_agent_graph_surface();
                    }
                }
            }
            ToolbarCommand::NewTab => {
                if let Err(error) = self.create_tab(None, true) {
                    error!(%error, "failed to create a new tab");
                }
            }
            ToolbarCommand::CloseTab { tab_id } => self.close_tab(tab_id),
            ToolbarCommand::ActivateTab { tab_id } => self.activate_tab(tab_id),
            ToolbarCommand::ReorderTab {
                tab_id,
                before_tab_id,
            } => self.reorder_tab(tab_id, before_tab_id),
            ToolbarCommand::DockTab { tab_id, side } => self.dock_tab(tab_id, side),
            ToolbarCommand::UndockTab { tab_id } => self.undock_tab(tab_id),
            ToolbarCommand::DetachTab {
                tab_id,
                screen_x,
                screen_y,
            } => {
                if screen_x
                    .zip(screen_y)
                    .is_some_and(|(x, y)| !self.point_is_outside_main_window(x, y))
                {
                    return;
                }
                if let Err(error) = self.detach_tab(event_loop, tab_id) {
                    warn!(%error, tab_id, "tab could not be opened in a separate window");
                }
            }
            ToolbarCommand::AttachTab { tab_id } => {
                if let Err(error) = self.attach_tab(tab_id) {
                    warn!(%error, tab_id, "tab could not be returned to the main window");
                }
            }
            ToolbarCommand::OpenSettings => self.open_settings(),
            ToolbarCommand::ToggleAgentPanel => self.toggle_agent_panel(),
            ToolbarCommand::ToggleBrowserPanel => {
                self.set_browser_panel_minimized(!self.browser_panel_minimized)
            }
            ToolbarCommand::AgentPanelCloseLayoutReady => self.commit_agent_panel_close(),
            ToolbarCommand::ToggleTerminalPanel => {
                if self.agent_graph_open {
                    let terminal_was_available =
                        self.terminal_panel_visible || self.terminal_window.is_some();
                    self.close_agent_graph();
                    if let Some(window) = self.terminal_window.as_ref() {
                        window.set_minimized(false);
                        window.focus_window();
                    } else if !terminal_was_available {
                        self.toggle_terminal_panel();
                    }
                } else {
                    self.toggle_terminal_panel();
                }
            }
            ToolbarCommand::CloseProjectBoard => self.close_agent_graph(),
        }
    }

    fn open_settings(&mut self) {
        if self.settings_open {
            return;
        }
        self.agent_graph_open = false;
        self.agent_graph_launcher_drag = None;
        self.agent_panel_close_pending = false;
        self.settings_open = true;
        // Resizing the native surface alone leaves the graph DOM expanded, so
        // restoring it after Settings would show a cropped heading, not the logo.
        // Project the collapsed state for both docked and detached Settings.
        self.render_agent_graph_surface();
        self.layout_agent_graph_surface();
        self.render_toolbar();
        self.render_agent_panel();
        if let Some(window) = self.agent_panel_window.as_ref() {
            window.set_minimized(false);
            window.focus_window();
        }
        if !self.agent_panel_ready {
            self.settings_surface_ready();
        }
    }

    fn settings_surface_ready(&mut self) {
        if !self.settings_open {
            return;
        }
        // Expand the already-themed settings WebView across the whole main surface.
        // JavaScript acknowledges a painted frame before the outgoing browser
        // content is hidden, so no native fallback background is exposed.
        self.layout_webviews();
        if let Some(panel) = &self.agent_panel
            && let Err(error) = panel.evaluate_script(
                "window.centralAgentCommitSettings && window.centralAgentCommitSettings();",
            )
        {
            warn!(%error, "settings surface could not be committed");
        }
    }

    fn settings_cover_ready(&self) {
        if !self.settings_open {
            return;
        }
        // The expanded settings surface has now painted. Removing the old tabs at
        // this point cannot expose WebView2's native fallback background.
        self.update_tab_webview_visibility();
        self.focus_current_surface();
    }

    fn close_settings(&mut self) {
        if !self.settings_open {
            return;
        }
        self.settings_open = false;
        // Reveal the browser first, then shrink the still-opaque settings surface.
        // Its DOM overlay stays mounted until the resized Agent home has painted.
        self.update_tab_webview_visibility();
        self.layout_webviews();
        self.render_toolbar();
        if self.agent_panel_visible {
            if let Some(panel) = &self.agent_panel
                && let Err(error) = panel.evaluate_script(
                    "window.centralAgentAwaitSettingsCloseCommit && window.centralAgentAwaitSettingsCloseCommit();",
                )
            {
                warn!(%error, "settings close layout could not be prepared");
                self.commit_settings_close();
            }
        } else {
            self.commit_settings_close();
        }
    }

    fn settings_close_layout_ready(&self) {
        if self.settings_open {
            return;
        }
        self.commit_settings_close();
    }

    fn commit_settings_close(&self) {
        if let Some(panel) = &self.agent_panel
            && let Err(error) = panel.evaluate_script(
                "window.centralAgentCommitSettingsClose && window.centralAgentCommitSettingsClose();",
            )
        {
            warn!(%error, "settings close could not be committed");
        }
        self.render_agent_panel();
        if self.agent_panel_window.is_some() {
            if let Some(panel) = &self.agent_panel {
                let _ = panel.focus();
            }
        } else {
            self.focus_current_surface();
        }
    }

    fn set_browser_panel_minimized(&mut self, minimized: bool) {
        if self.settings_open
            || self.agent_graph_open
            || self.agent_panel_window.is_some()
            || self.browser_panel_minimized == minimized
        {
            return;
        }
        self.browser_panel_minimized = minimized;
        if minimized {
            self.agent_panel_visible = true;
            self.agent_panel_close_pending = false;
            // Expand the themed chat surface before hiding native tab controllers.
            self.layout_webviews();
            self.update_tab_webview_visibility();
        } else {
            // Prepare and reveal the existing tabs behind the expanded chat,
            // then return the chat to its saved width. No navigation or reload.
            self.layout_tab_webviews();
            self.update_tab_webview_visibility();
            self.layout_webviews();
        }
        self.render_toolbar();
        self.render_agent_panel();
        self.focus_current_surface();
        self.save_session();
    }

    fn toggle_agent_panel(&mut self) {
        if self.settings_open {
            return;
        }
        if let Some(window) = self.agent_panel_window.as_ref() {
            window.set_minimized(false);
            window.focus_window();
            return;
        }
        if self.agent_panel_visible {
            self.begin_agent_panel_close();
            return;
        }
        self.agent_panel_close_pending = false;
        self.agent_panel_visible = true;
        self.layout_webviews();
        self.render_toolbar();
        self.render_agent_panel();
        self.save_session();
    }

    fn begin_agent_panel_close(&mut self) {
        self.browser_panel_minimized = false;
        self.agent_panel_visible = false;
        self.agent_panel_close_pending = true;

        // Expand every surface that was constrained by the Agent panel while the
        // old panel remains visible. The toolbar acknowledges several compositor
        // frames before the panel WebView is actually hidden.
        if let (Some(window), Some(toolbar)) = (self.window.as_ref(), self.toolbar.as_ref())
            && let Err(error) = toolbar.set_bounds(self.active_toolbar_bounds(window))
        {
            warn!(%error, "failed to expand the browser toolbar");
        }
        self.layout_terminal_webview();
        self.layout_tab_webviews();
        self.layout_agent_graph_surface();
        self.update_tab_webview_visibility();
        self.render_toolbar();
        self.render_agent_panel();
        self.save_session();

        let waiting_for_layout = self.toolbar_ready
            && self.toolbar.as_ref().is_some_and(|toolbar| {
                toolbar
                    .evaluate_script(
                        "window.centralAgentAwaitPanelCloseLayout && window.centralAgentAwaitPanelCloseLayout();",
                    )
                    .is_ok()
            });
        if !waiting_for_layout {
            self.commit_agent_panel_close();
        }
    }

    fn commit_agent_panel_close(&mut self) {
        if !self.agent_panel_close_pending || self.agent_panel_visible || self.settings_open {
            return;
        }
        self.agent_panel_close_pending = false;
        if let Some(panel) = &self.agent_panel
            && let Err(error) = panel.set_visible(false)
        {
            warn!(%error, "agent panel could not be hidden after layout commit");
        }
        self.focus_current_surface();
    }

    fn agent_panel_docked_in_main(&self) -> bool {
        self.agent_panel_visible && self.agent_panel_window.is_none() && !self.settings_open
    }

    fn browser_tabs_visible(&self) -> bool {
        !self.browser_panel_minimized && !self.settings_covering_main() && !self.agent_graph_open
    }

    fn active_toolbar_height(&self, window: &Window) -> u32 {
        if self.agent_graph_open {
            project_toolbar_height(window)
        } else if self.browser_tabs_visible() {
            toolbar_height(window)
        } else {
            navigation_toolbar_height(window)
        }
    }

    fn active_toolbar_bounds(&self, window: &Window) -> Rect {
        if self.agent_graph_open {
            return project_toolbar_bounds(window);
        }
        browser_toolbar_bounds(
            window,
            self.browser_tabs_visible()
                .then(|| self.docked_agent_panel_width(window))
                .flatten(),
            self.active_toolbar_height(window),
        )
    }

    fn effective_agent_panel_width(&self, window: &Window) -> u32 {
        configured_agent_panel_width(window, self.agent_panel_width_logical)
    }

    fn docked_agent_panel_width(&self, window: &Window) -> Option<u32> {
        self.agent_panel_docked_in_main()
            .then(|| self.effective_agent_panel_width(window))
    }

    fn set_agent_panel_width(&mut self, width: f64, persist: bool) {
        if !width.is_finite()
            || self.agent_panel_window.is_some()
            || self.settings_open
            || self.browser_panel_minimized
        {
            return;
        }
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let (minimum, maximum) = agent_panel_width_limits_logical(window);
        self.agent_panel_width_logical = Some(width.clamp(minimum, maximum));
        self.layout_webviews();
        if persist {
            self.save_session();
        }
    }

    fn settings_covering_main(&self) -> bool {
        self.settings_open && self.agent_panel_window.is_none()
    }

    fn detach_agent_panel(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        if let Some(window) = self.agent_panel_window.as_ref() {
            window.set_minimized(false);
            window.focus_window();
            return Ok(());
        }

        let monitor = self
            .window
            .as_ref()
            .and_then(Window::current_monitor)
            .or_else(|| event_loop.primary_monitor());
        let detached_window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Supervisor")
                    .with_window_icon(Some(brand::window_icon()))
                    .with_inner_size(LogicalSize::new(
                        DETACHED_AGENT_PANEL_WIDTH_LOGICAL,
                        DETACHED_AGENT_PANEL_HEIGHT_LOGICAL,
                    ))
                    .with_min_inner_size(LogicalSize::new(720.0, 540.0))
                    .with_resizable(true)
                    .with_visible(false),
            )
            .context("failed to create the detached agent window")?;
        if let Some(monitor) = monitor.as_ref() {
            center_window_on_monitor(&detached_window, monitor);
        }
        apply_native_window_theme(&detached_window, &self.theme);

        let panel = self
            .agent_panel
            .as_ref()
            .ok_or_else(|| anyhow!("agent panel is missing"))?;
        reparent_webview(panel, &detached_window).context("failed to detach the agent panel")?;
        self.agent_panel_window = Some(detached_window);
        self.browser_panel_minimized = false;
        self.agent_panel_visible = true;
        self.agent_panel_close_pending = false;
        self.layout_webviews();
        self.update_tab_webview_visibility();
        if let Some(window) = self.agent_panel_window.as_ref() {
            window.set_visible(true);
            window.focus_window();
        }
        self.render_toolbar();
        self.render_agent_panel();
        self.save_session();
        Ok(())
    }

    fn attach_agent_panel(&mut self) -> anyhow::Result<()> {
        if self.agent_panel_window.is_none() {
            self.agent_panel_visible = true;
            self.layout_webviews();
            self.render_toolbar();
            self.render_agent_panel();
            return Ok(());
        }

        {
            let main_window = self
                .window
                .as_ref()
                .ok_or_else(|| anyhow!("main window is missing"))?;
            let panel = self
                .agent_panel
                .as_ref()
                .ok_or_else(|| anyhow!("agent panel is missing"))?;
            reparent_webview(panel, main_window).context("failed to reattach the agent panel")?;
        }
        self.agent_panel_window.take();
        self.agent_panel_visible = true;
        self.agent_panel_close_pending = false;
        self.layout_webviews();
        self.update_tab_webview_visibility();
        if let Some(main_window) = self.window.as_ref() {
            main_window.set_minimized(false);
            main_window.focus_window();
        }
        self.render_toolbar();
        self.render_agent_panel();
        self.save_session();
        Ok(())
    }

    fn point_is_outside_main_window(&self, screen_x: i32, screen_y: i32) -> bool {
        let Some(window) = self.window.as_ref() else {
            return true;
        };
        let Ok(position) = window.outer_position() else {
            return true;
        };
        let size = window.outer_size();
        let scale = window.scale_factor();
        let x = (f64::from(screen_x) * scale).round() as i32;
        let y = (f64::from(screen_y) * scale).round() as i32;
        x < position.x
            || y < position.y
            || x >= position
                .x
                .saturating_add(i32::try_from(size.width).unwrap_or(i32::MAX))
            || y >= position
                .y
                .saturating_add(i32::try_from(size.height).unwrap_or(i32::MAX))
    }

    fn toggle_terminal_panel(&mut self) {
        if let Some(window) = self.terminal_window.as_ref() {
            window.set_minimized(false);
            window.focus_window();
            return;
        }
        if self.terminal_panel_visible {
            self.set_terminal_panel_visible(false);
            return;
        }
        if !self.terminal.is_running() {
            let callback = self.terminal_callback();
            if let Err(error) = self.terminal.open(callback) {
                self.push_chat_message(ChatRole::System, error, Vec::new());
            }
        }
        self.set_terminal_panel_visible(true);
    }

    fn set_terminal_panel_visible(&mut self, visible: bool) {
        if !visible && self.terminal_window.is_some() {
            if let Err(error) = self.attach_terminal_panel(false) {
                warn!(%error, "terminal could not be hidden from its detached window");
            }
            return;
        }
        self.terminal_panel_visible = visible;
        self.layout_webviews();
        self.render_toolbar();
        self.render_agent_panel();
    }

    fn set_terminal_dock(&mut self, dock: TerminalDock) {
        if self.terminal_dock == dock {
            return;
        }
        self.terminal_dock = dock;
        self.layout_webviews();
        self.render_toolbar();
        self.render_agent_panel();
        self.save_session();
    }

    fn detach_terminal_panel(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        if let Some(window) = self.terminal_window.as_ref() {
            window.set_minimized(false);
            window.focus_window();
            return Ok(());
        }
        if !self.terminal.is_running() {
            self.open_terminal();
        }

        let monitor = self
            .window
            .as_ref()
            .and_then(Window::current_monitor)
            .or_else(|| event_loop.primary_monitor());
        let detached_window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Supervisor — Terminal")
                    .with_window_icon(Some(brand::window_icon()))
                    .with_inner_size(LogicalSize::new(
                        DETACHED_TERMINAL_WIDTH_LOGICAL,
                        DETACHED_TERMINAL_HEIGHT_LOGICAL,
                    ))
                    .with_min_inner_size(LogicalSize::new(640.0, 420.0))
                    .with_resizable(true)
                    .with_visible(false),
            )
            .context("failed to create the detached terminal window")?;
        if let Some(monitor) = monitor.as_ref() {
            center_window_on_monitor(&detached_window, monitor);
        }
        apply_native_window_theme(&detached_window, &self.theme);

        let panel = self
            .terminal_panel
            .as_ref()
            .ok_or_else(|| anyhow!("terminal panel is missing"))?;
        reparent_webview(panel, &detached_window).context("failed to detach the terminal panel")?;
        self.terminal_window = Some(detached_window);
        self.terminal_panel_visible = true;
        self.layout_webviews();
        if let Some(window) = self.terminal_window.as_ref() {
            window.set_visible(true);
            window.focus_window();
        }
        self.render_toolbar();
        self.render_agent_panel();
        Ok(())
    }

    fn attach_terminal_panel(&mut self, visible: bool) -> anyhow::Result<()> {
        if self.terminal_window.is_none() {
            self.terminal_panel_visible = visible;
            self.layout_webviews();
            self.render_toolbar();
            self.render_agent_panel();
            return Ok(());
        }

        {
            let main_window = self
                .window
                .as_ref()
                .ok_or_else(|| anyhow!("main window is missing"))?;
            let panel = self
                .terminal_panel
                .as_ref()
                .ok_or_else(|| anyhow!("terminal panel is missing"))?;
            reparent_webview(panel, main_window)
                .context("failed to reattach the terminal panel")?;
        }
        self.terminal_window.take();
        self.terminal_panel_visible = visible;
        self.layout_webviews();
        if visible && let Some(main_window) = self.window.as_ref() {
            main_window.set_minimized(false);
            main_window.focus_window();
        }
        self.render_toolbar();
        self.render_agent_panel();
        Ok(())
    }

    fn detach_preview(&mut self, event_loop: &ActiveEventLoop) -> anyhow::Result<()> {
        if let Some(window) = self.preview_window.as_ref() {
            window.set_minimized(false);
            window.focus_window();
            return Ok(());
        }
        if !self.preview.visible || (self.preview.image.is_none() && !self.preview.capturing) {
            return Err(anyhow!("open an application preview before detaching it"));
        }

        let monitor = self
            .window
            .as_ref()
            .and_then(Window::current_monitor)
            .or_else(|| event_loop.primary_monitor());
        let detached_window = event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Supervisor")
                    .with_window_icon(Some(brand::window_icon()))
                    .with_inner_size(LogicalSize::new(
                        DETACHED_PREVIEW_WIDTH_LOGICAL,
                        DETACHED_PREVIEW_HEIGHT_LOGICAL,
                    ))
                    .with_min_inner_size(LogicalSize::new(520.0, 360.0))
                    .with_resizable(true)
                    .with_visible(false),
            )
            .context("failed to create the detached preview window")?;
        if let Some(monitor) = monitor.as_ref() {
            center_window_on_monitor(&detached_window, monitor);
        }
        apply_native_window_theme(&detached_window, &self.theme);

        let ipc_proxy = self.proxy.clone();
        let ready_proxy = self.proxy.clone();
        let background = theme_surface_color(&self.theme);
        let context = self
            .ui_context
            .as_mut()
            .ok_or_else(|| anyhow!("UI context is missing"))?;
        let panel = WebViewBuilder::new_with_web_context(context)
            .with_html(themed_ui_asset_html(
                self.ui_development_root.as_deref(),
                "assets/preview.html",
                PREVIEW_HTML,
                &self.theme,
            ))
            .with_bounds(full_window_bounds(&detached_window))
            .with_background_color(background)
            .with_visible(true)
            .with_devtools(cfg!(debug_assertions))
            .with_ipc_handler(move |request| {
                match serde_json::from_str::<AgentPanelMessage>(request.body()) {
                    Ok(message) => {
                        let _ = ipc_proxy.send_event(BrowserEvent::AgentPanel(message));
                    }
                    Err(error) => warn!(%error, "preview panel command rejected"),
                }
            })
            .with_on_page_load_handler(move |event, _| {
                if matches!(event, PageLoadEvent::Finished) {
                    let _ = ready_proxy.send_event(BrowserEvent::PreviewPanelReady);
                }
            })
            .build_as_child(&detached_window)
            .context("failed to create the detached preview surface")?;

        self.preview_panel_ready = false;
        self.preview_panel = Some(panel);
        self.preview_window = Some(detached_window);
        if let Some(window) = self.preview_window.as_ref() {
            window.set_visible(true);
            window.focus_window();
        }
        self.render_agent_panel();
        Ok(())
    }

    fn attach_preview(&mut self) {
        self.preview_panel.take();
        self.preview_window.take();
        self.preview_panel_ready = false;
        self.render_agent_panel();
        self.restream_preview_image();
    }

    fn destroy_preview_surface(&mut self) {
        self.preview_panel.take();
        self.preview_window.take();
        self.preview_panel_ready = false;
    }

    fn handle_agent_panel_message(
        &mut self,
        event_loop: &ActiveEventLoop,
        message: AgentPanelMessage,
    ) {
        match message {
            AgentPanelMessage::ProjectDiff {
                owner,
                view_id,
                request_id,
                action,
            } => self.request_project_diff(owner, view_id, request_id, action),

            AgentPanelMessage::CloseSettings => self.close_settings(),
            AgentPanelMessage::SettingsSurfaceReady => self.settings_surface_ready(),
            AgentPanelMessage::SettingsCoverReady => self.settings_cover_ready(),
            AgentPanelMessage::SettingsCloseLayoutReady => self.settings_close_layout_ready(),
            AgentPanelMessage::OpenAgentGraph => self.open_agent_graph(),
            AgentPanelMessage::OpenProjectRegistry => {
                self.project_registry_add_request_id =
                    self.project_registry_add_request_id.wrapping_add(1).max(1);
                if self.settings_open {
                    self.close_settings();
                }
                self.open_agent_graph();
                self.render_agent_graph_surface();
            }
            AgentPanelMessage::CloseAgentGraph => self.close_agent_graph(),
            AgentPanelMessage::BeginAgentGraphLauncherDrag { screen_x, screen_y } => {
                self.begin_agent_graph_launcher_drag(screen_x, screen_y)
            }
            AgentPanelMessage::AgentGraphLauncherDragSurfaceReady => {
                self.agent_graph_launcher_drag_surface_ready()
            }
            AgentPanelMessage::EndAgentGraphLauncherDrag {
                screen_x,
                screen_y,
                open_graph,
            } => self.end_agent_graph_launcher_drag(screen_x, screen_y, open_graph),
            AgentPanelMessage::Execute { request } => self.submit_agent_command(request.into()),

            AgentPanelMessage::SubmitChat {
                message,
                resume,
                timing,
                delivery,
                tab_ids,
                terminal_session_ids,
                file_ids,
            } => self.submit_chat_message(
                ChatSubmissionPrompt { message, resume },
                delivery,
                tab_ids,
                terminal_session_ids,
                file_ids,
                timing,
            ),
            AgentPanelMessage::CancelPendingAgentSubmission => {
                self.cancel_pending_agent_submission()
            }
            AgentPanelMessage::ClearAgentSubmissionQueue => self.clear_agent_submission_queue(),
            AgentPanelMessage::DiscardFailedCheckpoint { run_id } => {
                self.discard_failed_checkpoint(run_id)
            }
            AgentPanelMessage::SelectChatFiles => self.select_chat_files(),
            AgentPanelMessage::RemoveChatFile { file_id } => self.remove_chat_file(&file_id),
            AgentPanelMessage::OpenArtifact { artifact_id } => self.open_artifact(&artifact_id),
            AgentPanelMessage::SaveArtifact { artifact_id } => self.save_artifact(&artifact_id),
            AgentPanelMessage::ShowArtifactInFolder { artifact_id } => {
                self.show_artifact_in_folder(&artifact_id)
            }
            AgentPanelMessage::CaptureTabContext { tab_id } => {
                self.capture_tab_context(tab_id, false)
            }
            AgentPanelMessage::RefreshTabContext { tab_id } => {
                self.capture_tab_context(tab_id, true)
            }
            AgentPanelMessage::RemoveTabContext { tab_id } => self.remove_tab_context(tab_id),
            AgentPanelMessage::CaptureTerminalContext { session_id } => {
                self.capture_terminal_context(session_id, false)
            }
            AgentPanelMessage::RefreshTerminalContext { session_id } => {
                self.capture_terminal_context(session_id, true)
            }
            AgentPanelMessage::RemoveTerminalContext { session_id } => {
                self.remove_terminal_context(session_id)
            }
            AgentPanelMessage::SetTerminalContextFollow {
                session_id,
                enabled,
            } => self.set_terminal_context_follow(session_id, enabled),

            AgentPanelMessage::RetryClaudeProvider => self.start_claude_probe(),
            AgentPanelMessage::AppServerAccount { action } => self.app_server_account(action),
            AgentPanelMessage::AppServerMcp { action } => self.app_server_mcp(action),
            AgentPanelMessage::AppServerP2 { action } => self.app_server_p2(action),
            AgentPanelMessage::AppServerHistory { owner, action } => {
                self.app_server_history(owner, action)
            }
            AgentPanelMessage::AppServerSkills { owner, action } => {
                self.app_server_skills(owner, action)
            }
            AgentPanelMessage::AppServerPreferences { owner, action } => {
                self.app_server_preferences(owner, action)
            }
            AgentPanelMessage::AppServerAccess { owner, action } => {
                self.app_server_access(owner, action)
            }
            AgentPanelMessage::AppServerSteer { owner, input } => {
                self.app_server_steer(owner, input)
            }
            AgentPanelMessage::AppServerPrepare { owner } => self.select_native_preparation(owner),
            AgentPanelMessage::AppServerConversation { owner, action } => {
                self.app_server_conversation(owner, action)
            }
            AgentPanelMessage::ComposerDraft {
                owner,
                request_id,
                action,
            } => self.composer_draft(&owner, &request_id, action),
            AgentPanelMessage::AppServerRequest { owner, action } => {
                self.app_server_request(owner, action)
            }
            AgentPanelMessage::ConnectClaudeProvider => self.connect_claude_provider(),
            AgentPanelMessage::ResetClaudeSession => self.reset_claude_session(),
            AgentPanelMessage::RetrySubscriptionProvider { provider } => {
                self.start_subscription_probe(provider)
            }
            AgentPanelMessage::ConnectSubscriptionProvider { provider } => {
                self.connect_subscription_provider(provider)
            }
            AgentPanelMessage::SetAgentProvider { provider } => self.set_agent_provider(provider),
            AgentPanelMessage::RetrySshClient => {
                self.ssh.refresh_client();
                self.ssh.set_message("OpenSSH detection refreshed.", false);
                self.render_agent_panel();
            }
            AgentPanelMessage::SaveSshProfile {
                id,
                name,
                host,
                port,
                username,
                identity_file,
                agent_enabled,
            } => self.save_ssh_profile(SshProfileInput {
                id,
                name,
                host,
                port,
                username,
                identity_file,
                agent_enabled,
            }),
            AgentPanelMessage::DeleteSshProfile { profile_id } => {
                self.delete_ssh_profile(&profile_id)
            }
            AgentPanelMessage::OpenSshTerminal { profile_id } => {
                self.open_ssh_terminal(&profile_id)
            }
            AgentPanelMessage::OpenRemoteDesktop {
                profile_id,
                protocol,
                remote_port,
            } => self.open_remote_desktop(&profile_id, protocol, remote_port),
            AgentPanelMessage::SetSshSessionAgentReady { session_id, ready } => {
                self.set_ssh_session_agent_ready(session_id, ready)
            }

            AgentPanelMessage::SetAgentConfiguration {
                model,
                effort,
                service_tier,
                context_window,
                personality,
            } => self.set_agent_configuration(AgentSelection {
                model,
                effort,
                service_tier,
                context_window,
                personality,
            }),
            AgentPanelMessage::OpenTerminal => self.open_terminal(),
            AgentPanelMessage::CloseTerminal { session_id } => self.close_terminal(session_id),
            AgentPanelMessage::ActivateTerminal { session_id } => {
                if self.terminal.activate(session_id) {
                    self.render_agent_panel();
                }
            }
            AgentPanelMessage::TerminalWrite { session_id, data } => {
                self.write_terminal(session_id, data)
            }
            AgentPanelMessage::ResizeTerminal {
                session_id,
                rows,
                cols,
            } => {
                if let Err(error) = self.terminal.resize(session_id, rows, cols) {
                    warn!(%error, "terminal resize failed");
                }
            }
            AgentPanelMessage::ReorderTerminal {
                session_id,
                before_session_id,
            } => {
                if self.terminal.reorder(session_id, before_session_id) {
                    self.render_agent_panel();
                }
            }
            AgentPanelMessage::SetTerminalDock { dock } => self.set_terminal_dock(dock),
            AgentPanelMessage::SetTerminalPanelVisible { visible } => {
                self.set_terminal_panel_visible(visible)
            }
            AgentPanelMessage::SetAgentPanelWidth { width, persist } => {
                self.set_agent_panel_width(width, persist)
            }
            AgentPanelMessage::DetachAgentPanel => {
                if let Err(error) = self.detach_agent_panel(event_loop) {
                    warn!(%error, "agent panel could not be opened in a separate window");
                }
            }
            AgentPanelMessage::AttachAgentPanel => {
                if let Err(error) = self.attach_agent_panel() {
                    warn!(%error, "agent panel could not be returned to the main window");
                }
            }
            AgentPanelMessage::DetachTerminalPanel { screen_x, screen_y } => {
                if screen_x
                    .zip(screen_y)
                    .is_some_and(|(x, y)| !self.point_is_outside_main_window(x, y))
                {
                    return;
                }
                if let Err(error) = self.detach_terminal_panel(event_loop) {
                    warn!(%error, "terminal could not be opened in a separate window");
                }
            }
            AgentPanelMessage::AttachTerminalPanel => {
                if let Err(error) = self.attach_terminal_panel(true) {
                    warn!(%error, "terminal could not be returned to the main window");
                }
            }
            AgentPanelMessage::SetTheme { theme } => self.set_theme(&theme),
            AgentPanelMessage::SetSearchEngine { search_engine } => {
                self.set_search_engine(&search_engine)
            }
            AgentPanelMessage::SetAuditRetention { days } => self.set_audit_retention(days),
            AgentPanelMessage::SelectWorkspace => self.select_workspace(),
            AgentPanelMessage::CreateLocalProject { name } => self.create_project_folder(&name),
            AgentPanelMessage::ProjectBoard { action } => self.handle_project_board(action),
            AgentPanelMessage::CloneRepository { repository } => {
                self.select_clone_destination(&repository)
            }
            AgentPanelMessage::ConnectSshProject {
                profile_id,
                directory,
                name,
            } => self.connect_ssh_project(&profile_id, &directory, name.as_deref()),
            AgentPanelMessage::SetRemoteProjectPinned { id, pinned } => {
                self.set_remote_project_pinned(&id, pinned)
            }
            AgentPanelMessage::RemoveRemoteProject { id } => self.remove_remote_project(&id),
            AgentPanelMessage::OpenProject { source, id } => self.open_project(&source, &id),
            AgentPanelMessage::OpenProjectTerminal { source, id, git } => {
                self.open_project_terminal(event_loop, &source, &id, git)
            }
            AgentPanelMessage::ShowProjectFolder { root } => self.show_project_folder(&root),
            AgentPanelMessage::RefreshProjectMetadata { source, id } => {
                self.refresh_project_metadata(&source, &id)
            }
            AgentPanelMessage::SetReopenLastProject { enabled } => {
                self.set_reopen_last_project(enabled)
            }
            AgentPanelMessage::ActivateWorkspace { root } => self.activate_workspace(&root),
            AgentPanelMessage::CreateProjectChat { root } => self.create_project_chat(&root),
            AgentPanelMessage::RenameWorkspace { root, name } => {
                self.rename_workspace(&root, &name)
            }
            AgentPanelMessage::SetWorkspacePinned { root, pinned } => {
                self.set_workspace_pinned(&root, pinned)
            }
            AgentPanelMessage::ActivateProjectChat { chat_id } => {
                self.activate_project_chat(&chat_id)
            }
            AgentPanelMessage::RenameProjectChat { chat_id, title } => {
                self.rename_project_chat(&chat_id, &title)
            }
            AgentPanelMessage::MoveProjectChat {
                chat_id,
                project_root,
            } => self.move_project_chat(&chat_id, &project_root),
            AgentPanelMessage::SelectProjectForChat { chat_id } => {
                self.select_project_for_chat(&chat_id)
            }
            AgentPanelMessage::SetProjectChatPinned { chat_id, pinned } => {
                self.set_project_chat_pinned(&chat_id, pinned)
            }
            AgentPanelMessage::SetProjectChatArchived { chat_id, archived } => {
                self.set_project_chat_archived(&chat_id, archived)
            }
            AgentPanelMessage::SetProjectChatsArchived { root, archived } => {
                self.set_project_chats_archived(&root, archived)
            }
            AgentPanelMessage::DeleteProjectChat { chat_id } => self.delete_project_chat(&chat_id),
            AgentPanelMessage::SetWorkspaceArchived { root, archived } => {
                self.set_workspace_archived(&root, archived)
            }
            AgentPanelMessage::EjectWorkspace {
                root,
                stop_active_runs,
            } => self.eject_workspace(&root, stop_active_runs),
            AgentPanelMessage::ClearWorkspace => self.clear_workspace(),
            AgentPanelMessage::RefreshWorkspaceExplorer => {
                self.workspace.refresh_explorer();
                self.render_agent_panel();
            }
            AgentPanelMessage::CreateWorkspaceFile { path } => self.create_workspace_file(&path),
            AgentPanelMessage::Editor { action } => self.handle_editor_action(action),
            AgentPanelMessage::CreateWorkspaceDirectory { path } => {
                self.create_workspace_directory(&path)
            }
            AgentPanelMessage::ImportWorkspaceFiles { directory } => {
                self.import_workspace_files(&directory)
            }
            AgentPanelMessage::CancelManagedProcess { process_id } => {
                if let Err(error) = self.processes.cancel(process_id) {
                    self.push_chat_message(ChatRole::System, error, Vec::new());
                }
                self.render_agent_panel();
            }
            AgentPanelMessage::OpenProcessPreview { process_id, url } => {
                match self.processes.validated_preview_url(process_id, &url) {
                    Ok(url) => match self.create_tab(Some(url.clone()), true) {
                        Ok(tab_id) => {
                            if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) {
                                tab.web_preview = Some(WebPreviewMetadata {
                                    owner_process_id: process_id,
                                    origin: url_origin(&url).unwrap_or(url),
                                    process_running: true,
                                });
                            }
                            self.render_agent_panel();
                        }
                        Err(error) => {
                            self.push_chat_message(
                                ChatRole::System,
                                format!("Could not open the local web preview: {error}"),
                                Vec::new(),
                            );
                            self.render_agent_panel();
                        }
                    },
                    Err(message) => {
                        self.push_chat_message(ChatRole::System, message, Vec::new());
                        self.render_agent_panel();
                    }
                }
            }
            AgentPanelMessage::SetWindowAccess { enabled } => self.set_window_access(enabled),
            AgentPanelMessage::RefreshWindows => self.refresh_windows_for_user(),
            AgentPanelMessage::CaptureWindow { window_id } => {
                self.capture_window_for_user(window_id)
            }
            AgentPanelMessage::SetPreviewLive { enabled } => self.set_preview_live(enabled),
            AgentPanelMessage::DetachPreview => {
                if let Err(error) = self.detach_preview(event_loop) {
                    self.push_chat_message(
                        ChatRole::System,
                        format!("Could not detach the preview: {error}"),
                        Vec::new(),
                    );
                    self.render_agent_panel();
                }
            }
            AgentPanelMessage::AttachPreview => self.attach_preview(),
            AgentPanelMessage::ClosePreview => self.close_preview(),
            AgentPanelMessage::ResumeComputerControl => self.resume_computer_control(),
            AgentPanelMessage::OpenAgentGraphNode { record_type, id } => {
                if let Err(message) = self.expand_agent_graph_node(&record_type, &id) {
                    warn!(%message, record_type, id, "Agent Graph folder could not be expanded");
                }
                self.render_agent_graph_surface();
            }
            AgentPanelMessage::SetAgentGraphAgent {
                record_type,
                id,
                conversation_id,
                name,
                mission,
                provider,
                model,
                effort,
                service_tier,
                context_window,
            } => self.set_agent_graph_agent(AgentGraphAssignment {
                record_type,
                record_id: id,
                conversation_id,
                name,
                mission,
                provider,
                model,
                effort,
                service_tier,
                context_window,
            }),
            AgentPanelMessage::SetAgentGraphLink {
                source_record_type,
                source_id,
                source_conversation_id,
                target_record_type,
                target_id,
                target_conversation_id,
                linked,
            } => self.set_agent_graph_link(
                (&source_record_type, &source_id, source_conversation_id),
                (&target_record_type, &target_id, target_conversation_id),
                linked,
            ),
            AgentPanelMessage::RemoveAgentGraphAgent {
                record_type,
                id,
                conversation_id,
            } => self.remove_agent_graph_agent(&record_type, &id, conversation_id),
            AgentPanelMessage::RunAgentGraphAgent {
                record_type,
                id,
                conversation_id,
            } => self.run_agent_graph_agent(&record_type, &id, conversation_id),
            AgentPanelMessage::ContinueAgentGraphAgent {
                record_type,
                id,
                conversation_id,
                message,
                timing,
                delivery,
                file_ids,
                tab_ids,
                terminal_session_ids,
            } => self.continue_agent_graph_agent(AgentGraphContinuation {
                record_type,
                id,
                conversation_id,
                message,
                delivery,
                file_ids,
                tab_ids,
                terminal_session_ids,
                timing,
            }),
            AgentPanelMessage::GraphContexts { owner, action } => {
                self.manage_graph_contexts(&owner, action)
            }
            AgentPanelMessage::GraphFiles { owner, action } => {
                self.manage_graph_files(&owner, action)
            }
            AgentPanelMessage::ClearAgentGraphQueue {
                record_type,
                id,
                conversation_id,
            } => {
                self.clear_agent_graph_queue(&record_type, &id, conversation_id);
            }
            AgentPanelMessage::LoadAgentGraphHistory {
                record_type,
                id,
                conversation_id,
                before,
            } => {
                let node = agent_graph_conversation_key(&record_type, &id, conversation_id);
                if self.agent_graph_sessions.iter().any(|session| {
                    session.node_key == node
                        && session.record_type == record_type
                        && session.record_id == id
                        && before <= session.turns.len()
                }) {
                    let offset = before.saturating_sub(AGENT_GRAPH_HISTORY_PAGE_SIZE);
                    self.agent_graph_history_offsets
                        .entry(node)
                        .and_modify(|current| *current = (*current).min(offset))
                        .or_insert(offset);
                    self.render_agent_graph_surface();
                }
            }
            AgentPanelMessage::StopAgentGraphAgent {
                record_type,
                id,
                conversation_id,
            } => self.stop_agent_graph_agent(&record_type, &id, conversation_id),
            AgentPanelMessage::OpenCheckpoint {
                checkpoint_id,
                path,
            } => {
                if let Ok(mut time_machine) = self.time_machine.lock()
                    && let Err(message) =
                        time_machine.open_checkpoint(&checkpoint_id, path.as_deref())
                {
                    time_machine.set_error(message);
                }
                self.render_agent_panel();
            }
            AgentPanelMessage::SelectCheckpointFile {
                checkpoint_id,
                path,
            } => {
                if let Ok(mut time_machine) = self.time_machine.lock()
                    && let Err(message) = time_machine.select_checkpoint_file(&checkpoint_id, &path)
                {
                    time_machine.set_error(message);
                }
                self.render_agent_panel();
            }
            AgentPanelMessage::CloseCheckpoint => {
                if let Ok(mut time_machine) = self.time_machine.lock() {
                    time_machine.close_checkpoint();
                }
                self.render_agent_panel();
            }
            AgentPanelMessage::RestoreCheckpoint { checkpoint_id } => {
                self.restore_time_machine_checkpoint(&checkpoint_id, None)
            }
            AgentPanelMessage::RestoreCheckpointFile {
                checkpoint_id,
                path,
            } => self.restore_time_machine_checkpoint(&checkpoint_id, Some(&path)),
            AgentPanelMessage::StopAgent => self.stop_agent_run(),
            AgentPanelMessage::SetPermissionMode { mode } => self.set_permission_mode(mode),
            AgentPanelMessage::AuthorizeSession => self.authorize_session(),
            AgentPanelMessage::ApproveAction { request_id } => {
                self.approve_pending_action(&request_id)
            }
            AgentPanelMessage::DenyAction { request_id } => {
                self.deny_pending_action(&request_id, "Action denied by the user")
            }
        }
    }

    fn terminal_callback(&self) -> impl Fn(TerminalEvent) + Send + Sync + 'static {
        let proxy = self.proxy.clone();
        move |event| {
            let _ = proxy.send_event(BrowserEvent::Terminal(event));
        }
    }

    fn managed_process_callback(&self) -> impl Fn(ManagedProcessEvent) + Send + Sync + 'static {
        let proxy = self.proxy.clone();
        move |event| {
            let _ = proxy.send_event(BrowserEvent::ManagedProcess(event));
        }
    }

    fn open_terminal(&mut self) {
        let callback = self.terminal_callback();
        if let Err(error) = self.terminal.open(callback) {
            self.push_chat_message(ChatRole::System, error, Vec::new());
        }
        self.terminal_panel_visible = true;
        self.layout_webviews();
        self.render_toolbar();
        self.render_agent_panel();
        if let Some(window) = self.terminal_window.as_ref() {
            window.set_minimized(false);
            window.focus_window();
        }
    }

    fn ssh_configuration_locked(&self) -> bool {
        !self.pending_commands.is_empty()
            || self.any_agent_active()
            || self.terminal.is_busy()
            || self.remote_desktop.is_active()
    }

    fn save_ssh_profile(&mut self, input: SshProfileInput) {
        if self.ssh_configuration_locked() {
            self.ssh.set_message(
                "Stop the active agent, terminal command, or Linux desktop before changing server profiles.",
                true,
            );
            self.render_agent_panel();
            return;
        }
        if input
            .id
            .as_deref()
            .is_some_and(|profile_id| self.terminal.has_ssh_profile_session(profile_id))
        {
            self.ssh
                .set_message("Close this profile's SSH sessions before editing it.", true);
            self.render_agent_panel();
            return;
        }

        match self.ssh.upsert(input) {
            Ok(profile_id) => {
                self.ssh.set_message("Server profile saved locally.", false);
                self.audit.record(
                    "ssh_profile_saved",
                    "",
                    "ssh_profile",
                    &profile_id,
                    "local_configuration",
                    "saved",
                    "VPS profile metadata was saved by the trusted local UI",
                );
                self.save_session();
            }
            Err(message) => self.ssh.set_message(message, true),
        }
        self.render_agent_panel();
    }

    fn delete_ssh_profile(&mut self, profile_id: &str) {
        if self.ssh_configuration_locked() {
            self.ssh.set_message(
                "Stop the active agent, terminal command, or Linux desktop before deleting a server profile.",
                true,
            );
        } else if self.terminal.has_ssh_profile_session(profile_id) {
            self.ssh.set_message(
                "Close this profile's SSH sessions before deleting it.",
                true,
            );
        } else {
            match self.ssh.remove(profile_id) {
                Ok(profile) => {
                    self.remote_desktop.clear_profile(&profile.id);
                    let removed_ids = self
                        .remote_projects
                        .iter()
                        .filter(|project| project.ssh_profile_id == profile.id)
                        .map(|project| project.id.clone())
                        .collect::<Vec<_>>();
                    self.remote_projects
                        .retain(|project| project.ssh_profile_id != profile.id);
                    for id in removed_ids {
                        let key = remote_project_metadata_key(&id);
                        self.project_metadata.remove(&key);
                        self.project_scans_in_flight.remove(&key);
                        self.agent_graph_bindings
                            .retain(|binding| binding.record_id != id);
                    }
                    self.permission_policy.revoke_scope(CapabilityScope::Ssh);
                    self.ssh
                        .set_message(format!("Deleted server profile {}.", profile.name), false);
                    self.audit.record(
                        "ssh_profile_deleted",
                        "",
                        "ssh_profile",
                        &profile.id,
                        "local_configuration",
                        "deleted",
                        "VPS profile metadata was deleted by the trusted local UI",
                    );
                    self.save_session();
                }
                Err(message) => self.ssh.set_message(message, true),
            }
        }
        self.render_agent_panel();
    }

    fn open_ssh_terminal(&mut self, profile_id: &str) {
        if self.any_agent_active() || !self.pending_commands.is_empty() {
            self.ssh.set_message(
                "Stop the active agent before opening a new SSH session.",
                true,
            );
            self.render_agent_panel();
            return;
        }
        let launch = match self.ssh.launch_spec(profile_id) {
            Ok(launch) => launch,
            Err(message) => {
                self.ssh.set_message(message, true);
                self.render_agent_panel();
                return;
            }
        };
        let profile_name = launch.profile_name.clone();
        let callback = self.terminal_callback();
        match self.terminal.open_ssh(launch, callback) {
            Ok(session_id) => {
                self.ssh.set_message(
                    format!(
                        "Opened {profile_name}. Complete host verification and authentication in the visible terminal."
                    ),
                    false,
                );
                self.audit.record(
                    "ssh_session_opened",
                    "",
                    "terminal_session",
                    &session_id.to_string(),
                    "ssh",
                    "opened",
                    "Interactive SSH session opened by the trusted local UI",
                );
                self.terminal_panel_visible = true;
                self.layout_webviews();
                self.render_toolbar();
            }
            Err(message) => self.ssh.set_message(message, true),
        }
        self.render_agent_panel();
        if let Some(window) = self.terminal_window.as_ref() {
            window.set_minimized(false);
            window.focus_window();
        }
    }

    fn set_ssh_session_agent_ready(&mut self, session_id: u64, ready: bool) {
        if self.any_agent_active() || !self.pending_commands.is_empty() {
            self.ssh.set_message(
                "Stop the active agent before changing remote-session access.",
                true,
            );
            self.render_agent_panel();
            return;
        }
        match self.terminal.set_ssh_agent_ready(session_id, ready) {
            Ok(()) => {
                if !ready && !self.terminal.has_agent_ready_ssh_session() {
                    self.permission_policy.revoke_scope(CapabilityScope::Ssh);
                }
                self.audit.record(
                    "ssh_agent_access_changed",
                    "",
                    "terminal_session",
                    &session_id.to_string(),
                    "ssh",
                    if ready { "enabled" } else { "disabled" },
                    "Remote Agent access changed by the trusted local UI",
                );
                self.ssh.set_message(
                    if ready {
                        "Agent can now request commands in this authenticated SSH session."
                    } else {
                        "Agent access was disabled for this SSH session."
                    },
                    false,
                );
                if ready {
                    self.start_ssh_runtime_discovery(session_id);
                }
            }
            Err(message) => self.ssh.set_message(message, true),
        }
        self.render_agent_panel();
    }

    fn start_ssh_runtime_discovery(&mut self, session_id: u64) {
        let request_id = format!("ssh-runtime-discovery-{}", Uuid::new_v4().simple());
        match self
            .terminal
            .start_ssh_runtime_discovery(session_id, request_id.clone())
        {
            Ok(true) => {
                self.audit.record(
                    "ssh_runtime_discovery_started",
                    &request_id,
                    "terminal_session",
                    &session_id.to_string(),
                    "ssh",
                    "started",
                    "The Rust runtime started fixed read-only SSH capability discovery",
                );
            }
            Ok(false) => {}
            Err(message) => {
                warn!(%message, session_id, "local SSH capability discovery could not start");
                self.audit.record(
                    "ssh_runtime_discovery_deferred",
                    &request_id,
                    "terminal_session",
                    &session_id.to_string(),
                    "ssh",
                    "deferred",
                    "Local SSH discovery will retry transparently with the first remote command",
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn set_window_access(&mut self, enabled: bool) {
        if self.any_agent_active() || !self.pending_commands.is_empty() {
            self.push_chat_message(
                ChatRole::System,
                "Stop the active run before changing window access.".to_owned(),
                Vec::new(),
            );
            self.render_agent_panel();
            return;
        }
        self.window_access_enabled = cfg!(windows) && enabled;
        self.windows.clear();
        if !self.window_access_enabled {
            self.destroy_preview_surface();
            self.preview.close();
            self.provider_tool_images.clear();
            self.ui_automation
                .clear("Semantic computer control was disconnected.");
            self.permission_policy.revoke_scope(CapabilityScope::Window);
            self.permission_policy
                .revoke_scope(CapabilityScope::Capture);
            self.permission_policy
                .revoke_scope(CapabilityScope::UiAutomation);
        }
        self.save_session();
        self.render_agent_panel();
    }

    fn capture_window_for_user(&mut self, window_id: String) {
        if !self.window_access_enabled {
            self.push_chat_message(
                ChatRole::System,
                "Enable window awareness in Settings before capturing an application window."
                    .to_owned(),
                Vec::new(),
            );
            self.render_agent_panel();
            return;
        }
        if self.safety.paused() {
            self.push_chat_message(
                ChatRole::System,
                "Computer control is paused. Resume it in Settings before capturing a window."
                    .to_owned(),
                Vec::new(),
            );
            self.render_agent_panel();
            return;
        }
        if self.any_agent_active() || !self.pending_commands.is_empty() {
            self.push_chat_message(
                ChatRole::System,
                "Stop the active run before starting a manual window preview.".to_owned(),
                Vec::new(),
            );
            self.render_agent_panel();
            return;
        }
        if self.preview.capturing {
            self.push_chat_message(
                ChatRole::System,
                "A window capture is already in progress.".to_owned(),
                Vec::new(),
            );
            self.render_agent_panel();
            return;
        }
        match self.windows.take_capture_target(&window_id) {
            Ok(target) => self.start_window_capture(None, "capture_window".to_owned(), target),
            Err(message) => {
                self.preview.status = message.clone();
                self.push_chat_message(ChatRole::System, message, Vec::new());
                self.render_agent_panel();
            }
        }
    }

    fn start_window_capture(
        &mut self,
        request_id: Option<String>,
        action: String,
        target: crate::window_runtime::WindowCaptureTarget,
    ) {
        let owner_process_id = self.processes.process_id_for_pid(target.pid);
        let source_type = if owner_process_id
            .is_some_and(|process_id| self.processes.process_looks_like_game(process_id))
        {
            NativePreviewSource::GameWindow
        } else {
            NativePreviewSource::NativeWindow
        };
        let generation = self.preview.begin(
            target.clone(),
            owner_process_id,
            source_type,
            request_id.is_none(),
        );
        self.render_agent_panel();
        self.spawn_window_capture(request_id, action, target, generation);
    }

    fn spawn_window_capture(
        &self,
        request_id: Option<String>,
        action: String,
        target: crate::window_runtime::WindowCaptureTarget,
        generation: u64,
    ) {
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = capture_runtime::capture_window(target);
            let _ = proxy.send_event(BrowserEvent::WindowCapture {
                request_id,
                action,
                generation,
                result,
            });
        });
    }

    fn handle_window_capture(
        &mut self,
        request_id: Option<String>,
        action: String,
        generation: u64,
        result: Result<CapturedWindowImage, String>,
    ) {
        if generation != self.preview.generation {
            return;
        }
        self.preview.capturing = false;
        if !self.window_access_enabled {
            self.preview.close();
            self.render_agent_panel();
            return;
        }
        if let Some(request_id) = request_id.as_deref() {
            let current = self
                .run_id_for_request(request_id)
                .and_then(|run_id| self.agent_run_for_id(run_id))
                .is_some_and(AgentRun::is_active);
            if !current {
                self.preview.status =
                    "Capture discarded because the requesting run is no longer active.".to_owned();
                self.render_agent_panel();
                return;
            }
        }

        let manual_preview = request_id.is_none();
        match result {
            Ok(image) => {
                let data_url = format!(
                    "data:image/jpeg;base64,{}",
                    BASE64_STANDARD.encode(&image.jpeg)
                );
                self.preview.visible = true;
                self.preview.status = format!(
                    "{} · {} × {} · {} KB",
                    if self.preview.live {
                        "Live local preview"
                    } else {
                        "Fresh capture"
                    },
                    image.width,
                    image.height,
                    image.jpeg.len().div_ceil(1_024)
                );
                let source_type = self
                    .preview
                    .source_type
                    .unwrap_or(NativePreviewSource::NativeWindow)
                    .id();
                let data = json!({
                    "windowId": image.window_id,
                    "title": image.title,
                    "pid": image.pid,
                    "owned": image.owned,
                    "width": image.width,
                    "height": image.height,
                    "format": "jpeg",
                    "jpegBytes": image.jpeg.len(),
                    "sourceType": source_type,
                    "ownerProcessId": self.preview.owner_process_id,
                });
                self.preview.image = Some(image);
                if let Some(request_id) = request_id {
                    self.provider_tool_images
                        .insert(request_id.clone(), data_url.clone());
                    self.finish_command(
                        request_id,
                        action,
                        true,
                        "Application window captured".to_owned(),
                        data,
                    );
                } else {
                    self.render_agent_panel();
                }
                self.stream_preview_image(&data_url);
                if manual_preview
                    && self.preview.live
                    && self.preview.visible
                    && self.preview.target.is_some()
                {
                    self.preview.capturing = true;
                    self.schedule_window_capture_tick(generation);
                }
            }
            Err(message) => {
                if manual_preview {
                    self.preview
                        .stop_live(format!("Live preview stopped: {message}"));
                } else {
                    self.preview.status = message.clone();
                }
                if let Some(request_id) = request_id {
                    self.finish_command(request_id, action, false, message, Value::Null);
                } else {
                    self.push_chat_message(ChatRole::System, message, Vec::new());
                    self.render_agent_panel();
                }
            }
        }
    }

    fn schedule_window_capture_tick(&self, generation: u64) {
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(900));
            let _ = proxy.send_event(BrowserEvent::WindowCaptureTick { generation });
        });
    }

    fn continue_window_capture(&mut self, generation: u64) {
        if generation != self.preview.generation
            || !self.preview.live
            || !self.preview.visible
            || !self.preview.capturing
            || !self.window_access_enabled
            || self.safety.paused()
        {
            return;
        }
        let Some(target) = self.preview.target.clone() else {
            self.preview
                .stop_live("Live preview stopped because its target is unavailable.");
            self.render_agent_panel();
            return;
        };
        self.spawn_window_capture(None, "live_window_preview".to_owned(), target, generation);
    }

    fn close_preview(&mut self) {
        self.destroy_preview_surface();
        self.preview.close();
        self.render_agent_panel();
    }

    fn set_preview_live(&mut self, enabled: bool) {
        if !enabled {
            self.preview.stop_live("Live preview paused.");
            self.render_agent_panel();
            return;
        }
        if !self.window_access_enabled || self.safety.paused() {
            self.push_chat_message(
                ChatRole::System,
                "Resume computer control and enable window awareness before resuming a preview."
                    .to_owned(),
                Vec::new(),
            );
            self.render_agent_panel();
            return;
        }
        if self.preview.capturing {
            return;
        }
        match self.preview.resume_live() {
            Ok((generation, target)) => {
                self.render_agent_panel();
                self.spawn_window_capture(
                    None,
                    "live_window_preview".to_owned(),
                    target,
                    generation,
                );
            }
            Err(message) => {
                self.push_chat_message(ChatRole::System, message, Vec::new());
                self.render_agent_panel();
            }
        }
    }

    fn resume_computer_control(&mut self) {
        match self.safety.resume() {
            Ok(()) => {
                self.audit.record(
                    "computer_control_resumed",
                    "",
                    "computer_control",
                    "window",
                    "external_interaction",
                    "running",
                    "Computer control resumed by the trusted local UI",
                );
                self.windows.clear();
                self.ui_automation
                    .clear("Computer control resumed; scan windows before the next local action.");
                self.push_chat_message(
                    ChatRole::System,
                    "Computer control resumed. Fresh window and UI references are required."
                        .to_owned(),
                    Vec::new(),
                );
            }
            Err(message) => self.push_chat_message(ChatRole::System, message, Vec::new()),
        }
        self.render_agent_panel();
    }

    fn handle_safety_event(&mut self, event: SafetyEvent) {
        self.safety.handle_status_event(&event);
        match event {
            SafetyEvent::EmergencyStop => self.halt_computer_control(
                "Emergency stop activated. Agent and managed computer actions were stopped.",
                true,
            ),
            SafetyEvent::SecureDesktopEntered => self.halt_computer_control(
                "Windows entered a locked or secure desktop. Computer control was stopped.",
                false,
            ),
            SafetyEvent::HotkeyUnavailable(message) => self.halt_computer_control(
                &format!("Global emergency stop unavailable: {message}"),
                false,
            ),
            SafetyEvent::HotkeyReady | SafetyEvent::DefaultDesktopRestored => {
                self.render_agent_panel();
            }
        }
    }

    fn halt_computer_control(&mut self, reason: &str, always_announce: bool) {
        // A global stop must not make a newly idle owner drain its old queue.
        // Durable Agent receipts remain available for explicit retry, never replay.
        self.agent_submission_queue.clear();
        self.pending_agent_submissions.clear();
        self.audit.record(
            "safety_stop",
            "",
            "computer_control",
            "window",
            "destructive",
            "denied",
            reason,
        );
        let had_active_run = self.any_agent_active();
        let had_pending = !self.pending_commands.is_empty();
        self.pending_commands.clear();
        let managed_count = self
            .processes
            .cancel_all("Cancelled by the computer-control safety stop");
        let interrupted_terminal = self.terminal.interrupt_all_agent_commands();
        let mut stopped_run_ids = HashSet::new();

        for job in self.claude_jobs.values() {
            stopped_run_ids.insert(job.run_id);
            job.handle.cancel();
        }
        for job in self.subscription_jobs.values() {
            stopped_run_ids.insert(job.run_id);
            job.handle.cancel();
        }

        for run in self.main_runs.values_mut().filter(|run| run.is_active()) {
            stopped_run_ids.insert(run.id);
            if let Some(step) = run.steps.get_mut(run.current_index) {
                step.status = AgentStepStatus::Denied;
            }
            run.active_request_id = None;
            run.active_provider_call_id = None;
            run.phase = AgentPhase::Stopped;
            run.status = reason.to_owned();
        }
        for presentation in self
            .agent_graph_runs
            .values_mut()
            .filter(|presentation| presentation.runtime.is_active())
        {
            let run = &mut presentation.runtime;
            stopped_run_ids.insert(run.id);
            if let Some(step) = run.steps.get_mut(run.current_index) {
                step.status = AgentStepStatus::Denied;
            }
            run.active_request_id = None;
            run.active_provider_call_id = None;
            run.phase = AgentPhase::Stopped;
            run.status = reason.to_owned();
        }
        for run_id in stopped_run_ids {
            self.finish_run_when_quiescent(run_id);
            self.finish_run_timing(run_id, "safety_stop");
        }

        for entry in &mut self.action_log {
            if matches!(
                entry.status,
                CommandStatus::Running | CommandStatus::AwaitingApproval
            ) {
                entry.status = CommandStatus::Denied;
            }
        }
        self.permission_policy.set_mode(PermissionMode::EveryAction);
        self.remote_desktop.set_agent_control(false);
        self.windows.clear();
        self.ui_automation
            .clear("Semantic references cleared by the safety stop.");
        self.destroy_preview_surface();
        self.preview.close();
        self.provider_tool_images.clear();
        if always_announce
            || had_active_run
            || had_pending
            || managed_count > 0
            || !interrupted_terminal.is_empty()
        {
            self.push_chat_message(ChatRole::System, reason.to_owned(), Vec::new());
        }
        self.save_session();
        self.render_remote_desktop();
        self.render_agent_panel();
    }

    fn start_ui_inspection(
        &mut self,
        request_id: String,
        action: String,
        target: crate::window_runtime::WindowAutomationTarget,
    ) {
        if let Err(message) = self.ui_automation.begin_inspection(&target) {
            self.finish_command(request_id, action, false, message, Value::Null);
            return;
        }
        self.render_agent_panel();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result =
                ui_automation::inspect_window(target).map(UiAutomationTaskResult::Inspection);
            let _ = proxy.send_event(BrowserEvent::UiAutomation {
                request_id,
                action,
                result,
            });
        });
    }

    fn start_ui_action(
        &mut self,
        request_id: String,
        action: String,
        target: crate::ui_automation::UiElementTarget,
        operation: crate::ui_automation::UiElementOperation,
    ) {
        self.render_agent_panel();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = ui_automation::execute_action(target, operation)
                .map(UiAutomationTaskResult::Action);
            let _ = proxy.send_event(BrowserEvent::UiAutomation {
                request_id,
                action,
                result,
            });
        });
    }

    fn handle_ui_automation_result(
        &mut self,
        request_id: String,
        action: String,
        result: Result<UiAutomationTaskResult, String>,
    ) {
        let current = self
            .run_id_for_request(&request_id)
            .and_then(|run_id| self.agent_run_for_id(run_id))
            .is_some_and(AgentRun::is_active);
        if !current {
            self.ui_automation
                .clear("Semantic result discarded because its run is no longer active.");
            self.render_agent_panel();
            return;
        }
        match result {
            Ok(UiAutomationTaskResult::Inspection(inspection)) => {
                let data = self.ui_automation.accept_inspection(inspection);
                self.finish_command(
                    request_id,
                    action,
                    true,
                    "Semantic UI tree inspected".to_owned(),
                    data,
                );
            }
            Ok(UiAutomationTaskResult::Action(data)) => {
                self.ui_automation
                    .record_result(true, "Semantic UI action completed");
                self.finish_command(
                    request_id,
                    action,
                    true,
                    "Semantic UI action completed".to_owned(),
                    data,
                );
            }
            Err(message) => {
                self.ui_automation.record_result(false, &message);
                self.finish_command(request_id, action, false, message, Value::Null);
            }
        }
    }

    fn refresh_windows_for_user(&mut self) {
        if !self.window_access_enabled {
            self.push_chat_message(
                ChatRole::System,
                "Enable window awareness in Settings before scanning local windows.".to_owned(),
                Vec::new(),
            );
        } else {
            let owned_pids = self.processes.owned_pids();
            if let Err(error) = self.windows.refresh(&owned_pids) {
                self.push_chat_message(ChatRole::System, error, Vec::new());
            }
        }
        self.render_agent_panel();
    }

    fn write_terminal(&mut self, session_id: u64, data: String) {
        if data.is_empty() || data.len() > MAX_TERMINAL_INPUT_LEN || data.contains('\0') {
            self.push_chat_message(
                ChatRole::System,
                "Terminal input is empty or too long.".to_owned(),
                Vec::new(),
            );
            self.render_agent_panel();
            return;
        }
        let command_action = self.terminal.command_action(session_id).to_owned();
        match self.terminal.write(session_id, &data) {
            Ok(Some(request_id)) => {
                if command_action == SSH_RUNTIME_DISCOVERY_ACTION {
                    self.audit.record(
                        "ssh_runtime_discovery_interrupted",
                        &request_id,
                        "terminal_session",
                        &session_id.to_string(),
                        "ssh",
                        "interrupted",
                        "Local SSH capability discovery was interrupted by the user",
                    );
                    self.fail_pending_ssh_runtime_command(
                        session_id,
                        "Local SSH preparation was interrupted by the user",
                    );
                } else {
                    let resume_ssh_queue = command_action == "ssh_run";
                    self.finish_command(
                        request_id,
                        command_action,
                        false,
                        "Terminal command interrupted by the user".to_owned(),
                        Value::Null,
                    );
                    if resume_ssh_queue {
                        self.resume_pending_ssh_runtime_command(session_id);
                    }
                }
            }
            Ok(None) => {}
            Err(error) => self.push_chat_message(ChatRole::System, error, Vec::new()),
        }
    }

    fn close_terminal(&mut self, session_id: u64) {
        self.freeze_terminal_context(session_id);
        let command_action = self.terminal.command_action(session_id).to_owned();
        match self.terminal.close(session_id) {
            Ok(Some(request_id)) => {
                if command_action == SSH_RUNTIME_DISCOVERY_ACTION {
                    self.audit.record(
                        "ssh_runtime_discovery_interrupted",
                        &request_id,
                        "terminal_session",
                        &session_id.to_string(),
                        "ssh",
                        "interrupted",
                        "The SSH session closed during local capability discovery",
                    );
                    self.fail_pending_ssh_runtime_command(
                        session_id,
                        "The SSH session was closed during local runtime preparation",
                    );
                } else {
                    self.finish_command(
                        request_id,
                        command_action,
                        false,
                        "Terminal session closed by the user".to_owned(),
                        Value::Null,
                    );
                }
            }
            Ok(None) => {}
            Err(error) => self.push_chat_message(ChatRole::System, error, Vec::new()),
        }
        if !self.terminal.is_running() {
            self.set_terminal_panel_visible(false);
        }
        if !self.terminal.has_agent_ready_ssh_session() {
            self.permission_policy.revoke_scope(CapabilityScope::Ssh);
        }
        self.render_agent_panel();
    }

    fn set_theme(&mut self, requested: &str) {
        let theme = normalize_theme(requested).to_owned();
        if self.theme == theme {
            return;
        }
        self.theme = theme;
        self.apply_native_window_themes();
        self.apply_webview_backgrounds();
        self.refresh_start_pages();
        self.render_toolbar();
        self.render_agent_panel();
        self.render_remote_desktop();
        self.save_session();
    }

    fn set_search_engine(&mut self, requested: &str) {
        let next = search_engine(requested).id.to_owned();
        if self.search_engine == next {
            return;
        }
        self.search_engine = next;
        self.refresh_start_pages();
        self.render_agent_panel();
        self.save_session();
    }

    fn set_audit_retention(&mut self, days: u64) {
        match self.audit.set_retention_days(days) {
            Ok(()) => {
                self.audit.record(
                    "audit_retention_changed",
                    "",
                    "audit_retention",
                    "runtime",
                    "metadata",
                    "success",
                    "Audit retention changed by the trusted local UI",
                );
                self.save_session();
            }
            Err(message) => {
                self.push_chat_message(
                    ChatRole::System,
                    format!("Could not update audit retention: {message}"),
                    Vec::new(),
                );
            }
        }
        self.render_agent_panel();
    }

    fn handle_agent_graph_project_drop(&mut self, event: ProjectDropEvent) {
        match event {
            ProjectDropEvent::Enter {
                directory_count,
                file_count,
                position,
            } => {
                self.notify_agent_graph_file_drag(
                    "enter",
                    Some(position),
                    Some(file_count),
                    Some(directory_count),
                );
                self.project_drop_active = file_count == 0 && directory_count == 1;
                if file_count == 0 {
                    self.project_import_inspection_key = None;
                    self.project_import = if directory_count == 1 {
                        ProjectImportView::default()
                    } else {
                        ProjectImportView {
                            active: false,
                            kind: Some("local".to_owned()),
                            message: Some("Drop one project folder at a time.".to_owned()),
                            error: true,
                        }
                    };
                }
                self.render_agent_graph_surface();
            }
            ProjectDropEvent::Over { position } => {
                self.notify_agent_graph_file_drag("over", Some(position), None, None);
            }
            ProjectDropEvent::Leave => {
                self.notify_agent_graph_file_drag("leave", None, None, None);
                self.project_drop_active = false;
                self.render_agent_graph_surface();
            }
            ProjectDropEvent::Drop { paths, position } => {
                self.project_drop_active = false;
                if paths.iter().any(|path| !path.is_dir()) {
                    self.resolve_agent_graph_file_drop(paths, position);
                } else {
                    self.notify_agent_graph_file_drag(
                        "drop",
                        Some(position),
                        Some(0),
                        Some(paths.len()),
                    );
                    self.complete_agent_graph_file_drop(paths, None);
                }
            }
        }
    }

    fn complete_agent_graph_file_drop(&mut self, paths: Vec<PathBuf>, owner: Option<String>) {
        if let Some(owner) = owner {
            self.add_graph_file_paths(&owner, paths);
            return;
        }
        if paths.len() != 1 || !paths[0].is_dir() {
            self.project_import_inspection_key = None;
            self.project_import = ProjectImportView {
                active: false,
                kind: Some("local".to_owned()),
                message: Some(
                    "Drop files onto an open Project chat or Supervisor chat, or drop one folder to add a project."
                        .to_owned(),
                ),
                error: true,
            };
            self.render_agent_graph_surface();
            return;
        }
        self.connect_workspace_path(paths[0].clone());
    }

    fn select_workspace(&mut self) {
        let mut dialog = rfd::FileDialog::new().set_title("Add a Supervisor project");
        if let Some(root) = self.workspace.root() {
            dialog = dialog.set_directory(root);
        }
        let Some(path) = dialog.pick_folder() else {
            return;
        };
        self.connect_workspace_path(path);
    }

    fn create_project_folder(&mut self, name: &str) {
        if self.project_import.active {
            return;
        }
        let result = (|| {
            let name = crate::project_registry::validate_new_project_name(name)?;
            let mut dialog = rfd::FileDialog::new().set_title("Choose where to create the project");
            if let Some(documents) = directories::UserDirs::new()
                .and_then(|dirs| dirs.document_dir().map(Path::to_path_buf))
            {
                dialog = dialog.set_directory(documents);
            }
            let Some(parent) = dialog.pick_folder() else {
                return Ok(None);
            };
            crate::project_registry::create_local_project(&parent, name).map(Some)
        })();
        match result {
            Ok(Some(path)) => {
                self.connect_workspace_path(path);
                self.project_import.kind = Some("create".into());
            }
            Ok(None) => {
                self.project_import = ProjectImportView::default();
                self.project_import_inspection_key = None;
            }
            Err(message) => {
                self.project_import = ProjectImportView {
                    kind: Some("create".into()),
                    message: Some(message),
                    error: true,
                    ..ProjectImportView::default()
                };
                self.project_import_inspection_key = None;
            }
        }
        self.render_agent_panel();
    }

    fn connect_workspace_path(&mut self, path: PathBuf) {
        self.sync_active_project_chat();
        match self.workspace.connect(path) {
            Ok(()) => {
                let root = self
                    .workspace
                    .root()
                    .map(|root| root.display().to_string())
                    .unwrap_or_default();
                self.workspace_last_opened_ms
                    .insert(root.clone(), project_timestamp_ms());
                self.select_latest_chat_for_active_project();
                self.permission_policy
                    .revoke_scope(CapabilityScope::Workspace);
                self.terminal.reset_default_cwd();
                self.project_import = ProjectImportView {
                    active: false,
                    kind: Some("local".to_owned()),
                    message: Some("Project added. Detecting its metadata…".to_owned()),
                    error: false,
                };
                self.project_import_inspection_key = Some(root.clone());
                self.start_local_project_inspection(root);
                self.save_session();
            }
            Err(error) => {
                self.project_import_inspection_key = None;
                self.project_import = ProjectImportView {
                    active: false,
                    kind: Some("local".to_owned()),
                    message: Some(error),
                    error: true,
                };
            }
        }
        self.render_agent_panel();
    }

    fn select_clone_destination(&mut self, repository: &str) {
        if self.project_import.active {
            return;
        }
        let mut dialog = rfd::FileDialog::new().set_title("Choose where to clone the project");
        if let Some(root) = self.workspace.root().and_then(Path::parent) {
            dialog = dialog.set_directory(root);
        } else if let Some(documents) =
            directories::UserDirs::new().and_then(|dirs| dirs.document_dir().map(Path::to_path_buf))
        {
            dialog = dialog.set_directory(documents);
        }
        let Some(parent) = dialog.pick_folder() else {
            return;
        };
        let operation_id = Uuid::new_v4().to_string();
        self.project_import_inspection_key = None;
        self.project_import = ProjectImportView {
            active: true,
            kind: Some("clone".to_owned()),
            message: Some("Cloning repository…".to_owned()),
            error: false,
        };
        self.project_import_operation_id = Some(operation_id.clone());
        let repository = repository.to_owned();
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = clone_repository(&repository, &parent);
            let _ = proxy.send_event(BrowserEvent::ProjectCloneCompleted {
                operation_id,
                result,
            });
        });
        self.render_agent_graph_surface();
    }

    fn complete_project_clone(&mut self, operation_id: &str, result: Result<PathBuf, String>) {
        if self.project_import_operation_id.as_deref() != Some(operation_id) {
            return;
        }
        self.project_import_operation_id = None;
        self.project_import.active = false;
        match result {
            Ok(path) => {
                self.project_import.message = Some("Repository cloned and added.".to_owned());
                self.project_import.error = false;
                self.connect_workspace_path(path);
            }
            Err(message) => {
                self.project_import_inspection_key = None;
                self.project_import.message = Some(message);
                self.project_import.error = true;
                self.render_agent_graph_surface();
            }
        }
    }

    fn connect_ssh_project(&mut self, profile_id: &str, directory: &str, name: Option<&str>) {
        let Some(profile) = self.ssh.profile(profile_id) else {
            self.project_import_inspection_key = None;
            self.project_import = ProjectImportView {
                active: false,
                kind: Some("ssh".to_owned()),
                message: Some("Choose an existing SSH server profile.".to_owned()),
                error: true,
            };
            self.render_agent_graph_surface();
            return;
        };
        let directory = match normalize_remote_directory(directory) {
            Ok(directory) => directory,
            Err(message) => {
                self.project_import_inspection_key = None;
                self.project_import = ProjectImportView {
                    active: false,
                    kind: Some("ssh".to_owned()),
                    message: Some(message),
                    error: true,
                };
                self.render_agent_graph_surface();
                return;
            }
        };
        if let Some(project) = self
            .remote_projects
            .iter_mut()
            .find(|project| project.ssh_profile_id == profile_id && project.directory == directory)
        {
            project.last_opened_at_ms = project_timestamp_ms();
            self.project_import_inspection_key = None;
            self.project_import = ProjectImportView {
                active: false,
                kind: Some("ssh".to_owned()),
                message: Some(
                    "This SSH project was already added. Its existing card is ready.".to_owned(),
                ),
                error: false,
            };
            self.save_session();
            self.render_agent_graph_surface();
            return;
        }
        let now = project_timestamp_ms();
        let fallback_name = remote_project_name(&directory);
        let name = name
            .and_then(|value| normalized_user_title(value, MAX_PROJECT_LABEL_CHARS))
            .unwrap_or(fallback_name);
        let id = Uuid::new_v4().to_string();
        self.remote_projects.push(RemoteProject {
            id: id.clone(),
            name,
            ssh_profile_id: profile.id.clone(),
            directory,
            pinned: false,
            created_at_ms: now,
            last_opened_at_ms: now,
        });
        self.project_import = ProjectImportView {
            active: false,
            kind: Some("ssh".to_owned()),
            message: Some("SSH project added. Detecting its project metadata…".to_owned()),
            error: false,
        };
        self.project_import_inspection_key = Some(remote_project_metadata_key(&id));
        self.start_remote_project_inspection(id);
        self.save_session();
        self.render_agent_panel();
    }

    fn set_remote_project_pinned(&mut self, id: &str, pinned: bool) {
        let Some(project) = self
            .remote_projects
            .iter_mut()
            .find(|project| project.id == id)
        else {
            return;
        };
        project.pinned = pinned;
        self.save_session();
        self.render_agent_panel();
    }

    fn remove_remote_project(&mut self, id: &str) {
        if self.agent_graph_bindings.iter().any(|binding| {
            binding.ssh_profile_id.is_some()
                && binding.record_id == id
                && self
                    .active_agent_graph_node_keys()
                    .contains(&binding.node_key().as_str())
        }) {
            self.project_import_inspection_key = None;
            self.project_import = ProjectImportView {
                active: false,
                kind: Some("ssh".to_owned()),
                message: Some("Stop this project's agent before removing it.".to_owned()),
                error: true,
            };
            self.render_agent_graph_surface();
            return;
        }
        let before = self.remote_projects.len();
        self.remote_projects.retain(|project| project.id != id);
        if self.remote_projects.len() == before {
            return;
        }
        let key = remote_project_metadata_key(id);
        self.project_metadata.remove(&key);
        self.project_scans_in_flight.remove(&key);
        if self.project_import_inspection_key.as_deref() == Some(key.as_str()) {
            self.project_import = ProjectImportView::default();
            self.project_import_inspection_key = None;
        }
        self.agent_graph_bindings
            .retain(|binding| binding.record_id != id);
        self.save_session();
        self.render_agent_panel();
    }

    fn open_project(&mut self, source: &str, id: &str) {
        if source == "local" {
            self.activate_workspace(id);
            self.close_agent_graph();
            return;
        }
        if source == "ssh"
            && let Some(project) = self
                .remote_projects
                .iter_mut()
                .find(|project| project.id == id)
        {
            project.last_opened_at_ms = project_timestamp_ms();
            self.save_session();
            self.render_agent_graph_surface();
        }
    }

    fn report_project_terminal_error(&mut self, error: String) {
        if self.agent_graph_open {
            self.conversation_event(
                "central-agent:project-board-error",
                json!({"owner":"graph:project-board","error":error}),
            );
        } else {
            self.push_chat_message(ChatRole::System, error, Vec::new());
            self.render_agent_panel();
        }
    }

    fn reveal_project_terminal(&mut self, event_loop: &ActiveEventLoop) {
        self.terminal_panel_visible = true;
        if self.agent_graph_open || self.terminal_window.is_some() {
            // The board deliberately hides the docked terminal. Show this
            // user-requested session in Supervisor's detached terminal instead.
            if let Err(error) = self.detach_terminal_panel(event_loop) {
                self.report_project_terminal_error(format!(
                    "Could not show the project terminal: {error}"
                ));
            }
        } else {
            self.layout_webviews();
            self.render_toolbar();
        }
        self.render_agent_panel();
    }

    fn open_project_terminal(
        &mut self,
        event_loop: &ActiveEventLoop,
        source: &str,
        id: &str,
        git: bool,
    ) {
        if source == "local" {
            let root = match fs::canonicalize(id) {
                Ok(root) => root,
                Err(_) => {
                    self.report_project_terminal_error(
                        "This project folder is no longer available. Reconnect it and try again."
                            .into(),
                    );
                    return;
                }
            };
            if !self
                .workspace
                .project_roots()
                .iter()
                .any(|candidate| fs::canonicalize(candidate).is_ok_and(|known| known == root))
            {
                self.report_project_terminal_error(
                    "Reconnect this project before opening its terminal.".into(),
                );
                return;
            }
            let callback = self.terminal_callback();
            match self.terminal.open_in_directory(&root, callback) {
                Ok(session_id) => {
                    if git
                        && let Err(error) = self
                            .terminal
                            .write(session_id, "git status --short --branch\r")
                    {
                        self.report_project_terminal_error(error);
                    }
                    self.reveal_project_terminal(event_loop);
                }
                Err(error) => self.report_project_terminal_error(error),
            }
            return;
        }
        if source == "ssh" {
            let Some(project) = self
                .remote_projects
                .iter()
                .find(|project| project.id == id)
                .cloned()
            else {
                self.report_project_terminal_error(
                    "Reconnect this SSH project before opening its terminal.".into(),
                );
                return;
            };
            let mut launch = match self.ssh.launch_spec(&project.ssh_profile_id) {
                Ok(launch) => launch,
                Err(message) => {
                    self.report_project_terminal_error(message);
                    return;
                }
            };
            let directory = project.directory.replace('\'', "'\"'\"'");
            let remote_command = if git {
                format!(
                    "cd -- '{directory}' && git status --short --branch; exec \"${{SHELL:-/bin/sh}}\" -l"
                )
            } else {
                format!("cd -- '{directory}' && exec \"${{SHELL:-/bin/sh}}\" -l")
            };
            launch.arguments.push(remote_command);
            let callback = self.terminal_callback();
            match self.terminal.open_ssh(launch, callback) {
                Ok(_) => self.reveal_project_terminal(event_loop),
                Err(message) => self.report_project_terminal_error(message),
            }
            return;
        }
        self.report_project_terminal_error("Select a connected local or SSH project.".into());
    }

    fn show_project_folder(&mut self, root: &str) {
        let path = PathBuf::from(root);
        if self
            .workspace
            .project_roots()
            .iter()
            .any(|candidate| candidate.display().to_string().eq_ignore_ascii_case(root))
            && let Err(error) = open_with_default(&path)
        {
            self.push_chat_message(ChatRole::System, error, Vec::new());
            self.render_agent_panel();
        }
    }

    fn refresh_project_metadata(&mut self, source: &str, id: &str) {
        if source == "local" {
            self.start_local_project_inspection(id.to_owned());
        } else if source == "ssh" {
            self.start_remote_project_inspection(id.to_owned());
        }
        self.render_agent_graph_surface();
    }

    fn set_reopen_last_project(&mut self, enabled: bool) {
        self.reopen_last_project = enabled;
        self.save_session();
        self.render_agent_panel();
    }

    fn start_local_project_inspection(&mut self, root: String) {
        if root.is_empty() || !self.project_scans_in_flight.insert(root.clone()) {
            return;
        }
        let proxy = self.proxy.clone();
        let path = PathBuf::from(&root);
        std::thread::spawn(move || {
            let result = inspect_local_project(&path, project_timestamp_ms());
            let _ =
                proxy.send_event(BrowserEvent::ProjectInspectionCompleted { key: root, result });
        });
    }

    fn start_remote_project_inspection(&mut self, id: String) {
        let Some(project) = self
            .remote_projects
            .iter()
            .find(|project| project.id == id)
            .cloned()
        else {
            return;
        };
        let key = remote_project_metadata_key(&project.id);
        if !self.project_scans_in_flight.insert(key.clone()) {
            return;
        }
        let spec = match self.ssh.batch_command_spec(&project.ssh_profile_id) {
            Ok(spec) => spec,
            Err(message) => {
                let _ = self
                    .proxy
                    .send_event(BrowserEvent::ProjectInspectionCompleted {
                        key,
                        result: ProjectMetadata {
                            detected_at_ms: project_timestamp_ms(),
                            error: Some(message),
                            ..ProjectMetadata::default()
                        },
                    });
                return;
            }
        };
        let script = match remote_inspection_script(&project.directory) {
            Ok(script) => script,
            Err(message) => {
                let _ = self
                    .proxy
                    .send_event(BrowserEvent::ProjectInspectionCompleted {
                        key,
                        result: ProjectMetadata {
                            detected_at_ms: project_timestamp_ms(),
                            error: Some(message),
                            ..ProjectMetadata::default()
                        },
                    });
                return;
            }
        };
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = match run_batch_ssh_command(spec, &script) {
                Ok(output) => parse_remote_inspection(&output, project_timestamp_ms()),
                Err(message) => ProjectMetadata {
                    detected_at_ms: project_timestamp_ms(),
                    error: Some(message),
                    ..ProjectMetadata::default()
                },
            };
            let _ = proxy.send_event(BrowserEvent::ProjectInspectionCompleted { key, result });
        });
    }

    fn start_project_inspections(&mut self) {
        let now = project_timestamp_ms();
        let local = self
            .workspace
            .project_roots()
            .iter()
            .map(|root| root.display().to_string())
            .filter(|root| {
                self.project_metadata.get(root).is_none_or(|metadata| {
                    now.saturating_sub(metadata.detected_at_ms) > PROJECT_METADATA_FRESH_MS
                })
            })
            .collect::<Vec<_>>();
        for root in local {
            self.start_local_project_inspection(root);
        }
        let remote = self
            .remote_projects
            .iter()
            .filter(|project| {
                self.project_metadata
                    .get(&remote_project_metadata_key(&project.id))
                    .is_none_or(|metadata| {
                        now.saturating_sub(metadata.detected_at_ms) > PROJECT_METADATA_FRESH_MS
                    })
            })
            .map(|project| project.id.clone())
            .collect::<Vec<_>>();
        for id in remote {
            self.start_remote_project_inspection(id);
        }
    }

    fn complete_project_inspection(&mut self, key: String, result: ProjectMetadata) {
        if !self.project_scans_in_flight.remove(&key) {
            return;
        }
        let local_exists = self
            .workspace
            .project_roots()
            .iter()
            .any(|root| root.display().to_string() == key);
        let remote_exists = key
            .strip_prefix("ssh:")
            .is_some_and(|id| self.remote_projects.iter().any(|project| project.id == id));
        if !local_exists && !remote_exists {
            if self.project_import_inspection_key.as_deref() == Some(key.as_str()) {
                self.project_import = ProjectImportView::default();
                self.project_import_inspection_key = None;
                self.render_agent_panel();
            }
            return;
        }
        let completes_import = self.project_import_inspection_key.as_deref() == Some(key.as_str());
        self.project_metadata.insert(key, result);
        if completes_import {
            self.project_import = ProjectImportView::default();
            self.project_import_inspection_key = None;
        }
        self.save_session();
        self.render_agent_panel();
    }

    fn activate_workspace(&mut self, root: &str) {
        self.sync_active_project_chat();
        match self.workspace.activate(root) {
            Ok(()) => {
                self.workspace_last_opened_ms
                    .insert(root.to_owned(), project_timestamp_ms());
                self.select_latest_chat_for_active_project();
                self.permission_policy
                    .revoke_scope(CapabilityScope::Workspace);
                self.terminal.reset_default_cwd();
                self.start_local_project_inspection(root.to_owned());
                self.save_session();
            }
            Err(error) => self.push_chat_message(ChatRole::System, error, Vec::new()),
        }
        self.render_agent_panel();
    }

    fn sync_active_project_chat(&mut self) {
        let Some(chat_id) = self.active_project_chat_id.as_deref() else {
            return;
        };
        let messages = self.main_chat_messages().cloned().collect::<Vec<_>>();
        let updated_at_ms = messages
            .last()
            .map_or_else(unix_time_ms, |message| message.timestamp_ms);
        if let Some(chat) = self
            .project_chats
            .iter_mut()
            .find(|chat| chat.id == chat_id)
        {
            if !chat.title_custom {
                chat.title = project_chat_title(&messages);
            }
            chat.messages = messages;
            chat.updated_at_ms = updated_at_ms;
            chat.claude_session_id = self.claude_session_id.clone();
            chat.claude_session_cwd = self.claude_session_cwd.clone();
        }
    }

    fn load_project_chat_state(&mut self, chat_id: Option<String>) {
        let selected = chat_id.as_ref().and_then(|chat_id| {
            self.project_chats
                .iter()
                .find(|chat| chat.id == *chat_id && !chat.archived)
                .cloned()
        });
        self.active_project_chat_id = selected.as_ref().map(|chat| chat.id.clone());
        if let Some(chat) = &selected {
            self.chat_ownership.load(chat, &mut self.chat_messages);
        }
        self.claude_session_id = selected
            .as_ref()
            .and_then(|chat| chat.claude_session_id.clone());
        self.claude_session_cwd = selected
            .as_ref()
            .and_then(|chat| chat.claude_session_cwd.clone());
        self.switch_main_draft();
        self.sync_app_server_p2_scope();
    }

    fn select_latest_chat_for_active_project(&mut self) {
        let root = self.workspace.root().map(|root| root.display().to_string());
        let selected = root.as_deref().and_then(|root| {
            self.project_chats
                .iter()
                .filter(|chat| chat.project_root == root && !chat.archived)
                .max_by_key(|chat| chat.updated_at_ms)
                .map(|chat| chat.id.clone())
        });
        self.load_project_chat_state(selected);
    }

    fn ensure_active_project_chat(&mut self) {
        let root = self
            .workspace
            .root()
            .map(|root| root.display().to_string())
            .unwrap_or_default();
        if self
            .active_project_chat_id
            .as_deref()
            .is_some_and(|chat_id| {
                self.project_chats
                    .iter()
                    .any(|chat| chat.id == chat_id && chat.project_root == root && !chat.archived)
            })
        {
            return;
        }
        if let Some(chat_id) = self
            .project_chats
            .iter()
            .filter(|chat| chat.project_root == root && !chat.archived)
            .max_by_key(|chat| chat.updated_at_ms)
            .map(|chat| chat.id.clone())
        {
            if self.main_draft_owner == format!("draft:{root}") {
                self.main_draft_owner = format!("chat:{chat_id}");
            }
            self.load_project_chat_state(Some(chat_id));
            return;
        }
        let now_ms = unix_time_ms();
        let chat_id = Uuid::new_v4().to_string();
        self.project_chats.push(ProjectChat {
            id: chat_id.clone(),
            project_root: root,
            title: "New chat".to_owned(),
            title_custom: false,
            pinned: false,
            archived: false,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            messages: Vec::new(),
            claude_session_id: None,
            claude_session_cwd: None,
        });
        self.main_draft_owner = format!("chat:{chat_id}");
        self.load_project_chat_state(Some(chat_id));
    }

    fn main_chat_messages(&self) -> impl Iterator<Item = &ChatMessage> {
        self.chat_messages_for(self.active_project_chat_id.as_deref())
    }

    fn conversation_key(&self, graph_node: Option<&str>) -> String {
        graph_node
            .map(|node| format!("graph:{node}"))
            .unwrap_or_else(|| {
                self.active_project_chat_id
                    .as_ref()
                    .map(|id| format!("chat:{id}"))
                    .unwrap_or_else(|| "unbound".to_owned())
            })
    }

    fn project_conversation_history(&self, chat_id: Option<&str>) -> Vec<AgentConversationMessage> {
        self.chat_messages_for(chat_id)
            .filter_map(|message| {
                let role = match message.role {
                    ChatRole::User => "user",
                    ChatRole::Assistant if message.kind == ChatMessageKind::Message => "assistant",
                    ChatRole::Assistant | ChatRole::System => return None,
                };
                let text = message.text.trim();
                (!text.is_empty()).then(|| AgentConversationMessage {
                    role: role.to_owned(),
                    text: text.to_owned(),
                })
            })
            .collect()
    }

    fn create_project_chat(&mut self, root: &str) {
        if self.pending_workspace_ejections.contains(root) {
            self.render_agent_panel();
            return;
        }
        self.sync_active_project_chat();
        if let Err(error) = self.workspace.activate(root) {
            self.push_chat_message(ChatRole::System, error, Vec::new());
            self.render_agent_panel();
            return;
        }
        let now_ms = unix_time_ms();
        let chat_id = Uuid::new_v4().to_string();
        self.project_chats.push(ProjectChat {
            id: chat_id.clone(),
            project_root: root.to_owned(),
            title: "New chat".to_owned(),
            title_custom: false,
            pinned: false,
            archived: false,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            messages: Vec::new(),
            claude_session_id: None,
            claude_session_cwd: None,
        });
        self.load_project_chat_state(Some(chat_id));
        self.permission_policy
            .revoke_scope(CapabilityScope::Workspace);
        self.terminal.reset_default_cwd();
        self.save_session();
        self.render_agent_panel();
    }

    fn rename_workspace(&mut self, root: &str, name: &str) {
        if !self
            .workspace
            .view()
            .projects
            .iter()
            .any(|project| project.root == root)
        {
            return;
        }
        let Some(name) = normalized_user_title(name, MAX_PROJECT_LABEL_CHARS) else {
            return;
        };
        self.workspace_project_labels.insert(root.to_owned(), name);
        self.save_session();
        self.render_agent_panel();
    }

    fn set_workspace_pinned(&mut self, root: &str, pinned: bool) {
        let available = self
            .workspace
            .view()
            .projects
            .iter()
            .any(|project| project.root == root && !project.archived);
        if !available {
            return;
        }
        if pinned {
            self.pinned_workspace_roots.insert(root.to_owned());
        } else {
            self.pinned_workspace_roots.remove(root);
        }
        self.save_session();
        self.render_agent_panel();
    }

    fn activate_project_chat(&mut self, chat_id: &str) {
        let Some(target) = self
            .project_chats
            .iter()
            .find(|chat| chat.id == chat_id && !chat.archived)
            .cloned()
        else {
            return;
        };
        self.sync_active_project_chat();
        if let Err(error) = self.workspace.activate(&target.project_root) {
            self.push_chat_message(ChatRole::System, error, Vec::new());
            self.render_agent_panel();
            return;
        }
        self.load_project_chat_state(Some(target.id));
        self.select_native_preparation(self.composer_owner());
        self.permission_policy
            .revoke_scope(CapabilityScope::Workspace);
        self.terminal.reset_default_cwd();
        self.save_session();
        self.render_agent_panel();
    }

    fn rename_project_chat(&mut self, chat_id: &str, title: &str) {
        let Some(title) = normalized_user_title(title, MAX_PROJECT_CHAT_TITLE_CHARS) else {
            return;
        };

        self.sync_active_project_chat();
        let Some(chat) = self
            .project_chats
            .iter_mut()
            .find(|chat| chat.id == chat_id)
        else {
            return;
        };
        chat.title = title;
        chat.title_custom = true;
        chat.updated_at_ms = unix_time_ms();
        self.save_session();
        self.render_agent_panel();
    }

    fn move_project_chat(&mut self, chat_id: &str, project_root: &str) {
        if !self.chat_mutation_available(chat_id) {
            return;
        }
        let destination_available = self
            .workspace
            .view()
            .projects
            .iter()
            .any(|project| project.root == project_root && !project.archived);
        let movable = self
            .project_chats
            .iter()
            .any(|chat| chat.id == chat_id && !chat.archived && chat.project_root != project_root);
        if !destination_available || !movable {
            return;
        }

        self.sync_active_project_chat();
        if let Err(error) = self.workspace.activate(project_root) {
            self.push_chat_message(ChatRole::System, error, Vec::new());
            self.render_agent_panel();
            return;
        }
        if !reassign_project_chat(
            &mut self.project_chats,
            chat_id,
            project_root,
            unix_time_ms(),
        ) {
            return;
        }

        self.load_project_chat_state(Some(chat_id.to_owned()));
        self.permission_policy
            .revoke_scope(CapabilityScope::Workspace);
        self.terminal.reset_default_cwd();
        self.save_session();
        self.render_agent_panel();
    }

    fn select_project_for_chat(&mut self, chat_id: &str) {
        if !self.chat_mutation_available(chat_id) {
            return;
        }
        let Some(current_project_root) = self
            .project_chats
            .iter()
            .find(|chat| chat.id == chat_id && !chat.archived)
            .map(|chat| chat.project_root.clone())
        else {
            return;
        };

        let mut dialog = rfd::FileDialog::new().set_title("Choose a project for this chat");
        let current_directory = Path::new(&current_project_root);
        if current_directory.is_dir() {
            dialog = dialog.set_directory(current_directory);
        } else if let Some(root) = self.workspace.root() {
            dialog = dialog.set_directory(root);
        }
        let Some(path) = dialog.pick_folder() else {
            return;
        };

        self.sync_active_project_chat();
        if let Err(error) = self.workspace.connect(path) {
            self.push_chat_message(ChatRole::System, error, Vec::new());
            self.render_agent_panel();
            return;
        }
        let Some(project_root) = self.workspace.root().map(|root| root.display().to_string())
        else {
            return;
        };
        if project_root != current_project_root
            && !reassign_project_chat(
                &mut self.project_chats,
                chat_id,
                &project_root,
                unix_time_ms(),
            )
        {
            return;
        }

        self.load_project_chat_state(Some(chat_id.to_owned()));
        self.permission_policy
            .revoke_scope(CapabilityScope::Workspace);
        self.terminal.reset_default_cwd();
        self.save_session();
        self.render_agent_panel();
    }

    fn set_project_chat_pinned(&mut self, chat_id: &str, pinned: bool) {
        let Some(chat) = self
            .project_chats
            .iter_mut()
            .find(|chat| chat.id == chat_id && !chat.archived)
        else {
            return;
        };
        chat.pinned = pinned;
        self.save_session();
        self.render_agent_panel();
    }

    fn set_project_chat_archived(&mut self, chat_id: &str, archived: bool) {
        if !self.chat_mutation_available(chat_id) {
            return;
        }
        if !archived
            && self
                .project_chats
                .iter()
                .find(|chat| chat.id == chat_id)
                .is_some_and(|chat| {
                    self.workspace
                        .view()
                        .projects
                        .iter()
                        .any(|project| project.root == chat.project_root && project.archived)
                })
        {
            self.push_chat_message(
                ChatRole::System,
                "Restore the project before restoring one of its chats.".to_owned(),
                vec![],
            );
            self.render_agent_panel();
            return;
        }

        if archived
            && let Err(error) =
                self.cancel_submissions_for_owner(&format!("chat:{chat_id}"), true, true)
        {
            self.push_chat_message(ChatRole::System, error, vec![]);
            self.render_agent_panel();
            return;
        }
        self.sync_active_project_chat();
        let active = self.active_project_chat_id.as_deref() == Some(chat_id);
        let Some(chat) = self
            .project_chats
            .iter_mut()
            .find(|chat| chat.id == chat_id)
        else {
            return;
        };
        chat.archived = archived;
        if archived {
            chat.pinned = false;
        }
        chat.updated_at_ms = unix_time_ms();
        if archived {
            self.project_board_open_chat_ids.retain(|id| id != chat_id);
        }
        if active && archived {
            self.select_latest_chat_for_active_project();
        }
        self.save_session();
        self.render_agent_panel();
    }

    fn set_project_chats_archived(&mut self, root: &str, archived: bool) {
        if !self.project_mutation_available(root) {
            return;
        }
        if archived && let Err(error) = self.cancel_submissions_for_workspace(root) {
            self.push_chat_message(ChatRole::System, error, vec![]);
            self.render_agent_panel();
            return;
        }
        self.sync_active_project_chat();
        let active_chat_archived = archived
            && self
                .active_project_chat_id
                .as_deref()
                .is_some_and(|chat_id| {
                    self.project_chats
                        .iter()
                        .any(|chat| chat.id == chat_id && chat.project_root == root)
                });
        let mut changed = false;
        let changed_at_ms = unix_time_ms();
        for chat in self
            .project_chats
            .iter_mut()
            .filter(|chat| chat.project_root == root)
        {
            chat.archived = archived;
            if archived {
                chat.pinned = false;
            }
            chat.updated_at_ms = changed_at_ms;
            changed = true;
        }
        if !changed {
            return;
        }
        if archived {
            let archived_ids = self
                .project_chats
                .iter()
                .filter(|chat| chat.project_root == root)
                .map(|chat| chat.id.as_str())
                .collect::<HashSet<_>>();
            self.project_board_open_chat_ids
                .retain(|id| !archived_ids.contains(id.as_str()));
        }
        let restored_into_active_project = !archived
            && self.active_project_chat_id.is_none()
            && self
                .workspace
                .root()
                .is_some_and(|active| active.display().to_string() == root);
        if active_chat_archived || restored_into_active_project {
            self.select_latest_chat_for_active_project();
        }
        self.save_session();
        self.render_agent_panel();
    }

    fn delete_project_chat(&mut self, chat_id: &str) {
        if !self.chat_mutation_available(chat_id) {
            return;
        }
        if let Err(error) =
            self.cancel_submissions_for_owner(&format!("chat:{chat_id}"), true, true)
        {
            self.push_chat_message(ChatRole::System, error, vec![]);
            self.render_agent_panel();
            return;
        }
        self.sync_active_project_chat();
        let active = self.active_project_chat_id.as_deref() == Some(chat_id);
        let owner = format!("chat:{chat_id}");
        if let Err(error) = self.validate_app_server_unlink_owners(std::slice::from_ref(&owner)) {
            self.push_chat_message(ChatRole::System, error, vec![]);
            self.render_agent_panel();
            return;
        }
        if let Err(error) = self.forget_composer_draft(&owner) {
            self.push_chat_message(ChatRole::System, error, vec![]);
            self.render_agent_panel();
            return;
        }
        if let Err(error) = self.unlink_app_server_owners(std::slice::from_ref(&owner)) {
            self.push_chat_message(ChatRole::System, error, vec![]);
            self.render_agent_panel();
            return;
        }
        self.project_chats.retain(|chat| chat.id != chat_id);
        self.project_board_open_chat_ids.retain(|id| id != chat_id);
        self.project_chat_card_profiles.remove(chat_id);
        self.graph_files.forget(&owner);
        self.graph_contexts.drafts.remove(&owner);
        self.forget_chat_messages(chat_id);
        if active {
            self.select_latest_chat_for_active_project();
        }
        self.save_session();
        self.render_agent_panel();
    }

    fn set_workspace_archived(&mut self, root: &str, archived: bool) {
        if !self.project_mutation_available(root) {
            return;
        }
        if archived && let Err(error) = self.cancel_submissions_for_workspace(root) {
            self.push_chat_message(ChatRole::System, error, vec![]);
            self.render_agent_panel();
            return;
        }
        self.sync_active_project_chat();
        let result = if archived {
            self.workspace.archive_project(root)
        } else {
            self.workspace.restore_project(root)
        };
        if let Err(error) = result {
            self.push_chat_message(ChatRole::System, error, Vec::new());
            self.render_agent_panel();
            return;
        }
        for chat in self
            .project_chats
            .iter_mut()
            .filter(|chat| chat.project_root == root)
        {
            chat.archived = archived;
            if archived {
                chat.pinned = false;
            }
            chat.updated_at_ms = unix_time_ms();
        }
        if archived {
            self.pinned_workspace_roots.remove(root);
        }
        let archived_active_chat = archived
            && self
                .active_project_chat_id
                .as_deref()
                .is_some_and(|chat_id| {
                    self.project_chats
                        .iter()
                        .any(|chat| chat.id == chat_id && chat.project_root == root)
                });
        let restored_active_project = !archived
            && self
                .workspace
                .root()
                .is_some_and(|active| active.display().to_string() == root);
        if archived_active_chat || restored_active_project {
            self.select_latest_chat_for_active_project();
        }
        self.permission_policy
            .revoke_scope(CapabilityScope::Workspace);
        self.terminal.reset_default_cwd();
        self.save_session();
        self.render_agent_panel();
    }

    fn workspace_checkpoint_run_ids(&self, root: &str) -> Vec<u64> {
        let workspace_root = Path::new(root);
        let mut run_ids =
            checkpoint_run_ids_for_workspace(&self.time_machine_run_roots, workspace_root);
        run_ids.extend(
            self.run_execution_contexts
                .iter()
                .filter_map(|(id, context)| {
                    (context
                        .workspace_root
                        .as_deref()
                        .is_some_and(|scope| checkpoint_roots_overlap(scope, workspace_root))
                        && (self.agent_run_for_id(*id).is_some_and(AgentRun::is_active)
                            || self.run_has_provider_job(*id)
                            || self.processes.has_active_owner(*id)))
                    .then_some(*id)
                }),
        );
        run_ids.extend(
            self.time_machine_run_plans
                .iter()
                .filter_map(|(run_id, plan)| {
                    checkpoint_roots_overlap(&plan.root, workspace_root).then_some(*run_id)
                }),
        );
        run_ids.sort_unstable();
        run_ids.dedup();
        run_ids
    }

    fn workspace_has_active_agent(&self, root: &str) -> bool {
        self.app_server.workspace_busy(root)
            || self
                .workspace_checkpoint_run_ids(root)
                .into_iter()
                .any(|run_id| {
                    self.agent_run_for_id(run_id)
                        .is_some_and(AgentRun::is_active)
                        || self.run_has_provider_job(run_id)
                        || self.processes.has_active_owner(run_id)
                })
    }

    fn start_orphaned_checkpoint_finalizations(&mut self, run_ids: &[u64]) {
        for run_id in run_ids.iter().copied() {
            if !self.time_machine_run_roots.contains_key(&run_id)
                || self.time_machine_finalizations_in_flight.contains(&run_id)
                || self
                    .agent_run_for_id(run_id)
                    .is_some_and(AgentRun::is_active)
                || self.run_has_provider_job(run_id)
                || self.processes.has_active_owner(run_id)
            {
                continue;
            }
            self.retryable_time_machine_finalizations.remove(&run_id);
            self.time_machine_finalizations_in_flight.insert(run_id);
            let time_machine = Arc::clone(&self.time_machine);
            let proxy = self.proxy.clone();
            std::thread::spawn(move || {
                let result = time_machine
                    .lock()
                    .map_err(|_| "Time Machine storage is busy".to_owned())
                    .and_then(|mut time_machine| time_machine.finish_run(run_id));
                let _ = proxy.send_event(BrowserEvent::TimeMachineFinished { run_id, result });
            });
        }
    }

    fn eject_workspace(&mut self, root: &str, stop_active_runs: bool) {
        if self.app_server.workspace_busy(root) {
            self.push_chat_message(
                ChatRole::System,
                "Stop or reconcile this project's native Codex conversations before removing it."
                    .into(),
                vec![],
            );
            self.render_agent_panel();
            return;
        }
        let checkpoint_run_ids = self.workspace_checkpoint_run_ids(root);
        if !checkpoint_run_ids.is_empty() {
            if !stop_active_runs {
                self.push_chat_message(
                    ChatRole::System,
                    "This project still has an active agent or unfinished Time Machine checkpoint. Confirm “Stop agent and remove” to stop its work before removing the project and its chats."
                        .to_owned(),
                    Vec::new(),
                );
                self.render_agent_panel();
                return;
            }

            self.pending_workspace_ejections.insert(root.to_owned());
            if let Err(error) = self.cancel_submissions_for_workspace(root) {
                self.pending_workspace_ejections.remove(root);
                self.push_chat_message(ChatRole::System, error, vec![]);
                self.render_agent_panel();
                return;
            }
            let active_run_ids = checkpoint_run_ids
                .iter()
                .copied()
                .filter(|run_id| {
                    self.agent_run_for_id(*run_id)
                        .is_some_and(AgentRun::is_active)
                })
                .collect::<Vec<_>>();
            for run_id in active_run_ids {
                self.stop_agent_run_by_id(run_id);
            }
            self.start_orphaned_checkpoint_finalizations(&checkpoint_run_ids);
            self.render_agent_panel();
            return;
        }

        self.eject_workspace_now(root);
        self.render_agent_panel();
    }

    fn eject_workspace_now(&mut self, root: &str) {
        if let Err(error) = self.cancel_submissions_for_workspace(root) {
            self.pending_workspace_ejections.remove(root);
            self.push_chat_message(ChatRole::System, error, vec![]);
            return;
        }
        self.sync_active_project_chat();
        let active_project_removed = self
            .workspace
            .root()
            .is_some_and(|active| active.display().to_string() == root);
        let surface_owners = self
            .project_chats
            .iter()
            .filter(|chat| chat.project_root == root)
            .map(|chat| format!("chat:{}", chat.id))
            .chain(std::iter::once(format!("draft:{root}")))
            .collect::<Vec<_>>();
        if let Err(error) = self.validate_app_server_unlink_owners(&surface_owners) {
            self.pending_workspace_ejections.remove(root);
            self.push_chat_message(
                ChatRole::System,
                format!("Project retained: {error}"),
                vec![],
            );
            return;
        }
        for owner in &surface_owners {
            if let Err(error) = self.forget_composer_draft(owner) {
                self.pending_workspace_ejections.remove(root);
                self.push_chat_message(
                    ChatRole::System,
                    format!("Project retained: could not clear all local drafts. {error}"),
                    vec![],
                );
                return;
            }
        }
        if let Err(error) = self.unlink_app_server_owners(&surface_owners) {
            self.pending_workspace_ejections.remove(root);
            self.push_chat_message(
                ChatRole::System,
                format!("Project retained: {error}"),
                vec![],
            );
            return;
        }
        if let Err(error) = self.workspace.eject_project(root) {
            self.pending_workspace_ejections.remove(root);
            self.push_chat_message(ChatRole::System, error, Vec::new());
            return;
        }
        self.pending_workspace_ejections.remove(root);
        let removed_chat_ids = self
            .project_chats
            .iter()
            .filter(|chat| chat.project_root == root)
            .map(|chat| chat.id.clone())
            .collect::<Vec<_>>();
        for id in &removed_chat_ids {
            self.forget_chat_messages(id);
        }
        self.project_chats.retain(|chat| chat.project_root != root);
        self.workspace_project_labels.remove(root);
        self.pinned_workspace_roots.remove(root);
        self.workspace_last_opened_ms.remove(root);
        self.project_metadata.remove(root);
        self.project_scans_in_flight.remove(root);
        if self.project_import_inspection_key.as_deref() == Some(root) {
            self.project_import = ProjectImportView::default();
            self.project_import_inspection_key = None;
        }
        if active_project_removed {
            self.select_latest_chat_for_active_project();
        }
        self.permission_policy
            .revoke_scope(CapabilityScope::Workspace);
        self.terminal.reset_default_cwd();
        if self.workspace.root().is_none() {
            self.load_project_chat_state(None);
        }
        self.save_session();
    }

    fn finish_pending_workspace_ejections(&mut self) {
        let ready = self
            .pending_workspace_ejections
            .iter()
            .filter(|root| self.workspace_checkpoint_run_ids(root).is_empty())
            .cloned()
            .collect::<Vec<_>>();
        for root in ready {
            self.eject_workspace_now(&root);
        }
    }

    fn workspace_ui_mutation_available(&mut self) -> bool {
        if self.any_agent_active() || !self.pending_commands.is_empty() {
            self.push_chat_message(
                ChatRole::System,
                "Stop the active run and resolve pending approvals before changing project files."
                    .to_owned(),
                Vec::new(),
            );
            self.render_agent_panel();
            return false;
        }
        true
    }

    pub(crate) fn take_startup_error(&mut self) -> Option<anyhow::Error> {
        self.startup_error.take()
    }

    fn render_file_editor(&self) {
        if let Some(panel) = &self.agent_panel
            && let Ok(state) = serde_json::to_string(&self.file_editor.view())
        {
            let _ = panel.evaluate_script(&format!(
                "window.CentralAgentSvelte?.updateFileEditor({state});"
            ));
        }
    }

    fn editor_workspace(&self, root: &str) -> Result<WorkspaceRuntime, String> {
        let path = Path::new(root);
        if !self
            .workspace
            .project_roots()
            .iter()
            .any(|item| item == path)
        {
            return Err("Reconnect this file's original local project before opening, reloading or saving it. Your draft remains available.".into());
        }
        WorkspaceRuntime::scoped(path)
    }

    fn handle_editor_action(&mut self, action: crate::file_editor::EditorAction) {
        use crate::file_editor::EditorAction;
        let change_only = matches!(
            &action,
            EditorAction::Change { .. } | EditorAction::Selection { .. }
        );
        let result: Result<(), String> = (|| {
            match action {
                EditorAction::Ready => Ok(()),
                EditorAction::Open { root, path } => {
                    let workspace = self.editor_workspace(&root)?;
                    self.file_editor.open(&workspace, &path)
                }
                EditorAction::Change {
                    id,
                    content,
                    version,
                } => self.file_editor.change(&id, content, version),
                EditorAction::Selection {
                    id,
                    version,
                    start,
                    end,
                } => self.file_editor.select(
                    &id,
                    version,
                    crate::file_editor::Selection { start, end },
                ),
                EditorAction::Activate { id } => {
                    if let Some(id) = &id {
                        self.file_editor.document(id)?;
                    }
                    self.file_editor.active_id = id;
                    Ok(())
                }
                EditorAction::Save {
                    id,
                    content,
                    version,
                } => {
                    // Validate the exact visible buffer before saving, even if a previous edit was rejected.
                    self.file_editor.change(&id, content, version)?;
                    let root = self.file_editor.document(&id)?.file.root.clone();
                    if !self.workspace_checkpoint_run_ids(&root).is_empty() {
                        return Err("An agent or Time Machine checkpoint is using this project. Wait for it to finish before saving. Your edits remain in the editor.".into());
                    }
                    let workspace = self.editor_workspace(&root)?;
                    self.file_editor.save(&id, &workspace)?;
                    self.workspace.refresh_explorer();
                    self.render_agent_panel();
                    Ok(())
                }
                EditorAction::Reload { id, discard } => {
                    let root = self.file_editor.document(&id)?.file.root.clone();
                    let workspace = self.editor_workspace(&root)?;
                    self.file_editor.reload(&id, &workspace, discard)
                }
                EditorAction::Close { id, discard } => self.file_editor.close(&id, discard),
            }
        })();
        if let Err(error) = result {
            self.file_editor.status = error;
        } else if change_only {
            return;
        }
        self.render_file_editor();
    }

    fn create_workspace_file(&mut self, path: &str) {
        if !self.workspace_ui_mutation_available() {
            return;
        }
        if let Err(error) = self.workspace.create_file(path) {
            self.workspace.report_explorer_error(error);
        } else if let Some(root) = self.workspace.root() {
            self.handle_editor_action(crate::file_editor::EditorAction::Open {
                root: root.display().to_string(),
                path: path.to_owned(),
            });
        }
        self.render_agent_panel();
    }

    fn create_workspace_directory(&mut self, path: &str) {
        if !self.workspace_ui_mutation_available() {
            return;
        }
        if let Err(error) = self.workspace.create_directory(path) {
            self.workspace.report_explorer_error(error);
        }
        self.render_agent_panel();
    }

    fn import_workspace_files(&mut self, directory: &str) {
        if !self.workspace_ui_mutation_available() {
            return;
        }
        let target = match self.workspace.resolve_directory(Some(directory)) {
            Ok(target) => target,
            Err(error) => {
                self.workspace.report_explorer_error(error);
                self.render_agent_panel();
                return;
            }
        };
        let Some(paths) = rfd::FileDialog::new()
            .set_title("Import files into the active Supervisor project")
            .set_directory(target)
            .pick_files()
        else {
            return;
        };
        if let Err(error) = self.workspace.import_files(directory, paths) {
            self.workspace.report_explorer_error(error);
        }
        self.render_agent_panel();
    }

    fn file_attachment_dialog() -> rfd::FileDialog {
        rfd::FileDialog::new()
            .set_title("Attach files to Agent")
            .add_filter("Agent inputs", AGENT_FILE_DIALOG_EXTENSIONS)
            .add_filter("PNG and JPEG images", &["png", "jpg", "jpeg"])
            .add_filter("MP3 and WAV audio", &["mp3", "wav"])
            .add_filter("All files", &["*"])
    }

    fn select_chat_files(&mut self) {
        if !self.native_files_enabled(&self.composer_owner()) {
            self.push_chat_message(
                ChatRole::System,
                "Connect Codex and select a local conversation to attach files.".into(),
                vec![],
            );
            self.render_agent_panel();
            return;
        }
        if self.draft_file_attachments.len() >= MAX_FILE_ATTACHMENTS {
            self.push_chat_message(
                ChatRole::System,
                format!("You can attach at most {MAX_FILE_ATTACHMENTS} files."),
                Vec::new(),
            );
            self.render_agent_panel();
            return;
        }

        let Some(paths) = Self::file_attachment_dialog().pick_files() else {
            return;
        };
        self.add_chat_file_paths(paths);
    }

    fn add_chat_file_paths(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }

        let mut errors = Vec::new();
        for path in paths {
            if self.draft_file_attachments.len() >= MAX_FILE_ATTACHMENTS {
                errors.push(format!(
                    "Only the first {MAX_FILE_ATTACHMENTS} supported files were attached."
                ));
                break;
            }
            match PendingFileAttachment::load(&path) {
                Ok(attachment) => self.draft_file_attachments.push(attachment),
                Err(error) => errors.push(error),
            }
        }
        for error in errors.into_iter().take(4) {
            self.push_chat_message(ChatRole::System, error, Vec::new());
        }
        self.render_agent_panel();
    }

    fn remove_chat_file(&mut self, file_id: &str) {
        self.draft_file_attachments
            .retain(|attachment| attachment.id() != file_id);
        self.render_agent_panel();
    }

    fn clear_workspace(&mut self) {
        if self.any_agent_active() || !self.pending_commands.is_empty() {
            self.push_chat_message(
                ChatRole::System,
                "Stop the active run and resolve pending approvals before disconnecting projects."
                    .to_owned(),
                Vec::new(),
            );
            self.render_agent_panel();
            return;
        }
        let owners = self
            .agent_submission_queue
            .iter()
            .chain(self.pending_agent_submissions.iter())
            .filter_map(|submission| submission.owner().map(str::to_owned))
            .collect::<HashSet<_>>();
        for owner in owners {
            if let Err(error) = self.cancel_submissions_for_owner(&owner, true, true) {
                self.push_chat_message(ChatRole::System, error, vec![]);
                self.render_agent_panel();
                return;
            }
        }
        let native_owners = self
            .app_server
            .conversations
            .saved()
            .bindings
            .keys()
            .filter(|owner| owner.starts_with("chat:") || owner.starts_with("draft:"))
            .cloned()
            .collect::<Vec<_>>();
        if let Err(error) = self.unlink_app_server_owners(&native_owners) {
            self.push_chat_message(ChatRole::System, error, vec![]);
            self.render_agent_panel();
            return;
        }
        self.sync_active_project_chat();
        self.workspace.disconnect();
        self.workspace_project_labels.clear();
        self.pinned_workspace_roots.clear();
        self.workspace_last_opened_ms.clear();
        self.remote_projects.clear();
        self.project_metadata.clear();
        self.project_scans_in_flight.clear();
        self.project_chats.clear();
        self.chat_messages.clear();
        self.chat_ownership = chat_ownership::ChatOwnership::default();
        self.load_project_chat_state(None);
        self.permission_policy
            .revoke_scope(CapabilityScope::Workspace);
        self.terminal.reset_default_cwd();
        self.save_session();
        self.render_agent_panel();
    }

    fn start_ui_development_watcher(&mut self) {
        if self.ui_development_watcher_started {
            return;
        }
        let Some(root) = self.ui_development_root.clone() else {
            return;
        };
        self.ui_development_watcher_started = true;
        let proxy = self.proxy.clone();
        info!(root = %root.display(), "live UI development is active");
        ui_development::spawn_watcher(root, move |changed| {
            proxy
                .send_event(BrowserEvent::DevelopmentUiChanged(changed))
                .is_ok()
        });
    }

    fn reload_development_ui(&mut self, changed: &[String]) {
        let Some(development_root) = self.ui_development_root.as_deref() else {
            return;
        };
        let targets = development_ui_reload_targets(changed);
        if targets == DevelopmentUiReloadTargets::default() {
            return;
        }
        info!(files = ?changed, "reloading changed development UI surfaces");

        if targets.backdrop {
            let html = themed_ui_asset_html(
                Some(development_root),
                "assets/backdrop.html",
                BACKDROP_HTML,
                &self.theme,
            );
            if let Some(webview) = self.backdrop.as_ref() {
                load_development_html(webview, &html, "window backdrop");
            }
        }
        if targets.toolbar {
            let html = themed_ui_asset_html(
                Some(development_root),
                "assets/toolbar.html",
                TOOLBAR_HTML,
                &self.theme,
            );
            if let Some(webview) = self.toolbar.as_ref()
                && load_development_html(webview, &html, "toolbar")
            {
                self.toolbar_ready = false;
            }
        }
        if targets.agent_panel {
            let html = themed_ui_asset_html(
                Some(development_root),
                "assets/agent-panel.html",
                AGENT_PANEL_HTML,
                &self.theme,
            );
            if let Some(webview) = self.agent_panel.as_ref()
                && load_development_html(webview, &html, "agent panel")
            {
                self.agent_panel_ready = false;
            }
        }
        if targets.agent_graph_surface {
            let html = themed_ui_asset_html(
                Some(development_root),
                "assets/agent-graph.html",
                AGENT_GRAPH_HTML,
                &self.theme,
            );
            if let Some(webview) = self.agent_graph_surface.as_ref()
                && load_development_html(webview, &html, "agent graph surface")
            {
                self.agent_graph_surface_ready = false;
            }
        }
        if targets.terminal_panel {
            let html = terminal_panel_asset_html(Some(development_root), &self.theme);
            if let Some(webview) = self.terminal_panel.as_ref()
                && load_development_html(webview, &html, "terminal panel")
            {
                self.terminal_panel_ready = false;
            }
        }
        if targets.preview_panel {
            let html = themed_ui_asset_html(
                Some(development_root),
                "assets/preview.html",
                PREVIEW_HTML,
                &self.theme,
            );
            if let Some(webview) = self.preview_panel.as_ref()
                && load_development_html(webview, &html, "preview panel")
            {
                self.preview_panel_ready = false;
            }
        }
        if targets.start_pages {
            let html = self.start_page_html();
            for tab in self.tabs.iter_mut().filter(|tab| tab.is_start_page) {
                if load_development_html(&tab.webview, &html, "start page") {
                    tab.loading = true;
                }
            }
        }
        if targets.remote_desktop {
            let html = themed_ui_asset_html(
                Some(development_root),
                "assets/remote-desktop.html",
                REMOTE_DESKTOP_HTML,
                &self.theme,
            );
            let reloaded = self
                .remote_desktop_tab_id
                .and_then(|tab_id| self.tabs.iter_mut().find(|tab| tab.id == tab_id))
                .is_some_and(|tab| {
                    let loaded = load_development_html(&tab.webview, &html, "remote desktop panel");
                    if loaded {
                        tab.loading = true;
                    }
                    loaded
                });
            if reloaded {
                self.remote_desktop_ready = false;
            }
        }
        if targets.start_pages || targets.remote_desktop {
            self.render_toolbar();
        }
    }

    fn refresh_start_pages(&self) {
        let engine = search_engine(&self.search_engine);
        let Ok(theme) = serde_json::to_string(&self.theme) else {
            return;
        };
        let Ok(action) = serde_json::to_string(engine.action()) else {
            return;
        };
        let Ok(parameter) = serde_json::to_string(engine.query_parameter()) else {
            return;
        };
        let script = format!(
            "window.applyCentralAgentPreferences && window.applyCentralAgentPreferences({theme}, {action}, {parameter});"
        );
        for tab in &self.tabs {
            if tab.is_start_page
                && let Err(error) = tab.webview.evaluate_script(&script)
            {
                warn!(%error, tab_id = tab.id, "failed to refresh the start page");
            }
        }
    }

    fn apply_webview_backgrounds(&self) {
        let color = theme_surface_color(&self.theme);
        for (name, webview) in [
            ("window backdrop", self.backdrop.as_ref()),
            ("toolbar", self.toolbar.as_ref()),
            ("agent panel", self.agent_panel.as_ref()),
            ("terminal panel", self.terminal_panel.as_ref()),
            ("preview panel", self.preview_panel.as_ref()),
        ] {
            if let Some(webview) = webview
                && let Err(error) = webview.set_background_color(color)
            {
                warn!(%error, %name, "webview background was not updated");
            }
        }
        if let Some(backdrop) = &self.backdrop
            && let Ok(theme) = serde_json::to_string(&self.theme)
            && let Err(error) = backdrop.evaluate_script(&format!(
                "document.documentElement.dataset.theme = {theme};"
            ))
        {
            warn!(%error, "window backdrop theme was not updated");
        }
        for tab in &self.tabs {
            if let Err(error) = tab.webview.set_background_color(color) {
                warn!(%error, tab_id = tab.id, "tab background was not updated");
            }
        }
    }

    fn apply_native_window_themes(&self) {
        if let Some(window) = self.window.as_ref() {
            apply_native_window_theme(window, &self.theme);
        }
        for tab in &self.tabs {
            if let Some(window) = tab.detached_window.as_ref() {
                apply_native_window_theme(window, &self.theme);
            }
        }
        for window in [
            self.agent_panel_window.as_ref(),
            self.terminal_window.as_ref(),
            self.preview_window.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            apply_native_window_theme(window, &self.theme);
        }
    }

    fn start_page_html(&self) -> String {
        let engine = search_engine(&self.search_engine);
        ui_html_with_assets(
            self.ui_development_root.as_deref(),
            "assets/start-page.html",
            START_PAGE_HTML,
        )
        .replace("__THEME_ID__", &self.theme)
        .replace("__SEARCH_ACTION__", engine.action())
        .replace("__SEARCH_PARAMETER__", engine.query_parameter())
    }

    fn start_claude_probe(&mut self) {
        self.refresh_claude_catalog(false);
    }

    fn refresh_claude_catalog(&mut self, automatic: bool) {
        let busy = self.has_claude_jobs() || self.claude_login_in_progress;
        if !self
            .claude_catalog_refresh
            .begin(Instant::now(), automatic, busy)
        {
            return;
        }
        if !automatic {
            self.claude_provider = claude_provider::checking_view();
            self.render_agent_panel();
        }
        let proxy = self.proxy.clone();
        claude_provider::probe_async(move |provider| {
            let _ = proxy.send_event(BrowserEvent::ClaudeProviderProbed(provider));
        });
    }

    fn handle_claude_provider_probed(&mut self, result: ClaudeProbeResult) {
        let now = Instant::now();
        let busy = self.has_claude_jobs() || self.claude_login_in_progress;
        let Some(automatic) = self.claude_catalog_refresh.complete(now, busy) else {
            return;
        };
        let update = model_catalog::apply_catalog(
            &mut self.claude_provider,
            &mut self.claude_models,
            &mut self.claude_selection,
            result.provider,
            result.models,
            automatic,
        );
        self.claude_catalog_refresh
            .schedule_next(now, update.loaded);
        if update.selection_changed {
            self.save_session();
        }
        self.annotate_claude_usage();
        self.render_agent_panel();
    }

    fn connect_claude_provider(&mut self) {
        if self.any_agent_active()
            || self.claude_login_in_progress
            || self.claude_catalog_refresh.is_refreshing()
            || !self.pending_commands.is_empty()
            || !self.pending_claude_permissions.is_empty()
            || !self.pending_subscription_permissions.is_empty()
        {
            self.render_agent_panel();
            return;
        }
        self.claude_provider = claude_provider::checking_view();
        self.claude_login_in_progress = true;
        self.claude_provider.detail =
            "Complete the official Claude Code sign-in in the opened terminal…".to_owned();
        self.render_agent_panel();
        let proxy = self.proxy.clone();
        claude_provider::login_async(move |result| {
            let _ = proxy.send_event(BrowserEvent::ClaudeLoginCompleted(result));
        });
    }

    fn handle_claude_login_completed(&mut self, result: Result<(), String>) {
        self.claude_login_in_progress = false;
        match result {
            Ok(()) => self.start_claude_probe(),
            Err(message) => {
                self.claude_provider =
                    AgentProviderView::error(message, self.claude_provider.version.clone());
                self.render_agent_panel();
            }
        }
    }

    fn set_agent_provider(&mut self, provider: AgentProviderKind) {
        if self.active_main_busy() {
            self.render_agent_panel();
            return;
        }
        self.agent_provider = provider;
        if let Some(run) = self.active_main_run_mut() {
            run.context_usage = None;
        }
        self.save_session();
        self.render_agent_panel();
    }

    fn set_agent_configuration(&mut self, selection: AgentSelection) {
        if self.active_main_busy() {
            self.render_agent_panel();
            return;
        }
        let (models, target, provider_name) = match self.agent_provider {
            AgentProviderKind::CodexAppServer => {
                if let Err(error) = self.configure_app_server(selection) {
                    self.conversation_event(
                        "central-agent:conversation-error",
                        json!({"owner":self.composer_owner(),"error":error}),
                    );
                }
                self.save_session();
                self.render_agent_panel();
                return;
            }
            AgentProviderKind::ClaudeCode => (
                &self.claude_models,
                &mut self.claude_selection,
                "Claude Code",
            ),
            AgentProviderKind::Cursor => {
                (&self.cursor_models, &mut self.cursor_selection, "Cursor")
            }
            AgentProviderKind::GithubCopilot => (
                &self.github_copilot_models,
                &mut self.github_copilot_selection,
                "GitHub Copilot",
            ),
            AgentProviderKind::GoogleAntigravity => (
                &self.google_antigravity_models,
                &mut self.google_antigravity_selection,
                "Google AI Pro / Ultra",
            ),
            AgentProviderKind::OpencodeGo => (
                &self.opencode_go_models,
                &mut self.opencode_go_selection,
                "OpenCode Go",
            ),
        };
        if let Err(error) = provider_types::validate_selection(models, &selection) {
            warn!(%error, provider = provider_name, "agent configuration rejected");
            self.render_agent_panel();
            return;
        }
        *target = selection;
        if let Some(run) = self.active_main_run_mut() {
            run.context_usage = None;
        }
        self.save_session();
        self.render_agent_panel();
    }

    fn reset_claude_session(&mut self) {
        if self.active_main_busy() {
            return;
        }
        self.claude_session_id = None;
        self.claude_session_cwd = None;
        self.save_session();
        self.render_agent_panel();
    }

    fn start_subscription_probe(&mut self, provider: AgentProviderKind) {
        if !provider.uses_subscription_adapter() || self.has_subscription_jobs(provider) {
            return;
        }
        if let Some((view, _, _)) = self.subscription_state_mut(provider) {
            *view = subscription_provider::checking_view(provider);
        }
        self.render_agent_panel();
        let proxy = self.proxy.clone();
        subscription_provider::probe_async(provider, move |result| {
            let _ = proxy.send_event(BrowserEvent::SubscriptionProviderProbed { provider, result });
        });
    }

    fn handle_subscription_provider_probed(
        &mut self,
        provider: AgentProviderKind,
        result: SubscriptionProbeResult,
    ) {
        if self.has_subscription_jobs(provider) {
            return;
        }
        let mut changed = false;
        if let Some((view, models, selection)) = self.subscription_state_mut(provider) {
            *view = result.provider;
            *models = result.models;
            if view.can_run() {
                match provider_types::normalize_selection(models, selection) {
                    Some(normalized) => {
                        changed = normalized != *selection;
                        *selection = normalized;
                    }
                    None => {
                        *view = AgentProviderView::error(
                            format!(
                                "{} did not return a usable model configuration",
                                provider.label()
                            ),
                            view.version.clone(),
                        );
                    }
                }
            }
        }
        if changed {
            self.save_session();
        }
        self.render_agent_panel();
    }

    fn connect_subscription_provider(&mut self, provider: AgentProviderKind) {
        if !provider.uses_subscription_adapter()
            || self.any_agent_active()
            || !self.pending_commands.is_empty()
            || !self.pending_claude_permissions.is_empty()
            || !self.pending_subscription_permissions.is_empty()
        {
            self.render_agent_panel();
            return;
        }
        if let Some((view, _, _)) = self.subscription_state_mut(provider) {
            *view = subscription_provider::checking_view(provider);
            view.detail = format!(
                "Complete the official {} connection in the opened terminal…",
                provider.label()
            );
        }
        self.render_agent_panel();
        let proxy = self.proxy.clone();
        subscription_provider::login_async(provider, move |result| {
            let _ = proxy.send_event(BrowserEvent::SubscriptionLoginCompleted { provider, result });
        });
    }

    fn handle_subscription_login_completed(
        &mut self,
        provider: AgentProviderKind,
        result: Result<(), String>,
    ) {
        match result {
            Ok(()) => self.start_subscription_probe(provider),
            Err(message) => {
                if let Some((view, _, _)) = self.subscription_state_mut(provider) {
                    *view = AgentProviderView::error(message, view.version.clone());
                }
                self.render_agent_panel();
            }
        }
    }

    fn subscription_state(
        &self,
        provider: AgentProviderKind,
    ) -> Option<(&AgentProviderView, &[AgentModelOption], &AgentSelection)> {
        match provider {
            AgentProviderKind::Cursor => Some((
                &self.cursor_provider,
                &self.cursor_models,
                &self.cursor_selection,
            )),
            AgentProviderKind::GithubCopilot => Some((
                &self.github_copilot_provider,
                &self.github_copilot_models,
                &self.github_copilot_selection,
            )),
            AgentProviderKind::GoogleAntigravity => Some((
                &self.google_antigravity_provider,
                &self.google_antigravity_models,
                &self.google_antigravity_selection,
            )),
            AgentProviderKind::OpencodeGo => Some((
                &self.opencode_go_provider,
                &self.opencode_go_models,
                &self.opencode_go_selection,
            )),
            AgentProviderKind::ClaudeCode | AgentProviderKind::CodexAppServer => None,
        }
    }

    fn provider_configuration(
        &self,
        provider: AgentProviderKind,
    ) -> (&AgentProviderView, &[AgentModelOption], &AgentSelection) {
        match provider {
            AgentProviderKind::ClaudeCode => (
                &self.claude_provider,
                &self.claude_models,
                &self.claude_selection,
            ),
            AgentProviderKind::CodexAppServer => self.app_server.configuration(),
            provider => self.subscription_state(provider).unwrap_or((
                &self.claude_provider,
                &self.claude_models,
                &self.claude_selection,
            )),
        }
    }

    fn subscription_state_mut(
        &mut self,
        provider: AgentProviderKind,
    ) -> Option<(
        &mut AgentProviderView,
        &mut Vec<AgentModelOption>,
        &mut AgentSelection,
    )> {
        match provider {
            AgentProviderKind::Cursor => Some((
                &mut self.cursor_provider,
                &mut self.cursor_models,
                &mut self.cursor_selection,
            )),
            AgentProviderKind::GithubCopilot => Some((
                &mut self.github_copilot_provider,
                &mut self.github_copilot_models,
                &mut self.github_copilot_selection,
            )),
            AgentProviderKind::GoogleAntigravity => Some((
                &mut self.google_antigravity_provider,
                &mut self.google_antigravity_models,
                &mut self.google_antigravity_selection,
            )),
            AgentProviderKind::OpencodeGo => Some((
                &mut self.opencode_go_provider,
                &mut self.opencode_go_models,
                &mut self.opencode_go_selection,
            )),
            AgentProviderKind::ClaudeCode | AgentProviderKind::CodexAppServer => None,
        }
    }

    fn active_provider_view(&self) -> &AgentProviderView {
        match self.agent_provider {
            AgentProviderKind::CodexAppServer => self.app_server.configuration().0,
            AgentProviderKind::ClaudeCode => &self.claude_provider,
            provider => self
                .subscription_state(provider)
                .map(|state| state.0)
                .unwrap_or(&self.claude_provider),
        }
    }

    fn active_agent_models(&self) -> &[AgentModelOption] {
        match self.agent_provider {
            AgentProviderKind::CodexAppServer => self.app_server.configuration().1,
            AgentProviderKind::ClaudeCode => &self.claude_models,
            provider => self
                .subscription_state(provider)
                .map(|state| state.1)
                .unwrap_or(&self.claude_models),
        }
    }

    fn active_agent_selection(&self) -> &AgentSelection {
        match self.agent_provider {
            AgentProviderKind::CodexAppServer => self.app_server.configuration().2,
            AgentProviderKind::ClaudeCode => &self.claude_selection,
            provider => self
                .subscription_state(provider)
                .map(|state| state.2)
                .unwrap_or(&self.claude_selection),
        }
    }

    fn pending_approval_view(&self) -> Option<PendingCommandView> {
        self.active_main_run()
            .and_then(|run| self.pending_approval_for_run(run.id))
            .or_else(|| {
                self.pending_commands
                    .values()
                    .find(|request| self.run_id_for_request(&request.request_id).is_none())
                    .map(|request| {
                        let authorization = self.command_authorization(&request.command);
                        PendingCommandView {
                            request_id: request.request_id.clone(),
                            action: request.command.name().to_owned(),
                            summary: request.command.summary(),
                            scope: authorization.scope,
                            effect: authorization.effect,
                        }
                    })
            })
    }

    fn pending_approval_for_run(&self, run_id: u64) -> Option<PendingCommandView> {
        self.pending_claude_permissions
            .values()
            .find(|request| request.run_id == run_id)
            .map(|request| PendingCommandView {
                request_id: request.request_id.clone(),
                action: request.tool_name.clone(),
                summary: request.summary.clone(),
                scope: request.authorization.scope,
                effect: request.authorization.effect,
            })
            .or_else(|| {
                self.pending_subscription_permissions
                    .values()
                    .find(|request| request.run_id == run_id)
                    .map(|request| PendingCommandView {
                        request_id: request.request_id.clone(),
                        action: request.tool_name.clone(),
                        summary: request.summary.clone(),
                        scope: request.authorization.scope,
                        effect: request.authorization.effect,
                    })
            })
    }

    fn capture_tab_context(&mut self, tab_id: u64, force: bool) {
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == tab_id) else {
            return;
        };
        let title = tab.title.clone();
        let url = tab.url.clone();
        let source_revision = tab.content_revision;
        let now_ms = unix_time_ms();

        if !force
            && let Some(context) = self
                .draft_contexts
                .iter()
                .find(|context| context.tab_id == tab_id)
            && (context.is_capturing()
                || (context.is_ready() && !context.is_stale(Some(source_revision), now_ms)))
        {
            self.render_agent_panel();
            return;
        }

        if !self
            .draft_contexts
            .iter()
            .any(|context| context.tab_id == tab_id)
            && self.draft_contexts.len() >= MAX_CHAT_ATTACHMENTS
        {
            self.push_chat_message(
                ChatRole::System,
                format!("You can attach at most {MAX_CHAT_ATTACHMENTS} tabs."),
                Vec::new(),
            );
            self.render_agent_panel();
            return;
        }

        let capture_id = self.next_context_capture_id;
        self.next_context_capture_id += 1;
        let context =
            TabContextSnapshot::capturing(tab_id, capture_id, &title, &url, source_revision);
        if let Some(index) = self
            .draft_contexts
            .iter()
            .position(|context| context.tab_id == tab_id)
        {
            self.draft_contexts[index] = context;
        } else {
            self.draft_contexts.push(context);
        }
        self.render_agent_panel();

        self.request_tab_context_capture(tab_id, capture_id);
    }

    fn request_tab_context_capture(&mut self, tab_id: u64, capture_id: u64) {
        let proxy = self.proxy.clone();
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == tab_id) else {
            self.fail_tab_context_capture(
                tab_id,
                capture_id,
                "The tab is no longer available".to_owned(),
            );
            return;
        };
        if let Err(error) = tab.webview.evaluate_script_with_callback(
            agent_scripts::CAPTURE_TAB_CONTEXT,
            move |raw_result| {
                let _ = proxy.send_event(BrowserEvent::TabContextResult {
                    tab_id,
                    capture_id,
                    raw_result,
                });
            },
        ) {
            self.fail_tab_context_capture(tab_id, capture_id, format!("Capture failed: {error}"));
        }
    }

    fn remove_tab_context(&mut self, tab_id: u64) {
        self.draft_contexts
            .retain(|context| context.tab_id != tab_id);
        self.render_agent_panel();
    }

    fn fail_tab_context_capture(&mut self, tab_id: u64, capture_id: u64, message: String) {
        if let Some(context) = self.tab_draft_mut(tab_id, capture_id) {
            context.fail(limit_message(&message, 280));
            self.render_agent_panel();
        }
        self.emit_graph_contexts();
    }

    fn handle_tab_context_result(&mut self, tab_id: u64, capture_id: u64, raw_result: String) {
        let Some(context) = self.tab_draft(tab_id, capture_id) else {
            return;
        };
        let source_revision = context.source_revision;
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == tab_id) else {
            self.forget_tab_drafts(tab_id);
            self.render_agent_panel();
            return;
        };
        let fallback_title = tab.title.clone();
        let fallback_url = tab.url.clone();

        let snapshot = match parse_script_bridge_result(&raw_result) {
            Ok(result) if result.ok => TabContextSnapshot::from_value(
                tab_id,
                capture_id,
                &fallback_title,
                &fallback_url,
                source_revision,
                unix_time_ms(),
                result.data,
            ),
            Ok(result) => Err(result
                .error
                .map(|message| limit_message(&message, 280))
                .unwrap_or_else(|| "The page rejected the capture".to_owned())),
            Err(error) => Err(format!("Invalid page response: {error}")),
        };

        match snapshot {
            Ok(mut snapshot) => {
                #[cfg(windows)]
                {
                    let page_width = snapshot.page_width;
                    let page_height = snapshot.page_height;
                    snapshot.begin_visual_capture();
                    if let Some(context) = self.tab_draft_mut(tab_id, capture_id) {
                        *context = snapshot;
                    }
                    if let Err(message) =
                        self.request_tab_visual_capture(tab_id, capture_id, page_width, page_height)
                    {
                        warn!(%message, "visual tab snapshot unavailable");
                        if let Some(context) = self.tab_draft_mut(tab_id, capture_id) {
                            context.finish_visual_capture(None, true);
                        }
                    }
                }
                #[cfg(not(windows))]
                {
                    if let Some(context) = self.tab_draft_mut(tab_id, capture_id) {
                        *context = snapshot;
                    }
                }
            }
            Err(message) => self.fail_tab_context_capture(tab_id, capture_id, message),
        }
        self.emit_graph_contexts();
        self.render_agent_panel();
    }

    #[cfg(windows)]
    fn request_tab_visual_capture(
        &self,
        tab_id: u64,
        capture_id: u64,
        page_width: u32,
        page_height: u32,
    ) -> Result<(), String> {
        let tab = self
            .tabs
            .iter()
            .find(|tab| tab.id == tab_id)
            .ok_or_else(|| "The tab is no longer available".to_owned())?;
        let (params, truncated) = visual_capture_params(page_width, page_height);
        let proxy = self.proxy.clone();
        let handler = CallDevToolsProtocolMethodCompletedHandler::create(Box::new(
            move |result, raw_result| {
                let raw_result = match result {
                    Ok(()) => raw_result,
                    Err(error) => json!({ "error": error.to_string() }).to_string(),
                };
                let _ = proxy.send_event(BrowserEvent::TabVisualResult {
                    tab_id,
                    capture_id,
                    raw_result,
                    truncated,
                });
                Ok(())
            },
        ));
        let method = HSTRING::from("Page.captureScreenshot");
        let params = HSTRING::from(params);
        unsafe {
            tab.webview
                .webview()
                .CallDevToolsProtocolMethod(&method, &params, &handler)
                .map_err(|error| format!("Visual capture failed: {error}"))
        }
    }

    fn handle_tab_visual_result(
        &mut self,
        tab_id: u64,
        capture_id: u64,
        raw_result: String,
        truncated: bool,
    ) {
        let Some(context) = self.tab_draft_mut(tab_id, capture_id) else {
            return;
        };
        match decode_visual_capture(&raw_result) {
            Ok(jpeg) => context.finish_visual_capture(Some(jpeg), truncated),
            Err(message) => {
                warn!(%message, "visual tab snapshot was discarded");
                context.finish_visual_capture(None, true);
            }
        }
        self.emit_graph_contexts();
        self.render_agent_panel();
    }

    fn capture_terminal_context(&mut self, session_id: u64, force: bool) {
        let existing_index = self
            .draft_terminal_contexts
            .iter()
            .position(|context| context.snapshot.session_id == session_id);
        if existing_index.is_none()
            && self.draft_terminal_contexts.len() >= MAX_TERMINAL_CHAT_ATTACHMENTS
        {
            self.push_chat_message(
                ChatRole::System,
                format!("You can attach at most {MAX_TERMINAL_CHAT_ATTACHMENTS} terminal output snapshots."),
                Vec::new(),
            );
            self.render_agent_panel();
            return;
        }
        if existing_index.is_some() && !force {
            self.render_agent_panel();
            return;
        }

        match self.terminal.context_snapshot(session_id, unix_time_ms()) {
            Ok(snapshot) => {
                if let Some(index) = existing_index {
                    let follow_live = self.draft_terminal_contexts[index].follow_live;
                    self.draft_terminal_contexts[index] = DraftTerminalContext {
                        snapshot,
                        follow_live,
                        last_live_render_ms: unix_time_ms(),
                        live_update_scheduled: false,
                    };
                } else {
                    self.draft_terminal_contexts
                        .push(DraftTerminalContext::attached(snapshot, unix_time_ms()));
                }
            }
            Err(error) => self.push_chat_message(ChatRole::System, error, Vec::new()),
        }
        self.render_agent_panel();
    }

    fn remove_terminal_context(&mut self, session_id: u64) {
        self.draft_terminal_contexts
            .retain(|context| context.snapshot.session_id != session_id);
        self.render_agent_panel();
    }

    fn set_terminal_context_follow(&mut self, session_id: u64, enabled: bool) {
        let Some(index) = self
            .draft_terminal_contexts
            .iter()
            .position(|context| context.snapshot.session_id == session_id)
        else {
            return;
        };
        if enabled {
            match self.terminal.context_snapshot(session_id, unix_time_ms()) {
                Ok(snapshot) => {
                    self.draft_terminal_contexts[index].snapshot = snapshot;
                    self.draft_terminal_contexts[index].follow_live = true;
                    self.draft_terminal_contexts[index].last_live_render_ms = unix_time_ms();
                    self.draft_terminal_contexts[index].live_update_scheduled = false;
                }
                Err(error) => {
                    self.draft_terminal_contexts[index].follow_live = false;
                    self.push_chat_message(ChatRole::System, error, Vec::new());
                }
            }
        } else {
            self.draft_terminal_contexts[index].follow_live = false;
        }
        self.render_agent_panel();
    }

    fn refresh_following_terminal_context(&mut self, session_id: u64, force: bool) {
        self.refresh_graph_shells(session_id, force);
        let Some(index) = self
            .draft_terminal_contexts
            .iter()
            .position(|context| context.snapshot.session_id == session_id && context.follow_live)
        else {
            return;
        };
        let now_ms = unix_time_ms();
        let elapsed =
            now_ms.saturating_sub(self.draft_terminal_contexts[index].last_live_render_ms);
        if !force && elapsed < TERMINAL_LIVE_UI_INTERVAL_MS {
            if !self.draft_terminal_contexts[index].live_update_scheduled {
                self.draft_terminal_contexts[index].live_update_scheduled = true;
                let delay_ms = u64::try_from(TERMINAL_LIVE_UI_INTERVAL_MS - elapsed).unwrap_or(300);
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(delay_ms));
                    let _ = proxy.send_event(BrowserEvent::TerminalContextRefresh { session_id });
                });
            }
            return;
        }
        self.draft_terminal_contexts[index].live_update_scheduled = false;
        match self.terminal.context_snapshot(session_id, now_ms) {
            Ok(snapshot) => {
                self.draft_terminal_contexts[index].snapshot = snapshot;
                self.draft_terminal_contexts[index].last_live_render_ms = now_ms;
                self.stream_terminal_context_update(session_id);
            }
            Err(_) => {
                self.draft_terminal_contexts[index].follow_live = false;
                self.render_agent_panel();
            }
        }
    }

    fn freeze_terminal_context(&mut self, session_id: u64) {
        self.refresh_following_terminal_context(session_id, true);
        for draft in self.graph_contexts.drafts.values_mut() {
            for context in &mut draft.terminals {
                if context.snapshot.session_id == session_id {
                    context.follow_live = false;
                    context.live_update_scheduled = false;
                }
            }
        }
        if let Some(context) = self
            .draft_terminal_contexts
            .iter_mut()
            .find(|context| context.snapshot.session_id == session_id)
        {
            context.follow_live = false;
        }
    }

    fn submit_chat_message(
        &mut self,
        prompt: ChatSubmissionPrompt,
        delivery: AgentSubmissionDelivery,
        tab_ids: Vec<u64>,
        terminal_session_ids: Vec<u64>,
        file_ids: Vec<String>,
        timing: Option<app_server::delivery::ClientTiming>,
    ) {
        let ChatSubmissionPrompt {
            mut message,
            resume,
        } = prompt;
        let trace = app_server::delivery::Trace::new(timing);
        let provider = self.agent_provider;
        if resume {
            if delivery != AgentSubmissionDelivery::Start
                || !tab_ids.is_empty()
                || !terminal_session_ids.is_empty()
                || !file_ids.is_empty()
                || !message.trim().is_empty()
                || !self.main_agent_can_resume()
            {
                self.conversation_event(
                    "central-agent:conversation-error",
                    json!({"owner":self.composer_owner(),"error":"This response is no longer stopped. Your current draft and attachments were retained."}),
                );
                return;
            }
            message = RESUME_INTERRUPTED_PROMPT.to_owned();
        }
        if provider == AgentProviderKind::CodexAppServer
            && delivery == AgentSubmissionDelivery::Steer
        {
            self.conversation_event("central-agent:conversation-error", json!({"owner":self.composer_owner(),"error":"Native Send now requires the displayed turn ID. Reopen the delivery choices; this input was not queued or started."}));
            return;
        }
        let selection_override = match provider {
            AgentProviderKind::CodexAppServer => Some(self.app_server.configuration().2.clone()),
            AgentProviderKind::ClaudeCode => Some(self.claude_selection.clone()),
            provider => self
                .subscription_state(provider)
                .map(|state| state.2.clone()),
        };
        let submission = PendingAgentSubmission {
            delivery_trace: Some(trace),
            native_skills: vec![],
            native_apps: vec![],
            scope: None,

            snapshots: SubmissionSnapshots::default(),
            provider,
            message,
            tab_ids,
            terminal_session_ids,
            file_ids,
            selection_override,
            agent_graph_launch: None,
            card_draft: false,
            supervision_review: None,
        };
        if self.active_main_busy() {
            match delivery {
                AgentSubmissionDelivery::Steer => self.steer_active_agent_run(submission),
                AgentSubmissionDelivery::Start | AgentSubmissionDelivery::Queue => {
                    self.enqueue_agent_submission(submission)
                }
            }
        } else if !self.agent_submission_queue.is_empty() {
            self.enqueue_agent_submission(submission);
            self.start_next_agent_submission_if_idle();
        } else {
            self.submit_agent_submission(submission);
        }
    }

    fn steer_active_agent_run(&mut self, submission: PendingAgentSubmission) {
        self.enqueue_agent_submission(submission);
    }

    fn enqueue_agent_submission(&mut self, mut submission: PendingAgentSubmission) {
        if !self.accept_submission(&mut submission) {
            return;
        }
        submission.message = submission.message.trim().to_owned();
        if !submission.has_input() {
            self.push_chat_message(
                ChatRole::System,
                "The request cannot be empty.".to_owned(),
                Vec::new(),
            );
            self.render_agent_panel();
            return;
        }
        self.agent_submission_queue.push_back(submission.clone());
        self.render_agent_panel();
        self.acknowledge_submission(&submission);
    }

    fn start_next_agent_submission_if_idle(&mut self) {
        loop {
            let Some(index) =
                submission_scope::next_ready_index(&self.agent_submission_queue, |owner| {
                    !self.owner_has_running_work(owner)
                        && !self.owner_is_waiting_for_recovery(owner)
                        && !self
                            .agent_submission_queue
                            .iter()
                            .find(|entry| entry.owner() == Some(owner))
                            .is_some_and(|entry| {
                                entry.provider == AgentProviderKind::CodexAppServer
                                    && (!self.app_server.configuration().0.can_run()
                                        || self.app_server.native_config_pending())
                            })
                })
            else {
                return;
            };
            let submission = self
                .agent_submission_queue
                .remove(index)
                .expect("selected queued input");
            if submission.provider == AgentProviderKind::CodexAppServer {
                self.submit_app_server(submission, false);
            } else {
                self.submit_agent_submission(submission);
            }
        }
    }

    fn clear_agent_submission_queue(&mut self) {
        let owner = self.conversation_key(None);
        if let Err(error) = self.cancel_submissions_for_owner(&owner, true, false) {
            self.push_chat_message(
                ChatRole::System,
                format!("Could not clear the saved prompt queue: {error}"),
                vec![],
            );
        }
        self.render_agent_panel();
    }

    fn submit_agent_submission(&mut self, mut submission: PendingAgentSubmission) {
        // Native Codex never enters the legacy loop or checkpoint pipeline.
        if submission.provider == AgentProviderKind::CodexAppServer {
            self.submit_app_server(submission, true);
            return;
        }
        if !self.accept_submission(&mut submission) {
            return;
        }
        let submission_owner = submission
            .owner()
            .expect("validated submission owner")
            .to_owned();

        let submission_workspace = submission.workspace_root().map(PathBuf::from);
        let submission_visible = submission_owner == self.conversation_key(None);
        submission.message = submission.message.trim().to_owned();
        let mut message = submission.message.clone();
        let tab_ids = submission.tab_ids.clone();
        let terminal_session_ids = submission.terminal_session_ids.clone();
        let file_ids = submission.file_ids.clone();
        let selection_override = submission.selection_override.clone();
        let agent_graph_launch = submission.agent_graph_launch.clone();
        let is_agent_graph_run = agent_graph_launch.is_some();
        let run_provider = submission.provider;
        if self.owner_has_running_work(&submission_owner) {
            self.enqueue_agent_submission(submission);
            return;
        }
        if let Err(message) = validate_agent_graph_provider_location(
            run_provider,
            agent_graph_launch
                .as_ref()
                .and_then(|launch| launch.ssh_profile_id.as_deref()),
        ) {
            self.push_submission_message(&submission, ChatRole::System, message);
            self.render_agent_panel();
            return;
        }
        if self.owner_is_waiting_for_recovery(&submission_owner) {
            self.enqueue_agent_submission(submission);
            return;
        }
        let prospective_checkpoint_root = submission_workspace.clone();
        let recovery_run_id = prospective_checkpoint_root.as_ref().and_then(|root| {
            self.time_machine_run_roots
                .iter()
                .find(|(run_id, active_root)| {
                    checkpoint_roots_overlap(active_root, root)
                        && (self.retryable_time_machine_finalizations.contains(run_id)
                            || self.time_machine_finalizations_in_flight.contains(run_id))
                })
                .map(|(run_id, _)| *run_id)
        });
        if let Some(recovery_run_id) = recovery_run_id {
            self.retry_failed_time_machine_finalizations();
            self.push_submission_message(&submission,
                ChatRole::System,
                format!(
                    "Time Machine is recovering unfinished checkpoint {recovery_run_id}. This request is queued and will start automatically after recovery succeeds."
                ),
            );
            self.pending_agent_submissions.push_back(submission.clone());
            self.render_agent_panel();
            self.acknowledge_submission(&submission);
            return;
        }
        if !submission.has_input() {
            self.push_submission_message(
                &submission,
                ChatRole::System,
                "The request cannot be empty.".to_owned(),
            );
            self.render_agent_panel();
            return;
        }
        if message.is_empty() {
            message = "Review the attached context.".to_owned();
        }
        if self.owner_has_running_work(&submission_owner) {
            self.push_submission_message(
                &submission,
                ChatRole::System,
                "A run is already active. Stop it before sending a new request.".to_owned(),
            );
            self.render_agent_panel();
            return;
        }
        if !is_agent_graph_run
            && self
                .main_runs
                .latest(&submission_owner)
                .and_then(|run| run.active_request_id.as_deref())
                .is_some_and(|request_id| self.pending_commands.contains_key(request_id))
        {
            self.push_submission_message(
                &submission,
                ChatRole::System,
                "Resolve the action already awaiting approval first.".to_owned(),
            );
            self.render_agent_panel();
            return;
        }
        let provider_view = match run_provider {
            AgentProviderKind::ClaudeCode => &self.claude_provider,
            provider => self
                .subscription_state(provider)
                .map(|state| state.0)
                .unwrap_or(&self.claude_provider),
        };
        if !provider_view.can_run() {
            self.push_submission_message(
                &submission,
                ChatRole::System,
                format!(
                    "{} is not ready: {} Open Settings → AI provider to reconnect or retry.",
                    run_provider.label(),
                    provider_view.detail
                ),
            );
            self.render_agent_panel();
            return;
        }
        let requested_selection = selection_override.as_ref().unwrap_or(match run_provider {
            AgentProviderKind::ClaudeCode => &self.claude_selection,
            provider => self
                .subscription_state(provider)
                .map(|state| state.2)
                .unwrap_or(&self.claude_selection),
        });
        let provider_models = match run_provider {
            AgentProviderKind::ClaudeCode => &self.claude_models,
            provider => self
                .subscription_state(provider)
                .map(|state| state.1)
                .unwrap_or(&self.claude_models),
        };
        let selected_model =
            match provider_types::validate_selection(provider_models, requested_selection) {
                Ok(model) => model.clone(),
                Err(error) => {
                    self.push_submission_message(&submission, ChatRole::System, error);
                    self.render_agent_panel();
                    return;
                }
            };
        let selection = requested_selection.clone();
        let model_supports_images = selected_model.supports_image_input();
        let model_supports_audio = selected_model.supports_audio_input();

        if file_ids.len() > MAX_FILE_ATTACHMENTS {
            self.push_submission_message(
                &submission,
                ChatRole::System,
                format!("You can attach at most {MAX_FILE_ATTACHMENTS} files."),
            );
            self.render_agent_panel();
            return;
        }

        let mut requested_file_ids = Vec::new();
        let mut file_attachments = Vec::new();
        for file_id in file_ids {
            if requested_file_ids.contains(&file_id) {
                continue;
            }
            let Some(attachment) = submission
                .snapshots
                .files
                .iter()
                .find(|attachment| attachment.id() == file_id)
            else {
                self.push_submission_message(
                    &submission,
                    ChatRole::System,
                    "One of the file attachments does not belong to the current draft.".to_owned(),
                );
                self.render_agent_panel();
                return;
            };
            if attachment.kind() == FileAttachmentKind::Image && !model_supports_images {
                self.push_submission_message(&submission, ChatRole::System, "The selected Agent model does not accept image input. Choose an image-capable model or remove the image."
                        .to_owned());
                self.render_agent_panel();
                return;
            }
            if attachment.kind() == FileAttachmentKind::Audio && !model_supports_audio {
                self.push_submission_message(
                    &submission,
                    ChatRole::System,
                    "The selected Agent model does not accept audio input. Choose an audio-capable model or remove the audio file."
                        .to_owned(),
                );
                self.render_agent_panel();
                return;
            }
            requested_file_ids.push(file_id);
            file_attachments.push(attachment.view());
        }

        if tab_ids.len() > MAX_CHAT_ATTACHMENTS {
            self.push_submission_message(
                &submission,
                ChatRole::System,
                format!("You can attach at most {MAX_CHAT_ATTACHMENTS} tabs."),
            );
            self.render_agent_panel();
            return;
        }
        if terminal_session_ids.len() > MAX_TERMINAL_CHAT_ATTACHMENTS {
            self.push_submission_message(
                &submission,
                ChatRole::System,
                format!("You can attach at most {MAX_TERMINAL_CHAT_ATTACHMENTS} terminal output snapshots."),
            );
            self.render_agent_panel();
            return;
        }

        let mut requested_terminal_ids = Vec::new();
        for session_id in terminal_session_ids {
            if !requested_terminal_ids.contains(&session_id) {
                requested_terminal_ids.push(session_id);
            }
        }
        let mut terminal_drafts = Vec::new();
        for session_id in &requested_terminal_ids {
            let Some(context) = submission
                .snapshots
                .terminals
                .iter()
                .find(|context| context.snapshot.session_id == *session_id)
            else {
                self.push_submission_message(
                    &submission,
                    ChatRole::System,
                    "One of the shell attachments does not belong to the current draft.".to_owned(),
                );
                self.render_agent_panel();
                return;
            };
            terminal_drafts.push(context.clone());
        }

        let now_ms = unix_time_ms();
        let mut valid_tab_ids = Vec::new();
        let mut snapshots = Vec::new();
        for tab_id in tab_ids {
            if valid_tab_ids.contains(&tab_id) {
                continue;
            }
            let Some(context) = submission
                .snapshots
                .tabs
                .iter()
                .find(|context| context.tab_id == tab_id)
            else {
                self.push_submission_message(
                    &submission,
                    ChatRole::System,
                    "One of the attachments does not belong to the current draft.".to_owned(),
                );
                self.render_agent_panel();
                return;
            };
            if context.is_capturing() {
                self.push_submission_message(
                    &submission,
                    ChatRole::System,
                    "Wait for attached snapshots to finish before sending.".to_owned(),
                );
                self.render_agent_panel();
                return;
            }
            if !context.is_ready() {
                self.push_submission_message(
                    &submission,
                    ChatRole::System,
                    "Refresh or remove unavailable snapshots before sending.".to_owned(),
                );
                self.render_agent_panel();
                return;
            }
            valid_tab_ids.push(tab_id);
            snapshots.push(context.clone());
        }

        let conversation_key = submission_owner.clone();
        let chat_id = conversation_key.strip_prefix("chat:");
        let claude_session_id = if submission_visible {
            self.claude_session_id.clone()
        } else {
            self.project_chats
                .iter()
                .find(|chat| Some(chat.id.as_str()) == chat_id)
                .and_then(|chat| chat.claude_session_id.clone())
        };
        let claude_session_cwd = if submission_visible {
            self.claude_session_cwd.clone()
        } else {
            self.project_chats
                .iter()
                .find(|chat| Some(chat.id.as_str()) == chat_id)
                .and_then(|chat| chat.claude_session_cwd.clone())
        };

        // A Claude session resumes only in its own workspace. Otherwise the run starts
        // fresh and needs the local history, instead of silently losing the context.
        let claude_main_cwd = submission_workspace
            .clone()
            .unwrap_or_else(|| self.data_dir.join("claude-workspace"));
        let claude_resume_session_id = (run_provider == AgentProviderKind::ClaudeCode
            && !is_agent_graph_run
            && claude_session_cwd.as_deref()
                == Some(claude_main_cwd.display().to_string().as_str()))
        .then_some(claude_session_id)
        .flatten();
        let conversation_history = if is_agent_graph_run || claude_resume_session_id.is_some() {
            Vec::new()
        } else {
            self.project_conversation_history(chat_id)
        };
        let run_id = self.next_agent_run_id;
        self.next_agent_run_id += 1;

        self.run_timings.insert(
            run_id,
            AgentRunTiming {
                accepted_at_ms: unix_time_ms(),
                first_provider_event_at_ms: None,
            },
        );
        let agent_graph_ssh_profile_id = agent_graph_launch
            .as_ref()
            .and_then(|launch| launch.ssh_profile_id.clone());
        let native_targets = submission
            .scope
            .as_ref()
            .map(|scope| scope.native_targets.in_session(&self.native_session_id))
            .unwrap_or_default();
        self.run_execution_contexts.insert(
            run_id,
            run_context::RunExecutionContext {
                conversation_key,
                workspace_root: prospective_checkpoint_root.clone(),
                ssh_profile_id: agent_graph_ssh_profile_id.clone(),
                provider: run_provider,

                native_targets: native_targets.clone(),
                terminal_session_id: None,
            },
        );
        if agent_graph_ssh_profile_id.is_none() && submission_workspace.is_some() {
            self.live_diff_by_owner.insert(
                submission_owner.clone(),
                TrackedLiveDiff {
                    run_id,
                    view: LiveDiffView {
                        additions: 0,
                        deletions: 0,
                        file_count: 0,
                        active: true,
                    },
                },
            );
        } else {
            self.live_diff_by_owner.remove(&submission_owner);
        }
        if agent_graph_ssh_profile_id.is_none()
            && let Some(workspace_root) = submission_workspace.clone()
        {
            let checkpoint_root = agent_graph_launch
                .as_ref()
                .and_then(|launch| local_agent_graph_checkpoint_root(&workspace_root, launch))
                .unwrap_or_else(|| {
                    fs::canonicalize(&workspace_root).unwrap_or(workspace_root.clone())
                });
            let checkpoint_context = if is_agent_graph_run {
                "Agent Graph agent"
            } else {
                "Agent chat"
            };
            self.time_machine_run_plans.insert(
                run_id,
                TimeMachineRunPlan {
                    root: checkpoint_root,
                    provider: run_provider.label().to_owned(),
                    context: checkpoint_context.to_owned(),
                },
            );
        }
        let run_request = AgentRunRequest {
            run_id,
            prompt: message.clone(),
            conversation_history,
            active_tab_id: native_targets.browser_tab_id,
            tabs: native_targets.browser_tabs,
            contexts: snapshot_delivery::tabs(&snapshots),
            terminal_contexts: snapshot_delivery::terminals(&terminal_drafts),

            ssh_profiles: self
                .ssh
                .profiles()
                .iter()
                .filter(|profile| {
                    profile.agent_enabled
                        && self.terminal.has_agent_ready_ssh_profile(&profile.id)
                        && agent_graph_ssh_profile_id
                            .as_deref()
                            .is_none_or(|target_profile_id| profile.id == target_profile_id)
                })
                .map(|profile| AgentSshProfile {
                    id: profile.id.clone(),
                    name: profile.name.clone(),
                    working_directory: agent_graph_launch
                        .as_ref()
                        .filter(|launch| {
                            launch.ssh_profile_id.as_deref() == Some(profile.id.as_str())
                        })
                        .map(|launch| launch.project_directory.clone()),
                })
                .collect(),
            remote_desktop: {
                let view = self.remote_desktop.view();
                let matches_agent_graph_focus = agent_graph_ssh_profile_id
                    .as_deref()
                    .is_none_or(|profile_id| view.profile_id == Some(profile_id));
                if self.remote_desktop.agent_control_enabled()
                    && self.remote_desktop.is_connected()
                    && self.validate_remote_target(Some(run_id)).is_ok()
                    && model_supports_images
                    && matches_agent_graph_focus
                {
                    view.profile_id
                        .zip(view.profile_name)
                        .map(|(profile_id, profile_name)| AgentRemoteDesktop {
                            profile_id: profile_id.to_owned(),
                            profile_name: profile_name.to_owned(),
                            protocol: view.protocol.label().to_ascii_lowercase(),
                            width: view.width,
                            height: view.height,
                        })
                } else {
                    None
                }
            },
            selection: selection.clone(),
            workspace_enabled: submission_workspace.is_some()
                && agent_graph_ssh_profile_id.is_none(),
            window_access_enabled: self.window_access_enabled,
        };

        let attachments = snapshots
            .into_iter()
            .map(|snapshot| {
                let current_revision = self
                    .tabs
                    .iter()
                    .find(|tab| tab.id == snapshot.tab_id)
                    .map(|tab| tab.content_revision);
                let view = snapshot.view(current_revision, now_ms);
                ChatTabAttachment {
                    tab_id: view.tab_id,
                    title: view.title,
                    url: view.url,
                    preview_data_url: view.preview_data_url,
                    captured_at_ms: view.captured_at_ms,
                    text_char_count: view.text_char_count,
                    element_count: view.element_count,
                    link_count: view.link_count,
                    image_count: view.image_count,
                    visual_included: false,
                    estimated_token_count: view.estimated_token_count,
                    redaction_count: view.redaction_count,
                    truncated: view.truncated,
                    stale: view.stale,
                }
            })
            .collect::<Vec<_>>();
        let terminal_attachments = terminal_drafts
            .iter()
            .map(|context| ChatTerminalAttachment {
                session_id: context.snapshot.session_id,
                label: context.snapshot.label.clone(),
                kind: context.snapshot.kind,
                profile_id: context.snapshot.profile_id.clone(),
                remote_target: context.snapshot.remote_target.clone(),
                shell: context.snapshot.shell.clone(),
                cwd: context.snapshot.cwd.clone(),
                status: context.snapshot.status.clone(),
                phase: context.snapshot.phase,
                busy: context.snapshot.busy,
                output_preview: context.snapshot.output_preview(),
                last_exit_code: context.snapshot.last_exit_code,
                captured_at_ms: context.snapshot.captured_at_ms,
                estimated_token_count: context.snapshot.estimated_token_count,
                redaction_count: context.snapshot.redaction_count,
                truncated: context.snapshot.truncated,
                followed_live: context.follow_live,
            })
            .collect::<Vec<_>>();
        let display_message = agent_graph_launch
            .as_ref()
            .map(|launch| launch.user_request.clone())
            .unwrap_or_else(|| message.clone());
        self.push_chat_message_with_contexts(
            ChatRole::User,
            display_message,
            attachments,
            terminal_attachments,
            file_attachments,
        );
        if let Some(chat_message) = self.chat_messages.back_mut() {
            chat_message.run_id = Some(run_id);
            chat_message.provider = Some(run_provider);
            if let Some(chat_id) = submission_owner.strip_prefix("chat:") {
                self.chat_ownership.claim(chat_message.id, chat_id);
            }
        }

        if !is_agent_graph_run {
            self.save_project_chat_history();
        }

        let effective_profile_label = format!(
            "{} · {} · {}",
            selection.model,
            selection.effort,
            selection.service_tier.as_deref().unwrap_or("standard")
        );
        if let Some(launch) = agent_graph_launch {
            let mut runtime =
                AgentRun::planning_for_provider(run_id, message, run_provider.label());
            runtime.set_profile(effective_profile_label.clone());
            let turn = AgentGraphTurn {
                run_id,
                request: launch.user_request.clone(),
                provider: run_provider,
                selection: selection.clone(),
                started_at_ms: unix_time_ms(),
                finished_at_ms: None,
                phase: AgentPhase::Planning,
                status: runtime.status.clone(),
                messages: Vec::new(),
                steps: Vec::new(),
                checkpoint: None,
            };
            if let Some(session) = self
                .agent_graph_sessions
                .iter_mut()
                .find(|session| session.node_key == launch.node_key)
            {
                session.agent_name = launch.agent_name.clone();
                session.project_directory = launch.project_directory.clone();
                session.provider = run_provider;
                session.selection = selection.clone();
                session.turns.push(turn);
                session.updated_at_ms = unix_time_ms();
            }
            self.agent_graph_runs.insert(
                launch.node_key.clone(),
                AgentGraphRunPresentation {
                    node_key: launch.node_key,
                    run_id,
                    agent_name: launch.agent_name,
                    project_directory: launch.project_directory,
                    provider: run_provider,
                    selection: selection.clone(),
                    runtime,
                },
            );
            // Persist the durable turn shell before provider execution so an application exit can
            // mark it interrupted on the next startup without snapshotting in-flight writers.
            self.save_agent_graph_sessions();
            match run_provider {
                AgentProviderKind::ClaudeCode => {
                    let cwd = agent_graph_provider_working_directory(
                        submission_workspace.as_deref(),
                        self.time_machine_run_plans
                            .get(&run_id)
                            .map(|plan| &plan.root),
                        agent_graph_ssh_profile_id.is_some(),
                        &self.data_dir.join("claude-workspace"),
                    );
                    if let Err(error) = fs::create_dir_all(&cwd) {
                        self.fail_agent_graph_provider_start(
                            run_id,
                            format!("Claude Code workspace could not be prepared: {error}"),
                        );
                        return;
                    }
                    let cwd_key = cwd.display().to_string();
                    self.claude_provider =
                        claude_provider::running_view(self.claude_provider.version.clone());
                    self.annotate_claude_usage();
                    let proxy = self.proxy.clone();
                    let handle = claude_provider::run_async(
                        ClaudeRunRequest {
                            context: run_request,
                            cwd,
                            resume_session_id: None,
                        },
                        None,
                        move |event| {
                            let _ = proxy.send_event(BrowserEvent::ClaudeProvider(event));
                        },
                    );
                    self.claude_jobs.insert(
                        run_id,
                        ClaudeJob {
                            run_id,
                            cwd: cwd_key,
                            handle,
                        },
                    );
                }
                provider if provider.uses_subscription_adapter() => {
                    let cwd = agent_graph_provider_working_directory(
                        submission_workspace.as_deref(),
                        self.time_machine_run_plans
                            .get(&run_id)
                            .map(|plan| &plan.root),
                        agent_graph_ssh_profile_id.is_some(),
                        &self.data_dir.join("provider-workspace"),
                    );
                    if let Err(error) = fs::create_dir_all(&cwd) {
                        self.fail_agent_graph_provider_start(
                            run_id,
                            format!(
                                "{} workspace could not be prepared: {error}",
                                provider.label()
                            ),
                        );
                        return;
                    }
                    let version = self
                        .subscription_state(provider)
                        .and_then(|state| state.0.version.clone());
                    if let Some((view, _, _)) = self.subscription_state_mut(provider) {
                        *view = subscription_provider::running_view(provider, version);
                    }
                    let proxy = self.proxy.clone();
                    let handle = subscription_provider::run_async(
                        provider,
                        SubscriptionRunRequest {
                            context: run_request,
                            cwd,
                        },
                        None,
                        move |event| {
                            let _ = proxy
                                .send_event(BrowserEvent::SubscriptionProvider { provider, event });
                        },
                    );
                    self.subscription_jobs.insert(
                        run_id,
                        SubscriptionJob {
                            run_id,
                            provider,
                            handle,
                        },
                    );
                }
                _ => unreachable!(),
            }
            self.save_agent_graph_sessions();
            self.save_session();
        } else {
            let mut runtime =
                AgentRun::planning_for_provider(run_id, message, run_provider.label());
            runtime.set_profile(effective_profile_label);
            self.main_runs.insert(submission_owner, runtime);
            match run_provider {
                AgentProviderKind::ClaudeCode => {
                    let cwd = claude_main_cwd;
                    if let Err(error) = fs::create_dir_all(&cwd) {
                        if let Some(run) = self.agent_run_for_id_mut(run_id) {
                            run.phase = AgentPhase::Error;
                            run.status =
                                format!("Claude Code workspace could not be prepared: {error}");
                        }
                        self.finish_run_when_quiescent(run_id);
                        self.finish_run_timing(run_id, "start_error");
                        self.render_agent_panel();
                        return;
                    }
                    let cwd_key = cwd.display().to_string();
                    let resume_session_id = claude_resume_session_id;
                    self.claude_provider =
                        claude_provider::running_view(self.claude_provider.version.clone());
                    self.annotate_claude_usage();
                    let proxy = self.proxy.clone();
                    let handle = claude_provider::run_async(
                        ClaudeRunRequest {
                            context: run_request,
                            cwd,
                            resume_session_id,
                        },
                        None,
                        move |event| {
                            let _ = proxy.send_event(BrowserEvent::ClaudeProvider(event));
                        },
                    );
                    self.claude_jobs.insert(
                        run_id,
                        ClaudeJob {
                            run_id,
                            cwd: cwd_key,
                            handle,
                        },
                    );
                }
                provider if provider.uses_subscription_adapter() => {
                    let cwd = submission_workspace
                        .clone()
                        .unwrap_or_else(|| self.data_dir.join("provider-workspace"));
                    if let Err(error) = fs::create_dir_all(&cwd) {
                        if let Some(run) = self.agent_run_for_id_mut(run_id) {
                            run.phase = AgentPhase::Error;
                            run.status = format!(
                                "{} workspace could not be prepared: {error}",
                                provider.label()
                            );
                        }
                        self.finish_run_when_quiescent(run_id);
                        self.finish_run_timing(run_id, "start_error");
                        self.render_agent_panel();
                        return;
                    }
                    if let Some((view, _, _)) = self.subscription_state_mut(provider) {
                        *view = subscription_provider::running_view(provider, view.version.clone());
                    }
                    let proxy = self.proxy.clone();
                    let handle = subscription_provider::run_async(
                        provider,
                        SubscriptionRunRequest {
                            context: run_request,
                            cwd,
                        },
                        None,
                        move |event| {
                            let _ = proxy
                                .send_event(BrowserEvent::SubscriptionProvider { provider, event });
                        },
                    );
                    self.subscription_jobs.insert(
                        run_id,
                        SubscriptionJob {
                            run_id,
                            provider,
                            handle,
                        },
                    );
                }
                _ => unreachable!(),
            }
        }
        self.render_agent_panel();
        self.acknowledge_submission(&submission);
    }

    fn agent_run_for_id(&self, run_id: u64) -> Option<&AgentRun> {
        self.main_runs.get(run_id).or_else(|| {
            self.agent_graph_runs
                .values()
                .find(|presentation| presentation.run_id == run_id)
                .map(|presentation| &presentation.runtime)
        })
    }

    fn agent_run_for_id_mut(&mut self, run_id: u64) -> Option<&mut AgentRun> {
        if self.main_runs.get(run_id).is_some() {
            return self.main_runs.get_mut(run_id);
        }
        self.agent_graph_runs
            .values_mut()
            .find(|presentation| presentation.run_id == run_id)
            .map(|presentation| &mut presentation.runtime)
    }

    fn run_id_for_request(&self, request_id: &str) -> Option<u64> {
        self.main_runs
            .values()
            .find(|run| run.active_request_id.as_deref() == Some(request_id))
            .map(|run| run.id)
            .or_else(|| {
                self.agent_graph_runs
                    .values()
                    .find(|presentation| {
                        presentation.runtime.active_request_id.as_deref() == Some(request_id)
                    })
                    .map(|presentation| presentation.run_id)
            })
    }

    fn process_owner_scope_for_run(&self, run_id: Option<u64>) -> Option<String> {
        let run_id = run_id?;
        self.run_execution_contexts
            .get(&run_id)
            .map(|context| context.conversation_key.clone())
    }

    fn claude_job_for_id(&self, run_id: u64) -> Option<&ClaudeJob> {
        self.claude_jobs.get(&run_id)
    }

    fn take_claude_job(&mut self, run_id: u64) -> Option<ClaudeJob> {
        self.claude_jobs.remove(&run_id)
    }

    fn has_claude_jobs(&self) -> bool {
        !self.claude_jobs.is_empty()
    }

    fn subscription_job_for_id(
        &self,
        provider: AgentProviderKind,
        run_id: u64,
    ) -> Option<&SubscriptionJob> {
        self.subscription_jobs
            .get(&run_id)
            .filter(|job| job.provider == provider)
    }

    fn take_subscription_job(
        &mut self,
        provider: AgentProviderKind,
        run_id: u64,
    ) -> Option<SubscriptionJob> {
        if self
            .subscription_jobs
            .get(&run_id)
            .is_some_and(|job| job.provider == provider)
        {
            self.subscription_jobs.remove(&run_id)
        } else {
            None
        }
    }

    fn has_subscription_jobs(&self, provider: AgentProviderKind) -> bool {
        self.subscription_jobs
            .values()
            .any(|job| job.provider == provider)
    }

    fn any_agent_active(&self) -> bool {
        self.app_server.any_busy()
            || self.main_runs.values().any(AgentRun::is_active)
            || self
                .agent_graph_runs
                .values()
                .any(|presentation| presentation.runtime.is_active())
    }

    fn find_artifact(&self, artifact_id: &str) -> Option<ChatArtifact> {
        if artifact_id.len() > 128 {
            return None;
        }
        self.chat_messages
            .iter()
            .flat_map(|message| message.artifacts.iter())
            .chain(
                self.agent_graph_sessions
                    .iter()
                    .flat_map(|session| session.turns.iter())
                    .flat_map(|turn| turn.messages.iter())
                    .flat_map(|message| message.artifacts.iter()),
            )
            .chain(
                self.pending_artifacts
                    .values()
                    .flat_map(|artifacts| artifacts.iter()),
            )
            .find(|artifact| artifact.id == artifact_id)
            .cloned()
    }

    fn open_artifact(&mut self, artifact_id: &str) {
        let result = self
            .find_artifact(artifact_id)
            .ok_or_else(|| "The selected artifact is no longer available".to_owned())
            .and_then(|artifact| {
                if let Some(source_url) = artifact.source_url.as_deref() {
                    let url = Url::parse(source_url)
                        .map_err(|_| "The artifact source URL is invalid".to_owned())?;
                    if !matches!(url.scheme(), "http" | "https") {
                        return Err("Only HTTP and HTTPS artifact sources can be opened".to_owned());
                    }
                    self.create_tab(Some(url.to_string()), true)
                        .map(|_| ())
                        .map_err(|error| format!("The source could not be opened: {error}"))
                } else {
                    let path = self.artifact_store.stored_path(&artifact)?;
                    open_with_default(&path)
                }
            });
        self.finish_artifact_action(result, None);
    }

    fn save_artifact(&mut self, artifact_id: &str) {
        let Some(artifact) = self.find_artifact(artifact_id) else {
            self.finish_artifact_action(
                Err("The selected artifact is no longer available".to_owned()),
                None,
            );
            return;
        };
        let Some(destination) = rfd::FileDialog::new()
            .set_title("Save Supervisor artifact")
            .set_file_name(&artifact.title)
            .save_file()
        else {
            return;
        };
        let result = self.artifact_store.save_as(&artifact, &destination);
        self.finish_artifact_action(result, Some(format!("Saved {}", destination.display())));
    }

    fn show_artifact_in_folder(&mut self, artifact_id: &str) {
        let result = self
            .find_artifact(artifact_id)
            .ok_or_else(|| "The selected artifact is no longer available".to_owned())
            .and_then(|artifact| self.artifact_store.stored_path(&artifact))
            .and_then(|path| show_in_folder(&path));
        self.finish_artifact_action(result, None);
    }

    fn finish_artifact_action(&mut self, result: Result<(), String>, success: Option<String>) {
        let message = match result {
            Ok(()) => success,
            Err(error) => Some(format!("Artifact action failed: {error}")),
        };
        if let Some(message) = message {
            self.push_chat_message(ChatRole::System, message, Vec::new());
            self.render_agent_panel();
        }
    }

    fn attach_artifacts_to_run(&mut self, run_id: u64, artifacts: Vec<ChatArtifact>) -> bool {
        if artifacts.is_empty() {
            return false;
        }
        let mut attached = false;
        if let Some(message) = self.chat_messages.iter_mut().rev().find(|message| {
            message.run_id == Some(run_id)
                && message.role == ChatRole::Assistant
                && message.kind == ChatMessageKind::Message
        }) {
            extend_unique_artifacts(&mut message.artifacts, artifacts.iter().cloned());
            attached = true;
        }
        for session in &mut self.agent_graph_sessions {
            let Some(turn) = session.turns.iter_mut().find(|turn| turn.run_id == run_id) else {
                continue;
            };
            let target_index = turn
                .messages
                .iter()
                .rposition(|message| {
                    message.role == ChatRole::Assistant && message.kind == ChatMessageKind::Message
                })
                .or_else(|| {
                    turn.messages
                        .iter()
                        .rposition(|message| message.role != ChatRole::User)
                });
            if let Some(message) = target_index.and_then(|index| turn.messages.get_mut(index)) {
                extend_unique_artifacts(&mut message.artifacts, artifacts.iter().cloned());
                session.updated_at_ms = unix_time_ms();
                attached = true;
            }
        }
        if !attached {
            let pending = self.pending_artifacts.entry(run_id).or_default();
            extend_unique_artifacts(pending, artifacts);
        }
        attached
    }

    fn finalize_run_artifacts(&mut self, run_id: u64) {
        let mut artifacts = self.pending_artifacts.remove(&run_id).unwrap_or_default();
        if let Some(text) = self.chat_messages.iter().rev().find_map(|message| {
            (message.run_id == Some(run_id)
                && message.role == ChatRole::Assistant
                && message.kind == ChatMessageKind::Message)
                .then_some(message.text.as_str())
        }) {
            extend_unique_artifacts(&mut artifacts, extract_citation_artifacts(text));
        }
        if artifacts.is_empty() {
            return;
        }
        self.attach_artifacts_to_run(run_id, artifacts);
    }

    fn finalize_agent_activities(
        &mut self,
        run_id: u64,
        status: AgentStepStatus,
        detail: Option<&str>,
    ) {
        for message in &mut self.chat_messages {
            if message.kind == ChatMessageKind::Activity
                && message.run_id == Some(run_id)
                && message.streaming
            {
                message.set_activity_status(status, detail);
            }
        }
    }

    fn run_has_provider_job(&self, run_id: u64) -> bool {
        self.claude_job_for_id(run_id).is_some() || self.subscription_jobs.contains_key(&run_id)
    }

    fn run_after_time_machine_checkpoint(&mut self, action: PendingTimeMachineAction) {
        let run_id = action.run_id();
        if !action.may_mutate_workspace() || !self.time_machine_run_plans.contains_key(&run_id) {
            self.continue_time_machine_action(action);
            return;
        }
        if self.time_machine_run_roots.contains_key(&run_id) {
            if let Some(pending) = self.pending_time_machine_actions.get_mut(&run_id) {
                pending.push_back(action);
            } else {
                self.continue_time_machine_action(action);
            }
            return;
        }

        let plan = self.time_machine_run_plans[&run_id].clone();
        if let Some((other_run_id, _)) =
            self.time_machine_run_roots
                .iter()
                .find(|(other_run_id, root)| {
                    **other_run_id != run_id && checkpoint_roots_overlap(root, &plan.root)
                })
        {
            let message = if self
                .retryable_time_machine_finalizations
                .contains(other_run_id)
                || self
                    .time_machine_finalizations_in_flight
                    .contains(other_run_id)
            {
                format!(
                    "Time Machine is still recovering overlapping checkpoint {other_run_id}. Wait for recovery to finish before changing this workspace."
                )
            } else {
                format!(
                    "Another agent run ({other_run_id}) is already changing an overlapping workspace. This action was not started so both checkpoints remain independent."
                )
            };
            self.reject_time_machine_action(action, message);
            return;
        }

        let gate = RunStartGate::pending();
        self.time_machine_start_gates.insert(run_id, gate.clone());
        self.time_machine_run_roots
            .insert(run_id, plan.root.clone());
        self.pending_time_machine_actions
            .entry(run_id)
            .or_default()
            .push_back(action);
        if let Some(run) = self.agent_run_for_id_mut(run_id) {
            run.status = "Preparing a restore point before the first workspace change…".to_owned();
        }
        let worker_gate = gate.clone();
        let time_machine = Arc::clone(&self.time_machine);
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let checkpoint_started = Instant::now();
            let result = time_machine
                .lock()
                .map_err(|_| "Time Machine storage is unavailable".to_owned())
                .and_then(|mut time_machine| {
                    let result =
                        time_machine.begin_run(run_id, &plan.root, &plan.provider, &plan.context);
                    if let Err(error) = &result {
                        time_machine.set_error(error.clone());
                    }
                    result
                });
            info!(
                run_id,
                elapsed_ms = checkpoint_started.elapsed().as_millis(),
                success = result.is_ok(),
                "lazy Time Machine checkpoint prepared before first workspace change"
            );
            worker_gate.resolve(result.clone());
            let _ = proxy.send_event(BrowserEvent::TimeMachineStarted { run_id, result });
        });
        self.render_agent_panel();
        self.render_agent_graph_surface();
    }

    fn complete_time_machine_start(&mut self, run_id: u64, result: Result<(), String>) {
        let pending = self
            .pending_time_machine_actions
            .remove(&run_id)
            .unwrap_or_default();
        match result {
            Ok(()) => {
                self.time_machine_run_plans.remove(&run_id);
                if self
                    .agent_run_for_id(run_id)
                    .is_some_and(AgentRun::is_active)
                    && self.run_has_provider_job(run_id)
                {
                    for action in pending {
                        self.continue_time_machine_action(action);
                    }
                }
                self.start_time_machine_live_diff_scan(run_id);
            }
            Err(error) => {
                self.time_machine_run_roots.remove(&run_id);
                let _ = self.time_machine_start_gates.take_for_finalization(run_id);
                let message = format!(
                    "Time Machine could not prepare a restore point, so the workspace-changing action was not started: {error}"
                );
                if self
                    .agent_run_for_id(run_id)
                    .is_some_and(AgentRun::is_active)
                    && self.run_has_provider_job(run_id)
                {
                    for action in pending {
                        self.reject_time_machine_action(action, message.clone());
                    }
                }
                self.try_finish_run_when_quiescent(run_id);
            }
        }
        self.render_agent_panel();
        self.render_agent_graph_surface();
    }

    fn start_time_machine_live_diff_scan(&mut self, run_id: u64) {
        if !self.time_machine_run_roots.contains_key(&run_id)
            || self
                .time_machine_live_diff_scans_in_flight
                .contains(&run_id)
            || !self
                .live_diff_by_owner
                .values()
                .any(|tracked| tracked.run_id == run_id && tracked.view.active)
        {
            return;
        }
        self.time_machine_live_diff_scans_in_flight.insert(run_id);
        let time_machine = Arc::clone(&self.time_machine);
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = time_machine
                .lock()
                .map_err(|_| "Time Machine storage is busy".to_owned())
                .and_then(|time_machine| time_machine.live_diff(run_id));
            let _ = proxy.send_event(BrowserEvent::TimeMachineLiveDiffUpdated { run_id, result });
        });
    }

    fn complete_time_machine_live_diff(
        &mut self,
        run_id: u64,
        result: Result<Option<LiveDiffStats>, String>,
    ) {
        self.time_machine_live_diff_scans_in_flight.remove(&run_id);
        let owner = self
            .run_execution_contexts
            .get(&run_id)
            .map(|context| context.conversation_key.clone());
        let mut changed = false;
        if self.time_machine_run_roots.contains_key(&run_id)
            && let (Some(owner), Ok(Some(stats))) = (owner.as_ref(), result.as_ref())
            && self
                .live_diff_by_owner
                .get(owner)
                .is_some_and(|tracked| tracked.run_id == run_id && tracked.view.active)
        {
            let next = TrackedLiveDiff::from(*stats);
            changed = self.live_diff_by_owner.get(owner) != Some(&next);
            self.live_diff_by_owner.insert(owner.clone(), next);
        }
        if changed {
            self.render_agent_panel();
            self.render_agent_graph_surface();
        }
        if self.time_machine_run_roots.contains_key(&run_id)
            && owner.as_ref().is_some_and(|owner| {
                self.live_diff_by_owner
                    .get(owner)
                    .is_some_and(|tracked| tracked.run_id == run_id && tracked.view.active)
            })
        {
            let proxy = self.proxy.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(700));
                let _ = proxy.send_event(BrowserEvent::TimeMachineLiveDiffTick { run_id });
            });
        }
    }

    fn continue_time_machine_action(&mut self, action: PendingTimeMachineAction) {
        match action {
            PendingTimeMachineAction::AgentCommand { request, .. } => {
                self.execute_agent_command_now(request)
            }

            PendingTimeMachineAction::ClaudePermission {
                request,
                approval_message,
            } => {
                self.begin_command(
                    &request.request_id,
                    &request.tool_name,
                    request.summary,
                    request.authorization,
                );
                self.mark_claude_step_running(request.tool_use_id.as_deref());
                self.send_claude_permission_result(
                    request.run_id,
                    request.request_id,
                    true,
                    &approval_message,
                );
            }
            PendingTimeMachineAction::SubscriptionPermission {
                request,
                approval_message,
            } => {
                self.begin_command(
                    &request.request_id,
                    &request.tool_name,
                    request.summary,
                    request.authorization,
                );
                self.mark_subscription_step_running(request.tool_use_id.as_deref());
                self.send_subscription_permission_result(
                    request.provider,
                    request.run_id,
                    request.request_id,
                    true,
                    &approval_message,
                );
            }
        }
    }

    fn reject_time_machine_action(&mut self, action: PendingTimeMachineAction, message: String) {
        match action {
            PendingTimeMachineAction::AgentCommand { request, .. } => {
                let action_name = request.command.name().to_owned();
                let summary = request.command.summary();
                let authorization = self.command_authorization(&request.command);
                self.begin_command(&request.request_id, &action_name, summary, authorization);
                self.finish_command(request.request_id, action_name, false, message, Value::Null);
            }

            PendingTimeMachineAction::ClaudePermission { request, .. } => {
                self.begin_command(
                    &request.request_id,
                    &request.tool_name,
                    request.summary,
                    request.authorization,
                );
                self.mark_claude_action_status(&request.request_id, CommandStatus::Error, &message);
                self.send_claude_permission_result(
                    request.run_id,
                    request.request_id,
                    false,
                    &message,
                );
            }
            PendingTimeMachineAction::SubscriptionPermission { request, .. } => {
                self.begin_command(
                    &request.request_id,
                    &request.tool_name,
                    request.summary,
                    request.authorization,
                );
                self.mark_claude_action_status(&request.request_id, CommandStatus::Error, &message);
                self.send_subscription_permission_result(
                    request.provider,
                    request.run_id,
                    request.request_id,
                    false,
                    &message,
                );
            }
        }
    }

    fn finish_run_when_quiescent(&mut self, run_id: u64) {
        self.pending_run_finalizations.insert(run_id);
        if !self.run_has_provider_job(run_id) && self.processes.has_active_owner(run_id) {
            let outcome = self.agent_run_for_id(run_id).and_then(|run| {
                (run.phase != AgentPhase::Finalizing).then(|| PendingRunOutcome {
                    phase: run.phase,
                    status: run.status.clone(),
                })
            });
            if let Some(outcome) = outcome {
                self.pending_run_outcomes.insert(run_id, outcome);
            }
            if let Some(run) = self.agent_run_for_id_mut(run_id) {
                run.phase = AgentPhase::Finalizing;
                run.status =
                    "Waiting for background commands to finish; Stop cancels them.".to_owned();
            }
        }
        self.try_finish_run_when_quiescent(run_id);
    }

    fn try_finish_run_when_quiescent(&mut self, run_id: u64) -> bool {
        if !self.pending_run_finalizations.contains(&run_id)
            || !run_finalization_is_ready(
                self.run_has_provider_job(run_id),
                self.processes.has_active_owner(run_id),
            )
        {
            return false;
        }
        self.pending_run_finalizations.remove(&run_id);
        if !self.pending_run_outcomes.contains_key(&run_id) {
            let outcome = self.agent_run_for_id(run_id).and_then(|run| {
                (run.phase != AgentPhase::Finalizing).then(|| PendingRunOutcome {
                    phase: run.phase,
                    status: run.status.clone(),
                })
            });
            if let Some(outcome) = outcome {
                self.pending_run_outcomes.insert(run_id, outcome);
            }
        }
        if let Some(run) = self.agent_run_for_id_mut(run_id) {
            run.phase = AgentPhase::Finalizing;
            run.status = "Finalizing Time Machine checkpoint…".to_owned();
        }
        let checkpoint_pending = self.start_time_machine_checkpoint_finalization(run_id);
        if !checkpoint_pending {
            self.complete_run_presentation(run_id);
            self.finish_pending_workspace_ejections();
        }
        if !checkpoint_pending {
            self.start_next_agent_submission_if_idle();
        }
        true
    }

    fn complete_run_presentation(&mut self, run_id: u64) {
        if let Some(tracked) = self
            .live_diff_by_owner
            .values_mut()
            .find(|tracked| tracked.run_id == run_id)
        {
            tracked.view.active = false;
        }
        if let Some(outcome) = self.pending_run_outcomes.remove(&run_id)
            && let Some(run) = self.agent_run_for_id_mut(run_id)
        {
            run.phase = outcome.phase;
            run.status = outcome.status;
        }
        if self
            .agent_graph_runs
            .values()
            .any(|presentation| presentation.run_id == run_id)
        {
            self.snapshot_agent_graph_turn(run_id);
            self.retire_agent_graph_run(run_id);
        }
    }

    fn start_time_machine_checkpoint_finalization(&mut self, run_id: u64) -> bool {
        let Some(start_gate) = self.time_machine_start_gates.take_for_finalization(run_id) else {
            self.time_machine_run_plans.remove(&run_id);
            self.pending_time_machine_actions.remove(&run_id);
            return false;
        };
        self.time_machine_finalizations_in_flight.insert(run_id);
        let time_machine = Arc::clone(&self.time_machine);
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = match start_gate.wait(|| false) {
                Ok(()) => time_machine
                    .lock()
                    .map_err(|_| "Time Machine storage is busy".to_owned())
                    .and_then(|mut time_machine| time_machine.finish_run(run_id)),
                // The action receives the failed prerequisite through TimeMachineStarted. No
                // baseline manifest exists to finalize or retry in this case.
                Err(_) => Ok(None),
            };
            let _ = proxy.send_event(BrowserEvent::TimeMachineFinished { run_id, result });
        });
        true
    }

    fn retry_failed_time_machine_finalizations(&mut self) {
        let run_ids = self
            .retryable_time_machine_finalizations
            .drain()
            .collect::<Vec<_>>();
        for run_id in run_ids {
            self.time_machine_finalizations_in_flight.insert(run_id);
            let time_machine = Arc::clone(&self.time_machine);
            let proxy = self.proxy.clone();
            std::thread::spawn(move || {
                let result = time_machine
                    .lock()
                    .map_err(|_| "Time Machine storage is busy".to_owned())
                    .and_then(|mut time_machine| time_machine.finish_run(run_id));
                let _ = proxy.send_event(BrowserEvent::TimeMachineFinished { run_id, result });
            });
        }
    }

    fn resume_pending_agent_submission_if_safe(&mut self) {
        while let Some(index) = self
            .pending_agent_submissions
            .iter()
            .position(|submission| {
                !self.submission_recovery_blocked(submission)
                    && submission
                        .owner()
                        .is_some_and(|owner| !self.owner_has_running_work(owner))
            })
        {
            let pending = self
                .pending_agent_submissions
                .remove(index)
                .expect("selected recovery input");
            self.submit_agent_submission(pending);
        }
    }

    fn cancel_pending_agent_submission(&mut self) {
        if self.active_pending_submission().is_none() {
            return;
        }
        let owner = self.conversation_key(None);
        if let Err(error) = self.cancel_submissions_for_owner(&owner, false, true) {
            self.push_chat_message(ChatRole::System, error, vec![]);
            self.render_agent_panel();
            return;
        }
        self.push_chat_message(
            ChatRole::System,
            "Queued agent request cancelled. The unfinished Time Machine checkpoint remains recoverable and will be retried before another overlapping run can start."
                .to_owned(),
            Vec::new(),
        );
        self.render_agent_panel();
        self.render_agent_graph_surface();
    }

    fn discardable_queued_checkpoint_run_id(&self) -> Option<u64> {
        let pending = self.active_pending_submission()?;
        let root = pending.workspace_root()?;
        self.time_machine_run_roots
            .iter()
            .find(|(run_id, active_root)| {
                self.retryable_time_machine_finalizations.contains(run_id)
                    && !self.time_machine_finalizations_in_flight.contains(run_id)
                    && checkpoint_roots_overlap(active_root, root)
            })
            .map(|(run_id, _)| *run_id)
    }

    fn discard_failed_checkpoint(&mut self, run_id: u64) {
        if self.discardable_queued_checkpoint_run_id() != Some(run_id) {
            self.push_chat_message(
                ChatRole::System,
                "That unfinished checkpoint is not currently eligible for discard. Wait for recovery to finish or retry the queued request."
                    .to_owned(),
                Vec::new(),
            );
            self.render_agent_panel();
            self.render_agent_graph_surface();
            return;
        }
        let result = self
            .time_machine
            .lock()
            .map_err(|_| "Time Machine storage is busy".to_owned())
            .and_then(|mut time_machine| time_machine.discard_unfinished_run(run_id));
        match result {
            Ok(()) => {
                self.retryable_time_machine_finalizations.remove(&run_id);
                self.time_machine_finalizations_in_flight.remove(&run_id);
                self.time_machine_run_roots.remove(&run_id);
                self.push_chat_message(
                    ChatRole::System,
                    format!(
                        "Discarded the failed checkpoint for run {run_id}. It cannot be restored. The queued request can now start."
                    ),
                    Vec::new(),
                );
                self.save_session();
                self.resume_pending_agent_submission_if_safe();
            }
            Err(error) => {
                if let Ok(mut time_machine) = self.time_machine.lock() {
                    time_machine.set_error(error.clone());
                }
                self.push_chat_message(
                    ChatRole::System,
                    format!("The failed checkpoint could not be discarded: {error}"),
                    Vec::new(),
                );
            }
        }
        self.render_agent_panel();
        self.render_agent_graph_surface();
    }

    fn complete_time_machine_checkpoint(
        &mut self,
        run_id: u64,
        result: Result<Option<CheckpointSummary>, String>,
    ) {
        self.time_machine_run_plans.remove(&run_id);
        self.pending_time_machine_actions.remove(&run_id);
        self.complete_run_presentation(run_id);
        let recovered = result.is_ok();
        match result {
            Ok(Some(summary)) => {
                self.time_machine_finalizations_in_flight.remove(&run_id);
                self.retryable_time_machine_finalizations.remove(&run_id);
                self.time_machine_run_roots.remove(&run_id);
                if let Some(owner) = self
                    .run_execution_contexts
                    .get(&run_id)
                    .map(|context| context.conversation_key.clone())
                    && self
                        .live_diff_by_owner
                        .get(&owner)
                        .is_some_and(|tracked| tracked.run_id == run_id)
                {
                    self.live_diff_by_owner.insert(
                        owner,
                        TrackedLiveDiff {
                            run_id,
                            view: LiveDiffView {
                                additions: summary.total_additions,
                                deletions: summary.total_deletions,
                                file_count: summary.file_count,
                                active: false,
                            },
                        },
                    );
                }
                self.workspace.refresh_explorer();
                if should_publish_checkpoint_summary(&summary) {
                    if let Some(message) = self.chat_messages.iter_mut().rev().find(|message| {
                        message.run_id == Some(run_id)
                            && message.kind == ChatMessageKind::Message
                            && !matches!(message.role, ChatRole::User)
                    }) {
                        message.checkpoint = Some(summary.clone());
                    }
                    let mut graph_history_changed = false;
                    for session in &mut self.agent_graph_sessions {
                        if let Some(turn) =
                            session.turns.iter_mut().find(|turn| turn.run_id == run_id)
                        {
                            turn.checkpoint = Some(summary.clone());
                            graph_history_changed = true;
                        }
                    }
                    if graph_history_changed {
                        self.save_agent_graph_sessions();
                    }
                }
            }
            Ok(None) => {
                self.time_machine_finalizations_in_flight.remove(&run_id);
                self.retryable_time_machine_finalizations.remove(&run_id);
                self.time_machine_run_roots.remove(&run_id);
            }
            Err(error) => {
                self.time_machine_finalizations_in_flight.remove(&run_id);
                self.retryable_time_machine_finalizations.insert(run_id);
                if let Ok(mut time_machine) = self.time_machine.lock() {
                    time_machine.set_error(error.clone());
                }
                let blocked_removals = self
                    .time_machine_run_roots
                    .get(&run_id)
                    .map(|run_root| {
                        self.pending_workspace_ejections
                            .iter()
                            .filter(|project_root| {
                                checkpoint_roots_overlap(Path::new(project_root), run_root)
                            })
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let removal_waiting = !blocked_removals.is_empty();
                for project_root in blocked_removals {
                    self.pending_workspace_ejections.remove(&project_root);
                }
                let message = format!(
                    "Time Machine could not finalize this run: {error}. The unfinished checkpoint remains recoverable and will be retried before the next agent run.{} If recovery keeps failing, explicitly discard the failed checkpoint from the queued-request notice to continue without it.",
                    if removal_waiting {
                        " The project was kept connected because its checkpoint is not safe yet."
                    } else {
                        ""
                    }
                );
                let mut routed_to_graph_history = false;
                for session in &mut self.agent_graph_sessions {
                    let Some(turn) = session.turns.iter_mut().find(|turn| turn.run_id == run_id)
                    else {
                        continue;
                    };
                    turn.messages.push(AgentGraphHistoryMessage {
                        native: None,
                        id: self.next_chat_message_id,
                        role: ChatRole::System,
                        kind: ChatMessageKind::Message,
                        text: message.clone(),
                        timestamp_ms: unix_time_ms(),
                        artifacts: Vec::new(),
                    });
                    self.next_chat_message_id = self.next_chat_message_id.saturating_add(1);
                    session.updated_at_ms = unix_time_ms();
                    routed_to_graph_history = true;
                    break;
                }
                if routed_to_graph_history {
                    self.save_agent_graph_sessions();
                } else {
                    self.push_chat_message_for_run(run_id, ChatRole::System, message, Vec::new());
                }
            }
        }
        if recovered {
            self.finish_pending_workspace_ejections();
        }
        self.save_session();
        self.render_agent_panel();
        self.render_agent_graph_surface();
        if recovered {
            self.resume_pending_agent_submission_if_safe();
            self.start_next_agent_submission_if_idle();
        }
    }

    fn restore_time_machine_checkpoint(&mut self, checkpoint_id: &str, path: Option<&str>) {
        let result = self
            .time_machine
            .lock()
            .map_err(|_| "Time Machine storage is busy".to_owned())
            .and_then(|mut time_machine| match path {
                Some(path) => time_machine.restore_checkpoint_file(checkpoint_id, path),
                None => time_machine.restore_checkpoint(checkpoint_id),
            });
        match result {
            Ok(summary) => {
                self.workspace.refresh_explorer();
                if self.refresh_checkpoint_references(&summary) {
                    self.save_agent_graph_sessions();
                }
                if let Ok(mut time_machine) = self.time_machine.lock() {
                    time_machine.set_message(if let Some(path) = path {
                        format!("Restored {path} without overwriting newer manual edits.")
                    } else {
                        format!(
                            "Restored {} file(s) without overwriting newer manual edits.",
                            summary.file_count
                        )
                    });
                }
                self.save_session();
            }
            Err(message) => {
                if let Ok(mut time_machine) = self.time_machine.lock() {
                    time_machine.set_error(message);
                }
            }
        }
        self.render_agent_panel();
    }

    fn refresh_checkpoint_references(&mut self, summary: &CheckpointSummary) -> bool {
        for message in &mut self.chat_messages {
            if message
                .checkpoint
                .as_ref()
                .is_some_and(|checkpoint| checkpoint.id == summary.id)
            {
                message.checkpoint = Some(summary.clone());
            }
        }
        let mut graph_history_changed = false;
        for session in &mut self.agent_graph_sessions {
            for turn in &mut session.turns {
                if turn
                    .checkpoint
                    .as_ref()
                    .is_some_and(|checkpoint| checkpoint.id == summary.id)
                {
                    turn.checkpoint = Some(summary.clone());
                    graph_history_changed = true;
                }
            }
        }
        graph_history_changed
    }

    fn handle_claude_provider_event(&mut self, event: ClaudeProviderEvent) {
        match event {
            ClaudeProviderEvent::StreamDelta(delta) => {
                self.note_provider_event(delta.run_id, "stream_delta");
                self.handle_claude_stream_delta(delta);
            }
            ClaudeProviderEvent::ToolStarted(tool) => {
                self.note_provider_event(tool.run_id, "tool_started");
                self.handle_claude_tool_started(tool);
            }
            ClaudeProviderEvent::ToolFinished(tool) => {
                self.note_provider_event(tool.run_id, "tool_finished");
                self.handle_claude_tool_finished(tool);
            }
            ClaudeProviderEvent::PermissionRequested(request) => {
                self.note_provider_event(request.run_id, "permission_requested");
                self.handle_claude_permission_requested(request);
            }
            ClaudeProviderEvent::UsageLimit(update) => {
                self.note_provider_event(update.run_id, "usage_limit");
                self.claude_usage_limit = Some(update.limit);
                self.annotate_claude_usage();
                self.render_agent_panel();
            }
            ClaudeProviderEvent::Finished(result) => {
                let run_id = result.run_id;
                let outcome = if result.result.is_ok() {
                    "completed"
                } else {
                    "error"
                };
                self.note_provider_event(run_id, "finished");
                self.handle_claude_run_finished(result);
                self.finish_run_timing(run_id, outcome);
            }
        }
    }

    /// Keeps the latest subscription usage notice visible across provider status
    /// rewrites until its window resets.
    fn annotate_claude_usage(&mut self) {
        let now = claude_provider::unix_now_secs();
        if self
            .claude_usage_limit
            .as_ref()
            .is_some_and(|limit| limit.has_reset(now))
        {
            self.claude_usage_limit = None;
        }
        self.claude_usage_notice = claude_provider::replace_usage_notice(
            &mut self.claude_provider.detail,
            self.claude_usage_notice.as_deref(),
            self.claude_usage_limit.as_ref(),
            now,
        );
    }

    fn handle_claude_stream_delta(&mut self, delta: ClaudeStreamDelta) {
        let run_matches = self.claude_job_for_id(delta.run_id).is_some()
            && self
                .agent_run_for_id(delta.run_id)
                .is_some_and(AgentRun::is_active);
        if !run_matches || delta.delta.is_empty() {
            return;
        }

        let item_id = match delta.kind {
            ClaudeStreamKind::Text => "claude-output",
            ClaudeStreamKind::Thinking => "claude-thinking",
        };
        if delta.kind == ClaudeStreamKind::Text {
            for message in &mut self.chat_messages {
                if message.kind == ChatMessageKind::Reasoning
                    && message.run_id == Some(delta.run_id)
                {
                    message.streaming = false;
                }
            }
        }
        let existing = self.chat_messages.iter().position(|message| {
            message.run_id == Some(delta.run_id)
                && message.provider_item_id.as_deref() == Some(item_id)
        });
        let message_id = if let Some(index) = existing {
            let message = &mut self.chat_messages[index];
            message.text.push_str(&delta.delta);
            message.streaming = true;
            message.id
        } else {
            let id = self.next_chat_message_id;
            self.next_chat_message_id += 1;
            let mut message = match delta.kind {
                ClaudeStreamKind::Thinking => {
                    ChatMessage::reasoning(id, delta.run_id, item_id.to_owned(), 0, &delta.delta)
                }
                ClaudeStreamKind::Text => ChatMessage {
                    id,
                    role: ChatRole::Assistant,
                    kind: ChatMessageKind::Message,
                    text: delta.delta,
                    streaming: true,
                    timestamp_ms: unix_time_ms(),
                    attachments: Vec::new(),
                    terminal_attachments: Vec::new(),
                    file_attachments: Vec::new(),
                    artifacts: Vec::new(),
                    provider: Some(AgentProviderKind::ClaudeCode),
                    checkpoint: None,
                    activity_status: None,
                    activity_category: None,
                    activity_detail: None,
                    activity_context: None,
                    activity_additions: None,
                    activity_deletions: None,
                    activity_diff: None,
                    run_id: Some(delta.run_id),
                    provider_item_id: Some(item_id.to_owned()),
                    reasoning_summary_index: None,

                    native_turn_id: None,
                },
            };
            message.provider = Some(AgentProviderKind::ClaudeCode);
            self.chat_messages.push_back(message);
            id
        };
        if let Some(run) = self.agent_run_for_id_mut(delta.run_id) {
            run.status = match delta.kind {
                ClaudeStreamKind::Text => "Claude Code is responding…".to_owned(),
                ClaudeStreamKind::Thinking => "Claude Code is reasoning…".to_owned(),
            };
        }
        if existing.is_some() {
            self.stream_agent_message(message_id);
        } else {
            self.render_agent_panel();
        }
    }

    fn handle_claude_tool_started(&mut self, tool: ClaudeToolStarted) {
        if self.claude_job_for_id(tool.run_id).is_none() {
            return;
        }
        let index = {
            let Some(run) = self.agent_run_for_id_mut(tool.run_id) else {
                return;
            };
            let index = run.append_streamed_tool_step(tool.label.clone(), tool.kind);
            run.status = format!("Claude Code requested {}", tool.tool_name);
            index
        };
        self.push_agent_activity(
            tool.run_id,
            AgentProviderKind::ClaudeCode,
            tool.tool_use_id.clone(),
            &tool.label,
            AgentActivityMetadata::reported_files(tool.file_activity.as_ref()),
        );
        self.claude_tool_run_ids
            .insert(tool.tool_use_id.clone(), tool.run_id);
        self.claude_tool_steps.insert(tool.tool_use_id, index);
        self.render_agent_panel();
    }

    fn handle_claude_permission_requested(&mut self, request: ClaudePermissionRequest) {
        if self.claude_job_for_id(request.run_id).is_none() {
            return;
        }
        if let Some(tool_use_id) = request.tool_use_id.as_deref() {
            self.claude_tool_request_ids
                .insert(tool_use_id.to_owned(), request.request_id.clone());
        }
        match self.permission_policy.decision(request.authorization) {
            PermissionDecision::Execute => {
                self.run_after_time_machine_checkpoint(
                    PendingTimeMachineAction::ClaudePermission {
                        request: pending_claude_permission(request),
                        approval_message: "Approved by the active Supervisor permission mode"
                            .to_owned(),
                    },
                );
            }
            PermissionDecision::NeedsInitialAuthorization
            | PermissionDecision::NeedsActionApproval => {
                self.audit_command_event(
                    "approval_required",
                    &request.request_id,
                    &request.tool_name,
                    request.authorization,
                    CommandStatus::AwaitingApproval,
                    &request.summary,
                );
                self.action_log.push_front(ActionLogEntry {
                    request_id: request.request_id.clone(),
                    action: request.tool_name.clone(),
                    summary: request.summary.clone(),
                    scope: request.authorization.scope,
                    effect: request.authorization.effect,
                    status: CommandStatus::AwaitingApproval,
                    timestamp_ms: unix_time_ms(),
                });
                self.mark_claude_step_waiting(request.tool_use_id.as_deref());
                if let Some(run) = self.agent_run_for_id_mut(request.run_id) {
                    run.phase = AgentPhase::WaitingApproval;
                    run.status = format!("Waiting to approve {}", request.tool_name);
                }
                self.pending_claude_permissions.insert(
                    request.request_id.clone(),
                    pending_claude_permission(request),
                );
                self.render_agent_panel();
            }
            PermissionDecision::Denied => {
                self.begin_command(
                    &request.request_id,
                    &request.tool_name,
                    request.summary,
                    request.authorization,
                );
                self.mark_claude_action_status(
                    &request.request_id,
                    CommandStatus::Denied,
                    "Blocked by Supervisor's privileged-action boundary",
                );
                self.send_claude_permission_result(
                    request.run_id,
                    request.request_id,
                    false,
                    "This privileged action is not available to Supervisor",
                );
            }
        }
    }

    fn handle_claude_tool_finished(&mut self, tool: ClaudeToolFinished) {
        if self.claude_job_for_id(tool.run_id).is_none() {
            return;
        }
        self.claude_tool_run_ids.remove(&tool.tool_use_id);
        if let Some(index) = self.claude_tool_steps.remove(&tool.tool_use_id)
            && let Some(run) = self.agent_run_for_id_mut(tool.run_id)
            && let Some(step) = run.steps.get_mut(index)
        {
            step.status = if tool.success {
                AgentStepStatus::Success
            } else {
                AgentStepStatus::Error
            };
            step.detail = tool.detail.clone();
            run.current_index = run.steps.len();
            run.phase = AgentPhase::Planning;
            run.status = if tool.success {
                "Result returned; Claude Code is evaluating the next step…".to_owned()
            } else {
                "Tool error returned; Claude Code is evaluating an alternative…".to_owned()
            };
        }
        if let Some(request_id) = self.claude_tool_request_ids.remove(&tool.tool_use_id) {
            let already_denied = self.action_log.iter().any(|entry| {
                entry.request_id == request_id && entry.status == CommandStatus::Denied
            });
            if !already_denied {
                self.mark_claude_action_status(
                    &request_id,
                    if tool.success {
                        CommandStatus::Success
                    } else {
                        CommandStatus::Error
                    },
                    tool.detail.as_deref().unwrap_or(if tool.success {
                        "Claude Code tool completed"
                    } else {
                        "Claude Code tool failed"
                    }),
                );
            }
        }
        self.update_agent_activity_status(
            tool.run_id,
            &tool.tool_use_id,
            if tool.success {
                AgentStepStatus::Success
            } else {
                AgentStepStatus::Error
            },
            tool.detail.as_deref(),
        );
        self.render_agent_panel();
    }

    fn handle_claude_run_finished(&mut self, result: ClaudeRunResult) {
        let is_agent_graph_run = self
            .agent_graph_runs
            .values()
            .any(|presentation| presentation.run_id == result.run_id);
        let Some(job) = self.take_claude_job(result.run_id) else {
            return;
        };
        for message in &mut self.chat_messages {
            if message.run_id == Some(result.run_id) {
                message.streaming = false;
            }
        }
        self.pending_claude_permissions
            .retain(|_, request| request.run_id != result.run_id);
        if !self.has_claude_jobs() {
            self.claude_tool_steps.clear();
            self.claude_tool_run_ids.clear();
            self.claude_tool_request_ids.clear();
        }
        if self
            .agent_run_for_id(result.run_id)
            .is_some_and(|run| run.phase == AgentPhase::Stopped)
        {
            if !self.has_claude_jobs() {
                self.claude_provider = claude_provider::ready_view(
                    self.claude_provider.version.clone(),
                    "Run stopped; ready for a new request.",
                );
            }
            if is_agent_graph_run {
                self.finalize_run_artifacts(result.run_id);
            }
            self.finish_run_when_quiescent(result.run_id);
            self.save_session();
            self.annotate_claude_usage();
            self.render_agent_panel();
            return;
        }

        match result.result {
            Ok(completion) => {
                self.finalize_agent_activities(result.run_id, AgentStepStatus::Success, None);
                if !is_agent_graph_run
                    && let Some(session_id) = completion.session_id
                    && let Some(owner) = self
                        .run_execution_contexts
                        .get(&result.run_id)
                        .map(|context| context.conversation_key.clone())
                {
                    if owner == self.conversation_key(None) {
                        self.claude_session_id = Some(session_id.clone());
                        self.claude_session_cwd = Some(job.cwd.clone());
                    }
                    if let Some(chat) = owner
                        .strip_prefix("chat:")
                        .and_then(|id| self.project_chats.iter_mut().find(|chat| chat.id == id))
                    {
                        chat.claude_session_id = Some(session_id);
                        chat.claude_session_cwd = Some(job.cwd);
                    }
                }
                let existing = self.chat_messages.iter_mut().find(|message| {
                    message.run_id == Some(result.run_id)
                        && message.provider_item_id.as_deref() == Some("claude-output")
                });
                if let Some(message) = existing {
                    if !completion.assistant_message.trim().is_empty() {
                        message.text = completion.assistant_message;
                    }
                    message.streaming = false;
                } else if !completion.assistant_message.trim().is_empty() {
                    self.push_chat_message_for_run(
                        result.run_id,
                        ChatRole::Assistant,
                        completion.assistant_message,
                        Vec::new(),
                    );
                }
                let step_count = self
                    .agent_run_for_id(result.run_id)
                    .map(|run| run.steps.len())
                    .unwrap_or_default();
                if let Some(run) = self.agent_run_for_id_mut(result.run_id) {
                    run.phase = AgentPhase::Completed;
                    run.status = format!("Objective completed · {step_count} actions");
                }
                if !self.has_claude_jobs() {
                    self.claude_provider = claude_provider::ready_view(
                        self.claude_provider.version.clone(),
                        "Claude Code completed the run; the session can be continued.",
                    );
                }
            }
            Err(failure) => {
                let message = failure.message;
                self.finalize_agent_activities(
                    result.run_id,
                    AgentStepStatus::Error,
                    Some(&message),
                );
                if let Some(run) = self.agent_run_for_id_mut(result.run_id) {
                    run.phase = AgentPhase::Error;
                    run.status = message.clone();
                }
                self.push_chat_message_for_run(
                    result.run_id,
                    ChatRole::System,
                    format!("Claude Code run did not complete: {message}"),
                    Vec::new(),
                );
                // Structured Claude Code errors first; text matching covers older CLIs.
                let authentication_error = failure.kind == ClaudeFailureKind::Authentication
                    || (failure.kind == ClaudeFailureKind::Other
                        && (message.contains("401")
                            || message.to_ascii_lowercase().contains("authentication")
                            || message.to_ascii_lowercase().contains("oauth")));
                if !self.has_claude_jobs() {
                    let version = self.claude_provider.version.clone();
                    self.claude_provider = match failure.kind {
                        _ if authentication_error => AgentProviderView::unavailable(
                            "Anthropic rejected the saved Claude Code session. Choose Reconnect in Settings.",
                            version,
                        ),
                        // The appended usage notice names the window and its reset time.
                        ClaudeFailureKind::UsageLimit => claude_provider::ready_view(
                            version,
                            "Subscription limit reached; runs can continue after the reset.",
                        ),
                        ClaudeFailureKind::Billing => claude_provider::ready_view(
                            version,
                            format!(
                                "Anthropic declined the run for billing: {message}. Check this Claude account's plan or extra-usage settings."
                            ),
                        ),
                        _ => claude_provider::ready_view(
                            version,
                            format!("Last run ended with an error: {message}. Ready to retry."),
                        ),
                    };
                }
            }
        }
        self.finalize_run_artifacts(result.run_id);
        self.finish_run_when_quiescent(result.run_id);
        self.save_session();
        self.annotate_claude_usage();
        self.render_agent_panel();
    }

    fn handle_subscription_provider_event(
        &mut self,
        provider: AgentProviderKind,
        event: SubscriptionProviderEvent,
    ) {
        match event {
            SubscriptionProviderEvent::StreamDelta(delta) => {
                self.note_provider_event(delta.run_id, "stream_delta");
                self.handle_subscription_stream_delta(provider, delta)
            }
            SubscriptionProviderEvent::ToolStarted(tool) => {
                self.note_provider_event(tool.run_id, "tool_started");
                self.handle_subscription_tool_started(provider, tool)
            }
            SubscriptionProviderEvent::ToolFinished(tool) => {
                self.note_provider_event(tool.run_id, "tool_finished");
                self.handle_subscription_tool_finished(provider, tool)
            }
            SubscriptionProviderEvent::PermissionRequested(request) => {
                self.note_provider_event(request.run_id, "permission_requested");
                self.handle_subscription_permission_requested(provider, request)
            }
            SubscriptionProviderEvent::Finished(result) => {
                let run_id = result.run_id;
                let outcome = if result.result.is_ok() {
                    "completed"
                } else {
                    "error"
                };
                self.note_provider_event(run_id, "finished");
                self.handle_subscription_run_finished(provider, result);
                self.finish_run_timing(run_id, outcome);
            }
        }
    }

    fn subscription_job_matches(&self, provider: AgentProviderKind, run_id: u64) -> bool {
        self.subscription_job_for_id(provider, run_id).is_some()
            && self
                .agent_run_for_id(run_id)
                .is_some_and(AgentRun::is_active)
    }

    fn handle_subscription_stream_delta(
        &mut self,
        provider: AgentProviderKind,
        delta: SubscriptionStreamDelta,
    ) {
        if !self.subscription_job_matches(provider, delta.run_id) || delta.delta.is_empty() {
            return;
        }
        let item_id = format!(
            "{}-{}",
            provider.id(),
            match delta.kind {
                SubscriptionStreamKind::Text => "output",
                SubscriptionStreamKind::Thinking => "thinking",
            }
        );
        if delta.kind == SubscriptionStreamKind::Text {
            for message in &mut self.chat_messages {
                if message.kind == ChatMessageKind::Reasoning
                    && message.run_id == Some(delta.run_id)
                {
                    message.streaming = false;
                }
            }
        }
        let existing = self.chat_messages.iter().position(|message| {
            message.run_id == Some(delta.run_id)
                && message.provider_item_id.as_deref() == Some(item_id.as_str())
        });
        let message_id = if let Some(index) = existing {
            let message = &mut self.chat_messages[index];
            message.text.push_str(&delta.delta);
            message.streaming = true;
            message.id
        } else {
            let id = self.next_chat_message_id;
            self.next_chat_message_id += 1;
            let mut message = match delta.kind {
                SubscriptionStreamKind::Thinking => {
                    ChatMessage::reasoning(id, delta.run_id, item_id, 0, &delta.delta)
                }
                SubscriptionStreamKind::Text => ChatMessage {
                    id,
                    role: ChatRole::Assistant,
                    kind: ChatMessageKind::Message,
                    text: delta.delta,
                    streaming: true,
                    timestamp_ms: unix_time_ms(),
                    attachments: Vec::new(),
                    terminal_attachments: Vec::new(),
                    file_attachments: Vec::new(),
                    artifacts: Vec::new(),
                    provider: Some(provider),
                    checkpoint: None,
                    activity_status: None,
                    activity_category: None,
                    activity_detail: None,
                    activity_context: None,
                    activity_additions: None,
                    activity_deletions: None,
                    activity_diff: None,
                    run_id: Some(delta.run_id),
                    provider_item_id: Some(item_id),
                    reasoning_summary_index: None,

                    native_turn_id: None,
                },
            };
            message.provider = Some(provider);
            self.chat_messages.push_back(message);
            id
        };
        if let Some(run) = self.agent_run_for_id_mut(delta.run_id) {
            run.status = match delta.kind {
                SubscriptionStreamKind::Text => format!("{} is responding…", provider.label()),
                SubscriptionStreamKind::Thinking => format!("{} is reasoning…", provider.label()),
            };
        }
        if existing.is_some() {
            self.stream_agent_message(message_id);
        } else {
            self.render_agent_panel();
        }
    }

    fn handle_subscription_tool_started(
        &mut self,
        provider: AgentProviderKind,
        tool: SubscriptionToolStarted,
    ) {
        if !self.subscription_job_matches(provider, tool.run_id) {
            return;
        }
        let index = {
            let Some(run) = self.agent_run_for_id_mut(tool.run_id) else {
                return;
            };
            let index = run.append_streamed_tool_step(tool.label.clone(), tool.kind);
            run.status = format!("{} requested {}", provider.label(), tool.tool_name);
            index
        };
        self.push_agent_activity(
            tool.run_id,
            provider,
            tool.tool_use_id.clone(),
            &tool.label,
            AgentActivityMetadata::reported_files(tool.file_activity.as_ref()),
        );
        self.subscription_tool_run_ids
            .insert(tool.tool_use_id.clone(), tool.run_id);
        self.subscription_tool_steps.insert(tool.tool_use_id, index);
        self.render_agent_panel();
    }

    fn handle_subscription_permission_requested(
        &mut self,
        provider: AgentProviderKind,
        request: SubscriptionPermissionRequest,
    ) {
        if !self.subscription_job_matches(provider, request.run_id) {
            return;
        }
        if let Some(tool_use_id) = request.tool_use_id.as_deref() {
            self.subscription_tool_request_ids
                .insert(tool_use_id.to_owned(), request.request_id.clone());
        }
        match self.permission_policy.decision(request.authorization) {
            PermissionDecision::Execute => {
                self.run_after_time_machine_checkpoint(
                    PendingTimeMachineAction::SubscriptionPermission {
                        request: pending_subscription_permission(provider, request),
                        approval_message: "Approved by the active Supervisor permission mode"
                            .to_owned(),
                    },
                );
            }
            PermissionDecision::NeedsInitialAuthorization
            | PermissionDecision::NeedsActionApproval => {
                self.audit_command_event(
                    "approval_required",
                    &request.request_id,
                    &request.tool_name,
                    request.authorization,
                    CommandStatus::AwaitingApproval,
                    &request.summary,
                );
                self.action_log.push_front(ActionLogEntry {
                    request_id: request.request_id.clone(),
                    action: request.tool_name.clone(),
                    summary: request.summary.clone(),
                    scope: request.authorization.scope,
                    effect: request.authorization.effect,
                    status: CommandStatus::AwaitingApproval,
                    timestamp_ms: unix_time_ms(),
                });
                self.mark_subscription_step_waiting(request.tool_use_id.as_deref());
                if let Some(run) = self.agent_run_for_id_mut(request.run_id) {
                    run.phase = AgentPhase::WaitingApproval;
                    run.status = format!("Waiting to approve {}", request.tool_name);
                }
                self.pending_subscription_permissions.insert(
                    request.request_id.clone(),
                    pending_subscription_permission(provider, request),
                );
                self.render_agent_panel();
            }
            PermissionDecision::Denied => {
                self.begin_command(
                    &request.request_id,
                    &request.tool_name,
                    request.summary,
                    request.authorization,
                );
                self.mark_claude_action_status(
                    &request.request_id,
                    CommandStatus::Denied,
                    "Blocked by Supervisor's privileged-action boundary",
                );
                self.send_subscription_permission_result(
                    provider,
                    request.run_id,
                    request.request_id,
                    false,
                    "This privileged action is not available to Supervisor",
                );
            }
        }
    }

    fn handle_subscription_tool_finished(
        &mut self,
        provider: AgentProviderKind,
        tool: SubscriptionToolFinished,
    ) {
        if !self.subscription_job_matches(provider, tool.run_id) {
            return;
        }
        self.subscription_tool_run_ids.remove(&tool.tool_use_id);
        if let Some(index) = self.subscription_tool_steps.remove(&tool.tool_use_id)
            && let Some(run) = self.agent_run_for_id_mut(tool.run_id)
            && let Some(step) = run.steps.get_mut(index)
        {
            step.status = if tool.success {
                AgentStepStatus::Success
            } else {
                AgentStepStatus::Error
            };
            step.detail = tool.detail.clone();
            run.current_index = run.steps.len();
            run.phase = AgentPhase::Planning;
            run.status = if tool.success {
                format!(
                    "Result returned; {} is evaluating the next step…",
                    provider.label()
                )
            } else {
                format!(
                    "Tool error returned; {} is evaluating an alternative…",
                    provider.label()
                )
            };
        }
        if let Some(request_id) = self.subscription_tool_request_ids.remove(&tool.tool_use_id) {
            let already_denied = self.action_log.iter().any(|entry| {
                entry.request_id == request_id && entry.status == CommandStatus::Denied
            });
            if !already_denied {
                self.mark_claude_action_status(
                    &request_id,
                    if tool.success {
                        CommandStatus::Success
                    } else {
                        CommandStatus::Error
                    },
                    tool.detail.as_deref().unwrap_or(if tool.success {
                        "Provider tool completed"
                    } else {
                        "Provider tool failed"
                    }),
                );
            }
        }
        self.update_agent_activity_status(
            tool.run_id,
            &tool.tool_use_id,
            if tool.success {
                AgentStepStatus::Success
            } else {
                AgentStepStatus::Error
            },
            tool.detail.as_deref(),
        );
        self.render_agent_panel();
    }

    fn handle_subscription_run_finished(
        &mut self,
        provider: AgentProviderKind,
        result: SubscriptionRunResult,
    ) {
        let is_agent_graph_run = self
            .agent_graph_runs
            .values()
            .any(|presentation| presentation.run_id == result.run_id);
        let Some(_) = self.take_subscription_job(provider, result.run_id) else {
            return;
        };
        for message in &mut self.chat_messages {
            if message.run_id == Some(result.run_id) {
                message.streaming = false;
            }
        }
        self.pending_subscription_permissions
            .retain(|_, request| request.run_id != result.run_id);
        if !self.has_subscription_jobs(provider) {
            self.subscription_tool_steps.clear();
            self.subscription_tool_run_ids.clear();
            self.subscription_tool_request_ids.clear();
        }
        let version = self
            .subscription_state(provider)
            .and_then(|state| state.0.version.clone());
        if self
            .agent_run_for_id(result.run_id)
            .is_some_and(|run| run.phase == AgentPhase::Stopped)
        {
            if !self.has_subscription_jobs(provider)
                && let Some((view, _, _)) = self.subscription_state_mut(provider)
            {
                *view = subscription_provider::ready_view(
                    provider,
                    version,
                    "Run stopped; ready for a new request.",
                );
            }
            if is_agent_graph_run {
                self.finalize_run_artifacts(result.run_id);
            }
            self.finish_run_when_quiescent(result.run_id);
            self.save_session();
            self.render_agent_panel();
            return;
        }
        let output_item_id = format!("{}-output", provider.id());
        match result.result {
            Ok(completion) => {
                self.finalize_agent_activities(result.run_id, AgentStepStatus::Success, None);
                let existing = self.chat_messages.iter_mut().find(|message| {
                    message.run_id == Some(result.run_id)
                        && message.provider_item_id.as_deref() == Some(output_item_id.as_str())
                });
                if let Some(message) = existing {
                    if !completion.assistant_message.trim().is_empty() {
                        message.text = completion.assistant_message;
                    }
                    message.streaming = false;
                } else if !completion.assistant_message.trim().is_empty() {
                    self.push_chat_message_for_run(
                        result.run_id,
                        ChatRole::Assistant,
                        completion.assistant_message,
                        Vec::new(),
                    );
                }
                let step_count = self
                    .agent_run_for_id(result.run_id)
                    .map(|run| run.steps.len())
                    .unwrap_or_default();
                if let Some(run) = self.agent_run_for_id_mut(result.run_id) {
                    run.phase = AgentPhase::Completed;
                    run.status = format!("Objective completed · {step_count} actions");
                }
                if !self.has_subscription_jobs(provider)
                    && let Some((view, _, _)) = self.subscription_state_mut(provider)
                {
                    *view = subscription_provider::ready_view(
                        provider,
                        version,
                        format!(
                            "{} completed the run; ready for another request.",
                            provider.label()
                        ),
                    );
                }
            }
            Err(message) => {
                self.finalize_agent_activities(
                    result.run_id,
                    AgentStepStatus::Error,
                    Some(&message),
                );
                if let Some(run) = self.agent_run_for_id_mut(result.run_id) {
                    run.phase = AgentPhase::Error;
                    run.status = message.clone();
                }
                self.push_chat_message_for_run(
                    result.run_id,
                    ChatRole::System,
                    format!("{} run did not complete: {message}", provider.label()),
                    Vec::new(),
                );
                let authentication_error = message.contains("401")
                    || message.to_ascii_lowercase().contains("authentication")
                    || message.to_ascii_lowercase().contains("login")
                    || message.to_ascii_lowercase().contains("oauth");
                if !self.has_subscription_jobs(provider)
                    && let Some((view, _, _)) = self.subscription_state_mut(provider)
                {
                    *view = if authentication_error {
                        AgentProviderView::unavailable(
                            format!(
                                "{} rejected the saved account session. Choose Connect in Settings.",
                                provider.label()
                            ),
                            version,
                        )
                    } else {
                        subscription_provider::ready_view(
                            provider,
                            version,
                            format!("Last run ended with an error: {message}. Ready to retry."),
                        )
                    };
                }
            }
        }
        self.finalize_run_artifacts(result.run_id);
        self.finish_run_when_quiescent(result.run_id);
        self.save_session();
        self.render_agent_panel();
    }

    fn mark_subscription_step_waiting(&mut self, tool_use_id: Option<&str>) {
        let Some(tool_use_id) = tool_use_id else {
            return;
        };
        let Some(index) = self.subscription_tool_steps.get(tool_use_id).copied() else {
            return;
        };
        let Some(run_id) = self.subscription_tool_run_ids.get(tool_use_id).copied() else {
            return;
        };
        let changed = if let Some(run) = self.agent_run_for_id_mut(run_id)
            && let Some(step) = run.steps.get_mut(index)
        {
            step.status = AgentStepStatus::AwaitingApproval;
            true
        } else {
            false
        };
        if changed {
            self.update_agent_activity_status(
                run_id,
                tool_use_id,
                AgentStepStatus::AwaitingApproval,
                None,
            );
        }
    }

    fn mark_subscription_step_running(&mut self, tool_use_id: Option<&str>) {
        let Some(tool_use_id) = tool_use_id else {
            return;
        };
        let Some(index) = self.subscription_tool_steps.get(tool_use_id).copied() else {
            return;
        };
        let Some(run_id) = self.subscription_tool_run_ids.get(tool_use_id).copied() else {
            return;
        };
        let changed = if let Some(run) = self.agent_run_for_id_mut(run_id)
            && let Some(step) = run.steps.get_mut(index)
        {
            step.status = AgentStepStatus::Running;
            run.phase = phase_for_step(step.kind);
            run.status = step.label.clone();
            true
        } else {
            false
        };
        if changed {
            self.update_agent_activity_status(run_id, tool_use_id, AgentStepStatus::Running, None);
        }
    }

    fn send_subscription_permission_result(
        &mut self,
        provider: AgentProviderKind,
        run_id: u64,
        request_id: String,
        allow: bool,
        message: &str,
    ) {
        let result = self
            .subscription_job_for_id(provider, run_id)
            .ok_or_else(|| format!("{} session is unavailable", provider.label()))
            .and_then(|job| {
                job.handle
                    .send_permission_result(SubscriptionPermissionResult {
                        request_id,
                        allow,
                        message: message.to_owned(),
                    })
            });
        if let Err(error) = result {
            if let Some(run) = self.agent_run_for_id_mut(run_id) {
                run.phase = AgentPhase::Error;
                run.status = error.clone();
            }
            self.push_chat_message_for_run(run_id, ChatRole::System, error, Vec::new());
        }
    }

    fn mark_claude_step_waiting(&mut self, tool_use_id: Option<&str>) {
        let Some(tool_use_id) = tool_use_id else {
            return;
        };
        let Some(index) = self.claude_tool_steps.get(tool_use_id).copied() else {
            return;
        };
        let Some(run_id) = self.claude_tool_run_ids.get(tool_use_id).copied() else {
            return;
        };
        let changed = if let Some(run) = self.agent_run_for_id_mut(run_id)
            && let Some(step) = run.steps.get_mut(index)
        {
            step.status = AgentStepStatus::AwaitingApproval;
            true
        } else {
            false
        };
        if changed {
            self.update_agent_activity_status(
                run_id,
                tool_use_id,
                AgentStepStatus::AwaitingApproval,
                None,
            );
        }
    }

    fn mark_claude_step_running(&mut self, tool_use_id: Option<&str>) {
        let Some(tool_use_id) = tool_use_id else {
            return;
        };
        let Some(index) = self.claude_tool_steps.get(tool_use_id).copied() else {
            return;
        };
        let Some(run_id) = self.claude_tool_run_ids.get(tool_use_id).copied() else {
            return;
        };
        let changed = if let Some(run) = self.agent_run_for_id_mut(run_id)
            && let Some(step) = run.steps.get_mut(index)
        {
            step.status = AgentStepStatus::Running;
            run.phase = phase_for_step(step.kind);
            run.status = step.label.clone();
            true
        } else {
            false
        };
        if changed {
            self.update_agent_activity_status(run_id, tool_use_id, AgentStepStatus::Running, None);
        }
    }

    fn send_claude_permission_result(
        &mut self,
        run_id: u64,
        request_id: String,
        allow: bool,
        message: &str,
    ) {
        let result = self
            .claude_job_for_id(run_id)
            .ok_or_else(|| "Claude Code session is unavailable".to_owned())
            .and_then(|job| {
                job.handle.send_permission_result(ClaudePermissionResult {
                    request_id,
                    allow,
                    message: message.to_owned(),
                })
            });
        if let Err(error) = result {
            if let Some(run) = self.agent_run_for_id_mut(run_id) {
                run.phase = AgentPhase::Error;
                run.status = error.clone();
            }
            self.push_chat_message_for_run(run_id, ChatRole::System, error, Vec::new());
        }
    }

    fn mark_claude_action_status(&mut self, request_id: &str, status: CommandStatus, detail: &str) {
        let metadata = self
            .action_log
            .iter_mut()
            .find(|entry| entry.request_id == request_id)
            .map(|entry| {
                entry.status = status;
                (
                    entry.action.clone(),
                    entry.summary.clone(),
                    entry.scope,
                    entry.effect,
                )
            });
        if let Some((action, summary, scope, effect)) = metadata {
            self.audit_command_event(
                "command_finished",
                request_id,
                &action,
                ActionAuthorization::new(scope, effect),
                status,
                &summary,
            );
            self.last_result = Some(CommandResultView {
                request_id: request_id.to_owned(),
                action,
                ok: status == CommandStatus::Success,
                message: detail.to_owned(),
                data: Value::Null,
            });
        }
    }

    fn stop_agent_run(&mut self) {
        if self.stop_app_server(&self.conversation_key(None)) {
            return;
        }
        let Some(run_id) = self
            .active_main_run()
            .filter(|run| run.is_active())
            .map(|run| run.id)
        else {
            return;
        };
        self.stop_agent_run_by_id(run_id);
    }

    fn stop_agent_run_by_id(&mut self, run_id: u64) {
        let (run_id, active_request_id, _active_call_id) = {
            let Some(run) = self.agent_run_for_id(run_id) else {
                return;
            };
            if !run.is_active() {
                return;
            }
            (
                run.id,
                run.active_request_id.clone(),
                run.active_provider_call_id.clone(),
            )
        };
        self.finalize_agent_activities(
            run_id,
            AgentStepStatus::Denied,
            Some("Run stopped by the user"),
        );
        if self.claude_job_for_id(run_id).is_some() {
            let pending_ids = self
                .pending_claude_permissions
                .iter()
                .filter_map(|(id, request)| (request.run_id == run_id).then_some(id.clone()))
                .collect::<Vec<_>>();
            let pending = pending_ids
                .into_iter()
                .filter_map(|id| self.pending_claude_permissions.remove(&id))
                .collect::<Vec<_>>();
            for request in pending {
                self.mark_claude_action_status(
                    &request.request_id,
                    CommandStatus::Denied,
                    "Run stopped by the user",
                );
            }
            if let Some(job) = self.claude_job_for_id(run_id) {
                job.handle.cancel();
            }
            for message in &mut self.chat_messages {
                if message.run_id == Some(run_id) {
                    message.streaming = false;
                }
            }
            let stopped_tool_ids = self
                .claude_tool_run_ids
                .iter()
                .filter_map(|(tool_id, owner)| (*owner == run_id).then_some(tool_id.clone()))
                .collect::<Vec<_>>();
            for tool_id in stopped_tool_ids {
                self.claude_tool_run_ids.remove(&tool_id);
                self.claude_tool_steps.remove(&tool_id);
                self.claude_tool_request_ids.remove(&tool_id);
            }
            if let Some(run) = self.agent_run_for_id_mut(run_id) {
                for step in &mut run.steps {
                    if matches!(
                        step.status,
                        AgentStepStatus::Running | AgentStepStatus::AwaitingApproval
                    ) {
                        step.status = AgentStepStatus::Denied;
                    }
                }
                run.phase = AgentPhase::Stopped;
                run.status = "Run stopped".to_owned();
            }
            self.push_chat_message_for_run(
                run_id,
                ChatRole::Assistant,
                "Run stopped. Claude Code will not start further actions.".to_owned(),
                Vec::new(),
            );
            if !self.has_claude_jobs() {
                self.claude_provider = claude_provider::ready_view(
                    self.claude_provider.version.clone(),
                    "Run stopped; ready for a new request.",
                );
            }
            self.processes.cancel_owner(run_id);
            self.finish_run_when_quiescent(run_id);
            self.finish_run_timing(run_id, "stopped");
            self.save_session();
            self.render_agent_panel();
            return;
        }
        if let Some(provider) = self.subscription_jobs.get(&run_id).map(|job| job.provider) {
            let pending_ids = self
                .pending_subscription_permissions
                .iter()
                .filter_map(|(id, request)| (request.run_id == run_id).then_some(id.clone()))
                .collect::<Vec<_>>();
            let pending = pending_ids
                .into_iter()
                .filter_map(|id| self.pending_subscription_permissions.remove(&id))
                .collect::<Vec<_>>();
            for request in pending {
                self.mark_claude_action_status(
                    &request.request_id,
                    CommandStatus::Denied,
                    "Run stopped by the user",
                );
            }
            if let Some(job) = self.subscription_job_for_id(provider, run_id) {
                job.handle.cancel();
            }
            for message in &mut self.chat_messages {
                if message.run_id == Some(run_id) {
                    message.streaming = false;
                }
            }
            let stopped_tool_ids = self
                .subscription_tool_run_ids
                .iter()
                .filter_map(|(tool_id, owner)| (*owner == run_id).then_some(tool_id.clone()))
                .collect::<Vec<_>>();
            for tool_id in stopped_tool_ids {
                self.subscription_tool_run_ids.remove(&tool_id);
                self.subscription_tool_steps.remove(&tool_id);
                self.subscription_tool_request_ids.remove(&tool_id);
            }
            if let Some(run) = self.agent_run_for_id_mut(run_id) {
                for step in &mut run.steps {
                    if matches!(
                        step.status,
                        AgentStepStatus::Running | AgentStepStatus::AwaitingApproval
                    ) {
                        step.status = AgentStepStatus::Denied;
                    }
                }
                run.phase = AgentPhase::Stopped;
                run.status = "Run stopped".to_owned();
            }
            self.push_chat_message_for_run(
                run_id,
                ChatRole::Assistant,
                format!(
                    "Run stopped. {} will not start further actions.",
                    provider.label()
                ),
                Vec::new(),
            );
            let version = self
                .subscription_state(provider)
                .and_then(|state| state.0.version.clone());
            if !self.has_subscription_jobs(provider)
                && let Some((view, _, _)) = self.subscription_state_mut(provider)
            {
                *view = subscription_provider::ready_view(
                    provider,
                    version,
                    "Run stopped; ready for a new request.",
                );
            }
            self.processes.cancel_owner(run_id);
            self.finish_run_when_quiescent(run_id);
            self.finish_run_timing(run_id, "stopped");
            self.save_session();
            self.render_agent_panel();
            return;
        }

        if let Some(request_id) = active_request_id.as_deref() {
            self.remove_pending_ssh_runtime_command(request_id);
        }

        self.processes.cancel_owner(run_id);
        if let Some(request_id) = active_request_id.as_deref() {
            self.provider_tool_images.remove(request_id);
        }

        if let Some(request_id) = active_request_id.as_deref()
            && self.pending_commands.contains_key(request_id)
        {
            self.deny_pending_action(request_id, "Run stopped by the user");
        }

        if let Some(request_id) = active_request_id.as_deref()
            && self.terminal.is_busy()
        {
            if let Err(error) = self.terminal.cancel_agent_command(request_id) {
                warn!(%error, "active terminal command could not be interrupted");
            }
            if let Some(entry) = self
                .action_log
                .iter_mut()
                .find(|entry| entry.request_id == request_id)
            {
                entry.status = CommandStatus::Denied;
            }
        }

        if let Some(run) = self.agent_run_for_id_mut(run_id) {
            if let Some(step) = run.steps.get_mut(run.current_index) {
                step.status = AgentStepStatus::Denied;
            }
            run.active_request_id = None;
            run.active_provider_call_id = None;
            run.phase = AgentPhase::Stopped;
            run.status = "Run stopped".to_owned();
        }
        self.push_chat_message_for_run(
            run_id,
            ChatRole::Assistant,
            "Run stopped. No further steps will be started.".to_owned(),
            Vec::new(),
        );
        self.finalize_run_artifacts(run_id);

        self.finish_run_when_quiescent(run_id);
        self.finish_run_timing(run_id, "stopped");
        self.save_session();
        self.render_agent_panel();
    }

    fn push_chat_message(
        &mut self,
        role: ChatRole,
        text: String,
        attachments: Vec<ChatTabAttachment>,
    ) {
        self.push_chat_message_with_contexts(role, text, attachments, Vec::new(), Vec::new());
    }

    fn note_provider_event(&mut self, run_id: u64, event: &'static str) {
        let now_ms = unix_time_ms();
        let Some(timing) = self.run_timings.get_mut(&run_id) else {
            return;
        };
        if timing.first_provider_event_at_ms.is_none() {
            timing.first_provider_event_at_ms = Some(now_ms);
            info!(
                run_id,
                event,
                first_event_ms = now_ms.saturating_sub(timing.accepted_at_ms),
                "agent provider produced its first event"
            );
        }
    }

    fn finish_run_timing(&mut self, run_id: u64, outcome: &'static str) {
        let Some(timing) = self.run_timings.remove(&run_id) else {
            return;
        };
        let finished_at_ms = unix_time_ms();
        info!(
            run_id,
            outcome,
            total_ms = finished_at_ms.saturating_sub(timing.accepted_at_ms),
            first_event_ms = timing
                .first_provider_event_at_ms
                .map(|first| first.saturating_sub(timing.accepted_at_ms)),
            "agent run timing completed"
        );
    }

    fn push_chat_message_for_run(
        &mut self,
        run_id: u64,
        role: ChatRole,
        text: String,
        attachments: Vec<ChatTabAttachment>,
    ) {
        let provider = self
            .run_execution_contexts
            .get(&run_id)
            .map(|context| context.provider);
        self.push_chat_message(role, text, attachments);
        if let Some(message) = self.chat_messages.back_mut() {
            message.run_id = Some(run_id);
            message.provider = provider;
        }
    }

    fn push_agent_activity(
        &mut self,
        run_id: u64,
        provider: AgentProviderKind,
        item_id: String,
        label: &str,
        metadata: AgentActivityMetadata,
    ) {
        if self.agent_run_for_id(run_id).is_none() {
            return;
        }
        let id = self.next_chat_message_id;
        self.next_chat_message_id += 1;
        self.chat_messages.push_back(ChatMessage::activity(
            id, run_id, item_id, provider, label, metadata,
        ));
    }

    fn update_agent_activity_status(
        &mut self,
        run_id: u64,
        item_id: &str,
        status: AgentStepStatus,
        detail: Option<&str>,
    ) {
        let Some(message) = self
            .chat_messages
            .iter_mut()
            .find(|message| message.is_activity(run_id, item_id))
        else {
            return;
        };
        message.set_activity_status(status, detail);
        let message_id = message.id;
        self.stream_agent_message(message_id);
        self.render_agent_graph_surface();
    }

    fn push_chat_message_with_contexts(
        &mut self,
        role: ChatRole,
        text: String,
        attachments: Vec<ChatTabAttachment>,
        terminal_attachments: Vec<ChatTerminalAttachment>,
        file_attachments: Vec<FileAttachmentView>,
    ) {
        let id = self.next_chat_message_id;
        self.next_chat_message_id += 1;
        if let Some(chat_id) = self.active_project_chat_id.as_deref() {
            self.chat_ownership.claim(id, chat_id);
        }
        self.chat_messages.push_back(ChatMessage {
            id,
            role,
            kind: ChatMessageKind::Message,
            text,
            streaming: false,
            timestamp_ms: unix_time_ms(),
            attachments,
            terminal_attachments,
            file_attachments,
            artifacts: Vec::new(),
            provider: None,
            checkpoint: None,
            activity_status: None,
            activity_category: None,
            activity_detail: None,
            activity_context: None,
            activity_additions: None,
            activity_deletions: None,
            activity_diff: None,
            run_id: None,
            provider_item_id: None,
            reasoning_summary_index: None,

            native_turn_id: None,
        });
    }

    fn mark_agent_waiting(&mut self, request_id: &str) {
        let Some(run_id) = self.run_id_for_request(request_id) else {
            return;
        };
        let call_id = {
            let Some(run) = self.agent_run_for_id_mut(run_id) else {
                return;
            };
            if let Some(step) = run.steps.get_mut(run.current_index) {
                step.status = AgentStepStatus::AwaitingApproval;
            }
            run.phase = AgentPhase::WaitingApproval;
            run.status = "Waiting for authorization".to_owned();
            run.active_provider_call_id.clone()
        };
        if let Some(call_id) = call_id {
            self.update_agent_activity_status(
                run_id,
                &call_id,
                AgentStepStatus::AwaitingApproval,
                None,
            );
        }
    }

    fn mark_agent_running(&mut self, request_id: &str) {
        let Some(run_id) = self.run_id_for_request(request_id) else {
            return;
        };
        let call_id = {
            let Some(run) = self.agent_run_for_id_mut(run_id) else {
                return;
            };
            if let Some(step) = run.steps.get_mut(run.current_index) {
                step.status = AgentStepStatus::Running;
                run.phase = phase_for_step(step.kind);
                run.status = step.label.clone();
            }
            run.active_provider_call_id.clone()
        };
        if let Some(call_id) = call_id {
            self.update_agent_activity_status(run_id, &call_id, AgentStepStatus::Running, None);
        }
    }

    fn submit_agent_command(&mut self, request: AgentCommandRequest) {
        let request_id = request.request_id.clone();
        let action = request.command.name().to_owned();
        let summary = request.command.summary();
        let authorization = self.command_authorization(&request.command);

        if self
            .action_log
            .iter()
            .any(|entry| entry.request_id == request_id)
        {
            warn!(%request_id, "duplicate request_id ignored");
            return;
        }

        if self.safety.paused() && Self::command_needs_computer_control(&request.command) {
            self.begin_command(&request_id, &action, summary, authorization);
            self.finish_command(
                request_id,
                action,
                false,
                "Computer control is paused; resume it explicitly in Settings".to_owned(),
                Value::Null,
            );
            return;
        }

        if let Err(message) = request.validate() {
            self.begin_command(&request_id, &action, summary, authorization);
            self.finish_command(request_id, action, false, message.to_owned(), Value::Null);
            return;
        }

        if !request.command.requires_approval() {
            self.execute_agent_command(request);
            return;
        }

        match self.permission_policy.decision(authorization) {
            PermissionDecision::Execute => self.execute_agent_command(request),
            PermissionDecision::NeedsInitialAuthorization
            | PermissionDecision::NeedsActionApproval => {
                self.audit_command_event(
                    "approval_required",
                    &request_id,
                    &action,
                    authorization,
                    CommandStatus::AwaitingApproval,
                    &summary,
                );
                self.action_log.push_front(ActionLogEntry {
                    request_id: request_id.clone(),
                    action,
                    summary,
                    scope: authorization.scope,
                    effect: authorization.effect,
                    status: CommandStatus::AwaitingApproval,
                    timestamp_ms: unix_time_ms(),
                });
                self.pending_commands.insert(request_id.clone(), request);
                self.mark_agent_waiting(&request_id);
                self.render_agent_panel();
            }
            PermissionDecision::Denied => {
                self.begin_command(&request_id, &action, summary, authorization);
                self.finish_command_with_status(
                    request_id,
                    action,
                    false,
                    "This privileged action is not available to Supervisor".to_owned(),
                    Value::Null,
                    CommandStatus::Denied,
                );
            }
        }
    }

    fn command_authorization(&self, command: &AgentCommand) -> ActionAuthorization {
        let mut authorization = command.authorization();
        let destructive_reference = match command {
            AgentCommand::UiAutomation(UiAutomationCommand::Invoke { element_id })
            | AgentCommand::UiAutomation(UiAutomationCommand::Select { element_id }) => {
                self.ui_automation.is_destructive_reference(element_id)
            }
            _ => false,
        };
        if destructive_reference {
            authorization.effect = ActionEffect::Destructive;
        }
        authorization
    }

    fn command_needs_computer_control(command: &AgentCommand) -> bool {
        matches!(
            command,
            AgentCommand::RemoteDesktop(_)
                | AgentCommand::Capture(_)
                | AgentCommand::UiAutomation(_)
                | AgentCommand::Window(
                    WindowCommand::Focus { .. }
                        | WindowCommand::MoveResize { .. }
                        | WindowCommand::Minimize { .. }
                        | WindowCommand::Restore { .. }
                        | WindowCommand::Close { .. }
                )
        )
    }

    fn set_permission_mode(&mut self, mode: PermissionMode) {
        self.cancel_pending_action("Action cancelled after changing permission mode");
        self.permission_policy.set_mode(mode);
        self.audit.record(
            "permission_mode_changed",
            "",
            "permission_mode",
            "runtime",
            "authorization",
            &serialized_label(&mode),
            "Permission mode changed by the trusted local UI",
        );
        self.render_agent_panel();
    }

    fn authorize_session(&mut self) {
        let mut scopes = vec![
            CapabilityScope::Browser,
            CapabilityScope::Terminal,
            CapabilityScope::Workspace,
            CapabilityScope::Process,
        ];
        if self.window_access_enabled {
            scopes.push(CapabilityScope::Window);
            scopes.push(CapabilityScope::Capture);
            scopes.push(CapabilityScope::UiAutomation);
        }
        if self.terminal.has_agent_ready_ssh_session() {
            scopes.push(CapabilityScope::Ssh);
        }
        if self.remote_desktop.agent_control_enabled() && self.remote_desktop.is_connected() {
            scopes.push(CapabilityScope::RemoteDesktop);
        }
        if !self.permission_policy.authorize_scopes(scopes) {
            warn!("session authorization ignored in the current mode");
            return;
        }

        let pending = self
            .pending_commands
            .drain()
            .map(|(_, request)| request)
            .collect::<Vec<_>>();
        let (executable, deferred) =
            partition_session_authorized(&self.permission_policy, pending, |request| {
                self.command_authorization(&request.command)
            });
        for request in deferred {
            self.pending_commands
                .insert(request.request_id.clone(), request);
        }
        for request in executable {
            self.execute_agent_command(request);
        }

        let claude_pending = self
            .pending_claude_permissions
            .drain()
            .map(|(_, request)| request)
            .collect::<Vec<_>>();
        let (claude_executable, claude_deferred) =
            partition_session_authorized(&self.permission_policy, claude_pending, |request| {
                request.authorization
            });
        for request in claude_deferred {
            self.pending_claude_permissions
                .insert(request.request_id.clone(), request);
        }
        for request in claude_executable {
            self.run_after_time_machine_checkpoint(PendingTimeMachineAction::ClaudePermission {
                request,
                approval_message: "Approved by the initial Supervisor session authorization"
                    .to_owned(),
            });
        }
        let subscription_pending = self
            .pending_subscription_permissions
            .drain()
            .map(|(_, request)| request)
            .collect::<Vec<_>>();
        let (subscription_executable, subscription_deferred) = partition_session_authorized(
            &self.permission_policy,
            subscription_pending,
            |request| request.authorization,
        );
        for request in subscription_deferred {
            self.pending_subscription_permissions
                .insert(request.request_id.clone(), request);
        }
        for request in subscription_executable {
            self.run_after_time_machine_checkpoint(
                PendingTimeMachineAction::SubscriptionPermission {
                    request,
                    approval_message: "Approved by the initial Supervisor session authorization"
                        .to_owned(),
                },
            );
        }
        self.render_agent_panel();
    }

    fn approve_pending_action(&mut self, request_id: &str) {
        if let Some(request) = self.pending_commands.remove(request_id) {
            self.execute_agent_command(request);
            return;
        }
        if let Some(request) = self.pending_claude_permissions.remove(request_id) {
            self.run_after_time_machine_checkpoint(PendingTimeMachineAction::ClaudePermission {
                request,
                approval_message: "Approved once by the user".to_owned(),
            });
        } else if let Some(request) = self.pending_subscription_permissions.remove(request_id) {
            self.run_after_time_machine_checkpoint(
                PendingTimeMachineAction::SubscriptionPermission {
                    request,
                    approval_message: "Approved once by the user".to_owned(),
                },
            );
        } else {
            warn!(%request_id, "no action is awaiting approval");
            return;
        }
        self.render_agent_panel();
    }

    fn deny_pending_action(&mut self, request_id: &str, message: &str) {
        if let Some(request) = self.pending_commands.remove(request_id) {
            self.finish_command_with_status(
                request.request_id,
                request.command.name().to_owned(),
                false,
                message.to_owned(),
                Value::Null,
                CommandStatus::Denied,
            );
            return;
        }
        if let Some(request) = self.pending_claude_permissions.remove(request_id) {
            self.mark_claude_step_waiting(request.tool_use_id.as_deref());
            if let Some(tool_use_id) = request.tool_use_id.as_deref()
                && let Some(index) = self.claude_tool_steps.get(tool_use_id).copied()
                && let Some(run) = self.agent_run_for_id_mut(request.run_id)
                && let Some(step) = run.steps.get_mut(index)
            {
                step.status = AgentStepStatus::Denied;
            }
            if let Some(tool_use_id) = request.tool_use_id.as_deref() {
                self.update_agent_activity_status(
                    request.run_id,
                    tool_use_id,
                    AgentStepStatus::Denied,
                    Some(message),
                );
            }
            self.mark_claude_action_status(&request.request_id, CommandStatus::Denied, message);
            self.send_claude_permission_result(request.run_id, request.request_id, false, message);
        } else if let Some(request) = self.pending_subscription_permissions.remove(request_id) {
            self.mark_subscription_step_waiting(request.tool_use_id.as_deref());
            if let Some(tool_use_id) = request.tool_use_id.as_deref()
                && let Some(index) = self.subscription_tool_steps.get(tool_use_id).copied()
                && let Some(run) = self.agent_run_for_id_mut(request.run_id)
                && let Some(step) = run.steps.get_mut(index)
            {
                step.status = AgentStepStatus::Denied;
            }
            if let Some(tool_use_id) = request.tool_use_id.as_deref() {
                self.update_agent_activity_status(
                    request.run_id,
                    tool_use_id,
                    AgentStepStatus::Denied,
                    Some(message),
                );
            }
            self.mark_claude_action_status(&request.request_id, CommandStatus::Denied, message);
            self.send_subscription_permission_result(
                request.provider,
                request.run_id,
                request.request_id,
                false,
                message,
            );
        } else {
            warn!(%request_id, "no action is awaiting denial");
            return;
        }
        self.render_agent_panel();
    }

    fn cancel_pending_action(&mut self, message: &str) {
        let pending = self
            .pending_commands
            .drain()
            .map(|(_, request)| request)
            .collect::<Vec<_>>();
        for request in pending {
            self.finish_command_with_status(
                request.request_id,
                request.command.name().to_owned(),
                false,
                message.to_owned(),
                Value::Null,
                CommandStatus::Denied,
            );
        }
        let claude_pending = self
            .pending_claude_permissions
            .drain()
            .map(|(_, request)| request)
            .collect::<Vec<_>>();
        for request in claude_pending {
            self.mark_claude_action_status(&request.request_id, CommandStatus::Denied, message);
            self.send_claude_permission_result(request.run_id, request.request_id, false, message);
        }
        let subscription_pending = self
            .pending_subscription_permissions
            .drain()
            .map(|(_, request)| request)
            .collect::<Vec<_>>();
        for request in subscription_pending {
            self.mark_claude_action_status(&request.request_id, CommandStatus::Denied, message);
            self.send_subscription_permission_result(
                request.provider,
                request.run_id,
                request.request_id,
                false,
                message,
            );
        }
    }

    fn execute_workspace_command_for_run(
        &self,
        run_id: Option<u64>,
        command: &WorkspaceCommand,
    ) -> Result<Value, String> {
        match run_id {
            Some(run_id) => self.workspace_for_run(run_id)?.execute(command),
            None => self.workspace.execute(command),
        }
    }

    fn resolve_workspace_directory_for_run(
        &self,
        run_id: Option<u64>,
        relative: Option<&str>,
    ) -> Result<PathBuf, String> {
        match run_id {
            Some(run_id) => self.workspace_for_run(run_id)?.resolve_directory(relative),
            None => self.workspace.resolve_directory(relative),
        }
    }

    fn execute_agent_command(&mut self, request: AgentCommandRequest) {
        if let Some(run_id) = self.run_id_for_request(&request.request_id)
            && request.command.may_mutate_workspace()
        {
            self.run_after_time_machine_checkpoint(PendingTimeMachineAction::AgentCommand {
                run_id,
                request,
            });
            return;
        }
        self.execute_agent_command_now(request);
    }

    fn execute_agent_command_now(&mut self, request: AgentCommandRequest) {
        let owner_run_id = self.run_id_for_request(&request.request_id);
        let owner_scope = self
            .process_owner_scope_for_run(owner_run_id)
            .unwrap_or_else(|| format!("request:{}", request.request_id));
        let request_id = request.request_id;
        let action = request.command.name().to_owned();
        let summary = request.command.summary();
        let authorization = self.command_authorization(&request.command);
        self.mark_agent_running(&request_id);
        self.begin_command(&request_id, &action, summary, authorization);

        if self.safety.paused() && Self::command_needs_computer_control(&request.command) {
            self.finish_command(
                request_id,
                action,
                false,
                "Computer control was paused before execution".to_owned(),
                Value::Null,
            );
            return;
        }

        match request.command {
            AgentCommand::Browser(BrowserCommand::InspectPage) => {
                self.run_script_command(request_id, action, agent_scripts::INSPECT_PAGE.to_owned());
            }
            AgentCommand::Browser(BrowserCommand::Click { target }) => {
                self.run_script_command(request_id, action, agent_scripts::click(&target));
            }
            AgentCommand::Browser(BrowserCommand::SetText { target, value }) => {
                self.run_script_command(
                    request_id,
                    action,
                    agent_scripts::set_text(&target, &value),
                );
            }
            AgentCommand::Browser(BrowserCommand::Scroll { delta_y }) => {
                self.run_script_command(request_id, action, agent_scripts::scroll(delta_y));
            }
            AgentCommand::Browser(command) => {
                let result = self.execute_immediate_command(owner_run_id, command);
                match result {
                    Ok(data) => self.finish_command(
                        request_id,
                        action,
                        true,
                        "Command completed".to_owned(),
                        data,
                    ),
                    Err(message) => {
                        self.finish_command(request_id, action, false, message, Value::Null)
                    }
                }
            }
            AgentCommand::Terminal(TerminalCommand::Run { command }) => {
                self.execute_terminal_command(request_id, action, command);
            }
            AgentCommand::Ssh(SshCommand::Run {
                profile_id,
                profile_name,
                command,
            }) => self.execute_ssh_command(request_id, action, profile_id, profile_name, command),
            AgentCommand::Workspace(WorkspaceCommand::Run { command, path }) => {
                self.execute_managed_process_start(
                    request_id,
                    action,
                    command,
                    path,
                    owner_run_id,
                    owner_scope,
                );
            }
            AgentCommand::Workspace(command) => {
                let refresh_explorer = matches!(&command, WorkspaceCommand::ApplyPatch { .. });
                match self.execute_workspace_command_for_run(owner_run_id, &command) {
                    Ok(data) => {
                        if refresh_explorer
                            && owner_run_id
                                .and_then(|run_id| self.run_execution_contexts.get(&run_id))
                                .and_then(|context| context.workspace_root.as_ref())
                                .is_none_or(|scope| self.workspace.root() == Some(scope.as_path()))
                        {
                            self.workspace.refresh_explorer();
                        }
                        self.finish_command(
                            request_id,
                            action,
                            true,
                            "Workspace operation completed".to_owned(),
                            data,
                        );
                    }
                    Err(message) => {
                        self.finish_command(request_id, action, false, message, Value::Null);
                    }
                }
            }
            AgentCommand::Process(ProcessCommand::List) => self.finish_command(
                request_id,
                action,
                true,
                "Background agent command inventory read".to_owned(),
                self.processes.list_value_for_owner(&owner_scope),
            ),
            AgentCommand::Process(ProcessCommand::Status {
                process_id,
                stdout_offset,
                stderr_offset,
            }) => {
                match self.processes.status_value_for_owner(
                    process_id,
                    stdout_offset,
                    stderr_offset,
                    &owner_scope,
                ) {
                    Ok(data) => self.finish_command(
                        request_id,
                        action,
                        true,
                        "Managed process status read".to_owned(),
                        data,
                    ),
                    Err(message) => {
                        self.finish_command(request_id, action, false, message, Value::Null)
                    }
                }
            }
            AgentCommand::Process(ProcessCommand::Start { command, path }) => self
                .execute_managed_process_start(
                    request_id,
                    action,
                    command,
                    path,
                    owner_run_id,
                    owner_scope,
                ),
            AgentCommand::Process(ProcessCommand::WriteStdin {
                process_id,
                data,
                close,
            }) => match self
                .processes
                .write_stdin_for_owner(process_id, &data, close, &owner_scope)
            {
                Ok(result) => self.finish_command(
                    request_id,
                    action,
                    true,
                    "Managed process input sent".to_owned(),
                    result,
                ),
                Err(message) => {
                    self.finish_command(request_id, action, false, message, Value::Null)
                }
            },
            AgentCommand::Process(ProcessCommand::Cancel { process_id }) => {
                match self.processes.cancel_for_owner(process_id, &owner_scope) {
                    Ok(result) => self.finish_command(
                        request_id,
                        action,
                        true,
                        "Managed process cancellation requested".to_owned(),
                        result,
                    ),
                    Err(message) => {
                        self.finish_command(request_id, action, false, message, Value::Null)
                    }
                }
            }
            AgentCommand::Window(command) => {
                if !self.window_access_enabled {
                    self.finish_command(
                        request_id,
                        action,
                        false,
                        "Window awareness is disabled in Settings".to_owned(),
                        Value::Null,
                    );
                    return;
                }
                let result = if matches!(command, WindowCommand::List) {
                    let owned_pids = self.processes.owned_pids();
                    self.windows.refresh(&owned_pids)
                } else {
                    self.windows.execute(command)
                };
                match result {
                    Ok(data) => self.finish_command(
                        request_id,
                        action,
                        true,
                        "Window operation completed".to_owned(),
                        data,
                    ),
                    Err(message) => {
                        self.finish_command(request_id, action, false, message, Value::Null)
                    }
                }
            }
            AgentCommand::Capture(CaptureCommand::Window { window_id }) => {
                if !self.window_access_enabled {
                    self.finish_command(
                        request_id,
                        action,
                        false,
                        "Window capture is disabled in Settings".to_owned(),
                        Value::Null,
                    );
                    return;
                }
                if self.preview.capturing {
                    self.finish_command(
                        request_id,
                        action,
                        false,
                        "Another window capture is already in progress".to_owned(),
                        Value::Null,
                    );
                    return;
                }
                match self.windows.take_capture_target(&window_id) {
                    Ok(target) => self.start_window_capture(Some(request_id), action, target),
                    Err(message) => {
                        self.finish_command(request_id, action, false, message, Value::Null)
                    }
                }
            }
            AgentCommand::UiAutomation(command) => {
                if !self.window_access_enabled {
                    self.finish_command(
                        request_id,
                        action,
                        false,
                        "Semantic computer control is disabled in Settings".to_owned(),
                        Value::Null,
                    );
                    return;
                }
                match command {
                    UiAutomationCommand::Inspect { window_id } => {
                        match self.windows.take_automation_target(&window_id) {
                            Ok(target) => {
                                self.start_ui_inspection(request_id, action, target);
                            }
                            Err(message) => {
                                self.finish_command(
                                    request_id,
                                    action,
                                    false,
                                    message,
                                    Value::Null,
                                );
                            }
                        }
                    }
                    command => match self.ui_automation.take_action(command) {
                        Ok((target, operation)) => {
                            self.start_ui_action(request_id, action, target, operation);
                        }
                        Err(message) => {
                            self.finish_command(request_id, action, false, message, Value::Null);
                        }
                    },
                }
            }
            AgentCommand::RemoteDesktop(command) => {
                self.execute_remote_desktop_command(request_id, action, command)
            }
        }
    }

    fn execute_remote_desktop_command(
        &mut self,
        request_id: String,
        action: String,
        command: RemoteDesktopCommand,
    ) {
        if let Err(message) = self.validate_remote_target(self.run_id_for_request(&request_id)) {
            self.finish_command(request_id, action, false, message, Value::Null);
            return;
        }
        if !self.remote_desktop.agent_control_enabled() {
            self.finish_command(
                request_id,
                action,
                false,
                "Enable Agent control in the connected Linux Desktop tab before starting the run"
                    .to_owned(),
                Value::Null,
            );
            return;
        }
        if !self.remote_desktop.is_connected() || !self.remote_desktop_ready {
            self.finish_command(
                request_id,
                action,
                false,
                "The remote Linux desktop is not connected".to_owned(),
                Value::Null,
            );
            return;
        }

        let (width, height) = self.remote_desktop.dimensions();
        let validate_point = |x: u16, y: u16| {
            (x < width && y < height).then_some(()).ok_or_else(|| {
                format!("Remote desktop coordinate {x},{y} exceeds {width}×{height}")
            })
        };
        match command {
            RemoteDesktopCommand::Observe => {
                let Some(webview) = self.remote_desktop_webview() else {
                    self.finish_command(
                        request_id,
                        action,
                        false,
                        "The remote Linux desktop surface is unavailable".to_owned(),
                        Value::Null,
                    );
                    return;
                };
                let request_json =
                    serde_json::to_string(&request_id).unwrap_or_else(|_| "\"\"".to_owned());
                let script = format!(
                    "window.captureRemoteDesktopFrame && window.captureRemoteDesktopFrame({request_json});"
                );
                if let Err(error) = webview.evaluate_script(&script) {
                    self.finish_command(
                        request_id,
                        action,
                        false,
                        format!("Remote Linux desktop capture could not start: {error}"),
                        Value::Null,
                    );
                } else {
                    self.stream_remote_desktop_agent_action(json!({
                        "kind": "observe",
                        "label": "Agent is observing the desktop",
                    }));
                }
            }
            RemoteDesktopCommand::MovePointer { x, y } => {
                if let Err(message) = validate_point(x, y) {
                    self.finish_command(request_id, action, false, message, Value::Null);
                    return;
                }
                self.remote_desktop.pointer(x, y, 0);
                self.stream_remote_desktop_agent_action(json!({
                    "kind": "move_pointer", "x": x, "y": y,
                    "label": "Agent moved the pointer",
                }));
                self.finish_command(
                    request_id,
                    action,
                    true,
                    "Remote pointer moved".to_owned(),
                    json!({ "x": x, "y": y, "width": width, "height": height }),
                );
            }
            RemoteDesktopCommand::Click { x, y, button } => {
                if let Err(message) = validate_point(x, y) {
                    self.finish_command(request_id, action, false, message, Value::Null);
                    return;
                }
                let mask = match button {
                    RemoteDesktopPointerButton::Left => 1,
                    RemoteDesktopPointerButton::Middle => 2,
                    RemoteDesktopPointerButton::Right => 4,
                };
                self.remote_desktop.pointer(x, y, mask);
                self.remote_desktop.pointer(x, y, 0);
                self.stream_remote_desktop_agent_action(json!({
                    "kind": "click", "x": x, "y": y,
                    "label": format!("Agent clicked {button:?}"),
                }));
                self.finish_command(
                    request_id,
                    action,
                    true,
                    "Remote desktop click sent".to_owned(),
                    json!({ "x": x, "y": y, "button": format!("{button:?}").to_lowercase() }),
                );
            }
            RemoteDesktopCommand::Scroll { x, y, delta_y } => {
                if let Err(message) = validate_point(x, y) {
                    self.finish_command(request_id, action, false, message, Value::Null);
                    return;
                }
                self.remote_desktop
                    .pointer(x, y, if delta_y < 0 { 8 } else { 16 });
                self.remote_desktop.pointer(x, y, 0);
                self.stream_remote_desktop_agent_action(json!({
                    "kind": "scroll", "x": x, "y": y,
                    "label": "Agent scrolled the desktop",
                }));
                self.finish_command(
                    request_id,
                    action,
                    true,
                    "Remote desktop scroll sent".to_owned(),
                    json!({ "x": x, "y": y, "deltaY": delta_y }),
                );
            }
            RemoteDesktopCommand::Key {
                code,
                keysym,
                modifiers,
            } => {
                let modifier_keys = modifiers
                    .iter()
                    .map(|modifier| match modifier {
                        RemoteDesktopModifier::Control => ("ControlLeft", 0xffe3),
                        RemoteDesktopModifier::Alt => ("AltLeft", 0xffe9),
                        RemoteDesktopModifier::Shift => ("ShiftLeft", 0xffe1),
                        RemoteDesktopModifier::Meta => ("MetaLeft", 0xffeb),
                    })
                    .collect::<Vec<_>>();
                let result = self.remote_desktop.press_key_with_modifiers(
                    code.clone(),
                    keysym,
                    &modifier_keys,
                );
                let shortcut = modifiers
                    .iter()
                    .map(|modifier| format!("{modifier:?}"))
                    .chain(std::iter::once(code.clone()))
                    .collect::<Vec<_>>()
                    .join("+");
                self.stream_remote_desktop_agent_action(json!({
                    "kind": "key", "label": format!("Agent pressed {shortcut}"),
                }));
                match result {
                    Ok(()) => self.finish_command(
                        request_id,
                        action,
                        true,
                        "Remote desktop key sent".to_owned(),
                        json!({ "code": code, "keysym": keysym, "modifiers": modifiers }),
                    ),
                    Err(message) => {
                        self.finish_command(request_id, action, false, message, Value::Null)
                    }
                }
            }
            RemoteDesktopCommand::TypeText { text } => {
                let character_count = text.chars().count();
                let result = self.remote_desktop.type_text(&text);
                self.stream_remote_desktop_agent_action(json!({
                    "kind": "type_text",
                    "label": format!("Agent typed {character_count} character(s)"),
                }));
                match result {
                    Ok(()) => self.finish_command(
                        request_id,
                        action,
                        true,
                        "Remote desktop text sent".to_owned(),
                        json!({ "characterCount": character_count }),
                    ),
                    Err(message) => {
                        self.finish_command(request_id, action, false, message, Value::Null)
                    }
                }
            }
        }
    }

    fn execute_terminal_command(&mut self, request_id: String, action: String, command: String) {
        let result = if let Some(run_id) = self.run_id_for_request(&request_id) {
            self.local_terminal_for_run(run_id)
                .and_then(|(session_id, cwd)| {
                    self.terminal.run_agent_command_in_session(
                        session_id,
                        &cwd,
                        request_id.clone(),
                        &command,
                    )
                })
        } else {
            if !self.terminal.has_local_session() {
                let callback = self.terminal_callback();
                if let Err(message) = self.terminal.ensure_started(callback) {
                    self.finish_command(request_id, action, false, message, Value::Null);
                    return;
                }
            }
            self.terminal
                .run_agent_command(request_id.clone(), &command)
        };
        if let Err(message) = result {
            self.finish_command(request_id, action, false, message, Value::Null);
            return;
        }
        self.terminal_panel_visible = true;
        self.layout_webviews();
        self.render_toolbar();
        self.render_agent_panel();
    }

    fn execute_ssh_command(
        &mut self,
        request_id: String,
        action: String,
        profile_id: String,
        profile_name: String,
        command: String,
    ) {
        if let Some(run_id) = self.run_id_for_request(&request_id)
            && let Err(message) = self.validate_run_ssh_profile(run_id, &profile_id)
        {
            self.finish_command(request_id, action, false, message, Value::Null);
            return;
        }
        let Some(profile) = self.ssh.profile(&profile_id) else {
            self.finish_command(
                request_id,
                action,
                false,
                "The selected VPS profile no longer exists".to_owned(),
                Value::Null,
            );
            return;
        };
        if profile.name != profile_name || !profile.agent_enabled {
            self.finish_command(
                request_id,
                action,
                false,
                "This VPS profile is not enabled for Agent".to_owned(),
                Value::Null,
            );
            return;
        }
        if !self.terminal.has_agent_ready_ssh_profile(&profile_id) {
            self.finish_command(
                request_id,
                action,
                false,
                "Authenticate the SSH session manually and enable Agent for that live session"
                    .to_owned(),
                Value::Null,
            );
            return;
        }
        if let Some(session_id) = self.terminal.ssh_profile_session_id(&profile_id)
            && self.terminal.session_is_busy(session_id)
        {
            if let Some(run_id) = self.run_id_for_request(&request_id)
                && let Some(run) = self.agent_run_for_id_mut(run_id)
            {
                if let Some(step) = run.steps.get_mut(run.current_index) {
                    step.status = AgentStepStatus::Queued;
                }
                run.status = "Waiting for the shared SSH session…".to_owned();
            }
            self.pending_ssh_runtime_commands
                .entry(session_id)
                .or_default()
                .push_back(PendingSshRuntimeCommand {
                    request_id,
                    action,
                    profile_id,
                    profile_name,
                    command,
                });
            self.ssh.set_message(
                "Remote action queued behind another agent using this SSH session.",
                false,
            );
            self.render_agent_panel();
            return;
        }
        if let Err(message) =
            self.terminal
                .run_ssh_agent_command(&profile_id, request_id.clone(), &command)
        {
            self.finish_command(request_id, action, false, message, Value::Null);
            return;
        }
        self.terminal_panel_visible = true;
        self.layout_webviews();
        self.render_toolbar();
        self.render_agent_panel();
    }

    fn resume_pending_ssh_runtime_command(&mut self, session_id: u64) {
        let pending = self
            .pending_ssh_runtime_commands
            .get_mut(&session_id)
            .and_then(VecDeque::pop_front);
        if self
            .pending_ssh_runtime_commands
            .get(&session_id)
            .is_some_and(VecDeque::is_empty)
        {
            self.pending_ssh_runtime_commands.remove(&session_id);
        }
        let Some(pending) = pending else {
            return;
        };
        self.mark_agent_running(&pending.request_id);
        self.execute_ssh_command(
            pending.request_id,
            pending.action,
            pending.profile_id,
            pending.profile_name,
            pending.command,
        );
    }

    fn fail_pending_ssh_runtime_command(&mut self, session_id: u64, message: &str) {
        let pending = self
            .pending_ssh_runtime_commands
            .remove(&session_id)
            .unwrap_or_default();
        for pending in pending {
            self.finish_command(
                pending.request_id,
                pending.action,
                false,
                message.to_owned(),
                Value::Null,
            );
        }
    }

    fn remove_pending_ssh_runtime_command(&mut self, request_id: &str) {
        let session_ids = self
            .pending_ssh_runtime_commands
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for session_id in session_ids {
            if let Some(queue) = self.pending_ssh_runtime_commands.get_mut(&session_id) {
                queue.retain(|pending| pending.request_id != request_id);
                if queue.is_empty() {
                    self.pending_ssh_runtime_commands.remove(&session_id);
                }
            }
        }
    }

    fn execute_managed_process_start(
        &mut self,
        request_id: String,
        action: String,
        command: String,
        relative_path: Option<String>,
        owner_run_id: Option<u64>,
        owner_scope: String,
    ) {
        let cwd = match self
            .resolve_workspace_directory_for_run(owner_run_id, relative_path.as_deref())
        {
            Ok(cwd) => cwd,
            Err(message) => {
                self.finish_command(request_id, action, false, message, Value::Null);
                return;
            }
        };
        let callback = self.managed_process_callback();
        match self
            .processes
            .start(command, cwd, owner_run_id, Some(owner_scope), callback)
        {
            Ok(result) => self.finish_command(
                request_id,
                action,
                true,
                "Managed process started".to_owned(),
                result,
            ),
            Err(message) => self.finish_command(request_id, action, false, message, Value::Null),
        }
        self.render_agent_panel();
    }

    fn handle_managed_process_event(&mut self, event: ManagedProcessEvent) {
        let exited_process_id = match &event {
            ManagedProcessEvent::Exited { process_id, .. } => Some(*process_id),
            ManagedProcessEvent::Output { .. } => None,
        };
        let exited_owner_run_id =
            exited_process_id.and_then(|process_id| self.processes.owner_run_id(process_id));
        if self.processes.handle_event(event).is_some() {
            if let Some(process_id) = exited_process_id {
                for tab in &mut self.tabs {
                    if let Some(preview) = tab
                        .web_preview
                        .as_mut()
                        .filter(|preview| preview.owner_process_id == process_id)
                    {
                        preview.process_running = false;
                    }
                }
            }
            if exited_process_id
                .is_some_and(|process_id| self.preview.owner_process_id == Some(process_id))
                && (self.preview.live || self.preview.capturing)
            {
                self.preview
                    .stop_live("Live preview stopped because its managed process exited.");
            }
            // Process output is consumed by explicit process-status tool calls. It is intentionally
            // not part of the global panel state: reserializing the growing output buffer for every
            // chunk made long-running commands progressively slower even after the Managed Tasks UI
            // was removed. Only lifecycle changes need a full UI refresh.
            let finalized_run = exited_owner_run_id
                .is_some_and(|run_id| self.try_finish_run_when_quiescent(run_id));
            if finalized_run {
                self.save_session();
            }
            if exited_process_id.is_some() {
                self.render_agent_panel();
            }
        }
    }

    fn handle_terminal_event(&mut self, event: TerminalEvent) {
        match event {
            TerminalEvent::Output { session_id, chunk } => {
                if let Some(revision) = self.terminal.handle_output(session_id, &chunk) {
                    self.stream_terminal_output(session_id, revision, &chunk);
                    self.refresh_following_terminal_context(session_id, false);
                }
            }
            TerminalEvent::CommandFinished {
                session_id,
                request_id,
                action,
                exit_code,
                output,
                cwd,
            } => {
                let ok = exit_code == 0;
                let remote_target = self.terminal.remote_target(session_id).map(str::to_owned);
                let context_was_suppressed = self.terminal.context_output_is_suppressed(session_id);
                if !self.terminal.handle_command_finished(
                    session_id,
                    cwd.as_deref(),
                    ok,
                    Some(exit_code),
                ) {
                    return;
                }
                if action == SSH_RUNTIME_DISCOVERY_ACTION {
                    let discovery = if ok {
                        parse_ssh_capability_probe(&output).and_then(|capabilities| {
                            self.terminal.set_ssh_capabilities(session_id, capabilities)
                        })
                    } else {
                        Err(format!(
                            "Remote capability discovery exited with code {exit_code}"
                        ))
                    };
                    match discovery {
                        Ok(()) => {
                            self.audit.record(
                                "ssh_runtime_discovery_completed",
                                &request_id,
                                "terminal_session",
                                &session_id.to_string(),
                                "ssh",
                                "completed",
                                "The fixed capability bitset was parsed and cached locally",
                            );
                            self.ssh.set_message(
                                "SSH runtime prepared locally; no capability inventory was sent to Agent.",
                                false,
                            );
                        }
                        Err(message) => {
                            warn!(%message, session_id, "local SSH capability discovery failed");
                            self.audit.record(
                                "ssh_runtime_discovery_failed",
                                &request_id,
                                "terminal_session",
                                &session_id.to_string(),
                                "ssh",
                                "failed",
                                "The runtime kept portable POSIX fallback behavior enabled",
                            );
                            self.ssh.set_message(
                                "SSH preparation was incomplete; portable POSIX fallback remains available.",
                                false,
                            );
                        }
                    }
                    self.refresh_following_terminal_context(session_id, true);
                    self.resume_pending_ssh_runtime_command(session_id);
                    self.render_agent_panel();
                    return;
                }
                let mut provider_output = output;
                if action == "ssh_run" {
                    match extract_ssh_capability_probe(&provider_output) {
                        Ok(Some((capabilities, cleaned_output))) => {
                            if let Err(message) =
                                self.terminal.set_ssh_capabilities(session_id, capabilities)
                            {
                                warn!(%message, session_id, "local SSH capability cache was rejected");
                            } else {
                                self.audit.record(
                                    "ssh_runtime_discovery_completed",
                                    &request_id,
                                    "terminal_session",
                                    &session_id.to_string(),
                                    "ssh",
                                    "completed",
                                    "The fallback capability bitset was parsed locally during the first remote command",
                                );
                            }
                            provider_output = if cleaned_output.is_empty() {
                                "Command completed without output".to_owned()
                            } else {
                                cleaned_output
                            };
                        }
                        Ok(None) => {}
                        Err(message) => {
                            warn!(%message, session_id, "fallback SSH capability response was incomplete");
                            provider_output = strip_ssh_capability_probe(&provider_output);
                            if provider_output.is_empty() {
                                provider_output = "Command completed without output".to_owned();
                            }
                        }
                    }
                }
                if context_was_suppressed
                    && let Err(message) = self
                        .terminal
                        .append_sanitized_context_output(session_id, &provider_output)
                {
                    warn!(%message, session_id, "sanitized SSH command output was not attached to terminal context");
                }
                self.refresh_following_terminal_context(session_id, true);
                let message = if ok && remote_target.is_some() {
                    "Remote SSH command completed".to_owned()
                } else if ok {
                    "Terminal command completed".to_owned()
                } else if remote_target.is_some() {
                    format!("Remote SSH command exited with code {exit_code}")
                } else {
                    format!("Terminal command exited with code {exit_code}")
                };
                let resume_ssh_queue = action == "ssh_run";
                self.finish_command(
                    request_id,
                    action,
                    ok,
                    message,
                    json!({
                        "exitCode": exit_code,
                        "output": provider_output,
                        "cwd": cwd,
                        "remote": remote_target.is_some(),
                    }),
                );
                if resume_ssh_queue {
                    self.resume_pending_ssh_runtime_command(session_id);
                }
            }
            TerminalEvent::CommandFailed {
                session_id,
                request_id,
                action,
                message,
            } => {
                if self
                    .terminal
                    .handle_command_finished(session_id, None, false, None)
                {
                    self.refresh_following_terminal_context(session_id, true);
                    if action == SSH_RUNTIME_DISCOVERY_ACTION {
                        warn!(%message, session_id, "local SSH capability discovery failed");
                        self.audit.record(
                            "ssh_runtime_discovery_failed",
                            &request_id,
                            "terminal_session",
                            &session_id.to_string(),
                            "ssh",
                            "failed",
                            "The runtime kept portable POSIX fallback behavior enabled",
                        );
                        self.resume_pending_ssh_runtime_command(session_id);
                        self.render_agent_panel();
                        return;
                    }
                    let resume_ssh_queue = action == "ssh_run";
                    self.finish_command(request_id, action, false, message, Value::Null);
                    if resume_ssh_queue {
                        self.resume_pending_ssh_runtime_command(session_id);
                    }
                }
            }
            TerminalEvent::Exited {
                session_id,
                message,
            } => {
                self.freeze_terminal_context(session_id);
                if self.terminal.handle_exit(session_id, message) {
                    self.emit_graph_contexts();
                    self.fail_pending_ssh_runtime_command(
                        session_id,
                        "The SSH connection closed while the local runtime was preparing it",
                    );
                    if !self.terminal.has_agent_ready_ssh_session() {
                        self.permission_policy.revoke_scope(CapabilityScope::Ssh);
                    }
                    if !self.terminal.is_running() {
                        self.terminal_panel_visible = false;
                        self.layout_webviews();
                        self.render_toolbar();
                    }
                    self.render_agent_panel();
                }
            }
        }
    }

    fn execute_immediate_command(
        &mut self,
        run_id: Option<u64>,
        command: BrowserCommand,
    ) -> Result<Value, String> {
        match command {
            BrowserCommand::GetState => {
                let mut state = self.browser_state_value();
                state["activeTabId"] = json!(self.browser_target_for_run(run_id).ok());
                Ok(state)
            }
            BrowserCommand::Navigate { value } => {
                let url = normalize_address_input(&value, search_engine(&self.search_engine))
                    .ok_or_else(|| "Invalid address".to_owned())?;
                let active_id = self.browser_target_for_run(run_id)?;
                let tab = self
                    .tabs
                    .iter_mut()
                    .find(|tab| tab.id == active_id)
                    .ok_or_else(|| "No active tab".to_owned())?;
                tab.webview
                    .load_url(&url)
                    .map_err(|error| format!("Navigation failed: {error}"))?;
                tab.is_start_page = false;
                self.layout_agent_graph_surface();
                Ok(json!({ "url": url }))
            }
            BrowserCommand::Back => {
                let tab_id = self.browser_target_for_run(run_id)?;
                let tab = self
                    .tabs
                    .iter()
                    .find(|tab| tab.id == tab_id)
                    .ok_or_else(|| "No active tab".to_owned())?;
                tab.webview
                    .go_back()
                    .map_err(|error| format!("Back navigation failed: {error}"))?;
                Ok(Value::Null)
            }
            BrowserCommand::Forward => {
                let tab_id = self.browser_target_for_run(run_id)?;
                let tab = self
                    .tabs
                    .iter()
                    .find(|tab| tab.id == tab_id)
                    .ok_or_else(|| "No active tab".to_owned())?;
                tab.webview
                    .go_forward()
                    .map_err(|error| format!("Forward navigation failed: {error}"))?;
                Ok(Value::Null)
            }
            BrowserCommand::Reload => {
                let tab_id = self.browser_target_for_run(run_id)?;
                let tab = self
                    .tabs
                    .iter()
                    .find(|tab| tab.id == tab_id)
                    .ok_or_else(|| "No active tab".to_owned())?;
                tab.webview
                    .reload()
                    .map_err(|error| format!("Reload failed: {error}"))?;
                Ok(Value::Null)
            }
            BrowserCommand::Stop => {
                let tab_id = self.browser_target_for_run(run_id)?;
                let tab = self
                    .tabs
                    .iter()
                    .find(|tab| tab.id == tab_id)
                    .ok_or_else(|| "No active tab".to_owned())?;
                tab.webview
                    .evaluate_script("window.stop();")
                    .map_err(|error| format!("Stop failed: {error}"))?;
                Ok(Value::Null)
            }
            BrowserCommand::Home => {
                let html = self.start_page_html();
                let active_id = self.browser_target_for_run(run_id)?;
                let tab = self
                    .tabs
                    .iter_mut()
                    .find(|tab| tab.id == active_id)
                    .ok_or_else(|| "No active tab".to_owned())?;
                tab.webview
                    .load_html(&html)
                    .map_err(|error| format!("Could not open the start page: {error}"))?;
                tab.is_start_page = true;
                tab.loading = true;
                self.layout_agent_graph_surface();
                Ok(Value::Null)
            }
            BrowserCommand::NewTab { url } => {
                let url = url
                    .map(|value| {
                        normalize_address_input(&value, search_engine(&self.search_engine))
                            .ok_or_else(|| "Invalid new tab URL".to_owned())
                    })
                    .transpose()?;
                let tab_id = self
                    .create_tab(url, run_id.is_none())
                    .map_err(|error| format!("Could not create the new tab: {error}"))?;
                self.set_run_browser_target(run_id, tab_id);
                Ok(json!({ "tabId": tab_id }))
            }
            BrowserCommand::CloseTab { tab_id } => {
                if !self.tabs.iter().any(|tab| tab.id == tab_id) {
                    return Err("Tab not found".to_owned());
                }
                self.close_tab(tab_id);
                Ok(json!({ "tabId": tab_id }))
            }
            BrowserCommand::ActivateTab { tab_id } => {
                if !self.tabs.iter().any(|tab| tab.id == tab_id) {
                    return Err("Tab not found".to_owned());
                }
                self.set_run_browser_target(run_id, tab_id);
                if run_id.is_none() {
                    self.activate_tab(tab_id);
                }
                Ok(json!({ "tabId": tab_id }))
            }
            BrowserCommand::InspectPage
            | BrowserCommand::Click { .. }
            | BrowserCommand::SetText { .. }
            | BrowserCommand::Scroll { .. } => {
                Err("Asynchronous command was routed incorrectly".to_owned())
            }
        }
    }

    fn run_script_command(&mut self, request_id: String, action: String, script: String) {
        let proxy = self.proxy.clone();
        let callback_request_id = request_id.clone();
        let callback_action = action.clone();
        let target = self.browser_target_for_run(self.run_id_for_request(&request_id));
        let tab_id = match target {
            Ok(id) => id,
            Err(message) => {
                self.finish_command(request_id, action, false, message, Value::Null);
                return;
            }
        };
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == tab_id) else {
            self.finish_command(
                request_id,
                action,
                false,
                "No active tab".to_owned(),
                Value::Null,
            );
            return;
        };

        if let Err(error) = tab
            .webview
            .evaluate_script_with_callback(&script, move |raw_result| {
                let _ = proxy.send_event(BrowserEvent::ScriptResult {
                    request_id: callback_request_id.clone(),
                    action: callback_action.clone(),
                    raw_result,
                });
            })
        {
            self.finish_command(
                request_id,
                action,
                false,
                format!("Page execution failed: {error}"),
                Value::Null,
            );
        }
    }

    fn handle_script_result(&mut self, request_id: String, action: String, raw_result: String) {
        match parse_script_bridge_result(&raw_result) {
            Ok(result) if result.ok => self.finish_command(
                request_id,
                action,
                true,
                "Command completed".to_owned(),
                result.data,
            ),
            Ok(result) => self.finish_command(
                request_id,
                action,
                false,
                result
                    .error
                    .unwrap_or_else(|| "Command rejected by the page".to_owned()),
                result.data,
            ),
            Err(error) => self.finish_command(
                request_id,
                action,
                false,
                format!("Invalid page response: {error}"),
                Value::Null,
            ),
        }
    }

    fn begin_command(
        &mut self,
        request_id: &str,
        action: &str,
        summary: String,
        authorization: crate::permissions::ActionAuthorization,
    ) {
        let audit_summary = summary.clone();
        if let Some(entry) = self
            .action_log
            .iter_mut()
            .find(|entry| entry.request_id == request_id)
        {
            entry.status = CommandStatus::Running;
        } else {
            self.action_log.push_front(ActionLogEntry {
                request_id: request_id.to_owned(),
                action: action.to_owned(),
                summary,
                scope: authorization.scope,
                effect: authorization.effect,
                status: CommandStatus::Running,
                timestamp_ms: unix_time_ms(),
            });
        }
        self.audit_command_event(
            "command_started",
            request_id,
            action,
            authorization,
            CommandStatus::Running,
            &audit_summary,
        );
        self.render_agent_panel();
    }

    fn finish_command(
        &mut self,
        request_id: String,
        action: String,
        ok: bool,
        message: String,
        data: Value,
    ) {
        let status = if ok {
            CommandStatus::Success
        } else {
            CommandStatus::Error
        };
        self.finish_command_with_status(request_id, action, ok, message, data, status);
    }

    fn finish_command_with_status(
        &mut self,
        request_id: String,
        action: String,
        ok: bool,
        message: String,
        data: Value,
        status: CommandStatus,
    ) {
        let audit_metadata = if let Some(entry) = self
            .action_log
            .iter_mut()
            .find(|entry| entry.request_id == request_id)
        {
            entry.status = status;
            Some((entry.summary.clone(), entry.scope, entry.effect))
        } else {
            None
        };
        if let Some((summary, scope, effect)) = audit_metadata {
            self.audit_command_event(
                "command_finished",
                &request_id,
                &action,
                ActionAuthorization::new(scope, effect),
                status,
                &summary,
            );
        }
        self.last_result = Some(CommandResultView {
            request_id,
            action,
            ok,
            message,
            data,
        });

        self.render_agent_panel();
    }

    fn audit_command_event(
        &mut self,
        event_type: &str,
        request_id: &str,
        action: &str,
        authorization: ActionAuthorization,
        status: CommandStatus,
        summary: &str,
    ) {
        self.audit.record(
            event_type,
            request_id,
            action,
            &serialized_label(&authorization.scope),
            &serialized_label(&authorization.effect),
            &serialized_label(&status),
            summary,
        );
    }

    fn browser_state_value(&self) -> Value {
        json!({
            "activeTabId": self.active_tab_id,
            "tabs": self.tabs.iter().map(|tab| json!({
                "id": tab.id,
                "title": tab.title,
                "url": tab.url,
                "loading": tab.loading,
                "previewSourceType": tab.web_preview.as_ref().map(|_| "web"),
                "previewOwnerProcessId": tab.web_preview.as_ref().map(|preview| preview.owner_process_id),
                "previewProcessRunning": tab.web_preview.as_ref().map(|preview| preview.process_running),
            })).collect::<Vec<_>>(),
        })
    }

    fn handle_page_load(&mut self, tab_id: u64, loading: bool, url: String) {
        if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) {
            if tab
                .web_preview
                .as_ref()
                .is_some_and(|preview| url_origin(&url).as_deref() != Some(preview.origin.as_str()))
            {
                tab.web_preview = None;
            }
            tab.content_revision = tab.content_revision.saturating_add(1);
            tab.loading = loading;
            if !loading {
                tab.ready_to_show = true;
            }
            if is_supported_url(&url) {
                tab.is_start_page = false;
            }
            tab.url = address_for_display(&url);
            if !loading && tab.title.trim().is_empty() {
                tab.title = DEFAULT_TITLE.to_owned();
            }
        }

        if !loading {
            self.save_session();
        }
        if !loading && self.pending_tab_activation == Some(tab_id) {
            self.pending_tab_activation = None;
            self.activate_tab(tab_id);
            return;
        }
        // Search forms, links, redirects and browser history bypass the toolbar.
        // Update the separate native launcher on both navigation boundaries.
        self.layout_agent_graph_surface();
        self.update_window_title();
        self.render_toolbar();
        self.render_agent_panel();
    }

    fn handle_title_changed(&mut self, tab_id: u64, title: String) {
        if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == tab_id) {
            let title = title.trim();
            let next_title = if title.is_empty() {
                DEFAULT_TITLE.to_owned()
            } else {
                title.to_owned()
            };
            if tab.title != next_title {
                tab.content_revision = tab.content_revision.saturating_add(1);
                tab.title = next_title;
            }
        }
        self.update_window_title();
        self.render_toolbar();
        self.render_agent_panel();
    }

    fn active_tab(&self) -> Option<&BrowserTab> {
        let active_id = self.active_tab_id?;
        self.tabs.iter().find(|tab| tab.id == active_id)
    }

    fn layout_webviews(&self) {
        let Some(window) = &self.window else {
            return;
        };
        let background = theme_surface_color(&self.theme);

        if let Some(backdrop) = &self.backdrop
            && let Err(error) = backdrop.set_bounds(full_window_bounds(window))
        {
            warn!(%error, "failed to resize the window backdrop");
        }

        if let Some(toolbar) = &self.toolbar
            && let Err(error) = toolbar.set_bounds(self.active_toolbar_bounds(window))
        {
            warn!(%error, "failed to resize the toolbar");
        }
        if let Some(toolbar) = &self.toolbar {
            let visible = !self.settings_covering_main();
            if visible && let Err(error) = toolbar.set_background_color(background) {
                warn!(%error, "toolbar surface was not prepared before reveal");
            }
            if let Err(error) = toolbar.set_visible(visible) {
                warn!(%error, "toolbar visibility was not updated");
            }
        }

        if let Some(panel) = &self.agent_panel {
            let bounds = if let Some(detached_window) = self.agent_panel_window.as_ref() {
                full_window_bounds(detached_window)
            } else if self.settings_open {
                settings_panel_bounds(window)
            } else if self.browser_panel_minimized {
                browser_content_bounds_with_panel_width(
                    window,
                    self.active_toolbar_height(window),
                    None,
                    self.terminal_panel_visible && self.terminal_window.is_none(),
                    self.terminal_dock,
                )
            } else {
                agent_panel_bounds_with_width(
                    window,
                    self.effective_agent_panel_width(window),
                    if self.browser_tabs_visible() {
                        0
                    } else {
                        self.active_toolbar_height(window)
                    },
                )
            };
            if let Err(error) = panel.set_bounds(bounds) {
                warn!(%error, "failed to resize the agent panel");
            }
            let visible = self.agent_panel_window.is_some()
                || (!self.agent_graph_open && (self.agent_panel_visible || self.settings_open));
            if visible && let Err(error) = panel.set_background_color(background) {
                warn!(%error, "agent panel surface was not prepared before reveal");
            }
            if let Err(error) = panel.set_visible(visible) {
                warn!(%error, "agent panel visibility was not updated");
            }
        }

        self.layout_terminal_webview();
        self.layout_tab_webviews();
        self.layout_preview_webview();
        self.layout_agent_graph_surface();
    }

    fn layout_preview_webview(&self) {
        if let (Some(window), Some(panel)) =
            (self.preview_window.as_ref(), self.preview_panel.as_ref())
        {
            if let Err(error) = panel.set_bounds(full_window_bounds(window)) {
                warn!(%error, "failed to resize the detached preview");
            }
            if let Err(error) = panel.set_background_color(theme_surface_color(&self.theme)) {
                warn!(%error, "detached preview background was not updated");
            }
        }
    }

    fn layout_terminal_webview(&self) {
        let Some(window) = &self.window else {
            return;
        };
        let background = theme_surface_color(&self.theme);
        if let Some(panel) = &self.terminal_panel {
            let bounds = self
                .terminal_window
                .as_ref()
                .map(full_window_bounds)
                .unwrap_or_else(|| {
                    terminal_panel_bounds_with_panel_width(
                        window,
                        self.active_toolbar_height(window),
                        if self.browser_panel_minimized {
                            None
                        } else {
                            self.docked_agent_panel_width(window)
                        },
                        self.terminal_dock,
                    )
                });
            if let Err(error) = panel.set_bounds(bounds) {
                warn!(%error, "failed to resize the terminal panel");
            }
            let visible = if self.terminal_window.is_some() {
                true
            } else {
                self.terminal_panel_visible
                    && !self.settings_covering_main()
                    && !self.agent_graph_open
            };
            if visible && let Err(error) = panel.set_background_color(background) {
                warn!(%error, "terminal panel surface was not prepared before reveal");
            }
            if let Err(error) = panel.set_visible(visible) {
                warn!(%error, "terminal panel visibility was not updated");
            }
        }
    }

    fn layout_tab_webviews(&self) {
        let Some(window) = &self.window else {
            return;
        };
        let agent_panel_width = self.docked_agent_panel_width(window);
        let terminal_docked_in_main = self.terminal_panel_visible
            && self.terminal_window.is_none()
            && !self.settings_covering_main();
        let content = browser_content_area_with_panel_width(
            window,
            self.active_toolbar_height(window),
            agent_panel_width,
            terminal_docked_in_main,
            self.terminal_dock,
        );
        let (primary_area, docked_area) =
            split_tab_areas(content, self.docked_tab_id.is_some(), self.tab_dock_side);
        for tab in &self.tabs {
            if let Some(detached_window) = tab.detached_window.as_ref() {
                if let Err(error) = tab.webview.set_bounds(full_window_bounds(detached_window)) {
                    warn!(%error, tab_id = tab.id, "failed to resize the detached tab");
                }
                continue;
            }
            if self.browser_panel_minimized {
                // Retain a useful viewport for hidden pages and browser tools.
                continue;
            }
            let area = if self.docked_tab_id == Some(tab.id) {
                docked_area.unwrap_or(content)
            } else if self.primary_tab_id == Some(tab.id) {
                primary_area
            } else {
                content
            };
            if let Err(error) = tab.webview.set_bounds(area.into_rect()) {
                warn!(%error, tab_id = tab.id, "failed to resize the tab");
            }
        }
    }

    fn render_toolbar(&self) {
        if !self.toolbar_ready {
            return;
        }
        let Some(toolbar) = &self.toolbar else {
            return;
        };

        let active = self.active_tab();
        let state = ToolbarState {
            browser_panel_minimized: self.browser_panel_minimized,
            browser_panel_toggle_enabled: !self.settings_open
                && !self.agent_graph_open
                && self.agent_panel_window.is_none(),
            browser_tabs_visible: self.browser_tabs_visible(),
            project_board_open: self.agent_graph_open,
            tabs: self
                .tabs
                .iter()
                .map(|tab| ToolbarTab {
                    id: tab.id,
                    title: tab.title.clone(),
                    loading: tab.loading,
                    primary: self.primary_tab_id == Some(tab.id),
                    docked: self.docked_tab_id == Some(tab.id),
                    dock_side: (self.docked_tab_id == Some(tab.id)).then_some(self.tab_dock_side),
                    detached: tab.detached_window.is_some(),
                })
                .collect(),
            active_tab_id: self.active_tab_id,
            primary_tab_id: self.primary_tab_id,
            address: active.map(|tab| tab.url.clone()).unwrap_or_default(),
            can_go_back: active
                .and_then(|tab| tab.webview.can_go_back().ok())
                .unwrap_or(false),
            can_go_forward: active
                .and_then(|tab| tab.webview.can_go_forward().ok())
                .unwrap_or(false),
            loading: active.is_some_and(|tab| tab.loading),
            settings_open: self.settings_open,
            agent_panel_visible: self.agent_panel_visible,
            terminal_panel_visible: self.terminal_panel_visible,
            terminal_detached: self.terminal_window.is_some(),
            terminal_session_count: self.terminal.view().sessions.len(),
            terminal_dock: self.terminal_dock,
            theme: self.theme.clone(),
        };

        match serde_json::to_string(&state) {
            Ok(json) => {
                let script =
                    format!("window.renderBrowserState && window.renderBrowserState({json});");
                if let Err(error) = toolbar.evaluate_script(&script) {
                    warn!(%error, "toolbar update failed");
                }
            }
            Err(error) => warn!(%error, "failed to serialize toolbar state"),
        }
    }

    fn apply_chat_zoom(&self, action: ChatZoomAction) {
        let Some(panel) = self.agent_panel.as_ref() else {
            return;
        };
        let script = format!(
            "(() => {{
                const action = {};
                const root = document.documentElement;
                const current = Number(root.dataset.chatZoom || 0);
                const next = action === 'reset'
                    ? 0
                    : Math.max(-2, Math.min(8, current + (action === 'increase' ? 1 : -1)));
                if (typeof window.centralAgentSetChatZoom === 'function') {{
                    window.centralAgentSetChatZoom(next);
                }} else {{
                    root.style.setProperty('--ca-chat-font-offset', `${{next}}px`);
                    root.dataset.chatZoom = String(next);
                }}
            }})();",
            action.javascript_argument()
        );
        if let Err(error) = panel.evaluate_script(&script) {
            warn!(%error, ?action, "chat zoom shortcut could not be routed to the agent panel");
        }
    }

    fn time_machine_view(&self) -> TimeMachineView {
        self.time_machine
            .try_lock()
            .map(|time_machine| time_machine.view())
            .unwrap_or_else(|_| TimeMachineView {
                available: true,
                selected: None,
                message: Some("Preparing a local checkpoint…".to_owned()),
                message_is_error: false,
            })
    }

    fn render_agent_panel(&self) {
        if self.agent_graph_open {
            self.render_agent_graph_surface();
        }
        if !self.agent_panel_ready && !self.terminal_panel_ready {
            return;
        }

        let now_ms = unix_time_ms();
        let mut capabilities = vec![
            "tabs",
            "navigation",
            "page_inspection",
            "element_click",
            "text_input",
            "page_scroll",
            "interactive_terminal",
            "terminal_command",
            "ssh_terminal",
        ];
        if self.terminal.has_agent_ready_ssh_session() {
            capabilities.push("ssh_agent_command");
        }
        if self.remote_desktop.configured_profile_id().is_some() {
            capabilities.extend(["remote_linux_desktop", "remote_desktop_file_upload"]);
        }
        if self.remote_desktop.agent_control_enabled() && self.remote_desktop.is_connected() {
            capabilities.extend([
                "remote_desktop_agent_observation",
                "remote_desktop_agent_pointer",
                "remote_desktop_agent_keyboard",
            ]);
        }
        if self.workspace.root().is_some() {
            capabilities.extend([
                "workspace_list",
                "workspace_read",
                "workspace_search",
                "workspace_apply_patch",
                "workspace_run_command",
                "managed_processes",
                "process_status",
                "process_stdin",
                "process_cancellation",
                "local_web_previews",
            ]);
        }
        if self.window_access_enabled {
            capabilities.extend([
                "window_inventory",
                "window_capture",
                "semantic_ui_inspection",
                "semantic_ui_control",
                "window_focus",
                "window_move_resize",
                "window_minimize_restore",
                "window_close",
                "native_live_previews",
                "game_window_previews",
            ]);
        }
        let mut project_chats: Vec<_> = self
            .project_chats
            .iter()
            .map(|chat| self.chat_summary(chat))
            .collect();
        chat_ownership::project_chat_lineage(
            &mut project_chats,
            self.app_server.conversations.saved(),
        );
        let mut workspace = self.workspace.view();
        for project in &mut workspace.projects {
            if let Some(label) = self.workspace_project_labels.get(&project.root) {
                project.name = label.clone();
            }
            project.pinned = self.pinned_workspace_roots.contains(&project.root);
            project.agent_active = self.workspace_has_active_agent(&project.root);
            project.checkpoint_pending =
                !self.workspace_checkpoint_run_ids(&project.root).is_empty();
            project.removal_pending = self.pending_workspace_ejections.contains(&project.root);
        }
        if let Some(active) = workspace.projects.iter().find(|project| project.active) {
            workspace.name = Some(active.name.clone());
        }
        let mut main_agent = self
            .active_main_run()
            .map(AgentRun::view)
            .unwrap_or_else(AgentRuntimeView::idle);
        if self.agent_provider == AgentProviderKind::CodexAppServer {
            main_agent = self
                .app_server_run_view(&self.conversation_key(None))
                .unwrap_or_else(AgentRuntimeView::idle);
        }
        main_agent.active = self.active_main_busy();
        let agent_can_resume = self.main_agent_can_resume();
        let state = AgentPanelState {
            app_server: &self.app_server.view,
            conversation_state: self.conversation_state(&self.conversation_key(None)),
            runtime_busy: self.any_agent_active()
                || self.has_claude_jobs()
                || !self.subscription_jobs.is_empty()
                || !self.pending_commands.is_empty(),

            browser_panel_minimized: self.browser_panel_minimized,
            settings_open: self.settings_open,
            agent_panel_detached: self.agent_panel_window.is_some(),
            active_page: self.active_tab().map(|tab| AgentPanelPage {
                tab_id: tab.id,
                title: tab.title.clone(),
                url: tab.url.clone(),
                loading: tab.loading,
                preview_source_type: tab.web_preview.as_ref().map(|_| "web"),
                preview_owner_process_id: tab
                    .web_preview
                    .as_ref()
                    .map(|preview| preview.owner_process_id),
                preview_process_running: tab
                    .web_preview
                    .as_ref()
                    .map(|preview| preview.process_running),
            }),
            tabs: self
                .tabs
                .iter()
                .map(|tab| AgentPanelTab {
                    id: tab.id,
                    title: tab.title.clone(),
                    url: tab.url.clone(),
                    active: self.active_tab_id == Some(tab.id),
                    preview_source_type: tab.web_preview.as_ref().map(|_| "web"),
                    preview_owner_process_id: tab
                        .web_preview
                        .as_ref()
                        .map(|preview| preview.owner_process_id),
                    preview_process_running: tab
                        .web_preview
                        .as_ref()
                        .map(|preview| preview.process_running),
                })
                .collect(),
            action_log: self.action_log.iter().cloned().collect(),
            last_result: self.last_result.clone(),
            capabilities,
            permission: PermissionPanelState {
                mode: self.permission_policy.mode(),
                session_authorized: self.permission_policy.session_authorized(),
                authorized_scopes: self.permission_policy.authorized_scopes(),
                pending: self.pending_approval_view(),
                scope: "connected_capabilities",
                resets_on_restart: true,
            },
            agent: main_agent,
            agent_can_resume,
            queued_agent_request: self.active_pending_submission().is_some(),
            agent_submission_queue_count: self.active_submission_queue_count(),

            discardable_checkpoint_run_id: self.discardable_queued_checkpoint_run_id(),
            chat_messages: self
                .main_chat_messages()
                .map(|message| RenderedChatMessage::new(message, &self.artifact_store))
                .collect(),
            tab_contexts: self
                .draft_contexts
                .iter()
                .map(|context| {
                    let revision = self
                        .tabs
                        .iter()
                        .find(|tab| tab.id == context.tab_id)
                        .map(|tab| tab.content_revision);
                    context.view(revision, now_ms)
                })
                .collect(),
            terminal_contexts: self
                .draft_terminal_contexts
                .iter()
                .map(|context| context.view(self.terminal.has_session(context.snapshot.session_id)))
                .collect(),
            file_attachments: self
                .draft_file_attachments
                .iter()
                .map(PendingFileAttachment::view)
                .collect(),
            provider: self.active_provider_view(),
            providers: AgentProvidersView {
                selected: self.agent_provider,
                codex_app_server: self.app_server.configuration().0,

                claude_code: &self.claude_provider,
                cursor: &self.cursor_provider,
                github_copilot: &self.github_copilot_provider,
                google_antigravity: &self.google_antigravity_provider,
                opencode_go: &self.opencode_go_provider,
                claude_session_id: self.claude_session_id.as_deref(),
            },
            agent_configuration: AgentConfigurationView {
                models: self.active_agent_models(),
                selection: self.active_agent_selection(),
            },

            terminal: self.terminal.view(),
            ssh: self.ssh.view(),
            remote_desktop: self.remote_desktop.view(),
            terminal_panel_visible: self.terminal_panel_visible,
            terminal_detached: self.terminal_window.is_some(),
            terminal_dock: self.terminal_dock,
            system: SystemPanelState {
                window_access_enabled: self.window_access_enabled,
                windows: self.windows.view(),
                ui_automation: self.ui_automation.view(),
                safety: self.safety.view(),
            },
            hardening: HardeningPanelState {
                audit: self.audit.view(),
                isolation_backend: "capability_policy_and_job_objects",
                process_sandboxed: false,
                release_ready: false,
                status: "Commands run without elevation as the current Windows user. Managed commands have a 30-minute runtime limit, a 64 MiB combined output cap, and an 8 MiB stdin cap. Job Objects provide process ownership and cleanup, not filesystem/network isolation. Public autonomous computer-use release remains gated on code signing and an explicit isolation backend.",
            },
            preview: self.preview.view(self.preview_window.is_some()),
            appearance: AppearancePanelState {
                theme: self.theme.clone(),
                search_engine: self.search_engine.clone(),
                search_engines: search_engine_options(),
            },
            workspace,
            project_chats,
            active_project_chat_id: self.active_project_chat_id.as_deref(),
            agent_graph: AgentGraphAgentsView {
                bindings: &self.agent_graph_bindings,
                active_node_keys: self.active_agent_graph_node_keys(),
                links: &self.agent_graph_links,
            },
            time_machine: self.time_machine_view(),
        };

        match serde_json::to_string(&state) {
            Ok(json) => {
                let script = format!(
                    "window.renderAgentPanelState && window.renderAgentPanelState({json});"
                );
                if self.agent_panel_ready
                    && let Some(panel) = &self.agent_panel
                    && let Err(error) = panel.evaluate_script(&script)
                {
                    warn!(%error, "agent panel update failed");
                }
                if self.terminal_panel_ready
                    && let Some(panel) = &self.terminal_panel
                    && let Err(error) = panel.evaluate_script(&script)
                {
                    warn!(%error, "terminal panel update failed");
                }
            }
            Err(error) => warn!(%error, "failed to serialize the agent panel"),
        }
        self.render_detached_preview();
    }

    fn render_detached_preview(&self) {
        if !self.preview_panel_ready {
            return;
        }
        let Some(panel) = self.preview_panel.as_ref() else {
            return;
        };
        let Ok(preview) = serde_json::to_string(&self.preview.view(true)) else {
            return;
        };
        let Ok(theme) = serde_json::to_string(&self.theme) else {
            return;
        };
        let script =
            format!("window.renderPreviewState && window.renderPreviewState({preview}, {theme});");
        if let Err(error) = panel.evaluate_script(&script) {
            warn!(%error, "detached preview update failed");
        }
    }

    fn stream_agent_message(&self, message_id: u64) {
        if !self.agent_panel_ready && !self.agent_graph_surface_ready {
            return;
        }
        let Some(message) = self
            .chat_messages
            .iter()
            .find(|message| message.id == message_id)
        else {
            return;
        };
        let target_agent_graph_node_key = self
            .agent_graph_runs
            .values()
            .find(|run| message.run_id == Some(run.run_id))
            .map(|run| run.node_key.as_str());
        let target_project_chat_id = self.project_chats.iter().find_map(|chat| {
            let visible =
                self.project_board_open_chat_ids
                    .iter()
                    .any(|chat_id| chat_id == &chat.id)
                    || self.agent_graph_bindings.iter().any(|binding| {
                        binding.project_chat_id.as_deref() == Some(chat.id.as_str())
                    });
            (visible
                && !chat.archived
                && self.chat_ownership.belongs_to(
                    message,
                    Some(chat.id.as_str()),
                    &self.run_execution_contexts,
                ))
            .then_some(&chat.id)
        });
        let Ok(update) = serde_json::to_string(&json!({
            "id": message.id,
            "runId": message.run_id,
            "nodeKey": target_agent_graph_node_key,
            "chatId": target_project_chat_id,
            "text": message.text,
            "renderedHtml": rendered_message_html(message.role, message.kind, &message.text),
            "streaming": message.streaming,
            "kind": message.kind,
            "provider": message.provider.unwrap_or(self.agent_provider),
            "activityStatus": message.activity_status,
            "activityCategory": message.activity_category,
            "activityDetail": message.activity_detail,
            "activityContext": message.activity_context,
        })) else {
            return;
        };
        let belongs_to_any_agent_graph_run = target_agent_graph_node_key.is_some();
        if !belongs_to_any_agent_graph_run
            && self.message_in_active_chat(message)
            && self.agent_panel_ready
            && let Some(panel) = self.agent_panel.as_ref()
            && let Err(error) = panel.evaluate_script(&format!(
                "window.updateAgentMessage && window.updateAgentMessage({update});"
            ))
        {
            warn!(%error, "agent message stream update failed");
        }
        if belongs_to_any_agent_graph_run
            && self.agent_graph_surface_ready
            && let Some(surface) = self.agent_graph_surface.as_ref()
            && let Err(error) = surface.evaluate_script(&format!(
                "window.updateAgentGraphMessage && window.updateAgentGraphMessage({update});"
            ))
        {
            warn!(%error, "agent graph message stream update failed");
        }
        if target_project_chat_id.is_some()
            && self.agent_graph_surface_ready
            && let Some(surface) = self.agent_graph_surface.as_ref()
            && let Err(error) = surface.evaluate_script(&format!(
                "window.updateProjectChatMessage && window.updateProjectChatMessage({update});"
            ))
        {
            warn!(%error, "project chat card message stream update failed");
        }
    }

    fn stream_terminal_output(&self, session_id: u64, revision: u64, chunk: &str) {
        if !self.agent_panel_ready && !self.terminal_panel_ready {
            return;
        }
        let Ok(chunk) = serde_json::to_string(chunk) else {
            return;
        };
        let script = format!(
            "window.receiveTerminalOutput && window.receiveTerminalOutput({session_id}, {revision}, {chunk});"
        );
        for (name, ready, panel) in [
            (
                "agent panel",
                self.agent_panel_ready,
                self.agent_panel.as_ref(),
            ),
            (
                "terminal panel",
                self.terminal_panel_ready,
                self.terminal_panel.as_ref(),
            ),
        ] {
            if ready
                && let Some(panel) = panel
                && let Err(error) = panel.evaluate_script(&script)
            {
                warn!(%error, %name, "terminal stream update failed");
            }
        }
    }

    fn stream_terminal_context_update(&self, session_id: u64) {
        if !self.agent_panel_ready {
            return;
        }
        let Some(context) = self
            .draft_terminal_contexts
            .iter()
            .find(|context| context.snapshot.session_id == session_id)
        else {
            return;
        };
        let view = context.view(self.terminal.has_session(session_id));
        let Ok(json) = serde_json::to_string(&view) else {
            return;
        };
        let script =
            format!("window.updateTerminalContext && window.updateTerminalContext({json});");
        if let Some(panel) = self.agent_panel.as_ref()
            && let Err(error) = panel.evaluate_script(&script)
        {
            warn!(%error, "terminal context update failed");
        }
    }

    fn stream_preview_image(&self, data_url: &str) {
        let Ok(data_url) = serde_json::to_string(data_url) else {
            return;
        };
        let script = format!("window.showCapturePreview && window.showCapturePreview({data_url});");
        for (name, ready, panel) in [
            (
                "agent panel",
                self.agent_panel_ready,
                self.agent_panel.as_ref(),
            ),
            (
                "detached preview",
                self.preview_panel_ready,
                self.preview_panel.as_ref(),
            ),
        ] {
            if ready
                && let Some(panel) = panel
                && let Err(error) = panel.evaluate_script(&script)
            {
                warn!(%error, %name, "preview image update failed");
            }
        }
    }

    fn restream_preview_image(&self) {
        if let Some(image) = self.preview.image.as_ref() {
            let data_url = format!(
                "data:image/jpeg;base64,{}",
                BASE64_STANDARD.encode(&image.jpeg)
            );
            self.stream_preview_image(&data_url);
        }
    }

    fn update_window_title(&self) {
        let Some(window) = &self.window else {
            return;
        };
        window.set_title("Supervisor");
    }

    fn project_chat_history_snapshot(&self) -> ProjectChatHistory {
        let chats = self
            .project_chats
            .iter()
            .map(|chat| self.chat_history_snapshot(chat))
            .collect();
        ProjectChatHistory { chats }
    }

    fn save_project_chat_history(&self) {
        let path = self.data_dir.join(PROJECT_CHAT_HISTORY_FILE);
        let history = self.project_chat_history_snapshot();
        match serde_json::to_vec_pretty(&history) {
            Ok(json) => {
                if let Err(error) = write_file_atomically::<ProjectChatHistory>(&path, &json) {
                    warn!(%error, path = %path.display(), "project chat history was not saved");
                }
            }
            Err(error) => warn!(%error, "project chat history was not serialized"),
        }
    }

    fn session_snapshot(&self) -> BrowserSession {
        let active_index = self
            .active_tab_id
            .and_then(|id| self.tabs.iter().position(|tab| tab.id == id))
            .unwrap_or_default();
        let primary_index = self
            .primary_tab_id
            .and_then(|id| self.tabs.iter().position(|tab| tab.id == id));
        let docked_index = self
            .docked_tab_id
            .and_then(|id| self.tabs.iter().position(|tab| tab.id == id));
        BrowserSession {
            browser_panel_minimized: self.browser_panel_minimized,
            tabs: self
                .tabs
                .iter()
                .map(|tab| SessionTab {
                    url: is_supported_url(&tab.url).then(|| tab.url.clone()),
                    detached: tab.detached_window.is_some(),
                })
                .collect(),
            active_index,
            primary_index,
            docked_index,
            tab_dock_side: self.tab_dock_side,
            terminal_dock: self.terminal_dock,
            agent_panel_visible: self.agent_panel_visible,
            agent_panel_detached: self.agent_panel_window.is_some(),
            agent_panel_width_logical: self.agent_panel_width_logical,

            agent_provider: self.agent_provider,
            claude_selection: self.claude_selection.clone(),
            cursor_selection: self.cursor_selection.clone(),
            github_copilot_selection: self.github_copilot_selection.clone(),
            google_antigravity_selection: self.google_antigravity_selection.clone(),
            opencode_go_selection: self.opencode_go_selection.clone(),
            claude_session_id: self.claude_session_id.clone(),
            claude_session_cwd: self.claude_session_cwd.clone(),
            theme: self.theme.clone(),
            search_engine: self.search_engine.clone(),
            workspace_root: self.workspace.root().map(|path| path.display().to_string()),
            workspace_roots: self
                .workspace
                .project_roots()
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            archived_workspace_roots: self
                .workspace
                .archived_project_roots()
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            workspace_project_labels: self.workspace_project_labels.clone(),
            pinned_workspace_roots: {
                let mut roots = self
                    .pinned_workspace_roots
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>();
                roots.sort();
                roots
            },
            workspace_last_opened_ms: self.workspace_last_opened_ms.clone(),
            reopen_last_project: self.reopen_last_project,
            project_metadata: self.project_metadata.clone(),
            remote_projects: self.remote_projects.clone(),
            active_project_chat_id: self.active_project_chat_id.clone(),
            window_access_enabled: self.window_access_enabled,
            audit_retention_days: self.audit.retention_days(),
            agent_graph_bindings: self.agent_graph_bindings.clone(),
            // Graph-agent turns have their own atomic history file. Avoid cloning an unbounded
            // history into this frequently saved DTO only for serde to skip the field.
            agent_graph_sessions: Vec::new(),
            agent_graph_links: self.agent_graph_links.clone(),
            agent_graph_launcher_position: self.agent_graph_launcher_position,
            agent_graph_launcher_v1: true,
            ssh_profiles: self.ssh.profiles().to_vec(),
            remote_desktop: self.remote_desktop.preference(),
        }
    }

    fn save_session(&self) {
        let session = self.session_snapshot();
        let path = self.data_dir.join("session.json");

        match serde_json::to_vec_pretty(&session) {
            Ok(json) => {
                if let Err(error) = write_file_atomically::<BrowserSession>(&path, &json) {
                    warn!(%error, path = %path.display(), "session was not saved");
                }
            }
            Err(error) => warn!(%error, "session was not serialized"),
        }
        self.save_project_chat_history();
    }

    fn system_tray_available(&self) -> bool {
        #[cfg(windows)]
        {
            self.background_mode_enabled && self.system_tray.is_some()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    fn hide_application_windows(&self) {
        if let Some(window) = self.window.as_ref() {
            window.set_visible(false);
        }
        for window in [
            self.agent_panel_window.as_ref(),
            self.terminal_window.as_ref(),
            self.preview_window.as_ref(),
        ]
        .into_iter()
        .flatten()
        .chain(
            self.tabs
                .iter()
                .filter_map(|tab| tab.detached_window.as_ref()),
        ) {
            window.set_visible(false);
        }
        info!("Supervisor windows hidden; agent work continues in the system tray");
    }

    fn restore_application_windows(&mut self) {
        if let Some(window) = self.window.as_ref() {
            window.set_minimized(false);
            window.set_visible(true);
        }
        for window in [
            self.agent_panel_window.as_ref(),
            self.terminal_window.as_ref(),
            self.preview_window.as_ref(),
        ]
        .into_iter()
        .flatten()
        .chain(
            self.tabs
                .iter()
                .filter_map(|tab| tab.detached_window.as_ref()),
        ) {
            window.set_minimized(false);
            window.set_visible(true);
        }
        self.layout_webviews();
        if let Some(window) = self.window.as_ref() {
            window.focus_window();
        }
    }

    fn request_main_window_close(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.file_editor.flush() {
            self.file_editor.status =
                format!("Could not back up unsaved edits before closing: {error}");
            self.render_file_editor();
            return;
        }
        if self.system_tray_available() {
            self.save_session();
            self.hide_application_windows();
            return;
        }
        if !self.time_machine_finalizations_in_flight.is_empty() {
            let finalizing_run_ids = self
                .time_machine_finalizations_in_flight
                .iter()
                .copied()
                .collect::<Vec<_>>();
            for run_id in finalizing_run_ids {
                if let Some(run) = self.agent_run_for_id_mut(run_id) {
                    run.phase = AgentPhase::Finalizing;
                    run.status = "Finalizing Time Machine checkpoint before closing…".to_owned();
                }
            }
            self.render_agent_panel();
            self.render_agent_graph_surface();
            return;
        }
        self.finish_application_exit(
            event_loop,
            "Supervisor closed before the run completed",
            "app_close",
        );
    }

    #[cfg(windows)]
    fn handle_system_tray_action(
        &mut self,
        event_loop: &ActiveEventLoop,
        action: crate::system_tray::Action,
    ) {
        match action {
            crate::system_tray::Action::Open => {
                if self.tray_quit_deadline.is_none() {
                    self.restore_application_windows();
                }
            }
            crate::system_tray::Action::StopAllAndQuit => {
                self.begin_tray_quit(event_loop);
            }
            crate::system_tray::Action::Recreate => {
                let result = self
                    .system_tray
                    .as_mut()
                    .map(crate::system_tray::SystemTray::recreate);
                if let Some(Err(error)) = result {
                    warn!(%error, "system tray icon could not be restored after Explorer restarted");
                    self.system_tray.take();
                    self.background_mode_enabled = false;
                    if self.tray_quit_deadline.is_none() {
                        self.restore_application_windows();
                    }
                }
            }
        }
    }

    fn begin_tray_quit(&mut self, event_loop: &ActiveEventLoop) {
        if self.tray_quit_deadline.is_some() {
            return;
        }
        if let Err(error) = self.file_editor.flush() {
            self.file_editor.status =
                format!("Could not back up unsaved edits before quitting: {error}");
            self.restore_application_windows();
            self.render_file_editor();
            return;
        }

        // No accepted or queued request may start while the tray quit is
        // interrupting work that is already running.
        self.agent_submission_queue.clear();
        self.pending_agent_submissions.clear();
        self.tray_quit_deadline = Some(Instant::now() + Duration::from_secs(10));
        #[cfg(windows)]
        if let Some(tray) = self.system_tray.as_mut()
            && let Err(error) = tray.set_stopping()
        {
            warn!(%error, "system tray status could not be updated");
        }

        let native_owners = self.app_server.stoppable_owners();
        for owner in native_owners {
            self.stop_app_server(&owner);
        }

        let mut run_ids = self
            .main_runs
            .values()
            .filter(|run| run.is_active())
            .map(|run| run.id)
            .collect::<HashSet<_>>();
        run_ids.extend(
            self.agent_graph_runs
                .values()
                .filter(|presentation| presentation.runtime.is_active())
                .map(|presentation| presentation.run_id),
        );
        for run_id in run_ids {
            self.stop_agent_run_by_id(run_id);
        }

        let pending_request_ids = self.pending_commands.keys().cloned().collect::<Vec<_>>();
        for request_id in pending_request_ids {
            self.deny_pending_action(&request_id, "Supervisor is stopping all work and quitting");
        }
        self.processes
            .cancel_all("Cancelled because Supervisor is stopping all work and quitting");
        self.terminal.interrupt_all_agent_commands();
        self.remote_desktop.set_agent_control(false);
        self.hide_application_windows();
        self.save_session();
        self.try_finish_tray_quit(event_loop);
    }

    fn tray_quit_ready(&self) -> bool {
        self.app_server.stoppable_owners().is_empty()
            && self.claude_jobs.is_empty()
            && self.subscription_jobs.is_empty()
            && !self.main_runs.values().any(AgentRun::is_active)
            && !self
                .agent_graph_runs
                .values()
                .any(|presentation| presentation.runtime.is_active())
            && self.pending_run_finalizations.is_empty()
            && self.pending_run_outcomes.is_empty()
            && self.time_machine_finalizations_in_flight.is_empty()
    }

    fn try_finish_tray_quit(&mut self, event_loop: &ActiveEventLoop) {
        let Some(deadline) = self.tray_quit_deadline else {
            return;
        };
        let timed_out = Instant::now() >= deadline;
        if !timed_out && !self.tray_quit_ready() {
            return;
        }
        if timed_out && !self.tray_quit_ready() {
            warn!("tray quit grace period elapsed; remaining owned work will be terminated");
        }
        self.tray_quit_deadline = None;
        self.finish_application_exit(
            event_loop,
            "Supervisor quit from the tray before the run completed",
            "tray_quit",
        );
    }

    fn finish_application_exit(
        &mut self,
        event_loop: &ActiveEventLoop,
        unfinished_status: &str,
        timing_reason: &'static str,
    ) {
        self.remote_desktop.disconnect();
        let mut closing_run_ids = HashSet::new();

        for job in self.claude_jobs.values() {
            closing_run_ids.insert(job.run_id);
            job.handle.cancel();
        }
        for job in self.subscription_jobs.values() {
            closing_run_ids.insert(job.run_id);
            job.handle.cancel();
        }
        self.processes
            .cancel_all("Cancelled because Supervisor is closing");
        let graph_run_ids = self
            .agent_graph_runs
            .values()
            .map(|presentation| presentation.run_id)
            .collect::<Vec<_>>();
        for run in self.main_runs.values_mut().filter(|run| run.is_active()) {
            closing_run_ids.insert(run.id);
            run.phase = AgentPhase::Stopped;
            run.status = unfinished_status.to_owned();
        }
        for run_id in &graph_run_ids {
            closing_run_ids.insert(*run_id);
            if let Some(run) = self.agent_run_for_id_mut(*run_id) {
                run.phase = AgentPhase::Stopped;
                run.status = unfinished_status.to_owned();
            }
        }
        for run_id in closing_run_ids.iter().copied() {
            self.finish_run_timing(run_id, timing_reason);
        }
        // Do not snapshot or finalize an in-flight run here. Provider workers and
        // owned processes may still be flushing filesystem writes. The active Time
        // Machine manifest is intentionally left on disk for startup recovery.
        self.save_session();
        self.shutdown_for_exit();
        event_loop.exit();
    }

    fn shutdown_for_exit(&mut self) {
        // WebView2 controllers are apartment-bound. Release every controller on
        // the winit UI thread, before their host windows and shared contexts are
        // dropped during event-loop teardown. This also prevents a stale runtime
        // process from surfacing breakpoint dialogs on the next launch.
        #[cfg(windows)]
        self.system_tray.take();
        self.computer_use_frame.clear();
        self.app_server.shutdown_owned_process();
        self.preview_panel.take();
        self.terminal_panel.take();
        self.agent_graph_surface.take();
        self.agent_panel.take();
        self.toolbar.take();
        self.backdrop.take();
        self.tabs.clear();
        self.content_context.take();
        self.ui_context.take();
    }
}

impl ApplicationHandler<BrowserEvent> for BrowserApp {
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() || event_loop.exiting() {
            return;
        }

        self.try_finish_tray_quit(event_loop);
        if event_loop.exiting() {
            return;
        }
        self.refresh_claude_catalog(true);
        if self.file_editor.flush_due() {
            self.render_file_editor();
        }
        let deadline = [
            self.file_editor.deadline(),
            self.claude_catalog_refresh.deadline(),
            self.tray_quit_deadline,
        ]
        .into_iter()
        .flatten()
        .min();
        event_loop.set_control_flow(deadline.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(error) = self.initialize(event_loop) {
            error!(error = %format!("{error:#}"), "browser initialization failed");
            self.startup_error = Some(error);
            event_loop.exit();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: BrowserEvent) {
        match event {
            #[cfg(windows)]
            BrowserEvent::SystemTray(action) => self.handle_system_tray_action(event_loop, action),
            #[cfg(windows)]
            BrowserEvent::BrowserUseBridge(request) => {
                self.handle_browser_use_bridge_request(request)
            }
            BrowserEvent::AppServer(event) => self.handle_app_server_event(event_loop, event),
            BrowserEvent::Toolbar(command) => self.handle_toolbar_command(event_loop, command),
            BrowserEvent::ToolbarReady => {
                self.toolbar_ready = true;
                self.render_toolbar();
            }
            BrowserEvent::AgentPanel(message) => {
                self.handle_agent_panel_message(event_loop, message)
            }
            BrowserEvent::ScopedAgentPanel { owner, message } => {
                if !main_runs::requires_chat_owner(&message) || owner == self.composer_owner() {
                    self.handle_agent_panel_message(event_loop, message);
                } else {
                    self.conversation_event("central-agent:conversation-error", json!({"owner": owner,
                        "message": "The selected conversation changed. Return to the original chat and try again; its draft was kept."}));
                    self.render_agent_panel();
                }
            }
            BrowserEvent::AgentPanelReady => {
                self.agent_panel_ready = true;
                if self.app_server.take_startup_connection() {
                    self.app_server_account(app_server::AccountAction::Connect);
                }
                self.render_agent_panel();
                self.render_file_editor();
                self.restream_preview_image();
            }
            BrowserEvent::ChatZoom(action) => self.apply_chat_zoom(action),
            BrowserEvent::AgentGraphSurfaceReady => {
                self.agent_graph_surface_ready = true;
                self.layout_agent_graph_surface();
                self.render_agent_graph_surface();
            }
            BrowserEvent::TerminalPanel(message) => {
                self.handle_agent_panel_message(event_loop, message)
            }
            BrowserEvent::TerminalPanelReady => {
                self.terminal_panel_ready = true;
                self.render_agent_panel();
            }
            BrowserEvent::PreviewPanelReady => {
                self.preview_panel_ready = true;
                self.render_detached_preview();
                self.restream_preview_image();
            }
            BrowserEvent::RemoteDesktopPanel { tab_id, message } => {
                self.handle_remote_desktop_message(tab_id, message)
            }
            BrowserEvent::RemoteDesktop(event) => self.handle_remote_desktop_event(event),
            BrowserEvent::RemoteDesktopDrag { tab_id, event } => {
                self.handle_remote_desktop_drag(tab_id, event)
            }
            BrowserEvent::RemoteDesktopUploadCompleted { profile_id, result } => {
                self.complete_remote_desktop_upload(profile_id, result)
            }
            BrowserEvent::AgentGraphProjectDrop(event) => {
                self.handle_agent_graph_project_drop(event)
            }
            BrowserEvent::AgentGraphFileDropResolved { paths, owner } => {
                self.complete_agent_graph_file_drop(paths, owner)
            }
            BrowserEvent::ProjectInspectionCompleted { key, result } => {
                self.complete_project_inspection(key, result)
            }
            BrowserEvent::ProjectCloneCompleted {
                operation_id,
                result,
            } => self.complete_project_clone(&operation_id, result),
            BrowserEvent::PageLoad {
                tab_id,
                loading,
                url,
            } => self.handle_page_load(tab_id, loading, url),
            BrowserEvent::TitleChanged { tab_id, title } => {
                self.handle_title_changed(tab_id, title)
            }
            #[cfg(windows)]
            BrowserEvent::BrowserPopup { opener_tab_id, url } => {
                match self.create_tab(Some(url), true) {
                    Ok(tab_id) => self.adopt_browser_use_popup(opener_tab_id, tab_id),
                    Err(error) => error!(%error, "popup could not be converted into a tab"),
                }
            }
            BrowserEvent::OpenInNewTab(url) => {
                if let Err(error) = self.create_tab(Some(url), true) {
                    error!(%error, "popup could not be converted into a tab");
                }
            }
            BrowserEvent::ScriptResult {
                request_id,
                action,
                raw_result,
            } => self.handle_script_result(request_id, action, raw_result),
            BrowserEvent::TabContextResult {
                tab_id,
                capture_id,
                raw_result,
            } => self.handle_tab_context_result(tab_id, capture_id, raw_result),
            BrowserEvent::TabVisualResult {
                tab_id,
                capture_id,
                raw_result,
                truncated,
            } => self.handle_tab_visual_result(tab_id, capture_id, raw_result, truncated),

            BrowserEvent::ProjectDiff(reply) => self.handle_project_diff(reply),
            BrowserEvent::BoardWorktreeCompleted {
                operation_id,
                name,
                result,
            } => self.finish_board_worktree(&operation_id, &name, result),

            BrowserEvent::TimeMachineStarted { run_id, result } => {
                self.complete_time_machine_start(run_id, result)
            }
            BrowserEvent::TimeMachineFinished { run_id, result } => {
                self.complete_time_machine_checkpoint(run_id, result)
            }
            BrowserEvent::TimeMachineLiveDiffTick { run_id } => {
                self.start_time_machine_live_diff_scan(run_id)
            }
            BrowserEvent::TimeMachineLiveDiffUpdated { run_id, result } => {
                self.complete_time_machine_live_diff(run_id, result)
            }
            BrowserEvent::SupervisorReviewTick {
                node_key,
                generation,
            } => self.handle_supervision_review_tick(node_key, generation),
            BrowserEvent::ClaudeProviderProbed(provider) => {
                self.handle_claude_provider_probed(provider)
            }
            BrowserEvent::ClaudeLoginCompleted(result) => {
                self.handle_claude_login_completed(result)
            }
            BrowserEvent::ClaudeProvider(event) => self.handle_claude_provider_event(event),
            BrowserEvent::SubscriptionProviderProbed { provider, result } => {
                self.handle_subscription_provider_probed(provider, result)
            }
            BrowserEvent::SubscriptionLoginCompleted { provider, result } => {
                self.handle_subscription_login_completed(provider, result)
            }
            BrowserEvent::SubscriptionProvider { provider, event } => {
                self.handle_subscription_provider_event(provider, event)
            }
            BrowserEvent::Terminal(event) => self.handle_terminal_event(event),
            BrowserEvent::TerminalContextRefresh { session_id } => {
                self.refresh_following_terminal_context(session_id, true)
            }
            BrowserEvent::ManagedProcess(event) => self.handle_managed_process_event(event),
            BrowserEvent::WindowCapture {
                request_id,
                action,
                generation,
                result,
            } => self.handle_window_capture(request_id, action, generation, result),
            BrowserEvent::WindowCaptureTick { generation } => {
                self.continue_window_capture(generation)
            }
            BrowserEvent::UiAutomation {
                request_id,
                action,
                result,
            } => self.handle_ui_automation_result(request_id, action, result),
            BrowserEvent::Safety(event) => self.handle_safety_event(event),
            BrowserEvent::DevelopmentUiChanged(changed) => self.reload_development_ui(&changed),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let WindowEvent::ScaleFactorChanged { scale_factor, .. } = &event {
            for window in [
                self.window.as_ref(),
                self.agent_panel_window.as_ref(),
                self.terminal_window.as_ref(),
                self.preview_window.as_ref(),
            ]
            .into_iter()
            .flatten()
            .chain(
                self.tabs
                    .iter()
                    .filter_map(|tab| tab.detached_window.as_ref()),
            ) {
                if window.id() == window_id {
                    brand::apply_window_icons(window, *scale_factor, &self.theme);
                    break;
                }
            }
        }
        if self.window.as_ref().map(Window::id) == Some(window_id) {
            match event {
                WindowEvent::CloseRequested => self.request_main_window_close(event_loop),
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    self.layout_webviews();
                }
                _ => {}
            }
            return;
        }

        if self.agent_panel_window.as_ref().map(Window::id) == Some(window_id) {
            match event {
                WindowEvent::CloseRequested => {
                    if let Err(error) = self.attach_agent_panel() {
                        warn!(%error, "agent window could not be reattached");
                    }
                }
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    self.layout_webviews();
                }
                _ => {}
            }
            return;
        }

        if self.terminal_window.as_ref().map(Window::id) == Some(window_id) {
            match event {
                WindowEvent::CloseRequested => {
                    if let Err(error) = self.attach_terminal_panel(true) {
                        warn!(%error, "terminal window could not be reattached");
                    }
                }
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    self.layout_webviews();
                }
                _ => {}
            }
            return;
        }

        if self.preview_window.as_ref().map(Window::id) == Some(window_id) {
            match event {
                WindowEvent::CloseRequested => self.close_preview(),
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    self.layout_preview_webview();
                }
                _ => {}
            }
            return;
        }

        let detached_tab_id = self.tabs.iter().find_map(|tab| {
            tab.detached_window
                .as_ref()
                .filter(|window| window.id() == window_id)
                .map(|_| tab.id)
        });
        let Some(tab_id) = detached_tab_id else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => {
                if let Err(error) = self.attach_tab(tab_id) {
                    warn!(%error, tab_id, "detached tab window could not be reattached");
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.layout_webviews();
            }
            WindowEvent::Focused(true) => {
                self.active_tab_id = Some(tab_id);
                self.render_toolbar();
                self.render_agent_panel();
            }
            _ => {}
        }
    }
}

fn load_session(path: &Path) -> BrowserSession {
    match read_browser_session(path) {
        Ok(session) => session,
        Err(primary_error) => {
            let backup_path = backup_path_for(path);
            match read_browser_session(&backup_path) {
                Ok(session) => {
                    warn!(
                        error = %primary_error,
                        path = %path.display(),
                        backup = %backup_path.display(),
                        "session recovered from the last valid backup"
                    );
                    if let Err(error) = repair_primary_from_backup(path, &backup_path) {
                        warn!(
                            %error,
                            path = %path.display(),
                            backup = %backup_path.display(),
                            "recovered session primary could not be repaired; the valid backup will be preserved on the next save"
                        );
                    }
                    session
                }
                Err(backup_error)
                    if primary_error.kind() == io::ErrorKind::NotFound
                        && backup_error.kind() == io::ErrorKind::NotFound =>
                {
                    BrowserSession::default()
                }
                Err(backup_error) => {
                    warn!(
                        error = %primary_error,
                        backup_error = %backup_error,
                        path = %path.display(),
                        "session could not be recovered"
                    );
                    BrowserSession::default()
                }
            }
        }
    }
}

fn read_browser_session(path: &Path) -> io::Result<BrowserSession> {
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn load_project_chat_history(path: &Path) -> ProjectChatHistory {
    match read_project_chat_history(path) {
        Ok(history) => history,
        Err(primary_error) => {
            let backup_path = backup_path_for(path);
            match read_project_chat_history(&backup_path) {
                Ok(history) => {
                    warn!(
                        error = %primary_error,
                        path = %path.display(),
                        backup = %backup_path.display(),
                        "project chat history recovered from backup"
                    );
                    if let Err(error) = repair_primary_from_backup(path, &backup_path) {
                        warn!(%error, path = %path.display(), "project chat primary could not be repaired");
                    }
                    history
                }
                Err(backup_error)
                    if primary_error.kind() == io::ErrorKind::NotFound
                        && backup_error.kind() == io::ErrorKind::NotFound =>
                {
                    ProjectChatHistory::default()
                }
                Err(backup_error) => {
                    warn!(
                        error = %primary_error,
                        backup_error = %backup_error,
                        path = %path.display(),
                        "project chat history could not be recovered"
                    );
                    ProjectChatHistory::default()
                }
            }
        }
    }
}

fn read_project_chat_history(path: &Path) -> io::Result<ProjectChatHistory> {
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn normalize_loaded_project_chats(chats: &mut Vec<ProjectChat>) -> bool {
    let mut ids = HashSet::new();
    let mut artifact_duplicates_removed = false;
    chats.retain_mut(|chat| {
        chat.id = chat.id.trim().to_owned();
        chat.project_root = chat.project_root.trim().to_owned();
        if chat.id.is_empty() || !ids.insert(chat.id.clone()) {
            return false;
        }
        for message in &mut chat.messages {
            artifact_duplicates_removed |= remove_checkpoint_artifact_duplicates(
                &mut message.artifacts,
                message.checkpoint.as_ref(),
            );
            message.streaming = false;
        }
        if let Some(title) = normalized_user_title(&chat.title, MAX_PROJECT_CHAT_TITLE_CHARS) {
            chat.title = title;
        } else {
            chat.title = project_chat_title(&chat.messages);
            chat.title_custom = false;
        }
        true
    });
    artifact_duplicates_removed
}

fn remove_checkpoint_artifact_duplicates(
    artifacts: &mut Vec<ChatArtifact>,
    checkpoint: Option<&CheckpointSummary>,
) -> bool {
    let Some(checkpoint) = checkpoint else {
        return false;
    };
    let changed_paths = checkpoint
        .changes
        .iter()
        .map(|change| normalize_display_path(&change.path))
        .filter(|path| !path.is_empty())
        .collect::<HashSet<_>>();
    let previous_len = artifacts.len();
    artifacts.retain(|artifact| {
        let duplicates_checkpoint_file = artifact
            .display_path
            .as_deref()
            .map(normalize_display_path)
            .is_some_and(|path| changed_paths.contains(&path));
        !duplicates_checkpoint_file
    });
    artifacts.len() != previous_len
}

fn reassign_project_chat(
    chats: &mut [ProjectChat],
    chat_id: &str,
    project_root: &str,
    updated_at_ms: u128,
) -> bool {
    let Some(chat) = chats
        .iter_mut()
        .find(|chat| chat.id == chat_id && !chat.archived && chat.project_root != project_root)
    else {
        return false;
    };
    chat.project_root = project_root.to_owned();
    chat.updated_at_ms = updated_at_ms;
    // Provider-native resumable sessions are tied to their original working
    // directory. The visible conversation remains intact and is replayed when
    // the moved chat next starts a provider session in its new project.
    chat.claude_session_id = None;
    chat.claude_session_cwd = None;
    true
}

fn normalized_user_title(value: &str, max_chars: usize) -> Option<String> {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        return None;
    }
    Some(compact.chars().take(max_chars).collect())
}

fn project_chat_title<'a>(messages: impl IntoIterator<Item = &'a ChatMessage>) -> String {
    let title = messages
        .into_iter()
        .find(|message| message.role == ChatRole::User && message.kind == ChatMessageKind::Message)
        .map(|message| {
            message
                .text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    if title.is_empty() {
        return "New chat".to_owned();
    }
    let mut chars = title.chars();
    let compact = chars.by_ref().take(54).collect::<String>();
    if chars.next().is_some() {
        format!("{compact}…")
    } else {
        compact
    }
}

fn backup_path_for(path: &Path) -> PathBuf {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map_or_else(|| "bak".to_owned(), |extension| format!("{extension}.bak"));
    path.with_extension(extension)
}

fn write_file_atomically<T>(path: &Path, bytes: &[u8]) -> io::Result<()>
where
    T: DeserializeOwned,
{
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("central-agent-state");
    let temporary_path = parent.join(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
    let backup_path = backup_path_for(path);
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);

        if path.exists() {
            // A separately readable backup lets startup recover even if the machine loses power
            // during replacement or an older build left a truncated primary file.
            let primary_is_valid = fs::read(path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<T>(&bytes).ok())
                .is_some();
            if primary_is_valid || !backup_path.exists() {
                fs::copy(path, &backup_path)?;
            }
            replace_file(path, &temporary_path)?;
        } else {
            fs::rename(&temporary_path, path)?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

fn repair_primary_from_backup(path: &Path, backup_path: &Path) -> io::Result<()> {
    let bytes = fs::read(backup_path)?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("central-agent-state");
    let temporary_path = parent.join(format!(".{file_name}.{}.repair.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary_path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        if path.exists() {
            replace_file(path, &temporary_path)
        } else {
            fs::rename(&temporary_path, path)
        }
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

#[cfg(windows)]
fn replace_file(path: &Path, replacement: &Path) -> io::Result<()> {
    let path = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let replacement = replacement
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        ReplaceFileW(
            PCWSTR(path.as_ptr()),
            PCWSTR(replacement.as_ptr()),
            PCWSTR::null(),
            REPLACEFILE_WRITE_THROUGH,
            None,
            None,
        )
    }
    .map_err(io::Error::other)
}

#[cfg(not(windows))]
fn replace_file(path: &Path, replacement: &Path) -> io::Result<()> {
    fs::rename(replacement, path)
}

fn full_window_bounds(window: &Window) -> Rect {
    let size = window.inner_size();
    Rect {
        position: PhysicalPosition::new(0, 0).into(),
        size: size.into(),
    }
}

fn should_defer_new_tab_activation(
    activate: bool,
    primary_tab_id: Option<u64>,
    docked_tab_id: Option<u64>,
) -> bool {
    activate && (primary_tab_id.is_some() || docked_tab_id.is_some())
}

fn ordered_tab_visibility_updates(
    tabs: &[(u64, bool)],
    primary_tab_id: Option<u64>,
    docked_tab_id: Option<u64>,
) -> Vec<(usize, bool)> {
    let is_visible = |(tab_id, detached): &(u64, bool)| {
        *detached || Some(*tab_id) == primary_tab_id || Some(*tab_id) == docked_tab_id
    };
    let mut updates = Vec::with_capacity(tabs.len());
    updates.extend(
        tabs.iter()
            .enumerate()
            .filter(|(_, tab)| is_visible(tab))
            .map(|(index, _)| (index, true)),
    );
    updates.extend(
        tabs.iter()
            .enumerate()
            .filter(|(_, tab)| !is_visible(tab))
            .map(|(index, _)| (index, false)),
    );
    updates
}

#[cfg(windows)]
fn reparent_webview(webview: &WebView, window: &Window) -> anyhow::Result<()> {
    let handle = window
        .window_handle()
        .context("native window handle is unavailable")?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err(anyhow!("window is not backed by a Win32 HWND"));
    };
    webview
        .reparent(handle.hwnd.get())
        .context("WebView2 reparenting failed")
}

#[cfg(not(windows))]
fn reparent_webview(_webview: &WebView, _window: &Window) -> anyhow::Result<()> {
    Err(anyhow!(
        "detached WebView windows are currently supported on Windows"
    ))
}

#[cfg(windows)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NativeCaptionPalette {
    caption: u32,
    text: u32,
    border: u32,
    dark: bool,
}

#[cfg(windows)]
const fn color_ref(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 | ((green as u32) << 8) | ((blue as u32) << 16)
}

#[cfg(windows)]
fn native_caption_palette(theme: &str) -> NativeCaptionPalette {
    if normalize_theme(theme) == "central_dark" {
        NativeCaptionPalette {
            caption: color_ref(13, 13, 13),
            text: color_ref(241, 241, 239),
            border: color_ref(13, 13, 13),
            dark: true,
        }
    } else {
        NativeCaptionPalette {
            caption: color_ref(238, 238, 236),
            text: color_ref(23, 23, 23),
            border: color_ref(238, 238, 236),
            dark: false,
        }
    }
}

#[cfg(windows)]
fn apply_native_window_theme(window: &Window, theme: &str) {
    brand::apply_window_icons(window, window.scale_factor(), theme);
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let palette = native_caption_palette(theme);
    let hwnd = windows::Win32::Foundation::HWND(handle.hwnd.get() as *mut core::ffi::c_void);
    let dark_mode = i32::from(palette.dark);

    window.set_theme(Some(if palette.dark {
        Theme::Dark
    } else {
        Theme::Light
    }));

    for (attribute, value) in [
        (DWMWA_CAPTION_COLOR, palette.caption),
        (DWMWA_TEXT_COLOR, palette.text),
        (DWMWA_BORDER_COLOR, palette.border),
    ] {
        if let Err(error) = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                attribute,
                (&raw const value).cast(),
                std::mem::size_of_val(&value) as u32,
            )
        } {
            warn!(%error, ?attribute, "native window color was not applied");
        }
    }
    if let Err(error) = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&raw const dark_mode).cast(),
            std::mem::size_of_val(&dark_mode) as u32,
        )
    } {
        warn!(%error, "native window appearance mode was not applied");
    }
}

#[cfg(not(windows))]
fn apply_native_window_theme(window: &Window, theme: &str) {
    brand::apply_window_icons(window, window.scale_factor(), theme);
    window.set_theme(Some(if normalize_theme(theme) == "central_dark" {
        Theme::Dark
    } else {
        Theme::Light
    }));
}

fn project_toolbar_bounds(window: &Window) -> Rect {
    let size = window.inner_size();
    let side = project_toolbar_height(window)
        .min(size.width)
        .min(size.height);
    Rect {
        position: PhysicalPosition::new(0, 0).into(),
        size: PhysicalSize::new(side, side).into(),
    }
}

fn browser_toolbar_bounds(
    window: &Window,
    browser_panel_left: Option<u32>,
    requested_height: u32,
) -> Rect {
    browser_toolbar_area_for_size(window.inner_size(), browser_panel_left, requested_height)
        .into_rect()
}

fn browser_toolbar_area_for_size(
    size: PhysicalSize<u32>,
    browser_panel_left: Option<u32>,
    requested_height: u32,
) -> PixelArea {
    let left = browser_panel_left.unwrap_or_default().min(size.width);
    let height = requested_height.min(size.height);
    PixelArea {
        x: left as i32,
        y: 0,
        width: size.width.saturating_sub(left),
        height,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PixelArea {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

impl PixelArea {
    fn into_rect(self) -> Rect {
        Rect {
            position: PhysicalPosition::new(self.x, self.y).into(),
            size: PhysicalSize::new(self.width, self.height).into(),
        }
    }
}

fn browser_workspace_area_with_panel_width(
    window: &Window,
    toolbar_height: u32,
    panel_width: Option<u32>,
) -> PixelArea {
    browser_workspace_area_for_size(window.inner_size(), toolbar_height, panel_width)
}

fn browser_workspace_area_for_size(
    size: PhysicalSize<u32>,
    toolbar_height: u32,
    panel_width: Option<u32>,
) -> PixelArea {
    let toolbar_height = toolbar_height.min(size.height);
    let panel_width = panel_width.unwrap_or_default().min(size.width);
    PixelArea {
        x: panel_width as i32,
        y: toolbar_height as i32,
        width: size.width.saturating_sub(panel_width),
        height: size.height.saturating_sub(toolbar_height),
    }
}

fn browser_content_area_with_panel_width(
    window: &Window,
    toolbar_height: u32,
    panel_width: Option<u32>,
    terminal_panel_visible: bool,
    terminal_dock: TerminalDock,
) -> PixelArea {
    let area = browser_workspace_area_with_panel_width(window, toolbar_height, panel_width);
    if !terminal_panel_visible {
        return area;
    }
    let extent = match terminal_dock {
        TerminalDock::Right => terminal_width(window, area),
        TerminalDock::Bottom => terminal_height(window, area),
    };
    content_area_reserving_terminal(area, terminal_dock, extent)
}

fn content_area_reserving_terminal(
    mut area: PixelArea,
    dock: TerminalDock,
    extent: u32,
) -> PixelArea {
    match dock {
        TerminalDock::Right => area.width = area.width.saturating_sub(extent),
        TerminalDock::Bottom => area.height = area.height.saturating_sub(extent),
    }
    area
}

fn browser_content_bounds_with_panel_width(
    window: &Window,
    toolbar_height: u32,
    panel_width: Option<u32>,
    terminal_panel_visible: bool,
    terminal_dock: TerminalDock,
) -> Rect {
    browser_content_area_with_panel_width(
        window,
        toolbar_height,
        panel_width,
        terminal_panel_visible,
        terminal_dock,
    )
    .into_rect()
}

fn terminal_panel_bounds_with_panel_width(
    window: &Window,
    toolbar_height: u32,
    panel_width: Option<u32>,
    terminal_dock: TerminalDock,
) -> Rect {
    let workspace = browser_workspace_area_with_panel_width(window, toolbar_height, panel_width);
    let area = match terminal_dock {
        TerminalDock::Right => {
            let width = terminal_width(window, workspace);
            PixelArea {
                x: workspace
                    .x
                    .saturating_add(workspace.width.saturating_sub(width) as i32),
                y: workspace.y,
                width,
                height: workspace.height,
            }
        }
        TerminalDock::Bottom => {
            let height = terminal_height(window, workspace);
            PixelArea {
                x: workspace.x,
                y: workspace
                    .y
                    .saturating_add(workspace.height.saturating_sub(height) as i32),
                width: workspace.width,
                height,
            }
        }
    };
    area.into_rect()
}

fn terminal_width(window: &Window, workspace: PixelArea) -> u32 {
    let logical_width = match ui_layout_profile(window) {
        UiLayoutProfile::FullHd => TERMINAL_WIDTH_FULL_HD_LOGICAL,
        UiLayoutProfile::TwoK => TERMINAL_WIDTH_2K_LOGICAL,
        UiLayoutProfile::FourK => TERMINAL_WIDTH_4K_LOGICAL,
    };
    let preferred = (logical_width * window.scale_factor()).round() as u32;
    preferred.min(workspace.width.saturating_mul(45) / 100)
}

fn terminal_height(window: &Window, workspace: PixelArea) -> u32 {
    let logical_height = match ui_layout_profile(window) {
        UiLayoutProfile::FullHd => TERMINAL_HEIGHT_FULL_HD_LOGICAL,
        UiLayoutProfile::TwoK => TERMINAL_HEIGHT_2K_LOGICAL,
        UiLayoutProfile::FourK => TERMINAL_HEIGHT_4K_LOGICAL,
    };
    let preferred = (logical_height * window.scale_factor()).round() as u32;
    preferred.min(workspace.height.saturating_mul(48) / 100)
}

fn split_tab_areas(
    content: PixelArea,
    split: bool,
    dock_side: TabDockSide,
) -> (PixelArea, Option<PixelArea>) {
    if !split || content.width < 2 {
        return (content, None);
    }
    let left_width = content.width / 2;
    let right_width = content.width.saturating_sub(left_width);
    let left = PixelArea {
        width: left_width,
        ..content
    };
    let right = PixelArea {
        x: content.x.saturating_add(left_width as i32),
        width: right_width,
        ..content
    };
    match dock_side {
        TabDockSide::Left => (right, Some(left)),
        TabDockSide::Right => (left, Some(right)),
    }
}

fn agent_panel_bounds_with_width(window: &Window, panel_width: u32, top: u32) -> Rect {
    let size = window.inner_size();
    let top = top.min(size.height);
    let panel_width = panel_width.min(size.width);
    Rect {
        position: PhysicalPosition::new(0, top as i32).into(),
        size: PhysicalSize::new(panel_width, size.height.saturating_sub(top)).into(),
    }
}

fn settings_panel_bounds(window: &Window) -> Rect {
    full_window_bounds(window)
}

#[derive(Clone, Copy)]
struct AgentGraphLauncherPage {
    is_start_page: bool,
    loading: bool,
    detached: bool,
}

#[cfg(windows)]
fn raise_webview(webview: &WebView) {
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_TOP, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
    };

    if let Err(error) = unsafe {
        SetWindowPos(
            webview.hwnd(),
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    } {
        warn!(%error, "agent graph surface could not be raised above browser content");
    }
}

#[cfg(not(windows))]
fn raise_webview(_webview: &WebView) {}

fn toolbar_height(window: &Window) -> u32 {
    let logical_height = match ui_layout_profile(window) {
        UiLayoutProfile::FullHd => TOOLBAR_HEIGHT_FULL_HD_LOGICAL,
        UiLayoutProfile::TwoK => TOOLBAR_HEIGHT_2K_LOGICAL,
        UiLayoutProfile::FourK => TOOLBAR_HEIGHT_4K_LOGICAL,
    };
    (logical_height * window.scale_factor()).round() as u32
}

fn navigation_toolbar_height(window: &Window) -> u32 {
    (TOOLBAR_NAVIGATION_HEIGHT_LOGICAL * window.scale_factor()).round() as u32
}

fn project_toolbar_height(window: &Window) -> u32 {
    (PROJECT_TOOLBAR_HEIGHT_LOGICAL * window.scale_factor()).round() as u32
}

fn agent_panel_width(window: &Window) -> u32 {
    (default_agent_panel_width_logical(window) * window.scale_factor()).round() as u32
}

fn default_agent_panel_width_logical(window: &Window) -> f64 {
    match ui_layout_profile(window) {
        UiLayoutProfile::FullHd => AGENT_PANEL_WIDTH_FULL_HD_LOGICAL,
        UiLayoutProfile::TwoK => AGENT_PANEL_WIDTH_2K_LOGICAL,
        UiLayoutProfile::FourK => AGENT_PANEL_WIDTH_4K_LOGICAL,
    }
}

fn normalize_agent_panel_width_logical(width: Option<f64>) -> Option<f64> {
    width.filter(|width| width.is_finite() && *width > 0.0 && *width <= 10_000.0)
}

fn agent_panel_width_limits_logical(window: &Window) -> (f64, f64) {
    let logical_width = window
        .inner_size()
        .to_logical::<f64>(window.scale_factor())
        .width;
    let maximum = (logical_width - BROWSER_PANE_MIN_WIDTH_LOGICAL)
        .min(logical_width * 0.8)
        .max(AGENT_PANEL_MIN_WIDTH_LOGICAL.min(logical_width));
    (AGENT_PANEL_MIN_WIDTH_LOGICAL.min(maximum), maximum)
}

fn configured_agent_panel_width(window: &Window, override_width: Option<f64>) -> u32 {
    let (minimum, maximum) = agent_panel_width_limits_logical(window);
    let logical_width = override_width
        .unwrap_or_else(|| default_agent_panel_width_logical(window))
        .clamp(minimum, maximum);
    (logical_width * window.scale_factor()).round() as u32
}

fn ui_layout_profile(window: &Window) -> UiLayoutProfile {
    let logical = window.inner_size().to_logical::<f64>(window.scale_factor());
    ui_layout_profile_for_size(logical.width, logical.height)
}

fn ui_layout_profile_for_size(width: f64, height: f64) -> UiLayoutProfile {
    if width >= 3_400.0 && height >= 1_800.0 {
        UiLayoutProfile::FourK
    } else if width >= 2_300.0 && height >= 1_250.0 {
        UiLayoutProfile::TwoK
    } else {
        UiLayoutProfile::FullHd
    }
}

fn should_start_maximized(monitor: &MonitorHandle) -> bool {
    let logical = monitor.size().to_logical::<f64>(monitor.scale_factor());
    logical.width < INITIAL_WINDOW_WIDTH_LOGICAL + WINDOW_CHROME_WIDTH_ALLOWANCE_LOGICAL
        || logical.height < INITIAL_WINDOW_HEIGHT_LOGICAL + WINDOW_CHROME_HEIGHT_ALLOWANCE_LOGICAL
}

fn center_window_on_monitor(window: &Window, monitor: &MonitorHandle) {
    let position =
        centered_window_position(monitor.position(), monitor.size(), window.outer_size());
    window.set_outer_position(position);
}

fn load_development_html(webview: &WebView, html: &str, surface: &str) -> bool {
    match webview.load_html(html) {
        Ok(()) => true,
        Err(error) => {
            warn!(%error, %surface, "development UI surface could not be reloaded");
            false
        }
    }
}

fn centered_window_position(
    monitor_position: PhysicalPosition<i32>,
    monitor_size: PhysicalSize<u32>,
    window_size: PhysicalSize<u32>,
) -> PhysicalPosition<i32> {
    let offset_x = monitor_size.width.saturating_sub(window_size.width) / 2;
    let offset_y = monitor_size.height.saturating_sub(window_size.height) / 2;
    PhysicalPosition::new(
        monitor_position
            .x
            .saturating_add(i32::try_from(offset_x).unwrap_or(i32::MAX)),
        monitor_position
            .y
            .saturating_add(i32::try_from(offset_y).unwrap_or(i32::MAX)),
    )
}

const UI_ASSET_SCHEME: &str = "central-agent-ui";

fn ui_frontend_url() -> &'static str {
    if cfg!(any(target_os = "windows", target_os = "android")) {
        "http://central-agent-ui.localhost/frontend.js"
    } else {
        "central-agent-ui://localhost/frontend.js"
    }
}

fn frontend_asset_response(
    development_root: Option<&Path>,
    request: wry::http::Request<Vec<u8>>,
) -> wry::http::Response<std::borrow::Cow<'static, [u8]>> {
    use std::borrow::Cow;
    // This protocol serves one embedded asset, never arbitrary filesystem paths.
    let allowed = request.method() == wry::http::Method::GET
        && request.uri().host() == Some("localhost")
        && request.uri().path() == "/frontend.js"
        && request.uri().query().is_none();
    let body = if allowed {
        match development_root {
            Some(_) => Cow::Owned(
                ui_development::read_text(
                    development_root,
                    "ui/dist/central-agent-ui.js",
                    SVELTE_UI_JS,
                )
                .into_bytes(),
            ),
            None => Cow::Borrowed(SVELTE_UI_JS.as_bytes()),
        }
    } else {
        Cow::Borrowed(&b"Not found"[..])
    };
    wry::http::Response::builder()
        .status(if allowed { 200 } else { 404 })
        .header(
            "Content-Type",
            if allowed {
                "text/javascript; charset=utf-8"
            } else {
                "text/plain; charset=utf-8"
            },
        )
        .header("Cache-Control", "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .expect("fixed internal asset response headers are valid")
}

fn trusted_ui_builder<'a>(
    context: &'a mut WebContext,
    development_root: Option<&Path>,
) -> WebViewBuilder<'a> {
    if context.is_custom_protocol_registered(UI_ASSET_SCHEME) {
        return WebViewBuilder::new_with_web_context(context);
    }
    let root = development_root.map(Path::to_path_buf);
    WebViewBuilder::new_with_web_context(context)
        .with_custom_protocol(UI_ASSET_SCHEME.to_owned(), move |_, request| {
            frontend_asset_response(root.as_deref(), request)
        })
}

pub(crate) fn check_native_conversation() -> anyhow::Result<()> {
    app_server::acceptance::run()
}

/// Exercises the compiled UI without showing windows, opening projects or starting providers.
pub(crate) fn check_ui_startup() -> anyhow::Result<()> {
    #[derive(Debug, Deserialize)]
    struct CheckEvent {
        surface: String,
        passed: bool,
        errors: Vec<String>,
    }
    struct CheckApp {
        proxy: EventLoopProxy<CheckEvent>,
        window: Option<Window>,
        views: Vec<WebView>,
        contexts: Vec<WebContext>,
        profile: tempfile::TempDir,
        ready: HashSet<String>,
        error: Option<anyhow::Error>,
        deadline: Instant,
    }
    impl ApplicationHandler<CheckEvent> for CheckApp {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.window.is_some() {
                return;
            }
            let result: anyhow::Result<()> = (|| {
                let window = event_loop.create_window(
                    Window::default_attributes()
                        .with_visible(false)
                        .with_title("Supervisor startup check"),
                )?;
                self.window = Some(window);
                for (name, html, predicate) in [
                    (
                        "toolbar",
                        TOOLBAR_HTML,
                        "typeof window.renderBrowserState === 'function' && document.querySelector('[data-central-agent-svelte-mounted]')",
                    ),
                    (
                        "agent",
                        AGENT_PANEL_HTML,
                        "typeof window.renderAgentPanelState === 'function' && document.getElementById('chat-input') && document.querySelector('.file-editor-tabs')",
                    ),
                    (
                        "graph",
                        AGENT_GRAPH_HTML,
                        "typeof window.renderAgentGraphState === 'function' && typeof window.CentralAgentSvelte?.mountAgentGraphCard === 'function'",
                    ),
                    (
                        "start-page",
                        START_PAGE_HTML,
                        "document.querySelector('form input[aria-label=\"Search the web\"]') && document.querySelector('.mark-slot')",
                    ),
                ] {
                    self.contexts
                        .push(WebContext::new(Some(self.profile.path().join(name))));
                    let proxy = self.proxy.clone();
                    let native_media_probe = include_str!("../ui/tests/native-media-startup.js");
                    let chat_tree_probe = include_str!("../ui/tests/chat-tree-startup.js");
                    let profile_popup_probe = include_str!("../ui/tests/profile-popup-startup.js");
                    let configuration_probe =
                        include_str!("../ui/tests/agent-configuration-startup.js");
                    let plugin_picker_probe = include_str!("../ui/tests/plugin-picker-startup.js");
                    let settings_probe = include_str!("../ui/tests/settings-startup.js");
                    let graph_launcher_probe =
                        include_str!("../ui/tests/graph-launcher-startup.js");
                    let work_results_probe = include_str!("../ui/tests/work-results-startup.js");
                    let shell_layout_probe = include_str!("../ui/tests/shell-layout-startup.js");
                    let workspace_controls_probe =
                        include_str!("../ui/tests/workspace-controls-startup.js");
                    let work_disclosure_probe =
                        include_str!("../ui/tests/work-disclosure-startup.js");
                    let input_delivery_probe =
                        include_str!("../ui/tests/input-delivery-startup.js");
                    let delivery_timings_probe =
                        include_str!("../ui/tests/delivery-timings-startup.js");
                    let work_reference_messages =
                        serde_json::to_string(&app_server::work_reference_messages())?;
                    let work_reference_probe =
                        include_str!("../ui/tests/work-reference-startup.js");
                    let probe = format!(
                        r#"
                        const startupErrors = [];
                        let expectedNativeMediaError = null;
                        window.addEventListener('error', event => {{
                            if (event !== expectedNativeMediaError) startupErrors.push(event.message || 'Asset load failed');
                        }}, true);
                        window.addEventListener('load', async () => {{
                            const startupSurface = '{name}';
                            {shell_layout_probe}
                            // Check pristine initialization before fixture clicks can
                            // accidentally install a lazily nested host entry point.
                            if ('{name}' === 'agent' && typeof window.renderAgentPanelState !== 'function') {{
                                startupErrors.push('Agent state renderer is missing before first interaction');
                            }}
                            if ('{name}' === 'agent' || '{name}' === 'graph') {{
                                {work_disclosure_probe}
                                {input_delivery_probe}
                                {delivery_timings_probe}
                                const workReferenceMessages = {work_reference_messages};
                                {work_reference_probe}
                                {native_media_probe}
                                try {{
                                    const changes = Array.from({{ length: 200 }}, (_, i) => ({{ path: `src/file-${{i}}.rs`, status: 'modified' }}));
                                    const tree = window.createTimeMachineTree({{ changes }});
                                    document.body.append(tree);
                                    const style = getComputedStyle(tree);
                                    if (tree.querySelectorAll('.tm-file-row').length !== 200 || style.maxHeight !== 'none' || style.overflowY !== 'visible') throw new Error('Diff file tree is capped or has an inner scroll box');
                                    tree.remove();
                                }} catch (error) {{ startupErrors.push(String(error)); }}
                            }}
                            if ('{name}' === 'graph') {{
                                {graph_launcher_probe}
                                {work_results_probe}
                                document.documentElement.dataset.configurationProbeSurface = 'graph';
                                {configuration_probe}
                            }}
                            if ('{name}' === 'agent') {{
                                {chat_tree_probe}
                                {profile_popup_probe}
                                {configuration_probe}
                                {settings_probe}
                                {plugin_picker_probe}
                                try {{
                                    const sample = '@@ -1 +1 @@\n-old\n+needle one\n+needle two';
                                    const conversation = document.createElement('section'); conversation.className = 'conversation-section'; conversation.style.width = '900px';
                                    const row = document.createElement('div'); row.className = 'chat-message activity';
                                    conversation.append(row); document.body.append(conversation);
                                    window.updateActivityDetail(row, 'fixture.rs', 'synthetic', 'file', 'running', true, 2, 1, sample);
                                    const disclosure = row.querySelector('.activity-inline-diff');
                                    if (!row.classList.contains('has-activity-diff')) throw new Error('Diff activity retained the full-row hover state');
                                    const savedTheme = document.documentElement.dataset.theme;
                                    for (const theme of ['central', 'central_dark']) {{
                                        document.documentElement.dataset.theme = theme;
                                        const summaryBounds = disclosure.querySelector('summary').getBoundingClientRect();
                                        const rowBounds = row.getBoundingClientRect();
                                        if (summaryBounds.width <= 0 || summaryBounds.width >= rowBounds.width / 2 || summaryBounds.height > 40) throw new Error(`${{theme}}: diff toggle is not compact`);
                                    }}
                                    document.documentElement.dataset.theme = savedTheme;
                                    disclosure.open = true;
                                    await Promise.resolve(); await Promise.resolve();
                                    const diffStyle = getComputedStyle(row.querySelector('.diff-view'));
                                    if (['Top', 'Right', 'Bottom', 'Left'].some(side => parseFloat(diffStyle['border' + side + 'Width']) !== 0) || diffStyle.boxShadow !== 'none' || parseFloat(diffStyle.paddingLeft) !== 0) throw new Error('Diff inherited a section edge or inset');
                                    const input = row.querySelector('input[type="search"]');
                                    input.value = 'needle'; input.dispatchEvent(new Event('input', {{ bubbles: true }}));
                                    await Promise.resolve(); await Promise.resolve();
                                    if (row.querySelector('[data-diff-results]').textContent !== '1 / 2') throw new Error('Diff search did not select the first match');
                                    if (row.querySelectorAll('.cm-searchMatch').length !== 2 || row.querySelector('.cm-searchMatch-selected')?.textContent !== 'needle') throw new Error('Diff search did not highlight its matches');
                                    row.querySelector('[aria-label="Next match"]').click();
                                    await Promise.resolve(); await Promise.resolve();
                                    if (row.querySelector('[data-diff-results]').textContent !== '2 / 2' || row.querySelector('[data-diff-location]').textContent !== 'Diff line 4') throw new Error('Diff search did not advance to the second line');
                                    if (row.querySelector('.cm-searchMatch-selected')?.parentElement.textContent !== '+needle two') throw new Error('Diff highlight did not follow navigation');
                                    window.updateActivityDetail(row, 'fixture.rs', 'synthetic', 'file', 'success', false, 2, 1, sample);
                                    await Promise.resolve(); await Promise.resolve();
                                    if (!disclosure.open || row.querySelector('input[type="search"]') !== input || input.value !== 'needle' || row.querySelector('[data-diff-results]').textContent !== '2 / 2') throw new Error('Activity update reset the diff search');
                                    if (row.querySelectorAll('.cm-searchMatch').length !== 2) throw new Error('Activity update cleared diff highlights');
                                    input.value = 'not-in-the-diff'; input.dispatchEvent(new Event('input', {{ bubbles: true }}));
                                    await Promise.resolve(); await Promise.resolve();
                                    if (row.querySelector('[data-diff-results]').textContent !== 'No matches') throw new Error('Diff search no-result feedback failed');
                                    if (row.querySelector('.cm-searchMatch')) throw new Error('No-result search retained stale highlights');
                                     conversation.remove();
                                 }} catch (error) {{ startupErrors.push(String(error)); }}
                                 {workspace_controls_probe}
                             }}
                            window.ipc.postMessage('__central_startup:' + JSON.stringify({{
                                surface: '{name}', passed: startupErrors.length === 0 && Boolean(('{name}' === 'start-page' || window.CentralAgentSvelte) && ({predicate})), errors: startupErrors
                            }}));
                        }});
                    "#
                    );
                    let context = self.contexts.last_mut().expect("a check context was added");
                    let view = trusted_ui_builder(context, None)
                        .with_bounds(wry::Rect {
                            position: PhysicalPosition::new(0, 0).into(),
                            size: LogicalSize::new(1920.0, 1080.0).into(),
                        })
                        .with_initialization_script(probe)
                        .with_ipc_handler(move |request| {
                            if let Some(payload) = request.body().strip_prefix("__central_startup:")
                                && let Ok(event) = serde_json::from_str::<CheckEvent>(payload)
                            {
                                let _ = proxy.send_event(event);
                            }
                        })
                        .with_html(themed_ui_asset_html(
                            None,
                            "assets/check.html",
                            html,
                            "central_dark",
                        ))
                        .build_as_child(self.window.as_ref().expect("check window exists"))
                        .with_context(|| format!("{name} WebView failed during startup check"))?;
                    self.views.push(view);
                }
                Ok(())
            })();
            if let Err(error) = result {
                self.error = Some(error);
                event_loop.exit();
            }
        }
        fn user_event(&mut self, event_loop: &ActiveEventLoop, event: CheckEvent) {
            if !event.passed {
                self.error = Some(anyhow!(
                    "{} UI did not initialize: {:?}",
                    event.surface,
                    event.errors
                ));
                event_loop.exit();
            } else {
                self.ready.insert(event.surface);
                if self.ready.len() == 4 {
                    event_loop.exit();
                }
            }
        }
        fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            if Instant::now() >= self.deadline {
                self.error = Some(anyhow!(
                    "UI startup check timed out; ready surfaces: {:?}",
                    self.ready
                ));
                event_loop.exit();
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(self.deadline));
            }
        }
    }
    let event_loop = winit::event_loop::EventLoop::<CheckEvent>::with_user_event().build()?;
    let mut app = CheckApp {
        proxy: event_loop.create_proxy(),
        window: None,
        views: Vec::new(),
        contexts: Vec::new(),
        profile: tempfile::tempdir()?,
        ready: HashSet::new(),
        error: None,
        deadline: Instant::now() + Duration::from_secs(20),
    };
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error);
    }
    anyhow::ensure!(app.ready.len() == 4, "UI startup check did not complete");
    println!(
        "UI startup check passed: toolbar, Home, agent/editor, project board transitions (Light/Dark) and diff-search interaction verified in hidden WebViews."
    );
    Ok(())
}

fn ui_html_with_assets(
    development_root: Option<&Path>,
    template_path: &str,
    embedded_template: &str,
) -> String {
    let template = ui_development::read_text(development_root, template_path, embedded_template);
    let font = ui_development::read_bytes(
        development_root,
        "assets/fonts/inter/Inter-Variable.woff2",
        SUPERVISOR_UI_FONT,
    );
    let themes_css = ui_development::read_text(development_root, "assets/themes.css", THEMES_CSS)
        .replace(
            "__SUPERVISOR_UI_FONT_DATA_URL__",
            &format!("data:font/woff2;base64,{}", BASE64_STANDARD.encode(font)),
        );
    let file_icons_js =
        ui_development::read_text(development_root, "assets/file-icons.js", FILE_ICONS_JS);
    let svelte_ui_css = ui_development::read_text(
        development_root,
        "ui/dist/central-agent-ui.css",
        SVELTE_UI_CSS,
    )
    .replace("</style", "<\\/style");
    template
        .replace("/*__THEMES_CSS__*/", &themes_css)
        .replace("/*__SVELTE_UI_CSS__*/", &svelte_ui_css)
        .replace("/*__FILE_ICONS_JS__*/", &file_icons_js)
        .replace("__CENTRAL_AGENT_FRONTEND_URL__", ui_frontend_url())
        .replace(
            "__SUPERVISOR_WORDMARK_SVG__",
            &ui_development::read_text(
                development_root,
                "assets/supervisor-wordmark.svg",
                SUPERVISOR_WORDMARK_SVG,
            ),
        )
}

#[cfg(test)]
fn ui_html(template: &str) -> String {
    ui_html_with_assets(None, "assets/embedded.html", template)
}

fn themed_ui_asset_html(
    development_root: Option<&Path>,
    template_path: &str,
    embedded_template: &str,
    theme: &str,
) -> String {
    ui_html_with_assets(development_root, template_path, embedded_template)
        .replace("__THEME_ID__", normalize_theme(theme))
}

#[cfg(test)]
fn themed_ui_html(template: &str, theme: &str) -> String {
    ui_html(template).replace("__THEME_ID__", normalize_theme(theme))
}

fn url_origin(value: &str) -> Option<String> {
    let parsed = Url::parse(value).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| parsed.origin().ascii_serialization())
}

fn serialized_label<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned())
}

#[cfg(test)]
fn terminal_panel_html(theme: &str) -> String {
    themed_ui_html(AGENT_PANEL_HTML, theme).replacen(
        "<body>",
        "<body class=\"terminal-panel-mode\">",
        1,
    )
}

fn terminal_panel_asset_html(development_root: Option<&Path>, theme: &str) -> String {
    themed_ui_asset_html(
        development_root,
        "assets/agent-panel.html",
        AGENT_PANEL_HTML,
        theme,
    )
    .replacen("<body>", "<body class=\"terminal-panel-mode\">", 1)
}

fn default_theme() -> String {
    DEFAULT_THEME.to_owned()
}

fn default_search_engine() -> String {
    DEFAULT_SEARCH_ENGINE.to_owned()
}

fn default_agent_panel_visible() -> bool {
    true
}

fn default_reopen_last_project() -> bool {
    true
}

fn default_audit_retention_days() -> u64 {
    DEFAULT_AUDIT_RETENTION_DAYS
}

fn run_finalization_is_ready(provider_active: bool, owned_process_active: bool) -> bool {
    !provider_active && !owned_process_active
}

fn index_unfinished_time_machine_runs(
    runs: Vec<(u64, PathBuf)>,
) -> (HashMap<u64, PathBuf>, HashSet<u64>, u64) {
    let mut roots = HashMap::new();
    let mut retryable = HashSet::new();
    let mut next_run_id = 1_u64;
    for (run_id, root) in runs {
        roots.insert(run_id, root);
        retryable.insert(run_id);
        next_run_id = next_run_id.max(run_id.saturating_add(1));
    }
    (roots, retryable, next_run_id)
}

fn recovered_checkpoint_notice(run_id: u64) -> String {
    format!(
        "Time Machine recovered a workspace checkpoint left open by interrupted run {run_id}. The card below shows workspace changes detected since that checkpoint; it does not automatically attribute those changes to the agent. Review or restore them if needed."
    )
}

fn refresh_persisted_checkpoint_metadata(
    project_chats: &mut [ProjectChat],
    agent_graph_sessions: &mut [AgentGraphSession],
    time_machine: &TimeMachineRuntime,
) -> (bool, bool) {
    let mut project_changed = false;
    for message in project_chats
        .iter_mut()
        .flat_map(|chat| chat.messages.iter_mut())
    {
        let Some(checkpoint_id) = message
            .checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.id.clone())
        else {
            continue;
        };
        if let Some(summary) = time_machine.checkpoint_summary(&checkpoint_id)
            && message.checkpoint.as_ref() != Some(&summary)
        {
            message.checkpoint = Some(summary);
            project_changed = true;
        }
        if message.role == ChatRole::System
            && message
                .text
                .starts_with("Time Machine recovered the checkpoint for interrupted run ")
            && let Some(checkpoint) = message.checkpoint.as_ref()
        {
            message.text = recovered_checkpoint_notice(checkpoint.run_id);
            project_changed = true;
        }
    }

    let mut graph_changed = false;
    for turn in agent_graph_sessions
        .iter_mut()
        .flat_map(|session| session.turns.iter_mut())
    {
        let Some(checkpoint_id) = turn
            .checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.id.clone())
        else {
            continue;
        };
        if let Some(summary) = time_machine.checkpoint_summary(&checkpoint_id)
            && turn.checkpoint.as_ref() != Some(&summary)
        {
            turn.checkpoint = Some(summary);
            graph_changed = true;
        }
    }
    (project_changed, graph_changed)
}

fn should_publish_checkpoint_summary(summary: &CheckpointSummary) -> bool {
    summary.total_additions > 0 || summary.total_deletions > 0
}

fn attach_recovered_checkpoint_summaries(
    sessions: &mut [AgentGraphSession],
    summaries: Vec<CheckpointSummary>,
    next_chat_message_id: &mut u64,
) -> (VecDeque<ChatMessage>, bool) {
    let mut notices = VecDeque::new();
    let mut graph_history_changed = false;
    for summary in summaries {
        if !should_publish_checkpoint_summary(&summary) {
            continue;
        }
        let mut graph_turn_found = false;
        for session in sessions.iter_mut() {
            let Some(turn) = session
                .turns
                .iter_mut()
                .find(|turn| turn.run_id == summary.run_id)
            else {
                continue;
            };
            turn.checkpoint = Some(summary.clone());
            session.updated_at_ms = unix_time_ms();
            graph_turn_found = true;
            graph_history_changed = true;
            break;
        }
        if graph_turn_found {
            continue;
        }
        notices.push_back(ChatMessage {
            id: *next_chat_message_id,
            role: ChatRole::System,
            kind: ChatMessageKind::Message,
            text: recovered_checkpoint_notice(summary.run_id),
            streaming: false,
            timestamp_ms: unix_time_ms(),
            attachments: Vec::new(),
            terminal_attachments: Vec::new(),
            file_attachments: Vec::new(),
            artifacts: Vec::new(),
            provider: None,
            checkpoint: Some(summary.clone()),
            activity_status: None,
            activity_category: None,
            activity_detail: None,
            activity_context: None,
            activity_additions: None,
            activity_deletions: None,
            activity_diff: None,
            run_id: Some(summary.run_id),
            provider_item_id: None,
            reasoning_summary_index: None,

            native_turn_id: None,
        });
        *next_chat_message_id = next_chat_message_id.saturating_add(1);
    }
    (notices, graph_history_changed)
}

fn pending_claude_permission(request: ClaudePermissionRequest) -> PendingClaudePermission {
    PendingClaudePermission {
        run_id: request.run_id,
        request_id: request.request_id,
        tool_use_id: request.tool_use_id,
        tool_name: request.tool_name,
        summary: request.summary,
        authorization: request.authorization,
    }
}

fn pending_subscription_permission(
    provider: AgentProviderKind,
    request: SubscriptionPermissionRequest,
) -> PendingSubscriptionPermission {
    PendingSubscriptionPermission {
        run_id: request.run_id,
        provider,
        request_id: request.request_id,
        tool_use_id: request.tool_use_id,
        tool_name: request.tool_name,
        summary: request.summary,
        authorization: request.authorization,
    }
}

fn partition_session_authorized<T>(
    policy: &PermissionPolicy,
    requests: impl IntoIterator<Item = T>,
    authorization: impl Fn(&T) -> ActionAuthorization,
) -> (Vec<T>, Vec<T>) {
    requests
        .into_iter()
        .partition(|request| policy.decision(authorization(request)) == PermissionDecision::Execute)
}

#[cfg(test)]
#[test]
fn session_authorization_executes_only_actions_that_policy_rechecks_as_executable() {
    let mut policy = PermissionPolicy::default();
    policy.set_mode(PermissionMode::InitialAuthorization);
    assert!(policy.authorize_scopes([CapabilityScope::Workspace]));
    let requests = [
        ActionAuthorization::new(CapabilityScope::Workspace, ActionEffect::ReversibleWrite),
        ActionAuthorization::new(CapabilityScope::Workspace, ActionEffect::Destructive),
        ActionAuthorization::new(CapabilityScope::Process, ActionEffect::ProcessExecution),
    ];

    let (executable, deferred) =
        partition_session_authorized(&policy, requests, |request| *request);
    assert_eq!(executable, vec![requests[0]]);
    assert_eq!(deferred, vec![requests[1], requests[2]]);
}

#[cfg(test)]
#[test]
fn pending_action_ui_uses_bulk_authorization_only_before_the_initial_grant() {
    assert!(AGENT_PANEL_HTML.contains(
        "currentPermission.mode === \"initial_authorization\" && !currentPermission.sessionAuthorized"
    ));
    assert!(AGENT_GRAPH_HTML.contains(
        "state.permissionMode === \"initial_authorization\" && !state.sessionAuthorized"
    ));
    for surface in [AGENT_PANEL_HTML, AGENT_GRAPH_HTML] {
        assert!(surface.contains("approve_action"));
    }
}

fn authorization_may_mutate_workspace(authorization: ActionAuthorization) -> bool {
    authorization.effect != ActionEffect::ReadOnly
        && matches!(
            authorization.scope,
            CapabilityScope::Workspace | CapabilityScope::Terminal | CapabilityScope::Process
        )
}

fn submission_checkpoint_root(
    workspace_root: Option<&Path>,
    launch: Option<&AgentGraphLaunch>,
) -> Option<PathBuf> {
    if let Some(launch) = launch {
        if launch.ssh_profile_id.is_some() {
            return None;
        }
        let directory = Path::new(&launch.project_directory);
        if !directory.is_absolute() {
            return None;
        }
        // A selected graph node may be outside the main chat's project, or
        // exist without a selected main project. Never redirect it to that chat.
        return fs::canonicalize(directory)
            .ok()
            .filter(|root| root.is_dir());
    }
    workspace_root.map(|root| fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf()))
}

fn checkpoint_roots_overlap(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn checkpoint_run_ids_for_workspace(
    run_roots: &HashMap<u64, PathBuf>,
    workspace_root: &Path,
) -> Vec<u64> {
    let mut run_ids = run_roots
        .iter()
        .filter_map(|(run_id, run_root)| {
            checkpoint_roots_overlap(run_root, workspace_root).then_some(*run_id)
        })
        .collect::<Vec<_>>();
    run_ids.sort_unstable();
    run_ids
}

fn phase_for_step(kind: AgentStepKind) -> AgentPhase {
    match kind {
        AgentStepKind::Context | AgentStepKind::Observe => AgentPhase::Observing,
        AgentStepKind::Act => AgentPhase::Acting,
        AgentStepKind::Verify => AgentPhase::Verifying,
    }
}

fn unix_time_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn project_timestamp_ms() -> u64 {
    unix_time_ms().min(u64::MAX as u128) as u64
}

fn remote_project_metadata_key(id: &str) -> String {
    format!("ssh:{id}")
}

fn terminal_phase_name(phase: TerminalPhase) -> &'static str {
    match phase {
        TerminalPhase::Running => "running",
        TerminalPhase::Busy => "busy",
        TerminalPhase::Error => "error",
    }
}

fn terminal_kind_name(kind: TerminalKind) -> &'static str {
    match kind {
        TerminalKind::Local => "local",
        TerminalKind::Ssh => "ssh",
    }
}

fn limit_message(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn limit_context_text(value: &str, max_chars: usize) -> String {
    let mut characters = value.chars();
    let mut limited = characters.by_ref().take(max_chars).collect::<String>();
    if characters.next().is_some() {
        limited.push('…');
    }
    limited
}

fn parse_script_bridge_result(raw: &str) -> Result<ScriptBridgeResult, serde_json::Error> {
    let value: Value = serde_json::from_str(raw)?;
    match value {
        Value::String(serialized) => serde_json::from_str(&serialized),
        value => serde_json::from_value(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVELTE_CHAT_SOURCE: &str = include_str!("../ui/src/components/ChatSurface.svelte");
    const SVELTE_COMPOSER_SOURCE: &str = include_str!("../ui/src/components/Composer.svelte");
    const SVELTE_CONFIGURATION_SOURCE: &str =
        include_str!("../ui/src/components/AgentConfiguration.svelte");
    const SVELTE_MODEL_PICKER_SOURCE: &str =
        include_str!("../ui/src/components/ModelPicker.svelte");
    const SVELTE_AGENT_TIMELINE_SOURCE: &str = include_str!("../ui/src/lib/agent-timeline.ts");
    const SVELTE_EXPLORER_SOURCE: &str =
        include_str!("../ui/src/components/WorkspaceExplorer.svelte");
    const SVELTE_SETTINGS_SOURCE: &str =
        include_str!("../ui/src/components/SettingsNavigation.svelte");
    const SVELTE_SETTINGS_SHELL_SOURCE: &str =
        include_str!("../ui/src/components/SettingsShell.svelte");
    const SVELTE_SETTINGS_CATALOG_SOURCE: &str = include_str!("../ui/src/lib/settings-catalog.ts");
    const SVELTE_AGENT_GRAPH_CARD_SOURCE: &str =
        include_str!("../ui/src/components/AgentGraphCard.svelte");

    fn agent_ui_contains(value: &str) -> bool {
        [
            AGENT_PANEL_HTML,
            SVELTE_CHAT_SOURCE,
            SVELTE_COMPOSER_SOURCE,
            SVELTE_CONFIGURATION_SOURCE,
            SVELTE_MODEL_PICKER_SOURCE,
            SVELTE_AGENT_TIMELINE_SOURCE,
            SVELTE_EXPLORER_SOURCE,
            SVELTE_SETTINGS_SOURCE,
            SVELTE_SETTINGS_SHELL_SOURCE,
            SVELTE_SETTINGS_CATALOG_SOURCE,
        ]
        .iter()
        .any(|source| source.contains(value))
    }

    fn agent_graph_ui_contains(value: &str) -> bool {
        [
            AGENT_GRAPH_HTML,
            SVELTE_AGENT_GRAPH_CARD_SOURCE,
            include_str!("../ui/src/components/GraphTimeline.svelte"),
            include_str!("../ui/src/components/GraphMessage.svelte"),
            include_str!("../ui/src/components/ProjectBoard.svelte"),
            include_str!("../ui/src/components/ProjectRegistry.svelte"),
            include_str!("../ui/src/lib/graph-history.ts"),
        ]
        .iter()
        .any(|source| source.contains(value))
    }

    #[test]
    fn graph_history_pages_reach_the_first_turn_and_keep_loaded_older_turns_on_append() {
        let mut start = agent_graph_history_start(63, None);
        assert_eq!(start, 43);
        start = start.saturating_sub(AGENT_GRAPH_HISTORY_PAGE_SIZE);
        assert_eq!(agent_graph_history_start(64, Some(start)), 23);
        start = start.saturating_sub(AGENT_GRAPH_HISTORY_PAGE_SIZE);
        assert_eq!(agent_graph_history_start(64, Some(start)), 3);
        start = start.saturating_sub(AGENT_GRAPH_HISTORY_PAGE_SIZE);
        assert_eq!(agent_graph_history_start(65, Some(start)), 0);
        assert_eq!(agent_graph_history_start(0, Some(90)), 0);
        assert_eq!(agent_graph_history_start(4, None), 0);
    }

    #[test]
    fn start_page_embeds_accessible_lettering_without_a_browser_runtime() {
        let html = themed_ui_html(START_PAGE_HTML, "central_dark");
        assert!(html.contains("<h1 aria-label=\"Supervisor\">"));
        assert_eq!(html.matches("data-supervisor-logo=\"true\"").count(), 0);
        assert_eq!(html.matches("data-supervisor-wordmark=\"true\"").count(), 1);
        assert!(!html.contains("__SUPERVISOR_WORDMARK_SVG__"));
        assert!(!html.contains(ui_frontend_url()));
        assert!(html.contains("fill=\"currentColor\""));
        assert!(html.contains("class=\"mark-slot\""));
        assert!(html.contains("autofocus"));
    }

    #[test]
    fn modular_svelte_surfaces_load_local_script_before_legacy_bridge_startup() {
        let agent_html = themed_ui_html(AGENT_PANEL_HTML, "central");
        let agent_graph_html = themed_ui_html(AGENT_GRAPH_HTML, "central_dark");

        for html in [&agent_html, &agent_graph_html] {
            assert!(!html.contains("/*__SVELTE_UI_CSS__*/"));
            assert!(!html.contains("__CENTRAL_AGENT_FRONTEND_URL__"));
            assert!(html.contains(&format!("<script src=\"{}\"></script>", ui_frontend_url())));
            assert!(html.contains("data-central-agent-svelte"));
        }
        assert!(
            agent_html
                .find(ui_frontend_url())
                .expect("embedded Svelte runtime")
                < agent_html
                    .find("const pageTitle = document.getElementById(\"page-title\")")
                    .expect("legacy agent bridge startup")
        );
        assert!(
            agent_graph_html
                .find(ui_frontend_url())
                .expect("embedded Svelte runtime")
                < agent_graph_html
                    .find("const graphShell = document.getElementById(\"graph-shell\")")
                    .expect("legacy knowledge bridge startup")
        );
        assert!(agent_ui_contains("id=\"chat-input\""));
        assert!(agent_ui_contains("id=\"workspace-explorer-tree\""));
        assert!(SVELTE_SETTINGS_CATALOG_SOURCE.contains("{id:\"settings-general\""));
        assert!(agent_graph_ui_contains("data-role=\"message\""));
    }

    #[test]
    fn development_ui_assets_are_loaded_from_disk_and_reloads_stay_scoped() {
        let directory = tempfile::tempdir().unwrap();
        for relative in ["assets", "ui/dist"] {
            fs::create_dir_all(directory.path().join(relative)).unwrap();
        }
        fs::write(
            directory.path().join("assets/agent-panel.html"),
            "<html data-theme=\"__THEME_ID__\"><style>/*__THEMES_CSS__*/\n/*__SVELTE_UI_CSS__*/</style><body><script>/*__FILE_ICONS_JS__*/</script><script src=\"__CENTRAL_AGENT_FRONTEND_URL__\"></script></body></html>",
        )
        .unwrap();
        fs::write(
            directory.path().join("assets/themes.css"),
            ":root{--live:1}",
        )
        .unwrap();
        fs::write(
            directory.path().join("assets/file-icons.js"),
            "window.liveIcons=true;",
        )
        .unwrap();
        fs::write(
            directory.path().join("ui/dist/central-agent-ui.css"),
            ".live-ui{display:block}",
        )
        .unwrap();
        fs::write(
            directory.path().join("ui/dist/central-agent-ui.js"),
            "window.liveBundle=true;",
        )
        .unwrap();

        let html = themed_ui_asset_html(
            Some(directory.path()),
            "assets/agent-panel.html",
            "embedded",
            "central_dark",
        );
        assert!(html.contains("data-theme=\"central_dark\""));
        assert!(html.contains("--live:1"));
        assert!(html.contains(".live-ui{display:block}"));
        assert!(html.contains("window.liveIcons=true"));
        assert!(html.contains(ui_frontend_url()));
        let request = wry::http::Request::get("central-agent-ui://localhost/frontend.js")
            .body(Vec::new())
            .unwrap();
        let response = frontend_asset_response(Some(directory.path()), request);
        assert_eq!(response.body().as_ref(), b"window.liveBundle=true;");
        fs::write(
            directory.path().join("ui/dist/central-agent-ui.js"),
            "window.liveBundle=2;",
        )
        .unwrap();
        let request = wry::http::Request::get("central-agent-ui://localhost/frontend.js")
            .body(Vec::new())
            .unwrap();
        assert_eq!(
            frontend_asset_response(Some(directory.path()), request)
                .body()
                .as_ref(),
            b"window.liveBundle=2;"
        );
        assert!(ui_development::WATCHED_UI_FILES.contains(&"assets/supervisor-mark.svg"));

        let bundle = development_ui_reload_targets(&["ui/dist/central-agent-ui.js".to_owned()]);
        assert!(bundle.agent_panel);
        assert!(bundle.terminal_panel);
        assert!(bundle.agent_graph_surface);
        assert!(bundle.toolbar);
        assert!(!bundle.start_pages);
        assert!(!bundle.remote_desktop);

        let themes = development_ui_reload_targets(&["assets/themes.css".to_owned()]);
        assert!(themes.backdrop);
        assert!(themes.toolbar);
        assert!(themes.agent_panel);
        assert!(themes.agent_graph_surface);
        assert!(themes.terminal_panel);
        assert!(themes.preview_panel);
        assert!(themes.start_pages);
        assert!(themes.remote_desktop);
    }

    #[test]
    fn parses_direct_script_result() {
        let result = parse_script_bridge_result(r#"{"ok":true,"data":{"count":3}}"#).unwrap();
        assert!(result.ok);
        assert_eq!(result.data["count"], 3);
    }

    #[test]
    fn internal_html_stays_below_webview2_navigate_to_string_limit() {
        for template in [
            TOOLBAR_HTML,
            AGENT_PANEL_HTML,
            AGENT_GRAPH_HTML,
            PREVIEW_HTML,
            REMOTE_DESKTOP_HTML,
            START_PAGE_HTML,
            BACKDROP_HTML,
        ] {
            for theme in ["central", "central_dark"] {
                let html = themed_ui_html(template, theme);
                let utf16_bytes = html.encode_utf16().count() * 2;
                assert!(
                    utf16_bytes < 2 * 1024 * 1024,
                    "WebView2 HTML is too large: {utf16_bytes} UTF-16 bytes"
                );
                assert!(
                    !html.contains(SVELTE_UI_JS),
                    "Frontend must be a resource, not inlined into NavigateToString"
                );
            }
        }
    }

    #[test]
    fn ui_asset_protocol_serves_only_the_fixed_script() {
        let request = wry::http::Request::get("central-agent-ui://localhost/frontend.js")
            .body(Vec::new())
            .unwrap();
        let response = frontend_asset_response(None, request);
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["Cache-Control"], "no-store");
        assert_eq!(response.body().as_ref(), SVELTE_UI_JS.as_bytes());
        for url in [
            "central-agent-ui://localhost/../session.json",
            "central-agent-ui://localhost/frontend.js?path=C:/private",
            "central-agent-ui://other/frontend.js",
            "central-agent-ui://localhost/editor-drafts.json",
        ] {
            let request = wry::http::Request::get(url).body(Vec::new()).unwrap();
            assert_eq!(frontend_asset_response(None, request).status(), 404);
        }
        let request = wry::http::Request::post("central-agent-ui://localhost/frontend.js")
            .body(Vec::new())
            .unwrap();
        assert_eq!(frontend_asset_response(None, request).status(), 404);
    }

    #[test]
    fn parses_double_encoded_script_result() {
        let result =
            parse_script_bridge_result(r#""{\"ok\":false,\"error\":\"riferimento scaduto\"}""#)
                .unwrap();
        assert!(!result.ok);
        assert_eq!(result.error.as_deref(), Some("riferimento scaduto"));
    }

    #[test]
    fn application_chat_zoom_accelerators_are_scoped_and_layout_independent() {
        for key in [CHAT_ZOOM_VK_ADD, CHAT_ZOOM_VK_OEM_PLUS] {
            assert_eq!(
                chat_zoom_action_for_accelerator(true, true, false, key),
                Some(ChatZoomAction::Increase)
            );
        }
        for key in [CHAT_ZOOM_VK_SUBTRACT, CHAT_ZOOM_VK_OEM_MINUS] {
            assert_eq!(
                chat_zoom_action_for_accelerator(true, true, false, key),
                Some(ChatZoomAction::Decrease)
            );
        }
        for key in [CHAT_ZOOM_VK_ZERO, CHAT_ZOOM_VK_NUMPAD_ZERO] {
            assert_eq!(
                chat_zoom_action_for_accelerator(true, true, false, key),
                Some(ChatZoomAction::Reset)
            );
        }
        assert_eq!(
            chat_zoom_action_for_accelerator(true, false, false, CHAT_ZOOM_VK_OEM_PLUS),
            None
        );
        assert_eq!(
            chat_zoom_action_for_accelerator(true, true, true, CHAT_ZOOM_VK_OEM_PLUS),
            None
        );
        assert_eq!(
            chat_zoom_action_for_accelerator(false, true, false, CHAT_ZOOM_VK_OEM_PLUS),
            None
        );
    }

    #[test]
    fn injects_initial_theme_into_every_trusted_surface() {
        let backdrop = themed_ui_html(BACKDROP_HTML, "midnight");
        let toolbar = themed_ui_html(TOOLBAR_HTML, "midnight");
        let panel = terminal_panel_html("midnight");
        let preview = themed_ui_html(PREVIEW_HTML, "midnight");
        let agent_graph = themed_ui_html(AGENT_GRAPH_HTML, "midnight");
        assert!(backdrop.contains("data-theme=\"central\""));
        assert!(toolbar.contains("data-theme=\"central\""));
        assert!(panel.contains("data-theme=\"central\""));
        assert!(preview.contains("data-theme=\"central\""));
        assert!(agent_graph.contains("data-theme=\"central\""));
        assert!(agent_graph.contains("aria-label=\"Open project board\""));
        assert!(agent_graph.contains("data-graph-launcher-icon"));
        assert!(!agent_graph.contains("<span class=\"graph-mark\""));
        assert!(!agent_graph.contains("<h1>Agent graph</h1>"));
        assert!(!agent_graph_ui_contains("id=\"graph-summary\""));
        assert!(!agent_graph_ui_contains("id=\"close-graph\""));
        assert!(toolbar.contains("id=\"back-to-workspace\""));
        assert!(toolbar.contains("project-board-open"));
        assert!(!agent_graph.contains("__CENTRAL_AGENT_MARK_SVG__"));
        assert!(!toolbar.contains("__CENTRAL_AGENT_MARK_SVG__"));
        assert!(!panel.contains("__CENTRAL_AGENT_MARK_SVG__"));
        assert_eq!(toolbar.matches("class=\"brand-signal\"").count(), 0);
        assert_eq!(panel.matches("class=\"brand-signal\"").count(), 0);
        assert_eq!(agent_graph.matches("class=\"brand-signal\"").count(), 0);
        assert!(agent_graph.contains("data-central-agent-svelte=\"supervisor-logo\""));
        assert!(
            include_str!("../assets/supervisor-mark.svg").contains("data-supervisor-logo=\"true\"")
        );
        assert!(panel.contains("class=\"terminal-panel-mode\""));
        assert!(!backdrop.contains("__THEME_ID__"));
        assert!(!toolbar.contains("__THEME_ID__"));
        assert!(!preview.contains("__THEME_ID__"));
        assert!(!agent_graph.contains("__THEME_ID__"));
        assert!(!panel.contains("__FILE_ICONS_JS__"));
        assert!(!agent_graph.contains("__FILE_ICONS_JS__"));
        assert!(panel.contains("const centralFileIconData"));
        assert!(agent_graph.contains("const centralFileIconData"));

        let dark_toolbar = themed_ui_html(TOOLBAR_HTML, "central_dark");
        let dark_panel = terminal_panel_html("central_dark");
        let dark_backdrop = themed_ui_html(BACKDROP_HTML, "central_dark");
        let dark_preview = themed_ui_html(PREVIEW_HTML, "central_dark");
        let dark_agent_graph = themed_ui_html(AGENT_GRAPH_HTML, "central_dark");
        assert!(dark_toolbar.contains("data-theme=\"central_dark\""));
        assert!(dark_panel.contains("data-theme=\"central_dark\""));
        assert!(dark_backdrop.contains("data-theme=\"central_dark\""));
        assert!(dark_preview.contains("data-theme=\"central_dark\""));
        assert!(dark_agent_graph.contains("data-theme=\"central_dark\""));
    }

    #[cfg(windows)]
    #[test]
    fn native_caption_palette_matches_the_monochrome_ui_modes() {
        let light = native_caption_palette("central");
        assert_eq!(light.caption, color_ref(238, 238, 236));
        assert_eq!(light.text, color_ref(23, 23, 23));
        assert_eq!(light.border, color_ref(238, 238, 236));
        assert!(!light.dark);

        let dark = native_caption_palette("central_dark");
        assert_eq!(dark.caption, color_ref(13, 13, 13));
        assert_eq!(dark.text, color_ref(241, 241, 239));
        assert_eq!(dark.border, color_ref(13, 13, 13));
        assert!(dark.dark);
    }

    #[test]
    fn preview_surface_exposes_only_bounded_preview_controls() {
        assert!(PREVIEW_HTML.contains("send(\"set_preview_live\""));
        assert!(PREVIEW_HTML.contains("send(\"attach_preview\""));
        assert!(PREVIEW_HTML.contains("send(\"close_preview\""));
        assert!(PREVIEW_HTML.contains("data:image/jpeg;base64,"));
        assert!(!PREVIEW_HTML.contains("with_url"));
    }

    #[test]
    fn web_preview_association_uses_origins_not_paths() {
        assert_eq!(
            url_origin("http://127.0.0.1:5173/app?x=1").as_deref(),
            Some("http://127.0.0.1:5173")
        );
        assert_eq!(
            url_origin("http://127.0.0.1:5173/other").as_deref(),
            Some("http://127.0.0.1:5173")
        );
        assert_ne!(
            url_origin("http://127.0.0.1:5173/"),
            url_origin("http://127.0.0.1:5174/")
        );
        assert!(url_origin("file:///tmp/index.html").is_none());
    }

    #[test]
    fn closing_a_preview_invalidates_in_flight_capture_generations() {
        let target = crate::window_runtime::WindowCaptureTarget {
            window_id: "window-1-1".to_owned(),
            title: "Preview".to_owned(),
            pid: 42,
            owned: true,
            native_token: 123,
        };
        let mut preview = PreviewRuntime::default();
        let active_generation =
            preview.begin(target, Some(7), NativePreviewSource::NativeWindow, true);
        preview.close();
        assert_ne!(active_generation, preview.generation);
        assert!(!preview.visible);
        assert!(!preview.live);
        assert!(preview.target.is_none());
    }

    #[test]
    fn toolbar_keeps_new_tab_in_the_tab_flow_and_settings_at_the_edge() {
        let tabs = TOOLBAR_HTML.find("<div id=\"tabs\"").unwrap();
        let new_tab = TOOLBAR_HTML.find("id=\"new-tab\"").unwrap();
        let settings = TOOLBAR_HTML.find("id=\"open-settings\"").unwrap();
        assert!(tabs < new_tab && new_tab < settings);
        assert!(
            TOOLBAR_HTML
                .contains("tabsElement.replaceChildren(...state.tabs.map(makeTab), newTabButton);")
        );
        assert!(TOOLBAR_HTML.contains("send(\"open_settings\")"));
        assert!(TOOLBAR_HTML.contains("window.centralAgentAwaitPanelCloseLayout"));
        assert!(TOOLBAR_HTML.contains("send(\"agent_panel_close_layout_ready\")"));
        assert!(TOOLBAR_HTML.contains("tab-transition-ghost"));
        assert!(TOOLBAR_HTML.contains("captureTabSplitOrigin"));
        assert!(TOOLBAR_HTML.contains("animateTabSplit(splitOrigin, addedTab.id)"));
        assert!(TOOLBAR_HTML.contains("captureTabMergeOrigin"));
        assert!(TOOLBAR_HTML.contains("animateTabMerge(mergeOrigin)"));
        assert!(TOOLBAR_HTML.contains("animateTabReflow(reflowBounds)"));
        assert!(TOOLBAR_HTML.contains("state.browserTabsVisible !== false"));
        assert!(TOOLBAR_HTML.contains("body.browser-tabs-hidden .tab-strip"));
        assert!(TOOLBAR_HTML.contains("stroke-width: 2.4"));
        assert!(TOOLBAR_HTML.contains("border-radius: 12px !important"));
        assert!(TOOLBAR_HTML.contains("background: transparent"));
        assert!(!TOOLBAR_HTML.contains("id=\"security\""));

        let command: ToolbarCommand = serde_json::from_str(r#"{"type":"open_settings"}"#).unwrap();
        assert!(matches!(command, ToolbarCommand::OpenSettings));

        let command: ToolbarCommand =
            serde_json::from_str(r#"{"type":"agent_panel_close_layout_ready"}"#).unwrap();
        assert!(matches!(
            command,
            ToolbarCommand::AgentPanelCloseLayoutReady
        ));
    }

    #[test]
    fn closing_the_agent_panel_repositions_the_fixed_graph_launcher() {
        let source = include_str!("browser.rs");
        let close_transition = source
            .split_once("fn begin_agent_panel_close(&mut self)")
            .and_then(|(_, rest)| rest.split_once("fn commit_agent_panel_close"))
            .map(|(transition, _)| transition)
            .expect("agent panel close transition must remain available");

        assert!(close_transition.contains("self.layout_tab_webviews();"));
        assert!(close_transition.contains("self.layout_agent_graph_surface();"));
        assert!(
            close_transition.find("self.layout_agent_graph_surface();")
                < close_transition.find("self.render_toolbar();")
        );
    }

    #[test]
    fn opening_settings_projects_the_collapsed_graph_before_layout_and_settings() {
        let source = include_str!("browser.rs");
        let transition = source
            .split_once("fn open_settings(&mut self)")
            .and_then(|(_, rest)| rest.split_once("fn settings_surface_ready"))
            .map(|(transition, _)| transition)
            .expect("settings open transition must remain available");
        let position = |statement| {
            transition
                .find(statement)
                .unwrap_or_else(|| panic!("settings transition must include {statement}"))
        };

        assert!(
            position("self.agent_graph_open = false;")
                < position("self.render_agent_graph_surface();")
        );
        assert!(
            position("self.render_agent_graph_surface();")
                < position("self.layout_agent_graph_surface();")
        );
        assert!(
            position("self.layout_agent_graph_surface();") < position("self.render_agent_panel();")
        );
    }

    #[test]
    fn settings_are_a_dedicated_categorized_menu() {
        assert!(!AGENT_PANEL_HTML.contains("id=\"settings-toggle\""));
        assert!(!AGENT_PANEL_HTML.contains("id=\"theme-select\""));
        assert!(AGENT_PANEL_HTML.contains("data-theme-choice=\"central\""));
        assert!(AGENT_PANEL_HTML.contains("data-theme-choice=\"central_dark\""));
        assert!(agent_ui_contains("class=\"settings-shell\""));
        assert!(agent_ui_contains("class=\"settings-topbar\""));
        assert!(agent_ui_contains("class=\"settings-main\""));
        assert!(!agent_ui_contains("id=\"settings-search\""));
        assert!(!SVELTE_SETTINGS_SHELL_SOURCE.contains("searchSettings"));
        assert!(SVELTE_SETTINGS_SOURCE.contains("data-settings-nav-group"));
        assert!(SVELTE_SETTINGS_SOURCE.contains("class=\"settings-primary-tabs\""));
        assert!(SVELTE_SETTINGS_SOURCE.contains("class=\"settings-secondary-tabs\""));
        assert!(AGENT_PANEL_HTML.contains("body.settings-open .panel > header"));
        assert!(agent_ui_contains("id=\"settings-nav\""));
        assert!(AGENT_PANEL_HTML.contains("window.centralAgentOpenSettings"));
        assert!(AGENT_PANEL_HTML.contains("window.centralAgentCommitSettings"));
        assert!(AGENT_PANEL_HTML.contains("window.centralAgentAwaitSettingsCloseCommit"));
        assert!(AGENT_PANEL_HTML.contains("window.centralAgentCommitSettingsClose"));
        assert!(AGENT_PANEL_HTML.contains("sendControl(\"settings_surface_ready\")"));
        assert!(AGENT_PANEL_HTML.contains("sendControl(\"settings_cover_ready\")"));
        assert!(AGENT_PANEL_HTML.contains("sendControl(\"settings_close_layout_ready\")"));
        assert!(AGENT_PANEL_HTML.contains("settings-preparing"));
        assert!(AGENT_PANEL_HTML.contains("settings-closing"));
        assert!(AGENT_PANEL_HTML.contains("sendControl(\"close_settings\")"));
    }

    #[test]
    fn docked_settings_cover_the_browser_toolbar_and_full_window() {
        let source = include_str!("browser.rs");
        let layout = source
            .split_once("fn layout_webviews(&self)")
            .and_then(|(_, rest)| rest.split_once("fn layout_preview_webview"))
            .map(|(body, _)| body)
            .expect("webview layout must remain discoverable");
        let bounds = source
            .split_once("fn settings_panel_bounds(window: &Window)")
            .and_then(|(_, rest)| rest.split_once("#[derive(Clone, Copy)]"))
            .map(|(body, _)| body)
            .expect("settings bounds must remain discoverable");

        assert!(layout.contains("let visible = !self.settings_covering_main();"));
        assert!(layout.contains("toolbar.set_visible(visible)"));
        assert!(layout.contains("settings_panel_bounds(window)"));
        assert!(bounds.contains("full_window_bounds(window)"));
    }

    #[test]
    fn time_machine_is_available_in_chat_and_agent_graph_history() {
        for surface in [AGENT_PANEL_HTML, AGENT_GRAPH_HTML] {
            assert!(surface.contains("createCheckpointCard"));
            assert!(surface.contains("createTimeMachineTree"));
            assert!(surface.contains("open_checkpoint"));
            assert!(surface.contains("select_checkpoint_file"));
            assert!(surface.contains("restore_checkpoint_file"));
            assert!(surface.contains("restore_checkpoint"));
            assert!(surface.contains("ca-file-icon type-${key}"));
            assert!(surface.contains("populateCentralFileIcon(icon, key"));
            assert!(surface.contains("function centralFolderIcon("));
        }
        assert!(FILE_ICONS_JS.contains("\"rust\": Object.freeze"));
        assert!(FILE_ICONS_JS.contains("\"title\":\"Rust\""));
        assert!(FILE_ICONS_JS.contains("const centralFileIconAliases"));
        assert!(FILE_ICONS_JS.contains("cargo: \"rust\""));
        assert!(FILE_ICONS_JS.contains("const centralFileIconTones = Object.freeze"));
        assert!(FILE_ICONS_JS.contains("rust: \"#d76535\""));
        assert!(FILE_ICONS_JS.contains("typescript: \"#3178c6\""));
        assert!(FILE_ICONS_JS.contains("function centralFileIconElement("));
        assert!(FILE_ICONS_JS.contains("function drawCentralFileBrand("));
        assert!(THEMES_CSS.contains(".ca-file-icon.has-brand"));
        assert!(!THEMES_CSS.contains("--ca-file-tone: var(--ca-icon) !important"));
        assert!(THEMES_CSS.contains("color: var(--ca-file-tone) !important"));
        assert!(THEMES_CSS.contains(".ca-file-icon:is(.type-rust, .type-cargo)"));
        assert!(THEMES_CSS.contains(".ca-file-icon.type-docker"));
        assert!(THEMES_CSS.contains(".ca-file-icon.type-npm"));
        assert!(THEMES_CSS.contains("--ca-diff-addition: #16865a"));
        assert!(THEMES_CSS.contains("--ca-diff-deletion: #c74747"));
        assert!(AGENT_PANEL_HTML.contains("Borderless reasoning and semantic Time Machine diffs"));
        assert!(AGENT_PANEL_HTML.contains(".chat-message.system.has-checkpoint"));
        assert!(
            AGENT_PANEL_HTML.contains(".checkpoint-stats { font-size: var(--ca-type-label); }")
        );
        assert!(AGENT_GRAPH_HTML.contains(".checkpoint-card"));
        assert!(
            AGENT_GRAPH_HTML.contains(".checkpoint-stats { font-size: var(--ca-type-label); }")
        );
        assert!(AGENT_PANEL_HTML.contains("message.checkpoint"));
        assert!(agent_graph_ui_contains("turn.checkpoint"));
    }

    #[test]
    fn time_machine_checkpoint_finishes_before_presentation_and_actual_exit() {
        let source = include_str!("browser.rs");
        let quiescent = source
            .split_once("fn try_finish_run_when_quiescent")
            .and_then(|(_, rest)| {
                rest.split_once("fn complete_run_presentation")
                    .map(|(body, _)| body)
            })
            .expect("quiescent finalization must remain discoverable");
        let completion = source
            .split_once("fn complete_time_machine_checkpoint")
            .and_then(|(_, rest)| {
                rest.split_once("fn restore_time_machine_checkpoint")
                    .map(|(body, _)| body)
            })
            .expect("checkpoint completion must remain discoverable");
        let close_request = source
            .split_once("fn request_main_window_close")
            .and_then(|(_, rest)| {
                rest.split_once("fn handle_system_tray_action")
                    .map(|(body, _)| body)
            })
            .expect("main window close request must remain discoverable");
        let close_handler = source
            .split_once("if self.window.as_ref().map(Window::id) == Some(window_id)")
            .and_then(|(_, rest)| {
                rest.split_once("if self.agent_panel_window.as_ref().map(Window::id)")
                    .map(|(body, _)| body)
            })
            .expect("main window close handler must remain discoverable");

        assert!(quiescent.contains("run.phase = AgentPhase::Finalizing;"));
        assert!(quiescent.contains("Finalizing Time Machine checkpoint…"));
        assert!(!quiescent.contains("self.snapshot_agent_graph_turn(run_id);"));
        assert!(completion.contains("should_publish_checkpoint_summary(&summary)"));
        assert!(!completion.contains("ingest_path"));
        assert!(!completion.contains("attach_artifacts_to_run"));
        assert!(completion.contains("self.complete_run_presentation(run_id);"));
        assert!(close_request.contains("self.system_tray_available()"));
        assert!(close_request.contains("self.hide_application_windows();"));
        assert!(close_request.contains("if !self.time_machine_finalizations_in_flight.is_empty()"));
        assert!(close_request.contains("return;"));
        assert!(close_handler.contains("self.request_main_window_close(event_loop)"));
    }

    #[test]
    fn time_machine_recovery_keeps_the_original_submission_until_retry_succeeds() {
        let selection = AgentSelection {
            model: "gpt-5.6-luna".to_owned(),
            effort: "high".to_owned(),
            service_tier: Some("priority".to_owned()),
            personality: None,
            context_window: None,
        };
        let mut pending = VecDeque::from([PendingAgentSubmission {
            delivery_trace: None,
            native_skills: vec![],
            native_apps: vec![],
            scope: Some(submission_scope::SubmissionScope {
                native_access: Default::default(),
                native_summary: None,
                owner: "chat:original".to_owned(),

                workspace_root: None,
                native_targets: native_targets::NativeTargets::default(),
            }),

            snapshots: SubmissionSnapshots::default(),
            provider: AgentProviderKind::ClaudeCode,
            message: "preserve this request".to_owned(),
            tab_ids: vec![7],
            terminal_session_ids: vec![9],
            file_ids: vec!["attachment".to_owned()],
            selection_override: Some(selection.clone()),
            agent_graph_launch: None,
            card_draft: false,
            supervision_review: None,
        }]);

        assert!(submission_scope::next_ready_index(&pending, |_| false).is_none());
        assert!(
            !pending.is_empty(),
            "a failed retry must retain the request"
        );

        let index = submission_scope::next_ready_index(&pending, |_| true)
            .expect("successful recovery resumes the queued request");
        let resumed = pending.remove(index).unwrap();
        assert_eq!(resumed.provider, AgentProviderKind::ClaudeCode);
        assert_eq!(resumed.selection_override, Some(selection));
        assert_eq!(resumed.message, "preserve this request");
        assert_eq!(resumed.tab_ids, vec![7]);
        assert!(pending.is_empty(), "the request must resume exactly once");
        assert!(submission_scope::next_ready_index(&pending, |_| true).is_none());
    }

    #[test]
    fn startup_indexes_unfinished_time_machine_runs_before_allocating_new_ids() {
        let first = PathBuf::from(r"C:\workspace\first");
        let second = PathBuf::from(r"C:\workspace\second");
        let (roots, retryable, next_run_id) =
            index_unfinished_time_machine_runs(vec![(4, first.clone()), (11, second.clone())]);

        assert_eq!(roots.get(&4), Some(&first));
        assert_eq!(roots.get(&11), Some(&second));
        assert_eq!(retryable, HashSet::from([4, 11]));
        assert_eq!(next_run_id, 12);
    }

    #[test]
    fn recovered_interrupted_graph_run_reattaches_an_openable_checkpoint_card() {
        let store = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let store_root = store.path().join("time-machine");
        let source = workspace.path().join("main.rs");
        fs::write(&source, "before\n").unwrap();
        let mut runtime = TimeMachineRuntime::open(store_root.clone());
        runtime
            .begin_run(27, workspace.path(), "Agent", "Agent Graph agent")
            .unwrap();
        fs::write(&source, "after\n").unwrap();
        drop(runtime);

        let mut reopened = TimeMachineRuntime::open(store_root);
        let mut recovered = reopened.take_recovered_summaries();
        assert_eq!(recovered.len(), 1);
        let checkpoint_id = recovered[0].id.clone();
        let mut orphaned_main_summary = recovered[0].clone();
        orphaned_main_summary.run_id = 28;
        recovered.push(orphaned_main_summary);
        let mut sessions = vec![AgentGraphSession {
            node_key: "entity:project".to_owned(),
            record_type: "entity".to_owned(),
            record_id: "project".to_owned(),
            agent_name: "Project agent".to_owned(),
            project_directory: workspace.path().display().to_string(),
            provider: AgentProviderKind::ClaudeCode,
            selection: AgentSelection {
                model: "gpt-5.6-sol".to_owned(),
                effort: "high".to_owned(),
                service_tier: None,
                personality: None,
                context_window: None,
            },
            turns: vec![AgentGraphTurn {
                run_id: 27,
                request: "Edit the project".to_owned(),
                provider: AgentProviderKind::ClaudeCode,
                selection: AgentSelection::default(),
                started_at_ms: 1,
                finished_at_ms: Some(2),
                phase: AgentPhase::Stopped,
                status: "Interrupted".to_owned(),
                messages: vec![AgentGraphHistoryMessage {
                    native: None,
                    id: 88,
                    role: ChatRole::Assistant,
                    kind: ChatMessageKind::Message,
                    text: "Interrupted output".to_owned(),
                    timestamp_ms: 1,
                    artifacts: Vec::new(),
                }],
                steps: Vec::new(),
                checkpoint: None,
            }],
            updated_at_ms: 1,
        }];
        let mut next_message_id = next_agent_graph_history_message_id(&sessions);
        let (notices, history_changed) =
            attach_recovered_checkpoint_summaries(&mut sessions, recovered, &mut next_message_id);

        assert!(history_changed);
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].id, 89);
        assert_eq!(notices[0].run_id, Some(28));
        assert_eq!(next_message_id, 90);
        assert_eq!(
            sessions[0].turns[0]
                .checkpoint
                .as_ref()
                .map(|checkpoint| checkpoint.id.as_str()),
            Some(checkpoint_id.as_str())
        );
        reopened.open_checkpoint(&checkpoint_id, None).unwrap();
        assert_eq!(
            reopened
                .view()
                .selected
                .as_ref()
                .map(|selected| selected.checkpoint.id.as_str()),
            Some(checkpoint_id.as_str())
        );
    }

    #[test]
    fn zero_line_checkpoint_diff_is_not_published_to_chat() {
        let summary = CheckpointSummary {
            id: "checkpoint".to_owned(),
            run_id: 31,
            workspace_name: "Workspace".to_owned(),
            provider: "Agent".to_owned(),
            context: "Agent chat".to_owned(),
            created_at_ms: 1,
            finished_at_ms: 2,
            file_count: 1,
            total_additions: 0,
            total_deletions: 0,
            warning_count: 0,
            restored_files: 0,
            fully_restored: false,
            changes: Vec::new(),
        };
        let mut next_message_id = 1;
        let (notices, history_changed) =
            attach_recovered_checkpoint_summaries(&mut [], vec![summary], &mut next_message_id);

        assert!(!history_changed);
        assert!(notices.is_empty());
        assert_eq!(next_message_id, 1);
    }

    #[test]
    fn queued_time_machine_recovery_is_visible_and_cancellable_on_both_surfaces() {
        for contract in [
            "id=\"queued-agent-request\"",
            "id=\"cancel-queued-agent-request\"",
            "id=\"discard-failed-checkpoint\"",
        ] {
            assert!(agent_ui_contains(contract));
            assert!(agent_graph_ui_contains(contract));
        }
        for contract in [
            "cancel_pending_agent_submission",
            "discard_failed_checkpoint",
        ] {
            assert!(AGENT_PANEL_HTML.contains(contract));
            assert!(AGENT_GRAPH_HTML.contains(contract));
        }
    }

    #[test]
    fn remote_graph_nodes_require_a_provider_with_supervisor_ssh_tools() {
        let error =
            validate_agent_graph_provider_location(AgentProviderKind::CodexAppServer, Some("vps"))
                .unwrap_err();
        assert!(error.contains("requires a local directory"));
        assert!(
            validate_agent_graph_provider_location(AgentProviderKind::ClaudeCode, Some("vps"))
                .is_ok()
        );
        assert!(
            validate_agent_graph_provider_location(AgentProviderKind::ClaudeCode, None).is_ok()
        );
    }

    #[test]
    fn local_artifacts_are_rendered_and_actionable_on_both_agent_surfaces() {
        assert!(AGENT_PANEL_HTML.contains("createArtifactCollection"));
        assert!(AGENT_PANEL_HTML.contains("openArtifactViewer"));
        assert!(AGENT_PANEL_HTML.contains("highlightCodeElement"));
        assert!(AGENT_PANEL_HTML.contains("renderDiagram"));
        assert!(AGENT_GRAPH_HTML.contains("agentGraphArtifactList"));
        assert!(AGENT_GRAPH_HTML.contains("openAgentGraphArtifact"));
        assert!(AGENT_GRAPH_HTML.contains("highlightConsoleCode"));
        for surface in [AGENT_PANEL_HTML, AGENT_GRAPH_HTML] {
            assert!(surface.contains("open_artifact"));
            assert!(surface.contains("save_artifact"));
            assert!(surface.contains("show_artifact_in_folder"));
            assert!(surface.contains("sandbox\", \"allow-scripts"));
            assert!(surface.contains("connect-src 'none'"));
            assert!(surface.contains("requestFullscreen"));
        }
    }

    #[test]
    fn agent_runtime_summary_is_not_rendered_below_the_composer() {
        assert!(!AGENT_PANEL_HTML.contains("id=\"runtime-section\""));
        assert!(!AGENT_PANEL_HTML.contains(">Agent runtime<"));
        assert!(!AGENT_PANEL_HTML.contains("id=\"phase-badge\""));
        assert!(!AGENT_PANEL_HTML.contains("id=\"plan-list\""));
        assert!(!AGENT_PANEL_HTML.contains("id=\"process-section\""));
        assert!(!AGENT_PANEL_HTML.contains(">Managed tasks<"));
        assert!(
            !AGENT_PANEL_HTML
                .to_ascii_lowercase()
                .contains("managed tasks")
        );
        assert!(!AGENT_PANEL_HTML.contains("function renderProcesses("));
        assert!(!agent_ui_contains("id=\"control-indicator\""));
        assert!(!agent_ui_contains("Computer control ready"));
    }

    #[test]
    fn agent_panel_uses_a_coding_workspace_layout() {
        assert!(AGENT_PANEL_HTML.contains("id=\"agent-panel-resizer\""));
        assert!(AGENT_PANEL_HTML.contains("set_agent_panel_width"));
        assert!(AGENT_PANEL_HTML.contains("id=\"workspace-explorer\""));
        assert!(AGENT_PANEL_HTML.contains("id=\"workspace-explorer-resizer\""));
        assert!(agent_ui_contains("id=\"workspace-explorer-root-icon\""));
        assert!(AGENT_PANEL_HTML.contains("WORKSPACE_EXPLORER_WIDTH_KEY"));
        assert!(AGENT_PANEL_HTML.contains("class=\"conversation-section\""));
        assert!(AGENT_PANEL_HTML.contains("grid-template-rows: minmax(0, 1fr)"));
        assert!(AGENT_PANEL_HTML.contains(
            ".conversation-section { display: grid; width: 100%; height: auto; min-height: 0; align-self: stretch; grid-column: 3; grid-row: 1;"
        ));
        assert!(agent_ui_contains("id=\"conversation-workspace\""));
        assert!(AGENT_PANEL_HTML.contains("id=\"agent-panel-popout\""));
        assert!(AGENT_PANEL_HTML.contains("detach_agent_panel"));
        assert!(AGENT_PANEL_HTML.contains("attach_agent_panel"));
        assert!(AGENT_PANEL_HTML.contains("renderWorkspaceExplorer"));
        assert!(AGENT_PANEL_HTML.contains("function workspaceControlIcon(kind)"));
        assert!(AGENT_PANEL_HTML.contains("action.append(workspaceControlIcon(\"plus\"))"));
        assert!(AGENT_PANEL_HTML.contains("button.append(workspaceControlIcon(\"menu\"))"));
        assert!(!AGENT_PANEL_HTML.contains(".workspace-row-menu { padding-bottom: 5px;"));
        assert!(AGENT_PANEL_HTML.contains("refresh_workspace_explorer"));
        assert!(AGENT_PANEL_HTML.contains("double-click to reference in chat"));
        assert!(agent_ui_contains("id=\"workspace-explorer-add\""));
        assert!(agent_ui_contains("id=\"workspace-explorer-new-file\""));
        assert!(agent_ui_contains("id=\"workspace-explorer-new-folder\""));
        assert!(agent_ui_contains("id=\"workspace-explorer-import\""));
        assert!(agent_ui_contains("id=\"workspace-explorer-create\""));
        assert!(agent_ui_contains("id=\"workspace-explorer-projects\""));
        assert!(agent_ui_contains("id=\"workspace-remove-project-dialog\""));
        assert!(agent_ui_contains("id=\"workspace-remove-project-confirm\""));
        assert!(agent_ui_contains("id=\"workspace-remove-project-warning\""));
        assert!(agent_ui_contains("id=\"workspace-delete-chat-dialog\""));
        assert!(agent_ui_contains("id=\"workspace-delete-chat-confirm\""));
        assert!(agent_ui_contains("id=\"workspace-move-chat-dialog\""));
        assert!(agent_ui_contains("id=\"workspace-move-chat-projects\""));
        assert!(agent_ui_contains("id=\"workspace-move-chat-browse\""));
        assert!(AGENT_PANEL_HTML.contains("Stop agent and remove project?"));
        assert!(AGENT_PANEL_HTML.contains("stop_active_runs"));
        assert!(AGENT_PANEL_HTML.contains("Waiting for checkpoint"));
        assert!(AGENT_PANEL_HTML.contains(
            ".panel, body.settings-closing .panel { grid-template-rows: minmax(0, 1fr); }"
        ));
        assert!(
            AGENT_PANEL_HTML.contains("background: transparent !important; pointer-events: none;")
        );
        assert!(AGENT_PANEL_HTML.contains("openWorkspaceRemoveProjectDialog"));
        assert!(AGENT_PANEL_HTML.contains("openWorkspaceDeleteChatDialog"));
        assert!(AGENT_PANEL_HTML.contains("openWorkspaceMoveChatDialog"));
        assert!(AGENT_PANEL_HTML.contains("move_project_chat"));
        assert!(AGENT_PANEL_HTML.contains("select_project_for_chat"));
        assert!(!AGENT_PANEL_HTML.contains("window.confirm(`Remove ${projectName}"));
        assert!(!AGENT_PANEL_HTML.contains("window.confirm(`Delete “${chat.title"));
        assert!(AGENT_PANEL_HTML.contains("activate_workspace"));
        assert!(AGENT_PANEL_HTML.contains("create_workspace_file"));
        assert!(AGENT_PANEL_HTML.contains("create_workspace_directory"));
        assert!(AGENT_PANEL_HTML.contains("import_workspace_files"));
        assert!(!AGENT_PANEL_HTML.contains("createWorkspaceSectionLabel(\"Projects\")"));
        assert!(AGENT_PANEL_HTML.contains("createWorkspaceSectionLabel(\"Results\")"));
        assert!(
            AGENT_PANEL_HTML.contains(".workspace-project-chats { gap: 1px; padding: 0 0 7px; }")
        );
        assert!(
            AGENT_PANEL_HTML
                .replace("\r\n", "\n")
                .contains(".workspace-chat-row.active {\n        min-height: 34px;")
        );
        assert!(AGENT_PANEL_HTML.contains("createSupervisorMark()"));
        assert!(!AGENT_PANEL_HTML.contains("createWorkspaceTetrahedron"));
        assert!(!AGENT_PANEL_HTML.contains("data-tetra-piece"));
        assert!(!AGENT_PANEL_HTML.contains("tetrahedron-in-flight"));
        assert!(AGENT_PANEL_HTML.contains("updateWorkspaceActivity(state)"));
        assert!(AGENT_PANEL_HTML.contains("left: -68px; top: 50%; bottom: auto;"));
        assert!(AGENT_PANEL_HTML.contains("transform: translateY(-50%);"));
        assert!(AGENT_PANEL_HTML.contains(
            "border: 1px solid var(--ca-border); border-radius: var(--ca-radius-large);"
        ));
        assert!(AGENT_PANEL_HTML.contains(
            ".tetra-config-button[aria-expanded=\"true\"] { background: transparent; color: var(--ca-text); }"
        ));
        assert!(AGENT_PANEL_HTML.contains("function displayLocalPath(value)"));
        assert!(
            AGENT_PANEL_HTML
                .replace("\r\n", "\n")
                .contains("closeWorkspaceContextMenu();\n        closeAllProfilePickers();")
        );
        assert!(THEMES_CSS.contains("--ca-display-refresh-hz: 60;"));
        assert!(THEMES_CSS.contains("--ca-display-frame-duration: 16.667ms;"));
        assert!(AGENT_PANEL_HTML.contains("function calibrateDisplayRefresh()"));
        assert!(AGENT_PANEL_HTML.contains("root.dataset.displayRefreshHz = String(normalizedHz)"));
        assert!(AGENT_PANEL_HTML.contains("box-shadow: none !important; resize: none;"));
        assert!(!agent_ui_contains("id=\"provider-model-picker-button\""));
        assert!(!agent_ui_contains("id=\"execution-picker-button\""));
        assert!(agent_ui_contains("id=\"tetra-config-button\""));
        assert!(agent_ui_contains("data-compound=\"tetra-configuration\""));
        assert!(SVELTE_COMPOSER_SOURCE.contains("<SupervisorLogo />"));
        assert!(AGENT_PANEL_HTML.contains("const workingProjectRoots = new Set(projects"));
        assert!(
            AGENT_PANEL_HTML.contains("workingProjectRoots.has(String(chat.projectRoot || \"\"))")
        );
        assert!(AGENT_PANEL_HTML.contains("path.includes(picker.host)"));
        assert!(AGENT_PANEL_HTML.contains("setAgentConfigurationOpen(false, immediate)"));
        assert!(AGENT_PANEL_HTML.contains("setAgentConfigurationOpen(true)"));
        assert!(
            !AGENT_PANEL_HTML.contains("tetraConfig.selectedParts.size === codexPickers.length")
        );
        assert!(agent_ui_contains("label=\"Provider\""));
        assert!(agent_ui_contains("label=\"Model\""));
        assert!(agent_ui_contains("label=\"Effort\""));
        assert!(agent_ui_contains("label=\"Speed\""));
        assert!(!SVELTE_CONFIGURATION_SOURCE.contains("label=\"Context window\""));
        assert!(
            include_str!("../ui/src/components/MainAgentSettings.svelte")
                .contains("label=\"Context window\"")
        );
        assert!(agent_ui_contains("class=\"attach-files-plus\""));
        assert!(!agent_ui_contains("id=\"send-chat\""));
        assert!(!agent_ui_contains("Run with Agent"));
        assert!(!AGENT_PANEL_HTML.contains("files · ${folders} folders"));
        assert_eq!(AGENT_PANEL_WIDTH_FULL_HD_LOGICAL, 800.0);
        assert_eq!(AGENT_PANEL_WIDTH_2K_LOGICAL, 1_000.0);
        assert_eq!(AGENT_PANEL_WIDTH_4K_LOGICAL, 1_280.0);
    }

    #[test]
    fn tab_attachment_cards_show_a_thumbnail_instead_of_page_details() {
        assert!(AGENT_PANEL_HTML.contains("createPageThumbnail(context.previewDataUrl"));
        assert!(AGENT_PANEL_HTML.contains("createPageThumbnail(attachment.previewDataUrl"));
        assert!(AGENT_PANEL_HTML.contains("className = \"page-thumbnail-placeholder\""));
        assert!(!AGENT_PANEL_HTML.contains("attachment.textPreview"));
        assert!(!AGENT_PANEL_HTML.contains("context.textPreview"));
        assert!(!AGENT_PANEL_HTML.contains("context.elements?.length"));
        assert!(!AGENT_PANEL_HTML.contains("attachment.textCharCount"));
    }

    #[test]
    fn browser_tabs_can_be_dragged_into_the_chat_composer() {
        assert!(TOOLBAR_HTML.contains("element.draggable = true"));
        assert!(TOOLBAR_HTML.contains("application/x-central-agent-tab"));
        assert!(TOOLBAR_HTML.contains("central-agent-tab:${tab.id}"));
        assert!(AGENT_PANEL_HTML.contains("function draggedTabId(transfer)"));
        assert!(SVELTE_COMPOSER_SOURCE.contains("ondrop={handleDrop}"));
        assert!(SVELTE_COMPOSER_SOURCE.contains("central-agent:drop-context"));
        assert!(AGENT_PANEL_HTML.contains("central-agent:drop-context"));
        assert!(AGENT_PANEL_HTML.contains("addTabAttachment(tabId)"));

        // Wry replaces WebView2's drop target when a native drag handler is installed on
        // Windows, disabling the HTML API used by the cross-WebView tab protocol.
        let source = include_str!("browser.rs");
        let agent_builder = source
            .split_once("fn build_agent_panel")
            .and_then(|(_, rest)| rest.split_once("fn build_terminal_panel"))
            .map(|(builder, _)| builder)
            .expect("agent panel builder must remain discoverable");
        assert!(!agent_builder.contains(".with_drag_drop_handler"));
    }

    #[test]
    fn terminal_tabs_can_attach_bounded_follow_live_context() {
        assert!(AGENT_PANEL_HTML.contains("application/x-central-agent-terminal-context"));
        assert!(AGENT_PANEL_HTML.contains("central-agent-shell:${session.id}"));
        assert!(AGENT_PANEL_HTML.contains("capture_terminal_context"));
        assert!(AGENT_PANEL_HTML.contains("set_terminal_context_follow"));
        assert!(AGENT_PANEL_HTML.contains("Follow live"));
        assert!(AGENT_PANEL_HTML.contains("terminal_session_ids"));
        assert!(AGENT_PANEL_HTML.contains("window.updateTerminalContext"));
    }

    #[test]
    fn composer_remains_editable_and_routes_prompts_during_an_active_run() {
        assert!(SVELTE_COMPOSER_SOURCE.contains("maxlength=\"10000\""));
        assert!(SVELTE_COMPOSER_SOURCE.contains("oninput={handleInput}"));
        assert!(SVELTE_COMPOSER_SOURCE.contains("import { mainDrafts as drafts }"));
        let persistence_source = include_str!("../ui/src/lib/persisted-drafts.ts");
        assert!(persistence_source.contains("central-agent:composer-draft-result"));
        assert!(persistence_source.contains("central-agent:composer-draft-forgotten"));
        assert!(persistence_source.contains("central-agent:composer-draft-v1"));
        assert!(
            SVELTE_COMPOSER_SOURCE.contains("drafts.write(conversationKey, inputElement.value)")
        );
        assert!(SVELTE_COMPOSER_SOURCE.contains("drafts.read(conversationKey)"));
        let draft_source = include_str!("../ui/src/lib/conversation-drafts.ts");
        assert!(draft_source.contains("this.storage().setItem"));
        assert!(draft_source.contains("this.storage().getItem"));
        assert!(SVELTE_COMPOSER_SOURCE.contains("id=\"agent-delivery-now\""));
        assert!(SVELTE_COMPOSER_SOURCE.contains("id=\"agent-delivery-queue\""));
        assert!(SVELTE_COMPOSER_SOURCE.contains("let deliveryPending = $state(false)"));
        assert!(SVELTE_COMPOSER_SOURCE.contains("if (agentActive)"));
        assert!(SVELTE_COMPOSER_SOURCE.contains("deliveryPending = true"));
        assert!(SVELTE_COMPOSER_SOURCE.contains("deliverActiveRequest(\"steer\")"));
        assert!(SVELTE_COMPOSER_SOURCE.contains("deliverActiveRequest(\"queue\")"));
        assert!(SVELTE_COMPOSER_SOURCE.contains("hidden={!deliveryPending}"));
        assert!(SVELTE_COMPOSER_SOURCE.contains("</div>\n\n<div\n  id=\"agent-delivery-options\""));
        assert!(!AGENT_PANEL_HTML.contains("agentDeliveryOptions.hidden = !agentIsActive"));
        assert!(AGENT_PANEL_HTML.contains("central-agent:agent-state"));
        assert!(AGENT_PANEL_HTML.contains(".conversation-section > .agent-delivery-options"));
        assert!(SVELTE_COMPOSER_SOURCE.contains("central-agent:clear-prompt-queue"));
        assert!(SVELTE_COMPOSER_SOURCE.contains("id=\"send-agent\""));
        assert!(SVELTE_COMPOSER_SOURCE.contains("data-work-action={primaryAction}"));
        assert!(SVELTE_COMPOSER_SOURCE.contains("id=\"stop-agent\""));
        assert!(
            SVELTE_COMPOSER_SOURCE
                .contains("<rect x=\"6\" y=\"6\" width=\"12\" height=\"12\" rx=\"2\" />")
        );
        assert!(
            SVELTE_COMPOSER_SOURCE
                .contains("if (primaryAction === \"resume\") dispatchSubmission(\"start\", true)")
        );
        assert!(AGENT_PANEL_HTML.contains("state.agentCanResume"));
        assert!(AGENT_PANEL_HTML.contains("message, resume, delivery"));
        assert!(
            AGENT_PANEL_HTML
                .contains("const blocked = !agentIsActive && (Boolean(permissionPending)")
        );
        assert!(AGENT_PANEL_HTML.contains("state.agentSubmissionQueueCount"));
        assert!(AGENT_PANEL_HTML.contains("delivery, timing: event.detail.timing, tab_ids:"));
        assert!(SVELTE_AGENT_TIMELINE_SOURCE.contains("CHAT_ZOOM_STORAGE_KEY"));
        assert!(SVELTE_AGENT_TIMELINE_SOURCE.contains("handleChatZoomKeydown"));
        assert!(SVELTE_AGENT_TIMELINE_SOURCE.contains("NumpadAdd"));
        assert!(SVELTE_AGENT_TIMELINE_SOURCE.contains("window.centralAgentApplyChatZoom"));
        assert!(SVELTE_AGENT_TIMELINE_SOURCE.contains("window.centralAgentSetChatZoom"));
        assert!(
            SVELTE_AGENT_TIMELINE_SOURCE
                .contains("keyboardTarget.addEventListener(\"keydown\", handleChatZoomKeydown as EventListener, true)")
        );
        assert!(include_str!("browser.rs").contains("with_browser_accelerator_keys(false)"));
        assert!(include_str!("browser.rs").contains("install_chat_zoom_accelerator(&webview"));
        assert!(include_str!("browser.rs").contains("install_chat_zoom_accelerator(&toolbar"));
        assert!(include_str!("browser.rs").contains("install_chat_zoom_accelerator(&panel"));
        assert!(include_str!("browser.rs").contains("arguments.SetHandled(true)"));
        assert!(include_str!("browser.rs").contains(".with_hotkeys_zoom(false)"));
        assert!(THEMES_CSS.contains("--ca-chat-font-offset: 0px"));
        assert!(AGENT_PANEL_HTML.contains(
            ".chat-empty, .chat-message { font-size: calc(13px + var(--ca-chat-font-offset)); }"
        ));
        assert!(SVELTE_AGENT_TIMELINE_SOURCE.contains("new ResizeObserver"));
        assert!(
            SVELTE_AGENT_TIMELINE_SOURCE
                .contains("chatMessages.scrollTop = chatMessages.scrollHeight")
        );
    }

    #[test]
    fn ssh_profiles_and_live_agent_arm_are_exposed_in_the_trusted_ui() {
        assert!(SVELTE_SETTINGS_CATALOG_SOURCE.contains("{id:\"settings-servers\""));
        assert!(AGENT_PANEL_HTML.contains("id=\"ssh-profile-form\""));
        assert!(AGENT_PANEL_HTML.contains("sendControl(\"open_ssh_terminal\""));
        assert!(AGENT_PANEL_HTML.contains("set_ssh_session_agent_ready"));
        assert!(!AGENT_PANEL_HTML.contains("id=\"ssh-session-map\""));
        assert!(
            AGENT_PANEL_HTML
                .contains("Passwords, passphrases and private-key contents are not saved")
        );
        assert!(AGENT_PANEL_HTML.contains("profile.agentEnabled ? \"Agent eligible\""));
    }

    #[test]
    fn remote_linux_desktop_supports_rdp_and_vnc_over_ssh_with_direct_input() {
        assert!(AGENT_PANEL_HTML.contains("id=\"remote-desktop-open\""));
        assert!(AGENT_PANEL_HTML.contains("id=\"remote-desktop-protocol\""));
        assert!(AGENT_PANEL_HTML.contains("sendControl(\"open_remote_desktop\""));
        assert!(
            AGENT_PANEL_HTML.contains("Both services stay bound to the VPS loopback interface")
        );
        assert!(AGENT_PANEL_HTML.contains("id=\"remote-desktop-profile\""));
        assert!(REMOTE_DESKTOP_HTML.contains("message_type: \"pointer\""));
        assert!(REMOTE_DESKTOP_HTML.contains("message_type: \"key\""));
        assert!(REMOTE_DESKTOP_HTML.contains("window.receiveRemoteDesktopDraw"));
        assert!(REMOTE_DESKTOP_HTML.contains("window.receiveRemoteDesktopCursor"));
        assert!(REMOTE_DESKTOP_HTML.contains("createImageBitmap"));
        assert!(REMOTE_DESKTOP_HTML.contains(
            "pendingDrawFrames.push({ generation, frameId, flowControlled, operations })"
        ));
        assert!(REMOTE_DESKTOP_HTML.contains("const frame = pendingDrawFrames.shift()"));
        assert!(REMOTE_DESKTOP_HTML.contains("await nextPaint();"));
        assert!(REMOTE_DESKTOP_HTML.contains("window.receiveVncDesktopDraw"));
        assert!(REMOTE_DESKTOP_HTML.contains("pendingVncDrawOperations.push(...operations)"));
        assert!(REMOTE_DESKTOP_HTML.contains("sendPointer(event)"));
        assert!(!REMOTE_DESKTOP_HTML.contains("visibleFrameBacklog"));
        assert!(REMOTE_DESKTOP_HTML.contains("id=\"remote-cursor\""));
        assert!(REMOTE_DESKTOP_HTML.contains("id=\"agent-control\""));
        assert!(REMOTE_DESKTOP_HTML.contains("message_type: \"set_agent_control\""));
        assert!(REMOTE_DESKTOP_HTML.contains("window.captureRemoteDesktopFrame"));
        assert!(REMOTE_DESKTOP_HTML.contains("message_type: \"snapshot\""));
        assert!(REMOTE_DESKTOP_HTML.contains("id=\"drop-layer\""));
        assert!(REMOTE_DESKTOP_HTML.contains("~/CentralAgent-Uploads"));
        assert!(
            REMOTE_DESKTOP_HTML.contains("is used only for this connection and is never saved")
        );
    }

    #[test]
    fn activity_messages_follow_the_action_lifecycle() {
        let mut message = ChatMessage::activity(
            5,
            18,
            "tool-4".to_owned(),
            AgentProviderKind::ClaudeCode,
            "Inspect the workspace",
            AgentActivityMetadata::generic(),
        );

        assert!(message.is_activity(18, "tool-4"));
        assert_eq!(message.text, "Inspect the workspace");
        assert_eq!(message.activity_status, Some(AgentStepStatus::Running));
        assert_eq!(message.activity_category, Some(AgentActivityCategory::Tool));
        assert!(message.streaming);

        message.set_activity_status(
            AgentStepStatus::Error,
            Some("The remote command returned code 1"),
        );
        assert_eq!(message.activity_status, Some(AgentStepStatus::Error));
        assert!(!message.streaming);
        assert!(message.text.ends_with("The remote command returned code 1"));
    }

    #[test]
    fn project_agent_graph_is_interactive_from_connected_projects() {
        assert!(AGENT_GRAPH_HTML.contains("id=\"knowledge-logo\""));
        assert!(AGENT_GRAPH_HTML.contains("aria-label=\"Open project board\""));
        assert!(AGENT_GRAPH_HTML.contains("data-graph-launcher-icon"));
        assert!(AGENT_GRAPH_HTML.contains("border: 0; border-radius: 0"));
        assert!(AGENT_GRAPH_HTML.contains("draggable=\"false\""));
        assert!(AGENT_GRAPH_HTML.contains("drag-reflow"));
        assert!(AGENT_GRAPH_HTML.contains(
            "orb.addEventListener(\"click\", () => sendNonDraggingSurfaceChange(\"open_knowledge_graph\"))"
        ));
        assert!(START_PAGE_HTML.contains("class=\"mark-slot\""));
        assert!(!START_PAGE_HTML.contains("<svg"));

        for lane in ["projects", "chats", "supervisors", "files"] {
            assert!(agent_graph_ui_contains(&format!(
                "data-board-lane=\"{lane}\""
            )));
        }
        assert!(agent_graph_ui_contains("Find a project"));
        assert!(agent_graph_ui_contains("Drop a folder here"));
        assert!(agent_graph_ui_contains("Clone repository"));
        assert!(agent_graph_ui_contains("SSH directory"));
        assert!(!AGENT_GRAPH_HTML.contains("supervisor-filesystem://"));
        assert!(!AGENT_GRAPH_HTML.contains("supervisor-host://local"));
        assert!(!AGENT_GRAPH_HTML.contains("function stepLayout()"));
        assert!(!AGENT_GRAPH_HTML.contains("getContext(\"2d\")"));
        assert!(agent_graph_ui_contains("Supervised conversation"));
        assert!(agent_graph_ui_contains("Other saved agents"));
        assert!(AGENT_GRAPH_HTML.contains("project_board"));

        assert!(THEMES_CSS.contains(":root[data-theme=\"central\"]"));
        assert!(THEMES_CSS.contains(":root[data-theme=\"central_dark\"]"));
        assert!(AGENT_GRAPH_HTML.contains("send(\"set_knowledge_agent\""));
        assert!(AGENT_GRAPH_HTML.contains("send(\"run_knowledge_agent\""));
        assert!(AGENT_GRAPH_HTML.contains("send(\"continue_knowledge_agent\""));
        assert!(AGENT_GRAPH_HTML.contains("send(\"stop_knowledge_agent\""));
        assert!(AGENT_GRAPH_HTML.contains("send(\"remove_knowledge_agent\""));
        assert!(agent_graph_ui_contains("data-role=\"provider\""));
        assert!(agent_graph_ui_contains("data-role=\"model\""));
        assert!(agent_graph_ui_contains("data-role=\"effort\""));
        assert!(agent_graph_ui_contains("data-role=\"speed\""));
        assert!(agent_graph_ui_contains("data-action=\"approve\""));
        assert!(agent_graph_ui_contains("data-action=\"deny\""));
        assert!(agent_graph_ui_contains("class=\"agent-console-compose\""));
        assert!(agent_graph_ui_contains("data-action=\"stop\""));
        assert!(agent_graph_ui_contains("data-action=\"close\""));
        assert!(AGENT_GRAPH_HTML.contains("send(\"set_knowledge_agent_link\""));
        assert!(AGENT_GRAPH_HTML.contains("window.renderAgentGraphState"));
    }

    #[test]
    fn floating_agent_graph_launcher_position_is_normalized() {
        let normalized = normalize_agent_graph_launcher_position(AgentGraphLauncherPosition {
            x_ratio: 1.8,
            y_ratio: -0.4,
        });
        assert_eq!(normalized.x_ratio, 1.0);
        assert_eq!(normalized.y_ratio, 0.0);

        let fallback = normalize_agent_graph_launcher_position(AgentGraphLauncherPosition {
            x_ratio: f64::NAN,
            y_ratio: f64::INFINITY,
        });
        let default = AgentGraphLauncherPosition::default();
        assert_eq!(fallback.x_ratio, default.x_ratio);
        assert_eq!(fallback.y_ratio, default.y_ratio);
    }

    #[test]
    fn agent_graph_launcher_follows_home_navigation_without_overlaying_web_pages() {
        let page = |is_start_page, loading| {
            Some(AgentGraphLauncherPage {
                is_start_page,
                loading,
                detached: false,
            })
        };
        // Home -> search/URL -> loaded website -> explicit Home -> ready Home.
        let navigation = [
            (page(true, false), true),
            (page(false, true), false),
            (page(false, false), false),
            (page(true, true), false),
            (page(true, false), true),
        ];
        for (page, expected) in navigation {
            assert_eq!(
                agent_graph_surface_visible(page, false, false, false),
                expected
            );
        }
        assert!(!agent_graph_surface_visible(None, false, false, false));
        assert!(!agent_graph_surface_visible(
            Some(AgentGraphLauncherPage {
                is_start_page: true,
                loading: false,
                detached: true,
            }),
            false,
            false,
            false,
        ));
    }

    #[test]
    fn agent_graph_launcher_respects_graph_settings_and_browser_visibility() {
        for is_start_page in [false, true] {
            for loading in [false, true] {
                let page = Some(AgentGraphLauncherPage {
                    is_start_page,
                    loading,
                    detached: false,
                });
                assert!(!agent_graph_surface_visible(page, false, true, false));
                // An explicitly opened graph is independent of browser navigation.
                assert!(agent_graph_surface_visible(page, false, true, true));
                assert!(agent_graph_surface_visible(page, false, false, true));
                for expanded in [false, true] {
                    assert!(!agent_graph_surface_visible(page, true, false, expanded));
                    assert!(!agent_graph_surface_visible(page, true, true, expanded));
                }
            }
        }
    }

    #[test]
    fn agent_graph_launcher_stays_inside_the_primary_home_pane() {
        for (width, height, logo_size, main_height) in [
            (
                1_920,
                1_080,
                AGENT_GRAPH_LAUNCHER_SIZE_FULL_HD_LOGICAL as u32,
                START_PAGE_MAIN_HEIGHT_FULL_HD_LOGICAL,
            ),
            (
                2_560,
                1_440,
                AGENT_GRAPH_LAUNCHER_SIZE_2K_LOGICAL as u32,
                START_PAGE_MAIN_HEIGHT_2K_LOGICAL,
            ),
            (
                3_840,
                2_160,
                AGENT_GRAPH_LAUNCHER_SIZE_4K_LOGICAL as u32,
                START_PAGE_MAIN_HEIGHT_4K_LOGICAL,
            ),
        ] {
            for side in [TabDockSide::Left, TabDockSide::Right] {
                let window = PhysicalSize::new(width, height);
                let (primary, docked) = split_tab_areas(
                    PixelArea {
                        x: 0,
                        y: 104,
                        width: width - 440,
                        height: height - 344,
                    },
                    true,
                    side,
                );
                let position = agent_graph_launcher_home_position_for_area(
                    window,
                    primary,
                    logo_size,
                    1.0,
                    main_height,
                );
                let left = (position.x_ratio * f64::from(width - logo_size)).round() as i32;
                let top = (position.y_ratio * f64::from(height - logo_size)).round() as i32;
                assert!(
                    left >= primary.x
                        && left + logo_size as i32 <= primary.x + primary.width as i32
                );
                assert!(
                    top >= primary.y && top + logo_size as i32 <= primary.y + primary.height as i32
                );
                let docked = docked.unwrap();
                assert!(
                    left + logo_size as i32 <= docked.x || left >= docked.x + docked.width as i32
                );
            }
        }
    }

    #[test]
    fn agent_graph_launcher_home_position_matches_the_start_page_slot() {
        let position = agent_graph_launcher_home_position_for_area(
            PhysicalSize::new(1_920, 1_080),
            PixelArea {
                x: 0,
                y: 104,
                width: 1_480,
                height: 976,
            },
            AGENT_GRAPH_LAUNCHER_SIZE_FULL_HD_LOGICAL as u32,
            1.0,
            START_PAGE_MAIN_HEIGHT_FULL_HD_LOGICAL,
        );
        let area = PixelArea {
            x: (position.x_ratio * 1_808.0).round() as i32,
            y: (position.y_ratio * 968.0).round() as i32,
            width: 112,
            height: 112,
        };

        assert_eq!(area.x, 684);
        assert_eq!(area.y, 415);
    }

    #[test]
    fn local_graph_checkpoints_use_disjoint_project_roots() {
        let workspace = tempfile::tempdir().unwrap();
        let first = workspace.path().join("first");
        let second = workspace.path().join("second");
        let nested = first.join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(&second).unwrap();
        let launch = |path: &Path| AgentGraphLaunch {
            node_key: "entity:test".to_owned(),
            agent_name: "Test agent".to_owned(),
            project_directory: path.display().to_string(),
            ssh_profile_id: None,
            user_request: "Inspect".to_owned(),
        };

        let first_root =
            local_agent_graph_checkpoint_root(workspace.path(), &launch(&first)).unwrap();
        let second_root =
            local_agent_graph_checkpoint_root(workspace.path(), &launch(&second)).unwrap();
        let nested_root =
            local_agent_graph_checkpoint_root(workspace.path(), &launch(&nested)).unwrap();

        assert!(!checkpoint_roots_overlap(&first_root, &second_root));
        assert!(checkpoint_roots_overlap(&first_root, &nested_root));

        let mut remote_launch = launch(&first);
        remote_launch.ssh_profile_id = Some("remote-profile".to_owned());
        assert!(local_agent_graph_checkpoint_root(workspace.path(), &remote_launch).is_none());
        let elsewhere = tempfile::tempdir().unwrap();
        let elsewhere_root = fs::canonicalize(elsewhere.path()).unwrap();
        assert_eq!(
            submission_checkpoint_root(Some(workspace.path()), Some(&launch(elsewhere.path()))),
            Some(elsewhere_root.clone())
        );
        assert_eq!(
            submission_checkpoint_root(None, Some(&launch(elsewhere.path()))),
            Some(elsewhere_root)
        );
        assert!(
            submission_checkpoint_root(
                Some(workspace.path()),
                Some(&launch(&workspace.path().join("missing")))
            )
            .is_none()
        );
        assert!(
            submission_checkpoint_root(
                Some(workspace.path()),
                Some(&launch(Path::new("relative")))
            )
            .is_none()
        );
        assert!(submission_checkpoint_root(Some(workspace.path()), Some(&remote_launch)).is_none());
    }

    #[test]
    fn project_removal_finds_parent_and_nested_checkpoint_runs_only() {
        let project = PathBuf::from(r"C:\work\project");
        let run_roots = HashMap::from([
            (3, project.clone()),
            (7, project.join("crates").join("desktop")),
            (11, PathBuf::from(r"C:\work\other")),
        ]);

        assert_eq!(
            checkpoint_run_ids_for_workspace(&run_roots, &project),
            vec![3, 7]
        );
        assert_eq!(
            checkpoint_run_ids_for_workspace(&run_roots, &project.join("crates")),
            vec![3, 7]
        );
        assert!(
            checkpoint_run_ids_for_workspace(&run_roots, Path::new(r"C:\unrelated")).is_empty()
        );
    }

    #[test]
    fn local_graph_providers_start_in_the_exact_checkpoint_root() {
        let workspace = PathBuf::from("workspace");
        let project = PathBuf::from("workspace/project");
        let fallback = PathBuf::from("provider-fallback");

        assert_eq!(
            agent_graph_provider_working_directory(
                Some(&workspace),
                Some(&project),
                false,
                &fallback,
            ),
            project
        );
        assert_eq!(
            agent_graph_provider_working_directory(Some(&workspace), None, false, &fallback),
            workspace
        );
        assert_eq!(
            agent_graph_provider_working_directory(
                Some(Path::new("workspace")),
                Some(&PathBuf::from("workspace/project")),
                true,
                &fallback,
            ),
            fallback
        );
    }

    #[test]
    fn graph_conversation_identity_preserves_legacy_owners_and_isolates_branches() {
        let record_id = "5aab7f58-aed5-426f-8dab-34defe18fbba";
        let original: AgentGraphBinding = serde_json::from_value(json!({
            "recordType": "entity", "recordId": record_id,
            "projectDirectory": r"C:\work\project",
            "name": "Original", "mission": "Inspect the project",
            "selection": {"model":"fixture-model", "effort":"high", "serviceTier":null, "contextWindow":null}
        }))
        .unwrap();
        assert_eq!(original.conversation_id, None);
        assert_eq!(original.node_key(), format!("entity:{record_id}"));
        let branch = AgentGraphBinding {
            conversation_id: Some(Uuid::new_v4()),
            name: "Branch".into(),
            ..original.clone()
        };
        let sibling = AgentGraphBinding {
            conversation_id: Some(Uuid::new_v4()),
            name: "Sibling".into(),
            ..original.clone()
        };
        let restored: Vec<AgentGraphBinding> = serde_json::from_slice(
            &serde_json::to_vec(&vec![original.clone(), branch.clone(), sibling.clone()]).unwrap(),
        )
        .unwrap();
        let normalized: Vec<_> = restored
            .into_iter()
            .map(|binding| normalize_loaded_agent_graph_binding(binding).unwrap())
            .collect();
        let keys: std::collections::HashSet<_> =
            normalized.iter().map(AgentGraphBinding::node_key).collect();
        assert_eq!(keys.len(), 3);
        assert_eq!(normalized[0].node_key(), original.node_key());
        assert_eq!(
            normalized[1].node_key(),
            format!("conversation:{}", branch.conversation_id.unwrap())
        );
        assert_eq!(normalized[2].node_key(), sibling.node_key());
        for binding in &normalized {
            assert!(binding.matches_target("entity", record_id, binding.conversation_id));
            assert!(!binding.matches_target("relation", record_id, binding.conversation_id));
            assert!(!binding.matches_target("entity", "another-node", binding.conversation_id));
            for other in &normalized {
                assert_eq!(
                    binding.matches_target("entity", record_id, other.conversation_id),
                    binding.node_key() == other.node_key()
                );
            }
        }
        let mut retained = normalized;
        retained.retain(|binding| binding.node_key() != branch.node_key());
        assert_eq!(retained.len(), 2);
        assert!(
            retained
                .iter()
                .any(|binding| binding.node_key() == original.node_key())
        );
        assert!(
            retained
                .iter()
                .any(|binding| binding.node_key() == sibling.node_key())
        );
    }

    #[test]
    fn agent_graph_session_round_trips_history_per_node() {
        let session = AgentGraphSession {
            node_key: "entity:node-1".to_owned(),
            record_type: "entity".to_owned(),
            record_id: "node-1".to_owned(),
            agent_name: "Repository agent".to_owned(),
            project_directory: "/srv/project".to_owned(),
            provider: AgentProviderKind::ClaudeCode,
            selection: AgentSelection {
                model: "gpt-5.6-sol".to_owned(),
                effort: "high".to_owned(),
                service_tier: Some("priority".to_owned()),
                personality: None,
                context_window: None,
            },
            turns: vec![AgentGraphTurn {
                run_id: 41,
                request: "Inspect the repository".to_owned(),
                provider: AgentProviderKind::ClaudeCode,
                selection: AgentSelection {
                    model: "claude-sonnet-4-5".to_owned(),
                    effort: "high".to_owned(),
                    service_tier: None,
                    personality: None,
                    context_window: None,
                },
                started_at_ms: 100,
                finished_at_ms: Some(200),
                phase: AgentPhase::Completed,
                status: "Objective completed · 1 actions".to_owned(),
                messages: vec![AgentGraphHistoryMessage {
                    native: None,
                    id: 7,
                    role: ChatRole::Assistant,
                    kind: ChatMessageKind::Reasoning,
                    text: "Verified the manifest".to_owned(),
                    timestamp_ms: 150,
                    artifacts: Vec::new(),
                }],
                steps: vec![AgentGraphHistoryStep {
                    label: "Read manifest".to_owned(),
                    kind: AgentStepKind::Observe,
                    status: AgentStepStatus::Success,
                    detail: None,
                }],
                checkpoint: None,
            }],
            updated_at_ms: 200,
        };

        let restored: AgentGraphSession =
            serde_json::from_slice(&serde_json::to_vec(&session).unwrap()).unwrap();

        assert_eq!(restored.node_key, "entity:node-1");
        assert_eq!(restored.turns.len(), 1);
        assert_eq!(restored.turns[0].request, "Inspect the repository");
        assert_eq!(restored.turns[0].steps[0].status, AgentStepStatus::Success);
        assert_eq!(
            restored.turns[0].messages[0].kind,
            ChatMessageKind::Reasoning
        );
    }

    #[test]
    fn removes_legacy_checkpoint_artifacts_without_touching_explicit_outputs() {
        let checkpoint: CheckpointSummary = serde_json::from_value(json!({
            "id": "checkpoint-1",
            "runId": 17,
            "workspaceName": "Supervisor",
            "provider": "Agent",
            "context": "Project chat",
            "createdAtMs": 10,
            "finishedAtMs": 20,
            "fileCount": 1,
            "totalAdditions": 12,
            "totalDeletions": 3,
            "warningCount": 0,
            "restoredFiles": 0,
            "fullyRestored": false,
            "changes": [{
                "path": "assets/agent-panel.html",
                "status": "modified",
                "iconKey": "html",
                "additions": 12,
                "deletions": 3,
                "binary": false,
                "restored": false
            }]
        }))
        .unwrap();
        let artifact = |id: &str, display_path: &str| ChatArtifact {
            id: id.to_owned(),
            kind: crate::artifact::ChatArtifactKind::InteractivePreview,
            title: "agent-panel.html".to_owned(),
            display_path: Some(display_path.to_owned()),
            mime_type: Some("text/html".to_owned()),
            byte_count: Some(519 * 1024),
            storage_key: Some(format!("{id}.html")),
            language: Some("html".to_owned()),
            source_url: None,
        };
        let mut artifacts = vec![
            artifact("legacy", r".\assets\agent-panel.html"),
            artifact(
                "explicit",
                "Agent output/image-1/generated-agent-panel.html",
            ),
        ];

        assert!(remove_checkpoint_artifact_duplicates(
            &mut artifacts,
            Some(&checkpoint)
        ));
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].id, "explicit");
        assert!(!remove_checkpoint_artifact_duplicates(
            &mut artifacts,
            Some(&checkpoint)
        ));
    }

    #[test]
    fn project_chat_history_round_trips_by_workspace_and_normalizes_live_messages() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(PROJECT_CHAT_HISTORY_FILE);
        let mut message =
            ChatMessage::reasoning(17, 4, "draft".to_owned(), 0, "Build the project navigation");
        message.role = ChatRole::User;
        message.kind = ChatMessageKind::Message;
        message.artifacts.push(ChatArtifact {
            id: "artifact-1".to_owned(),
            kind: crate::artifact::ChatArtifactKind::Code,
            title: "main.rs".to_owned(),
            display_path: Some("src/main.rs".to_owned()),
            mime_type: Some("text/plain".to_owned()),
            byte_count: Some(42),
            storage_key: Some("artifact-1.rs".to_owned()),
            language: Some("rust".to_owned()),
            source_url: None,
        });
        let history = ProjectChatHistory {
            chats: vec![ProjectChat {
                id: "chat-1".to_owned(),
                project_root: r"C:\code\project".to_owned(),
                title: String::new(),
                title_custom: false,
                pinned: true,
                archived: false,
                created_at_ms: 10,
                updated_at_ms: 20,
                messages: vec![message],
                claude_session_id: Some("session-1".to_owned()),
                claude_session_cwd: Some(r"C:\code\project".to_owned()),
            }],
        };
        let json = serde_json::to_vec_pretty(&history).unwrap();
        write_file_atomically::<ProjectChatHistory>(&path, &json).unwrap();

        let mut restored = load_project_chat_history(&path).chats;
        normalize_loaded_project_chats(&mut restored);
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].project_root, r"C:\code\project");
        assert_eq!(restored[0].title, "Build the project navigation");
        assert!(restored[0].pinned);
        assert!(!restored[0].title_custom);
        assert!(!restored[0].messages[0].streaming);
        assert_eq!(restored[0].messages[0].artifacts[0].title, "main.rs");
        assert_eq!(restored[0].claude_session_id.as_deref(), Some("session-1"));
    }

    #[test]
    fn moving_a_project_chat_preserves_its_history_and_rebinds_future_runs() {
        let message =
            ChatMessage::reasoning(23, 7, "reasoning-1".to_owned(), 0, "Keep this conversation");
        let mut chats = vec![ProjectChat {
            id: "chat-1".to_owned(),
            project_root: r"C:\code\first".to_owned(),
            title: "Persistent conversation".to_owned(),
            title_custom: true,
            pinned: true,
            archived: false,
            created_at_ms: 10,
            updated_at_ms: 20,
            messages: vec![message],
            claude_session_id: Some("session-bound-to-first".to_owned()),
            claude_session_cwd: Some(r"C:\code\first".to_owned()),
        }];

        assert!(reassign_project_chat(
            &mut chats,
            "chat-1",
            r"C:\code\second",
            30,
        ));
        let moved = &chats[0];
        assert_eq!(moved.project_root, r"C:\code\second");
        assert_eq!(moved.title, "Persistent conversation");
        assert!(moved.title_custom);
        assert!(moved.pinned);
        assert_eq!(moved.messages.len(), 1);
        assert_eq!(moved.messages[0].text, "Keep this conversation");
        assert_eq!(moved.updated_at_ms, 30);
        assert!(moved.claude_session_id.is_none());
        assert!(moved.claude_session_cwd.is_none());
        assert!(!reassign_project_chat(
            &mut chats,
            "chat-1",
            r"C:\code\second",
            40,
        ));
    }

    #[test]
    fn graph_agent_history_is_not_rewritten_into_the_general_session() {
        let mut browser_session = BrowserSession::default();
        browser_session
            .agent_graph_sessions
            .push(AgentGraphSession {
                node_key: "entity:node-1".to_owned(),
                record_type: "entity".to_owned(),
                record_id: "node-1".to_owned(),
                agent_name: "Repository agent".to_owned(),
                project_directory: "/srv/project".to_owned(),
                provider: AgentProviderKind::ClaudeCode,
                selection: AgentSelection::default(),
                turns: Vec::new(),
                updated_at_ms: 1,
            });

        let serialized = serde_json::to_value(&browser_session).unwrap();
        assert!(serialized.get("knowledgeAgentSessions").is_none());
    }

    #[test]
    fn graph_checkpoint_diff_is_saved_without_duplicated_artifacts() {
        let source = include_str!("browser.rs");
        let completion = source
            .split_once("fn complete_time_machine_checkpoint")
            .and_then(|(_, rest)| rest.split_once("fn restore_time_machine_checkpoint"))
            .map(|(body, _)| body)
            .expect("checkpoint completion must remain discoverable");
        let restore = source
            .split_once("fn restore_time_machine_checkpoint")
            .and_then(|(_, rest)| rest.split_once("fn refresh_checkpoint_references"))
            .map(|(body, _)| body)
            .expect("checkpoint restore must remain discoverable");

        assert!(completion.contains("should_publish_checkpoint_summary(&summary)"));
        assert!(completion.contains("turn.checkpoint = Some(summary.clone())"));
        assert!(completion.contains("self.save_agent_graph_sessions()"));
        assert!(!completion.contains("self.attach_artifacts_to_run"));
        assert!(!completion.contains("ingest_path"));
        assert!(completion.contains("turn.messages.push(AgentGraphHistoryMessage"));
        assert!(completion.contains("routed_to_graph_history"));
        assert!(restore.contains("self.save_agent_graph_sessions()"));
    }

    #[test]
    fn agent_graph_provider_context_is_recent_compact_and_excludes_activity_noise() {
        let turns = (0..6)
            .map(|index| AgentGraphTurn {
                run_id: index,
                request: format!("Request {index}"),
                provider: AgentProviderKind::ClaudeCode,
                selection: AgentSelection::default(),
                started_at_ms: index as u128,
                finished_at_ms: Some(index as u128 + 1),
                phase: AgentPhase::Completed,
                status: format!("Completed {index}"),
                messages: vec![
                    AgentGraphHistoryMessage {
                        id: index * 2,
                        role: ChatRole::Assistant,
                        kind: ChatMessageKind::Activity,
                        text: "large tool transcript that should stay local".to_owned(),
                        native: None,
                        timestamp_ms: index as u128,
                        artifacts: Vec::new(),
                    },
                    AgentGraphHistoryMessage {
                        id: index * 2 + 1,
                        native: None,
                        role: ChatRole::Assistant,
                        kind: ChatMessageKind::Message,
                        text: format!("Verified result {index}"),
                        timestamp_ms: index as u128,
                        artifacts: Vec::new(),
                    },
                ],
                steps: Vec::new(),
                checkpoint: None,
            })
            .collect();
        let session = AgentGraphSession {
            node_key: "entity:node-1".to_owned(),
            record_type: "entity".to_owned(),
            record_id: "node-1".to_owned(),
            agent_name: "Repository agent".to_owned(),
            project_directory: "/srv/project".to_owned(),
            provider: AgentProviderKind::ClaudeCode,
            selection: AgentSelection::default(),
            turns,
            updated_at_ms: 10,
        };

        let context = compact_agent_graph_session(&session, 2);
        assert_eq!(context["olderTurnCount"], 4);
        assert_eq!(context["recentTurns"].as_array().unwrap().len(), 2);
        assert_eq!(context["recentTurns"][0]["request"], "Request 4");
        assert_eq!(
            context["recentTurns"][1]["messages"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            context["recentTurns"][1]["messages"][0]["text"],
            "Verified result 5"
        );
    }

    #[test]
    fn newly_attached_terminal_context_follows_live_by_default() {
        let context = DraftTerminalContext::attached(
            TerminalContextSnapshot {
                session_id: 3,
                label: "Shell 3".to_owned(),
                kind: TerminalKind::Local,
                profile_id: None,
                remote_target: None,
                phase: TerminalPhase::Running,
                busy: false,
                shell: "powershell.exe".to_owned(),
                cwd: "C:\\work".to_owned(),
                status: "Interactive shell is running".to_owned(),
                output: "PS C:\\work> car".to_owned(),
                source_output_char_count: 15,
                output_revision: 4,
                last_exit_code: None,
                captured_at_ms: 20,
                estimated_token_count: 64,
                redaction_count: 0,
                truncated: false,
            },
            20,
        );

        assert!(context.follow_live);
        assert_eq!(context.view(true).output_preview, "PS C:\\work> car");
    }

    #[test]
    fn reveals_incoming_tabs_before_hiding_outgoing_tabs() {
        let tabs = [(10, false), (20, false), (30, true), (40, false)];
        let updates = ordered_tab_visibility_updates(&tabs, Some(20), None);

        assert_eq!(updates, vec![(1, true), (2, true), (0, false), (3, false)]);
    }

    #[test]
    fn minimized_browser_keeps_detached_tabs_visible() {
        let tabs = [(10, false), (20, false), (30, true)];
        assert_eq!(
            ordered_tab_visibility_updates(&tabs, None, None),
            vec![(2, true), (0, false), (1, false)]
        );
        assert_eq!(
            ordered_tab_visibility_updates(&tabs, Some(10), Some(20)),
            vec![(0, true), (1, true), (2, true)]
        );
    }

    #[test]
    fn minimized_browser_layout_gives_agent_full_content_with_space_for_shells() {
        for (width, height, toolbar) in [
            (1920, 1080, 104),
            (2560, 1440, 128),
            (3840, 2160, 128),
            (2880, 1620, 156),
        ] {
            let size = PhysicalSize::new(width, height);
            let full = browser_workspace_area_for_size(size, toolbar, None);
            assert_eq!(
                full,
                PixelArea {
                    x: 0,
                    y: toolbar as i32,
                    width,
                    height: height - toolbar
                }
            );
            let restored = browser_workspace_area_for_size(size, toolbar, Some(900));
            assert_eq!(restored.x, 900);
            assert_eq!(restored.width, width - 900);
            let bottom = content_area_reserving_terminal(full, TerminalDock::Bottom, 300);
            assert_eq!(bottom.width, width);
            assert_eq!(bottom.height + 300, full.height);
            let right = content_area_reserving_terminal(full, TerminalDock::Right, 480);
            assert_eq!(right.width + 480, full.width);
            assert_eq!(right.height, full.height);
        }
    }

    #[test]
    fn browser_toolbar_follows_the_visible_browser_pane() {
        let size = PhysicalSize::new(1920, 1080);
        assert_eq!(
            browser_toolbar_area_for_size(size, Some(720), 104),
            PixelArea {
                x: 720,
                y: 0,
                width: 1200,
                height: 104,
            }
        );
        assert_eq!(
            browser_toolbar_area_for_size(size, None, 60),
            PixelArea {
                x: 0,
                y: 0,
                width: 1920,
                height: 60,
            }
        );
        assert_eq!(
            browser_toolbar_area_for_size(size, Some(2400), 1200),
            PixelArea {
                x: 1920,
                y: 0,
                width: 0,
                height: 1080,
            }
        );
    }

    #[test]
    fn minimized_browser_state_preserves_saved_split_and_round_trips() {
        let session = BrowserSession {
            browser_panel_minimized: true,
            agent_panel_width_logical: Some(1040.0),
            ..BrowserSession::default()
        };
        let encoded = serde_json::to_string(&session).unwrap();
        let restored: BrowserSession = serde_json::from_str(&encoded).unwrap();
        assert!(restored.browser_panel_minimized);
        assert_eq!(restored.agent_panel_width_logical, Some(1040.0));
        let command: ToolbarCommand =
            serde_json::from_str(r#"{"type":"toggle_browser_panel"}"#).unwrap();
        assert!(matches!(&command, ToolbarCommand::ToggleBrowserPanel));
        assert!(command.targets_browser());

        let command: ToolbarCommand =
            serde_json::from_str(r#"{"type":"close_project_board"}"#).unwrap();
        assert!(matches!(&command, ToolbarCommand::CloseProjectBoard));
        assert!(!command.targets_browser());
    }

    #[test]
    fn minimize_browser_changes_visibility_without_navigation_or_destroying_tabs() {
        let source = include_str!("browser.rs");
        let toggle = source
            .split("    fn set_browser_panel_minimized(")
            .nth(1)
            .unwrap()
            .split("    fn toggle_agent_panel(")
            .next()
            .unwrap();
        for forbidden in [
            "load_url(",
            "load_html(",
            "close_tab(",
            "tabs.clear(",
            "agent_panel_width_logical =",
        ] {
            assert!(
                !toggle.contains(forbidden),
                "minimizing must not perform {forbidden}"
            );
        }
        assert!(toggle.contains("self.update_tab_webview_visibility()"));
        assert!(TOOLBAR_HTML.contains("data-central-agent-svelte=\"browser-panel-toggle\""));
        assert!(TOOLBAR_HTML.contains("updateBrowserPanelToggle"));
        assert!(
            AGENT_PANEL_HTML
                .contains("agentPanelResizer.hidden = Boolean(state.browserPanelMinimized)")
        );
    }

    #[test]
    fn defers_new_tab_activation_only_when_a_pane_is_already_visible() {
        assert!(!should_defer_new_tab_activation(true, None, None));
        assert!(should_defer_new_tab_activation(true, Some(1), None));
        assert!(should_defer_new_tab_activation(true, None, Some(2)));
        assert!(!should_defer_new_tab_activation(false, Some(1), Some(2)));
    }

    #[test]
    fn reads_legacy_sessions_with_docking_defaults() {
        let session: BrowserSession = serde_json::from_str(
            r#"{"tabs":[{"url":null}],"activeIndex":0,"agentPanelVisible":true,"theme":"light","searchEngine":"duckduckgo"}"#,
        )
        .unwrap();
        assert_eq!(session.primary_index, None);
        assert_eq!(session.docked_index, None);
        assert_eq!(session.tab_dock_side, TabDockSide::Right);
        assert_eq!(session.terminal_dock, TerminalDock::Bottom);
        assert!(!session.tabs[0].detached);
        assert!(!session.agent_panel_detached);
        assert!(!session.browser_panel_minimized);
        assert!(session.agent_panel_width_logical.is_none());
        assert!(session.workspace_roots.is_empty());
        assert_eq!(session.agent_provider, AgentProviderKind::ClaudeCode);
        assert!(session.claude_session_id.is_none());
        assert!(session.claude_session_cwd.is_none());
        assert_eq!(session.audit_retention_days, DEFAULT_AUDIT_RETENTION_DAYS);
        assert!(session.agent_graph_bindings.is_empty());
        assert!(session.agent_graph_sessions.is_empty());
        assert!(session.ssh_profiles.is_empty());
        assert_eq!(
            session.remote_desktop.remote_port,
            crate::remote_desktop::DEFAULT_REMOTE_DESKTOP_PORT
        );
        assert!(session.remote_desktop.profile_id.is_none());
        assert!(!session.agent_graph_launcher_v1);
        assert_eq!(
            session.agent_graph_launcher_position.x_ratio,
            AgentGraphLauncherPosition::default().x_ratio
        );
        assert_eq!(
            session.agent_graph_launcher_position.y_ratio,
            AgentGraphLauncherPosition::default().y_ratio
        );
    }

    #[test]
    fn agent_graph_internal_names_preserve_the_existing_session_schema() {
        let session: BrowserSession = serde_json::from_value(json!({
            "tabs": [],
            "activeIndex": 0,
            "knowledgeAgentBindings": [{
                "recordType": "entity",
                "recordId": "2a568e92-25d8-4ff0-b97f-3945f3c090c2",
                "projectDirectory": r"C:\work\project",
                "name": "Project agent",
                "mission": "Verify the project",
                "provider": "codex_app_server",
                "selection": {
                    "model": "gpt-5.6-luna",
                    "effort": "low",
                    "serviceTier": null,
                    "contextWindow": null
                }
            }],
            "knowledgeAgentLinks": [{
                "sourceNodeKey": "entity:a",
                "targetNodeKey": "entity:b"
            }],
            "knowledgeOrbPosition": {"xRatio": 0.25, "yRatio": 0.75},
            "knowledgeLogoControlV1": true
        }))
        .unwrap();
        assert_eq!(session.agent_graph_bindings.len(), 1);
        assert_eq!(session.agent_graph_bindings[0].name, "Project agent");
        assert_eq!(session.agent_graph_links.len(), 1);
        assert_eq!(session.agent_graph_launcher_position.x_ratio, 0.25);
        assert!(session.agent_graph_launcher_v1);

        let stored = serde_json::to_value(session).unwrap();
        assert!(stored.get("knowledgeAgentBindings").is_some());
        assert!(stored.get("knowledgeAgentLinks").is_some());
        assert!(stored.get("knowledgeOrbPosition").is_some());
        assert!(stored.get("knowledgeLogoControlV1").is_some());
        assert!(stored.get("agentGraphBindings").is_none());
    }

    #[test]
    fn session_save_is_atomic_and_recovers_the_previous_valid_backup() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.json");
        let original = BrowserSession::default();
        write_file_atomically::<BrowserSession>(&path, &serde_json::to_vec(&original).unwrap())
            .unwrap();

        let updated = BrowserSession {
            theme: "dark".to_owned(),
            ..BrowserSession::default()
        };
        write_file_atomically::<BrowserSession>(&path, &serde_json::to_vec(&updated).unwrap())
            .unwrap();
        assert_eq!(load_session(&path).theme, "dark");

        fs::write(&path, b"{truncated").unwrap();
        assert_eq!(load_session(&path).theme, original.theme);
        assert_eq!(read_browser_session(&path).unwrap().theme, original.theme);

        let resaved = BrowserSession {
            theme: "dark".to_owned(),
            ..BrowserSession::default()
        };
        write_file_atomically::<BrowserSession>(&path, &serde_json::to_vec(&resaved).unwrap())
            .unwrap();
        assert_eq!(load_session(&path).theme, "dark");
    }

    #[test]
    fn atomic_save_does_not_replace_a_good_backup_with_a_corrupt_primary() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session.json");
        let original = BrowserSession::default();
        let updated = BrowserSession {
            theme: "dark".to_owned(),
            ..BrowserSession::default()
        };
        write_file_atomically::<BrowserSession>(&path, &serde_json::to_vec(&original).unwrap())
            .unwrap();
        write_file_atomically::<BrowserSession>(&path, &serde_json::to_vec(&updated).unwrap())
            .unwrap();
        fs::write(&path, b"{truncated").unwrap();

        write_file_atomically::<BrowserSession>(&path, &serde_json::to_vec(&updated).unwrap())
            .unwrap();

        assert_eq!(
            read_browser_session(&backup_path_for(&path)).unwrap().theme,
            original.theme
        );
        assert_eq!(read_browser_session(&path).unwrap().theme, "dark");
    }

    #[test]
    fn graph_agent_history_recovers_the_previous_valid_backup() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(AGENT_GRAPH_HISTORY_FILE);
        let session = AgentGraphSession {
            node_key: "entity:node-1".to_owned(),
            record_type: "entity".to_owned(),
            record_id: "node-1".to_owned(),
            agent_name: "Repository agent".to_owned(),
            project_directory: "/srv/project".to_owned(),
            provider: AgentProviderKind::ClaudeCode,
            selection: AgentSelection::default(),
            turns: Vec::new(),
            updated_at_ms: 1,
        };
        write_file_atomically::<Vec<AgentGraphSession>>(
            &path,
            &serde_json::to_vec(&vec![session.clone()]).unwrap(),
        )
        .unwrap();
        let updated = AgentGraphSession {
            updated_at_ms: 2,
            ..session
        };
        write_file_atomically::<Vec<AgentGraphSession>>(
            &path,
            &serde_json::to_vec(&vec![updated]).unwrap(),
        )
        .unwrap();
        fs::write(&path, b"{truncated").unwrap();

        let recovered = load_agent_graph_sessions(&path).unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].updated_at_ms, 1);
        assert_eq!(
            read_agent_graph_sessions(&path).unwrap()[0].updated_at_ms,
            1
        );
        write_file_atomically::<Vec<AgentGraphSession>>(
            &path,
            &serde_json::to_vec(&recovered).unwrap(),
        )
        .unwrap();
        assert_eq!(
            load_agent_graph_sessions(&path).unwrap()[0].updated_at_ms,
            1
        );
    }

    #[test]
    fn persists_a_workspace_bound_claude_code_session() {
        let session: BrowserSession = serde_json::from_str(
            r#"{"tabs":[],"activeIndex":0,"agentProvider":"claude_code","claudeSelection":{"model":"sonnet","effort":"high","serviceTier":null},"claudeSessionId":"6d24affd-e20b-4a94-a65f-bb897484348f","claudeSessionCwd":"C:\\work\\demo"}"#,
        )
        .unwrap();
        assert_eq!(session.agent_provider, AgentProviderKind::ClaudeCode);
        assert_eq!(session.claude_selection.model, "sonnet");
        assert_eq!(
            session.claude_session_id.as_deref(),
            Some("6d24affd-e20b-4a94-a65f-bb897484348f")
        );
        assert_eq!(session.claude_session_cwd.as_deref(), Some(r"C:\work\demo"));
    }

    #[test]
    fn parses_tab_tear_out_coordinates_and_explicit_reattach() {
        let detach: ToolbarCommand = serde_json::from_str(
            r#"{"type":"detach_tab","tab_id":8,"screen_x":2120,"screen_y":580}"#,
        )
        .unwrap();
        let attach: ToolbarCommand =
            serde_json::from_str(r#"{"type":"attach_tab","tab_id":8}"#).unwrap();

        assert!(matches!(
            detach,
            ToolbarCommand::DetachTab {
                tab_id: 8,
                screen_x: Some(2120),
                screen_y: Some(580)
            }
        ));
        assert!(matches!(attach, ToolbarCommand::AttachTab { tab_id: 8 }));
    }

    #[test]
    fn visual_capture_is_low_resolution_and_bounded() {
        let (params, truncated) = visual_capture_params(1_600, 80_000);
        let params: Value = serde_json::from_str(&params).unwrap();

        assert!(truncated);
        assert_eq!(params["format"], "jpeg");
        assert_eq!(params["quality"], 42);
        assert_eq!(params["clip"]["height"], 25_000);
        assert!(params["clip"]["scale"].as_f64().unwrap() < 0.3);
    }

    #[test]
    fn selects_full_hd_2k_and_4k_layout_profiles() {
        assert_eq!(
            ui_layout_profile_for_size(1_920.0, 1_080.0),
            UiLayoutProfile::FullHd
        );
        assert_eq!(
            ui_layout_profile_for_size(2_560.0, 1_440.0),
            UiLayoutProfile::TwoK
        );
        assert_eq!(
            ui_layout_profile_for_size(3_840.0, 2_160.0),
            UiLayoutProfile::FourK
        );
    }

    #[test]
    fn centers_window_inside_positive_or_negative_monitor_coordinates() {
        assert_eq!(
            centered_window_position(
                PhysicalPosition::new(0, 0),
                PhysicalSize::new(2_560, 1_440),
                PhysicalSize::new(1_920, 1_080),
            ),
            PhysicalPosition::new(320, 180)
        );
        assert_eq!(
            centered_window_position(
                PhysicalPosition::new(-3_840, 0),
                PhysicalSize::new(3_840, 2_160),
                PhysicalSize::new(1_920, 1_080),
            ),
            PhysicalPosition::new(-2_880, 540)
        );
    }

    #[test]
    fn splits_docked_tabs_without_losing_pixels() {
        let content = PixelArea {
            x: 0,
            y: 104,
            width: 1_479,
            height: 976,
        };
        let (primary, docked) = split_tab_areas(content, true, TabDockSide::Right);
        let docked = docked.unwrap();
        assert_eq!(primary.width + docked.width, content.width);
        assert_eq!(docked.x, primary.x + primary.width as i32);

        let (primary, docked) = split_tab_areas(content, true, TabDockSide::Left);
        let docked = docked.unwrap();
        assert_eq!(docked.x, content.x);
        assert_eq!(primary.x, docked.x + docked.width as i32);
    }

    #[test]
    fn keeps_single_tab_at_full_size() {
        let content = PixelArea {
            x: 3,
            y: 9,
            width: 800,
            height: 600,
        };
        assert_eq!(
            split_tab_areas(content, false, TabDockSide::Right),
            (content, None)
        );
    }
}
