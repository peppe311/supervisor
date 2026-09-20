//! Native metadata and an immutable, read-only snapshot of the bound Git repository.
use crate::{api::GitInfoUpdate, thread_sections::Section};
use serde::Serialize;
use serde_json::Value;
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitMetadata {
    pub sha: Option<String>,
    pub branch: Option<String>,
    pub origin_url: Option<String>,
}

impl GitMetadata {
    pub fn read(value: &Value) -> Result<Option<Self>, String> {
        if value.is_null() {
            return Ok(None);
        }
        if !value.is_object() {
            return Err("Invalid Git metadata".into());
        }
        let sha = optional_text(value, "sha", 64)?;
        if sha.as_ref().is_some_and(|s| !valid_sha(s)) {
            return Err("Invalid Git commit identity".into());
        }
        let branch = optional_text(value, "branch", 256)?;
        let origin = optional_text(value, "originUrl", 4096)?;
        Ok(Some(Self {
            sha,
            branch,
            origin_url: origin.as_deref().and_then(sanitize_origin),
        }))
    }

    pub fn update(&self) -> GitInfoUpdate {
        GitInfoUpdate {
            sha: Some(self.sha.clone()),
            branch: Some(self.branch.clone()),
            origin_url: Some(self.origin_url.clone()),
        }
    }
}

/// Rust-only metadata. Paths are used for binding verification, never links or commands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeMetadata {
    pub cwd: PathBuf,
    pub git: Option<GitMetadata>,
    pub section: Option<Section>,
}
impl NativeMetadata {
    pub fn read(thread: &Value) -> Result<Self, String> {
        let cwd = thread["cwd"]
            .as_str()
            .filter(|s| text(s, 16 * 1024))
            .ok_or("Thread metadata omitted its directory")?;
        if !Path::new(cwd).is_absolute() {
            return Err("Thread metadata requires an absolute directory".into());
        }
        Ok(Self {
            cwd: cwd.into(),
            git: GitMetadata::read(&thread["gitInfo"])?,
            section: (!thread["section"].is_null())
                .then(|| Section::read(&thread["section"]))
                .transpose()?,
        })
    }
}

/// Cannot be supplied through IPC. Capture and compare again immediately before dispatch.
#[derive(Debug, PartialEq, Eq)]
pub struct RepositorySnapshot {
    cwd: PathBuf,
    root: PathBuf,
    git: GitMetadata,
}
impl RepositorySnapshot {
    pub fn capture(cwd: &Path) -> Result<Self, String> {
        if !cwd.is_absolute() || !cwd.is_dir() {
            return Err("Select an existing local directory".into());
        }
        let cwd = cwd
            .canonicalize()
            .map_err(|_| "Repository directory is unavailable")?;
        let root = PathBuf::from(
            git_read(&cwd, &["rev-parse", "--show-toplevel"], false)?
                .ok_or("The directory is not a Git worktree")?,
        )
        .canonicalize()
        .map_err(|_| "Git worktree root is unavailable")?;
        if !cwd.starts_with(&root) {
            return Err("Git resolved a different worktree".into());
        }
        let sha = git_read(&cwd, &["rev-parse", "--verify", "--quiet", "HEAD"], true)?;
        if sha.as_ref().is_some_and(|s| !valid_sha(s)) {
            return Err("Invalid repository commit".into());
        }
        let branch = git_read(&cwd, &["symbolic-ref", "--quiet", "--short", "HEAD"], true)?;
        if branch.as_ref().is_some_and(|s| !text(s, 256)) {
            return Err("Invalid repository branch".into());
        }
        // Read the repository's origin only. URL rewriting or credential helpers are not run.
        let origin_url = git_read(
            &cwd,
            &["config", "--local", "--get", "remote.origin.url"],
            true,
        )?
        .as_deref()
        .and_then(sanitize_origin);
        Ok(Self {
            cwd,
            root,
            git: GitMetadata {
                sha,
                branch,
                origin_url,
            },
        })
    }
    pub fn directory(&self) -> &Path {
        &self.cwd
    }
    pub fn metadata(&self) -> &GitMetadata {
        &self.git
    }
    pub fn revalidate(&self, cwd: &Path) -> Result<(), String> {
        if &Self::capture(cwd)? != self {
            return Err("Repository changed; prepare a new metadata confirmation".into());
        }
        Ok(())
    }
}

pub(crate) fn text(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}
pub(crate) fn optional_text(
    value: &Value,
    field: &str,
    max: usize,
) -> Result<Option<String>, String> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if text(s, max) => Ok(Some(s.clone())),
        _ => Err(format!("Invalid native metadata field: {field}")),
    }
}
fn valid_sha(s: &str) -> bool {
    matches!(s.len(), 40 | 64) && s.bytes().all(|c| c.is_ascii_hexdigit())
}
pub fn sanitize_origin(value: &str) -> Option<String> {
    if !text(value, 4096) {
        return None;
    }
    let converted;
    let value = if !value.contains("://") {
        // SCP-like SSH origins. Omit the user portion along with any URL credentials.
        let (host, path) = value.split_once(':')?;
        let host = host.rsplit('@').next()?;
        if host.is_empty()
            || host.len() == 1
            || host.contains(['/', '\\'])
            || path.is_empty()
            || [
                "javascript",
                "vbscript",
                "data",
                "file",
                "mailto",
                "http",
                "https",
            ]
            .iter()
            .any(|scheme| host.eq_ignore_ascii_case(scheme))
        {
            return None;
        }
        converted = format!("ssh://{host}/{}", path.trim_start_matches('/'));
        &converted
    } else {
        value
    };
    let mut url = url::Url::parse(value).ok()?;
    if !matches!(url.scheme(), "https" | "http" | "ssh" | "git") || url.host_str().is_none() {
        return None;
    }
    url.set_password(None).ok()?;
    url.set_username("").ok()?;
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string())
}

fn git_read(cwd: &Path, args: &[&str], allow_absent: bool) -> Result<Option<String>, String> {
    // Resolve an absolute PATH executable before changing cwd; Windows must
    // not discover a repository-local git.exe through its current-directory search.
    let paths = std::env::var_os("PATH").ok_or("Git is unavailable on PATH")?;
    let executable = std::env::split_paths(&paths)
        .filter(|p| p.is_absolute())
        .map(|p| p.join(if cfg!(windows) { "git.exe" } else { "git" }))
        .find(|p| p.is_file())
        .ok_or("Git is unavailable on PATH")?;
    let mut command = Command::new(executable);
    crate::runtime::hide_window(&mut command);
    for (key, _) in std::env::vars_os() {
        if key
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GIT_")
        {
            command.env_remove(key);
        }
    }
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_TERMINAL_PROMPT", "0")
        .args([
            "--no-pager",
            "--no-optional-locks",
            "-c",
            "core.fsmonitor=false",
        ])
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|_| "Git metadata reader could not start")?;
    let stdout = child
        .stdout
        .take()
        .ok_or("Git output pipe is unavailable")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.take(8193).read_to_end(&mut bytes).map(|_| bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break Err("Git metadata read timed out or failed");
            }
        }
    };
    let bytes = reader
        .join()
        .map_err(|_| "Git metadata reader failed")?
        .map_err(|_| "Git metadata read failed")?;
    let status = status?;
    if allow_absent && status.code() == Some(1) {
        return Ok(None);
    }
    if !status.success() || bytes.len() > 8192 {
        return Err("Git metadata read failed or exceeded its bound".into());
    }
    let value = String::from_utf8(bytes).map_err(|_| "Git metadata is not UTF-8")?;
    let value = value.trim_end_matches(['\r', '\n']);
    if !text(value, 8192) {
        return Err("Git returned invalid metadata".into());
    }
    Ok(Some(value.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn origins_strip_credentials_queries_and_fragments_and_reject_local_or_active_urls() {
        assert_eq!(
            sanitize_origin("https://user:secret@example.test/org/repo.git?token=private#private")
                .as_deref(),
            Some("https://example.test/org/repo.git")
        );
        assert_eq!(
            sanitize_origin("git@example.test:org/repo.git").as_deref(),
            Some("ssh://example.test/org/repo.git")
        );
        for value in [
            "C:\\private\\repo",
            "file:///private/repo",
            "/local/repo",
            "javascript:alert(1)",
            "https://example.test/\nsecret",
        ] {
            assert_eq!(sanitize_origin(value), None);
        }
    }
    #[test]
    fn malformed_git_and_missing_directory_are_not_authoritative_metadata() {
        assert!(GitMetadata::read(&json!({"sha":"no-commit"})).is_err());
        assert!(GitMetadata::read(&json!({"branch":"x\nsecret"})).is_err());
        assert!(NativeMetadata::read(&json!({"cwd":"relative"})).is_err());
        let git =
            GitMetadata::read(&json!({"sha":null,"branch":null,"originUrl":"file:///secret"}))
                .unwrap()
                .unwrap();
        assert_eq!(git.update().origin_url, Some(None));
        assert_eq!(git.update().sha, Some(None));
    }
    fn git(cwd: &Path, args: &[&str]) {
        let mut command = Command::new("git");
        crate::runtime::hide_window(&mut command);
        let output = command.current_dir(cwd).args(args).output().unwrap();
        assert!(output.status.success(), "Fixture git failed");
    }
    #[test]
    fn repository_snapshot_handles_unborn_detached_and_changed_head_without_modifying_files() {
        let root = tempfile::tempdir().unwrap();
        git(root.path(), &["init", "--quiet", "-b", "p3-fixture"]);
        git(
            root.path(),
            &[
                "config",
                "remote.origin.url",
                "https://user:password@example.test/org/repo.git?secret=1",
            ],
        );
        let unborn = RepositorySnapshot::capture(root.path()).unwrap();
        assert!(unborn.metadata().sha.is_none());
        assert_eq!(unborn.metadata().branch.as_deref(), Some("p3-fixture"));
        assert_eq!(
            unborn.metadata().origin_url.as_deref(),
            Some("https://example.test/org/repo.git")
        );
        git(
            root.path(),
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.test",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=NUL",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "Fixture",
            ],
        );
        assert!(unborn.revalidate(root.path()).is_err());
        let committed = RepositorySnapshot::capture(root.path()).unwrap();
        assert!(committed.metadata().sha.is_some());
        git(
            root.path(),
            &[
                "-c",
                "core.hooksPath=NUL",
                "checkout",
                "--quiet",
                "--detach",
            ],
        );
        let detached = RepositorySnapshot::capture(root.path()).unwrap();
        assert!(detached.metadata().branch.is_none());
        assert_eq!(detached.metadata().sha, committed.metadata().sha);
        detached.revalidate(root.path()).unwrap();
        let non_repo = tempfile::tempdir().unwrap();
        assert!(RepositorySnapshot::capture(non_repo.path()).is_err());
    }
}
