//! Read back native review completion. The runtime can report a worker turn
//! with a different ID from review/start; only native history can close that
//! projected turn. Never synthesize terminal status from a review item or idle.
use super::*;

pub(super) struct Refresh {
    turn: String,
    dirty: bool,
}

impl Conversations {
    pub(super) fn observe_review_refresh(&mut self, owner: &str, method: &str, params: &Value) {
        if method == "item/completed"
            && params["item"]["type"] == "exitedReviewMode"
            && let Some(turn) = params["turnId"].as_str().filter(|id| !id.is_empty())
        {
            self.review_refreshes.insert(
                owner.into(),
                Refresh {
                    turn: turn.into(),
                    dirty: true,
                },
            );
        } else if (method == "thread/status/changed" && params["status"]["type"] == "idle")
            || method == "turn/completed"
        {
            if let Some(refresh) = self.review_refreshes.get_mut(owner) {
                refresh.dirty = true;
            } else if method == "turn/completed"
                && params["turn"]["items"].as_array().is_some_and(|items| {
                    items.iter().any(|item| item["type"] == "exitedReviewMode")
                })
                && let Some(turn) = params["turn"]["id"].as_str().filter(|id| !id.is_empty())
            {
                self.review_refreshes.insert(
                    owner.into(),
                    Refresh {
                        turn: turn.into(),
                        dirty: true,
                    },
                );
            }
        }
    }

    /// Drain after native notifications and responses. Coalesce events while a
    /// read is in flight; another attempt requires another native event or an
    /// explicit user read, not elapsed time or an automatic inference retry.
    pub fn take_review_refresh(&mut self, owner: &str) -> Result<Option<Request>, String> {
        if self
            .pending
            .values()
            .any(|request| request.owner == owner && matches!(request.kind, Kind::Read))
        {
            return Ok(None);
        }
        let Some(refresh) = self
            .review_refreshes
            .get_mut(owner)
            .filter(|refresh| refresh.dirty)
        else {
            return Ok(None);
        };
        refresh.dirty = false;
        self.action(owner, Action::Read).map(Some)
    }

    pub(super) fn reconcile_review_refresh(&mut self, owner: &str) {
        let Some(refresh) = self.review_refreshes.get(owner) else {
            return;
        };
        let confirmed = self
            .binding(owner)
            .and_then(|binding| self.mirror.thread(&binding.thread_id))
            .is_some_and(|thread| {
                thread.active_turn().is_none()
                    && thread.status.as_ref().is_some_and(|status| {
                        matches!(status["type"].as_str(), Some("idle" | "notLoaded"))
                    })
                    && thread.turns.iter().any(|turn| {
                        turn.id == refresh.turn
                            && matches!(
                                turn.status.as_str(),
                                "completed" | "failed" | "interrupted"
                            )
                    })
            });
        if confirmed {
            self.review_refreshes.remove(owner);
        }
    }
}
