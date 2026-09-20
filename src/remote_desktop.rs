use std::{
    fmt,
    net::{Ipv4Addr, SocketAddrV4, TcpListener},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use image::codecs::jpeg::JpegEncoder;
use ironrdp::{
    client::{
        config::{Config, ConfigBuilder, Destination},
        rdp::{RdpClient, RdpInputEvent, RdpOutputEvent},
    },
    pdu::{
        input::{
            MousePdu,
            fast_path::{FastPathInputEvent, KeyboardFlags},
            mouse::PointerFlags,
        },
        rdp::capability_sets::MajorPlatformType,
    },
};
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use tokio::{
    net::TcpStream,
    runtime::Builder,
    sync::{
        Notify,
        mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
    },
    time::{MissedTickBehavior, interval, sleep},
};
use vnc::{DesktopScreen, PixelFormat, Rect, VncConnector, VncEncoding, VncEvent, X11Event};

use crate::ssh_runtime::SshDesktopTunnelSpec;

pub(crate) const DEFAULT_RDP_PORT: u16 = 3389;
pub(crate) const DEFAULT_VNC_PORT: u16 = 5901;
pub(crate) const DEFAULT_REMOTE_DESKTOP_PORT: u16 = DEFAULT_RDP_PORT;
const VNC_TARGET_WIDTH: u16 = 1280;
const VNC_TARGET_HEIGHT: u16 = 720;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RemoteDesktopProtocol {
    #[default]
    Rdp,
    Vnc,
}

impl RemoteDesktopProtocol {
    pub(crate) const fn default_port(self) -> u16 {
        match self {
            Self::Rdp => DEFAULT_RDP_PORT,
            Self::Vnc => DEFAULT_VNC_PORT,
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Rdp => "RDP",
            Self::Vnc => "VNC",
        }
    }

    pub(crate) const fn clipboard_supported(self) -> bool {
        matches!(self, Self::Vnc)
    }
}

impl fmt::Display for RemoteDesktopProtocol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

const fn legacy_remote_desktop_protocol() -> RemoteDesktopProtocol {
    RemoteDesktopProtocol::Vnc
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct RemoteDesktopPreference {
    pub profile_id: Option<String>,
    #[serde(default = "legacy_remote_desktop_protocol")]
    pub protocol: RemoteDesktopProtocol,
    pub remote_port: u16,
}

impl Default for RemoteDesktopPreference {
    fn default() -> Self {
        Self {
            profile_id: None,
            protocol: RemoteDesktopProtocol::Rdp,
            remote_port: DEFAULT_REMOTE_DESKTOP_PORT,
        }
    }
}

impl RemoteDesktopPreference {
    pub(crate) fn normalized(mut self) -> Self {
        self.profile_id = self
            .profile_id
            .take()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if self.remote_port == 0 {
            self.remote_port = self.protocol.default_port();
        }
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RemoteDesktopPhase {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Error,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteDesktopView<'a> {
    pub configured: bool,
    pub phase: RemoteDesktopPhase,
    pub profile_id: Option<&'a str>,
    pub profile_name: Option<&'a str>,
    pub username: Option<&'a str>,
    pub remote_target: Option<&'a str>,
    pub protocol: RemoteDesktopProtocol,
    pub remote_port: u16,
    pub clipboard_supported: bool,
    pub width: u16,
    pub height: u16,
    pub status: &'a str,
    pub agent_control_enabled: bool,
    pub upload_busy: bool,
    pub upload_status: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "message_type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum RemoteDesktopMessage {
    Ready,
    Connect {
        #[serde(default)]
        username: String,
        #[serde(default)]
        password: String,
    },
    Disconnect,
    Refresh,
    Pointer {
        x: u16,
        y: u16,
        buttons: u8,
    },
    Key {
        #[serde(default)]
        code: String,
        keysym: u32,
        down: bool,
    },
    Clipboard {
        text: String,
    },
    SetAgentControl {
        enabled: bool,
    },
    Snapshot {
        request_id: String,
        width: u16,
        height: u16,
        #[serde(default)]
        data_url: String,
        #[serde(default)]
        error: Option<String>,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum RemoteDesktopDrawOp {
    Rgba {
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        data: String,
    },
    Jpeg {
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        data: String,
    },
    Copy {
        source_x: u16,
        source_y: u16,
        width: u16,
        height: u16,
        destination_x: u16,
        destination_y: u16,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum RemoteDesktopCursor {
    Default,
    Hidden,
    Position {
        x: u16,
        y: u16,
    },
    Bitmap {
        width: u16,
        height: u16,
        hotspot_x: u16,
        hotspot_y: u16,
        data: String,
    },
}

#[derive(Debug)]
pub(crate) enum RemoteDesktopEvent {
    Connected {
        generation: u64,
    },
    Resolution {
        generation: u64,
        width: u16,
        height: u16,
    },
    Draw {
        generation: u64,
        frame_id: u64,
        operations: Vec<RemoteDesktopDrawOp>,
    },
    Cursor {
        generation: u64,
        cursor: RemoteDesktopCursor,
    },
    Clipboard {
        generation: u64,
        text: String,
    },
    Bell {
        generation: u64,
    },
    Stopped {
        generation: u64,
        error: Option<String>,
    },
}

impl RemoteDesktopEvent {
    pub(crate) fn generation(&self) -> u64 {
        match self {
            Self::Connected { generation }
            | Self::Resolution { generation, .. }
            | Self::Draw { generation, .. }
            | Self::Cursor { generation, .. }
            | Self::Clipboard { generation, .. }
            | Self::Bell { generation }
            | Self::Stopped { generation, .. } => *generation,
        }
    }
}

#[derive(Debug)]
enum RemoteDesktopInput {
    Disconnect,
    Refresh,
    Pointer {
        x: u16,
        y: u16,
        buttons: u8,
    },
    Key {
        code: String,
        keysym: u32,
        down: bool,
    },
    Clipboard(String),
}

struct RemoteDesktopConnection {
    protocol: RemoteDesktopProtocol,
    spec: SshDesktopTunnelSpec,
    username: String,
    password: String,
}

type SharedTunnel = Arc<Mutex<Option<Child>>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FrameRegion {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

impl FrameRegion {
    fn union(self, other: Self) -> Self {
        let left = self.x.min(other.x);
        let top = self.y.min(other.y);
        let right = (u32::from(self.x) + u32::from(self.width))
            .max(u32::from(other.x) + u32::from(other.width));
        let bottom = (u32::from(self.y) + u32::from(self.height))
            .max(u32::from(other.y) + u32::from(other.height));
        Self {
            x: left,
            y: top,
            width: u16::try_from(right - u32::from(left)).unwrap_or(u16::MAX),
            height: u16::try_from(bottom - u32::from(top)).unwrap_or(u16::MAX),
        }
    }

    fn area(self) -> usize {
        usize::from(self.width) * usize::from(self.height)
    }
}

#[derive(Debug)]
struct RdpEncodeJob {
    pixels: Vec<u32>,
    region: FrameRegion,
    desktop_width: u16,
    desktop_height: u16,
    quality: u8,
}

#[derive(Debug, Default)]
struct RdpFrameState {
    width: u16,
    height: u16,
    pixels: Vec<u32>,
    dirty: Option<FrameRegion>,
    refine: Option<FrameRegion>,
    revision: u64,
}

#[derive(Debug, Default)]
struct RdpFrameMailbox {
    state: Mutex<RdpFrameState>,
    wake: Notify,
}

impl RdpFrameMailbox {
    fn submit(
        &self,
        desktop_width: u16,
        desktop_height: u16,
        region: FrameRegion,
        patch: Vec<u32>,
    ) -> Result<(), String> {
        let expected = region.area();
        if region.width == 0 || region.height == 0 || patch.len() != expected {
            return Err("The RDP dirty region dimensions were inconsistent".to_owned());
        }
        let right = u32::from(region.x) + u32::from(region.width);
        let bottom = u32::from(region.y) + u32::from(region.height);
        if right > u32::from(desktop_width) || bottom > u32::from(desktop_height) {
            return Err("The RDP dirty region exceeded the framebuffer".to_owned());
        }

        let mut state = self
            .state
            .lock()
            .map_err(|_| "The RDP frame mailbox is unavailable".to_owned())?;
        if state.width != desktop_width || state.height != desktop_height {
            state.width = desktop_width;
            state.height = desktop_height;
            state.pixels = vec![0; usize::from(desktop_width) * usize::from(desktop_height)];
            state.dirty = None;
            state.refine = None;
        }
        let destination_stride = usize::from(desktop_width);
        let source_stride = usize::from(region.width);
        for row in 0..usize::from(region.height) {
            let source_start = row * source_stride;
            let destination_start =
                (usize::from(region.y) + row) * destination_stride + usize::from(region.x);
            state.pixels[destination_start..destination_start + source_stride]
                .copy_from_slice(&patch[source_start..source_start + source_stride]);
        }
        state.dirty = Some(state.dirty.map_or(region, |dirty| dirty.union(region)));
        state.refine = Some(state.refine.map_or(region, |refine| refine.union(region)));
        state.revision = state.revision.saturating_add(1);
        drop(state);
        self.wake.notify_one();
        Ok(())
    }

    fn take_dirty(&self) -> Result<Option<RdpEncodeJob>, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "The RDP frame mailbox is unavailable".to_owned())?;
        let Some(region) = state.dirty.take() else {
            return Ok(None);
        };
        let quality = motion_jpeg_quality(region, state.width, state.height);
        Ok(Some(snapshot_rdp_region(&state, region, quality)))
    }

    fn refinement_state(&self) -> Result<(u64, bool), String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "The RDP frame mailbox is unavailable".to_owned())?;
        Ok((state.revision, state.refine.is_some()))
    }

    fn take_refinement(&self, revision: u64) -> Result<Option<RdpEncodeJob>, String> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| "The RDP frame mailbox is unavailable".to_owned())?;
        if state.revision != revision || state.dirty.is_some() {
            return Ok(None);
        }
        let Some(region) = state.refine.take() else {
            return Ok(None);
        };
        Ok(Some(snapshot_rdp_region(&state, region, 84)))
    }
}

fn snapshot_rdp_region(state: &RdpFrameState, region: FrameRegion, quality: u8) -> RdpEncodeJob {
    let mut pixels = Vec::with_capacity(region.area());
    let stride = usize::from(state.width);
    for row in 0..usize::from(region.height) {
        let start = (usize::from(region.y) + row) * stride + usize::from(region.x);
        pixels.extend_from_slice(&state.pixels[start..start + usize::from(region.width)]);
    }
    RdpEncodeJob {
        pixels,
        region,
        desktop_width: state.width,
        desktop_height: state.height,
        quality,
    }
}

fn motion_jpeg_quality(region: FrameRegion, desktop_width: u16, desktop_height: u16) -> u8 {
    let desktop_area = usize::from(desktop_width) * usize::from(desktop_height);
    if desktop_area == 0 {
        return 68;
    }
    let percentage = region.area().saturating_mul(100) / desktop_area;
    if percentage >= 45 {
        60
    } else if percentage >= 15 {
        66
    } else {
        74
    }
}

#[derive(Debug)]
enum PendingVncDrawOp {
    Rgba { region: FrameRegion, data: Vec<u8> },
    Jpeg { region: FrameRegion, data: Vec<u8> },
    Copy { destination: Rect, source: Rect },
}

#[derive(Debug, Default)]
struct VncDrawMailbox {
    operations: Mutex<Vec<PendingVncDrawOp>>,
    wake: Notify,
}

impl VncDrawMailbox {
    fn submit(&self, operations: Vec<PendingVncDrawOp>) -> Result<(), String> {
        if operations.is_empty() {
            return Ok(());
        }
        self.operations
            .lock()
            .map_err(|_| "The VNC draw mailbox is unavailable".to_owned())?
            .extend(operations);
        self.wake.notify_one();
        Ok(())
    }

    fn take(&self) -> Result<Vec<PendingVncDrawOp>, String> {
        let mut operations = self
            .operations
            .lock()
            .map_err(|_| "The VNC draw mailbox is unavailable".to_owned())?;
        Ok(std::mem::take(&mut *operations))
    }
}

#[derive(Debug)]
pub(crate) struct RemoteDesktopRuntime {
    preference: RemoteDesktopPreference,
    profile_name: Option<String>,
    username: Option<String>,
    remote_target: Option<String>,
    phase: RemoteDesktopPhase,
    width: u16,
    height: u16,
    status: String,
    agent_control_enabled: bool,
    upload_busy: bool,
    upload_status: Option<String>,
    generation: u64,
    input: Option<UnboundedSender<RemoteDesktopInput>>,
    tunnel: SharedTunnel,
}

impl RemoteDesktopRuntime {
    pub(crate) fn new(preference: RemoteDesktopPreference) -> Self {
        Self {
            preference: preference.normalized(),
            profile_name: None,
            username: None,
            remote_target: None,
            phase: RemoteDesktopPhase::Disconnected,
            width: 0,
            height: 0,
            status: "Open a saved SSH profile to configure the Linux desktop".to_owned(),
            agent_control_enabled: false,
            upload_busy: false,
            upload_status: None,
            generation: 0,
            input: None,
            tunnel: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn configure(
        &mut self,
        profile_id: String,
        profile_name: String,
        username: String,
        remote_target: String,
        protocol: RemoteDesktopProtocol,
        remote_port: u16,
    ) {
        let changed = self.preference.profile_id.as_deref() != Some(profile_id.as_str())
            || self.preference.protocol != protocol
            || self.preference.remote_port != remote_port;
        if changed {
            self.disconnect();
            self.upload_busy = false;
            self.upload_status = None;
        }
        self.preference.profile_id = Some(profile_id);
        self.preference.protocol = protocol;
        self.preference.remote_port = remote_port.max(1);
        self.profile_name = Some(profile_name);
        self.username = Some(username);
        self.remote_target = Some(remote_target);
        if self.phase == RemoteDesktopPhase::Disconnected {
            self.status = format!(
                "Ready to create a private {}-over-SSH connection from this device",
                protocol.label()
            );
        }
    }

    pub(crate) fn clear_profile(&mut self, profile_id: &str) {
        if self.preference.profile_id.as_deref() != Some(profile_id) {
            return;
        }
        self.disconnect();
        self.preference.profile_id = None;
        self.profile_name = None;
        self.username = None;
        self.remote_target = None;
        self.status = "Select another SSH profile for the Linux desktop".to_owned();
    }

    pub(crate) fn preference(&self) -> RemoteDesktopPreference {
        self.preference.clone()
    }

    pub(crate) fn configured_profile_id(&self) -> Option<&str> {
        self.preference.profile_id.as_deref()
    }

    pub(crate) fn view(&self) -> RemoteDesktopView<'_> {
        RemoteDesktopView {
            configured: self.preference.profile_id.is_some(),
            phase: self.phase,
            profile_id: self.preference.profile_id.as_deref(),
            profile_name: self.profile_name.as_deref(),
            username: self.username.as_deref(),
            remote_target: self.remote_target.as_deref(),
            protocol: self.preference.protocol,
            remote_port: self.preference.remote_port,
            clipboard_supported: self.preference.protocol.clipboard_supported(),
            width: self.width,
            height: self.height,
            status: &self.status,
            agent_control_enabled: self.agent_control_enabled,
            upload_busy: self.upload_busy,
            upload_status: self.upload_status.as_deref(),
        }
    }

    pub(crate) fn is_connected(&self) -> bool {
        self.phase == RemoteDesktopPhase::Connected && self.width > 0 && self.height > 0
    }

    pub(crate) fn dimensions(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    pub(crate) fn agent_control_enabled(&self) -> bool {
        self.agent_control_enabled
    }

    pub(crate) fn set_agent_control(&mut self, enabled: bool) -> bool {
        let enabled = enabled && self.is_connected();
        let changed = self.agent_control_enabled != enabled;
        self.agent_control_enabled = enabled;
        changed
    }

    pub(crate) fn begin_upload(&mut self, file_count: usize) -> Result<(), String> {
        if !self.is_connected() {
            return Err("Connect the remote Linux desktop before dropping files".to_owned());
        }
        if self.upload_busy {
            return Err("Another VPS file upload is already in progress".to_owned());
        }
        self.upload_busy = true;
        self.upload_status = Some(format!(
            "Uploading {file_count} file{} over the authenticated SSH profile…",
            if file_count == 1 { "" } else { "s" }
        ));
        Ok(())
    }

    pub(crate) fn finish_upload(&mut self, message: impl Into<String>) {
        self.upload_busy = false;
        self.upload_status = Some(message.into());
    }

    pub(crate) fn is_active(&self) -> bool {
        matches!(
            self.phase,
            RemoteDesktopPhase::Connecting | RemoteDesktopPhase::Connected
        )
    }

    pub(crate) fn report_error(&mut self, message: impl Into<String>) {
        self.phase = RemoteDesktopPhase::Error;
        self.status = message.into();
    }

    pub(crate) fn start<F>(
        &mut self,
        spec: SshDesktopTunnelSpec,
        username: String,
        password: String,
        notify: F,
    ) -> Result<(), String>
    where
        F: Fn(RemoteDesktopEvent) + Send + 'static,
    {
        self.disconnect();
        self.generation = self.generation.saturating_add(1);
        let generation = self.generation;
        let protocol = self.preference.protocol;
        let (input_tx, input_rx) = unbounded_channel();
        self.input = Some(input_tx);
        self.phase = RemoteDesktopPhase::Connecting;
        self.width = 0;
        self.height = 0;
        self.status = format!(
            "Creating private SSH tunnel to {} and {} port {}",
            spec.profile_name,
            protocol.label(),
            spec.remote_port
        );
        let tunnel = Arc::new(Mutex::new(None));
        self.tunnel = tunnel.clone();
        let worker = thread::Builder::new()
            .name("central-agent-remote-linux-desktop".to_owned())
            .spawn(move || {
                let runtime = match Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        notify(RemoteDesktopEvent::Stopped {
                            generation,
                            error: Some(format!("Remote desktop runtime could not start: {error}")),
                        });
                        return;
                    }
                };
                let connection = RemoteDesktopConnection {
                    protocol,
                    spec,
                    username,
                    password,
                };
                let result = runtime.block_on(run_remote_desktop(
                    generation,
                    connection,
                    input_rx,
                    tunnel.clone(),
                    &notify,
                ));
                stop_tunnel(&tunnel);
                notify(RemoteDesktopEvent::Stopped {
                    generation,
                    error: result.err(),
                });
            });
        if let Err(error) = worker {
            self.input = None;
            self.phase = RemoteDesktopPhase::Error;
            return Err(format!("Remote desktop worker could not start: {error}"));
        }
        Ok(())
    }

    pub(crate) fn disconnect(&mut self) {
        if let Some(input) = self.input.take() {
            let _ = input.send(RemoteDesktopInput::Disconnect);
        }
        stop_tunnel(&self.tunnel);
        self.generation = self.generation.saturating_add(1);
        self.phase = RemoteDesktopPhase::Disconnected;
        self.agent_control_enabled = false;
        self.width = 0;
        self.height = 0;
        self.status = if self.preference.profile_id.is_some() {
            "Remote Linux desktop disconnected".to_owned()
        } else {
            "Open a saved SSH profile to configure the Linux desktop".to_owned()
        };
    }

    pub(crate) fn refresh(&self) {
        self.send(RemoteDesktopInput::Refresh);
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn pointer(&self, x: u16, y: u16, buttons: u8) {
        self.send(RemoteDesktopInput::Pointer { x, y, buttons });
    }

    pub(crate) fn key(&self, code: String, keysym: u32, down: bool) {
        self.send(RemoteDesktopInput::Key { code, keysym, down });
    }

    pub(crate) fn press_key_with_modifiers(
        &self,
        code: String,
        keysym: u32,
        modifiers: &[(&str, u32)],
    ) -> Result<(), String> {
        if !self.is_connected() {
            return Err("The remote Linux desktop is not connected".to_owned());
        }
        if self.preference.protocol == RemoteDesktopProtocol::Rdp && rdp_scan_code(&code).is_none()
        {
            return Err(format!("The RDP key code {code} is not supported"));
        }
        for (modifier_code, modifier_keysym) in modifiers {
            if self.preference.protocol == RemoteDesktopProtocol::Rdp
                && rdp_scan_code(modifier_code).is_none()
            {
                return Err(format!("The RDP modifier {modifier_code} is not supported"));
            }
            self.key((*modifier_code).to_owned(), *modifier_keysym, true);
        }
        self.key(code.clone(), keysym, true);
        self.key(code, keysym, false);
        for (modifier_code, modifier_keysym) in modifiers.iter().rev() {
            self.key((*modifier_code).to_owned(), *modifier_keysym, false);
        }
        Ok(())
    }

    pub(crate) fn type_text(&self, text: &str) -> Result<(), String> {
        if !self.is_connected() {
            return Err("The remote Linux desktop is not connected".to_owned());
        }
        match self.preference.protocol {
            RemoteDesktopProtocol::Vnc => {
                for character in text.chars() {
                    let keysym = vnc_keysym_for_character(character).ok_or_else(|| {
                        format!(
                            "The character U+{:04X} cannot be typed through VNC",
                            character as u32
                        )
                    })?;
                    self.key(String::new(), keysym, true);
                    self.key(String::new(), keysym, false);
                }
            }
            RemoteDesktopProtocol::Rdp => {
                for character in text.chars() {
                    let (code, shifted, keysym) =
                        rdp_key_for_character(character).ok_or_else(|| {
                            format!(
                                "The character U+{:04X} cannot be typed through RDP",
                                character as u32
                            )
                        })?;
                    if shifted {
                        self.key("ShiftLeft".to_owned(), 0xffe1, true);
                    }
                    self.key(code.to_owned(), keysym, true);
                    self.key(code.to_owned(), keysym, false);
                    if shifted {
                        self.key("ShiftLeft".to_owned(), 0xffe1, false);
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn clipboard(&self, text: String) {
        self.send(RemoteDesktopInput::Clipboard(text));
    }

    fn send(&self, input: RemoteDesktopInput) {
        if let Some(sender) = self.input.as_ref() {
            let _ = sender.send(input);
        }
    }

    pub(crate) fn apply_event(&mut self, event: &RemoteDesktopEvent) -> bool {
        if event.generation() != self.generation {
            return false;
        }
        match event {
            RemoteDesktopEvent::Connected { .. } => {
                self.phase = RemoteDesktopPhase::Connected;
                self.status = format!(
                    "Connected through the private {}-over-SSH tunnel",
                    self.preference.protocol.label()
                );
            }
            RemoteDesktopEvent::Resolution { width, height, .. } => {
                self.width = *width;
                self.height = *height;
                self.status = format!(
                    "Connected through the private {}-over-SSH tunnel · {width} × {height}",
                    self.preference.protocol.label()
                );
            }
            RemoteDesktopEvent::Stopped { error, .. } => {
                self.input = None;
                self.agent_control_enabled = false;
                self.width = 0;
                self.height = 0;
                if let Some(error) = error {
                    self.phase = RemoteDesktopPhase::Error;
                    self.status = error.clone();
                } else {
                    self.phase = RemoteDesktopPhase::Disconnected;
                    self.status = "Remote Linux desktop disconnected".to_owned();
                }
            }
            RemoteDesktopEvent::Draw { .. }
            | RemoteDesktopEvent::Cursor { .. }
            | RemoteDesktopEvent::Clipboard { .. }
            | RemoteDesktopEvent::Bell { .. } => {}
        }
        true
    }
}

fn vnc_keysym_for_character(character: char) -> Option<u32> {
    Some(match character {
        '\n' | '\r' => 0xff0d,
        '\t' => 0xff09,
        '\u{8}' => 0xff08,
        character if !character.is_control() => {
            let codepoint = character as u32;
            if codepoint <= 0xff {
                codepoint
            } else {
                0x0100_0000 | codepoint
            }
        }
        _ => return None,
    })
}

fn rdp_key_for_character(character: char) -> Option<(&'static str, bool, u32)> {
    if character.is_ascii_alphabetic() {
        let upper = character.to_ascii_uppercase();
        let code = match upper {
            'A' => "KeyA",
            'B' => "KeyB",
            'C' => "KeyC",
            'D' => "KeyD",
            'E' => "KeyE",
            'F' => "KeyF",
            'G' => "KeyG",
            'H' => "KeyH",
            'I' => "KeyI",
            'J' => "KeyJ",
            'K' => "KeyK",
            'L' => "KeyL",
            'M' => "KeyM",
            'N' => "KeyN",
            'O' => "KeyO",
            'P' => "KeyP",
            'Q' => "KeyQ",
            'R' => "KeyR",
            'S' => "KeyS",
            'T' => "KeyT",
            'U' => "KeyU",
            'V' => "KeyV",
            'W' => "KeyW",
            'X' => "KeyX",
            'Y' => "KeyY",
            'Z' => "KeyZ",
            _ => return None,
        };
        return Some((code, character.is_ascii_uppercase(), character as u32));
    }
    let (code, shifted) = match character {
        '1' => ("Digit1", false),
        '2' => ("Digit2", false),
        '3' => ("Digit3", false),
        '4' => ("Digit4", false),
        '5' => ("Digit5", false),
        '6' => ("Digit6", false),
        '7' => ("Digit7", false),
        '8' => ("Digit8", false),
        '9' => ("Digit9", false),
        '0' => ("Digit0", false),
        '!' => ("Digit1", true),
        '@' => ("Digit2", true),
        '#' => ("Digit3", true),
        '$' => ("Digit4", true),
        '%' => ("Digit5", true),
        '^' => ("Digit6", true),
        '&' => ("Digit7", true),
        '*' => ("Digit8", true),
        '(' => ("Digit9", true),
        ')' => ("Digit0", true),
        '-' => ("Minus", false),
        '_' => ("Minus", true),
        '=' => ("Equal", false),
        '+' => ("Equal", true),
        '[' => ("BracketLeft", false),
        '{' => ("BracketLeft", true),
        ']' => ("BracketRight", false),
        '}' => ("BracketRight", true),
        '\\' => ("Backslash", false),
        '|' => ("Backslash", true),
        ';' => ("Semicolon", false),
        ':' => ("Semicolon", true),
        '\'' => ("Quote", false),
        '"' => ("Quote", true),
        '`' => ("Backquote", false),
        '~' => ("Backquote", true),
        ',' => ("Comma", false),
        '<' => ("Comma", true),
        '.' => ("Period", false),
        '>' => ("Period", true),
        '/' => ("Slash", false),
        '?' => ("Slash", true),
        ' ' => ("Space", false),
        '\n' | '\r' => ("Enter", false),
        '\t' => ("Tab", false),
        '\u{8}' => ("Backspace", false),
        _ => return None,
    };
    Some((code, shifted, character as u32))
}

impl Drop for RemoteDesktopRuntime {
    fn drop(&mut self) {
        self.disconnect();
    }
}

pub(crate) fn reserve_loopback_port() -> Result<u16, String> {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
        .and_then(|listener| listener.local_addr())
        .map(|address| address.port())
        .map_err(|error| format!("A private local port could not be reserved: {error}"))
}

async fn run_remote_desktop<F>(
    generation: u64,
    connection: RemoteDesktopConnection,
    input: UnboundedReceiver<RemoteDesktopInput>,
    tunnel: SharedTunnel,
    notify: &F,
) -> Result<(), String>
where
    F: Fn(RemoteDesktopEvent),
{
    start_tunnel(&connection.spec, &tunnel)?;
    match connection.protocol {
        RemoteDesktopProtocol::Rdp => {
            run_rdp(
                generation,
                &connection.spec,
                connection.username,
                connection.password,
                input,
                &tunnel,
                notify,
            )
            .await
        }
        RemoteDesktopProtocol::Vnc => {
            run_vnc(
                generation,
                &connection.spec,
                connection.password,
                input,
                &tunnel,
                notify,
            )
            .await
        }
    }
}

async fn wait_for_tunnel(
    spec: &SshDesktopTunnelSpec,
    input: &mut UnboundedReceiver<RemoteDesktopInput>,
    tunnel: &SharedTunnel,
) -> Result<Option<TcpStream>, String> {
    let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, spec.local_port);
    loop {
        if let Some(status) = tunnel_exit_status(tunnel)? {
            return Err(format!(
                "The SSH tunnel closed before the desktop connected ({status}). Verify the host key, private key, and remote listener on 127.0.0.1:{}.",
                spec.remote_port
            ));
        }
        match TcpStream::connect(address).await {
            Ok(stream) => return Ok(Some(stream)),
            Err(_) => {
                tokio::select! {
                    command = input.recv() => {
                        if command.is_none() || matches!(command, Some(RemoteDesktopInput::Disconnect)) {
                            return Ok(None);
                        }
                    }
                    _ = sleep(Duration::from_millis(100)) => {}
                }
            }
        }
    }
}

async fn run_rdp<F>(
    generation: u64,
    spec: &SshDesktopTunnelSpec,
    username: String,
    password: String,
    mut input: UnboundedReceiver<RemoteDesktopInput>,
    tunnel: &SharedTunnel,
    notify: &F,
) -> Result<(), String>
where
    F: Fn(RemoteDesktopEvent),
{
    let Some(probe) = wait_for_tunnel(spec, &mut input, tunnel).await? else {
        return Ok(());
    };
    drop(probe);

    let config = rdp_config(spec.local_port, username, password)?;

    let (output_sender, mut output) = tokio::sync::mpsc::channel(4);
    let client = RdpClient::new(config, output_sender);
    let rdp_input = client.input_sender();
    let client_task = client.run();
    tokio::pin!(client_task);

    let frame_mailbox = Arc::new(RdpFrameMailbox::default());
    let (encoded_sender, mut encoded_output) = tokio::sync::mpsc::channel(1);
    let _encoder_task = tokio::spawn(run_rdp_encoder(frame_mailbox.clone(), encoded_sender));

    let mut connected = false;
    let mut client_done = false;
    let mut width = 0;
    let mut height = 0;
    let mut previous_buttons = 0u8;
    let mut next_frame_id = 0_u64;
    loop {
        tokio::select! {
            () = &mut client_task, if !client_done => {
                client_done = true;
            }
            command = input.recv() => {
                let Some(command) = command else {
                    let _ = rdp_input.send(RdpInputEvent::Close);
                    return Ok(());
                };
                match command {
                    RemoteDesktopInput::Disconnect => {
                        let _ = rdp_input.send(RdpInputEvent::Close);
                        return Ok(());
                    }
                    RemoteDesktopInput::Refresh => {}
                    RemoteDesktopInput::Pointer { x, y, buttons } => {
                        let events = rdp_pointer_events(x, y, buttons, &mut previous_buttons);
                        rdp_input.send(RdpInputEvent::FastPath(events))
                            .map_err(|_| "The RDP input channel closed".to_owned())?;
                    }
                    RemoteDesktopInput::Key { code, down, .. } => {
                        let Some((scan_code, extended)) = rdp_scan_code(&code) else { continue; };
                        let mut flags = if extended { KeyboardFlags::EXTENDED } else { KeyboardFlags::empty() };
                        if !down {
                            flags |= KeyboardFlags::RELEASE;
                        }
                        let mut events = SmallVec::new();
                        events.push(FastPathInputEvent::KeyboardEvent(flags, scan_code));
                        rdp_input.send(RdpInputEvent::FastPath(events))
                            .map_err(|_| "The RDP input channel closed".to_owned())?;
                    }
                    RemoteDesktopInput::Clipboard(_) => {}
                }
            }
            event = output.recv() => {
                let Some(event) = event else {
                    return Err("The RDP display stream ended unexpectedly".to_owned());
                };
                match event {
                    RdpOutputEvent::Image {
                        buffer,
                        x,
                        y,
                        width: region_width,
                        height: region_height,
                        desktop_width,
                        desktop_height,
                    } => {
                        let frame_width = desktop_width.get();
                        let frame_height = desktop_height.get();
                        if !connected {
                            connected = true;
                            notify(RemoteDesktopEvent::Connected { generation });
                        }
                        if width != frame_width || height != frame_height {
                            width = frame_width;
                            height = frame_height;
                            notify(RemoteDesktopEvent::Resolution { generation, width, height });
                        }
                        frame_mailbox.submit(
                            frame_width,
                            frame_height,
                            FrameRegion {
                                x,
                                y,
                                width: region_width.get(),
                                height: region_height.get(),
                            },
                            buffer,
                        )?;
                    }
                    RdpOutputEvent::ConnectionFailure(error) => {
                        return Err(format!("RDP connection failed: {error}"));
                    }
                    RdpOutputEvent::Terminated(result) => {
                        return match result {
                            Ok(_) => Ok(()),
                            Err(error) => {
                                Err(format!("RDP session stopped: {error} · {error:?}"))
                            }
                        };
                    }
                    RdpOutputEvent::PointerDefault => notify(RemoteDesktopEvent::Cursor {
                        generation,
                        cursor: RemoteDesktopCursor::Default,
                    }),
                    RdpOutputEvent::PointerHidden => notify(RemoteDesktopEvent::Cursor {
                        generation,
                        cursor: RemoteDesktopCursor::Hidden,
                    }),
                    RdpOutputEvent::PointerPosition { x, y } => notify(RemoteDesktopEvent::Cursor {
                        generation,
                        cursor: RemoteDesktopCursor::Position { x, y },
                    }),
                    RdpOutputEvent::PointerBitmap(pointer) => notify(RemoteDesktopEvent::Cursor {
                        generation,
                        cursor: RemoteDesktopCursor::Bitmap {
                            width: pointer.width,
                            height: pointer.height,
                            hotspot_x: pointer.hotspot_x,
                            hotspot_y: pointer.hotspot_y,
                            data: BASE64_STANDARD.encode(&pointer.bitmap_data),
                        },
                    }),
                }
            }
            encoded = encoded_output.recv() => {
                let Some(encoded) = encoded else {
                    return Err("The RDP frame encoder stopped unexpectedly".to_owned());
                };
                next_frame_id = next_frame_id.wrapping_add(1);
                notify(RemoteDesktopEvent::Draw {
                    generation,
                    frame_id: next_frame_id,
                    operations: vec![encoded?],
                });
            }
        }
    }
}

async fn run_rdp_encoder(
    mailbox: Arc<RdpFrameMailbox>,
    output: tokio::sync::mpsc::Sender<Result<RemoteDesktopDrawOp, String>>,
) {
    loop {
        let notified = mailbox.wake.notified();
        tokio::pin!(notified);

        match mailbox.take_dirty() {
            Ok(Some(job)) => {
                if !encode_and_send_rdp_job(job, &output).await {
                    return;
                }
                continue;
            }
            Ok(None) => {}
            Err(error) => {
                let _ = output.send(Err(error)).await;
                return;
            }
        }

        let (revision, has_refinement) = match mailbox.refinement_state() {
            Ok(state) => state,
            Err(error) => {
                let _ = output.send(Err(error)).await;
                return;
            }
        };
        if !has_refinement {
            notified.await;
            continue;
        }

        tokio::select! {
            () = &mut notified => {}
            () = sleep(Duration::from_millis(180)) => {
                match mailbox.take_refinement(revision) {
                    Ok(Some(job)) => {
                        if !encode_and_send_rdp_job(job, &output).await {
                            return;
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let _ = output.send(Err(error)).await;
                        return;
                    }
                }
            }
        }
    }
}

async fn encode_and_send_rdp_job(
    job: RdpEncodeJob,
    output: &tokio::sync::mpsc::Sender<Result<RemoteDesktopDrawOp, String>>,
) -> bool {
    let encoded = tokio::task::spawn_blocking(move || encode_rdp_job(job))
        .await
        .map_err(|error| format!("The RDP frame encoder worker stopped: {error}"))
        .and_then(|result| result);
    output.send(encoded).await.is_ok()
}

fn rdp_config(local_port: u16, username: String, password: String) -> Result<Config, String> {
    ConfigBuilder::new()
        .with_destination(Destination::from_parts("127.0.0.1", local_port))
        .with_username(username)
        .with_password(password)
        .with_client_build(26_100)
        .with_client_dir(r"C:\Windows\System32\mstscax.dll")
        .with_client_name("CENTRALAGENT")
        .with_platform(MajorPlatformType::WINDOWS)
        .with_desktop_width(1280)
        .with_desktop_height(720)
        .with_tls(true)
        .with_credssp(false)
        .with_autologon(true)
        .with_server_pointer(true)
        .with_pointer_software_rendering(false)
        // xrdp 0.10 can send its login bitmap in a bulk-compressed form that
        // IronRDP 0.17 currently rejects before the first framebuffer. RemoteFX
        // remains available, while disabling bulk compression keeps this path
        // interoperable with the Debian xrdp server.
        .with_compression(false)
        .build()
        .map_err(|error| format!("RDP configuration could not be created: {error}"))
}

fn encode_rdp_job(job: RdpEncodeJob) -> Result<RemoteDesktopDrawOp, String> {
    let expected_pixels = job.region.area();
    if job.pixels.len() != expected_pixels {
        return Err("The RDP framebuffer dimensions were inconsistent".to_owned());
    }
    let right = u32::from(job.region.x) + u32::from(job.region.width);
    let bottom = u32::from(job.region.y) + u32::from(job.region.height);
    if right > u32::from(job.desktop_width) || bottom > u32::from(job.desktop_height) {
        return Err("The RDP encoded region exceeded the framebuffer".to_owned());
    }
    let mut rgb = Vec::with_capacity(job.pixels.len() * 3);
    for pixel in job.pixels {
        let [_, red, green, blue] = pixel.to_be_bytes();
        rgb.extend_from_slice(&[red, green, blue]);
    }
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, job.quality)
        .encode(
            &rgb,
            u32::from(job.region.width),
            u32::from(job.region.height),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|error| format!("The RDP framebuffer could not be encoded: {error}"))?;
    Ok(RemoteDesktopDrawOp::Jpeg {
        x: job.region.x,
        y: job.region.y,
        width: job.region.width,
        height: job.region.height,
        data: BASE64_STANDARD.encode(jpeg),
    })
}

fn rdp_pointer_events(
    x: u16,
    y: u16,
    buttons: u8,
    previous_buttons: &mut u8,
) -> SmallVec<[FastPathInputEvent; 2]> {
    let mut events = SmallVec::new();
    events.push(FastPathInputEvent::MouseEvent(MousePdu {
        flags: PointerFlags::MOVE,
        number_of_wheel_rotation_units: 0,
        x_position: x,
        y_position: y,
    }));
    let current = buttons & 0b111;
    for (mask, flag) in [
        (1, PointerFlags::LEFT_BUTTON),
        (2, PointerFlags::MIDDLE_BUTTON_OR_WHEEL),
        (4, PointerFlags::RIGHT_BUTTON),
    ] {
        let was_down = *previous_buttons & mask != 0;
        let is_down = current & mask != 0;
        if was_down != is_down {
            events.push(FastPathInputEvent::MouseEvent(MousePdu {
                flags: if is_down {
                    flag | PointerFlags::DOWN
                } else {
                    flag
                },
                number_of_wheel_rotation_units: 0,
                x_position: x,
                y_position: y,
            }));
        }
    }
    *previous_buttons = current;
    if buttons & 8 != 0 || buttons & 16 != 0 {
        events.push(FastPathInputEvent::MouseEvent(MousePdu {
            flags: PointerFlags::VERTICAL_WHEEL,
            number_of_wheel_rotation_units: if buttons & 8 != 0 { 120 } else { -120 },
            x_position: x,
            y_position: y,
        }));
    }
    events
}

fn rdp_scan_code(code: &str) -> Option<(u8, bool)> {
    let scan_code = match code {
        "Escape" => (0x01, false),
        "Digit1" => (0x02, false),
        "Digit2" => (0x03, false),
        "Digit3" => (0x04, false),
        "Digit4" => (0x05, false),
        "Digit5" => (0x06, false),
        "Digit6" => (0x07, false),
        "Digit7" => (0x08, false),
        "Digit8" => (0x09, false),
        "Digit9" => (0x0A, false),
        "Digit0" => (0x0B, false),
        "Minus" => (0x0C, false),
        "Equal" => (0x0D, false),
        "Backspace" => (0x0E, false),
        "Tab" => (0x0F, false),
        "KeyQ" => (0x10, false),
        "KeyW" => (0x11, false),
        "KeyE" => (0x12, false),
        "KeyR" => (0x13, false),
        "KeyT" => (0x14, false),
        "KeyY" => (0x15, false),
        "KeyU" => (0x16, false),
        "KeyI" => (0x17, false),
        "KeyO" => (0x18, false),
        "KeyP" => (0x19, false),
        "BracketLeft" => (0x1A, false),
        "BracketRight" => (0x1B, false),
        "Enter" => (0x1C, false),
        "ControlLeft" => (0x1D, false),
        "KeyA" => (0x1E, false),
        "KeyS" => (0x1F, false),
        "KeyD" => (0x20, false),
        "KeyF" => (0x21, false),
        "KeyG" => (0x22, false),
        "KeyH" => (0x23, false),
        "KeyJ" => (0x24, false),
        "KeyK" => (0x25, false),
        "KeyL" => (0x26, false),
        "Semicolon" => (0x27, false),
        "Quote" => (0x28, false),
        "Backquote" => (0x29, false),
        "ShiftLeft" => (0x2A, false),
        "Backslash" => (0x2B, false),
        "KeyZ" => (0x2C, false),
        "KeyX" => (0x2D, false),
        "KeyC" => (0x2E, false),
        "KeyV" => (0x2F, false),
        "KeyB" => (0x30, false),
        "KeyN" => (0x31, false),
        "KeyM" => (0x32, false),
        "Comma" => (0x33, false),
        "Period" => (0x34, false),
        "Slash" => (0x35, false),
        "ShiftRight" => (0x36, false),
        "NumpadMultiply" => (0x37, false),
        "AltLeft" => (0x38, false),
        "Space" => (0x39, false),
        "CapsLock" => (0x3A, false),
        "F1" => (0x3B, false),
        "F2" => (0x3C, false),
        "F3" => (0x3D, false),
        "F4" => (0x3E, false),
        "F5" => (0x3F, false),
        "F6" => (0x40, false),
        "F7" => (0x41, false),
        "F8" => (0x42, false),
        "F9" => (0x43, false),
        "F10" => (0x44, false),
        "NumLock" => (0x45, false),
        "ScrollLock" => (0x46, false),
        "Numpad7" => (0x47, false),
        "Numpad8" => (0x48, false),
        "Numpad9" => (0x49, false),
        "NumpadSubtract" => (0x4A, false),
        "Numpad4" => (0x4B, false),
        "Numpad5" => (0x4C, false),
        "Numpad6" => (0x4D, false),
        "NumpadAdd" => (0x4E, false),
        "Numpad1" => (0x4F, false),
        "Numpad2" => (0x50, false),
        "Numpad3" => (0x51, false),
        "Numpad0" => (0x52, false),
        "NumpadDecimal" => (0x53, false),
        "IntlBackslash" => (0x56, false),
        "F11" => (0x57, false),
        "F12" => (0x58, false),
        "NumpadEnter" => (0x1C, true),
        "ControlRight" => (0x1D, true),
        "NumpadDivide" => (0x35, true),
        "AltRight" => (0x38, true),
        "Home" => (0x47, true),
        "ArrowUp" => (0x48, true),
        "PageUp" => (0x49, true),
        "ArrowLeft" => (0x4B, true),
        "ArrowRight" => (0x4D, true),
        "End" => (0x4F, true),
        "ArrowDown" => (0x50, true),
        "PageDown" => (0x51, true),
        "Insert" => (0x52, true),
        "Delete" => (0x53, true),
        "MetaLeft" => (0x5B, true),
        "MetaRight" => (0x5C, true),
        "ContextMenu" => (0x5D, true),
        _ => return None,
    };
    Some(scan_code)
}

async fn run_vnc<F>(
    generation: u64,
    spec: &SshDesktopTunnelSpec,
    password: String,
    mut input: UnboundedReceiver<RemoteDesktopInput>,
    tunnel: &SharedTunnel,
    notify: &F,
) -> Result<(), String>
where
    F: Fn(RemoteDesktopEvent),
{
    let Some(stream) = wait_for_tunnel(spec, &mut input, tunnel).await? else {
        return Ok(());
    };
    let client = VncConnector::new(stream)
        .set_auth_method(async move { Ok(password) })
        .add_encoding(VncEncoding::Tight)
        .add_encoding(VncEncoding::Zrle)
        .add_encoding(VncEncoding::CopyRect)
        .add_encoding(VncEncoding::CursorPseudo)
        .add_encoding(VncEncoding::DesktopSizePseudo)
        .add_encoding(VncEncoding::ExtendedDesktopSizePseudo)
        .add_encoding(VncEncoding::Raw)
        .allow_shared(true)
        .set_pixel_format(PixelFormat::rgba())
        .build()
        .map_err(|error| format!("VNC negotiation could not start: {error}"))?
        .try_start()
        .await
        .map_err(|error| format!("VNC authentication or negotiation failed: {error}"))?
        .finish()
        .map_err(|error| format!("VNC connection could not be completed: {error}"))?;

    notify(RemoteDesktopEvent::Connected { generation });
    let mut refresh_tick = interval(Duration::from_millis(33));
    refresh_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    refresh_tick.tick().await;
    let mut draw_tick = interval(Duration::from_millis(8));
    draw_tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    draw_tick.tick().await;
    let draw_mailbox = Arc::new(VncDrawMailbox::default());
    let (encoded_sender, mut encoded_output) = tokio::sync::mpsc::channel(1);
    let _encoder_task = tokio::spawn(run_vnc_encoder(draw_mailbox.clone(), encoded_sender));
    let mut resize_request_pending = false;
    let mut next_frame_id = 0_u64;

    loop {
        tokio::select! {
            command = input.recv() => {
                let Some(command) = command else {
                    let _ = client.close().await;
                    return Ok(());
                };
                match command {
                    RemoteDesktopInput::Disconnect => {
                        let _ = client.close().await;
                        return Ok(());
                    }
                    RemoteDesktopInput::Refresh => client.input(X11Event::FullRefresh).await
                        .map_err(|error| format!("VNC refresh failed: {error}"))?,
                    RemoteDesktopInput::Pointer { x, y, buttons } => client
                        .input(X11Event::PointerEvent((x, y, buttons).into())).await
                        .map_err(|error| format!("VNC pointer input failed: {error}"))?,
                    RemoteDesktopInput::Key { keysym, down, .. } => client
                        .input(X11Event::KeyEvent((keysym, down).into())).await
                        .map_err(|error| format!("VNC keyboard input failed: {error}"))?,
                    RemoteDesktopInput::Clipboard(text) => client.input(X11Event::CopyText(text)).await
                        .map_err(|error| format!("VNC clipboard input failed: {error}"))?,
                }
            }
            _ = refresh_tick.tick() => {
                client.input(X11Event::Refresh).await
                    .map_err(|error| format!("VNC frame refresh failed: {error}"))?;
            }
            _ = draw_tick.tick() => {
                let mut operations = Vec::new();
                loop {
                    let event = client.poll_event().await
                        .map_err(|error| format!("VNC display stream failed: {error}"))?;
                    let Some(event) = event else { break; };
                    match event {
                        VncEvent::SetResolution(screen) => notify(RemoteDesktopEvent::Resolution {
                            generation, width: screen.width, height: screen.height,
                        }),
                        VncEvent::ExtendedDesktopSize { reason, result, width, height, screens } => {
                            if reason == 1 {
                                resize_request_pending = false;
                            }
                            let resize_succeeded = reason != 1 || result == 0;
                            if resize_succeeded
                                && !resize_request_pending
                                && (width != VNC_TARGET_WIDTH || height != VNC_TARGET_HEIGHT)
                                && screens.len() == 1
                            {
                                let screen = screens[0];
                                client.input(X11Event::SetDesktopSize {
                                    width: VNC_TARGET_WIDTH,
                                    height: VNC_TARGET_HEIGHT,
                                    screens: vec![DesktopScreen {
                                        id: screen.id,
                                        x: 0,
                                        y: 0,
                                        width: VNC_TARGET_WIDTH,
                                        height: VNC_TARGET_HEIGHT,
                                        flags: screen.flags,
                                    }],
                                }).await.map_err(|error| {
                                    format!("VNC could not request the 1280x720 desktop size: {error}")
                                })?;
                                resize_request_pending = true;
                            }
                        }
                        VncEvent::RawImage(rect, mut data) => {
                            for pixel in data.chunks_exact_mut(4) { pixel[3] = 255; }
                            operations.push(PendingVncDrawOp::Rgba {
                                region: FrameRegion {
                                    x: rect.x, y: rect.y, width: rect.width, height: rect.height,
                                },
                                data,
                            });
                        }
                        VncEvent::JpegImage(rect, data) => operations.push(PendingVncDrawOp::Jpeg {
                            region: FrameRegion {
                                x: rect.x, y: rect.y, width: rect.width, height: rect.height,
                            },
                            data,
                        }),
                        VncEvent::Copy(destination, source) => operations.push(PendingVncDrawOp::Copy {
                            destination,
                            source,
                        }),
                        VncEvent::SetCursor(rect, data) => notify(RemoteDesktopEvent::Cursor {
                            generation,
                            cursor: if rect.width == 0 || rect.height == 0 {
                                RemoteDesktopCursor::Hidden
                            } else {
                                RemoteDesktopCursor::Bitmap {
                                    width: rect.width,
                                    height: rect.height,
                                    hotspot_x: rect.x,
                                    hotspot_y: rect.y,
                                    data: BASE64_STANDARD.encode(data),
                                }
                            },
                        }),
                        VncEvent::Text(text) => notify(RemoteDesktopEvent::Clipboard { generation, text }),
                        VncEvent::Bell => notify(RemoteDesktopEvent::Bell { generation }),
                        VncEvent::Error(message) => return Err(format!("VNC server error: {message}")),
                        VncEvent::SetPixelFormat(_) => {}
                        _ => {}
                    }
                }
                if !operations.is_empty() {
                    draw_mailbox.submit(operations)?;
                }
            }
            encoded = encoded_output.recv() => {
                let Some(encoded) = encoded else {
                    return Err("The VNC frame encoder stopped unexpectedly".to_owned());
                };
                let operations = encoded?;
                if !operations.is_empty() {
                    next_frame_id = next_frame_id.wrapping_add(1);
                    notify(RemoteDesktopEvent::Draw {
                        generation,
                        frame_id: next_frame_id,
                        operations,
                    });
                }
            }
        }
    }
}

async fn run_vnc_encoder(
    mailbox: Arc<VncDrawMailbox>,
    output: tokio::sync::mpsc::Sender<Result<Vec<RemoteDesktopDrawOp>, String>>,
) {
    loop {
        let notified = mailbox.wake.notified();
        tokio::pin!(notified);
        let operations = match mailbox.take() {
            Ok(operations) => operations,
            Err(error) => {
                let _ = output.send(Err(error)).await;
                return;
            }
        };
        if operations.is_empty() {
            notified.await;
            continue;
        }
        let encoded = tokio::task::spawn_blocking(move || encode_vnc_operations(operations))
            .await
            .map_err(|error| format!("The VNC frame encoder worker stopped: {error}"));
        if output.send(encoded).await.is_err() {
            return;
        }
    }
}

fn encode_vnc_operations(operations: Vec<PendingVncDrawOp>) -> Vec<RemoteDesktopDrawOp> {
    operations
        .into_iter()
        .map(|operation| match operation {
            PendingVncDrawOp::Rgba { region, data } => RemoteDesktopDrawOp::Rgba {
                x: region.x,
                y: region.y,
                width: region.width,
                height: region.height,
                data: BASE64_STANDARD.encode(data),
            },
            PendingVncDrawOp::Jpeg { region, data } => RemoteDesktopDrawOp::Jpeg {
                x: region.x,
                y: region.y,
                width: region.width,
                height: region.height,
                data: BASE64_STANDARD.encode(data),
            },
            PendingVncDrawOp::Copy {
                destination,
                source,
            } => copy_operation(destination, source),
        })
        .collect()
}

fn copy_operation(destination: Rect, source: Rect) -> RemoteDesktopDrawOp {
    RemoteDesktopDrawOp::Copy {
        source_x: source.x,
        source_y: source.y,
        width: source.width,
        height: source.height,
        destination_x: destination.x,
        destination_y: destination.y,
    }
}

fn start_tunnel(spec: &SshDesktopTunnelSpec, tunnel: &SharedTunnel) -> Result<(), String> {
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let child = command
        .spawn()
        .map_err(|error| format!("The SSH tunnel process could not start: {error}"))?;
    let mut guard = tunnel
        .lock()
        .map_err(|_| "The SSH tunnel state is unavailable".to_owned())?;
    *guard = Some(child);
    Ok(())
}

fn tunnel_exit_status(tunnel: &SharedTunnel) -> Result<Option<String>, String> {
    let mut guard = tunnel
        .lock()
        .map_err(|_| "The SSH tunnel state is unavailable".to_owned())?;
    let Some(child) = guard.as_mut() else {
        return Ok(Some("process unavailable".to_owned()));
    };
    child
        .try_wait()
        .map(|status| status.map(|status| status.to_string()))
        .map_err(|error| format!("The SSH tunnel status could not be read: {error}"))
}

fn stop_tunnel(tunnel: &SharedTunnel) {
    let Ok(mut guard) = tunnel.lock() else {
        return;
    };
    let Some(mut child) = guard.take() else {
        return;
    };
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_restore_the_protocol_default_port() {
        let preference = RemoteDesktopPreference {
            profile_id: Some("  profile-1  ".to_owned()),
            protocol: RemoteDesktopProtocol::Rdp,
            remote_port: 0,
        }
        .normalized();
        assert_eq!(preference.profile_id.as_deref(), Some("profile-1"));
        assert_eq!(preference.remote_port, DEFAULT_RDP_PORT);
    }

    #[test]
    fn legacy_preferences_without_a_protocol_remain_vnc() {
        let preference: RemoteDesktopPreference =
            serde_json::from_str(r#"{"profileId":"legacy","remotePort":5901}"#).unwrap();
        assert_eq!(preference.protocol, RemoteDesktopProtocol::Vnc);
        assert_eq!(preference.remote_port, DEFAULT_VNC_PORT);
    }

    #[test]
    fn new_preferences_default_to_rdp() {
        let preference = RemoteDesktopPreference::default();
        assert_eq!(preference.protocol, RemoteDesktopProtocol::Rdp);
        assert_eq!(preference.remote_port, DEFAULT_RDP_PORT);
    }

    #[test]
    fn scan_codes_cover_navigation_and_text_keys() {
        assert_eq!(rdp_scan_code("KeyA"), Some((0x1E, false)));
        assert_eq!(rdp_scan_code("ArrowLeft"), Some((0x4B, true)));
        assert_eq!(rdp_scan_code("Unknown"), None);
    }

    #[test]
    fn text_input_maps_common_rdp_and_unicode_vnc_characters() {
        assert_eq!(rdp_key_for_character('A'), Some(("KeyA", true, 'A' as u32)));
        assert_eq!(
            rdp_key_for_character('?'),
            Some(("Slash", true, '?' as u32))
        );
        assert_eq!(
            rdp_key_for_character('\n'),
            Some(("Enter", false, '\n' as u32))
        );
        assert_eq!(rdp_key_for_character('€'), None);
        assert_eq!(vnc_keysym_for_character('\n'), Some(0xff0d));
        assert_eq!(vnc_keysym_for_character('€'), Some(0x0100_20ac));
    }

    #[test]
    fn rdp_config_avoids_the_xrdp_login_bitmap_bulk_compression_failure() {
        let config = rdp_config(3389, "indexflow".to_owned(), String::new()).unwrap();
        assert!(config.connector().compression_type.is_none());
    }

    #[test]
    fn frame_regions_union_without_losing_dirty_pixels() {
        let first = FrameRegion {
            x: 4,
            y: 8,
            width: 10,
            height: 6,
        };
        let second = FrameRegion {
            x: 10,
            y: 5,
            width: 8,
            height: 12,
        };
        assert_eq!(
            first.union(second),
            FrameRegion {
                x: 4,
                y: 5,
                width: 14,
                height: 12,
            }
        );
    }

    #[test]
    fn rdp_mailbox_coalesces_updates_into_the_latest_region_snapshot() {
        let mailbox = RdpFrameMailbox::default();
        mailbox
            .submit(
                4,
                4,
                FrameRegion {
                    x: 0,
                    y: 0,
                    width: 2,
                    height: 2,
                },
                vec![1; 4],
            )
            .unwrap();
        mailbox
            .submit(
                4,
                4,
                FrameRegion {
                    x: 1,
                    y: 1,
                    width: 2,
                    height: 2,
                },
                vec![2; 4],
            )
            .unwrap();

        let job = mailbox.take_dirty().unwrap().unwrap();
        assert_eq!(
            job.region,
            FrameRegion {
                x: 0,
                y: 0,
                width: 3,
                height: 3,
            }
        );
        assert_eq!(job.pixels, vec![1, 1, 0, 1, 2, 2, 0, 2, 2]);
        assert!(mailbox.take_dirty().unwrap().is_none());
    }

    #[test]
    fn rdp_encoder_preserves_dirty_region_coordinates() {
        let operation = encode_rdp_job(RdpEncodeJob {
            pixels: vec![0x00_FF_00_00; 4],
            region: FrameRegion {
                x: 7,
                y: 9,
                width: 2,
                height: 2,
            },
            desktop_width: 1280,
            desktop_height: 720,
            quality: 70,
        })
        .unwrap();
        match operation {
            RemoteDesktopDrawOp::Jpeg {
                x,
                y,
                width,
                height,
                data,
            } => {
                assert_eq!((x, y, width, height), (7, 9, 2, 2));
                assert!(!data.is_empty());
            }
            _ => panic!("RDP patches must be JPEG operations"),
        }
    }

    #[test]
    fn adaptive_quality_favors_small_updates() {
        let small = motion_jpeg_quality(
            FrameRegion {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            },
            1280,
            720,
        );
        let large = motion_jpeg_quality(
            FrameRegion {
                x: 0,
                y: 0,
                width: 1280,
                height: 720,
            },
            1280,
            720,
        );
        assert!(small > large);
    }

    #[test]
    fn loopback_port_reservation_never_returns_zero() {
        assert_ne!(reserve_loopback_port().unwrap(), 0);
    }
}
