//! Bounded hydration for the stable paginated Codex thread history API.
//! Pages are display-only snapshots and are never sent back as model context.
use super::*;
use std::collections::{BTreeMap, HashSet};

const MAX_PAGE_TURNS: usize = 64;
const MAX_HISTORY_TURNS: usize = 4096;
const MAX_HISTORY_PAGES: usize = 512;
const MAX_CURSOR_BYTES: usize = 16 * 1024;

#[derive(Default)]
pub(super) struct Pages {
    pending: BTreeMap<String, Pending>,
    sequence: u64,
}

struct Pending {
    sequence: u64,
    thread: String,
    expected_cursor: Option<String>,
    seen_cursors: HashSet<String>,
    seen_turns: HashSet<String>,
    turns: Vec<Value>,
    pages: usize,
    requested_at: u64,
}

pub(super) enum Effect {
    Continue {
        thread: String,
        cursor: String,
        call: Call,
    },
    Complete {
        thread: String,
        turns: Vec<Value>,
        requested_at: u64,
    },
}

impl Pages {
    pub(super) fn busy(&self, owner: &str) -> bool {
        self.pending.contains_key(owner)
    }

    pub(super) fn any_busy(&self) -> bool {
        !self.pending.is_empty()
    }

    pub(super) fn sequence(&self, owner: &str) -> Option<u64> {
        self.pending.get(owner).map(|p| p.sequence)
    }

    pub(super) fn begin(
        &mut self,
        owner: &str,
        thread: &str,
        requested_at: u64,
    ) -> Result<Call, String> {
        if owner.is_empty() || thread.is_empty() || self.pending.contains_key(owner) {
            return Err("Native history is already loading for this conversation".into());
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or("Native history sequence exhausted")?;
        self.pending.insert(
            owner.into(),
            Pending {
                sequence: self.sequence,
                thread: thread.into(),
                expected_cursor: None,
                seen_cursors: HashSet::new(),
                seen_turns: HashSet::new(),
                turns: Vec::new(),
                pages: 0,
                requested_at,
            },
        );
        Ok(api::list_thread_turns(thread, None))
    }

    pub(super) fn accept(
        &mut self,
        owner: &str,
        thread: &str,
        cursor: Option<&str>,
        result: Result<Value, CallError>,
    ) -> Result<Effect, String> {
        let page = result
            .map_err(|error| error.to_string())
            .and_then(parse_page);
        let outcome = (|| {
            let (turns, next_cursor) = page?;
            let pending = self
                .pending
                .get_mut(owner)
                .ok_or("This native history page belongs to an old request")?;
            if pending.thread != thread || pending.expected_cursor.as_deref() != cursor {
                return Err("Native history pagination changed while loading".into());
            }
            pending.pages = pending.pages.saturating_add(1);
            if pending.pages > MAX_HISTORY_PAGES
                || pending.turns.len().saturating_add(turns.len()) > MAX_HISTORY_TURNS
            {
                return Err(
                    "Native history is too large to display safely in one local conversation"
                        .into(),
                );
            }
            for turn in turns {
                let id = turn["id"]
                    .as_str()
                    .ok_or("Native history page contains a turn without an ID")?;
                if !pending.seen_turns.insert(id.into()) {
                    return Err("Native history pagination repeated a turn".into());
                }
                pending.turns.push(turn);
            }
            if let Some(next_cursor) = next_cursor {
                if !pending.seen_cursors.insert(next_cursor.clone()) {
                    return Err("Native history pagination repeated a cursor".into());
                }
                pending.expected_cursor = Some(next_cursor.clone());
                Ok(Some(next_cursor))
            } else {
                Ok(None)
            }
        })();

        match outcome {
            Err(error) => {
                self.pending.remove(owner);
                Err(error)
            }
            Ok(Some(cursor)) => Ok(Effect::Continue {
                thread: thread.into(),
                call: api::list_thread_turns(thread, Some(&cursor)),
                cursor,
            }),
            Ok(None) => {
                let pending = self.pending.remove(owner).unwrap();
                Ok(Effect::Complete {
                    thread: pending.thread,
                    turns: pending.turns,
                    requested_at: pending.requested_at,
                })
            }
        }
    }

    pub(super) fn clear(&mut self) {
        self.pending.clear();
    }

    pub(super) fn cancel(&mut self, owner: &str) {
        self.pending.remove(owner);
    }
}

pub(super) fn parse_page(value: Value) -> Result<(Vec<Value>, Option<String>), String> {
    let turns = value
        .get("data")
        .and_then(Value::as_array)
        .ok_or("Native history page omitted its turn list")?;
    if turns.len() > MAX_PAGE_TURNS {
        return Err("Native history page exceeded the requested bound".into());
    }
    for turn in turns {
        let id = turn
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty() && id.len() <= MAX_CURSOR_BYTES && !id.contains('\0'));
        if id.is_none()
            || !turn.get("items").is_some_and(Value::is_array)
            || !turn.get("status").is_some_and(Value::is_string)
        {
            return Err("Native history page contains an invalid turn".into());
        }
    }
    let next_cursor = match value.get("nextCursor") {
        Some(Value::Null) => None,
        Some(Value::String(cursor))
            if !cursor.is_empty() && cursor.len() <= MAX_CURSOR_BYTES && !cursor.contains('\0') =>
        {
            Some(cursor.clone())
        }
        _ => return Err("Native history pagination is incomplete".into()),
    };
    Ok((turns.clone(), next_cursor))
}

impl BrowserApp {
    pub(super) fn schedule_requested_native_history(
        &mut self,
        request: &ThreadRequest,
        owner: &str,
    ) -> Result<(), String> {
        if request.call.method == "thread/section/move" {
            let read = self
                .app_server
                .conversations
                .action(owner, ThreadAction::Read)?;
            self.dispatch_app_server_thread(read);
            return Ok(());
        }
        if !request.requests_history_hydration() || !request.omits_turns() {
            return Ok(());
        }
        match self.app_server.conversations.history_mode(owner) {
            Some("paginated") => self.begin_paginated_native_history(owner),
            Some("legacy") => {
                let next = self
                    .app_server
                    .conversations
                    .action(owner, ThreadAction::Read)?;
                if next.omits_turns() {
                    return Err("Legacy Codex history did not select full hydration".into());
                }
                if self.app_server.chat_reload.matches(request) {
                    self.app_server.chat_reload.start(&next);
                }
                self.dispatch_app_server_thread(next);
                Ok(())
            }
            _ => Err("Codex did not report a supported conversation history mode".into()),
        }
    }

    fn begin_paginated_native_history(&mut self, owner: &str) -> Result<(), String> {
        let binding = self
            .app_server
            .conversations
            .binding(owner)
            .ok_or("The native conversation was unlinked before history loading began")?;
        let thread = binding.thread_id.clone();
        let requested_at = self.app_server.conversations.mirror.revision();
        let call = self
            .app_server
            .history_pages
            .begin(owner, &thread, requested_at)?;
        let sequence = self
            .app_server
            .history_pages
            .sequence(owner)
            .ok_or("Missing history request identity")?;
        self.dispatch_native_history_page(owner.into(), thread, sequence, None, call);
        Ok(())
    }

    fn dispatch_native_history_page(
        &mut self,
        owner: String,
        thread: String,
        sequence: u64,
        cursor: Option<String>,
        call: Call,
    ) {
        let Some(client) = self.app_server.client.clone() else {
            self.native_history_page_reply(
                &owner,
                &thread,
                sequence,
                cursor.as_deref(),
                Err(CallError::Rejected(
                    "App Server disconnected before native history paging".into(),
                )),
            );
            return;
        };
        let epoch = self.app_server.epoch;
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = call.send(&client).and_then(|ticket| ticket.wait());
            let _ = proxy.send_event(BrowserEvent::AppServer(Event::HistoryPageReply {
                epoch,
                owner,
                thread,
                sequence,
                cursor,
                result,
            }));
        });
    }

    pub(super) fn native_history_page_reply(
        &mut self,
        owner: &str,
        thread: &str,
        sequence: u64,
        cursor: Option<&str>,
        result: Result<Value, CallError>,
    ) {
        // Cancellation/restart can reuse the same owner, thread and cursor.
        // A late page must not be mistaken for the new post-revert sequence.
        if self.app_server.history_pages.sequence(owner) != Some(sequence) {
            return;
        }
        match self
            .app_server
            .history_pages
            .accept(owner, thread, cursor, result)
        {
            Ok(Effect::Continue {
                thread,
                cursor,
                call,
            }) => self.dispatch_native_history_page(
                owner.into(),
                thread,
                sequence,
                Some(cursor),
                call,
            ),
            Ok(Effect::Complete {
                thread,
                turns,
                requested_at,
            }) => {
                let mut reloaded = false;
                match self.app_server.conversations.hydrate_paginated_history(
                    owner,
                    &thread,
                    &turns,
                    requested_at,
                ) {
                    // Hydration can reconcile an uncertain delivery receipt by
                    // its exact native clientId. Persist that authority change
                    // before telling the WebView it is resolved; otherwise a
                    // crash can resurrect the old warning from disk.
                    Ok(()) => match self.save_app_server_bindings() {
                        Ok(()) => {
                            reloaded = true;
                            self.reconcile_native_delegates(owner);
                            self.emit_app_server_conversation(owner, "history_hydrated");
                        }
                        Err(error) => self.emit_native_history_page_error(owner, error),
                    },
                    Err(error) => self.emit_native_history_page_error(owner, error),
                }
                self.app_server
                    .chat_reload
                    .page_reply(owner, sequence, reloaded);
                self.refresh_completed_native_review(owner);
            }
            Err(error) => {
                self.app_server
                    .chat_reload
                    .page_reply(owner, sequence, false);
                self.emit_native_history_page_error(owner, error);
            }
        }
    }

    fn emit_native_history_page_error(&self, owner: &str, error: String) {
        self.conversation_event(
            "central-agent:app-server-conversation",
            json!({
                "owner":owner,
                "error":error,
                "nativeConversation":self.app_server_conversation_view(owner)
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn post_revert_read_has_a_new_identity_even_for_the_same_initial_cursor() {
        let mut pages = Pages::default();
        pages.begin("chat:a", "thread-a", 7).unwrap();
        let old = pages.sequence("chat:a").unwrap();
        pages.cancel("chat:a");
        pages.begin("chat:a", "thread-a", 9).unwrap();
        assert_ne!(pages.sequence("chat:a"), Some(old));
        pages.clear();
        pages.begin("chat:a", "thread-a", 10).unwrap();
        assert!(pages.sequence("chat:a").unwrap() > old);
    }

    #[test]
    fn pages_are_bounded_ordered_and_require_unique_cursors_and_turns() {
        let mut pages = Pages::default();
        let call = pages.begin("chat:a", "thread-a", 7).unwrap();
        assert_eq!(call.method, "thread/turns/list");
        assert!(pages.busy("chat:a"));
        let effect = pages
            .accept(
                "chat:a",
                "thread-a",
                None,
                Ok(json!({
                    "data":[{"id":"turn-a","status":"completed","items":[],"itemsView":"summary"}],
                    "nextCursor":"next"
                })),
            )
            .unwrap();
        assert!(matches!(effect, Effect::Continue { cursor, .. } if cursor == "next"));
        let effect = pages
            .accept(
                "chat:a",
                "thread-a",
                Some("next"),
                Ok(json!({
                    "data":[{"id":"turn-b","status":"completed","items":[],"itemsView":"summary"}],
                    "nextCursor":null
                })),
            )
            .unwrap();
        match effect {
            Effect::Complete {
                thread,
                turns,
                requested_at,
            } => {
                assert_eq!(thread, "thread-a");
                assert_eq!(requested_at, 7);
                assert_eq!(turns[0]["id"], "turn-a");
                assert_eq!(turns[1]["id"], "turn-b");
            }
            Effect::Continue { .. } => panic!("history should be complete"),
        }
        assert!(!pages.any_busy());
    }

    #[test]
    fn malformed_or_repeated_pages_clear_busy_state() {
        let mut pages = Pages::default();
        pages.begin("chat:a", "thread-a", 0).unwrap();
        assert!(
            pages
                .accept(
                    "chat:a",
                    "thread-a",
                    None,
                    Ok(json!({"data":[],"nextCursor":"same"})),
                )
                .is_ok()
        );
        assert!(
            pages
                .accept(
                    "chat:a",
                    "thread-a",
                    Some("same"),
                    Ok(json!({"data":[],"nextCursor":"same"})),
                )
                .is_err()
        );
        assert!(!pages.busy("chat:a"));

        pages.begin("chat:a", "thread-a", 0).unwrap();
        assert!(
            pages
                .accept(
                    "chat:a",
                    "thread-a",
                    None,
                    Ok(json!({"data":[{"id":"bad"}],"nextCursor":null})),
                )
                .is_err()
        );
        assert!(!pages.busy("chat:a"));
    }
}
