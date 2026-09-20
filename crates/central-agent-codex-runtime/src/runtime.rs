//! Resolve only an explicit, packaged, public PATH or official desktop-app
//! executable. Never read Codex credential files, parse private application
//! state, or install software.
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
};

// Existing wire fixtures retain their baseline version. The shared manifest
// lists every exact runtime whose generated protocol passes verification.
pub const TESTED_VERSION: &str = "0.155.1";
pub const EXECUTABLE_OVERRIDE: &str = "CENTRAL_AGENT_CODEX_BIN";

fn supported_versions() -> &'static [String] {
    static VERSIONS: OnceLock<Vec<String>> = OnceLock::new();
    VERSIONS.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../protocol/app-server/supported-versions.json"
        ))
        .expect("The checked-in supported Codex versions must be valid JSON")
    })
}

fn validate_version(version: &str) -> Result<(), String> {
    if supported_versions()
        .iter()
        .any(|supported| supported == version)
    {
        Ok(())
    } else {
        Err(format!(
            "Installed Codex {version} is not supported by this Supervisor build. Supported versions: {}. Update Supervisor to connect with this Codex version.",
            supported_versions().join(", ")
        ))
    }
}

#[derive(Clone, Debug)]
pub struct Runtime {
    executable: PathBuf,
    cwd: PathBuf,
    version: String,
    home: Option<PathBuf>,
    isolate_storage: bool,
}

impl Runtime {
    pub fn discover(cwd: &Path) -> Result<Self, String> {
        let app = env::current_exe()
            .map_err(|e| format!("Could not resolve the application executable: {e}"))?;
        let paths = env::var_os("PATH");
        let explicit = env::var_os(EXECUTABLE_OVERRIDE);
        if explicit.as_ref().is_some_and(|value| !value.is_empty()) {
            let executable = resolve_executable(
                explicit.as_deref(),
                &app,
                paths.as_deref().map(env::split_paths).into_iter().flatten(),
            )?;
            // A user-selected executable is authoritative: never hide a bad
            // selection by falling through to another installation/account.
            return Self::from_executable(&executable, cwd);
        }

        let mut candidates = automatic_candidates(
            &app,
            paths.as_deref().map(env::split_paths).into_iter().flatten(),
        );
        for candidate in official_desktop_candidates() {
            push_unique(&mut candidates, candidate);
        }
        if candidates.is_empty() {
            return Err(format!(
                "Codex executable not found. Install the official CLI or desktop app, or set {EXECUTABLE_OVERRIDE}."
            ));
        }
        let mut failures = Vec::new();
        for executable in candidates {
            match Self::from_executable(&executable, cwd) {
                Ok(runtime) => return Ok(runtime),
                Err(error) => failures.push(error),
            }
        }
        Err(format!(
            "No compatible Codex installation was found. {}",
            failures
                .last()
                .cloned()
                .unwrap_or_else(|| "Verify the Codex installation.".into())
        ))
    }

    pub fn from_executable(executable: &Path, cwd: &Path) -> Result<Self, String> {
        if !executable.is_absolute() || !executable.is_file() {
            return Err("Select an existing absolute Codex executable path".into());
        }
        if !cwd.is_absolute() || !cwd.is_dir() {
            return Err("App Server requires an existing absolute working directory".into());
        }
        let executable = executable
            .canonicalize()
            .map_err(|e| format!("Codex executable is unavailable: {e}"))?;
        let mut command = Command::new(&executable);
        hide_window(&mut command);
        let output = command
            .arg("--version")
            .current_dir(cwd)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("Could not check the Codex version: {e}"))?;
        if !output.status.success() {
            return Err("Codex --version failed; verify the selected executable".into());
        }
        let text = String::from_utf8(output.stdout)
            .map_err(|_| "Codex returned an invalid version string")?;
        let version = text
            .trim()
            .strip_prefix("codex-cli ")
            .ok_or("The executable did not identify itself as Codex CLI")?
            .to_owned();
        validate_version(&version)?;
        Ok(Self {
            executable,
            cwd: cwd.to_owned(),
            version,
            home: None,
            isolate_storage: true,
        })
    }

    /// Scope this child process to a dedicated native profile. Never change the
    /// environment of Supervisor, the user's shell, or another Codex client.
    pub fn with_home(mut self, home: &Path) -> Result<Self, String> {
        if !home.is_absolute() || !home.is_dir() {
            return Err(
                "The dedicated Codex profile must be an existing absolute directory".into(),
            );
        }
        self.home = Some(home.canonicalize().map_err(|error| error.to_string())?);
        self.isolate_storage = true;
        Ok(self)
    }

    /// Read-only migration inspection of an explicitly identified old profile.
    /// Retain its native database and credential configuration. Product work
    /// must use with_home; this mode is only for public metadata reads.
    pub fn with_existing_home(self, home: &Path) -> Result<Self, String> {
        let mut runtime = self.with_home(home)?;
        runtime.isolate_storage = false;
        Ok(runtime)
    }

    pub fn home(&self) -> Option<&Path> {
        self.home.as_deref()
    }

    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub(crate) fn scoped_command(&self) -> Command {
        let mut command = Command::new(&self.executable);
        if let Some(home) = &self.home {
            command.env("CODEX_HOME", home);
            if self.isolate_storage {
                // SQLite and credentials must not escape through inherited paths or
                // a project config. Native administrator requirements still apply.
                let home_value = serde_json::to_string(&home.to_string_lossy()).unwrap();
                command
                    .env("CODEX_SQLITE_HOME", home)
                    .args(["-c", &format!("sqlite_home={home_value}")])
                    .args(["-c", "cli_auth_credentials_store=\"file\""]);
            }
        }
        command.current_dir(&self.cwd);
        hide_window(&mut command);
        command
    }

    fn base_command(&self) -> Command {
        let mut command = self.scoped_command();
        command
            .args(["app-server", "--listen", "stdio://"])
            .current_dir(&self.cwd);
        command
    }

    #[cfg(test)]
    pub(crate) fn command(&self) -> Command {
        let mut command = self.base_command();
        hide_window(&mut command);
        command
    }

    pub(crate) fn server_command(&self) -> (Command, bool) {
        let mut command = self.base_command();
        let prepare_hidden_console = prepare_process_tree_window(&mut command);
        (command, prepare_hidden_console)
    }
}

fn resolve_executable(
    explicit: Option<&std::ffi::OsStr>,
    app: &Path,
    paths: impl IntoIterator<Item = PathBuf>,
) -> Result<PathBuf, String> {
    if let Some(explicit) = explicit.filter(|value| !value.is_empty()) {
        let path = PathBuf::from(explicit);
        if !path.is_absolute() {
            return Err(format!(
                "{EXECUTABLE_OVERRIDE} must be an absolute executable path"
            ));
        }
        // A bad explicit choice is an error, never a quiet fallback to another account/runtime.
        return Ok(path);
    }
    automatic_candidates(app, paths)
        .into_iter()
        .next()
        .ok_or_else(|| {
            format!(
                "Codex executable not found. Install the official CLI or set {EXECUTABLE_OVERRIDE}."
            )
        })
}

fn automatic_candidates(app: &Path, paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let name = if cfg!(windows) { "codex.exe" } else { "codex" };
    let mut candidates = Vec::new();
    if let Some(directory) = app.parent() {
        let bundled = directory.join("resources").join("codex").join(name);
        if bundled.is_file() {
            push_unique(&mut candidates, bundled);
        }
    }
    for directory in paths
        .into_iter()
        .filter(|directory| directory.is_absolute())
    {
        let candidate = directory.join(name);
        if candidate.is_file() {
            push_unique(&mut candidates, candidate);
        }
    }
    candidates
}

fn push_unique(candidates: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !candidates.iter().any(|known| known == &candidate) {
        candidates.push(candidate);
    }
}

#[cfg(windows)]
fn official_desktop_candidates() -> Vec<PathBuf> {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map_or_else(Vec::new, |root| desktop_candidates_at(&root))
}

#[cfg(not(windows))]
fn official_desktop_candidates() -> Vec<PathBuf> {
    Vec::new()
}

/// The official Windows app keeps versioned binaries below this public install
/// directory. Enumeration is deliberately one level deep and never touches
/// credentials, sessions, configuration or other private Codex state.
fn desktop_candidates_at(local_app_data: &Path) -> Vec<PathBuf> {
    let root = local_app_data.join("OpenAI").join("Codex").join("bin");
    let Ok(canonical_root) = root.canonicalize() else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut candidates = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter_map(|entry| {
            let candidate = entry.path().join("codex.exe");
            let canonical = candidate.canonicalize().ok()?;
            (canonical.starts_with(&canonical_root) && canonical.is_file()).then(|| {
                let modified = candidate.metadata().and_then(|value| value.modified()).ok();
                (modified, canonical)
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    candidates.into_iter().map(|(_, path)| path).collect()
}

pub(crate) fn hide_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW, including diagnostics.
    }
    #[cfg(not(windows))]
    let _ = command;
}

/// A GUI host has no console for child tools to inherit. The transport uses a
/// no-console Win32 startup in that case so Windows Terminal cannot surface a
/// window for the server or its descendants. Console-attached tests and CLI
/// hosts keep their existing console and use the standard library process path.
pub(crate) fn prepare_process_tree_window(_command: &mut Command) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::System::Console::GetConsoleCP;
        if unsafe { GetConsoleCP() } == 0 {
            return true;
        }
    }
    #[cfg(not(windows))]
    let _ = _command;
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verified_runtime_versions_accept_the_updated_desktop_without_accepting_unknown_builds() {
        assert!(validate_version(TESTED_VERSION).is_ok());
        assert!(validate_version("0.154.0-alpha.6.2").is_ok());
        assert!(validate_version("0.155.0-alpha.2.6").is_ok());
        let mut versions = supported_versions().to_vec();
        versions.sort();
        versions.dedup();
        assert_eq!(versions.len(), supported_versions().len());
        for version in [
            "",
            "0.153.3",
            "0.154.0-alpha.6.1",
            "0.154.0-alpha.6.3",
            "0.154.0",
            "9.0.0",
            "codex-cli 0.153.4",
        ] {
            let error = validate_version(version).unwrap_err();
            assert!(error.contains(version));
            assert!(error.contains("Update Supervisor"));
        }
    }

    #[test]
    fn dedicated_profile_overrides_child_storage_without_changing_parent_environment() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("Supervisor profile");
        fs::create_dir(&home).unwrap();
        let parent_home = env::var_os("CODEX_HOME");
        let runtime = Runtime {
            executable: temp.path().join("codex.exe"),
            cwd: temp.path().into(),
            version: TESTED_VERSION.into(),
            home: None,
            isolate_storage: true,
        };
        assert!(runtime.command().get_envs().next().is_none());
        assert!(
            runtime
                .clone()
                .with_home(Path::new("relative-profile"))
                .is_err()
        );
        let source = runtime.clone().with_existing_home(&home).unwrap();
        assert_eq!(
            source.command().get_args().collect::<Vec<_>>(),
            ["app-server", "--listen", "stdio://"]
        );
        assert!(
            !source
                .command()
                .get_envs()
                .any(|(key, _)| key == "CODEX_SQLITE_HOME")
        );
        assert!(
            source
                .with_home(&home)
                .unwrap()
                .command()
                .get_envs()
                .any(|(key, _)| key == "CODEX_SQLITE_HOME")
        );
        let runtime = runtime.with_home(&home).unwrap();
        let command = runtime.command();
        let canonical = home.canonicalize().unwrap();
        for name in ["CODEX_HOME", "CODEX_SQLITE_HOME"] {
            assert!(
                command
                    .get_envs()
                    .any(|(key, value)| key == name && value == Some(canonical.as_os_str()))
            );
        }
        let args = command
            .get_args()
            .map(|arg| arg.to_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(args[0], "-c");
        assert_eq!(
            args[1],
            format!(
                "sqlite_home={}",
                serde_json::to_string(&canonical.to_string_lossy()).unwrap()
            )
        );
        assert_eq!(
            &args[2..],
            [
                "-c",
                "cli_auth_credentials_store=\"file\"",
                "app-server",
                "--listen",
                "stdio://"
            ]
        );
        assert_eq!(runtime.home(), Some(canonical.as_path()));
        assert_eq!(env::var_os("CODEX_HOME"), parent_home);
    }

    #[test]
    fn discovery_precedence_is_explicit_then_packaged_then_absolute_path() {
        let temp = tempfile::tempdir().unwrap();
        let app = temp.path().join("CentralAgent.exe");
        let packaged = temp.path().join("resources/codex");
        let public = temp.path().join("bin");
        std::fs::create_dir_all(&packaged).unwrap();
        std::fs::create_dir(&public).unwrap();
        let name = if cfg!(windows) { "codex.exe" } else { "codex" };
        std::fs::write(public.join(name), []).unwrap();
        assert_eq!(
            resolve_executable(None, &app, [public.clone()]).unwrap(),
            public.join(name)
        );
        std::fs::write(packaged.join(name), []).unwrap();
        assert_eq!(
            resolve_executable(None, &app, [public.clone()]).unwrap(),
            packaged.join(name)
        );
        let missing = temp.path().join("explicit-not-installed.exe");
        assert_eq!(
            resolve_executable(Some(missing.as_os_str()), &app, [public]).unwrap(),
            missing
        );
        assert!(resolve_executable(Some(std::ffi::OsStr::new("relative.exe")), &app, []).is_err());
        let app = temp.path().join("other/app.exe");
        assert!(resolve_executable(None, &app, [PathBuf::from(".")]).is_err());
    }

    #[test]
    fn desktop_discovery_is_bounded_to_version_directories() {
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("OpenAI/Codex/bin");
        let current = bin.join("current-build");
        let older = bin.join("older-build");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::create_dir_all(&older).unwrap();
        std::fs::write(current.join("codex.exe"), []).unwrap();
        std::fs::write(older.join("codex.exe"), []).unwrap();
        std::fs::write(bin.join("codex.exe"), []).unwrap();
        std::fs::create_dir_all(bin.join("not-a-build/nested")).unwrap();
        std::fs::write(bin.join("not-a-build/nested/codex.exe"), []).unwrap();

        let candidates = desktop_candidates_at(temp.path());
        let current = current.canonicalize().unwrap();
        let older = older.canonicalize().unwrap();
        assert_eq!(candidates.len(), 2);
        assert!(candidates.iter().all(|path| {
            path.parent()
                .is_some_and(|parent| parent == current || parent == older)
        }));
    }
}
