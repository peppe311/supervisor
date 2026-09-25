//! Local unsent text, never Codex history, queued input or credentials.
use super::*;
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Action {
    Read,
    Write { revision: String, text: String },
}
impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Read => "Read",
            Self::Write { .. } => "Write([redacted])",
        })
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    owner: String,
    revision: String,
    text: String,
}

fn path(root: &Path, owner: &str) -> PathBuf {
    root.join(format!("{:x}.json", Sha256::digest(owner.as_bytes())))
}

fn access(root: &Path, owner: &str, action: Action) -> Result<Record, String> {
    fs::create_dir_all(root).map_err(|_| "Local drafts directory is unavailable")?;
    let target = path(root, owner);
    // A file lock makes the read/version/write sequence exclusive even if two
    // Supervisor processes share the same local profile. Never wait on UI.
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(target.with_extension("lock"))
        .map_err(|_| "Local draft lock is unavailable")?;
    lock.try_lock()
        .map_err(|_| "Another window is saving this draft. Your text remains here.")?;
    let current = match fs::read(&target) {
        Ok(bytes) => {
            let record: Record = serde_json::from_slice(&bytes)
                .map_err(|_| "The saved draft is damaged; it was not overwritten")?;
            if record.owner != owner {
                return Err("The saved draft belongs to another conversation".into());
            }
            record
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Record {
            owner: owner.into(),
            ..Record::default()
        },
        Err(_) => return Err("The saved draft could not be read; it was not overwritten".into()),
    };
    match action {
        Action::Read => Ok(current),
        Action::Write { revision, text } => {
            if revision != current.revision {
                return Err("This draft changed in another window. Copy your text before closing; the saved draft was not overwritten.".into());
            }
            let next = Record {
                owner: owner.into(),
                revision: Uuid::new_v4().to_string(),
                text,
            };
            let bytes = serde_json::to_vec(&next).map_err(|_| "The draft could not be encoded")?;
            save(&target, &bytes)
                .map_err(|_| "The draft could not be saved. Copy your text before closing.")?;
            Ok(next)
        }
    }
}

fn save(target: &Path, bytes: &[u8]) -> io::Result<()> {
    let temporary = target.with_extension(format!("{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        if target.exists() {
            replace_file(target, &temporary)
        } else {
            fs::rename(&temporary, target)
        }
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

/// Seed only a newly allocated destination. A draft created in another
/// window wins; a handoff must never replace it.
fn seed(root: &Path, owner: &str, text: &str) -> Result<(), String> {
    let current = access(root, owner, Action::Read)?;
    if !current.revision.is_empty() || !current.text.is_empty() {
        return Err("The new agent already has a draft; it was retained.".into());
    }
    access(
        root,
        owner,
        Action::Write {
            revision: current.revision,
            text: text.into(),
        },
    )?;
    Ok(())
}

impl BrowserApp {
    pub(super) fn seed_work_draft(&self, owner: &str, text: &str) -> Result<(), String> {
        seed(&self.data_dir.join("composer-drafts"), owner, text)
    }
    pub(super) fn forget_composer_draft(&self, owner: &str) -> Result<(), String> {
        let root = self.data_dir.join("composer-drafts");
        let current = access(&root, owner, Action::Read)?;
        let cleared = access(
            &root,
            owner,
            Action::Write {
                revision: current.revision,
                text: String::new(),
            },
        )?;
        self.conversation_event(
            "central-agent:composer-draft-forgotten",
            json!({"owner":owner,"revision":cleared.revision}),
        );
        Ok(())
    }
    pub(super) fn composer_draft(&self, owner: &str, request_id: &str, action: Action) {
        if Uuid::parse_str(request_id).is_err() {
            return;
        }
        let known = owner.strip_prefix("draft:").is_some_and(|root| {
            root.is_empty()
                || self
                    .workspace
                    .project_roots()
                    .iter()
                    .chain(self.workspace.archived_project_roots())
                    .any(|path| path.display().to_string() == root)
        }) || owner
            .strip_prefix("chat:")
            .is_some_and(|id| self.project_chats.iter().any(|chat| chat.id == id))
            || owner.strip_prefix("graph:").is_some_and(|key| {
                self.agent_graph_bindings
                    .iter()
                    .any(|b| b.node_key() == key)
            });
        let read = matches!(action, Action::Read);
        let result = if known {
            access(&self.data_dir.join("composer-drafts"), owner, action)
        } else {
            Err("This conversation no longer exists; its draft was not saved".into())
        };
        let payload = match result {
            Ok(record) => {
                json!({"owner":owner,"requestId":request_id,"revision":record.revision,"text":read.then_some(record.text)})
            }
            Err(error) => json!({"owner":owner,"requestId":request_id,"error":error}),
        };
        self.conversation_event("central-agent:composer-draft-result", payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handoff_draft_is_durable_and_never_replaces_an_existing_draft() {
        let temp = tempfile::tempdir().unwrap();
        seed(
            temp.path(),
            "chat:new",
            "Continue from the verified checkpoint.",
        )
        .unwrap();
        assert_eq!(
            access(temp.path(), "chat:new", Action::Read).unwrap().text,
            "Continue from the verified checkpoint."
        );
        assert!(seed(temp.path(), "chat:new", "Replacement").is_err());
        assert_eq!(
            access(temp.path(), "chat:new", Action::Read).unwrap().text,
            "Continue from the verified checkpoint."
        );
        assert!(
            access(temp.path(), "chat:source", Action::Read)
                .unwrap()
                .text
                .is_empty()
        );
    }
    #[test]
    fn saved_text_survives_reopen_and_is_isolated_by_conversation() {
        let temp = tempfile::tempdir().unwrap();
        for owner in ["chat:a", "graph:node", "draft:C:\\fixture"] {
            let initial = access(temp.path(), owner, Action::Read).unwrap();
            assert!(initial.text.is_empty());
            let text = format!("Unsent {owner}\nItaliano: perché 🦀");
            let saved = access(
                temp.path(),
                owner,
                Action::Write {
                    revision: initial.revision,
                    text: text.clone(),
                },
            )
            .unwrap();
            let reopened = access(temp.path(), owner, Action::Read).unwrap();
            assert_eq!(reopened.text, text);
            assert_eq!(reopened.revision, saved.revision);
        }
    }
    #[test]
    fn old_writes_and_clears_cannot_overwrite_a_newer_draft() {
        let temp = tempfile::tempdir().unwrap();
        let first = access(
            temp.path(),
            "chat:a",
            Action::Write {
                revision: String::new(),
                text: "First".into(),
            },
        )
        .unwrap();
        let next = access(
            temp.path(),
            "chat:a",
            Action::Write {
                revision: first.revision.clone(),
                text: "Newer".into(),
            },
        )
        .unwrap();
        assert!(
            access(
                temp.path(),
                "chat:a",
                Action::Write {
                    revision: first.revision,
                    text: String::new()
                }
            )
            .is_err()
        );
        assert_eq!(
            access(temp.path(), "chat:a", Action::Read).unwrap().text,
            "Newer"
        );
        access(
            temp.path(),
            "chat:a",
            Action::Write {
                revision: next.revision,
                text: String::new(),
            },
        )
        .unwrap();
        assert!(
            access(temp.path(), "chat:a", Action::Read)
                .unwrap()
                .text
                .is_empty()
        );
    }
    #[test]
    fn corrupt_records_are_preserved_and_debug_never_contains_draft_text() {
        let temp = tempfile::tempdir().unwrap();
        let target = path(temp.path(), "chat:a");
        fs::write(&target, b"unreadable record").unwrap();
        let action = Action::Write {
            revision: String::new(),
            text: "PRIVATE DRAFT".into(),
        };
        assert!(!format!("{action:?}").contains("PRIVATE"));
        assert!(access(temp.path(), "chat:a", action).is_err());
        assert_eq!(fs::read(target).unwrap(), b"unreadable record");
    }

    #[test]
    fn a_held_cross_process_lock_never_blocks_or_overwrites_a_draft() {
        let temp = tempfile::tempdir().unwrap();
        let saved = access(
            temp.path(),
            "chat:a",
            Action::Write {
                revision: String::new(),
                text: "Keep".into(),
            },
        )
        .unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path(temp.path(), "chat:a").with_extension("lock"))
            .unwrap();
        lock.try_lock().unwrap();
        assert!(
            access(
                temp.path(),
                "chat:a",
                Action::Write {
                    revision: saved.revision,
                    text: "Rejected".into()
                }
            )
            .is_err()
        );
        drop(lock);
        assert_eq!(
            access(temp.path(), "chat:a", Action::Read).unwrap().text,
            "Keep"
        );
    }
}
