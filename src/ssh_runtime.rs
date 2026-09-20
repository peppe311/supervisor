use std::{
    env,
    ffi::OsString,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

const SSH_BINARY_ENV: &str = "CENTRAL_AGENT_SSH_BIN";
const SCP_BINARY_ENV: &str = "CENTRAL_AGENT_SCP_BIN";
const MAX_UPLOAD_FILES: usize = 64;
const MAX_SSH_PROFILES: usize = 24;
const MAX_PROFILE_NAME_CHARS: usize = 64;
const MAX_HOST_CHARS: usize = 253;
const MAX_USERNAME_CHARS: usize = 64;
const MAX_IDENTITY_PATH_CHARS: usize = 1_024;
const MAX_AUTOMATIC_CONFIG_BYTES: u64 = 64 * 1_024;
const MAX_BATCH_SSH_OUTPUT_BYTES: usize = 4 * 1_024 * 1_024;
pub(crate) const AUTOMATIC_SSH_CONFIG_FILE: &str = "ssh-auto-connect.json";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct SshProfile {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub identity_file: Option<String>,
    pub agent_enabled: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct SshProfileInput {
    pub id: Option<String>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub identity_file: Option<String>,
    pub agent_enabled: bool,
}

#[derive(Debug)]
pub(crate) struct AutomaticSshProfileInput {
    pub input: SshProfileInput,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
struct AutomaticSshConfig {
    profiles: Vec<AutomaticSshProfile>,
}

#[derive(Debug, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct AutomaticSshProfile {
    id: String,
    name: String,
    host: String,
    port: u16,
    username: String,
    identity_file: Option<String>,
    enabled: bool,
    agent_enabled: bool,
}

impl Default for AutomaticSshProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            host: String::new(),
            port: 22,
            username: String::new(),
            identity_file: None,
            enabled: false,
            agent_enabled: false,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SshLaunchSpec {
    pub profile_id: String,
    pub profile_name: String,
    pub target: String,
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub agent_allowed: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct SshBatchCommandSpec {
    pub program: PathBuf,
    pub arguments: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct SshDesktopTunnelSpec {
    pub profile_name: String,
    pub local_port: u16,
    pub remote_port: u16,
    pub program: PathBuf,
    pub arguments: Vec<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct SshUploadSpec {
    prepare: SshBatchCommandSpec,
    program: PathBuf,
    arguments: Vec<OsString>,
    remote_directory: String,
    file_names: Vec<String>,
    total_bytes: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct SshUploadResult {
    pub remote_directory: String,
    pub file_names: Vec<String>,
    pub total_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SshRuntimeView<'a> {
    available: bool,
    executable: Option<String>,
    version: Option<&'a str>,
    detail: &'a str,
    profiles: &'a [SshProfile],
    profile_limit: usize,
    profile_revision: u64,
    message: Option<&'a str>,
    message_is_error: bool,
}

#[derive(Debug, Default)]
struct SshClient {
    executable: Option<PathBuf>,
    version: Option<String>,
    detail: String,
}

#[derive(Debug, Default)]
pub(crate) struct SshRuntime {
    client: SshClient,
    profiles: Vec<SshProfile>,
    message: Option<String>,
    message_is_error: bool,
    profile_revision: u64,
}

impl SshRuntime {
    pub(crate) fn new(profiles: Vec<SshProfile>) -> Self {
        let mut runtime = Self::default();
        for profile in profiles {
            let input = SshProfileInput {
                id: Some(profile.id),
                name: profile.name,
                host: profile.host,
                port: profile.port,
                username: profile.username,
                identity_file: profile.identity_file,
                agent_enabled: profile.agent_enabled,
            };
            let Ok(profile) = normalize_profile(input, false) else {
                continue;
            };
            if runtime
                .profiles
                .iter()
                .any(|existing| existing.id == profile.id)
            {
                continue;
            }
            runtime.profiles.push(profile);
            if runtime.profiles.len() >= MAX_SSH_PROFILES {
                break;
            }
        }
        runtime.refresh_client();
        runtime
    }

    pub(crate) fn refresh_client(&mut self) {
        self.client = detect_ssh_client();
    }

    pub(crate) fn view(&self) -> SshRuntimeView<'_> {
        SshRuntimeView {
            available: self.client.executable.is_some(),
            executable: self
                .client
                .executable
                .as_ref()
                .map(|path| path.display().to_string()),
            version: self.client.version.as_deref(),
            detail: &self.client.detail,
            profiles: &self.profiles,
            profile_limit: MAX_SSH_PROFILES,
            profile_revision: self.profile_revision,
            message: self.message.as_deref(),
            message_is_error: self.message_is_error,
        }
    }

    pub(crate) fn set_message(&mut self, message: impl Into<String>, is_error: bool) {
        self.message = Some(message.into());
        self.message_is_error = is_error;
    }

    pub(crate) fn profiles(&self) -> &[SshProfile] {
        &self.profiles
    }

    pub(crate) fn profile(&self, profile_id: &str) -> Option<&SshProfile> {
        self.profiles
            .iter()
            .find(|profile| profile.id == profile_id)
    }

    pub(crate) fn upsert(&mut self, input: SshProfileInput) -> Result<String, String> {
        let profile = normalize_profile(input, true)?;
        let same_name = self.profiles.iter().any(|existing| {
            existing.id != profile.id && existing.name.eq_ignore_ascii_case(&profile.name)
        });
        if same_name {
            return Err("Another VPS profile already uses this name".to_owned());
        }

        if let Some(index) = self
            .profiles
            .iter()
            .position(|existing| existing.id == profile.id)
        {
            self.profiles[index] = profile.clone();
        } else {
            if self.profiles.len() >= MAX_SSH_PROFILES {
                return Err(format!(
                    "At most {MAX_SSH_PROFILES} VPS profiles can be saved"
                ));
            }
            self.profiles.push(profile.clone());
        }
        self.profile_revision = self.profile_revision.saturating_add(1);
        Ok(profile.id)
    }

    pub(crate) fn remove(&mut self, profile_id: &str) -> Result<SshProfile, String> {
        let profile_id = normalize_profile_id(profile_id)?;
        let index = self
            .profiles
            .iter()
            .position(|profile| profile.id == profile_id)
            .ok_or_else(|| "The selected VPS profile no longer exists".to_owned())?;
        let profile = self.profiles.remove(index);
        self.profile_revision = self.profile_revision.saturating_add(1);
        Ok(profile)
    }

    pub(crate) fn launch_spec(&self, profile_id: &str) -> Result<SshLaunchSpec, String> {
        let profile_id = normalize_profile_id(profile_id)?;
        let profile = self
            .profile(&profile_id)
            .ok_or_else(|| "The selected VPS profile no longer exists".to_owned())?;
        let program = self.client.executable.clone().ok_or_else(|| {
            format!(
                "OpenSSH Client is unavailable. Install it or set {SSH_BINARY_ENV}, then detect it again."
            )
        })?;
        if let Some(identity_file) = profile.identity_file.as_deref() {
            validate_identity_file(identity_file, true)?;
        }
        Ok(SshLaunchSpec {
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
            target: format!("{}@{}:{}", profile.username, profile.host, profile.port),
            program,
            arguments: build_ssh_arguments(profile),
            agent_allowed: profile.agent_enabled,
        })
    }

    pub(crate) fn batch_command_spec(
        &self,
        profile_id: &str,
    ) -> Result<SshBatchCommandSpec, String> {
        let profile_id = normalize_profile_id(profile_id)?;
        let profile = self
            .profile(&profile_id)
            .ok_or_else(|| "The selected VPS profile no longer exists".to_owned())?;
        let program = self.client.executable.clone().ok_or_else(|| {
            format!(
                "OpenSSH Client is unavailable. Install it or set {SSH_BINARY_ENV}, then detect it again."
            )
        })?;
        let identity_file = profile.identity_file.as_deref().ok_or_else(|| {
            "A non-interactive SSH command requires an explicit identity file".to_owned()
        })?;
        validate_identity_file(identity_file, true)?;
        Ok(SshBatchCommandSpec {
            program,
            arguments: build_batch_command_arguments(profile),
        })
    }

    pub(crate) fn key_only_agent_launch_spec(
        &self,
        profile_id: &str,
    ) -> Result<SshLaunchSpec, String> {
        let profile_id = normalize_profile_id(profile_id)?;
        let profile = self
            .profile(&profile_id)
            .ok_or_else(|| "The selected VPS profile no longer exists".to_owned())?;
        if !profile.agent_enabled {
            return Err("Enable Agent access on this server profile first".to_owned());
        }
        let program = self.client.executable.clone().ok_or_else(|| {
            format!(
                "OpenSSH Client is unavailable. Install it or set {SSH_BINARY_ENV}, then detect it again."
            )
        })?;
        let identity_file = profile.identity_file.as_deref().ok_or_else(|| {
            "Automatic agent connection requires an explicit SSH identity file".to_owned()
        })?;
        validate_identity_file(identity_file, true)?;
        Ok(SshLaunchSpec {
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
            target: format!("{}@{}:{}", profile.username, profile.host, profile.port),
            program,
            arguments: build_key_only_agent_arguments(profile),
            agent_allowed: true,
        })
    }

    pub(crate) fn desktop_tunnel_spec(
        &self,
        profile_id: &str,
        local_port: u16,
        remote_port: u16,
    ) -> Result<SshDesktopTunnelSpec, String> {
        if local_port == 0 || remote_port == 0 {
            return Err(
                "The local and remote desktop ports must be between 1 and 65535".to_owned(),
            );
        }
        let profile_id = normalize_profile_id(profile_id)?;
        let profile = self
            .profile(&profile_id)
            .ok_or_else(|| "The selected VPS profile no longer exists".to_owned())?;
        let program = self.client.executable.clone().ok_or_else(|| {
            format!(
                "OpenSSH Client is unavailable. Install it or set {SSH_BINARY_ENV}, then detect it again."
            )
        })?;
        let identity_file = profile.identity_file.as_deref().ok_or_else(|| {
            "Remote Linux Desktop requires an SSH profile with an explicit private-key path"
                .to_owned()
        })?;
        validate_identity_file(identity_file, true)?;
        Ok(SshDesktopTunnelSpec {
            profile_name: profile.name.clone(),
            local_port,
            remote_port,
            program,
            arguments: build_desktop_tunnel_arguments(
                profile,
                identity_file,
                local_port,
                remote_port,
            ),
        })
    }

    pub(crate) fn upload_spec(
        &self,
        profile_id: &str,
        paths: &[PathBuf],
    ) -> Result<SshUploadSpec, String> {
        if paths.is_empty() {
            return Err("Drop at least one local file onto the Linux desktop".to_owned());
        }
        if paths.len() > MAX_UPLOAD_FILES {
            return Err(format!(
                "A single drop can contain at most {MAX_UPLOAD_FILES} regular files"
            ));
        }
        let profile_id = normalize_profile_id(profile_id)?;
        let profile = self
            .profile(&profile_id)
            .ok_or_else(|| "The selected VPS profile no longer exists".to_owned())?;
        let identity_file = profile.identity_file.as_deref().ok_or_else(|| {
            "VPS file upload requires an SSH profile with an explicit private-key path".to_owned()
        })?;
        validate_identity_file(identity_file, true)?;
        let program = detect_scp_client(self.client.executable.as_deref()).ok_or_else(|| {
            format!("OpenSSH scp is unavailable. Install OpenSSH Client or set {SCP_BINARY_ENV}.")
        })?;

        let mut local_paths = Vec::with_capacity(paths.len());
        let mut file_names = Vec::with_capacity(paths.len());
        let mut total_bytes = 0_u64;
        for path in paths {
            if !path.is_absolute() {
                return Err("Dropped file paths must be absolute".to_owned());
            }
            let metadata = fs::symlink_metadata(path)
                .map_err(|_| format!("Dropped file cannot be read: {}", path.display()))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!(
                    "Only regular local files can be uploaded: {}",
                    path.display()
                ));
            }
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .ok_or_else(|| "A dropped file name is not supported".to_owned())?;
            if file_name.chars().any(char::is_control) {
                return Err("A dropped file name contains unsupported characters".to_owned());
            }
            total_bytes = total_bytes.saturating_add(metadata.len());
            local_paths.push(path.as_os_str().to_os_string());
            file_names.push(file_name.to_owned());
        }

        let upload_id = Uuid::new_v4().simple().to_string();
        let relative_directory = format!("CentralAgent-Uploads/{upload_id}");
        let remote_host = if profile.host.contains(':') {
            format!("[{}]", profile.host)
        } else {
            profile.host.clone()
        };
        let destination = format!("{}@{}:{relative_directory}/", profile.username, remote_host);
        let arguments = build_upload_arguments(profile, identity_file, local_paths, destination);
        Ok(SshUploadSpec {
            prepare: self.batch_command_spec(&profile_id)?,
            program,
            arguments,
            remote_directory: format!("~/{relative_directory}"),
            file_names,
            total_bytes,
        })
    }
}

pub(crate) fn run_ssh_upload(spec: SshUploadSpec) -> Result<SshUploadResult, String> {
    let remote_directory = spec.remote_directory.clone();
    let relative_directory = remote_directory
        .strip_prefix("~/")
        .ok_or_else(|| "The upload destination is invalid".to_owned())?;
    run_batch_ssh_command(
        spec.prepare,
        &format!("umask 077\nmkdir -p -- \"$HOME/{relative_directory}\""),
    )?;

    let mut command = Command::new(&spec.program);
    command
        .args(&spec.arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    hide_window(&mut command);
    let output = command
        .output()
        .map_err(|_| "The authenticated VPS file upload could not start".to_owned())?;
    if !output.status.success() {
        let detail = bounded_process_detail(&output.stderr);
        return Err(if detail.is_empty() {
            format!(
                "VPS file upload exited with code {}",
                output.status.code().unwrap_or(-1)
            )
        } else {
            format!("VPS file upload failed: {detail}")
        });
    }
    Ok(SshUploadResult {
        remote_directory,
        file_names: spec.file_names,
        total_bytes: spec.total_bytes,
    })
}

pub(crate) fn load_automatic_ssh_profiles(
    path: &Path,
) -> Result<Vec<AutomaticSshProfileInput>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let metadata = fs::metadata(path)
        .map_err(|_| "Automatic SSH configuration could not be inspected".to_owned())?;
    if !metadata.is_file() || metadata.len() > MAX_AUTOMATIC_CONFIG_BYTES {
        return Err("Automatic SSH configuration is not a bounded regular file".to_owned());
    }
    let bytes =
        fs::read(path).map_err(|_| "Automatic SSH configuration could not be read".to_owned())?;
    let config: AutomaticSshConfig = serde_json::from_slice(&bytes)
        .map_err(|_| "Automatic SSH configuration is invalid".to_owned())?;
    if config.profiles.len() > MAX_SSH_PROFILES {
        return Err(format!(
            "Automatic SSH configuration contains more than {MAX_SSH_PROFILES} profiles"
        ));
    }
    Ok(config
        .profiles
        .into_iter()
        .filter(|profile| profile.enabled)
        .map(|profile| AutomaticSshProfileInput {
            input: SshProfileInput {
                id: Some(profile.id),
                name: profile.name,
                host: profile.host,
                port: profile.port,
                username: profile.username,
                identity_file: profile.identity_file,
                agent_enabled: profile.agent_enabled,
            },
        })
        .collect())
}

pub(crate) fn run_batch_ssh_command(
    spec: SshBatchCommandSpec,
    script: &str,
) -> Result<String, String> {
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_window(&mut command);
    let mut child = command
        .spawn()
        .map_err(|_| "The non-interactive SSH process could not start".to_owned())?;
    let write_result = child
        .stdin
        .take()
        .ok_or_else(|| "The non-interactive SSH input pipe is unavailable".to_owned())
        .and_then(|mut stdin| {
            stdin
                .write_all(script.as_bytes())
                .and_then(|_| stdin.write_all(b"\nexit\n"))
                .map_err(|_| "The metadata request could not be sent over SSH".to_owned())
        });
    if let Err(message) = write_result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(message);
    }
    let output = child
        .wait_with_output()
        .map_err(|_| "The non-interactive SSH process did not finish cleanly".to_owned())?;
    if !output.status.success() {
        let detail = bounded_process_detail(&output.stderr);
        return Err(if detail.is_empty() {
            format!(
                "Non-interactive SSH command exited with code {}",
                output.status.code().unwrap_or(-1)
            )
        } else {
            format!("Non-interactive SSH command failed: {detail}")
        });
    }
    if output.stdout.len() > MAX_BATCH_SSH_OUTPUT_BYTES {
        return Err("The SSH command result exceeded its local size limit".to_owned());
    }
    String::from_utf8(output.stdout)
        .map_err(|_| "The SSH command result was not valid UTF-8".to_owned())
}

fn normalize_profile(
    input: SshProfileInput,
    require_identity_file: bool,
) -> Result<SshProfile, String> {
    let id = match input
        .id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        Some(id) => normalize_profile_id(id)?,
        None => Uuid::new_v4().to_string(),
    };
    let name = normalize_text(&input.name, "Profile name", MAX_PROFILE_NAME_CHARS)?;
    let host = normalize_host(&input.host)?;
    let username = normalize_username(&input.username)?;
    if input.port == 0 {
        return Err("SSH port must be between 1 and 65535".to_owned());
    }
    let identity_file = input
        .identity_file
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(|path| validate_identity_file(path, require_identity_file))
        .transpose()?
        .map(|path| path.display().to_string());
    Ok(SshProfile {
        id,
        name,
        host,
        port: input.port,
        username,
        identity_file,
        agent_enabled: input.agent_enabled,
    })
}

fn normalize_profile_id(value: &str) -> Result<String, String> {
    Uuid::parse_str(value.trim())
        .map(|id| id.to_string())
        .map_err(|_| "The VPS profile identifier is invalid".to_owned())
}

fn normalize_text(value: &str, label: &str, maximum: usize) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > maximum || value.chars().any(char::is_control) {
        return Err(format!(
            "{label} must contain between 1 and {maximum} characters"
        ));
    }
    Ok(value.to_owned())
}

fn normalize_host(value: &str) -> Result<String, String> {
    let value = value.trim();
    let value = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(value);
    let valid = !value.is_empty()
        && value.len() <= MAX_HOST_CHARS
        && !value.starts_with('-')
        && !value.contains("..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':'));
    if !valid {
        return Err(
            "Host must be a hostname or IP address, without a scheme, path, or SSH options"
                .to_owned(),
        );
    }
    Ok(value.to_ascii_lowercase())
}

fn normalize_username(value: &str) -> Result<String, String> {
    let value = value.trim();
    let valid = !value.is_empty()
        && value.chars().count() <= MAX_USERNAME_CHARS
        && !value.starts_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if !valid {
        return Err("SSH username contains unsupported characters".to_owned());
    }
    Ok(value.to_owned())
}

fn validate_identity_file(value: &str, require_file: bool) -> Result<PathBuf, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > MAX_IDENTITY_PATH_CHARS || value.contains('\0') {
        return Err("Identity-file path is invalid".to_owned());
    }
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err("Identity-file path must be absolute".to_owned());
    }
    if require_file {
        let metadata = fs::metadata(&path).map_err(|_| {
            "The selected identity file does not exist or cannot be read".to_owned()
        })?;
        if !metadata.is_file() {
            return Err("The selected identity path is not a file".to_owned());
        }
    }
    Ok(path)
}

fn build_ssh_arguments(profile: &SshProfile) -> Vec<String> {
    let mut arguments = vec![
        "-tt".to_owned(),
        "-F".to_owned(),
        ssh_config_null_path().to_owned(),
        "-p".to_owned(),
        profile.port.to_string(),
        "-l".to_owned(),
        profile.username.clone(),
        "-o".to_owned(),
        "StrictHostKeyChecking=ask".to_owned(),
        "-o".to_owned(),
        "ForwardAgent=no".to_owned(),
        "-o".to_owned(),
        "ForwardX11=no".to_owned(),
        "-o".to_owned(),
        "ClearAllForwardings=yes".to_owned(),
        "-o".to_owned(),
        "PermitLocalCommand=no".to_owned(),
        "-o".to_owned(),
        "ProxyCommand=none".to_owned(),
        "-o".to_owned(),
        "ProxyJump=none".to_owned(),
        "-o".to_owned(),
        "EscapeChar=none".to_owned(),
        "-o".to_owned(),
        "ConnectTimeout=15".to_owned(),
        "-o".to_owned(),
        "ServerAliveInterval=30".to_owned(),
        "-o".to_owned(),
        "ServerAliveCountMax=3".to_owned(),
    ];
    if let Some(identity_file) = profile.identity_file.as_deref() {
        arguments.extend([
            "-o".to_owned(),
            "IdentitiesOnly=yes".to_owned(),
            "-i".to_owned(),
            identity_file.to_owned(),
        ]);
    }
    arguments.push("--".to_owned());
    arguments.push(profile.host.clone());
    arguments
}

fn build_batch_command_arguments(profile: &SshProfile) -> Vec<String> {
    let mut arguments = vec![
        "-T".to_owned(),
        "-F".to_owned(),
        ssh_config_null_path().to_owned(),
        "-p".to_owned(),
        profile.port.to_string(),
        "-l".to_owned(),
        profile.username.clone(),
        "-o".to_owned(),
        "BatchMode=yes".to_owned(),
        "-o".to_owned(),
        "StrictHostKeyChecking=yes".to_owned(),
        "-o".to_owned(),
        "PasswordAuthentication=no".to_owned(),
        "-o".to_owned(),
        "KbdInteractiveAuthentication=no".to_owned(),
        "-o".to_owned(),
        "PreferredAuthentications=publickey".to_owned(),
        "-o".to_owned(),
        "ForwardAgent=no".to_owned(),
        "-o".to_owned(),
        "ForwardX11=no".to_owned(),
        "-o".to_owned(),
        "ClearAllForwardings=yes".to_owned(),
        "-o".to_owned(),
        "PermitLocalCommand=no".to_owned(),
        "-o".to_owned(),
        "ProxyCommand=none".to_owned(),
        "-o".to_owned(),
        "ProxyJump=none".to_owned(),
        "-o".to_owned(),
        "ConnectTimeout=10".to_owned(),
        "-o".to_owned(),
        "ConnectionAttempts=1".to_owned(),
        "-o".to_owned(),
        "ServerAliveInterval=10".to_owned(),
        "-o".to_owned(),
        "ServerAliveCountMax=2".to_owned(),
    ];
    if let Some(identity_file) = profile.identity_file.as_deref() {
        arguments.extend([
            "-o".to_owned(),
            "IdentitiesOnly=yes".to_owned(),
            "-i".to_owned(),
            identity_file.to_owned(),
        ]);
    }
    arguments.extend([
        "--".to_owned(),
        profile.host.clone(),
        "sh".to_owned(),
        "-s".to_owned(),
    ]);
    arguments
}

fn build_desktop_tunnel_arguments(
    profile: &SshProfile,
    identity_file: &str,
    local_port: u16,
    remote_port: u16,
) -> Vec<String> {
    vec![
        "-N".to_owned(),
        "-T".to_owned(),
        "-F".to_owned(),
        ssh_config_null_path().to_owned(),
        "-p".to_owned(),
        profile.port.to_string(),
        "-l".to_owned(),
        profile.username.clone(),
        "-o".to_owned(),
        "BatchMode=yes".to_owned(),
        "-o".to_owned(),
        "StrictHostKeyChecking=yes".to_owned(),
        "-o".to_owned(),
        "PasswordAuthentication=no".to_owned(),
        "-o".to_owned(),
        "KbdInteractiveAuthentication=no".to_owned(),
        "-o".to_owned(),
        "PreferredAuthentications=publickey".to_owned(),
        "-o".to_owned(),
        "IdentitiesOnly=yes".to_owned(),
        "-o".to_owned(),
        "ForwardAgent=no".to_owned(),
        "-o".to_owned(),
        "ForwardX11=no".to_owned(),
        "-o".to_owned(),
        "PermitLocalCommand=no".to_owned(),
        "-o".to_owned(),
        "ProxyCommand=none".to_owned(),
        "-o".to_owned(),
        "ProxyJump=none".to_owned(),
        "-o".to_owned(),
        "ExitOnForwardFailure=yes".to_owned(),
        "-o".to_owned(),
        "ServerAliveInterval=30".to_owned(),
        "-o".to_owned(),
        "ServerAliveCountMax=3".to_owned(),
        "-i".to_owned(),
        identity_file.to_owned(),
        "-L".to_owned(),
        format!("127.0.0.1:{local_port}:127.0.0.1:{remote_port}"),
        "--".to_owned(),
        profile.host.clone(),
    ]
}

fn build_key_only_agent_arguments(profile: &SshProfile) -> Vec<String> {
    let mut arguments = build_ssh_arguments(profile);
    for argument in &mut arguments {
        if argument == "StrictHostKeyChecking=ask" {
            *argument = "StrictHostKeyChecking=yes".to_owned();
        }
    }
    let option_index = arguments
        .iter()
        .position(|argument| argument == "--")
        .unwrap_or(arguments.len());
    arguments.splice(
        option_index..option_index,
        [
            "-o".to_owned(),
            "BatchMode=yes".to_owned(),
            "-o".to_owned(),
            "PasswordAuthentication=no".to_owned(),
            "-o".to_owned(),
            "KbdInteractiveAuthentication=no".to_owned(),
            "-o".to_owned(),
            "PreferredAuthentications=publickey".to_owned(),
        ],
    );
    arguments
}

fn build_upload_arguments(
    profile: &SshProfile,
    identity_file: &str,
    local_paths: Vec<OsString>,
    destination: String,
) -> Vec<OsString> {
    let mut arguments = [
        "-q",
        "-F",
        ssh_config_null_path(),
        "-P",
        &profile.port.to_string(),
        "-o",
        "BatchMode=yes",
        "-o",
        "StrictHostKeyChecking=yes",
        "-o",
        "PasswordAuthentication=no",
        "-o",
        "KbdInteractiveAuthentication=no",
        "-o",
        "PreferredAuthentications=publickey",
        "-o",
        "IdentitiesOnly=yes",
        "-o",
        "ForwardAgent=no",
        "-o",
        "ForwardX11=no",
        "-o",
        "PermitLocalCommand=no",
        "-o",
        "ProxyCommand=none",
        "-o",
        "ProxyJump=none",
        "-i",
        identity_file,
    ]
    .into_iter()
    .map(OsString::from)
    .collect::<Vec<_>>();
    arguments.extend(local_paths);
    arguments.push(OsString::from(destination));
    arguments
}

fn bounded_process_detail(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .chars()
        .filter(|character| !character.is_control() || matches!(character, ' ' | '\n' | '\t'))
        .take(600)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn ssh_config_null_path() -> &'static str {
    if cfg!(windows) { "NUL" } else { "/dev/null" }
}

fn detect_ssh_client() -> SshClient {
    let candidates = ssh_candidates();
    for candidate in candidates {
        if !candidate.is_file() {
            continue;
        }
        let mut command = Command::new(&candidate);
        command.arg("-V");
        hide_window(&mut command);
        match command.output() {
            Ok(output) if output.status.success() => {
                let raw = if output.stderr.is_empty() {
                    output.stdout
                } else {
                    output.stderr
                };
                let version = String::from_utf8_lossy(&raw)
                    .lines()
                    .map(str::trim)
                    .find(|line| !line.is_empty())
                    .unwrap_or("OpenSSH Client")
                    .chars()
                    .take(240)
                    .collect::<String>();
                return SshClient {
                    executable: Some(candidate),
                    version: Some(version),
                    detail: "OpenSSH is ready. Passwords and private-key contents are never stored by Supervisor.".to_owned(),
                };
            }
            Ok(_) | Err(_) => continue,
        }
    }
    SshClient {
        executable: None,
        version: None,
        detail: format!(
            "OpenSSH Client was not found. Install the Windows optional feature or set {SSH_BINARY_ENV}."
        ),
    }
}

fn detect_scp_client(ssh_program: Option<&Path>) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os(SCP_BINARY_ENV).filter(|value| !value.is_empty()) {
        candidates.push(PathBuf::from(path));
    }
    if let Some(ssh_program) = ssh_program
        && let Some(directory) = ssh_program.parent()
    {
        candidates.push(directory.join(if cfg!(windows) { "scp.exe" } else { "scp" }));
    }
    if let Some(path) = env::var_os("PATH") {
        let executable = if cfg!(windows) { "scp.exe" } else { "scp" };
        candidates.extend(env::split_paths(&path).map(|entry| entry.join(executable)));
    }
    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn ssh_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os(SSH_BINARY_ENV).filter(|value| !value.is_empty()) {
        candidates.push(PathBuf::from(path));
    }
    if cfg!(windows)
        && let Some(windows_dir) = env::var_os("WINDIR").filter(|value| !value.is_empty())
    {
        candidates.push(
            PathBuf::from(windows_dir)
                .join("System32")
                .join("OpenSSH")
                .join("ssh.exe"),
        );
    }
    if let Some(path) = env::var_os("PATH") {
        let executable = if cfg!(windows) { "ssh.exe" } else { "ssh" };
        candidates.extend(env::split_paths(&path).map(|entry| entry.join(executable)));
    }
    let mut unique = Vec::new();
    for candidate in candidates {
        if !unique
            .iter()
            .any(|existing: &PathBuf| paths_equal(existing, &candidate))
        {
            unique.push(candidate);
        }
    }
    unique
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}

#[cfg(windows)]
fn hide_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_window(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_input() -> SshProfileInput {
        SshProfileInput {
            id: None,
            name: "Production".to_owned(),
            host: "vps.example.com".to_owned(),
            port: 22,
            username: "deploy".to_owned(),
            identity_file: None,
            agent_enabled: false,
        }
    }

    #[test]
    fn normalizes_a_safe_profile_without_credentials() {
        let profile = normalize_profile(valid_input(), true).unwrap();
        assert!(Uuid::parse_str(&profile.id).is_ok());
        assert_eq!(profile.host, "vps.example.com");
        assert_eq!(profile.username, "deploy");
        assert_eq!(profile.port, 22);
        assert!(profile.identity_file.is_none());
    }

    #[test]
    fn rejects_hosts_that_could_become_ssh_options() {
        for host in [
            "-oProxyCommand=calc",
            "ssh://example.com",
            "name host",
            "a/../b",
        ] {
            let mut input = valid_input();
            input.host = host.to_owned();
            assert!(normalize_profile(input, true).is_err(), "accepted {host}");
        }
    }

    #[test]
    fn launch_arguments_keep_security_sensitive_features_disabled() {
        let profile = normalize_profile(valid_input(), true).unwrap();
        let arguments = build_ssh_arguments(&profile);
        assert!(arguments.windows(2).any(|pair| pair == ["-l", "deploy"]));
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["-F", ssh_config_null_path()])
        );
        assert!(
            arguments
                .iter()
                .any(|value| value == "StrictHostKeyChecking=ask")
        );
        assert!(arguments.iter().any(|value| value == "ForwardAgent=no"));
        assert!(
            arguments
                .iter()
                .any(|value| value == "ClearAllForwardings=yes")
        );
        assert!(arguments.iter().any(|value| value == "ProxyCommand=none"));
        assert_eq!(
            arguments.last().map(String::as_str),
            Some("vps.example.com")
        );
    }

    #[test]
    fn batch_command_arguments_are_key_only_and_non_interactive() {
        let mut profile = normalize_profile(valid_input(), true).unwrap();
        profile.identity_file = Some("C:\\Users\\fixture\\.ssh\\vps-key".to_owned());
        let arguments = build_batch_command_arguments(&profile);

        for required in [
            "BatchMode=yes",
            "StrictHostKeyChecking=yes",
            "PasswordAuthentication=no",
            "KbdInteractiveAuthentication=no",
            "ForwardAgent=no",
            "ClearAllForwardings=yes",
            "ProxyCommand=none",
            "IdentitiesOnly=yes",
        ] {
            assert!(arguments.iter().any(|value| value == required));
        }
        assert_eq!(
            arguments.iter().rev().take(3).cloned().collect::<Vec<_>>(),
            vec!["-s", "sh", "vps.example.com"]
        );
    }

    #[test]
    fn desktop_tunnel_is_key_only_and_loopback_on_both_ends() {
        let mut profile = normalize_profile(valid_input(), true).unwrap();
        profile.identity_file = Some("C:\\Users\\fixture\\.ssh\\vps-key".to_owned());
        let arguments = build_desktop_tunnel_arguments(
            &profile,
            profile.identity_file.as_deref().unwrap(),
            49152,
            5901,
        );

        for required in [
            "-N",
            "BatchMode=yes",
            "StrictHostKeyChecking=yes",
            "PasswordAuthentication=no",
            "KbdInteractiveAuthentication=no",
            "IdentitiesOnly=yes",
            "ExitOnForwardFailure=yes",
        ] {
            assert!(arguments.iter().any(|argument| argument == required));
        }
        assert!(
            arguments
                .windows(2)
                .any(|pair| pair == ["-L", "127.0.0.1:49152:127.0.0.1:5901"])
        );
        assert!(
            !arguments
                .iter()
                .any(|argument| argument == "ClearAllForwardings=yes")
        );
        assert_eq!(
            arguments.last().map(String::as_str),
            Some("vps.example.com")
        );
    }

    #[test]
    fn file_upload_arguments_are_key_only_and_keep_paths_as_separate_arguments() {
        let mut profile = normalize_profile(valid_input(), true).unwrap();
        profile.identity_file = Some("C:\\Users\\fixture\\.ssh\\vps-key".to_owned());
        let arguments = build_upload_arguments(
            &profile,
            profile.identity_file.as_deref().unwrap(),
            vec![
                OsString::from("C:\\Users\\fixture\\Desktop\\first file.txt"),
                OsString::from("C:\\Users\\fixture\\Desktop\\second.bin"),
            ],
            "deploy@vps.example.com:CentralAgent-Uploads/test/".to_owned(),
        );
        let values = arguments
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        for required in [
            "BatchMode=yes",
            "StrictHostKeyChecking=yes",
            "PasswordAuthentication=no",
            "KbdInteractiveAuthentication=no",
            "IdentitiesOnly=yes",
            "ProxyCommand=none",
        ] {
            assert!(values.iter().any(|value| value == required));
        }
        assert!(
            values
                .iter()
                .any(|value| value == "C:\\Users\\fixture\\Desktop\\first file.txt")
        );
        assert_eq!(
            values.last().map(String::as_str),
            Some("deploy@vps.example.com:CentralAgent-Uploads/test/")
        );
    }

    #[test]
    fn loads_only_enabled_bounded_automatic_profiles() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(AUTOMATIC_SSH_CONFIG_FILE);
        fs::write(
            &path,
            r#"{
              "profiles": [
                {
                  "id": "5cb7a263-3335-4ce9-b920-24f70b85faf0",
                  "name": "Debian VPS",
                  "host": "203.0.113.7",
                  "port": 22,
                  "username": "deploy",
                  "identityFile": "C:\\keys\\debian",
                  "enabled": true,
                  "agentEnabled": true
                },
                {
                  "id": "e21525d5-67bb-4dd9-a863-49a3d1ca1438",
                  "name": "Disabled",
                  "host": "203.0.113.8",
                  "port": 22,
                  "username": "deploy",
                  "enabled": false
                }
              ]
            }"#,
        )
        .unwrap();

        let profiles = load_automatic_ssh_profiles(&path).unwrap();

        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].input.name, "Debian VPS");
        assert!(profiles[0].input.agent_enabled);
    }

    #[test]
    fn key_only_agent_connection_is_non_interactive_and_keeps_a_remote_tty() {
        let mut profile = normalize_profile(valid_input(), true).unwrap();
        profile.agent_enabled = true;
        profile.identity_file = Some("C:\\Users\\fixture\\.ssh\\vps-key".to_owned());
        let arguments = build_key_only_agent_arguments(&profile);

        assert_eq!(arguments.first().map(String::as_str), Some("-tt"));
        for required in [
            "BatchMode=yes",
            "StrictHostKeyChecking=yes",
            "PasswordAuthentication=no",
            "KbdInteractiveAuthentication=no",
            "PreferredAuthentications=publickey",
        ] {
            assert!(arguments.iter().any(|argument| argument == required));
        }
        assert!(
            !arguments
                .iter()
                .any(|argument| argument == "StrictHostKeyChecking=ask")
        );
        assert_eq!(
            arguments.last().map(String::as_str),
            Some("vps.example.com")
        );
    }

    #[test]
    #[ignore = "requires an explicitly selected local configuration and reachable SSH host"]
    fn validates_an_explicit_external_automatic_profile_end_to_end() {
        let path = env::var_os("CENTRAL_AGENT_TEST_AUTO_SSH_CONFIG")
            .map(PathBuf::from)
            .expect("CENTRAL_AGENT_TEST_AUTO_SSH_CONFIG must select the configuration");
        let profiles = load_automatic_ssh_profiles(&path).unwrap();
        assert!(!profiles.is_empty());

        let mut runtime = SshRuntime::new(Vec::new());
        for profile in profiles {
            let profile_id = runtime.upsert(profile.input).unwrap();
            let spec = runtime.batch_command_spec(&profile_id).unwrap();
            let output =
                run_batch_ssh_command(spec, "printf 'central-agent-ssh-ready\\n'\n").unwrap();
            assert_eq!(output.trim(), "central-agent-ssh-ready");
        }
    }

    #[test]
    fn upsert_prevents_ambiguous_profile_names() {
        let mut runtime = SshRuntime::default();
        runtime.upsert(valid_input()).unwrap();
        let mut duplicate = valid_input();
        duplicate.host = "other.example.com".to_owned();
        assert!(runtime.upsert(duplicate).is_err());
    }

    #[test]
    fn loading_does_not_erase_a_profile_when_its_key_is_temporarily_missing() {
        let mut profile = normalize_profile(valid_input(), true).unwrap();
        profile.identity_file = Some(
            env::temp_dir()
                .join(format!("central-agent-missing-test-key-{}", Uuid::new_v4()))
                .display()
                .to_string(),
        );

        let runtime = SshRuntime::new(vec![profile]);

        assert_eq!(runtime.profiles().len(), 1);
        assert!(runtime.launch_spec(&runtime.profiles()[0].id).is_err());
    }
}
