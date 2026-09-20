use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use similar::{ChangeTag, TextDiff};
use uuid::Uuid;

use crate::sensitive_path::is_sensitive_path;

const MANIFEST_VERSION: u32 = 1;
const TEXT_SAMPLE_BYTES: usize = 8 * 1024;
const DEFAULT_MAX_SNAPSHOT_FILE_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_MAX_CHECKPOINT_BYTES: u64 = 512 * 1024 * 1024;
const DEFAULT_MAX_RETAINED_CHECKPOINTS: usize = 50;

#[derive(Clone, Copy, Debug)]
struct TimeMachineLimits {
    max_file_bytes: u64,
    max_checkpoint_bytes: u64,
    max_retained_checkpoints: usize,
}

impl Default for TimeMachineLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: DEFAULT_MAX_SNAPSHOT_FILE_BYTES,
            max_checkpoint_bytes: DEFAULT_MAX_CHECKPOINT_BYTES,
            max_retained_checkpoints: DEFAULT_MAX_RETAINED_CHECKPOINTS,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ChangedFileStatus {
    Added,
    Modified,
    Deleted,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChangedFileView {
    pub path: String,
    pub status: ChangedFileStatus,
    pub icon_key: String,
    pub additions: usize,
    pub deletions: usize,
    pub binary: bool,
    pub restored: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckpointSummary {
    pub id: String,
    pub run_id: u64,
    pub workspace_name: String,
    pub provider: String,
    pub context: String,
    pub created_at_ms: u128,
    pub finished_at_ms: u128,
    pub file_count: usize,
    pub total_additions: usize,
    pub total_deletions: usize,
    pub warning_count: usize,
    pub restored_files: usize,
    pub fully_restored: bool,
    pub changes: Vec<ChangedFileView>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveDiffStats {
    pub run_id: u64,
    pub additions: usize,
    pub deletions: usize,
    pub file_count: usize,
    pub active: bool,
}

impl LiveDiffStats {
    pub(crate) fn empty(run_id: u64, active: bool) -> Self {
        Self {
            run_id,
            additions: 0,
            deletions: 0,
            file_count: 0,
            active,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectedCheckpointView {
    pub checkpoint: CheckpointSummary,
    pub selected_path: Option<String>,
    pub diff: Option<String>,
    pub binary: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TimeMachineView {
    pub available: bool,
    pub selected: Option<SelectedCheckpointView>,
    pub message: Option<String>,
    pub message_is_error: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotFile {
    sha256: String,
    bytes: u64,
    text: bool,
    readonly: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    modified_ns: Option<u128>,
}

#[derive(Clone, Debug)]
struct CachedSnapshot {
    bytes: u64,
    modified_ns: Option<u128>,
    readonly: bool,
    snapshot: SnapshotFile,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChangedFile {
    path: String,
    status: ChangedFileStatus,
    icon_key: String,
    additions: usize,
    deletions: usize,
    binary: bool,
    restored: bool,
    before: Option<SnapshotFile>,
    after: Option<SnapshotFile>,
}

impl ChangedFile {
    fn view(&self) -> ChangedFileView {
        ChangedFileView {
            path: self.path.clone(),
            status: self.status,
            icon_key: self.icon_key.clone(),
            additions: self.additions,
            deletions: self.deletions,
            binary: self.binary,
            restored: self.restored,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckpointManifest {
    version: u32,
    id: String,
    run_id: u64,
    workspace_root: PathBuf,
    workspace_name: String,
    provider: String,
    context: String,
    created_at_ms: u128,
    finished_at_ms: Option<u128>,
    baseline: BTreeMap<String, SnapshotFile>,
    changes: Vec<ChangedFile>,
    warnings: Vec<String>,
}

impl CheckpointManifest {
    fn summary(&self) -> Option<CheckpointSummary> {
        let finished_at_ms = self.finished_at_ms?;
        let changes = self
            .changes
            .iter()
            .map(ChangedFile::view)
            .collect::<Vec<_>>();
        let restored_files = changes.iter().filter(|change| change.restored).count();
        Some(CheckpointSummary {
            id: self.id.clone(),
            run_id: self.run_id,
            workspace_name: self.workspace_name.clone(),
            provider: self.provider.clone(),
            context: self.context.clone(),
            created_at_ms: self.created_at_ms,
            finished_at_ms,
            file_count: changes.len(),
            total_additions: changes.iter().map(|change| change.additions).sum(),
            total_deletions: changes.iter().map(|change| change.deletions).sum(),
            warning_count: skipped_warning_path_count(&self.warnings),
            restored_files,
            fully_restored: !changes.is_empty() && restored_files == changes.len(),
            changes,
        })
    }
}

#[derive(Clone, Debug)]
struct SelectedCheckpoint {
    checkpoint_id: String,
    selected_path: Option<String>,
}

pub(crate) struct TimeMachineRuntime {
    root: PathBuf,
    blobs: PathBuf,
    manifests: PathBuf,
    quarantine: PathBuf,
    restore_journals: PathBuf,
    available: bool,
    manifest_index_complete: bool,
    active_runs: HashMap<u64, String>,
    checkpoints: HashMap<String, CheckpointManifest>,
    snapshot_cache: HashMap<PathBuf, CachedSnapshot>,
    selected: Option<SelectedCheckpoint>,
    message: Option<String>,
    message_is_error: bool,
    maintenance_warning: Option<String>,
    recovered_summaries: Vec<CheckpointSummary>,
    limits: TimeMachineLimits,
    #[cfg(test)]
    restore_fail_after_applied: Option<usize>,
}

impl TimeMachineRuntime {
    pub(crate) fn open(root: PathBuf) -> Self {
        let blobs = root.join("blobs");
        let manifests = root.join("checkpoints");
        let quarantine = root.join("quarantine");
        let restore_journals = root.join("restore-journals");
        let mut runtime = Self {
            root,
            blobs,
            manifests,
            quarantine,
            restore_journals,
            available: true,
            manifest_index_complete: true,
            active_runs: HashMap::new(),
            checkpoints: HashMap::new(),
            snapshot_cache: HashMap::new(),
            selected: None,
            message: None,
            message_is_error: false,
            maintenance_warning: None,
            recovered_summaries: Vec::new(),
            limits: TimeMachineLimits::default(),
            #[cfg(test)]
            restore_fail_after_applied: None,
        };
        let initialization = runtime
            .prepare_store()
            .and_then(|_| runtime.recover_restore_journals())
            .and_then(|_| runtime.load_manifests());
        if let Err(error) = initialization {
            runtime.available = false;
            runtime.message = Some(error);
            runtime.message_is_error = true;
        } else {
            if let Err(error) = runtime
                .enforce_retention()
                .and_then(|_| runtime.garbage_collect_blobs())
            {
                runtime.add_maintenance_warning(format!(
                    "Time Machine is available, but storage cleanup was deferred: {error}"
                ));
            }
            runtime.rebuild_snapshot_cache_from_latest_checkpoint();
        }
        runtime
    }

    pub(crate) fn begin_run(
        &mut self,
        run_id: u64,
        workspace_root: &Path,
        provider: &str,
        context: &str,
    ) -> Result<(), String> {
        if !self.available {
            return Err(self
                .message
                .clone()
                .unwrap_or_else(|| "Time Machine storage is unavailable".to_owned()));
        }
        if self.active_runs.contains_key(&run_id) {
            return Ok(());
        }
        let workspace_root = fs::canonicalize(workspace_root)
            .map_err(|error| format!("Time Machine could not open the workspace: {error}"))?;
        let (baseline, mut warnings) = self.capture_tree(&workspace_root)?;
        deduplicate_checkpoint_warnings(&mut warnings);
        let id = Uuid::new_v4().to_string();
        let workspace_name = workspace_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "Workspace".to_owned());
        let manifest = CheckpointManifest {
            version: MANIFEST_VERSION,
            id: id.clone(),
            run_id,
            workspace_root,
            workspace_name,
            provider: provider.to_owned(),
            context: context.to_owned(),
            created_at_ms: unix_time_ms(),
            finished_at_ms: None,
            baseline,
            changes: Vec::new(),
            warnings,
        };
        self.persist_manifest(&manifest)?;
        self.active_runs.insert(run_id, id.clone());
        self.checkpoints.insert(id, manifest);
        self.message = None;
        self.message_is_error = false;
        Ok(())
    }

    pub(crate) fn finish_run(&mut self, run_id: u64) -> Result<Option<CheckpointSummary>, String> {
        let Some(checkpoint_id) = self.active_runs.get(&run_id).cloned() else {
            return Ok(None);
        };
        let mut persistence_started = false;
        let completion = (|| {
            let mut manifest = self
                .checkpoints
                .get(&checkpoint_id)
                .cloned()
                .ok_or_else(|| "The active Time Machine manifest is unavailable".to_owned())?;
            let (current, warnings) = self.capture_tree(&manifest.workspace_root)?;
            manifest.warnings.extend(warnings);
            deduplicate_checkpoint_warnings(&mut manifest.warnings);
            if combined_snapshot_bytes(&manifest.baseline, &current)?
                > self.limits.max_checkpoint_bytes
            {
                return Err(format!(
                    "Time Machine checkpoint exceeds its {} byte safety limit after including both file versions",
                    self.limits.max_checkpoint_bytes
                ));
            }
            manifest.changes = self.compare_snapshots(&manifest.baseline, &current)?;
            manifest.finished_at_ms = Some(unix_time_ms());
            persistence_started = true;
            self.persist_manifest(&manifest)?;
            let summary = manifest
                .summary()
                .ok_or_else(|| "The Time Machine checkpoint did not finish".to_owned())?;
            self.checkpoints.insert(checkpoint_id.clone(), manifest);
            Ok(summary)
        })();
        let summary = match completion {
            Ok(summary) => summary,
            Err(error) => {
                // The unfinished manifest and active-run association remain intact so a caller can
                // retry after a transient capture, validation, or persistence failure. Blobs from
                // the failed current-state capture are not referenced by that manifest and can be
                // reclaimed immediately before persistence begins. Once persistence has started,
                // retain them because an error may have happened after the atomic replacement.
                if !persistence_started && let Err(cleanup_error) = self.garbage_collect_blobs() {
                    self.add_maintenance_warning(format!(
                        "Time Machine could not clean temporary checkpoint content after a failed finish: {cleanup_error}"
                    ));
                }
                return Err(error);
            }
        };
        self.active_runs.remove(&run_id);
        self.message = self
            .enforce_retention()
            .and_then(|_| self.garbage_collect_blobs())
            .err()
            .map(|error| {
                format!("Checkpoint saved, but Time Machine storage cleanup was deferred: {error}")
            });
        self.message_is_error = false;
        Ok(Some(summary))
    }

    /// Computes the current line delta against an active run's immutable
    /// baseline without creating blobs, manifests, or other persistent cache
    /// entries. This is deliberately a display projection; `finish_run`
    /// remains the authoritative checkpoint capture.
    pub(crate) fn live_diff(&self, run_id: u64) -> Result<Option<LiveDiffStats>, String> {
        let Some(checkpoint_id) = self.active_runs.get(&run_id) else {
            return Ok(None);
        };
        let manifest = self
            .checkpoints
            .get(checkpoint_id)
            .ok_or_else(|| "The active Time Machine manifest is unavailable".to_owned())?;
        self.live_diff_for_manifest(manifest).map(Some)
    }

    pub(crate) fn discard_unfinished_run(&mut self, run_id: u64) -> Result<(), String> {
        let checkpoint_id = self
            .active_runs
            .get(&run_id)
            .cloned()
            .or_else(|| {
                self.checkpoints
                    .values()
                    .find(|manifest| manifest.run_id == run_id)
                    .map(|manifest| manifest.id.clone())
            })
            .ok_or_else(|| "The unfinished checkpoint is unavailable".to_owned())?;
        if self
            .checkpoints
            .get(&checkpoint_id)
            .is_some_and(|manifest| manifest.finished_at_ms.is_some())
        {
            return Err("A completed checkpoint cannot be discarded".to_owned());
        }
        self.discard_unrecoverable_manifest(&checkpoint_id)?;
        if let Err(error) = self.garbage_collect_blobs() {
            self.add_maintenance_warning(format!(
                "The unfinished checkpoint was discarded, but unused content cleanup was deferred: {error}"
            ));
        }
        self.set_message(format!(
            "Discarded the unfinished checkpoint for run {run_id}. It cannot be restored."
        ));
        Ok(())
    }

    pub(crate) fn unfinished_runs(&self) -> Vec<(u64, PathBuf)> {
        let mut runs = self
            .active_runs
            .iter()
            .filter_map(|(run_id, checkpoint_id)| {
                self.checkpoints
                    .get(checkpoint_id)
                    .filter(|manifest| manifest.finished_at_ms.is_none())
                    .map(|manifest| (*run_id, manifest.workspace_root.clone()))
            })
            .collect::<Vec<_>>();
        runs.sort_by_key(|(run_id, _)| *run_id);
        runs
    }

    pub(crate) fn take_recovered_summaries(&mut self) -> Vec<CheckpointSummary> {
        std::mem::take(&mut self.recovered_summaries)
    }

    pub(crate) fn next_run_id_hint(&self) -> u64 {
        self.checkpoints
            .values()
            .map(|manifest| manifest.run_id)
            .max()
            .map_or(1, |run_id| run_id.saturating_add(1))
    }

    pub(crate) fn checkpoint_summary(&self, checkpoint_id: &str) -> Option<CheckpointSummary> {
        self.checkpoints
            .get(checkpoint_id)
            .and_then(CheckpointManifest::summary)
    }

    pub(crate) fn open_checkpoint(
        &mut self,
        checkpoint_id: &str,
        path: Option<&str>,
    ) -> Result<(), String> {
        let manifest = self
            .checkpoints
            .get(checkpoint_id)
            .ok_or_else(|| "The selected checkpoint is unavailable".to_owned())?;
        let selected_path = match path {
            Some(path) => {
                if !manifest.changes.iter().any(|change| change.path == path) {
                    return Err("The selected file does not belong to this checkpoint".to_owned());
                }
                Some(path.to_owned())
            }
            None => manifest.changes.first().map(|change| change.path.clone()),
        };
        self.selected = Some(SelectedCheckpoint {
            checkpoint_id: checkpoint_id.to_owned(),
            selected_path,
        });
        self.message = None;
        self.message_is_error = false;
        Ok(())
    }

    pub(crate) fn select_checkpoint_file(
        &mut self,
        checkpoint_id: &str,
        path: &str,
    ) -> Result<(), String> {
        self.open_checkpoint(checkpoint_id, Some(path))
    }

    pub(crate) fn close_checkpoint(&mut self) {
        self.selected = None;
        self.message = None;
        self.message_is_error = false;
    }

    pub(crate) fn restore_checkpoint(
        &mut self,
        checkpoint_id: &str,
    ) -> Result<CheckpointSummary, String> {
        self.restore(checkpoint_id, None)
    }

    pub(crate) fn restore_checkpoint_file(
        &mut self,
        checkpoint_id: &str,
        path: &str,
    ) -> Result<CheckpointSummary, String> {
        self.restore(checkpoint_id, Some(path))
    }

    pub(crate) fn set_error(&mut self, message: impl Into<String>) {
        self.message = Some(message.into());
        self.message_is_error = true;
    }

    pub(crate) fn set_message(&mut self, message: impl Into<String>) {
        self.message = Some(message.into());
        self.message_is_error = false;
    }

    pub(crate) fn view(&self) -> TimeMachineView {
        let selected = self
            .selected
            .as_ref()
            .and_then(|selection| self.selected_view(selection).ok());
        TimeMachineView {
            available: self.available,
            selected,
            message: self
                .message
                .clone()
                .or_else(|| self.maintenance_warning.clone()),
            message_is_error: self.message.is_some() && self.message_is_error,
        }
    }

    fn prepare_store(&self) -> Result<(), String> {
        fs::create_dir_all(&self.blobs)
            .and_then(|_| fs::create_dir_all(&self.manifests))
            .and_then(|_| fs::create_dir_all(&self.quarantine))
            .and_then(|_| fs::create_dir_all(&self.restore_journals))
            .map_err(|error| {
                format!(
                    "Time Machine storage could not be prepared at {}: {error}",
                    self.root.display()
                )
            })
    }

    fn add_maintenance_warning(&mut self, warning: impl Into<String>) {
        let warning = warning.into();
        if warning.trim().is_empty() {
            return;
        }
        self.maintenance_warning = Some(match self.maintenance_warning.take() {
            Some(existing) => format!("{existing}\n{warning}"),
            None => warning,
        });
    }

    fn load_manifests(&mut self) -> Result<(), String> {
        if fs::read_dir(&self.quarantine)
            .map_err(|error| format!("Time Machine quarantine could not be listed: {error}"))?
            .next()
            .is_some()
        {
            self.manifest_index_complete = false;
            self.add_maintenance_warning(
                "Time Machine found quarantined checkpoint metadata. Blob garbage collection remains disabled until it is reviewed.",
            );
        }
        let entries = fs::read_dir(&self.manifests)
            .map_err(|error| format!("Time Machine manifests could not be listed: {error}"))?;
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    self.manifest_index_complete = false;
                    self.add_maintenance_warning(format!(
                        "A Time Machine checkpoint entry could not be read. Blob garbage collection remains disabled: {error}"
                    ));
                    continue;
                }
            };
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(error) => {
                    self.quarantine_manifest(&path, &format!("could not be read: {error}"));
                    continue;
                }
            };
            let mut manifest = match serde_json::from_slice::<CheckpointManifest>(&bytes) {
                Ok(manifest) => manifest,
                Err(error) => {
                    self.quarantine_manifest(&path, &format!("contains invalid JSON: {error}"));
                    continue;
                }
            };
            let file_id = path.file_stem().and_then(|stem| stem.to_str());
            if manifest.version != MANIFEST_VERSION
                || file_id != Some(manifest.id.as_str())
                || Uuid::parse_str(&manifest.id).is_err()
            {
                self.quarantine_manifest(&path, "has an unsupported version or invalid identity");
                continue;
            }
            if sanitize_manifest_paths(&mut manifest) {
                self.persist_manifest(&manifest)?;
            }
            let checkpoint_id = manifest.id.clone();
            self.checkpoints.insert(checkpoint_id.clone(), manifest);
            if self.checkpoints[&checkpoint_id].finished_at_ms.is_none() {
                match self.recover_interrupted_manifest(&checkpoint_id) {
                    Ok(()) => {
                        if let Some(summary) = self.checkpoints[&checkpoint_id].summary() {
                            self.recovered_summaries.push(summary);
                        }
                    }
                    Err(error) => {
                        let run_id = self.checkpoints[&checkpoint_id].run_id;
                        self.active_runs.insert(run_id, checkpoint_id.clone());
                        self.add_maintenance_warning(format!(
                            "Interrupted checkpoint {checkpoint_id} could not be recovered yet and was retained: {error}. Retry finalization after the underlying problem is fixed, or explicitly discard it if its baseline is no longer needed."
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn discard_unrecoverable_manifest(&mut self, checkpoint_id: &str) -> Result<(), String> {
        let manifest = self
            .checkpoints
            .get(checkpoint_id)
            .ok_or_else(|| "The interrupted checkpoint is unavailable".to_owned())?;
        if manifest.finished_at_ms.is_some() {
            return Err("A completed checkpoint cannot be discarded as interrupted".to_owned());
        }
        let path = self.manifest_path(checkpoint_id);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Time Machine could not remove the unfinished manifest: {error}"
                ));
            }
        }
        self.checkpoints.remove(checkpoint_id);
        self.active_runs
            .retain(|_, active_checkpoint| active_checkpoint != checkpoint_id);
        if self
            .selected
            .as_ref()
            .is_some_and(|selected| selected.checkpoint_id == checkpoint_id)
        {
            self.selected = None;
        }
        Ok(())
    }

    fn quarantine_manifest(&mut self, path: &Path, reason: &str) {
        self.manifest_index_complete = false;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("checkpoint.json");
        let destination = self
            .quarantine
            .join(format!("{name}.corrupt-{}", Uuid::new_v4()));
        let disposition = match fs::rename(path, &destination) {
            Ok(()) => format!("moved to {}", destination.display()),
            Err(error) => format!("could not be moved to quarantine: {error}"),
        };
        self.add_maintenance_warning(format!(
            "Time Machine isolated unreadable checkpoint metadata ({name}: {reason}; {disposition}). Blob garbage collection remains disabled."
        ));
    }

    fn recover_interrupted_manifest(&mut self, checkpoint_id: &str) -> Result<(), String> {
        let mut manifest = self
            .checkpoints
            .get(checkpoint_id)
            .cloned()
            .ok_or_else(|| "The interrupted checkpoint is unavailable".to_owned())?;
        if manifest.finished_at_ms.is_some() {
            return Ok(());
        }
        let (current, warnings) = self.capture_tree(&manifest.workspace_root)?;
        if combined_snapshot_bytes(&manifest.baseline, &current)? > self.limits.max_checkpoint_bytes
        {
            return Err(format!(
                "recovered content exceeds the {} byte checkpoint limit",
                self.limits.max_checkpoint_bytes
            ));
        }
        manifest.warnings.extend(warnings);
        manifest
            .warnings
            .push("Recovered after Supervisor stopped before checkpoint completion.".to_owned());
        deduplicate_checkpoint_warnings(&mut manifest.warnings);
        manifest.changes = self.compare_snapshots(&manifest.baseline, &current)?;
        manifest.finished_at_ms = Some(unix_time_ms());
        self.persist_manifest(&manifest)?;
        self.checkpoints.insert(checkpoint_id.to_owned(), manifest);
        Ok(())
    }

    fn rebuild_snapshot_cache_from_latest_checkpoint(&mut self) {
        let Some(manifest) = self
            .checkpoints
            .values()
            .filter(|manifest| manifest.finished_at_ms.is_some())
            .max_by_key(|manifest| {
                (
                    manifest.finished_at_ms.unwrap_or_default(),
                    manifest.created_at_ms,
                    manifest.run_id,
                )
            })
            .cloned()
        else {
            return;
        };

        for (relative, snapshot) in manifest_current_snapshot(&manifest) {
            if snapshot.bytes > self.limits.max_file_bytes
                || snapshot.modified_ns.is_none()
                || !is_sha256_hex(&snapshot.sha256)
            {
                continue;
            }
            let relative_path = Path::new(&relative);
            if is_snapshot_excluded(relative_path) {
                continue;
            }
            let Ok(path) = checked_target(&manifest.workspace_root, &relative) else {
                continue;
            };
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            let readonly = metadata.permissions().readonly();
            let modified_ns = metadata_modified_ns(&metadata);
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.len() != snapshot.bytes
                || modified_ns != snapshot.modified_ns
                || readonly != snapshot.readonly
                || !self.blob_path(&snapshot.sha256).is_file()
            {
                continue;
            }
            self.snapshot_cache.insert(
                path,
                CachedSnapshot {
                    bytes: snapshot.bytes,
                    modified_ns,
                    readonly,
                    snapshot,
                },
            );
        }
    }

    fn enforce_retention(&mut self) -> Result<(), String> {
        if !self.manifest_index_complete {
            return Err(
                "checkpoint retention is disabled because manifest loading was incomplete"
                    .to_owned(),
            );
        }
        let mut completed = self
            .checkpoints
            .values()
            .filter_map(|manifest| {
                manifest.finished_at_ms.map(|finished_at_ms| {
                    (
                        manifest.id.clone(),
                        finished_at_ms,
                        manifest.created_at_ms,
                        manifest.run_id,
                    )
                })
            })
            .collect::<Vec<_>>();
        completed.sort_by(|left, right| {
            right
                .1
                .cmp(&left.1)
                .then_with(|| right.2.cmp(&left.2))
                .then_with(|| right.3.cmp(&left.3))
                .then_with(|| right.0.cmp(&left.0))
        });

        for (checkpoint_id, _, _, _) in completed
            .into_iter()
            .skip(self.limits.max_retained_checkpoints)
        {
            let path = self.manifest_path(&checkpoint_id);
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "Time Machine could not remove expired checkpoint {checkpoint_id}: {error}"
                    ));
                }
            }
            self.checkpoints.remove(&checkpoint_id);
            if self
                .selected
                .as_ref()
                .is_some_and(|selected| selected.checkpoint_id == checkpoint_id)
            {
                self.selected = None;
            }
        }
        Ok(())
    }

    fn garbage_collect_blobs(&self) -> Result<(), String> {
        if !self.manifest_index_complete {
            return Err(
                "blob garbage collection is disabled because manifest loading was incomplete"
                    .to_owned(),
            );
        }
        let referenced = self
            .checkpoints
            .values()
            .flat_map(manifest_blob_hashes)
            .collect::<BTreeSet<_>>();
        let entries = fs::read_dir(&self.blobs)
            .map_err(|error| format!("Time Machine blobs could not be listed: {error}"))?;
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            // Only content-addressed files created by Time Machine are eligible. Unknown files
            // and temporary captures are deliberately left untouched.
            if !is_sha256_hex(name) || referenced.contains(name) {
                continue;
            }
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(format!(
                        "Time Machine could not inspect orphaned blob {name}: {error}"
                    ));
                }
            };
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                continue;
            }
            fs::remove_file(&path).map_err(|error| {
                format!("Time Machine could not remove orphaned blob {name}: {error}")
            })?;
        }
        Ok(())
    }

    fn capture_tree(
        &mut self,
        workspace_root: &Path,
    ) -> Result<(BTreeMap<String, SnapshotFile>, Vec<String>), String> {
        let mut files = BTreeMap::new();
        let mut seen_paths = BTreeSet::new();
        let mut warnings = Vec::new();
        let mut queue = VecDeque::from([workspace_root.to_path_buf()]);
        let store_root = fs::canonicalize(&self.root).unwrap_or_else(|_| self.root.clone());
        let mut checkpoint_bytes = 0_u64;

        while let Some(directory) = queue.pop_front() {
            let entries = match fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) => {
                    warnings.push(format!(
                        "Could not read {}: {error}",
                        relative_display(workspace_root, &directory)
                    ));
                    continue;
                }
            };
            let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
            entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_lowercase());
            for entry in entries {
                let path = entry.path();
                if path.starts_with(&store_root) {
                    continue;
                }
                let metadata = match fs::symlink_metadata(&path) {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        warnings.push(format!(
                            "Could not inspect {}: {error}",
                            relative_display(workspace_root, &path)
                        ));
                        continue;
                    }
                };
                if metadata.file_type().is_symlink() {
                    warnings.push(format!(
                        "Skipped linked path {}",
                        relative_display(workspace_root, &path)
                    ));
                    continue;
                }
                if metadata.is_dir() {
                    let relative =
                        Path::new(&relative_display(workspace_root, &path)).to_path_buf();
                    if is_sensitive_path(&relative) {
                        warnings.push(format!(
                            "Skipped sensitive directory {}",
                            relative_display(workspace_root, &path)
                        ));
                    } else if !is_generated_directory(&entry.file_name().to_string_lossy()) {
                        queue.push_back(path);
                    }
                    continue;
                }
                if !metadata.is_file() {
                    continue;
                }
                let relative = relative_display(workspace_root, &path);
                if is_sensitive_path(Path::new(&relative)) {
                    warnings.push(format!("Skipped sensitive file {relative}"));
                    continue;
                }
                if metadata.len() > self.limits.max_file_bytes {
                    warnings.push(format!(
                        "Skipped {relative}: file is {} bytes, above the Time Machine per-file limit of {} bytes",
                        metadata.len(),
                        self.limits.max_file_bytes
                    ));
                    continue;
                }
                let estimated_checkpoint_bytes = checkpoint_bytes
                    .checked_add(metadata.len())
                    .ok_or_else(|| "Time Machine checkpoint size overflowed".to_owned())?;
                if estimated_checkpoint_bytes > self.limits.max_checkpoint_bytes {
                    let _ = self.garbage_collect_blobs();
                    return Err(format!(
                        "Time Machine checkpoint exceeds its {} byte safety limit while adding {relative}",
                        self.limits.max_checkpoint_bytes
                    ));
                }
                seen_paths.insert(path.clone());
                match self.cached_snapshot_file(&path, &metadata) {
                    Ok(snapshot) => {
                        let actual_checkpoint_bytes = checkpoint_bytes
                            .checked_add(snapshot.bytes)
                            .ok_or_else(|| "Time Machine checkpoint size overflowed".to_owned())?;
                        if actual_checkpoint_bytes > self.limits.max_checkpoint_bytes {
                            let _ = self.garbage_collect_blobs();
                            return Err(format!(
                                "Time Machine checkpoint exceeds its {} byte safety limit while adding {relative}",
                                self.limits.max_checkpoint_bytes
                            ));
                        }
                        checkpoint_bytes = actual_checkpoint_bytes;
                        files.insert(relative, snapshot);
                    }
                    Err(error) => warnings.push(format!("Skipped {relative}: {error}")),
                }
            }
        }
        self.snapshot_cache
            .retain(|path, _| !path.starts_with(workspace_root) || seen_paths.contains(path));
        Ok((files, warnings))
    }

    fn live_diff_for_manifest(
        &self,
        manifest: &CheckpointManifest,
    ) -> Result<LiveDiffStats, String> {
        let workspace_root = &manifest.workspace_root;
        let store_root = fs::canonicalize(&self.root).unwrap_or_else(|_| self.root.clone());
        let mut stats = LiveDiffStats::empty(manifest.run_id, true);
        let mut queue = VecDeque::from([workspace_root.clone()]);
        let mut seen = BTreeSet::new();
        let mut unreadable_directories = Vec::new();
        let mut scanned_changed_bytes = 0_u64;

        while let Some(directory) = queue.pop_front() {
            let entries = match fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) if directory == *workspace_root => {
                    return Err(format!(
                        "Live diff could not read the workspace root: {error}"
                    ));
                }
                Err(_) => {
                    unreadable_directories.push(relative_display(workspace_root, &directory));
                    continue;
                }
            };
            let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
            entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_lowercase());
            for entry in entries {
                let path = entry.path();
                if path.starts_with(&store_root) {
                    continue;
                }
                let relative = relative_display(workspace_root, &path);
                let metadata = match fs::symlink_metadata(&path) {
                    Ok(metadata) => metadata,
                    Err(_) => {
                        // The entry existed during enumeration. Do not briefly
                        // report its baseline as deleted while it is being
                        // atomically replaced by an editor.
                        seen.insert(relative);
                        continue;
                    }
                };
                if metadata.file_type().is_symlink() {
                    seen.insert(relative);
                    continue;
                }
                if metadata.is_dir() {
                    let relative_path = Path::new(&relative);
                    if !is_sensitive_path(relative_path)
                        && !is_generated_directory(&entry.file_name().to_string_lossy())
                    {
                        queue.push_back(path);
                    }
                    continue;
                }
                if !metadata.is_file() {
                    seen.insert(relative);
                    continue;
                }
                seen.insert(relative.clone());
                if is_sensitive_path(Path::new(&relative))
                    || metadata.len() > self.limits.max_file_bytes
                {
                    continue;
                }

                let previous = manifest.baseline.get(&relative);
                if previous.is_some_and(|file| {
                    file.bytes == metadata.len()
                        && file.modified_ns == metadata_modified_ns(&metadata)
                }) {
                    continue;
                }
                scanned_changed_bytes = scanned_changed_bytes
                    .checked_add(metadata.len())
                    .ok_or_else(|| "Live diff size overflowed".to_owned())?;
                if scanned_changed_bytes > self.limits.max_checkpoint_bytes {
                    return Err(format!(
                        "Live diff exceeds its {} byte safety limit",
                        self.limits.max_checkpoint_bytes
                    ));
                }
                let current = match read_live_file(&path, &metadata, self.limits.max_file_bytes) {
                    Ok(current) => current,
                    Err(_) => continue,
                };
                if previous.is_some_and(|file| file.sha256 == current.sha256) {
                    continue;
                }
                stats.file_count = stats.file_count.saturating_add(1);
                let Some(after_text) = current.text.as_deref() else {
                    continue;
                };
                let before_text = match previous {
                    None => String::new(),
                    Some(file) if file.text => match self.read_text_blob(Some(file)) {
                        Ok(text) => text,
                        Err(_) => continue,
                    },
                    Some(_) => continue,
                };
                let (additions, deletions) = text_diff_stats(&before_text, after_text);
                stats.additions = stats.additions.saturating_add(additions);
                stats.deletions = stats.deletions.saturating_add(deletions);
            }
        }

        for (relative, previous) in &manifest.baseline {
            if seen.contains(relative)
                || unreadable_directories
                    .iter()
                    .any(|directory| relative_is_within(relative, directory))
            {
                continue;
            }
            stats.file_count = stats.file_count.saturating_add(1);
            if !previous.text {
                continue;
            }
            let Ok(before_text) = self.read_text_blob(Some(previous)) else {
                continue;
            };
            let (additions, deletions) = text_diff_stats(&before_text, "");
            stats.additions = stats.additions.saturating_add(additions);
            stats.deletions = stats.deletions.saturating_add(deletions);
        }
        Ok(stats)
    }

    fn cached_snapshot_file(
        &mut self,
        path: &Path,
        metadata: &fs::Metadata,
    ) -> Result<SnapshotFile, String> {
        let bytes = metadata.len();
        let modified_ns = metadata_modified_ns(metadata);
        let readonly = metadata.permissions().readonly();
        if let Some(cached) = self.snapshot_cache.get(path)
            && cached.bytes == bytes
            && cached.modified_ns == modified_ns
            && cached.readonly == readonly
            && self.blob_path(&cached.snapshot.sha256).is_file()
            && verified_file_hash(path, metadata, self.limits.max_file_bytes)?
                == cached.snapshot.sha256
        {
            return Ok(cached.snapshot.clone());
        }

        let snapshot = self.snapshot_file(path, metadata)?;
        self.snapshot_cache.insert(
            path.to_path_buf(),
            CachedSnapshot {
                bytes,
                modified_ns,
                readonly,
                snapshot: snapshot.clone(),
            },
        );
        Ok(snapshot)
    }

    fn snapshot_file(&self, path: &Path, metadata: &fs::Metadata) -> Result<SnapshotFile, String> {
        let expected_bytes = metadata.len();
        let modified_ns = metadata_modified_ns(metadata);
        let readonly = metadata.permissions().readonly();
        let mut source =
            File::open(path).map_err(|error| format!("the file could not be opened: {error}"))?;
        let temp_path = self.blobs.join(format!(".capture-{}.tmp", Uuid::new_v4()));
        let mut destination = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| format!("the checkpoint blob could not be created: {error}"))?;
        let mut hasher = Sha256::new();
        let mut sample = Vec::with_capacity(TEXT_SAMPLE_BYTES);
        let mut bytes = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        let copy_result = (|| -> Result<(), String> {
            loop {
                let read = source
                    .read(&mut buffer)
                    .map_err(|error| format!("the file could not be read: {error}"))?;
                if read == 0 {
                    break;
                }
                if bytes.saturating_add(read as u64) > self.limits.max_file_bytes {
                    return Err(format!(
                        "file grew beyond the Time Machine per-file limit of {} bytes while being captured",
                        self.limits.max_file_bytes
                    ));
                }
                if sample.len() < TEXT_SAMPLE_BYTES {
                    let sample_bytes = (TEXT_SAMPLE_BYTES - sample.len()).min(read);
                    sample.extend_from_slice(&buffer[..sample_bytes]);
                }
                hasher.update(&buffer[..read]);
                destination.write_all(&buffer[..read]).map_err(|error| {
                    format!("the checkpoint blob could not be written: {error}")
                })?;
                bytes = bytes.saturating_add(read as u64);
            }
            destination
                .sync_all()
                .map_err(|error| format!("the checkpoint blob could not be finalized: {error}"))?;
            Ok(())
        })();
        if let Err(error) = copy_result {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }
        let finalized_metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) => {
                let _ = fs::remove_file(&temp_path);
                return Err(format!(
                    "the file could not be verified after capture: {error}"
                ));
            }
        };
        if !finalized_metadata.is_file()
            || finalized_metadata.file_type().is_symlink()
            || bytes != expected_bytes
            || finalized_metadata.len() != expected_bytes
            || metadata_modified_ns(&finalized_metadata) != modified_ns
            || finalized_metadata.permissions().readonly() != readonly
        {
            let _ = fs::remove_file(&temp_path);
            return Err("the file changed while its checkpoint was being captured".to_owned());
        }
        let sha256 = format!("{:x}", hasher.finalize());
        let blob_path = self.blob_path(&sha256);
        if blob_path.exists() {
            let _ = fs::remove_file(&temp_path);
        } else if let Err(error) = fs::rename(&temp_path, &blob_path) {
            let _ = fs::remove_file(&temp_path);
            return Err(format!("the checkpoint blob could not be stored: {error}"));
        }
        let text = !sample.contains(&0) && std::str::from_utf8(&sample).is_ok();
        Ok(SnapshotFile {
            sha256,
            bytes,
            text,
            readonly,
            modified_ns,
        })
    }

    fn compare_snapshots(
        &self,
        before: &BTreeMap<String, SnapshotFile>,
        after: &BTreeMap<String, SnapshotFile>,
    ) -> Result<Vec<ChangedFile>, String> {
        let paths = before
            .keys()
            .chain(after.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut changes = Vec::new();
        for path in paths {
            let previous = before.get(&path).cloned();
            let current = after.get(&path).cloned();
            if previous.as_ref().map(|file| &file.sha256)
                == current.as_ref().map(|file| &file.sha256)
            {
                continue;
            }
            let status = match (&previous, &current) {
                (None, Some(_)) => ChangedFileStatus::Added,
                (Some(_), None) => ChangedFileStatus::Deleted,
                (Some(_), Some(_)) => ChangedFileStatus::Modified,
                (None, None) => continue,
            };
            let binary = previous.as_ref().is_some_and(|file| !file.text)
                || current.as_ref().is_some_and(|file| !file.text);
            let (additions, deletions) = if binary {
                (0, 0)
            } else {
                self.diff_stats(previous.as_ref(), current.as_ref())?
            };
            changes.push(ChangedFile {
                path: path.clone(),
                status,
                icon_key: file_icon_key(&path).to_owned(),
                additions,
                deletions,
                binary,
                restored: false,
                before: previous,
                after: current,
            });
        }
        Ok(changes)
    }

    fn diff_stats(
        &self,
        before: Option<&SnapshotFile>,
        after: Option<&SnapshotFile>,
    ) -> Result<(usize, usize), String> {
        let before_text = self.read_text_blob(before)?;
        let after_text = self.read_text_blob(after)?;
        Ok(text_diff_stats(&before_text, &after_text))
    }

    fn selected_view(
        &self,
        selection: &SelectedCheckpoint,
    ) -> Result<SelectedCheckpointView, String> {
        let manifest = self
            .checkpoints
            .get(&selection.checkpoint_id)
            .ok_or_else(|| "The selected checkpoint is unavailable".to_owned())?;
        let checkpoint = manifest
            .summary()
            .ok_or_else(|| "The selected checkpoint is incomplete".to_owned())?;
        let Some(path) = selection.selected_path.as_deref() else {
            return Ok(SelectedCheckpointView {
                checkpoint,
                selected_path: None,
                diff: None,
                binary: false,
            });
        };
        let change = manifest
            .changes
            .iter()
            .find(|change| change.path == path)
            .ok_or_else(|| "The selected checkpoint file is unavailable".to_owned())?;
        let diff = if change.binary {
            Some(format!(
                "Binary file changed\n\nBefore: {}\nAfter: {}",
                change
                    .before
                    .as_ref()
                    .map(|file| format!("{} bytes", file.bytes))
                    .unwrap_or_else(|| "file did not exist".to_owned()),
                change
                    .after
                    .as_ref()
                    .map(|file| format!("{} bytes", file.bytes))
                    .unwrap_or_else(|| "file was deleted".to_owned())
            ))
        } else {
            let before = self.read_text_blob(change.before.as_ref())?;
            let after = self.read_text_blob(change.after.as_ref())?;
            Some(
                TextDiff::from_lines(&before, &after)
                    .unified_diff()
                    .context_radius(3)
                    .header(&format!("a/{}", change.path), &format!("b/{}", change.path))
                    .to_string(),
            )
        };
        Ok(SelectedCheckpointView {
            checkpoint,
            selected_path: Some(path.to_owned()),
            diff,
            binary: change.binary,
        })
    }

    fn restore(
        &mut self,
        checkpoint_id: &str,
        selected_path: Option<&str>,
    ) -> Result<CheckpointSummary, String> {
        let manifest = self
            .checkpoints
            .get(checkpoint_id)
            .cloned()
            .ok_or_else(|| "The selected checkpoint is unavailable".to_owned())?;
        if manifest.finished_at_ms.is_none() {
            return Err("The checkpoint is still being captured".to_owned());
        }
        let changes = match selected_path {
            Some(path) => vec![
                manifest
                    .changes
                    .iter()
                    .find(|change| change.path == path)
                    .cloned()
                    .ok_or_else(|| {
                        "The selected file does not belong to this checkpoint".to_owned()
                    })?,
            ],
            None => manifest.changes.clone(),
        };
        let operations = changes
            .iter()
            .map(|change| self.prepare_restore_operation(&manifest.workspace_root, change))
            .collect::<Result<Vec<_>, _>>()?;
        self.execute_restore_transaction(&manifest.workspace_root, &operations)?;

        let stored = self
            .checkpoints
            .get_mut(checkpoint_id)
            .ok_or_else(|| "The selected checkpoint disappeared during restore".to_owned())?;
        for operation in &operations {
            if let Some(change) = stored
                .changes
                .iter_mut()
                .find(|change| change.path == operation.path)
            {
                change.restored = true;
            }
        }
        let stored = stored.clone();
        self.persist_manifest(&stored)?;
        let summary = stored
            .summary()
            .ok_or_else(|| "The restored checkpoint summary is unavailable".to_owned())?;
        self.message = Some(if let Some(path) = selected_path {
            format!("Restored {path} from the checkpoint.")
        } else {
            format!(
                "Restored {} changed file(s) from the checkpoint.",
                changes.len()
            )
        });
        self.message_is_error = false;
        Ok(summary)
    }

    fn prepare_restore_operation(
        &self,
        workspace_root: &Path,
        change: &ChangedFile,
    ) -> Result<RestoreOperation, String> {
        let target = checked_target(workspace_root, &change.path)?;
        let current = current_file_hash(&target)?;
        let before_hash = change.before.as_ref().map(|file| file.sha256.as_str());
        let after_hash = change.after.as_ref().map(|file| file.sha256.as_str());
        let action = match (before_hash, after_hash, current.as_deref()) {
            (None, Some(after), Some(current)) if current == after => RestoreAction::Remove,
            (None, Some(_), None) => RestoreAction::None,
            (Some(before), None, None) => RestoreAction::Write {
                blob_hash: before.to_owned(),
                readonly: change.before.as_ref().is_some_and(|file| file.readonly),
            },
            (Some(before), None, Some(current)) if current == before => RestoreAction::None,
            (Some(before), Some(after), Some(current)) if current == after => {
                RestoreAction::Write {
                    blob_hash: before.to_owned(),
                    readonly: change.before.as_ref().is_some_and(|file| file.readonly),
                }
            }
            (Some(before), Some(_), Some(current)) if current == before => RestoreAction::None,
            _ => {
                return Err(format!(
                    "Restore conflict in {}. The file changed manually after the agent run; no checkpoint files were overwritten.",
                    change.path
                ));
            }
        };
        Ok(RestoreOperation {
            path: change.path.clone(),
            target,
            expected_current: current,
            action,
        })
    }

    fn execute_restore_transaction(
        &mut self,
        workspace_root: &Path,
        operations: &[RestoreOperation],
    ) -> Result<(), String> {
        self.recover_restore_journals()?;
        let mut journal = self.prepare_restore_journal(workspace_root, operations)?;
        if journal.entries.is_empty() {
            return Ok(());
        }
        self.persist_restore_journal(&journal)?;
        for index in 0..journal.entries.len() {
            #[cfg(test)]
            if self
                .restore_fail_after_applied
                .is_some_and(|limit| index >= limit)
            {
                let error = "injected restore transaction failure".to_owned();
                return self.rollback_after_restore_error(&journal, error);
            }
            if let Err(error) = self.apply_restore_journal_entry(&journal, index) {
                return self.rollback_after_restore_error(&journal, error);
            }
            journal.entries[index].applied = true;
            if let Err(error) = self.persist_restore_journal(&journal) {
                return self.rollback_after_restore_error(&journal, error);
            }
        }
        journal.committed = true;
        self.persist_restore_journal(&journal)?;
        self.finalize_restore_journal(&journal)?;
        fs::remove_file(self.restore_journal_path(&journal.id)).map_err(|error| {
            format!("Restore completed, but its journal could not be removed: {error}")
        })?;
        Ok(())
    }

    fn prepare_restore_journal(
        &self,
        workspace_root: &Path,
        operations: &[RestoreOperation],
    ) -> Result<RestoreJournal, String> {
        let journal_id = Uuid::new_v4().to_string();
        let mut entries = Vec::new();
        let result = (|| -> Result<(), String> {
            for operation in operations {
                let fresh_target = checked_target(workspace_root, &operation.path)?;
                if fresh_target != operation.target
                    || current_file_hash(&fresh_target)? != operation.expected_current
                {
                    return Err(format!(
                        "Restore conflict in {}. The file changed while restore was being prepared.",
                        operation.path
                    ));
                }
                let (action, blob_hash, replacement_readonly) = match &operation.action {
                    RestoreAction::None => continue,
                    RestoreAction::Remove => (RestoreJournalAction::Remove, None, false),
                    RestoreAction::Write {
                        blob_hash,
                        readonly,
                    } => (
                        RestoreJournalAction::Write,
                        Some(blob_hash.as_str()),
                        *readonly,
                    ),
                };
                let parent = fresh_target.parent().ok_or_else(|| {
                    format!("Restore target {} has no parent folder", operation.path)
                })?;
                fs::create_dir_all(parent).map_err(|error| {
                    format!(
                        "Could not prepare the folder for {}: {error}",
                        operation.path
                    )
                })?;
                let target_existed = fresh_target.is_file();
                let original_readonly = target_existed
                    && fs::metadata(&fresh_target)
                        .map(|metadata| metadata.permissions().readonly())
                        .unwrap_or(false);
                let backup = target_existed.then(|| {
                    parent.join(format!(".central-agent-restore-backup-{}", Uuid::new_v4()))
                });
                let staged = if let Some(blob_hash) = blob_hash {
                    let staged =
                        parent.join(format!(".central-agent-restore-stage-{}", Uuid::new_v4()));
                    self.stage_restore_blob(blob_hash, &staged, replacement_readonly)?;
                    Some(staged)
                } else {
                    None
                };
                entries.push(RestoreJournalEntry {
                    relative_path: operation.path.clone(),
                    target: fresh_target,
                    staged,
                    backup,
                    target_existed,
                    original_readonly,
                    replacement_readonly,
                    expected_current: operation.expected_current.clone(),
                    action,
                    applied: false,
                });
            }
            Ok(())
        })();
        if let Err(error) = result {
            for entry in &entries {
                if let Some(staged) = &entry.staged {
                    let _ = fs::remove_file(staged);
                }
            }
            return Err(error);
        }
        Ok(RestoreJournal {
            version: RESTORE_JOURNAL_VERSION,
            id: journal_id,
            workspace_root: workspace_root.to_path_buf(),
            committed: false,
            entries,
        })
    }

    fn stage_restore_blob(
        &self,
        blob_hash: &str,
        staged: &Path,
        readonly: bool,
    ) -> Result<(), String> {
        let result = (|| -> Result<(), String> {
            let mut source = File::open(self.blob_path(blob_hash))
                .map_err(|error| format!("Restore blob could not be opened: {error}"))?;
            let mut destination = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(staged)
                .map_err(|error| format!("Restore staging file could not be created: {error}"))?;
            let mut hasher = Sha256::new();
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                let read = source
                    .read(&mut buffer)
                    .map_err(|error| format!("Restore blob could not be read: {error}"))?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
                destination.write_all(&buffer[..read]).map_err(|error| {
                    format!("Restore staging file could not be written: {error}")
                })?;
            }
            destination.sync_all().map_err(|error| {
                format!("Restore staging file could not be synchronized: {error}")
            })?;
            drop(destination);
            if format!("{:x}", hasher.finalize()) != blob_hash {
                return Err("Restore blob failed its SHA-256 integrity check".to_owned());
            }
            set_file_readonly(staged, readonly)
                .map_err(|error| format!("Restore staging permissions could not be set: {error}"))
        })();
        if result.is_err() {
            let _ = fs::remove_file(staged);
        }
        result
    }

    fn apply_restore_journal_entry(
        &self,
        journal: &RestoreJournal,
        index: usize,
    ) -> Result<(), String> {
        let entry = &journal.entries[index];
        validate_restore_journal_entry(journal, entry)?;
        if current_file_hash(&entry.target)? != entry.expected_current {
            return Err(format!(
                "Restore conflict in {}. The file changed during transaction staging.",
                entry.relative_path
            ));
        }
        if entry.original_readonly && entry.target.exists() {
            set_file_readonly(&entry.target, false).map_err(|error| {
                format!(
                    "Could not unlock {} for restore: {error}",
                    entry.relative_path
                )
            })?;
        }
        let result = match entry.action {
            RestoreJournalAction::Remove => fs::rename(
                &entry.target,
                entry
                    .backup
                    .as_ref()
                    .ok_or_else(|| "Restore removal backup is missing".to_owned())?,
            )
            .map_err(|error| {
                format!(
                    "Could not stage removal of {}: {error}",
                    entry.relative_path
                )
            }),
            RestoreJournalAction::Write => atomic_replace_file_with_backup(
                entry
                    .staged
                    .as_ref()
                    .ok_or_else(|| "Restore replacement staging file is missing".to_owned())?,
                &entry.target,
                entry.backup.as_deref(),
            )
            .map_err(|error| {
                format!(
                    "Could not replace {} atomically: {error}",
                    entry.relative_path
                )
            }),
        };
        if result.is_err() && entry.original_readonly && entry.target.exists() {
            let _ = set_file_readonly(&entry.target, true);
        }
        result?;
        sync_parent_directory(&entry.target)
            .map_err(|error| format!("Restore folder could not be synchronized: {error}"))
    }

    fn rollback_after_restore_error<T>(
        &self,
        journal: &RestoreJournal,
        error: String,
    ) -> Result<T, String> {
        match self.rollback_restore_journal(journal) {
            Ok(()) => {
                let _ = fs::remove_file(self.restore_journal_path(&journal.id));
                Err(format!(
                    "{error}. All restore changes were rolled back; retry is safe."
                ))
            }
            Err(rollback_error) => Err(format!(
                "{error}. Automatic rollback is pending and will resume on next startup: {rollback_error}"
            )),
        }
    }

    fn rollback_restore_journal(&self, journal: &RestoreJournal) -> Result<(), String> {
        validate_restore_journal(journal)?;
        for entry in journal.entries.iter().rev() {
            if let Some(backup) = &entry.backup
                && backup.is_file()
            {
                if entry.target.exists() {
                    let _ = set_file_readonly(&entry.target, false);
                    fs::remove_file(&entry.target).map_err(|error| {
                        format!(
                            "Could not remove partial restore {}: {error}",
                            entry.relative_path
                        )
                    })?;
                }
                fs::rename(backup, &entry.target).map_err(|error| {
                    format!("Could not roll back {}: {error}", entry.relative_path)
                })?;
                set_file_readonly(&entry.target, entry.original_readonly).map_err(|error| {
                    format!(
                        "Could not restore permissions for {}: {error}",
                        entry.relative_path
                    )
                })?;
            } else if !entry.target_existed
                && entry.staged.as_ref().is_some_and(|staged| !staged.exists())
                && entry.target.exists()
            {
                let _ = set_file_readonly(&entry.target, false);
                fs::remove_file(&entry.target).map_err(|error| {
                    format!(
                        "Could not remove newly restored {}: {error}",
                        entry.relative_path
                    )
                })?;
            }
            if let Some(staged) = &entry.staged
                && staged.exists()
            {
                fs::remove_file(staged).map_err(|error| {
                    format!(
                        "Could not remove staging for {}: {error}",
                        entry.relative_path
                    )
                })?;
            }
            sync_parent_directory(&entry.target)
                .map_err(|error| format!("Rollback folder could not be synchronized: {error}"))?;
        }
        Ok(())
    }

    fn finalize_restore_journal(&self, journal: &RestoreJournal) -> Result<(), String> {
        validate_restore_journal(journal)?;
        for entry in &journal.entries {
            for auxiliary in [&entry.staged, &entry.backup].into_iter().flatten() {
                if auxiliary.exists() {
                    fs::remove_file(auxiliary).map_err(|error| {
                        format!(
                            "Committed restore cleanup could not remove {}: {error}",
                            auxiliary.display()
                        )
                    })?;
                }
            }
        }
        Ok(())
    }

    fn restore_journal_path(&self, journal_id: &str) -> PathBuf {
        self.restore_journals.join(format!("{journal_id}.json"))
    }

    fn persist_restore_journal(&self, journal: &RestoreJournal) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(journal)
            .map_err(|error| format!("Restore journal could not be serialized: {error}"))?;
        let destination = self.restore_journal_path(&journal.id);
        let temporary = self
            .restore_journals
            .join(format!(".restore-journal-{}.tmp", Uuid::new_v4()));
        let result = (|| -> Result<(), String> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
                .map_err(|error| format!("Restore journal could not be created: {error}"))?;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|error| format!("Restore journal could not be synchronized: {error}"))?;
            drop(file);
            atomic_replace_file(&temporary, &destination)
                .map_err(|error| format!("Restore journal could not be committed: {error}"))?;
            sync_parent_directory(&destination).map_err(|error| {
                format!("Restore journal folder could not be synchronized: {error}")
            })
        })();
        if result.is_err() {
            let _ = fs::remove_file(temporary);
        }
        result
    }

    fn recover_restore_journals(&mut self) -> Result<(), String> {
        let entries = fs::read_dir(&self.restore_journals)
            .map_err(|error| format!("Restore journals could not be listed: {error}"))?;
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let journal =
                match fs::read(&path)
                    .map_err(|error| error.to_string())
                    .and_then(|bytes| {
                        serde_json::from_slice::<RestoreJournal>(&bytes)
                            .map_err(|error| error.to_string())
                    }) {
                    Ok(journal) => journal,
                    Err(error) => {
                        return Err(format!(
                            "An unreadable restore journal was preserved at {}: {error}",
                            path.display()
                        ));
                    }
                };
            if path.file_stem().and_then(|name| name.to_str()) != Some(journal.id.as_str()) {
                return Err(format!(
                    "A restore journal with an invalid file identity was preserved at {}",
                    path.display()
                ));
            }
            if let Err(error) = validate_restore_journal(&journal) {
                return Err(format!(
                    "An invalid restore journal was preserved at {}: {error}",
                    path.display()
                ));
            }
            if journal.committed {
                self.finalize_restore_journal(&journal)?;
            } else {
                self.rollback_restore_journal(&journal)?;
            }
            fs::remove_file(&path).map_err(|error| {
                format!("Recovered restore journal could not be removed: {error}")
            })?;
        }
        Ok(())
    }

    fn read_text_blob(&self, file: Option<&SnapshotFile>) -> Result<String, String> {
        let Some(file) = file else {
            return Ok(String::new());
        };
        let bytes = fs::read(self.blob_path(&file.sha256))
            .map_err(|error| format!("A Time Machine blob could not be read: {error}"))?;
        String::from_utf8(bytes)
            .map_err(|_| "A file marked as text contains unsupported binary data".to_owned())
    }

    fn blob_path(&self, sha256: &str) -> PathBuf {
        self.blobs.join(sha256)
    }

    fn manifest_path(&self, checkpoint_id: &str) -> PathBuf {
        self.manifests.join(format!("{checkpoint_id}.json"))
    }

    fn persist_manifest(&self, manifest: &CheckpointManifest) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(manifest)
            .map_err(|error| format!("The checkpoint manifest could not be serialized: {error}"))?;
        let destination = self.manifest_path(&manifest.id);
        let temp_path = self
            .manifests
            .join(format!(".manifest-{}.tmp", Uuid::new_v4()));
        let write_result = (|| -> Result<(), String> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)
                .map_err(|error| {
                    format!("The checkpoint manifest temporary file could not be created: {error}")
                })?;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|error| {
                    format!("The checkpoint manifest could not be finalized: {error}")
                })?;
            drop(file);
            atomic_replace_file(&temp_path, &destination)
                .map_err(|error| format!("The checkpoint manifest could not be saved: {error}"))?;
            sync_parent_directory(&destination).map_err(|error| {
                format!("The checkpoint manifest directory could not be synchronized: {error}")
            })
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(temp_path);
        }
        write_result
    }
}

fn manifest_blob_hashes(manifest: &CheckpointManifest) -> BTreeSet<String> {
    let mut hashes = manifest
        .baseline
        .values()
        .map(|file| file.sha256.clone())
        .collect::<BTreeSet<_>>();
    for change in &manifest.changes {
        if let Some(file) = &change.before {
            hashes.insert(file.sha256.clone());
        }
        if let Some(file) = &change.after {
            hashes.insert(file.sha256.clone());
        }
    }
    hashes
}

struct LiveFile {
    sha256: String,
    text: Option<String>,
}

fn read_live_file(
    path: &Path,
    expected: &fs::Metadata,
    max_bytes: u64,
) -> Result<LiveFile, String> {
    if expected.len() > max_bytes {
        return Err(format!("file exceeds the {max_bytes} byte live-diff limit"));
    }
    let expected_modified_ns = metadata_modified_ns(expected);
    let bytes = fs::read(path).map_err(|error| format!("file could not be read: {error}"))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!(
            "file grew beyond the {max_bytes} byte live-diff limit"
        ));
    }
    let final_metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("file metadata could not be verified: {error}"))?;
    if !final_metadata.is_file()
        || final_metadata.file_type().is_symlink()
        || final_metadata.len() != expected.len()
        || bytes.len() as u64 != expected.len()
        || metadata_modified_ns(&final_metadata) != expected_modified_ns
    {
        return Err("file changed while its live diff was being read".to_owned());
    }
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    let text = if bytes[..bytes.len().min(TEXT_SAMPLE_BYTES)].contains(&0) {
        None
    } else {
        String::from_utf8(bytes).ok()
    };
    Ok(LiveFile { sha256, text })
}

fn text_diff_stats(before: &str, after: &str) -> (usize, usize) {
    let mut additions = 0_usize;
    let mut deletions = 0_usize;
    for change in TextDiff::from_lines(before, after).iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => additions = additions.saturating_add(1),
            ChangeTag::Delete => deletions = deletions.saturating_add(1),
            ChangeTag::Equal => {}
        }
    }
    (additions, deletions)
}

fn relative_is_within(relative: &str, directory: &str) -> bool {
    directory.is_empty()
        || relative == directory
        || relative
            .strip_prefix(directory)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn sanitize_manifest_paths(manifest: &mut CheckpointManifest) -> bool {
    let baseline_count = manifest.baseline.len();
    let change_count = manifest.changes.len();
    manifest
        .baseline
        .retain(|path, _| !is_snapshot_excluded(Path::new(path)));
    manifest
        .changes
        .retain(|change| !is_snapshot_excluded(Path::new(&change.path)));
    manifest.baseline.len() != baseline_count || manifest.changes.len() != change_count
}

fn manifest_current_snapshot(manifest: &CheckpointManifest) -> BTreeMap<String, SnapshotFile> {
    let mut current = manifest.baseline.clone();
    for change in &manifest.changes {
        let expected = if change.restored {
            change.before.as_ref()
        } else {
            change.after.as_ref()
        };
        if let Some(file) = expected {
            current.insert(change.path.clone(), file.clone());
        } else {
            current.remove(&change.path);
        }
    }
    current
}

fn metadata_modified_ns(metadata: &fs::Metadata) -> Option<u128> {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
}

fn is_snapshot_excluded(path: &Path) -> bool {
    is_sensitive_path(path)
        || path.parent().is_some_and(|parent| {
            parent.components().any(|component| {
                matches!(component, Component::Normal(name) if is_generated_directory(&name.to_string_lossy()))
            })
        })
}

fn combined_snapshot_bytes(
    before: &BTreeMap<String, SnapshotFile>,
    after: &BTreeMap<String, SnapshotFile>,
) -> Result<u64, String> {
    let mut unique = BTreeMap::<&str, u64>::new();
    for file in before.values().chain(after.values()) {
        unique.entry(&file.sha256).or_insert(file.bytes);
    }
    unique.values().try_fold(0_u64, |total, bytes| {
        total
            .checked_add(*bytes)
            .ok_or_else(|| "Time Machine checkpoint size overflowed".to_owned())
    })
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(windows)]
fn atomic_replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    atomic_replace_file_with_backup(source, destination, None)
}

#[cfg(windows)]
fn atomic_replace_file_with_backup(
    source: &Path,
    destination: &Path,
    backup: Option<&Path>,
) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    use windows::{
        Win32::Storage::FileSystem::{REPLACEFILE_WRITE_THROUGH, ReplaceFileW},
        core::PCWSTR,
    };

    if !destination.exists() {
        return fs::rename(source, destination);
    }
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
    let backup = backup.map(|backup| {
        backup
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>()
    });
    unsafe {
        ReplaceFileW(
            PCWSTR(destination.as_ptr()),
            PCWSTR(source.as_ptr()),
            backup
                .as_ref()
                .map_or(PCWSTR::null(), |backup| PCWSTR(backup.as_ptr())),
            REPLACEFILE_WRITE_THROUGH,
            None,
            None,
        )
    }
    .map_err(std::io::Error::other)
}

#[cfg(not(windows))]
fn atomic_replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    atomic_replace_file_with_backup(source, destination, None)
}

#[cfg(not(windows))]
fn atomic_replace_file_with_backup(
    source: &Path,
    destination: &Path,
    backup: Option<&Path>,
) -> std::io::Result<()> {
    if let Some(backup) = backup {
        if let Err(link_error) = fs::hard_link(destination, backup) {
            fs::copy(destination, backup).map_err(|copy_error| {
                std::io::Error::new(
                    copy_error.kind(),
                    format!(
                        "backup hard-link failed ({link_error}); backup copy failed ({copy_error})"
                    ),
                )
            })?;
            File::open(backup)?.sync_all()?;
        }
    }
    fs::rename(source, destination)
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[derive(Clone, Debug)]
struct RestoreOperation {
    path: String,
    target: PathBuf,
    expected_current: Option<String>,
    action: RestoreAction,
}

#[derive(Clone, Debug)]
enum RestoreAction {
    None,
    Remove,
    Write { blob_hash: String, readonly: bool },
}

const RESTORE_JOURNAL_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum RestoreJournalAction {
    Remove,
    Write,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RestoreJournalEntry {
    relative_path: String,
    target: PathBuf,
    staged: Option<PathBuf>,
    backup: Option<PathBuf>,
    target_existed: bool,
    original_readonly: bool,
    replacement_readonly: bool,
    expected_current: Option<String>,
    action: RestoreJournalAction,
    applied: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct RestoreJournal {
    version: u32,
    id: String,
    workspace_root: PathBuf,
    committed: bool,
    entries: Vec<RestoreJournalEntry>,
}

fn validate_restore_journal(journal: &RestoreJournal) -> Result<(), String> {
    if journal.version != RESTORE_JOURNAL_VERSION || Uuid::parse_str(&journal.id).is_err() {
        return Err("restore journal has an unsupported version or invalid identity".to_owned());
    }
    for entry in &journal.entries {
        validate_restore_journal_entry(journal, entry)?;
    }
    Ok(())
}

fn validate_restore_journal_entry(
    journal: &RestoreJournal,
    entry: &RestoreJournalEntry,
) -> Result<(), String> {
    let checked = checked_target(&journal.workspace_root, &entry.relative_path)?;
    if checked != entry.target {
        return Err("restore journal target does not match its workspace path".to_owned());
    }
    if entry.target_existed != entry.backup.is_some()
        || (matches!(entry.action, RestoreJournalAction::Remove) && !entry.target_existed)
        || (matches!(entry.action, RestoreJournalAction::Write) && entry.staged.is_none())
    {
        return Err("restore journal operation metadata is inconsistent".to_owned());
    }
    if let Some(staged) = &entry.staged {
        validate_restore_auxiliary(&entry.target, staged, ".central-agent-restore-stage-")?;
    }
    if let Some(backup) = &entry.backup {
        validate_restore_auxiliary(&entry.target, backup, ".central-agent-restore-backup-")?;
    }
    Ok(())
}

fn validate_restore_auxiliary(target: &Path, path: &Path, prefix: &str) -> Result<(), String> {
    if path.parent() != target.parent() {
        return Err("restore auxiliary file is outside the target filesystem".to_owned());
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "restore auxiliary file name is invalid".to_owned())?;
    let id = name
        .strip_prefix(prefix)
        .ok_or_else(|| "restore auxiliary file name is invalid".to_owned())?;
    Uuid::parse_str(id)
        .map(|_| ())
        .map_err(|_| "restore auxiliary file identity is invalid".to_owned())
}

fn verified_file_hash(
    path: &Path,
    expected: &fs::Metadata,
    max_bytes: u64,
) -> Result<String, String> {
    let expected_bytes = expected.len();
    let expected_modified_ns = metadata_modified_ns(expected);
    let expected_readonly = expected.permissions().readonly();
    if expected_bytes > max_bytes {
        return Err(format!(
            "file exceeds the {max_bytes} byte verification limit"
        ));
    }
    let mut file = File::open(path)
        .map_err(|error| format!("cached file could not be reopened for verification: {error}"))?;
    let mut hasher = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("cached file could not be verified: {error}"))?;
        if read == 0 {
            break;
        }
        bytes = bytes.saturating_add(read as u64);
        if bytes > max_bytes {
            return Err(format!(
                "file grew beyond the {max_bytes} byte verification limit"
            ));
        }
        hasher.update(&buffer[..read]);
    }
    let final_metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cached file metadata could not be verified: {error}"))?;
    if !final_metadata.is_file()
        || final_metadata.file_type().is_symlink()
        || bytes != expected_bytes
        || final_metadata.len() != expected_bytes
        || metadata_modified_ns(&final_metadata) != expected_modified_ns
        || final_metadata.permissions().readonly() != expected_readonly
    {
        return Err("file changed while its cached fingerprint was being verified".to_owned());
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn current_file_hash(path: &Path) -> Result<Option<String>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err("Restore refused to follow a symbolic link".to_owned())
        }
        Ok(metadata) if !metadata.is_file() => {
            Err("Restore target is no longer a regular file".to_owned())
        }
        Ok(_) => {
            let mut file = File::open(path)
                .map_err(|error| format!("Restore target could not be read: {error}"))?;
            let mut hasher = Sha256::new();
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                let read = file
                    .read(&mut buffer)
                    .map_err(|error| format!("Restore target could not be hashed: {error}"))?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
            }
            Ok(Some(format!("{:x}", hasher.finalize())))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("Restore target could not be inspected: {error}")),
    }
}

fn set_file_readonly(path: &Path, readonly: bool) -> std::io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = permissions.mode();
        permissions.set_mode(if readonly {
            mode & !0o222
        } else {
            mode | 0o200
        });
    }
    #[cfg(not(unix))]
    permissions.set_readonly(readonly);
    fs::set_permissions(path, permissions)
}

fn checked_target(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative = Path::new(relative);
    if relative.is_absolute() {
        return Err("Checkpoint paths must remain relative to the workspace".to_owned());
    }
    let mut target = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err("Checkpoint path traversal was rejected".to_owned());
        };
        let component_text = component.to_string_lossy();
        if component_text.contains(':') || component_text.contains('\0') {
            return Err("Checkpoint path contains an unsafe component".to_owned());
        }
        target.push(component);
        match fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("Checkpoint restore refused to follow a symbolic link".to_owned());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!("Checkpoint path could not be inspected: {error}"));
            }
        }
    }
    Ok(target)
}

fn is_generated_directory(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    matches!(
        name.as_str(),
        ".git"
            | ".hg"
            | ".svn"
            | "node_modules"
            | "target"
            | "build"
            | "dist"
            | ".next"
            | ".nuxt"
            | ".cache"
            | ".parcel-cache"
            | ".turbo"
            | ".venv"
            | "venv"
            | "__pycache__"
            | "coverage"
            | "vendor"
            | "obj"
            | "outputs"
            | ".gradle"
            | ".idea"
            | ".vs"
            | "out"
            | ".tox"
            | ".mypy_cache"
            | ".pytest_cache"
            | ".ruff_cache"
            | ".pnpm-store"
    ) || name.starts_with("target-")
        || name.starts_with("target_")
        || name.starts_with("outputs-")
        || name.starts_with("outputs_")
        || name.starts_with("cmake-build-")
}

fn skipped_warning_path(warning: &str) -> Option<&str> {
    for prefix in [
        "Skipped sensitive directory ",
        "Skipped sensitive file ",
        "Skipped linked path ",
        "Could not inspect ",
        "Could not read ",
    ] {
        if let Some(rest) = warning.strip_prefix(prefix) {
            return rest
                .split_once(": ")
                .map_or(Some(rest), |(path, _)| Some(path));
        }
    }
    warning
        .strip_prefix("Skipped ")
        .and_then(|rest| rest.split_once(": ").map(|(path, _)| path))
}

fn normalized_warning_path(warning: &str) -> Option<String> {
    skipped_warning_path(warning).map(|path| path.replace('\\', "/").to_ascii_lowercase())
}

fn skipped_warning_path_count(warnings: &[String]) -> usize {
    warnings
        .iter()
        .filter_map(|warning| normalized_warning_path(warning))
        .collect::<BTreeSet<_>>()
        .len()
}

fn deduplicate_checkpoint_warnings(warnings: &mut Vec<String>) {
    let mut paths = BTreeSet::new();
    let mut messages = BTreeSet::new();
    warnings.retain(|warning| match normalized_warning_path(warning) {
        Some(path) => paths.insert(path),
        None => messages.insert(warning.clone()),
    });
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) fn file_icon_key(path: &str) -> &'static str {
    let name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_ascii_lowercase();
    if name == ".env" || name.starts_with(".env.") {
        return "env";
    }
    if name.starts_with("readme") || name.starts_with("changelog") {
        return "markdown";
    }
    match name.as_str() {
        "cargo.toml" | "cargo.lock" | "rust-toolchain" | "rust-toolchain.toml" => "cargo",
        "dockerfile"
        | "compose.yml"
        | "compose.yaml"
        | "docker-compose.yml"
        | "docker-compose.yaml" => "docker",
        "package.json"
        | "package-lock.json"
        | "npm-shrinkwrap.json"
        | "pnpm-lock.yaml"
        | "yarn.lock"
        | "bun.lock"
        | "bun.lockb" => "npm",
        "tsconfig.json" | "tsconfig.base.json" => "typescript",
        "cmakelists.txt" | "makefile" => "cpp",
        "pyproject.toml" | "requirements.txt" => "python",
        "go.mod" | "go.sum" => "go",
        "composer.json" | "composer.lock" => "php",
        "gemfile" | "gemfile.lock" => "ruby",
        "pom.xml" | "build.gradle" | "build.gradle.kts" => "java",
        "pubspec.yaml" | "pubspec.lock" => "dart",
        "mix.exs" | "mix.lock" => "elixir",
        "astro.config.js" | "astro.config.mjs" | "astro.config.ts" => "astro",
        "svelte.config.js" | "svelte.config.mjs" => "svelte",
        ".gitignore" | ".gitattributes" | ".gitmodules" => "git",
        ".editorconfig" | ".prettierrc" | ".eslintrc" | "biome.json" | "vite.config.js"
        | "vite.config.mjs" | "vite.config.ts" => "config",
        "license" | "license.md" | "license.txt" | "copying" | "notice" => "license",
        _ => match Path::new(&name)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
        {
            "rs" => "rust",
            "c" | "h" => "c",
            "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" => "cpp",
            "ts" | "tsx" => "typescript",
            "js" | "jsx" | "mjs" | "cjs" => "javascript",
            "py" | "pyi" => "python",
            "go" => "go",
            "java" => "java",
            "kt" | "kts" => "kotlin",
            "cs" => "csharp",
            "swift" => "swift",
            "rb" => "ruby",
            "php" => "php",
            "dart" => "dart",
            "lua" => "lua",
            "r" => "r_language",
            "ex" | "exs" => "elixir",
            "vue" => "vue",
            "svelte" => "svelte",
            "astro" => "astro",
            "graphql" | "gql" => "graphql",
            "html" | "htm" => "html",
            "css" | "scss" | "sass" | "less" => "css",
            "json" | "jsonc" => "json",
            "toml" => "toml",
            "yaml" | "yml" => "yaml",
            "md" | "mdx" => "markdown",
            "sh" | "bash" | "zsh" | "fish" => "shell",
            "ps1" | "psm1" | "psd1" => "powershell",
            "sql" => "database",
            "db" | "sqlite" | "sqlite3" => "database",
            "csv" | "tsv" => "csv",
            "xml" => "xml",
            "svg" => "svg",
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "ico" | "bmp" => "image",
            "pdf" => "pdf",
            "doc" | "docx" | "odt" | "rtf" => "document",
            "xls" | "xlsx" | "ods" => "spreadsheet",
            "ppt" | "pptx" | "odp" => "presentation",
            "zip" | "7z" | "rar" | "tar" | "gz" | "bz2" | "xz" => "archive",
            "mp3" | "wav" | "flac" | "aac" | "ogg" | "m4a" => "audio",
            "mp4" | "mkv" | "mov" | "avi" | "webm" => "video",
            "ttf" | "otf" | "woff" | "woff2" => "font",
            "exe" | "dll" | "so" | "dylib" | "bin" => "binary",
            "wasm" => "wasm",
            "ini" | "conf" | "cfg" | "properties" => "config",
            "log" => "log",
            "lock" => "lock",
            "txt" => "text",
            _ => "file",
        },
    }
}

fn unix_time_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn runtime_and_workspace() -> (TempDir, TempDir, TimeMachineRuntime) {
        let store = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        let runtime = TimeMachineRuntime::open(store.path().join("time-machine"));
        (store, workspace, runtime)
    }

    fn legacy_snapshot(hash_digit: char) -> SnapshotFile {
        SnapshotFile {
            sha256: hash_digit.to_string().repeat(64),
            bytes: 4,
            text: true,
            readonly: false,
            modified_ns: None,
        }
    }

    #[test]
    fn checkpoint_detects_tree_changes_and_line_statistics() {
        let (_store, workspace, mut runtime) = runtime_and_workspace();
        fs::create_dir_all(workspace.path().join("src")).unwrap();
        fs::write(workspace.path().join("src/main.rs"), "fn main() {}\n").unwrap();
        fs::write(workspace.path().join("old.txt"), "old\n").unwrap();

        runtime
            .begin_run(7, workspace.path(), "Agent", "Chat")
            .unwrap();
        fs::write(
            workspace.path().join("src/main.rs"),
            "fn main() {\n    println!(\"hi\");\n}\n",
        )
        .unwrap();
        fs::write(
            workspace.path().join("Cargo.toml"),
            "[package]\nname='demo'\n",
        )
        .unwrap();
        fs::remove_file(workspace.path().join("old.txt")).unwrap();

        let summary = runtime.finish_run(7).unwrap().unwrap();
        assert_eq!(summary.file_count, 3);
        assert!(summary.total_additions >= 4);
        assert!(summary.total_deletions >= 2);
        assert_eq!(
            summary
                .changes
                .iter()
                .find(|change| change.path == "Cargo.toml")
                .unwrap()
                .icon_key,
            "cargo"
        );
    }

    #[test]
    fn live_diff_tracks_current_lines_without_writing_checkpoint_content() {
        let (_store, workspace, mut runtime) = runtime_and_workspace();
        let changed = workspace.path().join("changed.txt");
        let deleted = workspace.path().join("deleted.txt");
        fs::write(&changed, "one\ntwo\n").unwrap();
        fs::write(&deleted, "gone\n").unwrap();
        runtime
            .begin_run(77, workspace.path(), "Agent", "Chat")
            .unwrap();
        let blob_count = fs::read_dir(&runtime.blobs).unwrap().count();

        fs::write(&changed, "one\nchanged\nthree\n").unwrap();
        fs::write(workspace.path().join("created.txt"), "new a\nnew b\n").unwrap();
        fs::remove_file(&deleted).unwrap();
        let live = runtime.live_diff(77).unwrap().unwrap();
        assert_eq!(live.additions, 4);
        assert_eq!(live.deletions, 2);
        assert_eq!(live.file_count, 3);
        assert!(live.active);
        assert_eq!(fs::read_dir(&runtime.blobs).unwrap().count(), blob_count);

        fs::write(&changed, "one\ntwo\n").unwrap();
        fs::write(&deleted, "gone\n").unwrap();
        fs::remove_file(workspace.path().join("created.txt")).unwrap();
        let reverted = runtime.live_diff(77).unwrap().unwrap();
        assert_eq!(reverted.additions, 0);
        assert_eq!(reverted.deletions, 0);
        assert_eq!(reverted.file_count, 0);
        assert_eq!(fs::read_dir(&runtime.blobs).unwrap().count(), blob_count);
    }

    #[test]
    fn restore_file_rejects_manual_edits_then_restores_expected_content() {
        let (_store, workspace, mut runtime) = runtime_and_workspace();
        let file = workspace.path().join("main.rs");
        fs::write(&file, "before\n").unwrap();
        runtime
            .begin_run(1, workspace.path(), "Agent", "Chat")
            .unwrap();
        fs::write(&file, "after\n").unwrap();
        let summary = runtime.finish_run(1).unwrap().unwrap();

        fs::write(&file, "manual\n").unwrap();
        let error = runtime
            .restore_checkpoint_file(&summary.id, "main.rs")
            .unwrap_err();
        assert!(error.contains("changed manually"));

        fs::write(&file, "after\n").unwrap();
        let restored = runtime
            .restore_checkpoint_file(&summary.id, "main.rs")
            .unwrap();
        assert_eq!(fs::read_to_string(file).unwrap(), "before\n");
        assert!(restored.fully_restored);
    }

    #[test]
    fn complete_restore_handles_modified_created_and_deleted_files() {
        let (_store, workspace, mut runtime) = runtime_and_workspace();
        fs::write(workspace.path().join("modified.txt"), "before\n").unwrap();
        fs::write(workspace.path().join("deleted.txt"), "restore me\n").unwrap();
        runtime
            .begin_run(2, workspace.path(), "Agent", "Agent graph")
            .unwrap();
        fs::write(workspace.path().join("modified.txt"), "after\n").unwrap();
        fs::write(workspace.path().join("created.txt"), "new\n").unwrap();
        fs::remove_file(workspace.path().join("deleted.txt")).unwrap();
        let summary = runtime.finish_run(2).unwrap().unwrap();

        let restored = runtime.restore_checkpoint(&summary.id).unwrap();
        assert_eq!(
            fs::read_to_string(workspace.path().join("modified.txt")).unwrap(),
            "before\n"
        );
        assert!(!workspace.path().join("created.txt").exists());
        assert_eq!(
            fs::read_to_string(workspace.path().join("deleted.txt")).unwrap(),
            "restore me\n"
        );
        assert!(restored.fully_restored);
    }

    #[test]
    fn multi_file_restore_rolls_back_on_failure_and_can_be_retried() {
        let (_store, workspace, mut runtime) = runtime_and_workspace();
        let first = workspace.path().join("first.txt");
        let second = workspace.path().join("second.txt");
        fs::write(&first, "first-before\n").unwrap();
        fs::write(&second, "second-before\n").unwrap();
        runtime
            .begin_run(4, workspace.path(), "Agent", "Chat")
            .unwrap();
        fs::write(&first, "first-after\n").unwrap();
        fs::write(&second, "second-after\n").unwrap();
        let summary = runtime.finish_run(4).unwrap().unwrap();

        runtime.restore_fail_after_applied = Some(1);
        let error = runtime.restore_checkpoint(&summary.id).unwrap_err();
        assert!(error.contains("rolled back"));
        assert_eq!(fs::read_to_string(&first).unwrap(), "first-after\n");
        assert_eq!(fs::read_to_string(&second).unwrap(), "second-after\n");
        assert!(
            fs::read_dir(&runtime.restore_journals)
                .unwrap()
                .next()
                .is_none()
        );
        assert!(
            runtime.checkpoints[&summary.id]
                .changes
                .iter()
                .all(|change| !change.restored)
        );

        runtime.restore_fail_after_applied = None;
        let restored = runtime.restore_checkpoint(&summary.id).unwrap();
        assert!(restored.fully_restored);
        assert_eq!(fs::read_to_string(first).unwrap(), "first-before\n");
        assert_eq!(fs::read_to_string(second).unwrap(), "second-before\n");
    }

    #[test]
    fn reopening_rolls_back_an_uncommitted_restore_journal() {
        let store = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        let store_root = store.path().join("time-machine");
        let first = workspace.path().join("first.txt");
        let second = workspace.path().join("second.txt");
        fs::write(&first, "first-before\n").unwrap();
        fs::write(&second, "second-before\n").unwrap();

        let mut runtime = TimeMachineRuntime::open(store_root.clone());
        runtime
            .begin_run(5, workspace.path(), "Agent", "Chat")
            .unwrap();
        fs::write(&first, "first-after\n").unwrap();
        fs::write(&second, "second-after\n").unwrap();
        let summary = runtime.finish_run(5).unwrap().unwrap();
        let manifest = runtime.checkpoints[&summary.id].clone();
        let operations = manifest
            .changes
            .iter()
            .map(|change| runtime.prepare_restore_operation(workspace.path(), change))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let journal = runtime
            .prepare_restore_journal(workspace.path(), &operations)
            .unwrap();
        assert_eq!(journal.entries.len(), 2);
        runtime.persist_restore_journal(&journal).unwrap();
        runtime.apply_restore_journal_entry(&journal, 0).unwrap();
        drop(runtime);

        let reopened = TimeMachineRuntime::open(store_root);
        assert!(reopened.available);
        assert_eq!(fs::read_to_string(first).unwrap(), "first-after\n");
        assert_eq!(fs::read_to_string(second).unwrap(), "second-after\n");
        assert!(
            fs::read_dir(&reopened.restore_journals)
                .unwrap()
                .next()
                .is_none()
        );
        assert!(
            reopened.checkpoints[&summary.id]
                .changes
                .iter()
                .all(|change| !change.restored)
        );
    }

    #[test]
    fn icon_resolution_covers_special_names_and_languages() {
        assert_eq!(file_icon_key("Cargo.toml"), "cargo");
        assert_eq!(file_icon_key("Dockerfile"), "docker");
        assert_eq!(file_icon_key("web/package.json"), "npm");
        assert_eq!(file_icon_key("src/lib.rs"), "rust");
        assert_eq!(file_icon_key("src/main.cpp"), "cpp");
        assert_eq!(file_icon_key("src/view.tsx"), "typescript");
        assert_eq!(file_icon_key("go.mod"), "go");
        assert_eq!(file_icon_key("composer.json"), "php");
        assert_eq!(file_icon_key("build.gradle.kts"), "java");
        assert_eq!(file_icon_key("ui/Panel.vue"), "vue");
        assert_eq!(file_icon_key("assets/guide.pdf"), "pdf");
        assert_eq!(file_icon_key("reports/metrics.xlsx"), "spreadsheet");
        assert_eq!(file_icon_key(".env.local"), "env");
        assert_eq!(file_icon_key(".gitignore"), "git");
    }

    #[test]
    fn generated_build_variants_and_dependency_vendors_are_not_snapshotted() {
        let (_store, workspace, mut runtime) = runtime_and_workspace();
        for directory in [
            "target-release",
            "target_local",
            "cmake-build-debug",
            "vendor",
            "outputs",
            "outputs-release",
            "outputs_reasoning",
        ] {
            fs::create_dir_all(workspace.path().join(directory)).unwrap();
            fs::write(
                workspace.path().join(directory).join("generated.bin"),
                b"before",
            )
            .unwrap();
        }
        fs::write(workspace.path().join("source.rs"), "fn source() {}\n").unwrap();

        runtime
            .begin_run(91, workspace.path(), "Agent", "Chat")
            .unwrap();
        for directory in [
            "target-release",
            "target_local",
            "cmake-build-debug",
            "vendor",
            "outputs",
            "outputs-release",
            "outputs_reasoning",
        ] {
            fs::write(
                workspace.path().join(directory).join("generated.bin"),
                b"after",
            )
            .unwrap();
        }

        let summary = runtime.finish_run(91).unwrap().unwrap();
        assert!(summary.changes.is_empty());
        assert_eq!(runtime.snapshot_cache.len(), 1);
    }

    #[test]
    fn skipped_path_count_deduplicates_captures_and_ignores_recovery_notes() {
        let mut warnings = vec![
            "Skipped sensitive file secrets/development.pem".to_owned(),
            "Skipped sensitive file secrets/development.pem".to_owned(),
            "Could not read locked: access denied".to_owned(),
            "Recovered after Supervisor stopped before checkpoint completion.".to_owned(),
        ];

        assert_eq!(skipped_warning_path_count(&warnings), 2);
        deduplicate_checkpoint_warnings(&mut warnings);
        assert_eq!(warnings.len(), 3);
        assert_eq!(skipped_warning_path_count(&warnings), 2);
    }

    #[test]
    fn sensitive_credentials_and_secret_directories_are_never_snapshotted() {
        let (_store, workspace, mut runtime) = runtime_and_workspace();
        fs::create_dir_all(workspace.path().join(".ssh")).unwrap();
        fs::create_dir_all(workspace.path().join(".docker")).unwrap();
        fs::write(workspace.path().join(".env.local"), "TOKEN=before\n").unwrap();
        fs::write(workspace.path().join("private.pem"), "private-before\n").unwrap();
        fs::write(workspace.path().join(".ssh/id_ed25519"), "private-before\n").unwrap();
        fs::write(
            workspace.path().join(".docker/config.json"),
            "{\"auths\":{}}\n",
        )
        .unwrap();
        fs::write(workspace.path().join("main.rs"), "before\n").unwrap();

        runtime
            .begin_run(92, workspace.path(), "Agent", "Chat")
            .unwrap();
        let checkpoint_id = runtime.active_runs.get(&92).unwrap();
        let manifest = runtime.checkpoints.get(checkpoint_id).unwrap();
        assert_eq!(
            manifest.baseline.keys().collect::<Vec<_>>(),
            vec!["main.rs"]
        );
        assert!(
            manifest
                .warnings
                .iter()
                .any(|warning| warning.contains("sensitive"))
        );

        fs::write(workspace.path().join(".env.local"), "TOKEN=after\n").unwrap();
        fs::write(workspace.path().join("private.pem"), "private-after\n").unwrap();
        fs::write(workspace.path().join("main.rs"), "after\n").unwrap();
        let summary = runtime.finish_run(92).unwrap().unwrap();
        assert_eq!(summary.file_count, 1);
        assert_eq!(summary.changes[0].path, "main.rs");
    }

    #[test]
    fn large_files_are_skipped_and_total_checkpoint_size_is_enforced() {
        let (_store, workspace, mut runtime) = runtime_and_workspace();
        runtime.limits.max_file_bytes = 4;
        runtime.limits.max_checkpoint_bytes = 8;
        fs::write(workspace.path().join("large.bin"), b"12345").unwrap();
        fs::write(workspace.path().join("small.txt"), b"1234").unwrap();

        runtime
            .begin_run(93, workspace.path(), "Agent", "Chat")
            .unwrap();
        let checkpoint_id = runtime.active_runs.get(&93).unwrap();
        let manifest = runtime.checkpoints.get(checkpoint_id).unwrap();
        assert!(!manifest.baseline.contains_key("large.bin"));
        assert!(manifest.baseline.contains_key("small.txt"));
        assert!(
            manifest
                .warnings
                .iter()
                .any(|warning| warning.contains("per-file limit"))
        );

        let (_store, workspace, mut runtime) = runtime_and_workspace();
        runtime.limits.max_file_bytes = 8;
        runtime.limits.max_checkpoint_bytes = 6;
        fs::write(workspace.path().join("changed.txt"), b"1234").unwrap();
        runtime
            .begin_run(94, workspace.path(), "Agent", "Chat")
            .unwrap();
        let checkpoint_id = runtime.active_runs.get(&94).unwrap().clone();
        fs::write(workspace.path().join("changed.txt"), b"5678").unwrap();
        let error = runtime.finish_run(94).unwrap_err();
        assert!(error.contains("both file versions"));
        assert_eq!(runtime.active_runs.get(&94), Some(&checkpoint_id));
        assert!(runtime.checkpoints[&checkpoint_id].finished_at_ms.is_none());

        runtime.limits.max_checkpoint_bytes = 8;
        let summary = runtime.finish_run(94).unwrap().unwrap();
        assert_eq!(summary.id, checkpoint_id);
        assert_eq!(summary.file_count, 1);
        assert!(!runtime.active_runs.contains_key(&94));
        assert!(runtime.checkpoints[&checkpoint_id].finished_at_ms.is_some());

        let (_store, workspace, mut runtime) = runtime_and_workspace();
        runtime.limits.max_file_bytes = 8;
        runtime.limits.max_checkpoint_bytes = 6;
        fs::write(workspace.path().join("a.txt"), b"1234").unwrap();
        fs::write(workspace.path().join("b.txt"), b"5678").unwrap();
        let error = runtime
            .begin_run(96, workspace.path(), "Agent", "Chat")
            .unwrap_err();
        assert!(error.contains("checkpoint exceeds"));
        assert_eq!(
            fs::read_dir(&runtime.blobs)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| { entry.file_name().to_str().is_some_and(is_sha256_hex) })
                .count(),
            0
        );
    }

    #[test]
    fn unfinished_checkpoint_requires_explicit_discard_and_completed_checkpoint_is_protected() {
        let (_store, workspace, mut runtime) = runtime_and_workspace();
        fs::write(workspace.path().join("main.rs"), "before\n").unwrap();
        runtime
            .begin_run(140, workspace.path(), "Agent", "Chat")
            .unwrap();
        let unfinished_id = runtime.active_runs[&140].clone();

        runtime.discard_unfinished_run(140).unwrap();
        assert!(!runtime.active_runs.contains_key(&140));
        assert!(!runtime.checkpoints.contains_key(&unfinished_id));
        assert!(!runtime.manifest_path(&unfinished_id).exists());

        runtime
            .begin_run(141, workspace.path(), "Agent", "Chat")
            .unwrap();
        let completed = runtime.finish_run(141).unwrap().unwrap();
        let error = runtime.discard_unfinished_run(141).unwrap_err();
        assert!(error.contains("completed checkpoint cannot be discarded"));
        assert!(runtime.checkpoints.contains_key(&completed.id));
        assert!(runtime.manifest_path(&completed.id).exists());
    }

    #[test]
    fn manifests_are_replaced_without_leaving_temporary_files() {
        let (_store, workspace, mut runtime) = runtime_and_workspace();
        fs::write(workspace.path().join("main.rs"), "before\n").unwrap();
        runtime
            .begin_run(95, workspace.path(), "Agent", "Chat")
            .unwrap();
        let checkpoint_id = runtime.active_runs.get(&95).unwrap().clone();
        fs::write(workspace.path().join("main.rs"), "after\n").unwrap();
        runtime.finish_run(95).unwrap().unwrap();

        let bytes = fs::read(runtime.manifest_path(&checkpoint_id)).unwrap();
        let manifest = serde_json::from_slice::<CheckpointManifest>(&bytes).unwrap();
        assert!(manifest.finished_at_ms.is_some());
        assert_eq!(manifest.changes.len(), 1);
        assert!(
            fs::read_dir(&runtime.manifests)
                .unwrap()
                .filter_map(Result::ok)
                .all(
                    |entry| entry.path().extension().and_then(|value| value.to_str())
                        != Some("tmp")
                )
        );
    }

    #[test]
    fn retention_removes_old_manifests_and_gc_only_removes_orphaned_blobs() {
        let (_store, workspace, mut runtime) = runtime_and_workspace();
        runtime.limits.max_retained_checkpoints = 2;
        let file = workspace.path().join("main.rs");
        fs::write(&file, "zero\n").unwrap();
        let mut checkpoint_ids = Vec::new();
        for run_id in 1..=3 {
            runtime
                .begin_run(run_id, workspace.path(), "Agent", "Chat")
                .unwrap();
            fs::write(&file, format!("{run_id}\n")).unwrap();
            checkpoint_ids.push(runtime.finish_run(run_id).unwrap().unwrap().id);
        }
        assert_eq!(runtime.checkpoints.len(), 2);
        assert!(!runtime.manifest_path(&checkpoint_ids[0]).exists());
        assert!(runtime.manifest_path(&checkpoint_ids[1]).exists());
        assert!(runtime.manifest_path(&checkpoint_ids[2]).exists());

        let orphan_name = "f".repeat(64);
        let orphan = runtime.blobs.join(&orphan_name);
        let unknown = runtime.blobs.join("do-not-delete.txt");
        fs::write(&orphan, b"orphan").unwrap();
        fs::write(&unknown, b"unknown").unwrap();
        runtime.garbage_collect_blobs().unwrap();
        assert!(!orphan.exists());
        assert!(unknown.exists());
    }

    #[test]
    fn reopen_rebuilds_cache_from_the_latest_completed_workspace_state() {
        let store = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        let store_root = store.path().join("time-machine");
        let file = workspace.path().join("main.rs");
        fs::write(&file, "before\n").unwrap();

        let mut runtime = TimeMachineRuntime::open(store_root.clone());
        runtime
            .begin_run(120, workspace.path(), "Agent", "Chat")
            .unwrap();
        fs::write(&file, "after\n").unwrap();
        let summary = runtime.finish_run(120).unwrap().unwrap();
        let manifest_json = fs::read_to_string(runtime.manifest_path(&summary.id)).unwrap();
        assert!(manifest_json.contains("\"modifiedNs\""));
        let expected_hash = current_file_hash(&file).unwrap().unwrap();
        drop(runtime);

        let mut reopened = TimeMachineRuntime::open(store_root);
        let canonical_file = fs::canonicalize(&file).unwrap();
        let cached = reopened.snapshot_cache.get(&canonical_file).unwrap();
        assert_eq!(cached.snapshot.sha256, expected_hash);

        let metadata = fs::metadata(&canonical_file).unwrap();
        let hit = reopened
            .cached_snapshot_file(&canonical_file, &metadata)
            .unwrap();
        assert_eq!(hit.sha256, expected_hash);
    }

    #[test]
    fn cache_rehash_detects_same_size_changes_even_when_metadata_appears_unchanged() {
        let (_store, workspace, mut runtime) = runtime_and_workspace();
        let file = workspace.path().join("same-size.txt");
        fs::write(&file, "aaaa").unwrap();
        let metadata = fs::metadata(&file).unwrap();
        let original = runtime.cached_snapshot_file(&file, &metadata).unwrap();

        fs::write(&file, "bbbb").unwrap();
        let changed_metadata = fs::metadata(&file).unwrap();
        let cached = runtime.snapshot_cache.get_mut(&file).unwrap();
        cached.bytes = changed_metadata.len();
        cached.modified_ns = metadata_modified_ns(&changed_metadata);
        cached.readonly = changed_metadata.permissions().readonly();

        let refreshed = runtime
            .cached_snapshot_file(&file, &changed_metadata)
            .unwrap();
        assert_ne!(refreshed.sha256, original.sha256);
        assert_eq!(refreshed.sha256, current_file_hash(&file).unwrap().unwrap());
    }

    #[test]
    fn manifests_without_persistent_fingerprints_remain_compatible() {
        let legacy = serde_json::json!({
            "sha256": "a".repeat(64),
            "bytes": 4,
            "text": true,
            "readonly": false
        });
        let snapshot = serde_json::from_value::<SnapshotFile>(legacy).unwrap();
        assert_eq!(snapshot.modified_ns, None);
    }

    #[test]
    fn corrupt_manifest_is_quarantined_and_disables_destructive_collection() {
        let store = TempDir::new().unwrap();
        let store_root = store.path().join("time-machine");
        let runtime = TimeMachineRuntime::open(store_root.clone());
        let corrupt_id = Uuid::new_v4().to_string();
        fs::write(runtime.manifest_path(&corrupt_id), b"{not-json").unwrap();
        let orphan = runtime.blobs.join("f".repeat(64));
        fs::write(&orphan, b"must survive").unwrap();
        drop(runtime);

        let reopened = TimeMachineRuntime::open(store_root);
        assert!(reopened.available);
        assert!(!reopened.manifest_index_complete);
        assert!(orphan.exists());
        assert!(reopened.garbage_collect_blobs().is_err());
        assert!(
            reopened
                .view()
                .message
                .is_some_and(|message| message.contains("garbage collection remains disabled"))
        );
        assert!(fs::read_dir(&reopened.quarantine).unwrap().next().is_some());
        assert!(!reopened.manifest_path(&corrupt_id).exists());
    }

    #[test]
    fn reopening_finishes_an_interrupted_checkpoint_instead_of_deleting_it() {
        let store = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        let store_root = store.path().join("time-machine");
        let file = workspace.path().join("main.rs");
        fs::write(&file, "before\n").unwrap();
        let mut runtime = TimeMachineRuntime::open(store_root.clone());
        runtime
            .begin_run(122, workspace.path(), "Agent", "Chat")
            .unwrap();
        let checkpoint_id = runtime.active_runs.get(&122).unwrap().clone();
        fs::write(&file, "after\n").unwrap();
        drop(runtime);

        let mut reopened = TimeMachineRuntime::open(store_root);
        let recovered = reopened.checkpoints.get(&checkpoint_id).unwrap();
        assert!(recovered.finished_at_ms.is_some());
        assert_eq!(recovered.changes.len(), 1);
        assert_eq!(recovered.changes[0].path, "main.rs");
        assert!(
            recovered
                .warnings
                .iter()
                .any(|warning| warning.contains("Recovered after Supervisor stopped"))
        );
        assert!(reopened.manifest_path(&checkpoint_id).exists());
        let recovered_summaries = reopened.take_recovered_summaries();
        assert_eq!(recovered_summaries.len(), 1);
        assert_eq!(recovered_summaries[0].id, checkpoint_id);
        assert_eq!(recovered_summaries[0].run_id, 122);
        assert_eq!(reopened.next_run_id_hint(), 123);
        assert!(reopened.take_recovered_summaries().is_empty());
    }

    #[test]
    fn failed_startup_recovery_retains_the_baseline_and_can_be_retried() {
        let store = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        let store_root = store.path().join("time-machine");
        let file = workspace.path().join("main.rs");
        fs::write(&file, "before\n").unwrap();
        let mut runtime = TimeMachineRuntime::open(store_root.clone());
        runtime
            .begin_run(123, workspace.path(), "Agent", "Chat")
            .unwrap();
        let checkpoint_id = runtime.active_runs.get(&123).unwrap().clone();
        let baseline_hash = runtime.checkpoints[&checkpoint_id].baseline["main.rs"]
            .sha256
            .clone();
        fs::remove_file(runtime.blob_path(&baseline_hash)).unwrap();
        fs::write(&file, "after\n").unwrap();
        drop(runtime);

        let mut reopened = TimeMachineRuntime::open(store_root);

        assert!(reopened.available);
        assert!(reopened.checkpoints.contains_key(&checkpoint_id));
        assert_eq!(reopened.active_runs.get(&123), Some(&checkpoint_id));
        assert_eq!(
            reopened.unfinished_runs(),
            vec![(123, fs::canonicalize(workspace.path()).unwrap())]
        );
        assert!(reopened.take_recovered_summaries().is_empty());
        assert!(reopened.manifest_path(&checkpoint_id).exists());
        assert!(
            reopened
                .view()
                .message
                .is_some_and(|message| message.contains("was retained"))
        );

        fs::write(reopened.blob_path(&baseline_hash), b"before\n").unwrap();
        let summary = reopened.finish_run(123).unwrap().unwrap();
        assert_eq!(summary.id, checkpoint_id);
        assert_eq!(summary.file_count, 1);
        assert_eq!(summary.changes[0].path, "main.rs");
        assert!(!reopened.active_runs.contains_key(&123));
        assert!(reopened.unfinished_runs().is_empty());
        assert!(
            reopened.checkpoints[&checkpoint_id]
                .finished_at_ms
                .is_some()
        );
    }

    #[test]
    fn reopening_sanitizes_legacy_sensitive_entries_and_collects_only_their_blobs() {
        let store = TempDir::new().unwrap();
        let workspace = TempDir::new().unwrap();
        let store_root = store.path().join("time-machine");
        let runtime = TimeMachineRuntime::open(store_root.clone());
        let checkpoint_id = Uuid::new_v4().to_string();
        let secret_before = legacy_snapshot('a');
        let secret_after = legacy_snapshot('b');
        let output_before = legacy_snapshot('c');
        let prefixed_output_before = legacy_snapshot('1');
        let normal_before = legacy_snapshot('d');
        let normal_after = legacy_snapshot('e');
        let output_after = legacy_snapshot('f');
        let prefixed_output_after = legacy_snapshot('2');
        for snapshot in [
            &secret_before,
            &secret_after,
            &output_before,
            &output_after,
            &prefixed_output_before,
            &prefixed_output_after,
            &normal_before,
            &normal_after,
        ] {
            fs::write(runtime.blob_path(&snapshot.sha256), b"blob").unwrap();
        }
        let manifest = CheckpointManifest {
            version: MANIFEST_VERSION,
            id: checkpoint_id.clone(),
            run_id: 121,
            workspace_root: workspace.path().to_path_buf(),
            workspace_name: "Legacy workspace".to_owned(),
            provider: "Agent".to_owned(),
            context: "Chat".to_owned(),
            created_at_ms: 1,
            finished_at_ms: Some(2),
            baseline: BTreeMap::from([
                (".env".to_owned(), secret_before.clone()),
                ("outputs/CentralAgent.exe".to_owned(), output_before.clone()),
                (
                    "outputs-reasoning-row-fix/CentralAgent.exe".to_owned(),
                    prefixed_output_before.clone(),
                ),
                ("src/main.rs".to_owned(), normal_before.clone()),
            ]),
            changes: vec![
                ChangedFile {
                    path: ".env".to_owned(),
                    status: ChangedFileStatus::Modified,
                    icon_key: "env".to_owned(),
                    additions: 1,
                    deletions: 1,
                    binary: false,
                    restored: false,
                    before: Some(secret_before),
                    after: Some(secret_after),
                },
                ChangedFile {
                    path: "outputs/CentralAgent.exe".to_owned(),
                    status: ChangedFileStatus::Modified,
                    icon_key: "binary".to_owned(),
                    additions: 0,
                    deletions: 0,
                    binary: true,
                    restored: false,
                    before: Some(output_before),
                    after: Some(output_after),
                },
                ChangedFile {
                    path: "outputs-reasoning-row-fix/CentralAgent.exe".to_owned(),
                    status: ChangedFileStatus::Modified,
                    icon_key: "binary".to_owned(),
                    additions: 0,
                    deletions: 0,
                    binary: true,
                    restored: false,
                    before: Some(prefixed_output_before),
                    after: Some(prefixed_output_after),
                },
                ChangedFile {
                    path: "src/main.rs".to_owned(),
                    status: ChangedFileStatus::Modified,
                    icon_key: "rust".to_owned(),
                    additions: 2,
                    deletions: 1,
                    binary: false,
                    restored: false,
                    before: Some(normal_before.clone()),
                    after: Some(normal_after.clone()),
                },
            ],
            warnings: vec!["preserve me".to_owned()],
        };
        runtime.persist_manifest(&manifest).unwrap();
        drop(runtime);

        let reopened = TimeMachineRuntime::open(store_root);
        let sanitized = reopened.checkpoints.get(&checkpoint_id).unwrap();
        assert_eq!(
            sanitized.baseline.keys().collect::<Vec<_>>(),
            vec!["src/main.rs"]
        );
        assert_eq!(sanitized.changes.len(), 1);
        assert_eq!(sanitized.changes[0].path, "src/main.rs");
        assert_eq!(sanitized.warnings, vec!["preserve me"]);
        let summary = sanitized.summary().unwrap();
        assert_eq!(summary.file_count, 1);
        assert_eq!(summary.total_additions, 2);
        assert_eq!(summary.total_deletions, 1);

        let persisted = serde_json::from_slice::<CheckpointManifest>(
            &fs::read(reopened.manifest_path(&checkpoint_id)).unwrap(),
        )
        .unwrap();
        assert_eq!(persisted.baseline.len(), 1);
        assert_eq!(persisted.changes.len(), 1);
        for removed_hash in ['a', 'b', 'c', 'f', '1', '2'] {
            assert!(
                !reopened
                    .blob_path(&removed_hash.to_string().repeat(64))
                    .exists()
            );
        }
        assert!(reopened.blob_path(&normal_before.sha256).exists());
        assert!(reopened.blob_path(&normal_after.sha256).exists());
    }

    #[test]
    fn checkpoint_paths_cannot_escape_the_workspace() {
        let (_store, workspace, mut runtime) = runtime_and_workspace();
        fs::write(workspace.path().join("main.rs"), "before\n").unwrap();
        runtime
            .begin_run(3, workspace.path(), "Agent", "Chat")
            .unwrap();
        fs::write(workspace.path().join("main.rs"), "after\n").unwrap();
        let summary = runtime.finish_run(3).unwrap().unwrap();
        assert!(
            runtime
                .restore_checkpoint_file(&summary.id, "../escape.txt")
                .is_err()
        );
    }
}
