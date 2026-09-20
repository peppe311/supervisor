use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const AUDIT_VERSION: u8 = 1;
pub(crate) const DEFAULT_AUDIT_RETENTION_DAYS: u64 = 30;
const MAX_AUDIT_FILES: usize = 64;
const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuditRuntimeView {
    pub available: bool,
    pub integrity_verified: bool,
    pub session_id: String,
    pub entries: u64,
    pub recovered_unclean_sessions: usize,
    pub retention_days: u64,
    pub maximum_files: usize,
    pub directory: String,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditRecord {
    version: u8,
    session_id: String,
    sequence: u64,
    timestamp_ms: u128,
    event_type: String,
    request_id_hash: String,
    action: String,
    scope: String,
    effect: String,
    status: String,
    summary_hash: String,
    previous_hash: String,
    record_hash: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditPayload<'a> {
    version: u8,
    session_id: &'a str,
    sequence: u64,
    timestamp_ms: u128,
    event_type: &'a str,
    request_id_hash: &'a str,
    action: &'a str,
    scope: &'a str,
    effect: &'a str,
    status: &'a str,
    summary_hash: &'a str,
    previous_hash: &'a str,
}

pub(crate) struct AuditRuntime {
    directory: PathBuf,
    session_id: String,
    file: Option<File>,
    sequence: u64,
    previous_hash: String,
    integrity_verified: bool,
    recovered_unclean_sessions: usize,
    status: String,
    retention_days: u64,
}

impl AuditRuntime {
    pub(crate) fn open(data_directory: &Path, retention_days: u64) -> Self {
        let directory = data_directory.join("audit");
        let session_id = Uuid::new_v4().to_string();
        let retention_days = normalize_retention_days(retention_days);
        let mut runtime = Self {
            directory: directory.clone(),
            session_id,
            file: None,
            sequence: 0,
            previous_hash: GENESIS_HASH.to_owned(),
            integrity_verified: true,
            recovered_unclean_sessions: 0,
            status: "Opening the local audit journal…".to_owned(),
            retention_days,
        };

        if let Err(error) = fs::create_dir_all(&directory) {
            runtime.integrity_verified = false;
            runtime.status = format!("Audit journal is unavailable: {error}");
            return runtime;
        }

        let verification = verify_existing_files(&directory);
        runtime.integrity_verified = verification.invalid_files == 0;
        runtime.recovered_unclean_sessions = verification.unclean_sessions;
        let _ = prune_old_audit_files(&directory, retention_days);

        let path = directory.join(format!(
            "audit-{}-{}.jsonl",
            unix_time_ms(),
            runtime.session_id
        ));
        match OpenOptions::new().create_new(true).append(true).open(path) {
            Ok(file) => {
                runtime.file = Some(file);
                runtime.status = if verification.invalid_files == 0 {
                    if verification.unclean_sessions == 0 {
                        "Audit hash chains verified; current session journal is active".to_owned()
                    } else {
                        format!(
                            "Audit hash chains verified; {} prior session(s) ended unexpectedly",
                            verification.unclean_sessions
                        )
                    }
                } else {
                    format!(
                        "Integrity warning: {} existing audit file(s) failed verification",
                        verification.invalid_files
                    )
                };
                runtime.record(
                    "session_started",
                    "",
                    "application_session",
                    "runtime",
                    "metadata",
                    "running",
                    "Supervisor session started",
                );
            }
            Err(error) => {
                runtime.integrity_verified = false;
                runtime.status = format!("Could not create the current audit journal: {error}");
            }
        }
        runtime
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record(
        &mut self,
        event_type: &str,
        request_id: &str,
        action: &str,
        scope: &str,
        effect: &str,
        status: &str,
        summary: &str,
    ) {
        if self.file.is_none() {
            return;
        }
        let sequence = self.sequence.saturating_add(1);
        let timestamp_ms = unix_time_ms();
        let request_id_hash = digest_text(request_id);
        let summary_hash = digest_text(summary);
        let action = bounded_label(action);
        let scope = bounded_label(scope);
        let effect = bounded_label(effect);
        let status = bounded_label(status);
        let payload = AuditPayload {
            version: AUDIT_VERSION,
            session_id: &self.session_id,
            sequence,
            timestamp_ms,
            event_type,
            request_id_hash: &request_id_hash,
            action: &action,
            scope: &scope,
            effect: &effect,
            status: &status,
            summary_hash: &summary_hash,
            previous_hash: &self.previous_hash,
        };
        let record_hash = hash_payload(&payload);
        let record = AuditRecord {
            version: AUDIT_VERSION,
            session_id: self.session_id.clone(),
            sequence,
            timestamp_ms,
            event_type: event_type.to_owned(),
            request_id_hash,
            action,
            scope,
            effect,
            status,
            summary_hash,
            previous_hash: self.previous_hash.clone(),
            record_hash: record_hash.clone(),
        };
        let write_result = serde_json::to_vec(&record)
            .map_err(|error| error.to_string())
            .and_then(|bytes| {
                let file = self.file.as_mut().expect("audit file checked above");
                file.write_all(&bytes)
                    .and_then(|_| file.write_all(b"\n"))
                    .and_then(|_| file.flush())
                    .and_then(|_| file.sync_data())
                    .map_err(|error| error.to_string())
            });
        match write_result {
            Ok(()) => {
                self.sequence = sequence;
                self.previous_hash = record_hash;
            }
            Err(error) => {
                self.file = None;
                self.integrity_verified = false;
                self.status = format!("Audit journal stopped after a write failure: {error}");
            }
        }
    }

    pub(crate) fn view(&self) -> AuditRuntimeView {
        AuditRuntimeView {
            available: self.file.is_some(),
            integrity_verified: self.integrity_verified,
            session_id: self.session_id.clone(),
            entries: self.sequence,
            recovered_unclean_sessions: self.recovered_unclean_sessions,
            retention_days: self.retention_days,
            maximum_files: MAX_AUDIT_FILES,
            directory: self.directory.display().to_string(),
            status: self.status.clone(),
        }
    }

    pub(crate) fn set_retention_days(&mut self, retention_days: u64) -> Result<(), String> {
        if !matches!(retention_days, 7 | 30 | 90) {
            return Err("Audit retention must be 7, 30, or 90 days".to_owned());
        }
        prune_old_audit_files(&self.directory, retention_days)?;
        self.retention_days = retention_days;
        self.status = format!("Audit hash chains verified; retention is now {retention_days} days");
        Ok(())
    }

    pub(crate) fn retention_days(&self) -> u64 {
        self.retention_days
    }
}

impl Drop for AuditRuntime {
    fn drop(&mut self) {
        self.record(
            "session_closed",
            "",
            "application_session",
            "runtime",
            "metadata",
            "success",
            "Supervisor session closed cleanly",
        );
    }
}

struct VerificationSummary {
    invalid_files: usize,
    unclean_sessions: usize,
}

fn verify_existing_files(directory: &Path) -> VerificationSummary {
    let mut summary = VerificationSummary {
        invalid_files: 0,
        unclean_sessions: 0,
    };
    let Ok(entries) = fs::read_dir(directory) else {
        return summary;
    };
    for path in entries.flatten().map(|entry| entry.path()).filter(|path| {
        path.extension()
            .is_some_and(|extension| extension == "jsonl")
    }) {
        match verify_file(&path) {
            Ok(clean) => {
                if !clean {
                    summary.unclean_sessions = summary.unclean_sessions.saturating_add(1);
                }
            }
            Err(()) => summary.invalid_files = summary.invalid_files.saturating_add(1),
        }
    }
    summary
}

fn verify_file(path: &Path) -> Result<bool, ()> {
    let file = File::open(path).map_err(|_| ())?;
    let mut previous_hash = GENESIS_HASH.to_owned();
    let mut expected_sequence = 1_u64;
    let mut last_event = None;
    let mut saw_record = false;
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|_| ())?;
        let record: AuditRecord = serde_json::from_str(&line).map_err(|_| ())?;
        if record.version != AUDIT_VERSION
            || record.sequence != expected_sequence
            || record.previous_hash != previous_hash
        {
            return Err(());
        }
        let payload = AuditPayload {
            version: record.version,
            session_id: &record.session_id,
            sequence: record.sequence,
            timestamp_ms: record.timestamp_ms,
            event_type: &record.event_type,
            request_id_hash: &record.request_id_hash,
            action: &record.action,
            scope: &record.scope,
            effect: &record.effect,
            status: &record.status,
            summary_hash: &record.summary_hash,
            previous_hash: &record.previous_hash,
        };
        if hash_payload(&payload) != record.record_hash {
            return Err(());
        }
        previous_hash = record.record_hash;
        expected_sequence = expected_sequence.saturating_add(1);
        last_event = Some(record.event_type);
        saw_record = true;
    }
    if !saw_record {
        return Err(());
    }
    Ok(last_event.as_deref() == Some("session_closed"))
}

fn prune_old_audit_files(directory: &Path, retention_days: u64) -> Result<(), String> {
    let mut files = fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path
                .extension()
                .is_none_or(|extension| extension != "jsonl")
            {
                return None;
            }
            let modified = entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .unwrap_or(UNIX_EPOCH);
            Some((path, modified))
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
    let maximum_age = Duration::from_secs(retention_days * 24 * 60 * 60);
    for (index, (path, modified)) in files.into_iter().enumerate() {
        let expired = SystemTime::now()
            .duration_since(modified)
            .is_ok_and(|age| age > maximum_age);
        if (expired || index >= MAX_AUDIT_FILES) && verify_file(&path) == Ok(true) {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn normalize_retention_days(value: u64) -> u64 {
    match value {
        7 | 30 | 90 => value,
        _ => DEFAULT_AUDIT_RETENTION_DAYS,
    }
}

fn hash_payload(payload: &AuditPayload<'_>) -> String {
    let bytes = serde_json::to_vec(payload).expect("audit payload is serializable");
    hex_digest(&bytes)
}

fn digest_text(value: &str) -> String {
    hex_digest(value.as_bytes())
}

fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn bounded_label(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(96)
        .collect()
}

fn unix_time_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_verifies_a_metadata_only_hash_chain() {
        let directory = tempfile::tempdir().unwrap();
        let audit_directory = directory.path().join("audit");
        {
            let mut runtime = AuditRuntime::open(directory.path(), DEFAULT_AUDIT_RETENTION_DAYS);
            runtime.record(
                "command_finished",
                "request-secret-id",
                "workspace_apply_patch",
                "workspace",
                "filesystem_write",
                "success",
                "private command and content",
            );
            assert!(runtime.view().available);
            assert_eq!(runtime.view().entries, 2);
        }
        let summary = verify_existing_files(&audit_directory);
        assert_eq!(summary.invalid_files, 0);
        assert_eq!(summary.unclean_sessions, 0);
        let path = fs::read_dir(&audit_directory)
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|value| value == "jsonl"))
            .unwrap();
        let contents = fs::read_to_string(path).unwrap();
        assert!(!contents.contains("request-secret-id"));
        assert!(!contents.contains("private command and content"));
    }

    #[test]
    fn detects_a_modified_audit_record() {
        let directory = tempfile::tempdir().unwrap();
        let audit_directory = directory.path().join("audit");
        {
            let mut runtime = AuditRuntime::open(directory.path(), DEFAULT_AUDIT_RETENTION_DAYS);
            runtime.record(
                "safety_stop",
                "",
                "computer_control",
                "window",
                "destructive",
                "denied",
                "Emergency stop",
            );
        }
        let path = fs::read_dir(&audit_directory)
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|value| value == "jsonl"))
            .unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        fs::write(&path, contents.replacen("success", "tampered", 1)).unwrap();
        assert!(verify_file(&path).is_err());
    }

    #[test]
    fn retention_accepts_only_the_visible_settings_choices() {
        let directory = tempfile::tempdir().unwrap();
        let mut runtime = AuditRuntime::open(directory.path(), DEFAULT_AUDIT_RETENTION_DAYS);
        assert!(runtime.set_retention_days(7).is_ok());
        assert_eq!(runtime.retention_days(), 7);
        assert!(runtime.set_retention_days(365).is_err());
        assert_eq!(runtime.retention_days(), 7);
    }
}
