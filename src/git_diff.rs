//! Explicit local Git inspection, not agent-attributed changes or a model tool.
use serde::{Deserialize, Serialize};
use similar::TextDiff;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Section {
    Staged,
    Unstaged,
    Untracked,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Entry {
    pub path: String,
    pub icon_key: String,
    pub sections: Vec<Section>,
    pub blocked: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Listing {
    pub entries: Vec<Entry>,
    pub unsupported_names: usize,
}

pub(crate) struct Reader {
    root: PathBuf,
    binary: PathBuf,
    filter_overrides: Vec<String>,
    repository_root: Option<PathBuf>,
}

impl Reader {
    pub fn new(root: &Path) -> Result<Self, String> {
        let root =
            fs::canonicalize(root).map_err(|error| format!("Project unavailable: {error}"))?;
        if !root.is_dir() {
            return Err("Select a local project directory.".into());
        }
        let binary = find_git()
            .ok_or("Git is not installed on PATH. Install Git to inspect repository changes.")?;
        let mut reader = Self {
            root,
            binary,
            filter_overrides: vec![],
            repository_root: None,
        };
        // Even --no-textconv does not disable clean/process filters when Git
        // compares the working tree. Discover names only, never expose values.
        let result = reader
            .command()
            .args([
                "config",
                "--null",
                "--name-only",
                "--get-regexp",
                "^filter\\..*\\.(clean|smudge|process|required)$",
            ])
            .output()
            .map_err(|error| error.to_string())?;
        if !result.status.success() && result.status.code() != Some(1) {
            return Err(failure(&result));
        }
        for key in result
            .stdout
            .split(|byte| *byte == 0)
            .filter(|key| !key.is_empty())
        {
            let key = std::str::from_utf8(key)
                .map_err(|_| "Unsupported Git filter configuration name.")?;
            if key.chars().any(char::is_control) || !key.starts_with("filter.") {
                return Err("Unsafe Git filter configuration name; no diff was read.".into());
            }
            reader.filter_overrides.push(format!(
                "{key}={}",
                if key.ends_with(".required") {
                    "false"
                } else {
                    ""
                }
            ));
        }
        let root_bytes = run(reader.command().args(["rev-parse", "--show-toplevel"]))?;
        let repository_root = std::str::from_utf8(&root_bytes)
            .map_err(|_| "Git returned an unsupported project path.")?
            .trim_end_matches(['\r', '\n']);
        let repository_root =
            fs::canonicalize(repository_root).map_err(|error| error.to_string())?;
        if !reader.root.starts_with(&repository_root) {
            return Err("Git's configured worktree is outside this selected project. No file contents were read.".into());
        }
        reader.repository_root = Some(repository_root);
        Ok(reader)
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.binary);
        command.current_dir(&self.root);
        if let Some(root) = &self.repository_root {
            command.arg("--work-tree").arg(root);
        }
        // Do not inherit another process's repository/index, external drivers,
        // injected config or credentials. The project is resolved exclusively here.
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
                "--literal-pathspecs",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "core.hooksPath=",
                "-c",
                "protocol.allow=never",
            ]);
        for setting in &self.filter_overrides {
            command.args(["-c", setting]);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        command
    }

    fn diff_command(&self, section: Section) -> Command {
        let mut command = self.command();
        command.args([
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
            "--ignore-submodules=all",
            "--relative",
            "--no-color",
            "--src-prefix=a/",
            "--dst-prefix=b/",
            "--no-indent-heuristic",
        ]);
        if section == Section::Staged {
            // Staged gitlink changes are blob metadata and require no submodule
            // traversal. Keep them listed, even if their directory is blocked.
            command.args(["--cached", "--ignore-submodules=none", "--submodule=short"]);
        }
        command
    }

    pub fn list(&self) -> Result<Listing, String> {
        let mut entries = BTreeMap::<String, Entry>::new();
        let mut unsupported_names = 0;
        for section in [Section::Staged, Section::Unstaged, Section::Untracked] {
            let mut command = if section == Section::Untracked {
                let mut command = self.command();
                command.args(["ls-files", "--others", "--exclude-standard", "-z"]);
                command
            } else {
                let mut command = self.diff_command(section);
                command.args(["--name-only", "-z"]);
                command
            };
            let bytes = run(command.args(["--", "."]))?;
            if !bytes.is_empty() && bytes.last() != Some(&0) {
                return Err("Git returned an incomplete filename list.".into());
            }
            for bytes in bytes
                .split(|byte| *byte == 0)
                .filter(|part| !part.is_empty())
            {
                let Ok(path) = std::str::from_utf8(bytes) else {
                    unsupported_names += 1;
                    continue;
                };
                let entry = entries.entry(path.into()).or_insert_with(|| Entry {
                    path: path.into(),
                    icon_key: crate::time_machine::file_icon_key(path).into(),
                    sections: vec![],
                    blocked: crate::workspace::checked_review_path(&self.root, path).err(),
                });
                if !entry.sections.contains(&section) {
                    entry.sections.push(section);
                }
            }
        }
        Ok(Listing {
            entries: entries.into_values().collect(),
            unsupported_names,
        })
    }

    pub fn read(&self, path: &str, section: Section) -> Result<String, String> {
        let absolute = crate::workspace::checked_review_path(&self.root, path)?;
        if section != Section::Untracked {
            let bytes = run(self.diff_command(section).args(["--patch", "--", path]))?;
            return String::from_utf8(bytes).map_err(|_| "This diff is not UTF-8 text. Inspect it with an external editor; no bytes were converted or omitted.".into());
        }
        // The file can have been staged/ignored since its list entry was shown.
        let names = run(self.command().args([
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            path,
        ]))?;
        if !names
            .split(|byte| *byte == 0)
            .any(|bytes| bytes == path.as_bytes())
        {
            return Err("This file is no longer untracked. Refresh the changed-file list.".into());
        }
        let bytes =
            fs::read(absolute).map_err(|error| format!("Could not read the new file: {error}"))?;
        if bytes.contains(&0) {
            return Ok(format!(
                "Binary untracked file: {path}\n{} bytes · no text diff available\n",
                bytes.len()
            ));
        }
        let text = std::str::from_utf8(&bytes).map_err(|_| {
            "This new file is not UTF-8 text. Its contents were not converted or omitted."
                .to_owned()
        })?;
        if text.is_empty() {
            return Ok(format!("Empty untracked file: {path}\n"));
        }
        Ok(TextDiff::from_lines("", text)
            .unified_diff()
            .header("/dev/null", &format!("b/{path}"))
            .to_string())
    }
}

fn run(command: &mut Command) -> Result<Vec<u8>, String> {
    let output = command
        .output()
        .map_err(|error| format!("Could not run local Git: {error}"))?;
    if !output.status.success() {
        return Err(failure(&output));
    }
    Ok(output.stdout)
}

fn failure(output: &Output) -> String {
    format!(
        "Local Git inspection failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

pub(crate) fn find_git() -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    let name = if cfg!(windows) { "git.exe" } else { "git" };
    // In particular, don't let a git.exe in the selected repository shadow Git.
    std::env::split_paths(&paths)
        .filter(|path| path.is_absolute())
        .map(|path| path.join(name))
        .find(|path| path.is_file())
        .and_then(|path| fs::canonicalize(path).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_git(root: &Path) -> Command {
        let mut command =
            Command::new(find_git().expect("Git is required by repository diff tests"));
        command.current_dir(root).args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.hooksPath=",
            "-c",
            "commit.gpgSign=false",
        ]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        command
    }

    fn git(root: &Path, args: &[&str]) {
        let result = fixture_git(root).args(args).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    fn repo() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        git(temp.path(), &["init", "-q"]);
        temp
    }
    #[test]
    fn separates_index_worktree_and_untracked_without_modifying_any_state() {
        let temp = repo();
        let root = temp.path();
        fs::write(root.join("tracked.rs"), "original\n").unwrap();
        fs::write(root.join("deleted.txt"), "removed\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "initial"]);
        fs::write(root.join("tracked.rs"), "staged\n").unwrap();
        git(root, &["add", "tracked.rs"]);
        fs::write(root.join("tracked.rs"), "working\n").unwrap();
        fs::remove_file(root.join("deleted.txt")).unwrap();
        fs::write(root.join("new [x].rs"), "new\n").unwrap();
        fs::write(root.join("new x.rs"), "different\n").unwrap();
        fs::write(root.join(".env"), "PRIVATE=fixture\n").unwrap();
        let before_index = fs::read(root.join(".git/index")).unwrap();
        let reader = Reader::new(root).unwrap();
        let listing = reader.list().unwrap();
        assert_eq!(listing.entries.len(), 5);
        assert_eq!(
            listing
                .entries
                .iter()
                .find(|entry| entry.path == "tracked.rs")
                .unwrap()
                .sections,
            [Section::Staged, Section::Unstaged]
        );
        assert!(
            listing
                .entries
                .iter()
                .find(|entry| entry.path == ".env")
                .unwrap()
                .blocked
                .is_some()
        );
        assert!(reader.read(".env", Section::Untracked).is_err());
        let staged = reader.read("tracked.rs", Section::Staged).unwrap();
        assert!(
            staged.contains("-original")
                && staged.contains("+staged")
                && !staged.contains("working")
        );
        let working = reader.read("tracked.rs", Section::Unstaged).unwrap();
        assert!(working.contains("-staged") && working.contains("+working"));
        assert!(
            reader
                .read("deleted.txt", Section::Unstaged)
                .unwrap()
                .contains("-removed")
        );
        assert!(
            reader
                .read("new [x].rs", Section::Untracked)
                .unwrap()
                .contains("+new")
        );
        assert_eq!(fs::read(root.join(".git/index")).unwrap(), before_index);
        assert_eq!(
            fs::read_to_string(root.join("tracked.rs")).unwrap(),
            "working\n"
        );
        git(root, &["add", "new [x].rs"]);
        assert!(reader.read("new [x].rs", Section::Untracked).is_err());
    }
    #[test]
    fn unborn_repository_and_subproject_scope_do_not_include_sibling_changes() {
        let temp = repo();
        let root = temp.path();
        fs::create_dir(root.join("inside")).unwrap();
        fs::write(root.join("inside/a.rs"), "inside\n").unwrap();
        fs::write(root.join("outside.rs"), "outside\n").unwrap();
        git(root, &["add", "."]);
        let reader = Reader::new(&root.join("inside")).unwrap();
        let listing = reader.list().unwrap();
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(listing.entries[0].path, "a.rs");
        assert!(
            reader
                .read("a.rs", Section::Staged)
                .unwrap()
                .contains("+inside")
        );
        assert!(reader.read("../outside.rs", Section::Staged).is_err());
        assert!(reader.read("a.rs ", Section::Staged).is_err());
    }
    #[test]
    fn external_diff_textconv_and_clean_filters_are_not_executed() {
        let temp = repo();
        let root = temp.path();
        fs::write(root.join("file.txt"), "old\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "initial"]);
        fs::write(root.join(".gitattributes"), "*.txt filter=trap diff=trap\n").unwrap();
        fs::write(root.join("file.txt"), "new\n").unwrap();
        let trap = "echo unexpected > was-executed.txt";
        for name in [
            "filter.trap.clean",
            "filter.trap.process",
            "diff.trap.command",
            "diff.trap.textconv",
            "diff.external",
            "core.fsmonitor",
        ] {
            git(root, &["config", name, trap]);
        }
        git(root, &["config", "filter.trap.required", "true"]);
        let reader = Reader::new(root).unwrap();
        reader.list().unwrap();
        assert!(
            reader
                .read("file.txt", Section::Unstaged)
                .unwrap()
                .contains("+new")
        );
        assert!(!root.join("was-executed.txt").exists());
    }
    #[test]
    fn binary_empty_and_non_repository_states_are_explicit() {
        let temp = repo();
        let root = temp.path();
        fs::write(root.join("binary.bin"), [0, 1, 2]).unwrap();
        fs::write(root.join("empty"), "").unwrap();
        let reader = Reader::new(root).unwrap();
        assert!(
            reader
                .read("binary.bin", Section::Untracked)
                .unwrap()
                .contains("Binary untracked")
        );
        assert!(
            reader
                .read("empty", Section::Untracked)
                .unwrap()
                .contains("Empty untracked")
        );
        let empty = tempfile::tempdir().unwrap();
        assert!(Reader::new(empty.path()).is_err());
    }

    #[test]
    fn repository_config_cannot_redirect_the_selected_project_to_another_worktree() {
        let temp = repo();
        let outside = tempfile::tempdir().unwrap();
        git(
            temp.path(),
            &["config", "core.worktree", outside.path().to_str().unwrap()],
        );
        assert!(Reader::new(temp.path()).is_err());
    }

    #[test]
    fn staged_submodule_reference_is_not_silently_lost_or_traversed() {
        let temp = repo();
        let root = temp.path();
        fs::write(root.join("base.txt"), "base\n").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-qm", "base"]);
        let id = fixture_git(root)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        let id = std::str::from_utf8(&id.stdout).unwrap().trim();
        git(
            root,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                "160000",
                id,
                "module",
            ],
        );
        fs::create_dir(root.join("module")).unwrap();
        let listing = Reader::new(root).unwrap().list().unwrap();
        let module = listing
            .entries
            .iter()
            .find(|entry| entry.path == "module")
            .unwrap();
        assert_eq!(module.sections, [Section::Staged]);
        assert!(module.blocked.is_some());
    }
}
