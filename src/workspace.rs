use std::{
    collections::{HashSet, VecDeque},
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};

use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    commands::{TextReplacement, WorkspaceCommand},
    sensitive_path::is_sensitive_path,
    time_machine::file_icon_key,
};

const MAX_FILE_BYTES: u64 = 1_000_000;
const MAX_SEARCH_FILE_BYTES: u64 = 512_000;
const MAX_SEARCH_FILES: usize = 500;
const MAX_SEARCH_DEPTH: usize = 16;
const MAX_EXPLORER_ENTRIES: usize = 1_500;
const MAX_EXPLORER_DEPTH: usize = 24;
const MAX_WORKSPACE_PATH_CHARS: usize = 1_024;

#[derive(Debug, Default)]
pub(crate) struct WorkspaceRuntime {
    root: Option<PathBuf>,
    project_roots: Vec<PathBuf>,
    archived_project_roots: Vec<PathBuf>,
    restore_error: Option<String>,
    explorer_entries: Vec<WorkspaceExplorerEntry>,
    explorer_truncated: bool,
    explorer_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceView {
    pub connected: bool,
    pub root: Option<String>,
    pub name: Option<String>,
    pub projects: Vec<WorkspaceProjectView>,
    pub status: String,
    pub capabilities: Vec<&'static str>,
    pub entries: Vec<WorkspaceExplorerEntry>,
    pub truncated: bool,
    pub explorer_error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceProjectView {
    pub root: String,
    pub name: String,
    pub active: bool,
    pub archived: bool,
    pub pinned: bool,
    pub agent_active: bool,
    pub checkpoint_pending: bool,
    pub removal_pending: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceExplorerEntry {
    path: String,
    kind: &'static str,
    bytes: Option<u64>,
    icon_key: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceEntry {
    path: String,
    kind: &'static str,
    bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchMatch {
    path: String,
    line: usize,
    preview: String,
}

impl WorkspaceRuntime {
    pub(crate) fn read_editor_file(
        &self,
        relative: &str,
    ) -> Result<crate::file_editor::EditorFile, String> {
        use std::io::Read;
        let path = self.resolve_existing(relative)?;
        self.ensure_readable_file(&path)?;
        let mut bytes = Vec::new();
        fs::File::open(&path)
            .map_err(|error| error.to_string())?
            .take(MAX_FILE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err(
                "The editor supports text files up to 1 MB; this file was not partially opened."
                    .into(),
            );
        }
        let sha256 = sha256_hex(&bytes);
        let bom = bytes.starts_with(&[0xef, 0xbb, 0xbf]);
        let text = std::str::from_utf8(if bom { &bytes[3..] } else { &bytes })
            .map_err(|_| "The editor supports UTF-8 text files, not binary or other encodings.")?;
        if text.contains('\0') {
            return Err("Binary files cannot be edited as source code.".into());
        }
        let crlf = text.contains("\r\n");
        let content = text.replace("\r\n", "\n");
        if content.contains('\r') || (crlf && text.replace("\r\n", "").contains('\n')) {
            return Err(
                "Mixed or legacy line endings are not yet supported; the file was left unchanged."
                    .into(),
            );
        }
        Ok(crate::file_editor::EditorFile {
            root: self.connected_root()?.display().to_string(),
            path: self.relative_display(&path)?,
            content,
            sha256,
            bom,
            line_ending: if crlf { "CRLF" } else { "LF" }.into(),
            read_only: fs::metadata(&path)
                .map_err(|error| error.to_string())?
                .permissions()
                .readonly(),
            icon_key: file_icon_key(relative).into(),
        })
    }

    pub(crate) fn save_editor_file(
        &self,
        relative: &str,
        content: &str,
        expected_sha256: &str,
    ) -> Result<crate::file_editor::EditorFile, String> {
        let current = self.read_editor_file(relative)?;
        if current.sha256 != expected_sha256 {
            return Err("The file changed on disk. Your draft is safe. Copy it or reload the disk version before saving; no changes were overwritten.".into());
        }
        if current.read_only {
            return Err("This file is read-only.".into());
        }
        let bytes = current.encode(content)?;
        let path = self.resolve_existing(relative)?;
        // Prepare a complete replacement before the final optimistic revision check.
        let metadata = fs::metadata(&path).map_err(|error| error.to_string())?;
        let mut staged = create_atomic_temp(&path)?;
        staged
            .as_file()
            .set_permissions(metadata.permissions())
            .map_err(|error| error.to_string())?;
        write_and_sync_temp(&mut staged, &bytes)?;
        let checked_path = self.resolve_existing(relative)?;
        if checked_path != path || self.read_editor_file(relative)?.sha256 != expected_sha256 {
            return Err("The file changed during save. Your draft was retained; retry after reviewing the disk version.".into());
        }
        let staged = staged.into_temp_path();
        atomic_replace_file(staged.as_ref(), &path)
            .map_err(|error| format!("Could not save the file: {error}"))?;
        sync_parent_directory(&path).map_err(|error| error.to_string())?;
        Ok(crate::file_editor::EditorFile {
            sha256: sha256_hex(&bytes),
            content: content.into(),
            ..current
        })
    }

    /// Builds a lightweight workspace view rooted at a directory that has already been selected
    /// for an agent run. Unlike `connect`, this intentionally does not crawl the directory for the
    /// explorer: tool execution only needs the validated root and must stay inexpensive.
    pub(crate) fn scoped(path: &Path) -> Result<Self, String> {
        let canonical = fs::canonicalize(path)
            .map_err(|error| format!("Could not open the agent workspace scope: {error}"))?;
        let metadata = fs::metadata(&canonical)
            .map_err(|error| format!("Could not inspect the agent workspace scope: {error}"))?;
        if !metadata.is_dir() {
            return Err("The agent workspace scope must be a directory".to_owned());
        }
        Ok(Self {
            root: Some(canonical.clone()),
            project_roots: vec![canonical],
            ..Self::default()
        })
    }

    pub(crate) fn restore_projects(
        paths: Vec<PathBuf>,
        archived_paths: Vec<PathBuf>,
        active: Option<PathBuf>,
    ) -> Self {
        let mut runtime = Self::default();

        for path in paths {
            if let Ok(canonical) = canonical_workspace_root(&path) {
                push_unique_project(&mut runtime.project_roots, canonical);
            }
        }

        for path in archived_paths {
            if let Ok(canonical) = canonical_workspace_root(&path)
                && !runtime
                    .project_roots
                    .iter()
                    .any(|project| same_project_root(project, &canonical))
            {
                push_unique_project(&mut runtime.archived_project_roots, canonical);
            }
        }

        let active = active.and_then(|path| match canonical_workspace_root(&path) {
            Ok(canonical) => {
                push_unique_project(&mut runtime.project_roots, canonical.clone());
                Some(canonical)
            }
            Err(error) => {
                runtime.restore_error = Some(error);
                None
            }
        });

        runtime.root = active.or_else(|| runtime.project_roots.first().cloned());
        if runtime.root.is_some() {
            runtime.refresh_explorer();
        }
        runtime
    }

    pub(crate) fn connect(&mut self, path: PathBuf) -> Result<(), String> {
        let canonical = canonical_workspace_root(&path)?;
        self.archived_project_roots
            .retain(|project| !same_project_root(project, &canonical));
        push_unique_project(&mut self.project_roots, canonical.clone());
        self.root = Some(canonical);
        self.restore_error = None;
        self.refresh_explorer();
        Ok(())
    }

    pub(crate) fn activate(&mut self, root: &str) -> Result<(), String> {
        let selected = self
            .project_roots
            .iter()
            .find(|candidate| candidate.display().to_string() == root)
            .cloned()
            .ok_or_else(|| "The selected project is not connected to Supervisor".to_owned())?;
        self.root = Some(selected);
        self.restore_error = None;
        self.refresh_explorer();
        Ok(())
    }

    pub(crate) fn disconnect(&mut self) {
        self.root = None;
        self.project_roots.clear();
        self.archived_project_roots.clear();
        self.restore_error = None;
        self.explorer_entries.clear();
        self.explorer_truncated = false;
        self.explorer_error = None;
    }

    pub(crate) fn deactivate(&mut self) {
        self.root = None;
        self.restore_error = None;
        self.explorer_entries.clear();
        self.explorer_truncated = false;
        self.explorer_error = None;
    }

    pub(crate) fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub(crate) fn project_roots(&self) -> &[PathBuf] {
        &self.project_roots
    }

    pub(crate) fn archived_project_roots(&self) -> &[PathBuf] {
        &self.archived_project_roots
    }

    pub(crate) fn archive_project(&mut self, root: &str) -> Result<(), String> {
        let index = self
            .project_roots
            .iter()
            .position(|candidate| candidate.display().to_string() == root)
            .ok_or_else(|| "The selected project is not connected".to_owned())?;
        let archived = self.project_roots.remove(index);
        push_unique_project(&mut self.archived_project_roots, archived.clone());
        if self
            .root
            .as_ref()
            .is_some_and(|active| same_project_root(active, &archived))
        {
            self.root = self.project_roots.first().cloned();
            self.refresh_explorer();
        }
        Ok(())
    }

    pub(crate) fn restore_project(&mut self, root: &str) -> Result<(), String> {
        let index = self
            .archived_project_roots
            .iter()
            .position(|candidate| candidate.display().to_string() == root)
            .ok_or_else(|| "The selected archived project is unavailable".to_owned())?;
        let restored = canonical_workspace_root(&self.archived_project_roots[index])?;
        self.archived_project_roots.remove(index);
        push_unique_project(&mut self.project_roots, restored.clone());
        if self.root.is_none() {
            self.root = Some(restored);
            self.refresh_explorer();
        }
        Ok(())
    }

    pub(crate) fn eject_project(&mut self, root: &str) -> Result<(), String> {
        let mut removed = None;
        if let Some(index) = self
            .project_roots
            .iter()
            .position(|candidate| candidate.display().to_string() == root)
        {
            removed = Some(self.project_roots.remove(index));
        } else if let Some(index) = self
            .archived_project_roots
            .iter()
            .position(|candidate| candidate.display().to_string() == root)
        {
            removed = Some(self.archived_project_roots.remove(index));
        }
        let removed = removed.ok_or_else(|| "The selected project is not connected".to_owned())?;
        if self
            .root
            .as_ref()
            .is_some_and(|active| same_project_root(active, &removed))
        {
            self.root = self.project_roots.first().cloned();
            self.refresh_explorer();
        }
        Ok(())
    }

    pub(crate) fn view(&self) -> WorkspaceView {
        let root = self.root.as_ref();
        let name = root.map(|path| workspace_name(path));
        let projects = self
            .project_roots
            .iter()
            .map(|project| WorkspaceProjectView {
                root: project.display().to_string(),
                name: workspace_name(project),
                active: root.is_some_and(|active| same_project_root(active, project)),
                archived: false,
                pinned: false,
                agent_active: false,
                checkpoint_pending: false,
                removal_pending: false,
            })
            .chain(
                self.archived_project_roots
                    .iter()
                    .map(|project| WorkspaceProjectView {
                        root: project.display().to_string(),
                        name: workspace_name(project),
                        active: false,
                        archived: true,
                        pinned: false,
                        agent_active: false,
                        checkpoint_pending: false,
                        removal_pending: false,
                    }),
            )
            .collect::<Vec<_>>();
        WorkspaceView {
            connected: root.is_some(),
            root: root.map(|path| path.display().to_string()),
            name,
            projects,
            status: if root.is_some() {
                format!(
                    "{} project{} connected. Agent file paths are relative to the active project.",
                    self.project_roots.len(),
                    if self.project_roots.len() == 1 {
                        ""
                    } else {
                        "s"
                    }
                )
            } else if let Some(error) = &self.restore_error {
                format!("The saved workspace is unavailable: {error}")
            } else {
                "No workspace connected".to_owned()
            },
            capabilities: vec![
                "list_files",
                "read_file",
                "search_text",
                "apply_patch",
                "run_command",
            ],
            entries: self.explorer_entries.clone(),
            truncated: self.explorer_truncated,
            explorer_error: self.explorer_error.clone(),
        }
    }

    pub(crate) fn refresh_explorer(&mut self) {
        self.explorer_entries.clear();
        self.explorer_truncated = false;
        self.explorer_error = None;
        let Some(root) = self.root.clone() else {
            return;
        };
        let mut queue = VecDeque::from([(root.clone(), 0_usize)]);

        while let Some((directory, depth)) = queue.pop_front() {
            let mut children = match fs::read_dir(&directory) {
                Ok(children) => children.filter_map(Result::ok).collect::<Vec<_>>(),
                Err(error) => {
                    self.explorer_error.get_or_insert_with(|| {
                        format!("Part of the workspace could not be listed: {error}")
                    });
                    continue;
                }
            };
            children.sort_by(|left, right| {
                let left_directory = left.file_type().is_ok_and(|kind| kind.is_dir());
                let right_directory = right.file_type().is_ok_and(|kind| kind.is_dir());
                right_directory.cmp(&left_directory).then_with(|| {
                    left.file_name()
                        .to_string_lossy()
                        .to_lowercase()
                        .cmp(&right.file_name().to_string_lossy().to_lowercase())
                })
            });

            for child in children {
                if self.explorer_entries.len() >= MAX_EXPLORER_ENTRIES {
                    self.explorer_truncated = true;
                    return;
                }
                let path = child.path();
                let Ok(metadata) = fs::symlink_metadata(&path) else {
                    continue;
                };
                let Ok(relative) = path.strip_prefix(&root) else {
                    continue;
                };
                let display = relative_display(relative);
                let link_like = is_link_like(&metadata);
                if metadata.is_dir() && !link_like {
                    if should_skip_directory(&path, &root) {
                        continue;
                    }
                    self.explorer_entries.push(WorkspaceExplorerEntry {
                        path: display,
                        kind: "directory",
                        bytes: None,
                        icon_key: "folder",
                    });
                    if depth < MAX_EXPLORER_DEPTH {
                        queue.push_back((path, depth + 1));
                    } else {
                        self.explorer_truncated = true;
                    }
                } else if metadata.is_file() && !link_like {
                    self.explorer_entries.push(WorkspaceExplorerEntry {
                        icon_key: file_icon_key(&display),
                        path: display,
                        kind: "file",
                        bytes: Some(metadata.len()),
                    });
                } else if link_like {
                    self.explorer_entries.push(WorkspaceExplorerEntry {
                        path: display,
                        kind: "link",
                        bytes: None,
                        icon_key: "file",
                    });
                }
            }
        }
    }

    pub(crate) fn report_explorer_error(&mut self, error: String) {
        self.explorer_error = Some(error);
    }

    pub(crate) fn create_file(&mut self, relative: &str) -> Result<(), String> {
        let path = self.resolve_new_entry(relative)?;
        write_new_atomically(&path, b"")?;
        self.refresh_explorer();
        Ok(())
    }

    pub(crate) fn create_directory(&mut self, relative: &str) -> Result<(), String> {
        let path = self.resolve_new_entry(relative)?;
        fs::create_dir(&path)
            .map_err(|error| format!("Could not create the workspace folder: {error}"))?;
        sync_parent_directory(&path)
            .map_err(|error| format!("Could not sync the workspace directory: {error}"))?;
        self.refresh_explorer();
        Ok(())
    }

    pub(crate) fn import_files(
        &mut self,
        directory: &str,
        sources: Vec<PathBuf>,
    ) -> Result<usize, String> {
        if sources.is_empty() {
            return Ok(0);
        }
        let target_directory = self.resolve_directory(Some(directory))?;
        if self.is_sensitive_path(&target_directory) {
            return Err("Supervisor does not import files into known secret paths".to_owned());
        }
        let root = self.connected_root()?;
        let target_relative = target_directory
            .strip_prefix(root)
            .map_err(|_| "The import destination escaped its project root".to_owned())?;
        let mut destination_keys = HashSet::new();
        let mut planned = Vec::with_capacity(sources.len());

        for source in sources {
            let metadata = fs::symlink_metadata(&source)
                .map_err(|error| format!("Could not inspect an imported file: {error}"))?;
            if is_link_like(&metadata) || !metadata.is_file() {
                return Err(format!(
                    "Only regular files can be imported: {}",
                    source.display()
                ));
            }
            if is_sensitive_path(&source) {
                return Err(format!(
                    "Supervisor does not import known secret-file paths: {}",
                    source.display()
                ));
            }
            let file_name = source
                .file_name()
                .ok_or_else(|| "An imported file name is invalid".to_owned())?;
            let relative =
                checked_relative_path(&relative_display(&target_relative.join(file_name)))?;
            if relative.as_os_str().is_empty() || is_sensitive_path(&relative) {
                return Err("Supervisor does not import known secret-file paths".to_owned());
            }
            let destination = target_directory.join(file_name);
            ensure_new_entry_available(&destination)?;
            if !destination_keys.insert(filesystem_collision_key(&destination)) {
                return Err(format!(
                    "More than one selected file would be imported as {}",
                    file_name.to_string_lossy()
                ));
            }
            planned.push((source, destination));
        }

        let imported = planned.len();
        for (source, destination) in planned {
            copy_new_atomically(&source, &destination)?;
        }
        self.refresh_explorer();
        Ok(imported)
    }

    pub(crate) fn execute(&self, command: &WorkspaceCommand) -> Result<Value, String> {
        match command {
            WorkspaceCommand::State => self.state(),
            WorkspaceCommand::List {
                path,
                depth,
                max_entries,
            } => self.list(path.as_deref(), *depth as usize, *max_entries),
            WorkspaceCommand::Read {
                path,
                start_line,
                line_count,
            } => self.read(path, *start_line, *line_count),
            WorkspaceCommand::Search {
                query,
                path,
                case_sensitive,
                max_results,
            } => self.search(query, path.as_deref(), *case_sensitive, *max_results),
            WorkspaceCommand::ApplyPatch {
                path,
                expected_sha256,
                replacements,
                create_content,
            } => self.apply_patch(
                path,
                expected_sha256.as_deref(),
                replacements,
                create_content.as_deref(),
            ),
            WorkspaceCommand::Run { .. } => {
                Err("Workspace commands must be run through the managed process adapter".to_owned())
            }
        }
    }

    pub(crate) fn resolve_directory(&self, relative: Option<&str>) -> Result<PathBuf, String> {
        let path = self.resolve_existing(relative.unwrap_or_default())?;
        if !path.is_dir() {
            return Err("The workspace command directory is not a directory".to_owned());
        }
        Ok(path)
    }

    fn state(&self) -> Result<Value, String> {
        let root = self.connected_root()?;
        let name = root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "workspace".to_owned());
        Ok(json!({
            "connected": true,
            "name": name,
            "pathPolicy": "relative_paths_only",
            "writePolicy": "exact_replacements_with_sha256",
            "processPolicy": "managed_job_object_current_user_authority"
        }))
    }

    fn list(
        &self,
        relative: Option<&str>,
        max_depth: usize,
        max_entries: usize,
    ) -> Result<Value, String> {
        let start = self.resolve_existing(relative.unwrap_or_default())?;
        if !start.is_dir() {
            return Err("The workspace list path is not a directory".to_owned());
        }

        let mut queue = VecDeque::from([(start.clone(), 0_usize)]);
        let mut entries = Vec::new();
        let mut truncated = false;

        while let Some((directory, depth)) = queue.pop_front() {
            let mut children = fs::read_dir(&directory)
                .map_err(|error| format!("Could not list the workspace directory: {error}"))?
                .filter_map(Result::ok)
                .collect::<Vec<_>>();
            children.sort_by_key(|entry| entry.file_name().to_string_lossy().to_lowercase());

            for child in children {
                if entries.len() >= max_entries {
                    truncated = true;
                    break;
                }
                let path = child.path();
                let metadata = fs::symlink_metadata(&path)
                    .map_err(|error| format!("Could not inspect a workspace entry: {error}"))?;
                let link_like = is_link_like(&metadata);
                let kind = if link_like {
                    "link"
                } else if metadata.is_dir() {
                    "directory"
                } else if metadata.is_file() {
                    "file"
                } else {
                    "other"
                };
                entries.push(WorkspaceEntry {
                    path: self.relative_display(&path)?,
                    kind,
                    bytes: metadata.is_file().then_some(metadata.len()),
                });

                if metadata.is_dir() && !link_like && depth < max_depth {
                    queue.push_back((path, depth + 1));
                }
            }
            if truncated {
                break;
            }
        }

        Ok(json!({
            "path": self.relative_display(&start)?,
            "entries": entries,
            "truncated": truncated
        }))
    }

    fn read(&self, relative: &str, start_line: usize, line_count: usize) -> Result<Value, String> {
        let path = self.resolve_existing(relative)?;
        self.ensure_readable_file(&path)?;
        let bytes = fs::read(&path)
            .map_err(|error| format!("Could not read the workspace file: {error}"))?;
        let text = String::from_utf8(bytes)
            .map_err(|_| "Workspace V1 reads UTF-8 text files only".to_owned())?;
        let total_lines = text.lines().count();
        let selected = text
            .lines()
            .skip(start_line.saturating_sub(1))
            .take(line_count)
            .collect::<Vec<_>>()
            .join("\n");
        let returned_lines = selected.lines().count();

        Ok(json!({
            "path": self.relative_display(&path)?,
            "startLine": start_line,
            "endLine": start_line.saturating_add(returned_lines.saturating_sub(1)),
            "totalLines": total_lines,
            "content": selected,
            "sha256": sha256_hex(text.as_bytes()),
            "truncated": start_line.saturating_sub(1) + returned_lines < total_lines
        }))
    }

    fn search(
        &self,
        query: &str,
        relative: Option<&str>,
        case_sensitive: bool,
        max_results: usize,
    ) -> Result<Value, String> {
        let start = self.resolve_existing(relative.unwrap_or_default())?;
        let mut queue = VecDeque::from([(start.clone(), 0_usize)]);
        let mut matches = Vec::new();
        let mut inspected_files = 0_usize;
        let needle = if case_sensitive {
            query.to_owned()
        } else {
            query.to_lowercase()
        };

        while let Some((path, depth)) = queue.pop_front() {
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("Could not inspect the workspace search path: {error}"))?;
            if is_link_like(&metadata) {
                continue;
            }
            if metadata.is_dir() {
                if depth >= MAX_SEARCH_DEPTH || should_skip_directory(&path, &start) {
                    continue;
                }
                let mut children = fs::read_dir(&path)
                    .map_err(|error| format!("Could not search the workspace directory: {error}"))?
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .collect::<Vec<_>>();
                children.sort();
                for child in children {
                    queue.push_back((child, depth + 1));
                }
                continue;
            }
            if !metadata.is_file()
                || metadata.len() > MAX_SEARCH_FILE_BYTES
                || self.is_sensitive_path(&path)
            {
                continue;
            }
            inspected_files += 1;
            if inspected_files > MAX_SEARCH_FILES {
                break;
            }
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            let Ok(text) = String::from_utf8(bytes) else {
                continue;
            };
            for (index, line) in text.lines().enumerate() {
                let haystack = if case_sensitive {
                    line.to_owned()
                } else {
                    line.to_lowercase()
                };
                if haystack.contains(&needle) {
                    matches.push(SearchMatch {
                        path: self.relative_display(&path)?,
                        line: index + 1,
                        preview: truncate_chars(line.trim(), 300),
                    });
                    if matches.len() >= max_results {
                        return Ok(json!({
                            "query": query,
                            "matches": matches,
                            "inspectedFiles": inspected_files,
                            "truncated": true
                        }));
                    }
                }
            }
        }

        Ok(json!({
            "query": query,
            "matches": matches,
            "inspectedFiles": inspected_files,
            "truncated": inspected_files >= MAX_SEARCH_FILES
        }))
    }

    fn apply_patch(
        &self,
        relative: &str,
        expected_sha256: Option<&str>,
        replacements: &[TextReplacement],
        create_content: Option<&str>,
    ) -> Result<Value, String> {
        let root = self.connected_root()?;
        let relative_path = checked_relative_path(relative)?;
        if is_sensitive_path(&relative_path) {
            return Err("Supervisor does not read or modify known secret-file paths".to_owned());
        }
        let candidate = root.join(&relative_path);

        if candidate.exists() {
            let path = self.resolve_existing(relative)?;
            self.ensure_readable_file(&path)?;
            let bytes = fs::read(&path)
                .map_err(|error| format!("Could not read the workspace file: {error}"))?;
            let mut text = String::from_utf8(bytes)
                .map_err(|_| "Workspace V1 patches UTF-8 text files only".to_owned())?;
            let previous_hash = sha256_hex(text.as_bytes());
            if expected_sha256 != Some(previous_hash.as_str()) {
                return Err(format!(
                    "The file changed after it was read. Read it again before patching (current sha256: {previous_hash})"
                ));
            }

            for replacement in replacements {
                let occurrences = text.match_indices(&replacement.old_text).count();
                if occurrences != 1 {
                    return Err(format!(
                        "An exact patch target occurred {occurrences} times; every oldText must match exactly once"
                    ));
                }
                text = text.replacen(&replacement.old_text, &replacement.new_text, 1);
            }
            if text.len() as u64 > MAX_FILE_BYTES {
                return Err("The patched file exceeds the Workspace V1 size limit".to_owned());
            }
            write_existing_atomically(&path, text.as_bytes())?;
            let new_hash = sha256_hex(text.as_bytes());
            return Ok(json!({
                "path": self.relative_display(&path)?,
                "created": false,
                "replacements": replacements.len(),
                "previousSha256": previous_hash,
                "sha256": new_hash,
                "bytes": text.len()
            }));
        }

        let content = create_content.ok_or_else(|| {
            "The target file does not exist; createContent is required".to_owned()
        })?;
        if content.len() as u64 > MAX_FILE_BYTES {
            return Err("The new file exceeds the Workspace V1 size limit".to_owned());
        }
        let parent = candidate
            .parent()
            .ok_or_else(|| "The workspace file has no parent directory".to_owned())?;
        let parent_relative = parent
            .strip_prefix(root)
            .map_err(|_| "The workspace file escaped its root".to_owned())?;
        let parent = self.resolve_existing(&relative_display(parent_relative))?;
        if !parent.is_dir() {
            return Err("The new file parent is not a directory".to_owned());
        }
        let path = parent.join(
            relative_path
                .file_name()
                .ok_or_else(|| "The workspace file name is invalid".to_owned())?,
        );
        write_new_atomically(&path, content.as_bytes())?;
        Ok(json!({
            "path": self.relative_display(&path)?,
            "created": true,
            "replacements": 0,
            "sha256": sha256_hex(content.as_bytes()),
            "bytes": content.len()
        }))
    }

    fn connected_root(&self) -> Result<&Path, String> {
        self.root.as_deref().ok_or_else(|| {
            "Connect a workspace in Settings before using workspace tools".to_owned()
        })
    }

    fn resolve_new_entry(&self, relative: &str) -> Result<PathBuf, String> {
        let root = self.connected_root()?;
        let relative = checked_relative_path(relative)?;
        if relative.as_os_str().is_empty() {
            return Err("Enter a file or folder name inside the active project".to_owned());
        }
        if is_sensitive_path(&relative) {
            return Err(
                "Supervisor does not create files or folders in known secret paths".to_owned(),
            );
        }
        let parent_relative = relative
            .parent()
            .ok_or_else(|| "The workspace entry has no parent directory".to_owned())?;
        let parent = self.resolve_existing(&relative_display(parent_relative))?;
        if !parent.is_dir() {
            return Err("The new workspace entry parent is not a directory".to_owned());
        }
        let file_name = relative
            .file_name()
            .ok_or_else(|| "The workspace entry name is invalid".to_owned())?;
        let path = parent.join(file_name);
        if !path.starts_with(root) {
            return Err("The new workspace entry escaped its project root".to_owned());
        }
        ensure_new_entry_available(&path)?;
        Ok(path)
    }

    fn resolve_existing(&self, relative: &str) -> Result<PathBuf, String> {
        let root = self.connected_root()?;
        let relative = checked_relative_path(relative)?;
        reject_link_components(root, &relative)?;
        let candidate = fs::canonicalize(root.join(&relative))
            .map_err(|error| format!("Workspace path is unavailable: {error}"))?;
        if !candidate.starts_with(root) {
            return Err("The workspace path escaped its connected root".to_owned());
        }
        Ok(candidate)
    }

    fn ensure_readable_file(&self, path: &Path) -> Result<(), String> {
        let metadata = fs::metadata(path)
            .map_err(|error| format!("Could not inspect the workspace file: {error}"))?;
        if !metadata.is_file() {
            return Err("The workspace path is not a regular file".to_owned());
        }
        if metadata.len() > MAX_FILE_BYTES {
            return Err("The workspace file exceeds the 1 MB V1 limit".to_owned());
        }
        if self.is_sensitive_path(path) {
            return Err("Supervisor does not read or modify known secret-file paths".to_owned());
        }
        Ok(())
    }

    fn is_sensitive_path(&self, path: &Path) -> bool {
        self.root
            .as_ref()
            .and_then(|root| path.strip_prefix(root).ok())
            .is_some_and(is_sensitive_path)
    }

    fn relative_display(&self, path: &Path) -> Result<String, String> {
        let root = self.connected_root()?;
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "The workspace result escaped its root".to_owned())?;
        let display = relative_display(relative);
        Ok(if display.is_empty() {
            ".".to_owned()
        } else {
            display
        })
    }
}

fn canonical_workspace_root(path: &Path) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("Could not open the selected workspace: {error}"))?;
    let metadata = fs::metadata(&canonical)
        .map_err(|error| format!("Could not inspect the selected workspace: {error}"))?;
    if !metadata.is_dir() {
        return Err("The selected workspace must be a directory".to_owned());
    }
    Ok(canonical)
}

fn push_unique_project(projects: &mut Vec<PathBuf>, project: PathBuf) {
    if !projects
        .iter()
        .any(|candidate| same_project_root(candidate, &project))
    {
        projects.push(project);
    }
}

fn workspace_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn same_project_root(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn filesystem_collision_key(path: &Path) -> String {
    #[cfg(windows)]
    {
        path.to_string_lossy().to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    {
        path.to_string_lossy().into_owned()
    }
}

fn checked_relative_path(value: &str) -> Result<PathBuf, String> {
    if value.chars().count() > MAX_WORKSPACE_PATH_CHARS {
        return Err("The workspace path is too long".to_owned());
    }
    let path = Path::new(value.trim());
    if path.is_absolute() {
        return Err("Workspace tools accept relative paths only".to_owned());
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => {
                let value_text = value.to_string_lossy();
                if value_text.contains(':') || value_text.contains('\0') {
                    return Err(
                        "Workspace paths cannot contain alternate streams or NULs".to_owned()
                    );
                }
                normalized.push(value);
            }
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err("Workspace paths cannot traverse outside the connected root".to_owned());
            }
        }
    }
    Ok(normalized)
}

/// Check a user-selected diff path, including a missing deleted tracked file.
pub(crate) fn checked_review_path(root: &Path, value: &str) -> Result<PathBuf, String> {
    if value != value.trim() || value.chars().any(char::is_control) {
        return Err("This path cannot be safely represented in the diff viewer.".into());
    }
    let relative = checked_relative_path(value)?;
    if relative.as_os_str().is_empty() || is_sensitive_path(&relative) {
        return Err(
            "Known credential and secret paths are excluded from local diff previews.".into(),
        );
    }
    reject_link_components(root, &relative)?;
    let target = root.join(relative);
    match fs::symlink_metadata(&target) {
        Ok(metadata) if !metadata.is_file() => {
            return Err(
                "This entry is not an ordinary file. Submodules and links are not traversed."
                    .into(),
            );
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err(format!("Could not inspect this path: {error}"));
        }
        _ => {}
    }
    // A deleted tracked file is valid; only its repository blobs will be read.
    Ok(target)
}

fn ensure_new_entry_available(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(format!(
            "A file or folder named {} already exists",
            path.file_name()
                .map(|name| name.to_string_lossy())
                .unwrap_or_else(|| path.as_os_str().to_string_lossy())
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "Could not validate the new workspace entry: {error}"
        )),
    }
}

fn reject_link_components(root: &Path, relative: &Path) -> Result<(), String> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        if let Component::Normal(value) = component {
            current.push(value);
            match fs::symlink_metadata(&current) {
                Ok(metadata) if is_link_like(&metadata) => {
                    return Err("Workspace paths cannot cross links or reparse points".to_owned());
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Err(error) => {
                    return Err(format!("Could not validate the workspace path: {error}"));
                }
            }
        }
    }
    Ok(())
}

fn is_link_like(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn should_skip_directory(path: &Path, search_root: &Path) -> bool {
    if path == search_root {
        return false;
    }
    path.file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .is_some_and(|name| {
            matches!(
                name.as_str(),
                ".git" | ".svn" | ".hg" | "node_modules" | "target" | ".idea" | ".vs"
            )
        })
}

fn write_existing_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("Could not inspect the workspace file for writing: {error}"))?;
    if !metadata.is_file() {
        return Err("The workspace patch target is not a regular file".to_owned());
    }
    if metadata.permissions().readonly() {
        return Err("Could not write the workspace file because it is read-only".to_owned());
    }

    let mut temporary = create_atomic_temp(path)?;
    temporary
        .as_file()
        .set_permissions(metadata.permissions())
        .map_err(|error| format!("Could not prepare the atomic workspace file: {error}"))?;
    write_and_sync_temp(&mut temporary, bytes)?;

    let temporary_path = temporary.into_temp_path();
    atomic_replace_file(temporary_path.as_ref(), path)
        .map_err(|error| format!("Could not atomically replace the workspace file: {error}"))?;
    sync_parent_directory(path)
        .map_err(|error| format!("Could not sync the workspace directory: {error}"))
}

fn write_new_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut temporary = create_atomic_temp(path)?;
    write_and_sync_temp(&mut temporary, bytes)?;

    let committed = temporary
        .persist_noclobber(path)
        .map_err(|error| format!("Could not create the workspace file: {}", error.error))?;
    committed
        .sync_all()
        .map_err(|error| format!("Could not sync the new workspace file: {error}"))?;
    sync_parent_directory(path)
        .map_err(|error| format!("Could not sync the workspace directory: {error}"))
}

fn copy_new_atomically(source: &Path, destination: &Path) -> Result<(), String> {
    let mut source_file = fs::File::open(source)
        .map_err(|error| format!("Could not open the imported file: {error}"))?;
    let mut temporary = create_atomic_temp(destination)?;
    std::io::copy(&mut source_file, temporary.as_file_mut())
        .and_then(|_| temporary.as_file_mut().flush())
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| format!("Could not stage the imported file: {error}"))?;

    let committed = temporary
        .persist_noclobber(destination)
        .map_err(|error| format!("Could not import the workspace file: {}", error.error))?;
    committed
        .sync_all()
        .map_err(|error| format!("Could not sync the imported workspace file: {error}"))?;
    sync_parent_directory(destination)
        .map_err(|error| format!("Could not sync the workspace directory: {error}"))
}

fn create_atomic_temp(path: &Path) -> Result<tempfile::NamedTempFile, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "The workspace file has no parent directory".to_owned())?;
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "workspace-file".into());
    tempfile::Builder::new()
        .prefix(&format!(".{file_name}.central-agent-"))
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(|error| format!("Could not stage the atomic workspace file: {error}"))
}

fn write_and_sync_temp(
    temporary: &mut tempfile::NamedTempFile,
    bytes: &[u8],
) -> Result<(), String> {
    temporary
        .write_all(bytes)
        .and_then(|_| temporary.flush())
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| format!("Could not write the staged workspace file: {error}"))
}

#[cfg(windows)]
fn atomic_replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    use windows::{
        Win32::Storage::FileSystem::{REPLACEFILE_WRITE_THROUGH, ReplaceFileW},
        core::PCWSTR,
    };

    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    unsafe {
        ReplaceFileW(
            PCWSTR(destination.as_ptr()),
            PCWSTR(source.as_ptr()),
            PCWSTR::null(),
            REPLACEFILE_WRITE_THROUGH,
            None,
            None,
        )
    }
    .map_err(std::io::Error::other)
}

#[cfg(not(windows))]
fn atomic_replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn relative_display(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_no_atomic_staging_files(directory: &Path) {
        let names = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(
            names
                .iter()
                .all(|name| !name.contains(".central-agent-") || !name.ends_with(".tmp")),
            "atomic workspace staging files were not cleaned up: {names:?}"
        );
    }

    fn runtime() -> (tempfile::TempDir, WorkspaceRuntime) {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("src")).unwrap();
        fs::write(
            directory.path().join("src/main.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .unwrap();
        let mut runtime = WorkspaceRuntime::default();
        runtime.connect(directory.path().to_path_buf()).unwrap();
        (directory, runtime)
    }

    #[test]
    fn rejects_absolute_and_parent_paths() {
        let (_directory, runtime) = runtime();
        assert!(runtime.read("../outside.txt", 1, 20).is_err());
        assert!(runtime.read("C:/Windows/win.ini", 1, 20).is_err());
    }

    #[test]
    fn scoped_runtime_uses_its_own_root_without_building_an_explorer() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path().join("project");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("local.txt"), "scoped").unwrap();
        fs::write(directory.path().join("outside.txt"), "outside").unwrap();

        let runtime = WorkspaceRuntime::scoped(&project).unwrap();
        assert_eq!(
            runtime.root(),
            Some(project.canonicalize().unwrap().as_path())
        );
        assert!(runtime.view().entries.is_empty());
        assert!(runtime.read("local.txt", 1, 10).is_ok());
        assert!(runtime.read("../outside.txt", 1, 10).is_err());
    }

    #[test]
    fn keeps_multiple_projects_and_switches_only_to_a_connected_root() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        fs::write(first.path().join("first.txt"), "first").unwrap();
        fs::write(second.path().join("second.txt"), "second").unwrap();

        let mut runtime = WorkspaceRuntime::default();
        runtime.connect(first.path().to_path_buf()).unwrap();
        runtime.connect(second.path().to_path_buf()).unwrap();
        let first_root = first.path().canonicalize().unwrap();
        let second_root = second.path().canonicalize().unwrap();

        assert_eq!(runtime.project_roots().len(), 2);
        assert_eq!(runtime.root(), Some(second_root.as_path()));
        runtime.activate(&first_root.display().to_string()).unwrap();
        assert_eq!(runtime.root(), Some(first_root.as_path()));
        assert!(runtime.view().projects[0].active);
        assert!(!runtime.view().projects[1].active);
        assert!(runtime.activate("not-a-connected-project").is_err());
    }

    #[test]
    fn connecting_the_same_canonical_project_twice_does_not_duplicate_it() {
        let project = tempfile::tempdir().unwrap();
        let mut runtime = WorkspaceRuntime::default();
        runtime.connect(project.path().to_path_buf()).unwrap();
        runtime.connect(project.path().join(".")).unwrap();

        assert_eq!(runtime.project_roots().len(), 1);
        assert_eq!(
            runtime.root(),
            Some(project.path().canonicalize().unwrap().as_path())
        );
    }

    #[test]
    fn restores_saved_projects_with_the_previous_active_project() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let second_root = second.path().canonicalize().unwrap();
        let runtime = WorkspaceRuntime::restore_projects(
            vec![first.path().to_path_buf(), second.path().to_path_buf()],
            Vec::new(),
            Some(second.path().to_path_buf()),
        );

        assert_eq!(runtime.project_roots().len(), 2);
        assert_eq!(runtime.root(), Some(second_root.as_path()));
        assert_eq!(
            runtime
                .view()
                .projects
                .iter()
                .filter(|project| project.active)
                .count(),
            1
        );
    }

    #[test]
    fn archives_restores_and_ejects_individual_projects() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let first_root = first.path().canonicalize().unwrap();
        let second_root = second.path().canonicalize().unwrap();
        let mut runtime = WorkspaceRuntime::restore_projects(
            vec![first.path().to_path_buf(), second.path().to_path_buf()],
            Vec::new(),
            Some(first.path().to_path_buf()),
        );

        runtime
            .archive_project(&first_root.display().to_string())
            .unwrap();
        assert_eq!(runtime.root(), Some(second_root.as_path()));
        assert_eq!(runtime.project_roots().len(), 1);
        assert_eq!(runtime.archived_project_roots().len(), 1);
        assert!(
            runtime
                .view()
                .projects
                .iter()
                .any(|project| project.archived)
        );

        runtime
            .restore_project(&first_root.display().to_string())
            .unwrap();
        assert_eq!(runtime.project_roots().len(), 2);
        assert!(runtime.archived_project_roots().is_empty());

        runtime
            .eject_project(&first_root.display().to_string())
            .unwrap();
        assert_eq!(runtime.project_roots().len(), 1);
        assert!(
            runtime
                .view()
                .projects
                .iter()
                .all(|project| project.root != first_root.display().to_string())
        );
    }

    #[test]
    fn reads_lists_and_searches_bounded_utf8_files() {
        let (_directory, runtime) = runtime();
        let listing = runtime.list(Some("src"), 1, 20).unwrap();
        assert_eq!(listing["entries"][0]["path"], "src/main.rs");

        let read = runtime.read("src/main.rs", 1, 20).unwrap();
        assert!(read["content"].as_str().unwrap().contains("println"));
        assert_eq!(read["sha256"].as_str().unwrap().len(), 64);

        let search = runtime.search("hello", None, false, 10).unwrap();
        assert_eq!(search["matches"][0]["line"], 2);
    }

    #[test]
    fn explorer_is_cached_bounded_and_uses_local_file_icons() {
        let (directory, mut runtime) = runtime();
        fs::write(
            directory.path().join("Cargo.toml"),
            "[package]\nname='demo'\n",
        )
        .unwrap();
        fs::write(directory.path().join("package.json"), "{}").unwrap();
        fs::write(directory.path().join("Dockerfile"), "FROM scratch\n").unwrap();
        fs::create_dir(directory.path().join("target")).unwrap();
        fs::write(directory.path().join("target/generated.rs"), "generated").unwrap();
        runtime.refresh_explorer();

        let view = runtime.view();
        assert!(view.entries.iter().any(|entry| {
            entry.path == "Cargo.toml" && entry.kind == "file" && entry.icon_key == "cargo"
        }));
        assert!(view.entries.iter().any(|entry| {
            entry.path == "package.json" && entry.kind == "file" && entry.icon_key == "npm"
        }));
        assert!(view.entries.iter().any(|entry| {
            entry.path == "Dockerfile" && entry.kind == "file" && entry.icon_key == "docker"
        }));
        assert!(
            !view
                .entries
                .iter()
                .any(|entry| entry.path.starts_with("target"))
        );
        assert!(!view.truncated);
    }

    #[test]
    fn patches_only_the_exact_version_that_was_read() {
        let (directory, runtime) = runtime();
        let read = runtime.read("src/main.rs", 1, 20).unwrap();
        let hash = read["sha256"].as_str().unwrap();
        let result = runtime
            .apply_patch(
                "src/main.rs",
                Some(hash),
                &[TextReplacement {
                    old_text: "hello".to_owned(),
                    new_text: "workspace".to_owned(),
                }],
                None,
            )
            .unwrap();
        assert_eq!(result["created"], false);
        assert!(
            fs::read_to_string(directory.path().join("src/main.rs"))
                .unwrap()
                .contains("workspace")
        );
        assert!(
            runtime
                .apply_patch(
                    "src/main.rs",
                    Some(hash),
                    &[TextReplacement {
                        old_text: "workspace".to_owned(),
                        new_text: "stale".to_owned(),
                    }],
                    None,
                )
                .is_err()
        );
    }

    #[test]
    fn creates_only_inside_an_existing_workspace_directory() {
        let (directory, runtime) = runtime();
        runtime
            .apply_patch("src/lib.rs", None, &[], Some("pub fn ready() {}\n"))
            .unwrap();
        assert!(directory.path().join("src/lib.rs").is_file());
        assert!(
            runtime
                .apply_patch("missing/lib.rs", None, &[], Some("no"))
                .is_err()
        );
    }

    #[test]
    fn explorer_actions_create_files_and_folders_without_overwriting() {
        let (directory, mut runtime) = runtime();

        runtime.create_directory("src/generated").unwrap();
        runtime.create_file("src/generated/mod.rs").unwrap();

        let created = directory.path().join("src/generated/mod.rs");
        assert!(created.is_file());
        assert!(
            runtime
                .view()
                .entries
                .iter()
                .any(|entry| entry.path == "src/generated/mod.rs")
        );
        fs::write(&created, "manual edit").unwrap();
        assert!(runtime.create_file("src/generated/mod.rs").is_err());
        assert_eq!(fs::read_to_string(&created).unwrap(), "manual edit");
        assert!(runtime.create_directory("../outside").is_err());
        assert!(runtime.create_file("missing/file.rs").is_err());
    }

    #[test]
    fn explorer_import_copies_regular_files_and_rejects_collisions_and_secrets() {
        let (directory, mut runtime) = runtime();
        let source_directory = tempfile::tempdir().unwrap();
        let source = source_directory.path().join("helper.rs");
        fs::write(&source, "pub fn helper() {}\n").unwrap();

        assert_eq!(
            runtime.import_files("src", vec![source.clone()]).unwrap(),
            1
        );
        let imported = directory.path().join("src/helper.rs");
        assert_eq!(
            fs::read_to_string(&imported).unwrap(),
            "pub fn helper() {}\n"
        );
        assert!(runtime.import_files("src", vec![source]).is_err());
        assert_eq!(
            fs::read_to_string(&imported).unwrap(),
            "pub fn helper() {}\n"
        );

        let secret = source_directory.path().join(".env");
        fs::write(&secret, "TOKEN=secret").unwrap();
        assert!(runtime.import_files("src", vec![secret]).is_err());
        assert!(!directory.path().join("src/.env").exists());
    }

    #[test]
    fn atomic_workspace_writes_commit_complete_files_without_staging_residue() {
        let directory = tempfile::tempdir().unwrap();
        let existing = directory.path().join("existing.txt");
        let created = directory.path().join("created.txt");
        fs::write(&existing, "before").unwrap();

        write_existing_atomically(&existing, b"after").unwrap();
        write_new_atomically(&created, b"created atomically").unwrap();

        assert_eq!(fs::read(&existing).unwrap(), b"after");
        assert_eq!(fs::read(&created).unwrap(), b"created atomically");
        assert_no_atomic_staging_files(directory.path());
    }

    #[test]
    fn failed_atomic_creation_does_not_clobber_or_leave_a_staging_file() {
        let directory = tempfile::tempdir().unwrap();
        let existing = directory.path().join("already-there.txt");
        fs::write(&existing, "original").unwrap();

        let result = write_new_atomically(&existing, b"replacement");

        assert!(result.is_err());
        assert_eq!(fs::read(&existing).unwrap(), b"original");
        assert_no_atomic_staging_files(directory.path());
    }

    #[test]
    fn refuses_known_secret_file_contents() {
        let (directory, runtime) = runtime();
        fs::write(directory.path().join(".env"), "TOKEN=secret").unwrap();
        assert!(runtime.read(".env", 1, 20).is_err());
        let search = runtime.search("secret", None, false, 20).unwrap();
        assert!(search["matches"].as_array().unwrap().is_empty());
    }
}
