//! One-time, lossless transfer of Supervisor-owned native rollout files.
//! Codex rebuilds its own indexes; Supervisor never reconstructs a transcript,
//! imports credentials/configuration, or changes the source profile.
use central_agent_codex_runtime::{conversations::Saved, runtime::Runtime};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
};

const MARKER: &str = "supervisor-profile.json";

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Report {
    version: u32,
    source: Option<PathBuf>,
    copied: BTreeMap<String, CopyRecord>,
    missing: Vec<String>,
    #[serde(default)]
    hydrated: bool,
    #[serde(default)]
    metadata_captured: bool,
    #[serde(default)]
    metadata_imported: bool,
    #[serde(default)]
    metadata: BTreeMap<String, Metadata>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Metadata {
    name: Option<String>,
    goal: Option<central_agent_codex_runtime::goals::Goal>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CopyRecord {
    path: PathBuf,
    bytes: u64,
    sha256: String,
}

pub(super) fn prepare(data: &Path, saved: &Saved) -> Result<PathBuf, String> {
    let source = std::env::var_os("CODEX_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".codex")));
    prepare_from(data, saved, source.as_deref())
        .map_err(|error| format!("Could not prepare Supervisor's separate Codex profile: {error}. The shared profile has not been changed."))
}

fn prepare_from(data: &Path, saved: &Saved, source: Option<&Path>) -> Result<PathBuf, String> {
    saved.validate()?;
    let data = data.canonicalize().map_err(|error| error.to_string())?;
    let home = data.join("codex");
    // A committed profile is authoritative even after deletion or sign-out.
    // Never restore files from the old profile on a later connection.
    if home.try_exists().map_err(|error| error.to_string())? {
        if !ordinary(&home)?.is_dir() {
            return Err("The existing profile has no valid migration record".into());
        }
        let record = ordinary(&home.join(MARKER))
            .map_err(|_| "The existing profile has no valid migration record")?;
        if !record.is_file() || record.len() > 1024 * 1024 {
            return Err("The migration record is invalid or exceeds its size limit".into());
        }
        let bytes = fs::read(home.join(MARKER)).map_err(|error| error.to_string())?;
        if bytes.len() > 1024 * 1024 {
            return Err("The migration record exceeds its size limit".into());
        }
        let report: Report = serde_json::from_slice(&bytes)
            .map_err(|_| "The existing migration record is invalid")?;
        if report.version != 1 {
            return Err("Unsupported migration record version".into());
        }
        if report
            .source
            .as_ref()
            .is_some_and(|source| home.starts_with(source) || source.starts_with(&home))
        {
            return Err("The separate and shared profiles overlap".into());
        }
        return Ok(home);
    }
    let source = source
        .map(|path| {
            if !path.is_absolute() {
                return Err("The previous Codex profile path must be absolute".to_owned());
            }
            if !path.try_exists().map_err(|error| error.to_string())? {
                return Ok(None);
            }
            if !ordinary(path)?.is_dir() {
                return Err("The previous Codex profile is not a directory".into());
            }
            path.canonicalize()
                .map(Some)
                .map_err(|error| error.to_string())
        })
        .transpose()?
        .flatten();
    if source
        .as_ref()
        .is_some_and(|source| home.starts_with(source) || source.starts_with(&home))
    {
        return Err("The separate and shared profiles overlap".into());
    }
    let deleted = saved
        .bindings
        .values()
        .filter(|binding| binding.deleted)
        .map(|binding| binding.thread_id.clone())
        .collect::<BTreeSet<_>>();
    let mut ids = saved
        .bindings
        .values()
        .filter(|binding| !binding.deleted)
        .map(|binding| binding.thread_id.clone())
        .collect::<BTreeSet<_>>();
    // Also retain known parents/children that have no open local chat card.
    ids.extend(saved.fork_origins.values().cloned());
    for (child, parent) in saved.lineage.iter().chain(saved.delegations.iter()) {
        ids.insert(child.clone());
        ids.insert(parent.clone());
    }
    ids.retain(|id| !deleted.contains(id));
    if ids
        .iter()
        .any(|id| id.len() != 36 || uuid::Uuid::parse_str(id).is_err())
    {
        return Err("A saved native conversation identifier is invalid".into());
    }
    let mut files = BTreeMap::new();
    if let Some(source) = source.as_ref().filter(|_| !ids.is_empty()) {
        let mut budget = 200_000;
        for directory in ["sessions", "archived_sessions"] {
            let root = source.join(directory);
            if root.try_exists().map_err(|error| error.to_string())? {
                scan(source, &root, &ids, &mut files, 0, &mut budget)?;
            }
        }
    }
    let stage = tempfile::Builder::new()
        .prefix(".codex-profile-")
        .tempdir_in(&data)
        .map_err(|error| error.to_string())?;
    let mut report = Report {
        version: 1,
        source: source.clone(),
        copied: BTreeMap::new(),
        missing: Vec::new(),
        hydrated: false,
        metadata_captured: false,
        metadata_imported: false,
        metadata: BTreeMap::new(),
    };
    for id in ids {
        if let Some(relative) = files.get(&id) {
            let origin = source.as_ref().unwrap().join(relative);
            let destination = stage.path().join(relative);
            let (bytes, sha256) = copy_rollout(&origin, &destination, &id)?;
            report.copied.insert(
                id,
                CopyRecord {
                    path: relative.clone(),
                    bytes,
                    sha256,
                },
            );
        } else {
            report.missing.push(id);
        }
    }
    let mut marker =
        fs::File::create(stage.path().join(MARKER)).map_err(|error| error.to_string())?;
    marker
        .write_all(&serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?)
        .and_then(|()| marker.sync_all())
        .map_err(|error| error.to_string())?;
    drop(marker);
    // The caller holds the Supervisor binding lock. A failed transfer never
    // publishes a partial profile or falls back to the shared account.
    fs::rename(stage.path(), &home).map_err(|error| error.to_string())?;
    Ok(home)
}

fn ordinary(path: &Path) -> Result<fs::Metadata, String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    let mut redirected = metadata.file_type().is_symlink();
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        redirected |= metadata.file_attributes() & 0x400 != 0;
    }
    if redirected || !(metadata.is_dir() || metadata.is_file()) {
        return Err("Profile migration does not follow filesystem links".into());
    }
    Ok(metadata)
}

fn scan(
    source: &Path,
    directory: &Path,
    ids: &BTreeSet<String>,
    files: &mut BTreeMap<String, PathBuf>,
    depth: usize,
    budget: &mut usize,
) -> Result<(), String> {
    if depth > 5 || !ordinary(directory)?.is_dir() {
        return Err("Unexpected native session directory structure".into());
    }
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        if *budget == 0 {
            return Err("Native session enumeration exceeded its limit".into());
        }
        *budget -= 1;
        let entry = entry.map_err(|error| error.to_string())?;
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if kind.is_dir() {
            scan(source, &entry.path(), ids, files, depth + 1, budget)?;
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name
            .to_str()
            .and_then(|name| name.strip_prefix("rollout-"))
            .and_then(|name| name.strip_suffix(".jsonl"))
        else {
            continue;
        };
        let Some(id) = name.get(name.len().saturating_sub(36)..) else {
            continue;
        };
        if !ids.contains(id) {
            continue;
        }
        if !ordinary(&entry.path())?.is_file() {
            return Err("A native session is not a regular file".into());
        }
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|error| error.to_string())?
            .to_owned();
        if files.insert(id.to_owned(), relative).is_some() {
            return Err(format!("More than one native session file matches {id}"));
        }
    }
    Ok(())
}

fn digest(path: &Path) -> Result<String, String> {
    let mut input = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = input.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn copy_rollout(source: &Path, destination: &Path, id: &str) -> Result<(u64, String), String> {
    let before = ordinary(source)?;
    // Native histories can contain large image/tool payloads. Stream exactly
    // the observed file plus one byte to detect growth without a memory spike
    // or an arbitrary history-size cutoff.
    let transfer_limit = before
        .len()
        .checked_add(1)
        .ok_or("Native session size overflow")?;
    let mut first = Vec::new();
    BufReader::new(
        fs::File::open(source)
            .map_err(|error| error.to_string())?
            .take(4 * 1024 * 1024),
    )
    .read_until(b'\n', &mut first)
    .map_err(|error| error.to_string())?;
    let meta: serde_json::Value =
        serde_json::from_slice(&first).map_err(|_| "Invalid native session metadata")?;
    if meta["type"] != "session_meta" || meta["payload"]["id"] != id {
        return Err("Native session metadata does not match the saved conversation".into());
    }
    fs::create_dir_all(destination.parent().unwrap()).map_err(|error| error.to_string())?;
    let input = fs::File::open(source).map_err(|error| error.to_string())?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| error.to_string())?;
    let bytes = io::copy(&mut input.take(transfer_limit), &mut output)
        .map_err(|error| error.to_string())?;
    output.sync_all().map_err(|error| error.to_string())?;
    drop(output);
    let sha256 = digest(destination)?;
    let original_hash = digest(source)?;
    let after = ordinary(source)?;
    if bytes != before.len()
        || bytes != after.len()
        || before.modified().ok() != after.modified().ok()
        || sha256 != original_hash
    {
        return Err(
            "A native conversation changed during transfer; close its running client and reconnect"
                .into(),
        );
    }
    Ok((bytes, sha256))
}

#[cfg(test)]
mod tests;

mod hydration;
mod metadata;
pub(super) fn finish(runtime: &Runtime) -> Result<(), String> {
    hydration::finish(runtime).map_err(|error| format!(
        "Supervisor's history transfer needs to finish before connecting: {error}. Reconnect to retry; the original archive is unchanged."
    ))
}
