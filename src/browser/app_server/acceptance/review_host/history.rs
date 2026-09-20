//! Acceptance oracle only. Native live items need not survive thread/read.
//! Every extra projected identity must have been received in this owned thread.
use super::*;
use central_agent_codex_runtime::mirror::Turn;

#[derive(Default)]
pub(super) struct Evidence(BTreeMap<(String, String), String>);

impl Evidence {
    fn item(&mut self, turn: &str, item: &Value) {
        if let (Some(id), Some(kind)) = (item["id"].as_str(), item["type"].as_str()) {
            self.0.insert((turn.into(), id.into()), kind.into());
        }
    }

    fn turn(&mut self, turn: &Value) {
        if let Some(id) = turn["id"].as_str() {
            for item in turn["items"].as_array().into_iter().flatten() {
                self.item(id, item);
            }
        }
    }

    pub(super) fn observe(&mut self, owner: &str, thread: Option<&str>, event: &BrowserEvent) {
        let Some(thread) = thread else { return };
        match event {
            BrowserEvent::AppServer(Event::Transport {
                event: TransportEvent::Notification { method, params },
                ..
            }) if params["threadId"] == thread => {
                if matches!(method.as_str(), "item/started" | "item/completed") {
                    if let Some(turn) = params["turnId"].as_str() {
                        self.item(turn, &params["item"]);
                    }
                } else if matches!(method.as_str(), "turn/started" | "turn/completed") {
                    self.turn(&params["turn"]);
                }
            }
            BrowserEvent::AppServer(Event::ConversationReply {
                request,
                result: Ok(value),
                ..
            }) if request.owner == owner && request.call.params["threadId"] == thread => {
                if matches!(request.call.method, "turn/start" | "review/start") {
                    self.turn(&value["turn"]);
                } else if value["thread"]["id"] == thread {
                    for turn in value["thread"]["turns"].as_array().into_iter().flatten() {
                        self.turn(turn);
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn matches(&self, projected: &Turn, native: &Value) -> bool {
        let Some(items) = native["items"].as_array() else {
            return false;
        };
        if native["id"] != projected.id {
            return false;
        }
        let mut next = 0;
        let mut seen = std::collections::HashSet::new();
        for item in &projected.items {
            if !seen.insert(&item.id) {
                return false;
            }
            if let Some(index) = items.iter().position(|value| value["id"] == item.id) {
                if index != next || items[index]["type"] != item.value["type"] {
                    return false;
                }
                next += 1;
            } else if self
                .0
                .get(&(projected.id.clone(), item.id.clone()))
                .is_none_or(|kind| item.value["type"] != *kind)
            {
                return false;
            }
        }
        next == items.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use central_agent_codex_runtime::mirror::Mirror;

    #[test]
    fn history_allows_only_observed_live_extras_and_all_persisted_items_in_order() {
        let native = json!({"id":"review","status":"interrupted","items":[
            {"id":"entry","type":"enteredReviewMode"},
            {"id":"exit","type":"exitedReviewMode"}
        ]});
        let mut mirror = Mirror::default();
        mirror
            .hydrate(&json!({"id":"thread","turns":[native]}), 0)
            .unwrap();
        let extra = json!({"id":"command","type":"commandExecution","status":"inProgress"});
        mirror
            .notify(
                "item/started",
                &json!({"threadId":"thread","turnId":"review","item":extra}),
            )
            .unwrap();
        let projected = &mirror.thread("thread").unwrap().turns[0];
        let mut evidence = Evidence::default();
        assert!(!evidence.matches(projected, &native));
        evidence.item("sibling", &extra);
        assert!(!evidence.matches(projected, &native));
        evidence.item("review", &extra);
        assert!(evidence.matches(projected, &native));
        let mut changed = projected.clone();
        changed.items.swap(0, 1);
        assert!(!evidence.matches(&changed, &native));
        changed = projected.clone();
        changed.items.remove(0);
        assert!(!evidence.matches(&changed, &native));
        changed = projected.clone();
        changed.items[2].value["type"] = json!("fileChange");
        assert!(!evidence.matches(&changed, &native));
        changed = projected.clone();
        changed.items.push(changed.items[0].clone());
        assert!(!evidence.matches(&changed, &native));
        changed = projected.clone();
        changed.items.push(changed.items[2].clone());
        assert!(!evidence.matches(&changed, &native));
    }
}
