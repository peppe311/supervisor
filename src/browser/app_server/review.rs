//! Native reviewer orchestration: configure the existing session, then call
//! review/start. No local Git inspection, synthetic prompt or review engine.
use super::*;

pub(super) struct Pending {
    root: PathBuf,
    target: api::ReviewTarget,
    delivery: api::ReviewDelivery,
    destination_name: Option<String>,
    destination: Option<branches::BranchTarget>,
    pub(super) sending: bool,
    pub(super) cancelled: bool,
}
impl Pending {
    pub(super) fn request_stop(&mut self) -> bool {
        self.cancelled = true;
        !self.sending
    }
}
impl BrowserApp {
    pub(super) fn refresh_completed_native_review(&mut self, owner: &str) {
        // The metadata reply starts an asynchronous page sequence. Native idle
        // and completion events may arrive during those pages: leave their
        // refresh dirty until the current sequence finishes instead of starting
        // an overlapping history load.
        if self.app_server.history_pages.busy(owner) {
            return;
        }
        match self.app_server.conversations.take_review_refresh(owner) {
            Ok(Some(request)) => self.dispatch_app_server_thread(request),
            Ok(None) => {}
            Err(error) => self.conversation_event(
                "central-agent:app-server-conversation",
                json!({"owner":owner,"error":error}),
            ),
        }
    }

    pub(super) fn prepare_native_review(
        &mut self,
        owner: &str,
        expected_directory: &str,
        target: api::ReviewTarget,
        delivery: api::ReviewDelivery,
        destination_name: Option<String>,
    ) -> Result<(), String> {
        if self.app_server.native_config_pending() {
            return Err(
                "Wait for the shared Codex configuration change before starting a review".into(),
            );
        }
        target.validate()?;
        if !self.native_access_selected(owner) || !self.app_server.configuration().0.can_run() {
            return Err("Select and connect Codex before starting a review".into());
        }
        api::Access::ReadOnly.validate_requirements(self.app_server.requirements.as_ref())?;
        let destination = self.conversation_target(owner)?;
        let root = destination
            .root
            .filter(|root| !destination.remote && root.is_absolute() && root.is_dir())
            .ok_or("Choose an existing local directory for review")?;
        if !same_local_path(&root, Path::new(expected_directory))
            || self
                .pending_workspace_ejections
                .contains(&root.display().to_string())
        {
            return Err("The project changed. Confirm the review again.".into());
        }
        let destination_name = match delivery {
            api::ReviewDelivery::Inline => None,
            api::ReviewDelivery::Detached => {
                let history_mode = self
                    .app_server
                    .conversations
                    .binding(owner)
                    .and_then(|binding| {
                        self.app_server
                            .conversations
                            .mirror
                            .thread(&binding.thread_id)
                    })
                    .and_then(|thread| thread.history_mode.as_deref());
                if history_mode != Some("legacy") {
                    return Err("Codex 0.153.4 supports separate review only for a native legacy history. Paginated or unknown histories must use inline review, or an explicit fork followed by inline review.".into());
                }
                Some(
                    normalized_user_title(
                        destination_name.as_deref().unwrap_or_default(),
                        MAX_PROJECT_LABEL_CHARS,
                    )
                    .ok_or("Name the separate review conversation")?,
                )
            }
        };
        let request = self.app_server.conversations.prepare_review(owner, &root)?;
        self.app_server.roots.insert(owner.into(), root.clone());
        self.app_server.reviews.insert(
            owner.into(),
            Pending {
                root,
                target,
                delivery,
                destination_name,
                destination: None,
                sending: false,
                cancelled: false,
            },
        );
        self.dispatch_app_server_thread(request);
        Ok(())
    }

    pub(super) fn start_prepared_native_review(&mut self, owner: &str) -> Result<(), String> {
        let (root, review_target, delivery, destination_name, cancelled) = {
            let pending = self
                .app_server
                .reviews
                .get(owner)
                .ok_or("This review is no longer pending")?;
            (
                pending.root.clone(),
                pending.target.clone(),
                pending.delivery,
                pending.destination_name.clone(),
                pending.cancelled,
            )
        };
        if cancelled {
            self.app_server.reviews.remove(owner);
            self.emit_app_server_conversation(owner, "review_cancelled");
            return Ok(());
        }
        if !self.native_access_selected(owner) || !self.app_server.configuration().0.can_run() {
            return Err("Codex configuration changed before the review started".into());
        }
        let conversation = self.conversation_target(owner)?;
        if conversation.remote || conversation.root.as_ref() != Some(&root) || !root.is_dir() {
            return Err("The review directory changed. Confirm it again.".into());
        }
        api::Access::ReadOnly.validate_requirements(self.app_server.requirements.as_ref())?;
        let receipt = Uuid::new_v4().to_string();
        let request = match delivery {
            api::ReviewDelivery::Inline => {
                self.app_server
                    .conversations
                    .start_review(owner, &review_target, &receipt)?
            }
            api::ReviewDelivery::Detached => {
                // Local ownership is durably allocated before Codex can create
                // the native review thread. A lost ACK is recovered through the
                // existing fork-origin/native-history workflow, never replayed.
                let branch = self.create_native_destination(
                    owner,
                    destination_name
                        .as_deref()
                        .ok_or("The separate review destination is missing")?,
                    &root,
                )?;
                self.app_server
                    .roots
                    .insert(branch.owner.clone(), root.clone());
                self.app_server.reviews.get_mut(owner).unwrap().destination = Some(branch.clone());
                self.app_server.conversations.start_detached_review(
                    owner,
                    &branch.owner,
                    &review_target,
                    &receipt,
                )?
            }
        };
        if let Err(error) = self.save_app_server_bindings() {
            let _ = self
                .app_server
                .conversations
                .complete(&request, Err(CallError::Rejected(error.clone())));
            return Err(error);
        }
        self.app_server.reviews.get_mut(owner).unwrap().sending = true;
        self.dispatch_app_server_thread(request);
        Ok(())
    }

    pub(super) fn fail_native_review(&mut self, owner: &str, error: String) {
        if let Some(branch) = self
            .app_server
            .reviews
            .remove(owner)
            .and_then(|review| review.destination)
        {
            // Preserve an already-saved empty destination. For an uncertain
            // delivery its pendingFork marker enables exact native-history
            // recovery; for a definitive rejection it remains an ordinary,
            // unbound local conversation rather than silently deleting UI data.
            self.app_server.last_branches.insert(owner.into(), branch);
        }
        self.conversation_event(
            "central-agent:app-server-conversation",
            json!({"owner":owner,"error":error}),
        );
    }
    pub(super) fn accepted_native_review(&mut self, owner: &str) {
        let cancelled = self
            .app_server
            .reviews
            .remove(owner)
            .is_some_and(|review| review.cancelled);
        if cancelled && self.app_server.conversations.active_turn(owner).is_some() {
            self.stop_app_server(owner);
        }
    }

    pub(super) fn accepted_detached_native_review(
        &mut self,
        source_owner: &str,
        destination_owner: &str,
    ) {
        let Some(review) = self.app_server.reviews.remove(source_owner) else {
            return;
        };
        if let Some(branch) = review
            .destination
            .filter(|branch| branch.owner == destination_owner)
        {
            self.app_server
                .last_branches
                .insert(source_owner.into(), branch);
        }
        self.emit_app_server_conversation(source_owner, "review_detached");
        if review.cancelled
            && self
                .app_server
                .conversations
                .active_turn(destination_owner)
                .is_some()
        {
            self.stop_app_server(destination_owner);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn review_preparation_reserves_only_its_owner_and_stop_remains_pending() {
        let mut state = State::default();
        state.reviews.insert(
            "graph:a".into(),
            Pending {
                root: PathBuf::from("C:\\fixture"),
                target: api::ReviewTarget::UncommittedChanges,
                delivery: api::ReviewDelivery::Inline,
                destination_name: None,
                destination: None,
                sending: false,
                cancelled: false,
            },
        );
        assert!(state.busy("graph:a"));
        assert!(!state.busy("chat:other"));
        assert!(state.any_busy());
        let review = state.reviews.get_mut("graph:a").unwrap();
        assert!(review.request_stop());
        assert!(review.cancelled);
        review.sending = true;
        assert!(!review.request_stop());
        assert!(state.busy("graph:a"));
        state.reviews.remove("graph:a");
        assert!(!state.any_busy());
    }
}
