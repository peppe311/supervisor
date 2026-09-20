//! Bind native children using explicit App Server ancestry only. This module
//! does not spawn agents, supply prompts, copy transcripts or grant consent.
use super::*;

#[derive(Clone, Debug)]
pub struct Delegation {
    pub parent_owner: String,
    pub parent_thread: String,
    pub child_thread: String,
    generation: u64,
    live: bool,
}

pub(super) fn validate_links(links: &BTreeMap<String, String>) -> Result<(), String> {
    for (child, parent) in links {
        if !lineage_id(child) || !lineage_id(parent) || child == parent {
            return Err("Invalid native delegation ancestry".into());
        }
        let mut seen = HashSet::from([child.as_str()]);
        let mut cursor = parent.as_str();
        loop {
            if !seen.insert(cursor) {
                return Err("Cyclic native delegation ancestry".into());
            }
            let Some(next) = links.get(cursor) else { break };
            cursor = next;
        }
    }
    Ok(())
}

impl Conversations {
    fn delegation(&self, parent: &str, child: &str, live: bool) -> Option<Delegation> {
        if !lineage_id(parent)
            || !lineage_id(child)
            || parent == child
            || self.owner_for_thread(child).is_some()
        {
            return None;
        }
        let owner = self.owner_for_thread(parent)?;
        let binding = self.binding(owner)?;
        if binding.deleted || binding.archived {
            return None;
        }
        Some(Delegation {
            parent_owner: owner.into(),
            parent_thread: parent.into(),
            child_thread: child.into(),
            generation: self.generation,
            live,
        })
    }

    fn item_delegations(&self, parent: &str, item: &Value, live: bool) -> Vec<Delegation> {
        if item["type"] != "collabAgentToolCall"
            || item["tool"] != "spawnAgent"
            || item["status"] != "completed"
            || item["senderThreadId"] != parent
        {
            return vec![];
        }
        item["receiverThreadIds"]
            .as_array()
            .into_iter()
            .flatten()
            .take(64)
            .filter_map(Value::as_str)
            .filter_map(|child| self.delegation(parent, child, live))
            .collect()
    }

    /// A completed spawn is authoritative even if no child thread/started is
    /// sent. Its event precedes child turns in the native 0.153.4 stream.
    pub fn discover_delegations(&self, method: &str, params: &Value) -> Vec<Delegation> {
        if method == "thread/started" {
            let thread = &params["thread"];
            return thread["source"]["subagent"]["thread_spawn"]["parent_thread_id"]
                .as_str()
                .zip(thread["id"].as_str())
                .and_then(|(parent, child)| self.delegation(parent, child, true))
                .into_iter()
                .collect();
        }
        if method != "item/completed" {
            return vec![];
        }
        params["threadId"]
            .as_str()
            .map(|parent| self.item_delegations(parent, &params["item"], true))
            .unwrap_or_default()
    }

    /// Reconcile children from native history read on this connection. A
    /// historical spawn never claims the child is still running or subscribed.
    pub fn historical_delegations(&self, owner: &str) -> Vec<Delegation> {
        if !self.observed(owner) {
            return vec![];
        }
        let Some(binding) = self.binding(owner) else {
            return vec![];
        };
        let Some(thread) = self.mirror.thread(&binding.thread_id) else {
            return vec![];
        };
        thread
            .turns
            .iter()
            .flat_map(|turn| turn.items.iter())
            .flat_map(|item| self.item_delegations(&binding.thread_id, &item.value, false))
            .collect()
    }

    /// The host must allocate a durable local destination first, then persist
    /// Saved before publishing the child to its controls or accepting requests.
    pub fn adopt_delegation(
        &mut self,
        candidate: &Delegation,
        destination: &str,
    ) -> Result<(), String> {
        if candidate.generation != self.generation
            || !valid_owner(destination)
            || self.binding(destination).is_some()
            || self.saved.fork_origins.contains_key(destination)
            || self
                .delegation(
                    &candidate.parent_thread,
                    &candidate.child_thread,
                    candidate.live,
                )
                .is_none_or(|fresh| fresh.parent_owner != candidate.parent_owner)
        {
            return Err("The native delegation no longer matches its owner".into());
        }
        let mut links = self.saved.delegations.clone();
        links.insert(
            candidate.child_thread.clone(),
            candidate.parent_thread.clone(),
        );
        validate_links(&links)?;
        self.saved.delegations = links;
        self.saved.bindings.insert(
            destination.into(),
            Binding {
                thread_id: candidate.child_thread.clone(),
                session_id: None,
                archived: false,
                deleted: false,
                never_submitted: false,
            },
        );
        if candidate.live {
            self.loaded.insert(candidate.child_thread.clone());
            self.observed.insert(candidate.child_thread.clone());
            self.initializing_delegates
                .insert(candidate.child_thread.clone());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn book() -> Conversations {
        let mut saved = Saved::default();
        saved.bindings.insert(
            "chat:parent".into(),
            Binding {
                thread_id: "parent".into(),
                session_id: None,
                archived: false,
                deleted: false,
                never_submitted: false,
            },
        );
        Conversations::restore(saved).unwrap()
    }
    fn spawn() -> Value {
        json!({"threadId":"parent","turnId":"p-turn","item":{
            "id":"spawn","type":"collabAgentToolCall","tool":"spawnAgent","status":"completed",
            "senderThreadId":"parent","receiverThreadIds":["child"]}})
    }
    #[test]
    fn native_spawn_binds_before_child_events_and_keeps_requests_and_interrupt_on_child() {
        let mut book = book();
        let candidate = book
            .discover_delegations("item/completed", &spawn())
            .remove(0);
        book.adopt_delegation(&candidate, "chat:child").unwrap();
        assert!(book.busy("chat:child"));
        assert!(!book.ready_for_turn("chat:child"));
        assert_eq!(book.owner_for_thread("child"), Some("chat:child"));
        assert!(
            book.discover_delegations("item/completed", &spawn())
                .is_empty()
        );
        book.notification("turn/started", &json!({"threadId":"child","turn":{"id":"child-turn","status":"inProgress","items":[]}})).unwrap();
        let interrupt = book.action("chat:child", Action::Interrupt).unwrap();
        assert_eq!(
            interrupt.call.params,
            json!({"threadId":"child","turnId":"child-turn"})
        );
        assert_eq!(book.active_turn("chat:parent"), None);
        assert_eq!(
            book.saved.delegations.get("child").map(String::as_str),
            Some("parent")
        );
        assert!(book.saved.lineage.is_empty());
        let restored = Conversations::restore(
            serde_json::from_value(serde_json::to_value(book.saved()).unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(restored.owner_for_thread("child"), Some("chat:child"));
        assert!(!restored.ready_for_turn("chat:child"));
        assert!(!restored.is_thread_loaded("chat:child"));
    }
    #[test]
    fn unowned_failed_started_retargeted_and_stale_spawns_cannot_claim_children() {
        let mut book = book();
        for (field, value) in [
            ("status", "inProgress"),
            ("status", "failed"),
            ("tool", "sendInput"),
            ("senderThreadId", "unowned"),
        ] {
            let mut params = spawn();
            params["item"][field] = json!(value);
            assert!(
                book.discover_delegations("item/completed", &params)
                    .is_empty()
            );
        }
        assert!(
            book.discover_delegations("item/started", &spawn())
                .is_empty()
        );
        let mut params = spawn();
        params["threadId"] = json!("unowned");
        assert!(
            book.discover_delegations("item/completed", &params)
                .is_empty()
        );
        let candidate = book
            .discover_delegations("item/completed", &spawn())
            .remove(0);
        assert!(book.adopt_delegation(&candidate, "chat:parent").is_err());
        book.disconnect();
        assert!(book.adopt_delegation(&candidate, "chat:child").is_err());
    }
    #[test]
    fn explicit_native_source_and_historical_spawn_are_supported_without_fork_guessing() {
        let mut book = book();
        let thread = json!({"thread":{"id":"child","forkedFromId":"parent","source":"vscode"}});
        assert!(
            book.discover_delegations("thread/started", &thread)
                .is_empty()
        );
        let mut thread = thread;
        thread["thread"]["source"] =
            json!({"subagent":{"thread_spawn":{"parent_thread_id":"parent","depth":1}}});
        assert_eq!(
            book.discover_delegations("thread/started", &thread).len(),
            1
        );
        book.notification("thread/started", &json!({"thread":{"id":"parent","turns":[{"id":"old","status":"completed","items":[spawn()["item"]]}]}})).unwrap();
        let candidate = book.historical_delegations("chat:parent").remove(0);
        book.adopt_delegation(&candidate, "graph:node:child")
            .unwrap();
        assert!(!book.busy("graph:node:child"));
        assert!(!book.is_thread_loaded("graph:node:child"));
        assert!(
            validate_links(&BTreeMap::from([
                ("a".into(), "b".into()),
                ("b".into(), "a".into())
            ]))
            .is_err()
        );
    }
}
