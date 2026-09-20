//! On-demand, read-only expansion of one persisted turn. No resume or model turn.
use super::*;
use central_agent_codex_runtime::mirror::Turn;

#[derive(Default)]
pub(super) struct History {
    sequence: u64,
    states: BTreeMap<(String, String, String), Load>,
}
struct Load {
    request: u64,
    revision: u64,
    pending: bool,
    error: Option<String>,
}
impl History {
    pub(super) fn view(&self, owner: &str, thread: &str, turn: &Turn) -> Value {
        let load = self
            .states
            .get(&(owner.into(), thread.into(), turn.id.clone()));
        let state = if turn.active() {
            "live"
        } else if turn.items_view.as_deref() == Some("full") {
            "loaded"
        } else if load.is_some_and(|s| s.pending) {
            "loading"
        } else if load.is_some_and(|s| s.error.is_some()) {
            "error"
        } else {
            "unloaded"
        };
        json!({"owner":owner,"threadId":thread,"turnId":turn.id,"state":state,
            "error":load.and_then(|s| s.error.as_deref())})
    }
    pub(super) fn disconnect(&mut self) {
        for state in self.states.values_mut().filter(|s| s.pending) {
            state.pending = false;
            state.error = Some("Connessione interrotta. Riconnetti Codex e riprova.".into());
        }
    }
}

// Seek using summary pages, then obtain the provider's cursor immediately
// before the requested turn. At most one turn's full items are ever fetched.
fn read_turn(
    mut call: impl FnMut(Call) -> Result<Value, String>,
    thread: &str,
    turn: &str,
) -> Result<Value, String> {
    let mut cursor: Option<String> = None;
    let mut seen = HashSet::new();
    for _ in 0..256 {
        let (turns, next) =
            history_pages::parse_page(call(api::list_thread_turns(thread, cursor.as_deref()))?)?;
        if let Some(offset) = turns.iter().position(|t| t["id"] == turn) {
            if offset > 0 {
                let (prefix, after) = history_pages::parse_page(call(
                    api::list_thread_turns_view(thread, cursor.as_deref(), offset as u32, false),
                )?)?;
                if prefix.len() != offset
                    || prefix.iter().zip(&turns).any(|(a, b)| a["id"] != b["id"])
                {
                    return Err("La cronologia è cambiata durante il caricamento. Riprova.".into());
                }
                cursor = Some(
                    after
                        .ok_or("La posizione del turno non è più disponibile. Ricarica la chat.")?,
                );
            }
            let (mut full, _) = history_pages::parse_page(call(api::list_thread_turns_view(
                thread,
                cursor.as_deref(),
                1,
                true,
            ))?)?;
            if full.len() != 1 || full[0]["id"] != turn || full[0]["itemsView"] != "full" {
                return Err(
                    "I dettagli ricevuti appartengono a un altro turno. Ricarica la chat.".into(),
                );
            }
            if full[0]["items"]
                .as_array()
                .is_none_or(|items| items.len() > 16384)
            {
                return Err(
                    "Questo turno contiene troppe attività per essere mostrato interamente.".into(),
                );
            }
            return Ok(full.remove(0));
        }
        let Some(next) = next else {
            return Err("Il turno non è più presente nella cronologia.".into());
        };
        if !seen.insert(next.clone()) {
            return Err("La cronologia ha ripetuto una pagina.".into());
        }
        cursor = Some(next);
    }
    Err("La ricerca del turno ha raggiunto il limite della cronologia.".into())
}

impl BrowserApp {
    pub(super) fn read_native_work_history(
        &mut self,
        owner: &str,
        thread: &str,
        turn: &str,
    ) -> Result<(), String> {
        let book = &self.app_server.conversations;
        let selected = book
            .mirror
            .thread(thread)
            .and_then(|t| t.turns.iter().find(|t| t.id == turn))
            .ok_or("Il turno non appartiene alla cronologia caricata.")?;
        if selected.active() {
            return Err("Il turno è ancora in corso.".into());
        }
        if selected.items_view.as_deref() == Some("full") {
            return Ok(());
        }
        let key = (owner.to_owned(), thread.to_owned(), turn.to_owned());
        if self
            .app_server
            .work_history
            .states
            .get(&key)
            .is_some_and(|s| s.pending)
        {
            return Ok(());
        }
        self.app_server.work_history.sequence = self
            .app_server
            .work_history
            .sequence
            .checked_add(1)
            .ok_or("Troppe richieste di cronologia")?;
        let request = self.app_server.work_history.sequence;
        let mut load = Load {
            request,
            revision: book.mirror.revision(),
            pending: false,
            error: None,
        };
        if !self.app_server.view.connected || self.app_server.client.is_none() {
            load.error = Some("Riconnetti Codex per caricare le attività di questo turno.".into());
        } else if self
            .app_server
            .work_history
            .states
            .values()
            .filter(|s| s.pending)
            .count()
            >= 4
        {
            load.error = Some("Altre attività sono in caricamento. Riprova tra poco.".into());
        } else {
            load.pending = true;
        }
        let pending = load.pending;
        self.app_server.work_history.states.insert(key, load);
        self.emit_app_server_conversation(owner, "work_history_loading");
        if pending {
            let client = self
                .app_server
                .client
                .clone()
                .ok_or("Connessione non disponibile")?;
            let proxy = self.proxy.clone();
            let (owner, thread, turn) = (owner.to_owned(), thread.to_owned(), turn.to_owned());
            let epoch = self.app_server.epoch;
            std::thread::spawn(move || {
                let result = read_turn(
                    |call| {
                        call.send(&client)
                            .and_then(|ticket| ticket.wait())
                            .map_err(|e| e.to_string())
                    },
                    &thread,
                    &turn,
                );
                let _ = proxy.send_event(BrowserEvent::AppServer(Event::WorkHistoryReply {
                    epoch,
                    owner,
                    thread,
                    turn,
                    request,
                    result,
                }));
            });
        }
        Ok(())
    }

    pub(super) fn native_work_history_reply(
        &mut self,
        owner: &str,
        thread: &str,
        turn: &str,
        request: u64,
        result: Result<Value, String>,
    ) {
        let key = (owner.to_owned(), thread.to_owned(), turn.to_owned());
        let Some(load) = self.app_server.work_history.states.get(&key) else {
            return;
        };
        if load.request != request || !load.pending {
            return;
        }
        if self
            .app_server
            .conversations
            .binding(owner)
            .is_none_or(|b| b.deleted || b.thread_id != thread)
        {
            self.app_server.work_history.states.remove(&key);
            return;
        }
        let revision = load.revision;
        let result = result.and_then(|value| {
            self.app_server
                .conversations
                .mirror
                .hydrate_turn_detail(thread, &value, revision)
        });
        if let Some(load) = self.app_server.work_history.states.get_mut(&key) {
            load.pending = false;
            load.error = result.err();
        }
        self.emit_app_server_conversation(owner, "work_history_loaded");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page(ids: &[&str], next: Option<&str>) -> Value {
        json!({"data":ids.iter().map(|id| json!({"id":id,"status":"completed","items":[],"itemsView":"summary"})).collect::<Vec<_>>(),"nextCursor":next})
    }
    #[test]
    fn seeks_with_provider_cursors_and_fetches_only_selected_full_turn() {
        let mut calls = Vec::new();
        let result = read_turn(|call| {
            calls.push(call.params.clone());
            Ok(match calls.len() {
                1 => page(&["a", "b"], Some("page-2")),
                2 => page(&["c", "wanted", "e"], None),
                3 => page(&["c"], Some("before-wanted")),
                _ => json!({"data":[{"id":"wanted","itemsView":"full","status":"completed","items":[{"id":"tool","type":"commandExecution"}]}],"nextCursor":"after"}),
            })
        }, "thread", "wanted").unwrap();
        assert_eq!(result["id"], "wanted");
        assert_eq!(calls[2]["cursor"], "page-2");
        assert_eq!(calls[2]["limit"], 1);
        assert_eq!(calls[3]["cursor"], "before-wanted");
        assert_eq!(calls.iter().filter(|c| c["itemsView"] == "full").count(), 1);
        assert_eq!(calls[3]["limit"], 1);
    }
    #[test]
    fn rejects_changed_turns_and_repeated_cursors() {
        assert!(read_turn(|_| Ok(page(&["other"], Some("repeat"))), "thread", "wanted").is_err());
        let mut count = 0;
        assert!(read_turn(|_| { count += 1; Ok(if count == 1 { page(&["wanted"], None) }
            else { json!({"data":[{"id":"foreign","itemsView":"full","items":[],"status":"completed"}],"nextCursor":null}) }) }, "thread", "wanted").is_err());
    }
}
