//! Explicit board operations. Rust validates roots; UI navigation grants no authority.
use super::*;

impl BrowserApp {
    fn connected_board_root(&self, root: &str) -> Result<PathBuf, String> {
        let path = fs::canonicalize(root).map_err(|error| error.to_string())?;
        if !self
            .workspace
            .project_roots()
            .iter()
            .any(|known| fs::canonicalize(known).is_ok_and(|known| known == path))
        {
            return Err("Reconnect this local project before using it.".into());
        }
        Ok(path)
    }
    pub(super) fn create_board_worktree(&mut self, root: &str, name: &str) -> Result<(), String> {
        let root = self.connected_board_root(root)?;
        if self.project_import.active {
            return Err("Wait for the current project operation to finish.".into());
        }
        crate::project_registry::validate_new_project_name(name)?;
        let name = name.trim().to_owned();
        let Some(parent) = rfd::FileDialog::new()
            .set_title("Choose the parent folder for the isolated task")
            .set_directory(root.parent().unwrap_or(&root))
            .pick_folder()
        else {
            return Ok(());
        };
        let operation_id = Uuid::new_v4().to_string();
        self.project_import_operation_id = Some(operation_id.clone());
        self.project_import = ProjectImportView {
            active: true,
            kind: Some("worktree".into()),
            message: Some("Creating isolated task…".into()),
            error: false,
        };
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = crate::project_registry::create_task_worktree(&root, &parent, &name);
            let _ = proxy.send_event(BrowserEvent::BoardWorktreeCompleted {
                operation_id,
                name,
                result,
            });
        });
        Ok(())
    }
    pub(super) fn finish_board_worktree(
        &mut self,
        operation_id: &str,
        name: &str,
        result: Result<PathBuf, String>,
    ) {
        if self.project_import_operation_id.as_deref() != Some(operation_id) {
            return;
        }
        self.project_import_operation_id = None;
        self.project_import.active = false;
        match result {
            Ok(path) => {
                self.connect_workspace_path(path.clone());
                if self.workspace.root() != Some(path.as_path()) {
                    return;
                }
                let root = path.to_string_lossy().into_owned();
                self.workspace_project_labels
                    .insert(root.clone(), format!("{name} · worktree"));
                if let Some(chat_id) = self.active_project_chat_id.clone() {
                    self.rename_project_chat(&chat_id, name);
                    self.project_board_open_chat_ids = vec![chat_id];
                }
                self.save_session();
                self.conversation_event(
                    "central-agent:board-worktree-ready",
                    json!({"owner":"graph:project-board","root":root}),
                );
            }
            Err(error) => {
                self.project_import = ProjectImportView {
                    active: false,
                    kind: Some("worktree".into()),
                    message: Some(error.clone()),
                    error: true,
                };
                self.conversation_event(
                    "central-agent:project-board-error",
                    json!({"owner":"graph:project-board","error":error}),
                );
            }
        }
        self.render_agent_graph_surface();
    }
}
