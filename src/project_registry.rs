use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};

const MAX_METADATA_FILE_BYTES: u64 = 512 * 1024;
const MAX_INSTRUCTION_FILES: usize = 16;
const MAX_INSTRUCTION_SCAN_ENTRIES: usize = 600;
const MAX_INSTRUCTION_SCAN_DEPTH: usize = 3;
const MAX_COMMANDS: usize = 10;
const MAX_GIT_STATUS_LINES: usize = 10_000;
const MAX_REPOSITORY_URL_CHARS: usize = 2_048;
const MAX_REMOTE_DIRECTORY_CHARS: usize = 1_024;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct ProjectCommand {
    pub label: String,
    pub command: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct ProjectMetadata {
    pub git_repository: bool,
    pub git_branch: Option<String>,
    pub git_modified: usize,
    pub worktree_count: usize,
    pub stack: Vec<String>,
    pub instruction_files: Vec<String>,
    pub commands: Vec<ProjectCommand>,
    pub files: Vec<ProjectFile>,
    pub files_truncated: bool,
    pub detected_at_ms: u64,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectFile {
    pub path: String,
    pub kind: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct RemoteProject {
    pub id: String,
    pub name: String,
    pub ssh_profile_id: String,
    pub directory: String,
    pub pinned: bool,
    pub created_at_ms: u64,
    pub last_opened_at_ms: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectImportView {
    pub active: bool,
    pub kind: Option<String>,
    pub message: Option<String>,
    pub error: bool,
}

pub(crate) fn validate_new_project_name(name: &str) -> Result<&str, String> {
    let name = name.trim();
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    if name.is_empty()
        || name.chars().count() > 80
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|ch| ch.is_control() || "<>:\"/\\|?*".contains(ch))
        || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        })
    {
        return Err("Enter a folder name of 1–80 characters without reserved Windows names or path separators.".into());
    }
    Ok(name)
}

pub(crate) fn create_local_project(parent: &Path, name: &str) -> Result<PathBuf, String> {
    let name = validate_new_project_name(name)?;
    let parent =
        fs::canonicalize(parent).map_err(|error| format!("Destination unavailable: {error}"))?;
    if !parent.is_dir() {
        return Err("Choose a destination folder.".into());
    }
    let root = parent.join(name);
    // create_dir is exclusive: never reuse or overwrite an existing directory or link.
    fs::create_dir(&root).map_err(|error| if error.kind() == std::io::ErrorKind::AlreadyExists {
        "A file or folder with this name already exists. Choose another name or add the existing folder.".into()
    } else { format!("Could not create the project folder: {error}") })?;
    Ok(root)
}

pub(crate) fn normalize_remote_directory(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_REMOTE_DIRECTORY_CHARS
        || value.chars().any(char::is_control)
        || !value.starts_with('/')
    {
        return Err("Enter an absolute remote directory beginning with /.".to_owned());
    }
    let mut normalized = value.replace('\\', "/");
    while normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }
    Ok(normalized)
}

pub(crate) fn remote_project_name(directory: &str) -> String {
    directory
        .trim_end_matches('/')
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or("SSH project")
        .chars()
        .take(80)
        .collect()
}

pub(crate) fn inspect_local_project(root: &Path, detected_at_ms: u64) -> ProjectMetadata {
    let mut metadata = ProjectMetadata {
        detected_at_ms,
        ..ProjectMetadata::default()
    };
    let root = match fs::canonicalize(root) {
        Ok(root) if root.is_dir() => root,
        Ok(_) => {
            metadata.error = Some("The selected project is not a directory.".to_owned());
            return metadata;
        }
        Err(error) => {
            metadata.error = Some(format!("Project unavailable: {error}"));
            return metadata;
        }
    };

    inspect_git(&root, &mut metadata);
    inspect_stack_and_commands(&root, &mut metadata);
    metadata.instruction_files = find_instruction_files(&root);
    metadata
}

fn inspect_git(root: &Path, metadata: &mut ProjectMetadata) {
    let Some(binary) = crate::git_diff::find_git() else {
        return;
    };
    let inside = run_git(&binary, root, &["rev-parse", "--is-inside-work-tree"])
        .is_ok_and(|output| output.trim() == "true");
    if !inside {
        return;
    }
    metadata.git_repository = true;
    metadata.git_branch = run_git(
        &binary,
        root,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
    )
    .ok()
    .map(|value| value.trim().to_owned())
    .filter(|value| !value.is_empty())
    .or_else(|| {
        run_git(&binary, root, &["rev-parse", "--short", "HEAD"])
            .ok()
            .map(|value| format!("detached · {}", value.trim()))
    });
    metadata.git_modified = run_git(
        &binary,
        root,
        &["status", "--porcelain=v1", "--untracked-files=normal"],
    )
    .map(|output| output.lines().take(MAX_GIT_STATUS_LINES).count())
    .unwrap_or_default();
    metadata.worktree_count = run_git(&binary, root, &["worktree", "list", "--porcelain"])
        .map(|output| {
            output
                .lines()
                .filter(|line| line.starts_with("worktree "))
                .count()
        })
        .unwrap_or(1)
        .max(1);
}

fn git_command(binary: &Path, root: &Path) -> Command {
    let mut command = Command::new(binary);
    command.current_dir(root);
    for (name, _) in std::env::vars_os() {
        if name
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GIT_")
        {
            command.env_remove(name);
        }
    }
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("LC_ALL", "C")
        .args([
            "--no-pager",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    hide_window(&mut command);
    command
}

fn run_git(binary: &Path, root: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = git_command(binary, root)
        .args(arguments)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err("Git metadata is unavailable.".to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// An isolated working copy is a user project, never a disposable build cache.
pub(crate) fn validate_task_worktree_source(root: &Path) -> Result<PathBuf, String> {
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let binary = crate::git_diff::find_git().ok_or("Install Git before creating a worktree.")?;
    let repository = run_git(&binary, &root, &["rev-parse", "--show-toplevel"]).map_err(|_| {
        "New task in a worktree requires a Git repository. Commit the project files first."
            .to_owned()
    })?;
    if fs::canonicalize(repository.trim()).map_err(|e| e.to_string())? != root {
        return Err(
            "Add the repository root as a project before creating an isolated task.".into(),
        );
    }
    run_git(&binary, &root, &["rev-parse", "--verify", "HEAD"])
        .map_err(|_| "Create the first commit before using worktrees.".to_owned())?;
    if run_git(&binary, &root, &["ls-tree", "--name-only", "HEAD"])?
        .trim()
        .is_empty()
    {
        return Err(
            "The current Git commit contains no files. Commit the project files before creating a worktree."
                .into(),
        );
    }
    Ok(root)
}

pub(crate) fn create_task_worktree(
    root: &Path,
    parent: &Path,
    name: &str,
) -> Result<PathBuf, String> {
    let name = validate_new_project_name(name)?;
    let root = validate_task_worktree_source(root)?;
    let parent = fs::canonicalize(parent).map_err(|e| e.to_string())?;
    if !parent.is_dir() || parent.starts_with(&root) {
        return Err("Choose a destination outside the original project.".into());
    }
    let destination = parent.join(name);
    if fs::symlink_metadata(&destination).is_ok() {
        return Err(
            "The destination already exists. Choose a new task name or parent folder.".into(),
        );
    }
    let binary = crate::git_diff::find_git().ok_or("Install Git before creating a worktree.")?;
    let slug: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let branch = format!(
        "codex/{}-{}",
        slug.trim_matches('-'),
        &uuid::Uuid::new_v4().to_string()[..8]
    );
    let mut command = git_command(&binary, &root);
    // Checkout must not invoke repository-provided filters or hooks.
    let filters = git_command(&binary, &root)
        .args([
            "config",
            "--null",
            "--name-only",
            "--get-regexp",
            "^filter\\..*\\.(clean|smudge|process|required)$",
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !filters.status.success() && filters.status.code() != Some(1) {
        return Err("Could not inspect Git checkout filters.".into());
    }
    for key in filters
        .stdout
        .split(|byte| *byte == 0)
        .filter(|key| !key.is_empty())
    {
        let key = std::str::from_utf8(key).map_err(|_| "Unsupported Git filter name.")?;
        if key.chars().any(char::is_control) || !key.starts_with("filter.") {
            return Err("Unsupported Git filter configuration.".into());
        }
        command.arg("-c").arg(format!(
            "{key}={}",
            if key.ends_with(".required") {
                "false"
            } else {
                ""
            }
        ));
    }
    // Git for Windows does not accept Win32 verbatim prefixes in worktree paths.
    // The canonical path above remains the authority for scope validation.
    let destination_text = destination
        .to_str()
        .ok_or("Git requires a Unicode destination path.")?;
    let git_destination = if let Some(unc) = destination_text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        destination_text
            .strip_prefix(r"\\?\")
            .unwrap_or(destination_text)
            .to_owned()
    };
    let result = command
        .args(["-c", "core.longpaths=true"])
        .args(["worktree", "add", "--no-track", "-b", &branch, "--"])
        .arg(&git_destination)
        .arg("HEAD")
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    if !result.status.success() {
        // Delete only this operation's fresh, merged branch; Git refuses if a
        // worktree already uses it. Never remove a partially created directory.
        let _ = run_git(&binary, &root, &["branch", "-d", "--", &branch]);
        return Err(format!(
            "Worktree creation failed: {}",
            String::from_utf8_lossy(&result.stderr)
                .chars()
                .take(2000)
                .collect::<String>()
        ));
    }
    fs::canonicalize(destination).map_err(|e| e.to_string())
}

fn inspect_stack_and_commands(root: &Path, metadata: &mut ProjectMetadata) {
    let mut markers = BTreeSet::new();
    collect_marker_names(root, &mut markers);
    for child in ["ui", "web", "frontend", "backend", "app", "src"] {
        let path = root.join(child);
        if path.is_dir() {
            collect_marker_names(&path, &mut markers);
        }
    }

    let has = |name: &str| markers.contains(&name.to_ascii_lowercase());
    if has("cargo.toml") {
        push_stack(&mut metadata.stack, "Rust");
        push_command(&mut metadata.commands, "Build", "cargo build");
        push_command(&mut metadata.commands, "Test", "cargo test");
    }
    if has("svelte.config.js") || has("svelte.config.mjs") || has("svelte.config.ts") {
        push_stack(&mut metadata.stack, "Svelte");
    }
    if has("tsconfig.json") {
        push_stack(&mut metadata.stack, "TypeScript");
    }
    if has("package.json") {
        if !metadata
            .stack
            .iter()
            .any(|value| value == "Svelte" || value == "TypeScript")
        {
            push_stack(&mut metadata.stack, "JavaScript");
        }
        inspect_package_scripts(root, metadata);
    }
    if has("pyproject.toml") || has("requirements.txt") || has("setup.py") {
        push_stack(&mut metadata.stack, "Python");
        push_command(&mut metadata.commands, "Test", "python -m pytest");
    }
    if has("go.mod") {
        push_stack(&mut metadata.stack, "Go");
        push_command(&mut metadata.commands, "Test", "go test ./...");
    }
    if has("pom.xml") || has("build.gradle") || has("build.gradle.kts") {
        push_stack(&mut metadata.stack, "Java");
    }
    if markers
        .iter()
        .any(|name| name.ends_with(".sln") || name.ends_with(".csproj"))
    {
        push_stack(&mut metadata.stack, ".NET");
        push_command(&mut metadata.commands, "Build", "dotnet build");
        push_command(&mut metadata.commands, "Test", "dotnet test");
    }
    if has("composer.json") {
        push_stack(&mut metadata.stack, "PHP");
    }
    if has("gemfile") {
        push_stack(&mut metadata.stack, "Ruby");
    }
    if has("pubspec.yaml") {
        push_stack(&mut metadata.stack, "Dart");
    }
    if has("mix.exs") {
        push_stack(&mut metadata.stack, "Elixir");
    }
    if has("cmakelists.txt") || has("makefile") {
        push_stack(&mut metadata.stack, "C/C++");
    }
    if has("dockerfile")
        || has("compose.yml")
        || has("compose.yaml")
        || has("docker-compose.yml")
        || has("docker-compose.yaml")
    {
        push_stack(&mut metadata.stack, "Docker");
    }
}

fn collect_marker_names(directory: &Path, markers: &mut BTreeSet<String>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten().take(256) {
        let path = entry.path();
        if path.is_file()
            && let Some(name) = path.file_name().and_then(|name| name.to_str())
        {
            markers.insert(name.to_ascii_lowercase());
        }
    }
}

fn inspect_package_scripts(root: &Path, metadata: &mut ProjectMetadata) {
    let candidates = ["", "ui", "web", "frontend", "backend", "app"]
        .map(|directory| root.join(directory).join("package.json"));
    for path in candidates {
        let Ok(file_metadata) = fs::metadata(&path) else {
            continue;
        };
        if !file_metadata.is_file() || file_metadata.len() > MAX_METADATA_FILE_BYTES {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else { continue };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            continue;
        };
        let Some(scripts) = value.get("scripts").and_then(serde_json::Value::as_object) else {
            continue;
        };
        for preferred in ["dev", "build", "test", "check", "lint", "start"] {
            if scripts
                .get(preferred)
                .and_then(serde_json::Value::as_str)
                .is_some()
            {
                push_command(
                    &mut metadata.commands,
                    &title_case(preferred),
                    &format!("npm run {preferred}"),
                );
            }
        }
    }
}

fn push_command(commands: &mut Vec<ProjectCommand>, label: &str, command: &str) {
    if commands.len() >= MAX_COMMANDS || commands.iter().any(|entry| entry.command == command) {
        return;
    }
    commands.push(ProjectCommand {
        label: label.to_owned(),
        command: command.to_owned(),
    });
}

fn push_stack(stack: &mut Vec<String>, name: &str) {
    if !stack.iter().any(|current| current == name) {
        stack.push(name.to_owned());
    }
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
}

fn find_instruction_files(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    let mut scanned = 0_usize;
    while let Some((directory, depth)) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            scanned += 1;
            if scanned > MAX_INSTRUCTION_SCAN_ENTRIES || found.len() >= MAX_INSTRUCTION_FILES {
                break;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_file() && name.eq_ignore_ascii_case("AGENTS.md") {
                found.push(
                    path.strip_prefix(root)
                        .unwrap_or(&path)
                        .display()
                        .to_string(),
                );
            } else if kind.is_dir()
                && depth < MAX_INSTRUCTION_SCAN_DEPTH
                && !name.starts_with('.')
                && !matches!(
                    name.to_ascii_lowercase().as_str(),
                    "node_modules"
                        | "target"
                        | "vendor"
                        | "venv"
                        | "dist"
                        | "build"
                        | "out"
                        | "coverage"
                )
            {
                pending.push((path, depth + 1));
            }
        }
        if scanned > MAX_INSTRUCTION_SCAN_ENTRIES || found.len() >= MAX_INSTRUCTION_FILES {
            break;
        }
    }
    found.sort_by_key(|path| path.to_ascii_lowercase());
    found
}

pub(crate) fn clone_repository(repository: &str, parent: &Path) -> Result<PathBuf, String> {
    let repository = normalize_repository_url(repository)?;
    let parent = fs::canonicalize(parent)
        .map_err(|error| format!("The destination is unavailable: {error}"))?;
    if !parent.is_dir() {
        return Err("Choose a destination folder.".to_owned());
    }
    let name = repository_directory_name(&repository)?;
    let destination = parent.join(name);
    if destination.exists() {
        return Err(
            "A folder with this repository name already exists in the destination.".to_owned(),
        );
    }
    let binary =
        crate::git_diff::find_git().ok_or_else(|| "Git is not installed on PATH.".to_owned())?;
    let mut command = Command::new(binary);
    command
        .current_dir(&parent)
        .args([
            "-c",
            "core.hooksPath=",
            "clone",
            "--no-recurse-submodules",
            "--",
        ])
        .arg(&repository)
        .arg(&destination)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .env("LC_ALL", "C");
    hide_window(&mut command);
    let output = command
        .output()
        .map_err(|error| format!("Could not start Git clone: {error}"))?;
    if !output.status.success() {
        if destination.parent() == Some(parent.as_path()) && destination.exists() {
            let _ = fs::remove_dir_all(&destination);
        }
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim().chars().take(1_200).collect::<String>();
        return Err(if detail.is_empty() {
            "Git could not clone this repository.".to_owned()
        } else {
            format!("Git clone failed: {detail}")
        });
    }
    fs::canonicalize(&destination)
        .map_err(|error| format!("The cloned project could not be opened: {error}"))
}

fn normalize_repository_url(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > MAX_REPOSITORY_URL_CHARS
        || value.chars().any(char::is_control)
    {
        return Err("Enter a valid repository URL.".to_owned());
    }
    let supported_url = url::Url::parse(value).is_ok_and(|url| {
        matches!(url.scheme(), "https" | "http" | "ssh" | "git") && url.host().is_some()
    });
    let scp_like = value
        .split_once(':')
        .is_some_and(|(host, path)| host.contains('@') && !host.contains('/') && !path.is_empty());
    if !supported_url && !scp_like {
        return Err("Use an HTTPS, SSH, Git, or git@host repository URL.".to_owned());
    }
    Ok(value.to_owned())
}

fn repository_directory_name(repository: &str) -> Result<String, String> {
    let tail = repository
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', ':'])
        .next()
        .unwrap_or_default()
        .trim_end_matches(".git")
        .trim();
    let name = tail
        .chars()
        .filter(|character| {
            !character.is_control()
                && !matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
        })
        .take(120)
        .collect::<String>();
    if name.is_empty() || name == "." || name == ".." {
        return Err("The repository URL does not contain a usable project name.".to_owned());
    }
    Ok(name)
}

pub(crate) fn remote_inspection_script(directory: &str) -> Result<String, String> {
    let directory = normalize_remote_directory(directory)?;
    let directory = shell_single_quote(&directory);
    Ok(format!(
        r#"project={directory}
if [ ! -d "$project" ]; then printf 'ERROR\tRemote directory does not exist.\n'; exit 4; fi
cd -- "$project" || exit 4
printf 'READY\t1\n'
export GIT_OPTIONAL_LOCKS=0 GIT_TERMINAL_PROMPT=0
if command -v git >/dev/null 2>&1 && git --no-pager -c core.fsmonitor=false -c core.hooksPath= rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  printf 'GIT\t1\n'
  branch=$(git --no-pager -c core.fsmonitor=false -c core.hooksPath= symbolic-ref --quiet --short HEAD 2>/dev/null || git --no-pager -c core.fsmonitor=false -c core.hooksPath= rev-parse --short HEAD 2>/dev/null || true)
  printf 'BRANCH\t%s\n' "$branch"
  modified=$(git --no-pager -c core.fsmonitor=false -c core.hooksPath= status --porcelain=v1 --untracked-files=normal 2>/dev/null | awk 'NR<=10000{{count++}} END{{print count+0}}')
  printf 'MODIFIED\t%s\n' "$modified"
  worktrees=$(git --no-pager -c core.fsmonitor=false -c core.hooksPath= worktree list --porcelain 2>/dev/null | awk '/^worktree /{{count++}} END{{print count+0}}')
  printf 'WORKTREES\t%s\n' "$worktrees"
fi
for marker in Cargo.toml package.json svelte.config.js svelte.config.mjs svelte.config.ts tsconfig.json pyproject.toml requirements.txt go.mod pom.xml build.gradle build.gradle.kts composer.json Gemfile pubspec.yaml mix.exs CMakeLists.txt Makefile Dockerfile compose.yml compose.yaml docker-compose.yml docker-compose.yaml; do
  if [ -f "$marker" ] || [ -f "ui/$marker" ] || [ -f "frontend/$marker" ] || [ -f "backend/$marker" ]; then printf 'MARKER\t%s\n' "$marker"; fi
done
find . -maxdepth 3 \( -path './.*' -o -name node_modules -o -name target -o -name vendor -o -name venv -o -name dist -o -name build -o -name out -o -name coverage \) -prune -o -type f -name AGENTS.md -print 2>/dev/null | head -n 16 | while IFS= read -r item; do printf 'INSTRUCTION\t%s\n' "${{item#./}}"; done
find . -mindepth 1 -maxdepth 3 \( -name '.*' -o -name node_modules -o -name target -o -name vendor -o -name venv -o -name dist -o -name build -o -name out -o -name coverage \) -prune -o \( -type f -o -type d \) -printf 'FILE\t%y\t%P\n' 2>/dev/null | head -n 601"#
    ))
}

pub(crate) fn parse_remote_inspection(output: &str, detected_at_ms: u64) -> ProjectMetadata {
    let mut metadata = ProjectMetadata {
        detected_at_ms,
        ..ProjectMetadata::default()
    };
    let mut markers = BTreeSet::new();
    let mut ready = false;
    for line in output.lines().take(2_000) {
        let Some((key, value)) = line.split_once('\t') else {
            continue;
        };
        let value = value.trim();
        match key {
            "READY" => ready = true,
            "ERROR" => metadata.error = Some(value.chars().take(240).collect()),
            "GIT" => metadata.git_repository = value == "1",
            "BRANCH" if !value.is_empty() => {
                metadata.git_branch = Some(value.chars().take(240).collect())
            }
            "MODIFIED" => metadata.git_modified = value.parse().unwrap_or_default(),
            "WORKTREES" => metadata.worktree_count = value.parse().unwrap_or_default(),
            "MARKER" => {
                markers.insert(value.to_ascii_lowercase());
            }
            "INSTRUCTION" if metadata.instruction_files.len() < MAX_INSTRUCTION_FILES => metadata
                .instruction_files
                .push(value.chars().take(500).collect()),
            "FILE" => {
                if let Some((kind, path)) = value.split_once('\t')
                    && matches!(kind, "d" | "f")
                    && !path.is_empty()
                    && path.len() <= 2048
                    && !path.chars().any(|ch| ch.is_control() || ch == '\\')
                    && !path.split('/').any(|part| matches!(part, "" | "." | ".."))
                {
                    if metadata.files.len() < MAX_INSTRUCTION_SCAN_ENTRIES {
                        metadata.files.push(ProjectFile {
                            path: path.into(),
                            kind: if kind == "d" { "directory" } else { "file" }.into(),
                        });
                    } else {
                        metadata.files_truncated = true;
                    }
                }
            }
            _ => {}
        }
    }
    if !ready && metadata.error.is_none() {
        metadata.error = Some("The remote project metadata response was incomplete.".to_owned());
    }
    apply_remote_markers(&markers, &mut metadata);
    metadata
}

fn apply_remote_markers(markers: &BTreeSet<String>, metadata: &mut ProjectMetadata) {
    let has = |name: &str| markers.contains(&name.to_ascii_lowercase());
    if has("cargo.toml") {
        push_stack(&mut metadata.stack, "Rust");
        push_command(&mut metadata.commands, "Build", "cargo build");
        push_command(&mut metadata.commands, "Test", "cargo test");
    }
    if has("svelte.config.js") || has("svelte.config.mjs") || has("svelte.config.ts") {
        push_stack(&mut metadata.stack, "Svelte");
    }
    if has("tsconfig.json") {
        push_stack(&mut metadata.stack, "TypeScript");
    }
    if has("package.json") && metadata.stack.is_empty() {
        push_stack(&mut metadata.stack, "JavaScript");
    }
    if has("package.json") {
        push_command(&mut metadata.commands, "Build", "npm run build");
        push_command(&mut metadata.commands, "Test", "npm test");
    }
    if has("pyproject.toml") || has("requirements.txt") {
        push_stack(&mut metadata.stack, "Python");
    }
    if has("go.mod") {
        push_stack(&mut metadata.stack, "Go");
    }
    if has("pom.xml") || has("build.gradle") || has("build.gradle.kts") {
        push_stack(&mut metadata.stack, "Java");
    }
    if has("composer.json") {
        push_stack(&mut metadata.stack, "PHP");
    }
    if has("gemfile") {
        push_stack(&mut metadata.stack, "Ruby");
    }
    if has("pubspec.yaml") {
        push_stack(&mut metadata.stack, "Dart");
    }
    if has("mix.exs") {
        push_stack(&mut metadata.stack, "Elixir");
    }
    if has("cmakelists.txt") || has("makefile") {
        push_stack(&mut metadata.stack, "C/C++");
    }
    if has("dockerfile")
        || has("compose.yml")
        || has("compose.yaml")
        || has("docker-compose.yml")
        || has("docker-compose.yaml")
    {
        push_stack(&mut metadata.stack, "Docker");
    }
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn hide_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_worktree_isolates_files_and_preserves_dirty_original_and_existing_destinations() {
        let parent = tempfile::tempdir().unwrap();
        let source = parent.path().join("source");
        fs::create_dir(&source).unwrap();
        let binary = crate::git_diff::find_git().expect("Git is required for worktree tests");
        run_git(&binary, &source, &["init"]).unwrap();
        fs::write(source.join("app.txt"), "committed").unwrap();
        run_git(&binary, &source, &["add", "app.txt"]).unwrap();
        run_git(
            &binary,
            &source,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.test",
                "-c",
                "commit.gpgSign=false",
                "commit",
                "-m",
                "fixture",
            ],
        )
        .unwrap();
        fs::write(source.join("app.txt"), "unsaved original").unwrap();
        let task = create_task_worktree(&source, parent.path(), "Isolated task").unwrap();
        assert_eq!(
            fs::read_to_string(task.join("app.txt")).unwrap(),
            "committed"
        );
        fs::write(task.join("app.txt"), "task changes").unwrap();
        assert_eq!(
            fs::read_to_string(source.join("app.txt")).unwrap(),
            "unsaved original"
        );
        assert!(
            run_git(&binary, &task, &["branch", "--show-current"])
                .unwrap()
                .starts_with("codex/")
        );
        assert!(create_task_worktree(&source, parent.path(), "Isolated task").is_err());
        assert_eq!(
            fs::read_to_string(task.join("app.txt")).unwrap(),
            "task changes"
        );
        assert!(create_task_worktree(&source, &source, "nested").is_err());
        assert!(create_task_worktree(&source, parent.path(), "../escape").is_err());
    }

    #[test]
    fn task_worktree_rejects_projects_without_committed_files_before_creating_a_folder() {
        let parent = tempfile::tempdir().unwrap();
        let source = parent.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("draft.txt"), "not committed").unwrap();
        assert!(
            validate_task_worktree_source(&source)
                .unwrap_err()
                .contains("requires a Git repository")
        );
        assert!(!parent.path().join("isolated").exists());

        let binary = crate::git_diff::find_git().expect("Git is required for worktree tests");
        run_git(&binary, &source, &["init"]).unwrap();
        run_git(
            &binary,
            &source,
            &[
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.test",
                "-c",
                "commit.gpgSign=false",
                "commit",
                "--allow-empty",
                "-m",
                "empty fixture",
            ],
        )
        .unwrap();
        assert!(
            create_task_worktree(&source, parent.path(), "isolated")
                .unwrap_err()
                .contains("current Git commit contains no files")
        );
        assert!(!parent.path().join("isolated").exists());
    }

    #[test]
    fn creates_only_a_new_child_directory_and_never_reuses_existing_content() {
        let parent = tempfile::tempdir().unwrap();
        let project = create_local_project(parent.path(), "My project").unwrap();
        assert_eq!(
            project.parent().unwrap(),
            parent.path().canonicalize().unwrap()
        );
        assert!(project.is_dir());
        assert_eq!(fs::read_dir(&project).unwrap().count(), 0);
        fs::write(project.join("keep.txt"), "keep").unwrap();
        assert!(create_local_project(parent.path(), "My project").is_err());
        assert_eq!(
            fs::read_to_string(project.join("keep.txt")).unwrap(),
            "keep"
        );
        assert!(create_local_project(&project.join("keep.txt"), "Other").is_err());
        assert!(create_local_project(&parent.path().join("missing"), "Other").is_err());
    }

    #[test]
    fn new_project_names_cannot_escape_the_chosen_parent_or_use_device_names() {
        for name in [
            "",
            " ",
            ".",
            "..",
            "../outside",
            "one/two",
            "one\\two",
            "C:\\other",
            "file:stream",
            "CON",
            "NUL.txt",
            "com1.log",
            "LPT9",
            "COM¹",
            "bad?",
            "bad.",
            "bad\0name",
            "line\nname",
        ] {
            assert!(validate_new_project_name(name).is_err(), "{name:?}");
        }
        assert!(validate_new_project_name(&"x".repeat(81)).is_err());
        assert_eq!(validate_new_project_name("  My app  ").unwrap(), "My app");
        assert!(validate_new_project_name("app.v2").is_ok());
    }

    #[test]
    fn normalizes_remote_directories_without_browsing_the_server() {
        assert_eq!(
            normalize_remote_directory(" /srv/app/// ").unwrap(),
            "/srv/app"
        );
        assert!(normalize_remote_directory("relative/app").is_err());
        assert!(normalize_remote_directory("/srv/app\nrm -rf /").is_err());
    }

    #[test]
    fn parses_bounded_remote_metadata() {
        let metadata = parse_remote_inspection(
            "READY\t1\nGIT\t1\nBRANCH\tmain\nMODIFIED\t12\nWORKTREES\t2\nMARKER\tCargo.toml\nMARKER\tsvelte.config.js\nINSTRUCTION\tAGENTS.md\n",
            42,
        );
        assert!(metadata.git_repository);
        assert_eq!(metadata.git_branch.as_deref(), Some("main"));
        assert_eq!(metadata.git_modified, 12);
        assert_eq!(metadata.worktree_count, 2);
        assert_eq!(metadata.stack, ["Rust", "Svelte"]);
        assert_eq!(metadata.instruction_files, ["AGENTS.md"]);
    }

    #[test]
    fn remote_files_are_bounded_relative_and_do_not_follow_links() {
        let mut output = "READY\t1\nFILE\td\tsrc\nFILE\tf\tsrc/main.rs\nFILE\tl\tlink\nFILE\tf\t../escape\nFILE\tf\t/etc/passwd\nFILE\tf\tsrc/../escape\n".to_owned();
        for index in 0..650 {
            output.push_str(&format!("FILE\tf\tfile{index}.ts\n"));
        }
        let metadata = parse_remote_inspection(&output, 1);
        assert_eq!(metadata.files.len(), MAX_INSTRUCTION_SCAN_ENTRIES);
        assert!(metadata.files_truncated);
        assert_eq!(metadata.files[0].path, "src");
        assert_eq!(metadata.files[0].kind, "directory");
        assert_eq!(metadata.files[1].path, "src/main.rs");
        assert!(
            !metadata
                .files
                .iter()
                .any(|file| file.path.contains("escape") || file.path == "link")
        );
        let script = remote_inspection_script("/srv/my project").unwrap();
        assert!(script.contains("cd -- \"$project\""));
        assert!(script.contains("-maxdepth 3"));
        assert!(script.contains("head -n 601"));
        assert!(!script.contains("find -L"));
    }

    #[test]
    fn repository_name_is_safe_for_a_destination_folder() {
        assert_eq!(
            repository_directory_name("https://github.com/openai/codex.git").unwrap(),
            "codex"
        );
        assert_eq!(
            repository_directory_name("git@github.com:openai/codex.git").unwrap(),
            "codex"
        );
        assert!(normalize_repository_url("file:///tmp/repo").is_err());
    }

    #[test]
    fn detects_project_stack_instructions_and_common_commands_without_heavy_trees() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nname='fixture'\n",
        )
        .unwrap();
        fs::write(root.path().join("AGENTS.md"), "fixture instructions").unwrap();
        let ui = root.path().join("ui");
        fs::create_dir(&ui).unwrap();
        fs::write(ui.join("svelte.config.js"), "export default {};").unwrap();
        fs::write(ui.join("tsconfig.json"), "{}").unwrap();
        fs::write(
            ui.join("package.json"),
            r#"{"scripts":{"build":"vite build","check":"svelte-check"}}"#,
        )
        .unwrap();
        let ignored = root.path().join("node_modules/dependency");
        fs::create_dir_all(&ignored).unwrap();
        fs::write(ignored.join("AGENTS.md"), "must stay ignored").unwrap();

        let metadata = inspect_local_project(root.path(), 77);
        assert_eq!(metadata.detected_at_ms, 77);
        assert!(metadata.stack.iter().any(|value| value == "Rust"));
        assert!(metadata.stack.iter().any(|value| value == "Svelte"));
        assert!(metadata.stack.iter().any(|value| value == "TypeScript"));
        assert!(
            metadata
                .commands
                .iter()
                .any(|entry| entry.command == "cargo test")
        );
        assert!(
            metadata
                .commands
                .iter()
                .any(|entry| entry.command == "npm run check")
        );
        assert_eq!(metadata.instruction_files, ["AGENTS.md"]);
    }
}
