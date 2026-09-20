//! Restore display history through native reads, never resume or replay work.
use super::*;
use std::collections::VecDeque;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct View {
    pub(super) loading: bool,
    checked: bool,
    total: usize,
    loaded: usize,
    unavailable: usize,
    skipped: usize,
}

#[derive(Default)]
pub(super) struct Reload {
    pub(super) view: View,
    pending: VecDeque<(String, String)>,
    active: Option<Active>,
}

struct Active {
    owner: String,
    read_id: String,
    page_sequence: Option<u64>,
}

impl Reload {
    fn begin(&mut self, saved: &Saved, preferred: &str) {
        if self.view.loading {
            return;
        }
        let mut owners = saved
            .bindings
            .iter()
            .filter(|(_, binding)| !binding.deleted)
            .map(|(owner, binding)| (owner.clone(), binding.thread_id.clone()))
            .collect::<Vec<_>>();
        owners.sort_by_key(|(owner, _)| owner != preferred);
        self.view = View {
            loading: !owners.is_empty(),
            checked: true,
            total: owners.len(),
            ..View::default()
        };
        self.pending = owners.into();
        self.active = None;
    }

    fn next(&mut self) -> Option<(String, String)> {
        if self.active.is_some() {
            return None;
        }
        let next = self.pending.pop_front();
        if next.is_none() {
            self.view.loading = false;
        }
        next
    }

    pub(super) fn start(&mut self, request: &ThreadRequest) {
        self.active = Some(Active {
            owner: request.owner.clone(),
            read_id: request.diagnostic_id(),
            page_sequence: None,
        });
    }

    pub(super) fn matches(&self, request: &ThreadRequest) -> bool {
        request.call.method == "thread/read"
            && self.active.as_ref().is_some_and(|active| {
                active.owner == request.owner && active.read_id == request.diagnostic_id()
            })
    }

    pub(super) fn read_reply(
        &mut self,
        request: &ThreadRequest,
        success: bool,
        sequence: Option<u64>,
    ) {
        if !self.matches(request) {
            return;
        }
        if success && request.omits_turns() && sequence.is_some() {
            self.active.as_mut().unwrap().page_sequence = sequence;
        } else {
            self.finish(success && !request.omits_turns());
        }
    }

    pub(super) fn page_reply(&mut self, owner: &str, sequence: u64, success: bool) {
        if self
            .active
            .as_ref()
            .is_some_and(|active| active.owner == owner && active.page_sequence == Some(sequence))
        {
            self.finish(success);
        }
    }

    fn finish(&mut self, success: bool) {
        self.active = None;
        if success {
            self.view.loaded += 1;
        } else {
            self.view.unavailable += 1;
        }
    }

    pub(super) fn disconnect(&mut self) {
        if self.view.loading {
            self.view.unavailable += self.pending.len() + usize::from(self.active.is_some());
            self.pending.clear();
            self.active = None;
            self.view.loading = false;
        }
    }
}

impl BrowserApp {
    pub(super) fn begin_native_chat_reload(&mut self) {
        if self.app_server.binding_lock.is_none() || self.app_server.binding_store_error.is_some() {
            self.app_server.view.errors.insert(
                "chat-reload",
                "Chat history is unavailable until Supervisor's local archive can be read safely."
                    .into(),
            );
            return;
        }
        self.app_server.view.errors.remove("chat-reload");
        let preferred = self.composer_owner();
        self.app_server
            .chat_reload
            .begin(self.app_server.conversations.saved(), &preferred);
    }

    pub(super) fn advance_native_chat_reload(&mut self) {
        if !self.app_server.view.connected {
            return;
        }
        if self.app_server.binding_store_error.is_some() {
            self.app_server.chat_reload.disconnect();
            return;
        }
        while let Some((owner, thread)) = self.app_server.chat_reload.next() {
            // Refresh only the captured binding. Never redirect a queued read
            // after unlink/rebind, or disturb active work and draft ownership.
            let unchanged = self
                .app_server
                .conversations
                .binding(&owner)
                .is_some_and(|binding| !binding.deleted && binding.thread_id == thread);
            let active = self.app_server.conversations.active_turn(&owner).is_some()
                || self.app_server.submissions.contains_key(&owner)
                || self.app_server.reviews.contains_key(&owner)
                || self.app_server.steering.contains_key(&owner)
                || self.app_server.history_pages.busy(&owner)
                || self.app_server.goals.writing(&owner);
            // An unresolved delivery receipt is deliberately not active work:
            // its read may reconcile the native client ID, without resending it.
            if !unchanged || active {
                self.app_server.chat_reload.view.skipped += 1;
                continue;
            }
            match self
                .app_server
                .conversations
                .action(&owner, ThreadAction::Read)
            {
                Ok(request) => {
                    self.app_server.chat_reload.start(&request);
                    self.dispatch_app_server_thread(request);
                    return;
                }
                Err(_) => self.app_server.chat_reload.view.unavailable += 1,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use central_agent_codex_runtime::conversations::Binding;

    fn saved() -> Saved {
        let mut saved = Saved::default();
        for (owner, archived, deleted) in [
            ("chat:first", false, false),
            ("graph:second", true, false),
            ("chat:deleted", false, true),
        ] {
            saved.bindings.insert(
                owner.into(),
                Binding {
                    thread_id: format!("native-{owner}"),
                    session_id: None,
                    archived,
                    deleted,
                    never_submitted: false,
                },
            );
        }
        saved
    }

    #[test]
    fn restore_prioritizes_selection_and_waits_for_exact_complete_history() {
        let saved = saved();
        let mut book = Conversations::restore(saved.clone()).unwrap();
        let mut reload = Reload::default();
        reload.begin(&saved, "graph:second");
        assert_eq!(reload.view.total, 2);
        assert_eq!(reload.next().unwrap().0, "graph:second");
        let request = book.action("graph:second", ThreadAction::Read).unwrap();
        assert_eq!(request.call.method, "thread/read");
        assert!(request.omits_turns());
        reload.start(&request);
        reload.begin(&saved, "chat:first"); // Duplicate intent cannot replace the batch.
        assert!(reload.next().is_none());
        let unrelated = book.action("graph:second", ThreadAction::Read).unwrap();
        reload.read_reply(&unrelated, false, None);
        assert_eq!(reload.view.unavailable, 0);
        reload.read_reply(&request, true, Some(7));
        assert_eq!(reload.view.loaded, 0); // Metadata alone is not restored messages.
        reload.page_reply("chat:first", 7, true);
        reload.page_reply("graph:second", 6, true);
        assert_eq!(reload.view.loaded, 0);
        reload.page_reply("graph:second", 7, true);
        assert_eq!(reload.view.loaded, 1);
        assert_eq!(reload.next().unwrap().0, "chat:first");
        let missing = book.action("chat:first", ThreadAction::Read).unwrap();
        reload.start(&missing);
        reload.read_reply(&missing, false, None);
        assert!(reload.next().is_none());
        assert!(!reload.view.loading);
        assert_eq!(reload.view.unavailable, 1);
        assert_eq!(
            serde_json::to_value(book.saved()).unwrap(),
            serde_json::to_value(saved).unwrap()
        );
    }

    #[test]
    fn disconnect_preserves_progress_and_requires_a_new_batch() {
        let saved = saved();
        let mut book = Conversations::restore(saved.clone()).unwrap();
        let mut reload = Reload::default();
        reload.begin(&saved, "chat:first");
        reload.next().unwrap();
        let request = book.action("chat:first", ThreadAction::Read).unwrap();
        reload.start(&request);
        reload.disconnect();
        assert!(!reload.view.loading);
        assert_eq!(reload.view.unavailable, 2);
        reload.read_reply(&request, true, Some(1));
        reload.page_reply("chat:first", 1, true);
        assert_eq!(reload.view.loaded, 0);
        assert!(reload.next().is_none());
        reload.begin(&saved, "chat:first");
        assert!(reload.view.loading);
        assert_eq!(reload.view.unavailable, 0);
    }
}
