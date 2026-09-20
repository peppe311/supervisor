//! Track native compaction delivery, never compact or reconstruct history here.
use super::*;

pub(super) struct Pending {
    pub(super) accepted: bool,
    previous_turns: HashSet<String>,
    turn: Option<String>,
    completed_item: bool,
    stop_requested: bool,
}

impl Conversations {
    pub fn can_compact(&self, owner: &str) -> bool {
        self.ready_for_turn(owner)
            && self
                .binding(owner)
                .and_then(|binding| self.mirror.thread(&binding.thread_id))
                .is_some_and(|thread| !thread.turns.is_empty())
    }

    pub(super) fn begin_compaction(
        &mut self,
        owner: &str,
        operation_id: &str,
    ) -> Result<(), String> {
        if !self.can_compact(owner) || operation_id.trim().is_empty() {
            return Err(
                "Resume an idle native conversation with history before compacting its context"
                    .into(),
            );
        }
        let thread_id = self.binding(owner).unwrap().thread_id.clone();
        self.saved.bindings.get_mut(owner).unwrap().never_submitted = false;
        let previous_turns = self
            .mirror
            .thread(&thread_id)
            .unwrap()
            .turns
            .iter()
            .map(|turn| turn.id.clone())
            .collect();
        self.saved.unresolved.insert(
            owner.into(),
            Receipt {
                thread_id,
                message_id: operation_id.into(),
                steer: false,
                review: false,
                compact: true,
            },
        );
        self.compactions.insert(
            owner.into(),
            Pending {
                accepted: false,
                previous_turns,
                turn: None,
                completed_item: false,
                stop_requested: false,
            },
        );
        Ok(())
    }

    pub fn compaction_pending(&self, owner: &str) -> bool {
        self.compactions.contains_key(owner)
    }

    pub fn compaction_status(&self, owner: &str) -> Option<&'static str> {
        self.compactions.get(owner).map(|pending| {
            if pending.stop_requested {
                "stop_requested"
            } else if pending.turn.is_some() {
                "running"
            } else if pending.accepted {
                "accepted"
            } else {
                "sending"
            }
        })
    }

    pub fn request_compaction_stop(&mut self, owner: &str) -> bool {
        if let Some(pending) = self.compactions.get_mut(owner) {
            pending.stop_requested = true;
            return true;
        }
        false
    }

    pub fn take_compaction_stop(&mut self, owner: &str) -> bool {
        if self.active_turn(owner).is_none() {
            return false;
        }
        self.compactions
            .get_mut(owner)
            .is_some_and(|pending| std::mem::take(&mut pending.stop_requested))
    }

    pub(super) fn observe_compaction(&mut self, owner: &str, method: &str, params: &Value) {
        let Some(pending) = self.compactions.get_mut(owner) else {
            return;
        };
        let turn = if method.starts_with("turn/") {
            params["turn"]["id"].as_str()
        } else {
            params["turnId"].as_str()
        };
        let Some(turn) = turn.filter(|turn| !pending.previous_turns.contains(*turn)) else {
            return;
        };
        if method == "turn/started" && pending.turn.is_none() {
            pending.turn = Some(turn.into());
        }
        if matches!(method, "item/started" | "item/completed")
            && params["item"]["type"] == "contextCompaction"
        {
            if pending.turn.is_none() {
                pending.turn = Some(turn.into());
            }
            if pending.turn.as_deref() == Some(turn) && method == "item/completed" {
                pending.completed_item = true;
            }
        }
        if pending.turn.as_deref() == Some(turn) && method == "turn/completed" {
            let status = params["turn"]["status"].as_str();
            let resolved = matches!(status, Some("failed" | "interrupted"))
                || status == Some("completed") && pending.completed_item;
            self.compactions.remove(owner);
            if resolved {
                self.saved.unresolved.remove(owner);
            }
            // A completed turn without native compaction evidence is not proof
            // of this operation. Keep its receipt for explicit history review.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> Conversations {
        let root = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        for (owner, id) in [("chat:a", "a"), ("graph:b", "b")] {
            let request = book
                .open(owner, root.path(), &Profile::default(), Access::ReadOnly)
                .unwrap();
            book.complete(&request,Ok(json!({"thread":{"id":id,"status":{"type":"idle"},"turns":[{"id":"previous","status":"completed","items":[]}]}}))).unwrap();
        }
        book
    }
    fn start(book: &mut Conversations) -> Request {
        book.action(
            "chat:a",
            Action::Compact {
                operation_id: "operation-a".into(),
            },
        )
        .unwrap()
    }
    fn turn(book: &mut Conversations, method: &str, status: &str) {
        book.notification(
            method,
            &json!({"threadId":"a","turn":{"id":"compacting","status":status,"items":[]}}),
        )
        .unwrap();
    }
    fn item(book: &mut Conversations) {
        book.notification("item/completed",&json!({"threadId":"a","turnId":"compacting","item":{"id":"compact-item","type":"contextCompaction"}})).unwrap();
    }
    #[test]
    fn compaction_ack_is_not_completion_and_only_its_owner_is_reserved() {
        let mut book = setup();
        let request = start(&mut book);
        assert_eq!(request.call.method, "thread/compact/start");
        assert_eq!(request.call.params, json!({"threadId":"a"}));
        assert!(book.saved().unresolved["chat:a"].compact);
        assert!(book.busy("chat:a"));
        assert!(!book.busy("graph:b"));
        book.complete(&request, Ok(json!({}))).unwrap();
        assert_eq!(book.compaction_status("chat:a"), Some("accepted"));
        assert!(
            book.resolve_delivery_after_user_review("chat:a", "operation-a")
                .is_err()
        );
        turn(&mut book, "turn/started", "inProgress");
        item(&mut book);
        assert!(book.busy("chat:a"));
        turn(&mut book, "turn/completed", "completed");
        assert!(!book.compaction_pending("chat:a"));
        assert!(!book.busy("chat:a"));
        assert!(!book.saved().unresolved.contains_key("chat:a"));
    }
    #[test]
    fn native_compaction_completion_before_ack_never_reopens_the_operation() {
        let mut book = setup();
        let request = start(&mut book);
        turn(&mut book, "turn/started", "inProgress");
        item(&mut book);
        turn(&mut book, "turn/completed", "completed");
        book.complete(&request, Ok(json!({}))).unwrap();
        assert!(!book.compaction_pending("chat:a"));
        assert!(!book.busy("chat:a"));
    }
    #[test]
    fn disconnected_compaction_requires_explicit_history_review_never_transcript_matching() {
        let mut book = setup();
        let request = start(&mut book);
        book.complete(&request, Ok(json!({}))).unwrap();
        let saved = serde_json::to_vec(book.saved()).unwrap();
        book.disconnect();
        let mut restored = Conversations::restore(serde_json::from_slice(&saved).unwrap()).unwrap();
        assert!(!restored.compaction_pending("chat:a"));
        assert!(restored.busy("chat:a"));
        assert!(
            restored
                .resolve_delivery_after_user_review("chat:a", "operation-a")
                .is_err()
        );
        let read = restored.action("chat:a", Action::Read).unwrap();
        restored
            .complete(
                &read,
                Ok(json!({"thread":{"id":"a","status":{"type":"notLoaded"},"turns":[]}})),
            )
            .unwrap();
        assert!(restored.saved().unresolved["chat:a"].compact);
        restored
            .resolve_delivery_after_user_review("chat:a", "operation-a")
            .unwrap();
        assert!(!restored.busy("chat:a"));
    }
    #[test]
    fn rejected_compaction_clears_receipt_but_malformed_ack_remains_uncertain() {
        let mut book = setup();
        let request = start(&mut book);
        assert!(
            book.complete(&request, Err(CallError::Rejected("Not sent".into())))
                .is_err()
        );
        assert!(!book.compaction_pending("chat:a"));
        assert!(!book.busy("chat:a"));
        let request = start(&mut book);
        assert!(
            book.complete(&request, Ok(json!({"unknown":true})))
                .is_err()
        );
        assert!(!book.compaction_pending("chat:a"));
        assert!(book.saved().unresolved["chat:a"].compact);
    }
    #[test]
    fn compaction_stop_waits_for_native_turn_and_is_consumed_once() {
        let mut book = setup();
        let request = start(&mut book);
        book.complete(&request, Ok(json!({}))).unwrap();
        assert!(book.request_compaction_stop("chat:a"));
        assert!(!book.take_compaction_stop("chat:a"));
        turn(&mut book, "turn/started", "inProgress");
        assert!(book.take_compaction_stop("chat:a"));
        assert!(!book.take_compaction_stop("chat:a"));
        let interrupt = book.action("chat:a", Action::Interrupt).unwrap();
        assert_eq!(interrupt.call.params["turnId"], "compacting");
        book.complete(&interrupt, Ok(json!({}))).unwrap();
        turn(&mut book, "turn/completed", "interrupted");
        assert!(!book.busy("chat:a"));
    }
    #[test]
    fn stale_or_foreign_completion_cannot_resolve_compaction_and_empty_sessions_are_ineligible() {
        let mut book = setup();
        let request = start(&mut book);
        book.complete(&request, Ok(json!({}))).unwrap();
        for id in ["a", "foreign", "b"] {
            book.notification(
                "turn/completed",
                &json!({"threadId":id,"turn":{"id":"previous","status":"completed","items":[]}}),
            )
            .unwrap();
        }
        assert!(book.compaction_pending("chat:a"));
        let legacy: Receipt =
            serde_json::from_value(json!({"threadId":"a","messageId":"old","steer":false}))
                .unwrap();
        assert!(!legacy.compact);
        let mut invalid = book.saved().clone();
        invalid.unresolved.get_mut("chat:a").unwrap().review = true;
        assert!(invalid.validate().is_err());
        book.notification("thread/closed", &json!({"threadId":"a"}))
            .unwrap();
        assert!(!book.compaction_pending("chat:a"));
        assert!(book.saved().unresolved.contains_key("chat:a"));
        let mut empty = setup();
        empty.mirror.forget("a");
        assert!(!empty.can_compact("chat:a"));
    }
}
