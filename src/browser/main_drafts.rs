//! Selection swaps the view of a draft; it does not discard or retarget it.
use super::*;

#[derive(Clone, Debug)]
struct DraftProfile {
    provider: AgentProviderKind,
    selections: [AgentSelection; 5],
}

#[derive(Clone, Debug)]
pub(super) struct MainDraft {
    pub snapshots: SubmissionSnapshots,
    profile: DraftProfile,
}

pub(super) fn remove_accepted_attachments(
    current: &mut SubmissionSnapshots,
    accepted: &SubmissionSnapshots,
) {
    current.tabs.retain(|tab| {
        !accepted
            .tabs
            .iter()
            .any(|sent| sent.tab_id == tab.tab_id && sent.capture_id == tab.capture_id)
    });
    current.terminals.retain(|terminal| {
        !accepted.terminals.iter().any(|sent| {
            sent.snapshot.session_id == terminal.snapshot.session_id
                && sent.snapshot.captured_at_ms == terminal.snapshot.captured_at_ms
        })
    });
    current
        .files
        .retain(|file| !accepted.files.iter().any(|sent| sent.id() == file.id()));
}

fn exchange_draft(
    saved: &mut HashMap<String, MainDraft>,
    previous: &str,
    next: &str,
    outgoing: MainDraft,
    keep_previous: bool,
) -> Option<MainDraft> {
    let incoming = saved.remove(next);
    if keep_previous {
        saved.insert(previous.to_owned(), outgoing);
    }
    incoming
}

fn find_capture_mut<'a>(
    live: &'a mut [TabContextSnapshot],
    saved: &'a mut HashMap<String, MainDraft>,
    tab_id: u64,
    capture_id: u64,
) -> Option<&'a mut TabContextSnapshot> {
    live.iter_mut()
        .chain(
            saved
                .values_mut()
                .flat_map(|draft| &mut draft.snapshots.tabs),
        )
        .find(|c| c.tab_id == tab_id && c.capture_id == capture_id)
}

impl BrowserApp {
    pub(super) fn consume_accepted_attachments(&mut self, submission: &PendingAgentSubmission) {
        let Some(owner) = submission.owner() else {
            return;
        };
        if submission.card_draft {
            self.graph_files.consume(owner, &submission.snapshots.files);
            self.graph_contexts.consume(owner, &submission.snapshots);
            self.emit_graph_contexts();
            return;
        }
        self.consume_snapshot_attachments(owner, &submission.snapshots);
    }

    pub(super) fn consume_snapshot_attachments(
        &mut self,
        owner: &str,
        snapshots: &SubmissionSnapshots,
    ) {
        if owner.starts_with("graph:")
            || self.graph_files.has_draft(owner)
            || self.graph_contexts.drafts.contains_key(owner)
        {
            self.graph_files.consume(owner, &snapshots.files);
            self.graph_contexts.consume(owner, snapshots);
            self.emit_graph_contexts();
            return;
        }
        if owner == self.main_draft_owner {
            let mut current = SubmissionSnapshots {
                tabs: std::mem::take(&mut self.draft_contexts),
                terminals: std::mem::take(&mut self.draft_terminal_contexts),
                files: std::mem::take(&mut self.draft_file_attachments),
            };
            remove_accepted_attachments(&mut current, snapshots);
            self.draft_contexts = current.tabs;
            self.draft_terminal_contexts = current.terminals;
            self.draft_file_attachments = current.files;
        } else if let Some(draft) = self.main_drafts.get_mut(owner) {
            remove_accepted_attachments(&mut draft.snapshots, snapshots);
        }
    }

    pub(super) fn draft_key(&self) -> String {
        self.active_project_chat_id.as_ref().map_or_else(
            || {
                format!(
                    "draft:{}",
                    self.workspace
                        .root()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                )
            },
            |id| format!("chat:{id}"),
        )
    }

    pub(super) fn switch_main_draft(&mut self) {
        let next_owner = self.draft_key();
        if self.main_draft_owner == next_owner {
            return;
        }
        let outgoing = MainDraft {
            snapshots: SubmissionSnapshots {
                tabs: std::mem::take(&mut self.draft_contexts),
                terminals: std::mem::take(&mut self.draft_terminal_contexts),
                files: std::mem::take(&mut self.draft_file_attachments),
            },
            profile: DraftProfile {
                provider: self.agent_provider,
                selections: [
                    self.claude_selection.clone(),
                    self.cursor_selection.clone(),
                    self.github_copilot_selection.clone(),
                    self.google_antigravity_selection.clone(),
                    self.opencode_go_selection.clone(),
                ],
            },
        };
        let keep_previous = self.main_draft_owner.starts_with("draft:")
            || self
                .main_draft_owner
                .strip_prefix("chat:")
                .is_some_and(|id| self.project_chats.iter().any(|chat| chat.id == id));
        let incoming = exchange_draft(
            &mut self.main_drafts,
            &self.main_draft_owner,
            &next_owner,
            outgoing,
            keep_previous,
        );
        self.main_draft_owner = next_owner;
        if let Some(incoming) = incoming {
            self.draft_contexts = incoming.snapshots.tabs;
            self.draft_terminal_contexts = incoming.snapshots.terminals;
            self.draft_file_attachments = incoming.snapshots.files;
            self.agent_provider = incoming.profile.provider;
            [
                self.claude_selection,
                self.cursor_selection,
                self.github_copilot_selection,
                self.google_antigravity_selection,
                self.opencode_go_selection,
            ] = incoming.profile.selections;
            // A throttled follow-live callback may have fired while another draft was visible.
            for context in &mut self.draft_terminal_contexts {
                context.live_update_scheduled = false;
            }
            let following = self
                .draft_terminal_contexts
                .iter()
                .filter(|c| c.follow_live)
                .map(|c| c.snapshot.session_id)
                .collect::<Vec<_>>();
            for session_id in following {
                self.refresh_following_terminal_context(session_id, true);
            }
        }
    }

    pub(super) fn tab_draft(&self, tab_id: u64, capture_id: u64) -> Option<&TabContextSnapshot> {
        self.draft_contexts
            .iter()
            .chain(self.main_drafts.values().flat_map(|d| &d.snapshots.tabs))
            .chain(self.graph_contexts.drafts.values().flat_map(|d| &d.tabs))
            .find(|c| c.tab_id == tab_id && c.capture_id == capture_id)
    }

    pub(super) fn tab_draft_mut(
        &mut self,
        tab_id: u64,
        capture_id: u64,
    ) -> Option<&mut TabContextSnapshot> {
        find_capture_mut(
            &mut self.draft_contexts,
            &mut self.main_drafts,
            tab_id,
            capture_id,
        )
        .or_else(|| {
            self.graph_contexts
                .drafts
                .values_mut()
                .flat_map(|d| &mut d.tabs)
                .find(|c| c.tab_id == tab_id && c.capture_id == capture_id)
        })
    }

    pub(super) fn forget_tab_drafts(&mut self, tab_id: u64) {
        self.draft_contexts.retain(|c| c.tab_id != tab_id);
        for draft in self.main_drafts.values_mut() {
            draft.snapshots.tabs.retain(|c| c.tab_id != tab_id);
        }
        for draft in self.graph_contexts.drafts.values_mut() {
            draft.tabs.retain(|c| c.tab_id != tab_id);
        }
        self.emit_graph_contexts();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(provider: AgentProviderKind, model: &str, capture_id: u64) -> MainDraft {
        let selection = AgentSelection {
            model: model.to_owned(),
            ..Default::default()
        };
        MainDraft {
            snapshots: SubmissionSnapshots {
                tabs: vec![TabContextSnapshot::capturing(
                    7,
                    capture_id,
                    model,
                    "https://example.com",
                    1,
                )],
                ..Default::default()
            },
            profile: DraftProfile {
                provider,
                selections: std::array::from_fn(|_| selection.clone()),
            },
        }
    }

    #[test]
    fn navigation_restores_profiles_and_pending_captures_without_cross_chat_updates() {
        let mut saved = HashMap::new();
        let a = draft(AgentProviderKind::ClaudeCode, "model-a", 10);
        let mut b = draft(AgentProviderKind::ClaudeCode, "model-b", 11);
        assert!(exchange_draft(&mut saved, "chat:a", "chat:b", a, true).is_none());
        // Both chats captured tab 7. Only A's capture 10 may receive this late response.
        find_capture_mut(&mut b.snapshots.tabs, &mut saved, 7, 10)
            .unwrap()
            .text = "A's completed page".to_owned();
        assert!(b.snapshots.tabs[0].text.is_empty());
        assert!(find_capture_mut(&mut b.snapshots.tabs, &mut saved, 7, 99).is_none());
        let a = exchange_draft(&mut saved, "chat:b", "chat:a", b, true).unwrap();
        assert_eq!(a.profile.provider, AgentProviderKind::ClaudeCode);
        assert_eq!(a.profile.selections[0].model, "model-a");
        assert_eq!(a.snapshots.tabs[0].text, "A's completed page");
        let b = exchange_draft(&mut saved, "chat:a", "chat:b", a, true).unwrap();
        assert_eq!(b.profile.provider, AgentProviderKind::ClaudeCode);
        assert_eq!(b.profile.selections[1].model, "model-b");
        assert_eq!(b.snapshots.tabs[0].capture_id, 11);
    }

    #[test]
    fn deleted_owner_is_not_resurrected_when_selection_changes() {
        let mut saved = HashMap::new();
        let a = draft(AgentProviderKind::ClaudeCode, "model-a", 10);
        exchange_draft(&mut saved, "chat:deleted", "chat:b", a, false);
        assert!(!saved.contains_key("chat:deleted"));
    }

    #[test]
    fn queued_delivery_never_removes_a_new_capture_of_the_same_tab() {
        let old = draft(AgentProviderKind::ClaudeCode, "original", 10);
        let mut next = draft(AgentProviderKind::ClaudeCode, "new capture", 11);
        remove_accepted_attachments(&mut next.snapshots, &old.snapshots);
        assert_eq!(next.snapshots.tabs.len(), 1);
        let accepted = next.snapshots.clone();
        remove_accepted_attachments(&mut next.snapshots, &accepted);
        assert!(next.snapshots.tabs.is_empty());
    }
}
