//! Native thread lifecycle controls. No filesystem checkpoint or local-history
//! replacement is implied by archiving, restoring or deleting a Codex thread.
use super::*;
use central_agent_codex_runtime::conversations::Binding;

impl ConversationAction {
    pub(super) fn validate_target(&self, binding: Option<&Binding>) -> Result<(), String> {
        if let Self::AppsControl {
            expected_thread_id, ..
        } = self
        {
            let current = binding.filter(|binding| !binding.deleted);
            let valid = match (expected_thread_id.as_deref(), current) {
                (None, None) => true,
                (Some(expected), Some(binding)) => {
                    !expected.is_empty() && binding.thread_id == expected
                }
                _ => false,
            };
            return valid.then_some(()).ok_or_else(|| {
                "The native conversation changed. Reopen its controls before continuing.".into()
            });
        }
        let expected = match self {
            Self::Rename {
                expected_thread_id, ..
            }
            | Self::McpStatus { expected_thread_id }
            | Self::McpControl {
                expected_thread_id, ..
            }
            | Self::HooksStatus {
                expected_thread_id, ..
            }
            | Self::Goal {
                expected_thread_id, ..
            }
            | Self::StartReview {
                expected_thread_id, ..
            }
            | Self::ResetUnused { expected_thread_id }
            | Self::Compact { expected_thread_id }
            | Self::Archive { expected_thread_id }
            | Self::Unarchive { expected_thread_id }
            | Self::NewFork {
                expected_thread_id, ..
            }
            | Self::OpenDelegate {
                expected_thread_id, ..
            }
            | Self::EventLog { expected_thread_id }
            | Self::ReadWork {
                expected_thread_id, ..
            }
            | Self::Delete { expected_thread_id } => expected_thread_id,
            _ => return Ok(()),
        };
        if expected.is_empty()
            || binding.is_none_or(|b| {
                &b.thread_id != expected || b.deleted && !matches!(self, Self::EventLog { .. })
            })
        {
            return Err(
                "The native conversation changed. Reopen its controls before continuing.".into(),
            );
        }
        Ok(())
    }
}

impl BrowserApp {
    pub(in crate::browser) fn app_server_conversation_view(&self, owner: &str) -> Value {
        let book = &self.app_server.conversations;
        let binding = book.binding(owner);
        let thread = binding.and_then(|b| book.mirror.thread(&b.thread_id));
        let apps = if let Some(binding) = binding.filter(|binding| !binding.deleted) {
            self.app_server.apps.view(owner, Some(&binding.thread_id))
        } else if binding.is_none() {
            self.app_server.apps.view(owner, None)
        } else {
            None
        };
        json!({
            "visible":self.native_access_selected(owner),
            "connected":self.app_server.view.connected && self.app_server.binding_store_error.is_none(),
            "binding":binding,
            "name":thread.and_then(|t| t.name.as_deref()),
            "historyMode":thread.and_then(|t| t.history_mode.as_deref()),
            "completedTurns":thread.map(|thread| thread.turns.iter().filter(|turn| !turn.active()).map(|turn| json!({"id":turn.id,"status":turn.status})).collect::<Vec<_>>()).unwrap_or_default(),
            "nativeLoaded":book.is_thread_loaded(owner),
            "threadSettings":thread.and_then(|t| t.settings.as_ref()),
            "threadSettingsCurrent":self.app_server.view.connected && book.is_thread_loaded(owner) && thread.is_some_and(|t| t.settings_current),
            "goal":binding.filter(|b| !b.deleted).and_then(|b| self.app_server.goals.view(owner, &b.thread_id)),
            "mcp":binding.filter(|b| !b.deleted).and_then(|b| self.app_server.thread_mcp.view(owner, &b.thread_id)),
            "apps":apps,
            "hooks":binding.filter(|b| !b.deleted).and_then(|b| self.app_server.hooks.view(owner, &b.thread_id)),
            "directory":self.conversation_target(owner).ok().and_then(|target|target.root).map(|root|display_path(&root)),
            "importAvailable":owner.starts_with("chat:") || owner.starts_with("graph:"),
            "pendingFork":book.saved().fork_origins.get(owner),
            "unusedLink":book.unused_link(owner),
            "readyForTurn":book.ready_for_turn(owner),
            "canCompact":book.can_compact(owner),
            "compaction":book.compaction_status(owner),
            "lastBranch":self.app_server.last_branches.get(owner),
            "delegates":self.native_delegate_views(owner),
            "observed":book.observed(owner),
            "busy":self.app_server.busy(owner) || self.conversation_busy(owner)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn destructive_intents_require_the_exact_still_existing_native_binding() {
        let mut binding = Binding {
            thread_id: "native-a".into(),
            session_id: None,
            archived: false,
            deleted: false,
            never_submitted: false,
        };
        for kind in [
            "archive",
            "unarchive",
            "delete",
            "rename",
            "new_fork",
            "start_review",
            "reset_unused",
            "compact",
            "mcp_status",
            "mcp_control",
            "apps_control",
            "hooks_status",
            "goal",
        ] {
            let mut value = json!({"kind":kind,"expected_thread_id":"native-a"});
            if kind == "goal" {
                value["action"] = json!({"kind":"refresh"});
            }
            if kind == "mcp_control" {
                value["action"] =
                    json!({"kind":"login","inventory_id":"observed","name":"fixture"});
            }
            if kind == "apps_control" {
                value["action"] = json!({"kind":"refresh"});
            }
            if kind == "hooks_status" {
                value["expected_directory"] = json!("C:\\fixture");
            }
            if kind == "rename" || kind == "new_fork" {
                value["name"] = json!("New name");
            }
            if kind == "start_review" {
                value["target"] = json!({"type":"uncommittedChanges"});
            }
            if kind == "new_fork"
                || kind == "start_review"
                || kind == "goal"
                || kind == "hooks_status"
            {
                value["expected_directory"] = json!("C:\\fixture");
                let mut missing_directory = value.clone();
                missing_directory
                    .as_object_mut()
                    .unwrap()
                    .remove("expected_directory");
                assert!(serde_json::from_value::<ConversationAction>(missing_directory).is_err());
            }
            let action: ConversationAction = serde_json::from_value(value.clone()).unwrap();
            assert!(action.validate_target(Some(&binding)).is_ok());
            assert!(action.validate_target(None).is_err());
            binding.thread_id = "replacement".into();
            assert!(action.validate_target(Some(&binding)).is_err());
            binding.thread_id = "native-a".into();
            binding.deleted = true;
            assert!(action.validate_target(Some(&binding)).is_err());
            binding.deleted = false;
            value.as_object_mut().unwrap().remove("expected_thread_id");
            if kind == "apps_control" {
                let draft: ConversationAction = serde_json::from_value(value).unwrap();
                assert!(draft.validate_target(None).is_ok());
                assert!(draft.validate_target(Some(&binding)).is_err());
            } else {
                assert!(serde_json::from_value::<ConversationAction>(value).is_err());
            }
        }
    }
}
