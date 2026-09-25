//! Provider-independent routing for graph controls and local project views.
use super::*;
pub(super) struct Target {
    pub node_key: Option<String>,
    pub root: Option<PathBuf>,
    pub remote: bool,
}
impl BrowserApp {
    pub(super) fn acknowledge_submission(&mut self, submission: &PendingAgentSubmission) {
        if let Some(owner) = submission.owner() {
            self.app_server
                .consume_skills(owner, &submission.native_skills);
            self.app_server.consume_apps(owner, &submission.native_apps);
            self.emit_app_server_skills(owner, None);
            self.emit_app_server_conversation(owner, "apps_consumed");
        }
        self.consume_accepted_attachments(submission);
        self.render_agent_panel();
        self.conversation_event("central-agent:submission-accepted", json!({
            "owner":submission.owner(),
            "input":submission.agent_graph_launch.as_ref().map(|launch| launch.user_request.as_str()).unwrap_or(&submission.message)
        }));
    }
    pub(super) fn conversation_state(&self, owner: &str) -> Value {
        json!({"owner":owner,"active":self.owner_has_running_work(owner),
            "liveDiff":self.live_diff_for_owner(owner),
            "nativeAccess":self.app_server_access_view(owner),
            "nativeMessages":self.app_server_messages(owner),
            "nativeUsage":self.app_server_usage(owner),
            "nativeBinding":self.app_server.conversations.binding(owner),
            "nativeConversation":self.app_server_conversation_view(owner),
            "nativeLoaded":self.app_server.conversations.binding(owner).is_some_and(|b|self.app_server.conversations.mirror.thread(&b.thread_id).is_some()),
            "unresolved":self.app_server.delivery_warning(owner),
            "nativeRequests":self.app_server_request_views(owner),
            "busy":self.conversation_busy(owner),
            "supportsSteer":self.native_access_selected(owner) && self.app_server.steer_target(owner).is_some(),
            "nativeTurnId":self.app_server.conversations.active_turn(owner),
            "fileAttachments":self.graph_files.views(owner),"contextAttachments":self.graph_context_state(owner),"enabled":self.native_files_enabled(owner),
            "queued":self.agent_submission_queue.iter().filter(|s|s.owner()==Some(owner)).count()})
    }

    pub(super) fn live_diff_for_owner(&self, owner: &str) -> Option<LiveDiffView> {
        let native_selected = owner.strip_prefix("graph:").map_or_else(
            || {
                owner
                    .strip_prefix("chat:")
                    .and_then(|chat_id| self.project_chat_card_profiles.get(chat_id))
                    .map_or(
                        self.agent_provider == AgentProviderKind::CodexAppServer,
                        |profile| profile.provider == AgentProviderKind::CodexAppServer,
                    )
            },
            |node_key| {
                self.agent_graph_bindings.iter().any(|binding| {
                    binding.node_key() == node_key
                        && binding.provider == AgentProviderKind::CodexAppServer
                })
            },
        );
        if native_selected || self.app_server.busy(owner) {
            return self.native_live_diff(owner);
        }
        self.live_diff_by_owner
            .get(owner)
            .map(|tracked| tracked.view)
    }

    fn native_live_diff(&self, owner: &str) -> Option<LiveDiffView> {
        let binding = self.app_server.conversations.binding(owner)?;
        let thread = self
            .app_server
            .conversations
            .mirror
            .thread(&binding.thread_id)?;
        let busy = self.app_server.busy(owner);
        let turn = match thread.active_turn() {
            Some(turn) => turn,
            None if busy => {
                return Some(LiveDiffView {
                    additions: 0,
                    deletions: 0,
                    file_count: 0,
                    active: true,
                });
            }
            None => thread.turns.last()?,
        };
        let (additions, deletions, file_count, observed) = native_turn_diff_stats(turn);
        let active = busy || turn.active();
        (active || observed).then_some(LiveDiffView {
            additions,
            deletions,
            file_count,
            active,
        })
    }
    pub(super) fn conversation_target(&self, owner: &str) -> Result<Target, String> {
        if let Some(node) = owner.strip_prefix("graph:") {
            let binding = self
                .agent_graph_bindings
                .iter()
                .find(|binding| binding.node_key() == node)
                .ok_or("This graph agent is no longer assigned. Reopen its node.")?;

            let context =
                self.agent_graph_node_context(&binding.record_type, &binding.record_id)?;
            let remote = context.ssh_profile_id.is_some();
            let root = control_root(
                remote,
                context.project_directory.as_deref(),
                self.workspace.root(),
            );
            return Ok(Target {
                node_key: Some(node.into()),
                root,
                remote,
            });
        }
        if let Some(chat_id) = owner.strip_prefix("chat:") {
            let chat = self
                .project_chats
                .iter()
                .find(|chat| chat.id == chat_id && !chat.archived)
                .ok_or("This project conversation is no longer available.")?;
            return Ok(Target {
                node_key: None,
                root: (!chat.project_root.is_empty()).then(|| PathBuf::from(&chat.project_root)),
                remote: false,
            });
        }
        if owner != self.composer_owner() {
            return Err(
                "The selected conversation changed. Reopen the control in the intended chat."
                    .into(),
            );
        }
        Ok(Target {
            node_key: None,
            root: self.workspace.root().map(Path::to_path_buf),
            remote: false,
        })
    }

    pub(super) fn composer_owner(&self) -> String {
        self.active_project_chat_id
            .as_ref()
            .map(|id| format!("chat:{id}"))
            .unwrap_or_else(|| {
                format!(
                    "draft:{}",
                    self.workspace
                        .root()
                        .map(|root| root.display().to_string())
                        .unwrap_or_else(|| "local".to_owned())
                )
            })
    }
    pub(super) fn conversation_event(&self, event: &str, detail: Value) {
        let owner = detail
            .get("owner")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let graph_owner = owner.starts_with("graph:");
        let project_card_owner = self.is_project_chat_card_owner(owner);
        let supervised_owner = self.is_supervised_project_chat_owner(owner);
        let board_history_update = event == "central-agent:app-server-conversation"
            && owner.strip_prefix("chat:").is_some_and(|id| {
                self.project_chats
                    .iter()
                    .any(|chat| chat.id == id && !chat.archived)
            });
        let main_owner = !graph_owner && (!project_card_owner || owner == self.composer_owner());
        let script = format!(
            "window.dispatchEvent(new CustomEvent({}, {{ detail: {} }}));",
            json!(event),
            detail
        );
        for panel in [
            (main_owner).then_some(self.agent_panel.as_ref()).flatten(),
            (graph_owner || project_card_owner || supervised_owner || board_history_update)
                .then_some(self.agent_graph_surface.as_ref())
                .flatten(),
        ]
        .into_iter()
        .flatten()
        {
            if let Err(error) = panel.evaluate_script(&script) {
                warn!(%error, "conversation event could not reach the composer");
            }
        }
    }
    pub(super) fn conversation_busy(&self, owner: &str) -> bool {
        self.owner_has_unfinished_work(owner)
            || self
                .pending_agent_submissions
                .iter()
                .chain(self.agent_submission_queue.iter())
                .any(|s| s.owner() == Some(owner))
    }
}

fn native_turn_diff_stats(
    turn: &central_agent_codex_runtime::mirror::Turn,
) -> (usize, usize, usize, bool) {
    let mut paths = HashSet::<String>::new();
    let mut item_diffs = Vec::new();
    for item in &turn.items {
        if item.value["type"] != "fileChange"
            || matches!(item.value["status"].as_str(), Some("declined" | "failed"))
        {
            continue;
        }
        for change in item.value["changes"].as_array().into_iter().flatten() {
            if let Some(path) = change["path"].as_str().filter(|path| !path.is_empty()) {
                paths.insert(path.to_owned());
            }
            if let Some(diff) = change["diff"].as_str().filter(|diff| !diff.is_empty()) {
                item_diffs.push(diff);
            }
        }
    }
    let aggregate = turn.diff.as_deref().filter(|diff| !diff.is_empty());
    let mut additions = 0_usize;
    let mut deletions = 0_usize;
    if let Some(diff) = aggregate {
        (additions, deletions) = unified_diff_line_counts(diff);
        if paths.is_empty() {
            paths.extend(
                diff.lines()
                    .filter_map(|line| line.strip_prefix("diff --git "))
                    .map(str::to_owned),
            );
        }
    } else {
        for diff in &item_diffs {
            let (added, removed) = unified_diff_line_counts(diff);
            additions = additions.saturating_add(added);
            deletions = deletions.saturating_add(removed);
        }
    }
    let observed = aggregate.is_some() || !item_diffs.is_empty() || !paths.is_empty();
    (additions, deletions, paths.len(), observed)
}

fn unified_diff_line_counts(diff: &str) -> (usize, usize) {
    let mut additions = 0_usize;
    let mut deletions = 0_usize;
    let structured = diff.lines().any(|line| line.starts_with("@@"));
    let mut in_hunk = !structured;
    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            in_hunk = !structured;
            continue;
        }
        if line.starts_with("@@") {
            in_hunk = true;
            continue;
        }
        if !in_hunk || (!structured && (line.starts_with("+++ ") || line.starts_with("--- "))) {
            continue;
        }
        if line.starts_with('+') {
            additions = additions.saturating_add(1);
        } else if line.starts_with('-') {
            deletions = deletions.saturating_add(1);
        }
    }
    (additions, deletions)
}
fn control_root(remote: bool, directory: Option<&str>, fallback: Option<&Path>) -> Option<PathBuf> {
    if remote {
        None
    } else {
        directory
            .map(PathBuf::from)
            .or_else(|| fallback.map(Path::to_path_buf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_diff_counts_content_lines_without_headers() {
        assert_eq!(
            unified_diff_line_counts(
                "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1,3 @@\n-old\n+new\n+more\n+++ literal code\n context\n"
            ),
            (3, 1)
        );
    }

    #[test]
    fn native_turn_prefers_aggregate_diff_and_deduplicates_file_paths() {
        let mut mirror = central_agent_codex_runtime::mirror::Mirror::default();
        mirror
            .notify(
                "item/fileChange/patchUpdated",
                &json!({"threadId":"thread","turnId":"turn","itemId":"patch","changes":[
                    {"path":"src/main.rs","diff":"-old\n+new\n"},
                    {"path":"src/main.rs","diff":"+second\n"}
                ]}),
            )
            .unwrap();
        let turn = &mirror.thread("thread").unwrap().turns[0];
        assert_eq!(native_turn_diff_stats(turn), (2, 1, 1, true));

        mirror
            .notify(
                "turn/diff/updated",
                &json!({"threadId":"thread","turnId":"turn","diff":"--- a/src/main.rs\n+++ b/src/main.rs\n-old\n+new\n"}),
            )
            .unwrap();
        let turn = &mirror.thread("thread").unwrap().turns[0];
        assert_eq!(native_turn_diff_stats(turn), (1, 1, 1, true));
    }
}
