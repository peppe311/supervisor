//! Prepare only the selected existing conversation. Never creates a thread/turn.
use super::*;

#[derive(Default)]
pub(super) struct Preparation {
    selected: Option<String>,
    pending: Option<(String, String)>,
    attempted: std::collections::VecDeque<(u64, String, String)>,
}

impl Preparation {
    fn begin(
        &mut self,
        epoch: u64,
        book: &mut Conversations,
        cwd: &Path,
        access: api::Access,
    ) -> Option<ThreadRequest> {
        let owner = self.selected.as_deref()?;
        if self.pending.is_some()
            || book.is_thread_loaded(owner)
            || book.busy(owner)
            || book.active_turn(owner).is_some()
            || !cwd.is_dir()
        {
            return None;
        }
        let binding = book.binding(owner)?;
        if binding.archived || binding.deleted || binding.never_submitted {
            return None;
        }
        let key = (epoch, owner.to_owned(), binding.thread_id.clone());
        if self.attempted.contains(&key) {
            return None;
        }
        self.attempted.push_back(key);
        while self.attempted.len() > 64 {
            self.attempted.pop_front();
        }
        let request = book
            .open(owner, cwd, &api::Profile::default(), access)
            .ok()?;
        self.pending = Some((owner.into(), request.diagnostic_id()));
        Some(request)
    }
    pub(super) fn pending(&self, owner: &str) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|(target, _)| target == owner)
    }
    pub(super) fn selected_owner(&self) -> Option<&str> {
        self.selected.as_deref()
    }
    pub(super) fn finish(&mut self, request: &ThreadRequest) -> bool {
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, id)| *id == request.diagnostic_id())
        {
            self.pending = None;
            return true;
        }
        false
    }
    pub(super) fn disconnect(&mut self) {
        self.pending = None;
        self.attempted.clear();
    }
    pub(super) fn forget(&mut self, owner: &str) {
        if self.selected.as_deref() == Some(owner) {
            self.selected = None;
        }
        self.attempted.retain(|(_, target, _)| target != owner);
    }
}

impl BrowserApp {
    pub(in crate::browser) fn select_native_preparation(&mut self, owner: String) {
        if !self.native_access_selected(&owner) || self.conversation_target(&owner).is_err() {
            return;
        }
        // A delayed main-window event cannot prepare a chat selected elsewhere.
        if !owner.starts_with("graph:") && owner != self.composer_owner() {
            return;
        }
        self.app_server.preparation.selected = Some(owner);
        self.advance_native_preparation();
    }

    pub(super) fn advance_native_preparation(&mut self) {
        if !self.app_server.configuration().0.can_run()
            || self.app_server.native_config_pending()
            || self.app_server.preparation.pending.is_some()
        {
            return;
        }
        let Some(owner) = self.app_server.preparation.selected.clone() else {
            return;
        };
        if !self.native_access_selected(&owner)
            || self.app_server.conversations.is_thread_loaded(&owner)
            || self.app_server.busy(&owner)
            || !self.app_server.requests.views(&owner).is_empty()
        {
            return;
        }
        let Ok(target) = self.conversation_target(&owner) else {
            return;
        };
        if target.remote {
            return;
        }
        let cwd = target
            .root
            .unwrap_or_else(|| self.data_dir.join("codex-workspace"));
        if !cwd.is_dir() {
            return;
        }
        // open() validates ownership, uncertain delivery, active work and directory;
        // an existing binding emits thread/resume with excludeTurns, without input.
        let access = self.app_server.access();
        if let Some(request) = self.app_server.preparation.begin(
            self.app_server.epoch,
            &mut self.app_server.conversations,
            &cwd,
            access,
        ) {
            self.app_server.roots.insert(owner.clone(), cwd);
            self.dispatch_app_server_thread(request);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn saved_book() -> Conversations {
        let mut saved = Saved::default();
        for (owner, id) in [("chat:a", "native-a"), ("graph:b", "native-b")] {
            saved.bindings.insert(
                owner.into(),
                serde_json::from_value(json!({"threadId":id,"sessionId":null,"archived":false}))
                    .unwrap(),
            );
        }
        Conversations::restore(saved).unwrap()
    }
    #[test]
    fn selected_session_is_resumed_once_without_phantom_work_or_a_prompt() {
        let temp = tempfile::tempdir().unwrap();
        let mut state = State {
            conversations: saved_book(),
            ..State::default()
        };
        state.preparation.selected = Some("chat:a".into());
        assert!(!state.conversations.ready_for_turn("chat:a"));
        let request = state
            .preparation
            .begin(
                1,
                &mut state.conversations,
                temp.path(),
                api::Access::ReadOnly,
            )
            .unwrap();
        assert_eq!(request.call.method, "thread/resume");
        assert_eq!(request.call.params["threadId"], "native-a");
        assert_eq!(request.call.params["excludeTurns"], true);
        for key in ["input", "model", "reasoningEffort", "history"] {
            assert!(request.call.params.get(key).is_none());
        }
        assert!(state.conversations.busy("chat:a"));
        assert!(
            !state.busy("chat:a"),
            "Prewarming alone must not show Stop or enqueue the user's prompt"
        );
        assert!(
            state
                .preparation
                .begin(
                    1,
                    &mut state.conversations,
                    temp.path(),
                    api::Access::ReadOnly
                )
                .is_none()
        );
        assert!(state.preparation.finish(&request));
        state
            .conversations
            .complete(
                &request,
                Ok(json!({"thread":{"id":"native-a","status":{"type":"idle"},"turns":[]}})),
            )
            .unwrap();
        assert!(state.conversations.ready_for_turn("chat:a"));
        assert!(!state.conversations.is_thread_loaded("graph:b"));
        assert!(
            state
                .preparation
                .begin(
                    1,
                    &mut state.conversations,
                    temp.path(),
                    api::Access::ReadOnly
                )
                .is_none()
        );
        let prompt = state
            .conversations
            .start_turn(
                "chat:a",
                "user-input",
                vec![api::text_input("Prompt")],
                temp.path(),
                &api::Profile::default(),
                api::Access::ReadOnly,
            )
            .unwrap();
        assert_eq!(
            prompt.call.method, "turn/start",
            "Prepared send needs no extra resume round trip"
        );
        assert!(
            state
                .conversations
                .start_turn(
                    "chat:a",
                    "duplicate",
                    vec![api::text_input("Prompt")],
                    temp.path(),
                    &api::Profile::default(),
                    api::Access::ReadOnly
                )
                .is_err()
        );
    }
    #[test]
    fn failures_retry_only_on_user_send_or_reconnect_and_late_replies_keep_new_target() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = saved_book();
        let mut preparation = Preparation {
            selected: Some("chat:a".into()),
            ..Preparation::default()
        };
        let old = preparation
            .begin(1, &mut book, temp.path(), api::Access::ReadOnly)
            .unwrap();
        preparation.finish(&old);
        assert!(
            book.complete(&old, Err(CallError::Rejected("fixture".into())))
                .is_err()
        );
        assert!(
            preparation
                .begin(1, &mut book, temp.path(), api::Access::ReadOnly)
                .is_none()
        );
        // A real user send may retry immediately; automatic warmup cannot loop.
        assert!(
            book.open(
                "chat:a",
                temp.path(),
                &api::Profile::default(),
                api::Access::ReadOnly
            )
            .is_ok()
        );
        book.disconnect();
        preparation.disconnect();
        preparation.selected = Some("graph:b".into());
        let new = preparation
            .begin(2, &mut book, temp.path(), api::Access::ReadOnly)
            .unwrap();
        assert!(!preparation.finish(&old));
        assert!(preparation.pending("graph:b"));
        assert_eq!(new.owner, "graph:b");
        preparation.forget("graph:b");
        assert!(preparation.selected.is_none());
    }
    #[test]
    fn new_archived_deleted_and_uncertain_threads_are_not_automatically_opened() {
        let temp = tempfile::tempdir().unwrap();
        let mut preparation = Preparation {
            selected: Some("chat:a".into()),
            ..Preparation::default()
        };
        assert!(
            preparation
                .begin(
                    1,
                    &mut Conversations::default(),
                    temp.path(),
                    api::Access::ReadOnly
                )
                .is_none()
        );
        for flag in ["archived", "deleted", "neverSubmitted"] {
            let mut saved = serde_json::to_value(saved_book().saved()).unwrap();
            saved["bindings"]["chat:a"][flag] = json!(true);
            let mut book = Conversations::restore(serde_json::from_value(saved).unwrap()).unwrap();
            assert!(
                preparation
                    .begin(1, &mut book, temp.path(), api::Access::ReadOnly)
                    .is_none(),
                "{flag}"
            );
        }
        let mut saved = saved_book().saved().clone();
        saved.unresolved.insert(
            "chat:a".into(),
            serde_json::from_value(
                json!({"threadId":"native-a","messageId":"pending","steer":false}),
            )
            .unwrap(),
        );
        let mut book = Conversations::restore(saved).unwrap();
        assert!(
            preparation
                .begin(1, &mut book, temp.path(), api::Access::ReadOnly)
                .is_none()
        );
        assert!(book.saved().unresolved.contains_key("chat:a"));
    }
}
