//! Owner-scoped, read-only project changes. The WebView never chooses a Git root.
use super::*;
use crate::git_diff::{Entry, Listing, Reader, Section};

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum Action {
    List {},
    Read { path: String, section: Section },
    Close {},
}

#[derive(Default)]
pub(super) struct State {
    views: HashMap<String, View>,
}

struct View {
    view_id: String,
    query: String,
    root: PathBuf,
    entries: Vec<Entry>,
}

impl State {
    fn close(&mut self, owner: &str, view: &str) {
        if self
            .views
            .get(owner)
            .is_some_and(|saved| saved.view_id == view)
        {
            self.views.remove(owner);
        }
    }

    fn matches(&self, reply: &Reply) -> bool {
        self.views.get(&reply.owner).is_some_and(|saved| {
            saved.view_id == reply.view_id
                && saved.query == reply.request_id
                && saved.root == reply.root
        })
    }
}

fn local_diff_root(remote: bool, directory: Option<&Path>) -> Result<PathBuf, String> {
    if remote {
        return Err("/diff inspects local Git changes. SSH nodes cannot use the local project's files; no SSH connection was opened.".into());
    }
    directory.filter(|root| root.is_absolute()).map(Path::to_path_buf)
        .ok_or_else(|| "Choose a node or project with an explicit absolute local directory to inspect its Git changes.".into())
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(super) enum Response {
    List {
        listing: Listing,
    },
    Read {
        path: String,
        section: Section,
        content: String,
    },
}

#[derive(Debug)]
pub(crate) struct Reply {
    owner: String,
    view_id: String,
    request_id: String,
    root: PathBuf,
    result: Result<Response, String>,
}

fn allowed(entries: &[Entry], path: &str, section: Section) -> Result<(), String> {
    let entry = entries
        .iter()
        .find(|entry| entry.path == path && entry.sections.contains(&section))
        .ok_or("Select a changed file from the current project list.")?;
    if let Some(reason) = &entry.blocked {
        return Err(reason.clone());
    }
    Ok(())
}

impl BrowserApp {
    pub(super) fn project_diff_root(&self, owner: &str) -> Result<PathBuf, String> {
        let target = self.conversation_target(owner)?;
        let root = if let Some(node) = target.node_key {
            let binding = self
                .agent_graph_bindings
                .iter()
                .find(|binding| binding.node_key() == node)
                .ok_or("This graph agent is no longer assigned.")?;
            // Non-filesystem Agent Graph nodes may use a project fallback for
            // execution. A filename inspection must not silently borrow it.
            self.agent_graph_node_context(&binding.record_type, &binding.record_id)?
                .project_directory
                .map(PathBuf::from)
        } else {
            target.root
        };
        local_diff_root(target.remote, root.as_deref())
    }

    pub(super) fn request_project_diff(
        &mut self,
        owner: String,
        view_id: String,
        request_id: String,
        action: Action,
    ) {
        let result = (|| -> Result<(), String> {
            // Closing only invalidates this exact view. It must still work if
            // navigation already selected a different chat/provider.
            if matches!(action, Action::Close {}) {
                self.project_diff.close(&owner, &view_id);
                return Ok(());
            }
            let root = self.project_diff_root(&owner)?;
            if matches!(action, Action::List {}) {
                self.project_diff.views.insert(
                    owner.clone(),
                    View {
                        view_id: view_id.clone(),
                        query: request_id.clone(),
                        root: root.clone(),
                        entries: vec![],
                    },
                );
            } else {
                let current = self
                    .project_diff
                    .views
                    .get_mut(&owner)
                    .ok_or("Open /diff in this conversation before reading a file.")?;
                if current.view_id != view_id || current.root != root {
                    return Err("The project changed. Reopen /diff in the intended chat.".into());
                }
                if let Action::Read { path, section } = &action {
                    allowed(&current.entries, path, *section)?;
                }
                current.query = request_id.clone();
            }
            let proxy = self.proxy.clone();
            let (owner, view_id, request_id) = (owner.clone(), view_id.clone(), request_id.clone());
            std::thread::spawn(move || {
                let result = Reader::new(&root).and_then(|reader| match action {
                    Action::List {} => reader.list().map(|listing| Response::List { listing }),
                    Action::Read { path, section } => {
                        reader.read(&path, section).map(|content| Response::Read {
                            path,
                            section,
                            content,
                        })
                    }
                    Action::Close {} => unreachable!(),
                });
                let _ = proxy.send_event(BrowserEvent::ProjectDiff(Reply {
                    owner,
                    view_id,
                    request_id,
                    root,
                    result,
                }));
            });
            Ok(())
        })();
        if let Err(error) = result {
            self.conversation_event(
                "central-agent:project-diff-result",
                json!({"owner":owner,"viewId":view_id,"requestId":request_id,"error":error}),
            );
        }
    }

    pub(super) fn handle_project_diff(&mut self, reply: Reply) {
        if !self.project_diff.matches(&reply) {
            return;
        }
        if self.project_diff_root(&reply.owner).ok().as_ref() != Some(&reply.root) {
            self.project_diff.close(&reply.owner, &reply.view_id);
            self.conversation_event("central-agent:project-diff-result", json!({
                "owner":reply.owner,"viewId":reply.view_id,"requestId":reply.request_id,
                "error":"The conversation's directory or provider changed. Reopen /diff; the previous result was discarded."
            }));
            return;
        }
        let current = self
            .project_diff
            .views
            .get_mut(&reply.owner)
            .expect("matched view");
        current.query.clear();
        let mut detail =
            json!({"owner":reply.owner,"viewId":reply.view_id,"requestId":reply.request_id});
        match reply.result {
            Ok(response) => {
                if let Response::List { listing } = &response {
                    current.entries = listing.entries.clone();
                }
                detail["result"] = json!(response);
            }
            Err(error) => detail["error"] = json!(error),
        }
        self.conversation_event("central-agent:project-diff-result", detail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_changes_require_an_explicit_local_directory_not_an_ssh_or_relative_path() {
        let directory = std::env::temp_dir();
        assert_eq!(local_diff_root(false, Some(&directory)).unwrap(), directory);
        assert!(local_diff_root(true, Some(&directory)).is_err());
        assert!(local_diff_root(true, Some(Path::new("/home/project"))).is_err());
        assert!(local_diff_root(false, None).is_err());
        assert!(local_diff_root(false, Some(Path::new("relative/project"))).is_err());
    }

    #[test]
    fn concurrent_diff_views_do_not_share_lists_or_accept_closed_stale_or_foreign_results() {
        let mut state = State::default();
        let root = std::env::temp_dir();
        for owner in ["chat:main", "graph:node", "graph:conversation:branch"] {
            state.views.insert(
                owner.into(),
                View {
                    view_id: format!("view:{owner}"),
                    query: format!("query:{owner}"),
                    root: root.clone(),
                    entries: vec![Entry {
                        path: format!("{owner}.rs"),
                        icon_key: "rust".into(),
                        sections: vec![Section::Unstaged],
                        blocked: None,
                    }],
                },
            );
        }
        let mut reply = Reply {
            owner: "graph:node".into(),
            view_id: "view:graph:node".into(),
            request_id: "query:graph:node".into(),
            root: root.clone(),
            result: Err("fixture".into()),
        };
        assert!(state.matches(&reply));
        reply.owner = "graph:conversation:branch".into();
        assert!(!state.matches(&reply));
        reply.owner = "graph:node".into();
        reply.root = root.join("different-project");
        assert!(!state.matches(&reply));
        reply.root = root;
        state.views.get_mut("graph:node").unwrap().query = "newer-read".into();
        assert!(!state.matches(&reply));
        reply.request_id = "newer-read".into();
        assert!(state.matches(&reply));
        assert!(
            allowed(
                &state.views["graph:node"].entries,
                "chat:main.rs",
                Section::Unstaged
            )
            .is_err()
        );
        state.close("graph:node", "old-view");
        assert!(state.matches(&reply));
        state.close("graph:node", "view:graph:node");
        assert!(!state.matches(&reply));
        assert_eq!(state.views.len(), 2);
        assert_eq!(
            state.views["graph:conversation:branch"].entries[0].path,
            "graph:conversation:branch.rs"
        );
        assert_eq!(state.views["chat:main"].query, "query:chat:main");
    }

    #[test]
    fn diff_ipc_cannot_select_a_root_command_or_unlisted_file() {
        let request = json!({"message_type":"project_diff","owner":"chat:one","view_id":"view","request_id":"query","action":{"kind":"list"}});
        assert!(serde_json::from_value::<AgentPanelMessage>(request.clone()).is_ok());
        for key in ["root", "command", "threadId"] {
            let mut invalid = request.clone();
            invalid["action"][key] = json!("injected");
            assert!(serde_json::from_value::<AgentPanelMessage>(invalid).is_err());
        }
        let entries = vec![
            Entry {
                path: "a.rs".into(),
                icon_key: "rust".into(),
                sections: vec![Section::Staged],
                blocked: None,
            },
            Entry {
                path: ".env".into(),
                icon_key: "file".into(),
                sections: vec![Section::Untracked],
                blocked: Some("Secret".into()),
            },
        ];
        assert!(allowed(&entries, "a.rs", Section::Staged).is_ok());
        assert!(allowed(&entries, "a.rs", Section::Untracked).is_err());
        assert!(allowed(&entries, "../foreign", Section::Staged).is_err());
        assert!(allowed(&entries, ".env", Section::Untracked).is_err());
    }
}
