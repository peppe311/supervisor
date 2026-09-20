//! File snapshots belong to a graph draft, never the selected main composer.
use super::*;

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Action {
    Select {},
    Remove { id: String },
}

#[derive(Default)]
pub(super) struct State {
    drafts: HashMap<String, Vec<PendingFileAttachment>>,
}

impl State {
    /// Consume paths explicitly returned by the local picker, retaining the owner.
    pub(super) fn add_paths(&mut self, owner: &str, paths: Vec<PathBuf>) -> Result<(), String> {
        let files = self.drafts.entry(owner.into()).or_default();
        let mut errors = Vec::new();
        for path in paths {
            if files.len() >= MAX_FILE_ATTACHMENTS {
                errors.push(format!(
                    "Only {MAX_FILE_ATTACHMENTS} files can be attached at once."
                ));
                break;
            }
            match PendingFileAttachment::load(&path) {
                Ok(file) => files.push(file),
                Err(error) => errors.push(error),
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("\n"))
        }
    }
    pub(super) fn forget(&mut self, owner: &str) {
        self.drafts.remove(owner);
    }
    pub(super) fn views(&self, owner: &str) -> Vec<FileAttachmentView> {
        self.drafts
            .get(owner)
            .into_iter()
            .flatten()
            .map(PendingFileAttachment::view)
            .collect()
    }
    pub(super) fn has_draft(&self, owner: &str) -> bool {
        self.drafts
            .get(owner)
            .is_some_and(|files| !files.is_empty())
    }

    pub(super) fn capture(
        &self,
        owner: &str,
        ids: &[String],
    ) -> Result<Vec<PendingFileAttachment>, String> {
        if ids.len() > MAX_FILE_ATTACHMENTS {
            return Err(format!(
                "You can attach at most {MAX_FILE_ATTACHMENTS} files."
            ));
        }
        let mut files = Vec::new();
        for id in ids {
            if files
                .iter()
                .any(|file: &PendingFileAttachment| file.id() == id)
            {
                return Err(
                    "The same file attachment was submitted twice. Refresh the node's draft."
                        .into(),
                );
            }
            let file = self
                .drafts
                .get(owner)
                .into_iter()
                .flatten()
                .find(|file| file.id() == id)
                .ok_or(
                    "A file no longer belongs to this node's draft. Reattach it before sending.",
                )?;
            files.push(file.clone());
        }
        Ok(files)
    }

    pub(super) fn consume(&mut self, owner: &str, accepted: &[PendingFileAttachment]) {
        if let Some(files) = self.drafts.get_mut(owner) {
            files.retain(|file| !accepted.iter().any(|sent| sent.id() == file.id()));
            if files.is_empty() {
                self.drafts.remove(owner);
            }
        }
    }
}

impl BrowserApp {
    fn validate_graph_file_owner(&self, owner: &str) -> Result<(), String> {
        if let Some(node) = owner.strip_prefix("graph:") {
            if !self
                .agent_graph_bindings
                .iter()
                .any(|binding| binding.node_key() == node)
            {
                return Err("This node is no longer assigned. Reopen its agent.".into());
            }
        } else if !self.is_project_chat_card_owner(owner) {
            return Err("Reopen this project conversation before attaching files.".into());
        }
        Ok(())
    }

    pub(super) fn manage_graph_files(&mut self, owner: &str, action: Action) {
        let result = (|| -> Result<(), String> {
            self.validate_graph_file_owner(owner)?;
            match action {
                Action::Remove { id } => {
                    if let Some(files) = self.graph_files.drafts.get_mut(owner) {
                        files.retain(|file| file.id() != id);
                    }
                }
                Action::Select {} => {
                    if !self.native_files_enabled(owner) {
                        return Err("Connect Codex and select a local node to attach files.".into());
                    }
                    self.conversation_target(owner)?;
                    if self.graph_files.views(owner).len() >= MAX_FILE_ATTACHMENTS {
                        return Err(format!(
                            "You can attach at most {MAX_FILE_ATTACHMENTS} files."
                        ));
                    }
                    let Some(paths) = Self::file_attachment_dialog().pick_files() else {
                        return Ok(());
                    };
                    // Keep the captured owner, never the current main chat or hovered node.
                    self.conversation_target(owner)?;
                    self.graph_files.add_paths(owner, paths)?;
                }
            }
            Ok(())
        })();
        self.conversation_event(
            "central-agent:graph-files-result",
            json!({"owner":owner,"error":result.err(),"files":self.graph_files.views(owner)}),
        );
        self.render_agent_graph_surface();
    }

    pub(super) fn add_graph_file_paths(&mut self, owner: &str, paths: Vec<PathBuf>) {
        let result = (|| -> Result<(), String> {
            self.validate_graph_file_owner(owner)?;
            if !self.native_files_enabled(owner) {
                return Err(
                    "Connect Codex and select a local conversation to attach files.".into(),
                );
            }
            self.conversation_target(owner)?;
            if self.graph_files.views(owner).len() >= MAX_FILE_ATTACHMENTS {
                return Err(format!(
                    "You can attach at most {MAX_FILE_ATTACHMENTS} files."
                ));
            }
            self.graph_files.add_paths(owner, paths)
        })();
        self.conversation_event(
            "central-agent:graph-files-result",
            json!({"owner":owner,"error":result.err(),"files":self.graph_files.views(owner)}),
        );
        self.render_agent_graph_surface();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picker_result_keeps_valid_files_without_accepting_secrets_or_other_owners() {
        let directory = tempfile::tempdir().unwrap();
        let note = directory.path().join("note.txt");
        let secret = directory.path().join(".env");
        fs::write(&note, "selected fixture").unwrap();
        // This verifies the prohibited filename, not a credential-shaped value.
        fs::write(&secret, "public example data").unwrap();
        let mut state = State::default();
        assert!(
            state
                .add_paths("graph:a", vec![note.clone(), secret])
                .is_err()
        );
        let selected = state.views("graph:a");
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "note.txt");
        assert!(state.views("graph:b").is_empty());
        assert!(state.capture("graph:b", &[selected[0].id.clone()]).is_err());
        for _ in 1..MAX_FILE_ATTACHMENTS {
            state.add_paths("graph:a", vec![note.clone()]).unwrap();
        }
        assert!(state.add_paths("graph:a", vec![note]).is_err());
        assert_eq!(state.views("graph:a").len(), MAX_FILE_ATTACHMENTS);
    }

    #[test]
    fn graph_file_snapshots_are_owner_bound_and_survive_queue_serialization_and_new_drafts() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Cargo.toml");
        fs::write(&path, "[package]\nname = \"snapshot\"\n").unwrap();
        let a = PendingFileAttachment::load(&path).unwrap();
        let b = PendingFileAttachment::load(&path).unwrap();
        let a_id = a.id().to_owned();
        let b_id = b.id().to_owned();
        let mut state = State {
            drafts: HashMap::from([("graph:a".into(), vec![a]), ("graph:b".into(), vec![b])]),
        };
        assert!(
            state
                .capture("graph:a", std::slice::from_ref(&b_id))
                .is_err()
        );
        assert!(
            state
                .capture("chat:main", std::slice::from_ref(&a_id))
                .is_err()
        );
        assert!(
            state
                .capture("graph:a", &[a_id.clone(), a_id.clone()])
                .is_err()
        );
        assert!(
            state
                .capture("graph:a", &vec![a_id.clone(); MAX_FILE_ATTACHMENTS + 1])
                .is_err()
        );
        let accepted = SubmissionSnapshots {
            files: state
                .capture("graph:a", std::slice::from_ref(&a_id))
                .unwrap(),
            ..Default::default()
        };
        let stored = serde_json::to_vec(&accepted).unwrap();
        fs::write(&path, "new manual version").unwrap();
        let newer = PendingFileAttachment::load(&path).unwrap();
        let newer_id = newer.id().to_owned();
        state.drafts.get_mut("graph:a").unwrap().push(newer);
        let recovered: SubmissionSnapshots = serde_json::from_slice(&stored).unwrap();
        assert!(
            matches!(recovered.files[0].input().content, crate::file_attachment::AgentFileContent::Text(text) if text.contains("snapshot"))
        );
        state.consume("graph:a", &recovered.files);
        assert_eq!(state.views("graph:a")[0].id, newer_id);
        assert_eq!(state.views("graph:a")[0].icon_key, "cargo");
        assert_eq!(state.views("graph:b")[0].id, b_id);
        assert_eq!(state.views("graph:a").len(), 1);
        state.forget("graph:a");
        assert!(state.views("graph:a").is_empty());
        assert_eq!(state.views("graph:b").len(), 1);
        assert_eq!(recovered.files[0].id(), a_id);
    }

    #[test]
    fn graph_file_intents_do_not_accept_paths_or_file_contents_from_the_webview() {
        assert!(serde_json::from_value::<Action>(json!({"kind":"select"})).is_ok());
        assert!(
            serde_json::from_value::<Action>(json!({"kind":"select","path":"C:/secret.txt"}))
                .is_err()
        );
        assert!(serde_json::from_value::<Action>(json!({"kind":"remove","id":"known"})).is_ok());
        assert!(
            serde_json::from_value::<Action>(json!({"kind":"remove","id":"known","bytes":"fake"}))
                .is_err()
        );
        let parsed: AgentPanelMessage = serde_json::from_value(json!({"message_type":"continue_knowledge_agent","record_type":"entity","id":"a","message":"","file_ids":["file-a"],"delivery":"queue"})).unwrap();
        assert!(
            matches!(parsed, AgentPanelMessage::ContinueAgentGraphAgent {file_ids, delivery:AgentSubmissionDelivery::Queue, ..} if file_ids == ["file-a"])
        );
    }
}
