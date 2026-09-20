//! Local manual editor. Content enters an agent draft only through explicit snapshots.
mod context;
use crate::workspace::WorkspaceRuntime;
pub(crate) use context::Selection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use uuid::Uuid;

const MAX_DOCUMENT_BYTES: usize = 1_000_000;
const MAX_DRAFT_BYTES: usize = 16_000_000;
// JSON can escape a single text byte into six bytes (for example a control character).
const MAX_RECOVERY_BYTES: usize = MAX_DRAFT_BYTES * 6 + 4_000_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditorFile {
    pub root: String,
    pub path: String,
    pub content: String,
    pub sha256: String,
    pub line_ending: String,
    pub bom: bool,
    pub read_only: bool,
    #[serde(default)]
    pub icon_key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, WorkspaceRuntime, FileEditor) {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().join("project");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("main.rs"), "fn main() {}\n").unwrap();
        let workspace = WorkspaceRuntime::scoped(&project).unwrap();
        let editor = FileEditor::load(temp.path().join("recovery/drafts.json"));
        (temp, workspace, editor)
    }

    #[test]
    fn editor_preserves_bom_crlf_unicode_and_empty_files() {
        let (temp, workspace, _) = fixture();
        let path = temp.path().join("project/main.rs");
        fs::write(&path, "\u{feff}prima\r\nseconda\r\n").unwrap();
        let file = workspace.read_editor_file("main.rs").unwrap();
        assert_eq!(file.content, "prima\nseconda\n");
        assert_eq!(file.line_ending, "CRLF");
        assert!(file.bom);
        assert_eq!(file.icon_key, "rust");
        workspace
            .save_editor_file("main.rs", "città 🦀\n", &file.sha256)
            .unwrap();
        assert_eq!(fs::read(&path).unwrap(), "\u{feff}città 🦀\r\n".as_bytes());
        fs::write(&path, "").unwrap();
        let empty = workspace.read_editor_file("main.rs").unwrap();
        workspace
            .save_editor_file("main.rs", "new\n", &empty.sha256)
            .unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), "new\n");
    }

    #[test]
    fn editor_conflicts_keep_disk_and_draft_unchanged() {
        let (temp, workspace, mut editor) = fixture();
        editor.open(&workspace, "main.rs").unwrap();
        let id = editor.active_id.clone().unwrap();
        editor.change(&id, "manual edit".into(), 1).unwrap();
        fs::write(temp.path().join("project/main.rs"), "external edit").unwrap();
        assert!(
            editor
                .save(&id, &workspace)
                .unwrap_err()
                .contains("changed")
        );
        assert_eq!(
            fs::read_to_string(temp.path().join("project/main.rs")).unwrap(),
            "external edit"
        );
        assert_eq!(editor.document(&id).unwrap().file.content, "manual edit");
        assert!(editor.document(&id).unwrap().dirty);
    }

    #[test]
    fn editor_recovers_only_dirty_drafts_and_requires_discard_confirmation() {
        let (_temp, workspace, mut editor) = fixture();
        editor.open(&workspace, "main.rs").unwrap();
        let id = editor.active_id.clone().unwrap();
        editor.change(&id, "draft".into(), 1).unwrap();
        editor.flush().unwrap();
        let mut recovered = FileEditor::load(editor.storage.clone());
        assert_eq!(recovered.documents[0].file.content, "draft");
        assert!(recovered.active_id.is_none());
        assert!(recovered.close(&id, false).is_err());
        assert!(recovered.reload(&id, &workspace, false).is_err());
        recovered.save(&id, &workspace).unwrap();
        assert!(
            FileEditor::load(editor.storage.clone())
                .documents
                .is_empty()
        );
        recovered.close(&id, false).unwrap();
    }

    #[test]
    fn editor_refuses_retarget_and_stale_updates() {
        let (temp, workspace, mut editor) = fixture();
        editor.open(&workspace, "main.rs").unwrap();
        editor.open(&workspace, "./main.rs").unwrap();
        assert_eq!(editor.documents.len(), 1);
        let id = editor.active_id.clone().unwrap();
        editor.change(&id, "new".into(), 2).unwrap();
        assert!(editor.change(&id, "old".into(), 1).is_err());
        assert_eq!(editor.document(&id).unwrap().file.content, "new");
        let other = temp.path().join("other");
        fs::create_dir(&other).unwrap();
        fs::write(other.join("main.rs"), "other project").unwrap();
        assert!(
            editor
                .save(&id, &WorkspaceRuntime::scoped(&other).unwrap())
                .is_err()
        );
        assert_eq!(
            fs::read_to_string(other.join("main.rs")).unwrap(),
            "other project"
        );
    }

    #[test]
    fn editor_rejects_binary_large_mixed_secret_and_outside_files() {
        let (temp, workspace, _) = fixture();
        for (name, bytes) in [
            ("binary.bin", vec![0, 1, 2]),
            ("encoding.txt", vec![0xff, 0xfe]),
            ("large.txt", vec![b'a'; MAX_DOCUMENT_BYTES + 1]),
            ("mixed.txt", b"a\r\nb\n".to_vec()),
            (".env", b"SECRET=fixture".to_vec()),
        ] {
            fs::write(temp.path().join("project").join(name), bytes).unwrap();
            assert!(workspace.read_editor_file(name).is_err(), "accepted {name}");
        }
        assert!(workspace.read_editor_file("../outside.txt").is_err());
        assert!(workspace.read_editor_file("C:/Windows/win.ini").is_err());
        assert!(workspace.read_editor_file(".").is_err());
    }

    #[test]
    fn editor_read_only_save_does_not_replace_file() {
        let (temp, workspace, _) = fixture();
        let path = temp.path().join("project/main.rs");
        let original = fs::metadata(&path).unwrap().permissions();
        let mut readonly = original.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&path, readonly).unwrap();
        let file = workspace.read_editor_file("main.rs").unwrap();
        assert!(file.read_only);
        let result = workspace.save_editor_file("main.rs", "changed", &file.sha256);
        fs::set_permissions(&path, original).unwrap();
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "fn main() {}\n");
    }

    #[test]
    fn editor_undo_to_original_is_not_a_dirty_draft() {
        let (_temp, workspace, mut editor) = fixture();
        editor.open(&workspace, "main.rs").unwrap();
        let doc = editor.documents[0].clone();
        editor.change(&doc.id, "draft".into(), 1).unwrap();
        editor.change(&doc.id, doc.file.content, 2).unwrap();
        assert!(!editor.document(&doc.id).unwrap().dirty);
        editor.flush().unwrap();
        assert!(
            FileEditor::load(editor.storage.clone())
                .documents
                .is_empty()
        );
    }

    #[test]
    fn editor_invalid_recovery_is_retained() {
        let (temp, workspace, _) = fixture();
        let storage = temp.path().join("broken.json");
        fs::write(&storage, b"invalid fixture").unwrap();
        let mut editor = FileEditor::load(storage.clone());
        editor.open(&workspace, "main.rs").unwrap();
        let id = editor.active_id.clone().unwrap();
        editor.change(&id, "draft".into(), 1).unwrap();
        assert!(editor.flush().is_err());
        assert_eq!(fs::read(&storage).unwrap(), b"invalid fixture");
        // Explicitly saved files allow closing even if the old recovery file is corrupt.
        editor.save(&id, &workspace).unwrap();
        assert!(editor.flush().is_ok());
    }
}

impl EditorFile {
    pub fn encode(&self, content: &str) -> Result<Vec<u8>, String> {
        if content.contains(['\0', '\r']) {
            return Err("The editor buffer contains invalid text characters.".into());
        }
        let text = if self.line_ending == "CRLF" {
            content.replace('\n', "\r\n")
        } else {
            content.into()
        };
        let mut bytes = if self.bom {
            vec![0xef, 0xbb, 0xbf]
        } else {
            Vec::new()
        };
        bytes.extend_from_slice(text.as_bytes());
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err("The edited file exceeds the editor's 1 MB limit.".into());
        }
        Ok(bytes)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditorDocument {
    pub id: String,
    #[serde(flatten)]
    pub file: EditorFile,
    pub dirty: bool,
    pub version: u64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum EditorAction {
    Open {
        root: String,
        path: String,
    },
    Change {
        id: String,
        content: String,
        version: u64,
    },
    Activate {
        id: Option<String>,
    },
    Selection {
        id: String,
        version: u64,
        start: usize,
        end: usize,
    },
    Save {
        id: String,
        content: String,
        version: u64,
    },
    Reload {
        id: String,
        #[serde(default)]
        discard: bool,
    },
    Close {
        id: String,
        #[serde(default)]
        discard: bool,
    },
    Ready,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EditorView<'a> {
    documents: &'a [EditorDocument],
    active_id: &'a Option<String>,
    status: &'a str,
}

pub(crate) struct FileEditor {
    pub documents: Vec<EditorDocument>,
    pub active_id: Option<String>,
    pub status: String,
    selections: std::collections::HashMap<String, (u64, Selection)>,
    storage: PathBuf,
    flush_at: Option<Instant>,
    recovery_blocked: bool,
}

impl FileEditor {
    pub fn load(storage: PathBuf) -> Self {
        let mut editor = Self {
            documents: vec![],
            active_id: None,
            status: String::new(),
            selections: Default::default(),
            storage,
            flush_at: None,
            recovery_blocked: false,
        };
        let restored = (|| {
            use std::io::Read;
            let file = fs::File::open(&editor.storage)?;
            let mut bytes = vec![];
            file.take((MAX_RECOVERY_BYTES + 1) as u64)
                .read_to_end(&mut bytes)?;
            if bytes.len() > MAX_RECOVERY_BYTES {
                return Err(std::io::Error::other("Editor recovery file is too large"));
            }
            let docs: Vec<EditorDocument> = serde_json::from_slice(&bytes)?;
            if docs.iter().map(|doc| doc.file.content.len()).sum::<usize>() > MAX_DRAFT_BYTES
                || docs.iter().any(|doc| {
                    doc.file.encode(&doc.file.content).is_err() || Uuid::parse_str(&doc.id).is_err()
                })
            {
                return Err(std::io::Error::other("Invalid editor draft data"));
            }
            Ok(docs)
        })();
        match restored {
            Ok(documents) => {
                editor.documents = documents;
                if !editor.documents.is_empty() {
                    editor.status =
                        "Recovered unsaved drafts. Select a file tab to continue.".into();
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                editor.status = format!(
                    "Could not recover editor drafts: {error}. The original recovery file was retained."
                );
                editor.recovery_blocked = true;
            }
        }
        editor
    }

    pub fn view(&self) -> EditorView<'_> {
        EditorView {
            documents: &self.documents,
            active_id: &self.active_id,
            status: &self.status,
        }
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.flush_at
    }
    fn changed(&mut self) {
        self.flush_at
            .get_or_insert_with(|| Instant::now() + Duration::from_millis(500));
    }

    pub fn flush(&mut self) -> Result<(), String> {
        if self.flush_at.is_none() {
            return Ok(());
        }
        if self.recovery_blocked {
            if self.documents.iter().all(|doc| !doc.dirty) {
                self.flush_at = None;
                return Ok(());
            }
            return Err("Draft recovery is unavailable: the previous recovery file needs attention. Save your files explicitly.".into());
        }
        let drafts = self
            .documents
            .iter()
            .filter(|doc| doc.dirty)
            .collect::<Vec<_>>();
        let bytes = serde_json::to_vec(&drafts).map_err(|error| error.to_string())?;
        if bytes.len() > MAX_RECOVERY_BYTES {
            return Err("Editor recovery is full. Save files explicitly before closing.".into());
        }
        let parent = self
            .storage
            .parent()
            .ok_or("Editor recovery directory unavailable")?;
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        let mut staged =
            tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
        staged
            .write_all(&bytes)
            .and_then(|_| staged.as_file().sync_all())
            .map_err(|error| error.to_string())?;
        staged
            .persist(&self.storage)
            .map_err(|error| error.error.to_string())?;
        self.flush_at = None;
        Ok(())
    }

    pub fn flush_due(&mut self) -> bool {
        if self.flush_at.is_some_and(|at| Instant::now() >= at)
            && let Err(error) = self.flush()
        {
            self.status = format!("Draft backup failed: {error}");
            self.flush_at = Some(Instant::now() + Duration::from_secs(5));
            return true;
        }
        false
    }

    pub fn document(&self, id: &str) -> Result<&EditorDocument, String> {
        self.documents
            .iter()
            .find(|doc| doc.id == id)
            .ok_or_else(|| "This editor document is no longer open.".into())
    }

    pub fn open(&mut self, workspace: &WorkspaceRuntime, path: &str) -> Result<(), String> {
        let root = workspace
            .root()
            .ok_or("Project unavailable")?
            .display()
            .to_string();
        if let Some(doc) = self
            .documents
            .iter()
            .find(|doc| doc.file.root == root && doc.file.path == path)
        {
            self.active_id = Some(doc.id.clone());
            return Ok(());
        }
        let file = workspace.read_editor_file(path)?;
        if let Some(doc) = self
            .documents
            .iter()
            .find(|doc| doc.file.root == file.root && doc.file.path == file.path)
        {
            self.active_id = Some(doc.id.clone());
            return Ok(());
        }
        self.check_capacity(file.content.len(), None)?;
        let id = Uuid::new_v4().to_string();
        self.documents.push(EditorDocument {
            id: id.clone(),
            file,
            dirty: false,
            version: 0,
        });
        self.active_id = Some(id);
        self.status.clear();
        Ok(())
    }

    fn check_capacity(&self, bytes: usize, replacing: Option<&str>) -> Result<(), String> {
        let used: usize = self
            .documents
            .iter()
            .filter(|doc| Some(doc.id.as_str()) != replacing)
            .map(|doc| doc.file.content.len())
            .sum();
        if bytes + used > MAX_DRAFT_BYTES {
            return Err("Editor memory is full. Save and close some file tabs first.".into());
        }
        Ok(())
    }

    pub fn change(&mut self, id: &str, content: String, version: u64) -> Result<(), String> {
        self.check_capacity(content.len(), Some(id))?;
        let doc = self
            .documents
            .iter_mut()
            .find(|doc| doc.id == id)
            .ok_or("Document unavailable")?;
        if version <= doc.version {
            return if doc.file.content == content {
                Ok(())
            } else {
                Err("This editor update is stale. The newer draft was retained.".into())
            };
        }
        if doc.file.read_only {
            return Err("This file is read-only.".into());
        }
        let encoded = doc.file.encode(&content)?;
        doc.dirty = format!("{:x}", Sha256::digest(&encoded)) != doc.file.sha256;
        doc.file.content = content;
        doc.version = version;
        self.changed();
        Ok(())
    }

    pub fn save(&mut self, id: &str, workspace: &WorkspaceRuntime) -> Result<(), String> {
        let doc = self.document(id)?.clone();
        self.ensure_root(&doc, workspace)?;
        let file =
            workspace.save_editor_file(&doc.file.path, &doc.file.content, &doc.file.sha256)?;
        let target = self
            .documents
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or("Document unavailable")?;
        target.file = file;
        target.dirty = false;
        self.status = "Saved".into();
        self.changed();
        self.flush()
    }

    pub fn reload(
        &mut self,
        id: &str,
        workspace: &WorkspaceRuntime,
        discard: bool,
    ) -> Result<(), String> {
        let doc = self.document(id)?.clone();
        self.ensure_root(&doc, workspace)?;
        if doc.dirty && !discard {
            return Err("Confirm before discarding unsaved edits.".into());
        }
        let file = workspace.read_editor_file(&doc.file.path)?;
        let target = self
            .documents
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or("Document unavailable")?;
        target.file = file;
        target.dirty = false;
        target.version += 1;
        self.status = "Reloaded from disk".into();
        self.changed();
        self.flush()
    }

    fn ensure_root(
        &self,
        doc: &EditorDocument,
        workspace: &WorkspaceRuntime,
    ) -> Result<(), String> {
        if workspace.root() != Some(Path::new(&doc.file.root)) {
            return Err("The editor cannot retarget a file to another project.".into());
        }
        Ok(())
    }

    pub fn close(&mut self, id: &str, discard: bool) -> Result<(), String> {
        if self.document(id)?.dirty && !discard {
            return Err("Confirm before closing a file with unsaved edits.".into());
        }
        self.documents.retain(|doc| doc.id != id);
        self.selections.remove(id);
        if self.active_id.as_deref() == Some(id) {
            self.active_id = None;
        }
        self.changed();
        self.flush()
    }
}
