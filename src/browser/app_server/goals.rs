//! Owner-scoped native goal controls. Codex owns state/accounting and any work.
use super::*;
use central_agent_codex_runtime::goals::{self as native, Edit, Goal};

fn same_directory(a: &Path, b: &Path) -> bool {
    a == b
        || fs::canonicalize(a)
            .ok()
            .zip(fs::canonicalize(b).ok())
            .is_some_and(|(a, b)| a == b)
}

fn serialize_directory<S>(directory: &Path, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&display_path(directory))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Value {
        let fixtures: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/crates/central-agent-codex-runtime/tests/native-goals.json"
        )))
        .unwrap();
        fixtures[1]["params"].clone()
    }
    fn read(state: &mut Goals, owner: &str, root: &Path, value: Value) {
        let (id, call) = state
            .action(owner, "native-a", root, Action::Refresh {})
            .unwrap();
        assert_eq!(call.method, "thread/goal/get");
        assert!(state.reply(owner, "native-a", &id, Ok(value)).is_none());
    }
    fn change(state: &mut Goals, owner: &str, root: &Path, edit: Edit) -> String {
        let view_id = state.view(owner, "native-a").unwrap().view_id.clone();
        let (id, call) = state
            .action(owner, "native-a", root, Action::Change { view_id, edit })
            .unwrap();
        assert_eq!(call.method, "thread/goal/get");
        assert!(state.writing(owner));
        id
    }
    #[test]
    fn fresh_native_preflight_precedes_the_exact_owner_write() {
        let root = tempfile::tempdir().unwrap();
        let mut state = Goals::default();
        read(&mut state, "chat:a", root.path(), json!({"goal":null}));
        read(&mut state, "graph:b", root.path(), json!({"goal":null}));
        let id = change(
            &mut state,
            "chat:a",
            root.path(),
            Edit::Replace {
                objective: "Finish the project".into(),
                status: native::UserStatus::Paused,
                token_budget: None,
            },
        );
        assert!(
            state
                .reply("graph:b", "native-a", &id, Ok(json!({"goal":null})))
                .is_none()
        );
        let (write, call) = state
            .reply("chat:a", "native-a", &id, Ok(json!({"goal":null})))
            .unwrap();
        assert_eq!(
            call.params,
            json!({"threadId":"native-a","objective":"Finish the project","status":"paused","tokenBudget":null})
        );
        state.reply("chat:a", "native-a", &write, Ok(fixture()));
        assert!(state.view("chat:a", "native-a").unwrap().goal.is_some());
        assert!(state.view("graph:b", "native-a").unwrap().goal.is_none());
        assert!(!state.any_writing());
    }
    #[test]
    fn goal_view_serializes_the_same_display_directory_as_the_conversation() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = fs::canonicalize(temp.path()).unwrap();
        let mut state = Goals::default();
        read(&mut state, "chat:a", &canonical, json!({"goal":null}));
        let value = serde_json::to_value(state.view("chat:a", "native-a").unwrap()).unwrap();
        assert_eq!(value["directory"], display_path(&canonical));
    }
    #[test]
    fn preflight_detects_other_client_changes_without_writing() {
        let root = tempfile::tempdir().unwrap();
        let mut state = Goals::default();
        read(&mut state, "chat:a", root.path(), fixture());
        let id = change(&mut state, "chat:a", root.path(), Edit::Clear {});
        let mut changed = fixture();
        changed["goal"]["objective"] = json!("Another client goal");
        assert!(
            state
                .reply("chat:a", "native-a", &id, Ok(changed))
                .is_none()
        );
        assert!(
            state
                .view("chat:a", "native-a")
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("nothing was written")
        );
        assert!(!state.any_writing());
    }
    #[test]
    fn fresh_notifications_win_over_delayed_read_and_write_replies() {
        let root = tempfile::tempdir().unwrap();
        let mut state = Goals::default();
        read(&mut state, "graph:a", root.path(), fixture());
        let view_id = state.view("graph:a", "native-a").unwrap().view_id.clone();
        let id = change(
            &mut state,
            "graph:a",
            root.path(),
            Edit::Budget {
                token_budget: Some(100),
            },
        );
        let mut updated = fixture();
        updated["threadId"] = json!("native-a");
        updated["goal"]["tokensUsed"] = json!(10);
        state.notify("thread/goal/updated", &updated);
        assert_eq!(state.view("graph:a", "native-a").unwrap().view_id, view_id);
        let (write, call) = state
            .reply("graph:a", "native-a", &id, Ok(fixture()))
            .unwrap();
        assert_eq!(
            call.params,
            json!({"threadId":"native-a","tokenBudget":100})
        );
        updated["goal"]["tokenBudget"] = json!(100);
        updated["goal"]["tokensUsed"] = json!(20);
        state.notify("thread/goal/updated", &updated);
        let mut old_result = updated.clone();
        old_result["goal"]["tokensUsed"] = json!(10);
        state.reply("graph:a", "native-a", &write, Ok(old_result));
        assert_eq!(
            state
                .view("graph:a", "native-a")
                .unwrap()
                .goal
                .as_ref()
                .unwrap()
                .tokens_used,
            20
        );
    }
    #[test]
    fn malformed_notification_and_disconnect_invalidate_pending_edit_authority() {
        let root = tempfile::tempdir().unwrap();
        let mut state = Goals::default();
        for method in ["thread/goal/updated", "thread/closed"] {
            read(&mut state, "chat:a", root.path(), fixture());
            let id = change(&mut state, "chat:a", root.path(), Edit::Clear {});
            state.notify(method, &json!({"threadId":"native-a"}));
            assert!(
                state
                    .reply("chat:a", "native-a", &id, Ok(fixture()))
                    .is_none()
            );
            assert!(!state.view("chat:a", "native-a").unwrap().current);
        }
        read(&mut state, "chat:a", root.path(), fixture());
        let id = change(&mut state, "chat:a", root.path(), Edit::Clear {});
        state.disconnect();
        assert!(
            state
                .reply("chat:a", "native-a", &id, Ok(fixture()))
                .is_none()
        );
        assert!(!state.any_writing());
        assert!(!state.view("chat:a", "native-a").unwrap().current);
    }
    #[test]
    fn boolean_clear_acknowledgement_and_unknown_outcome_are_not_replayed() {
        let root = tempfile::tempdir().unwrap();
        let mut state = Goals::default();
        for cleared in [true, false] {
            read(&mut state, "chat:a", root.path(), fixture());
            let id = change(&mut state, "chat:a", root.path(), Edit::Clear {});
            let (write, call) = state
                .reply("chat:a", "native-a", &id, Ok(fixture()))
                .unwrap();
            assert_eq!(call.method, "thread/goal/clear");
            state.reply("chat:a", "native-a", &write, Ok(json!({"cleared":cleared})));
            assert!(state.view("chat:a", "native-a").unwrap().current);
            assert!(state.view("chat:a", "native-a").unwrap().goal.is_none());
        }
        read(&mut state, "chat:a", root.path(), fixture());
        let id = change(&mut state, "chat:a", root.path(), Edit::Clear {});
        let (write, _) = state
            .reply("chat:a", "native-a", &id, Ok(fixture()))
            .unwrap();
        assert!(
            state
                .reply(
                    "chat:a",
                    "native-a",
                    &write,
                    Err(CallError::Disconnected {
                        reason: "test disconnect".into(),
                        delivery_unknown: true
                    })
                )
                .is_none()
        );
        let view = state.view("chat:a", "native-a").unwrap();
        assert!(!view.current);
        assert!(view.error.as_ref().unwrap().contains("unknown"));
        assert!(view.pending.is_none());
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Action {
    Refresh {},
    Change { view_id: String, edit: Edit },
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct View {
    thread_id: String,
    #[serde(serialize_with = "serialize_directory")]
    directory: PathBuf,
    view_id: String,
    goal: Option<Goal>,
    current: bool,
    busy: bool,
    writing: bool,
    error: Option<String>,
    #[serde(skip)]
    pending: Option<Pending>,
    #[serde(skip)]
    notifications: u64,
}
struct Pending {
    id: String,
    notifications: u64,
    phase: Phase,
}
enum Phase {
    Read,
    Check { edit: Edit, expected: Option<Goal> },
    Write { clear: bool },
}
#[derive(Default)]
pub(super) struct Goals {
    views: BTreeMap<String, View>,
}
fn same(a: &Option<Goal>, b: &Option<Goal>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.same_configuration(b),
        _ => false,
    }
}
impl View {
    fn update(&mut self, goal: Option<Goal>) {
        if !same(&self.goal, &goal) {
            self.view_id = Uuid::new_v4().to_string();
        }
        self.goal = goal;
        self.current = true;
        self.error = None;
    }
    fn call(&mut self, phase: Phase, call: Call) -> (String, Call) {
        let id = Uuid::new_v4().to_string();
        self.writing = !matches!(phase, Phase::Read);
        self.busy = true;
        self.error = None;
        self.pending = Some(Pending {
            id: id.clone(),
            notifications: self.notifications,
            phase,
        });
        (id, call)
    }
}
impl Goals {
    pub(super) fn view(&self, owner: &str, thread: &str) -> Option<&View> {
        self.views.get(owner).filter(|v| v.thread_id == thread)
    }
    pub(super) fn writing(&self, owner: &str) -> bool {
        self.views.get(owner).is_some_and(|v| v.writing)
    }
    pub(super) fn any_writing(&self) -> bool {
        self.views.values().any(|v| v.writing)
    }
    fn action(
        &mut self,
        owner: &str,
        thread: &str,
        directory: &Path,
        action: Action,
    ) -> Result<(String, Call), String> {
        let view = self.views.entry(owner.into()).or_insert_with(|| View {
            thread_id: thread.into(),
            directory: directory.into(),
            view_id: Uuid::new_v4().to_string(),
            goal: None,
            current: false,
            busy: false,
            writing: false,
            error: None,
            pending: None,
            notifications: 0,
        });
        if view.busy {
            return Err("Wait for the native goal operation".into());
        }
        if view.thread_id != thread || !same_directory(&view.directory, directory) {
            if !matches!(action, Action::Refresh {}) {
                return Err(
                    "Refresh the goal after changing its native conversation or directory".into(),
                );
            }
            view.thread_id = thread.into();
            view.directory = directory.into();
            view.goal = None;
            view.current = false;
            view.view_id = Uuid::new_v4().to_string();
        }
        match action {
            Action::Refresh {} => {
                view.current = false;
                Ok(view.call(Phase::Read, native::get(thread)))
            }
            Action::Change { view_id, edit } => {
                if !view.current || view.view_id != view_id {
                    return Err("The goal changed. Refresh and confirm the new state".into());
                }
                if view.goal.is_none() && !matches!(edit, Edit::Replace { .. }) {
                    return Err("There is no native goal to change".into());
                }
                edit.call(thread)?;
                Ok(view.call(
                    Phase::Check {
                        edit,
                        expected: view.goal.clone(),
                    },
                    native::get(thread),
                ))
            }
        }
    }
    fn reply(
        &mut self,
        owner: &str,
        thread: &str,
        id: &str,
        result: Result<Value, CallError>,
    ) -> Option<(String, Call)> {
        let view = self
            .views
            .get_mut(owner)
            .filter(|v| v.thread_id == thread && v.pending.as_ref().is_some_and(|p| p.id == id))?;
        let pending = view.pending.take()?;
        view.busy = false;
        view.writing = false;
        let write = matches!(pending.phase, Phase::Write { .. });
        let result = (|| -> Result<Option<(String, Call)>, String> {
            let value = result.map_err(|e| e.to_string())?;
            let goal = if matches!(pending.phase, Phase::Write { clear: true }) {
                native::decode_clear(&value)?;
                None
            } else {
                native::decode(thread, &value)?
            };
            if let Phase::Check { edit, expected } = pending.phase {
                if !same(&goal, &expected)
                    || view.notifications != pending.notifications
                        && (!view.current || !same(&view.goal, &expected))
                {
                    if view.notifications == pending.notifications {
                        view.update(goal);
                    }
                    return Err("The native goal changed before this edit. Refresh and confirm again; nothing was written".into());
                }
                // This native API has no compare-and-swap. The read narrows races;
                // it cannot promise atomic isolation from other Codex clients.
                let call = edit.call(thread)?;
                let clear = matches!(edit, Edit::Clear {});
                return Ok(Some(view.call(Phase::Write { clear }, call)));
            }
            if view.notifications == pending.notifications {
                view.update(goal);
            }
            Ok(None)
        })();
        match result {
            Ok(next) => next,
            Err(error) => {
                view.current = false;
                view.error = Some(if write {
                    format!(
                        "{error}. Goal update outcome may be unknown; refresh before another change. Nothing is replayed."
                    )
                } else {
                    error
                });
                None
            }
        }
    }
    pub(super) fn disconnect(&mut self) -> Vec<String> {
        self.views.iter_mut().map(|(owner,view)| {view.current=false;view.busy=false;view.writing=false;view.pending=None;view.error=Some("Disconnected. Refresh the native goal after reconnecting; no edit was replayed.".into());owner.clone()}).collect()
    }
    pub(super) fn notify(&mut self, method: &str, params: &Value) -> Vec<String> {
        let Some(thread) = params["threadId"].as_str() else {
            return vec![];
        };
        let invalidate = matches!(
            method,
            "thread/closed" | "thread/deleted" | "thread/archived"
        ) || method == "thread/status/changed"
            && params["status"]["type"] == "notLoaded";
        if !invalidate && !matches!(method, "thread/goal/updated" | "thread/goal/cleared") {
            return vec![];
        }
        let mut owners = vec![];
        for (owner, view) in self.views.iter_mut().filter(|(_, v)| v.thread_id == thread) {
            view.notifications += 1;
            if invalidate {
                view.current = false;
                view.pending = None;
                view.busy = false;
                view.writing = false;
                view.error =
                    Some("The native conversation changed. Resume and refresh its goal.".into());
            } else {
                let goal = if method == "thread/goal/cleared" {
                    Ok(None)
                } else {
                    native::decode(thread, params)
                };
                match goal {
                    Ok(goal) => view.update(goal),
                    Err(error) => {
                        view.current = false;
                        view.error = Some(error);
                    }
                }
            }
            owners.push(owner.clone());
        }
        owners
    }
}
impl BrowserApp {
    pub(super) fn control_native_goal(
        &mut self,
        owner: &str,
        thread: &str,
        expected_directory: &str,
        action: Action,
    ) -> Result<(), String> {
        let target = self.conversation_target(owner)?;
        let root = target.root.ok_or("Connect a local directory first")?;
        if target.remote
            || !self.native_access_selected(owner)
            || !same_directory(&root, Path::new(expected_directory))
        {
            return Err(
                "Select this goal's original local Codex conversation and directory".into(),
            );
        }
        self.app_server
            .conversations
            .binding(owner)
            .filter(|b| b.thread_id == thread && !b.archived && !b.deleted)
            .ok_or("Select an active native conversation")?;
        if !self.app_server.conversations.is_thread_loaded(owner) {
            return Err("Resume this native conversation before managing its goal; no session was resumed automatically".into());
        }
        if self.app_server.native_config_pending()
            || self.app_server.delivery_warning(owner).is_some()
        {
            return Err("Resolve pending native configuration or uncertain delivery first".into());
        }
        let (id, call) = self.app_server.goals.action(owner, thread, &root, action)?;
        self.native_goal_call(owner.into(), thread.into(), id, call);
        self.emit_app_server_conversation(owner, "stream");
        Ok(())
    }
    fn native_goal_call(&self, owner: String, thread: String, id: String, call: Call) {
        let Some(client) = self.app_server.client.clone() else {
            return;
        };
        let epoch = self.app_server.epoch;
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = call.send(&client).and_then(|ticket| ticket.wait());
            let _ = proxy.send_event(BrowserEvent::AppServer(Event::GoalReply {
                epoch,
                owner,
                thread,
                id,
                result,
            }));
        });
    }
    pub(super) fn native_goal_reply(
        &mut self,
        owner: &str,
        thread: &str,
        id: &str,
        result: Result<Value, CallError>,
    ) {
        // Do not let a stale reply invalidate a newer operation's authority.
        if self
            .app_server
            .goals
            .view(owner, thread)
            .is_none_or(|v| v.pending.as_ref().is_none_or(|p| p.id != id))
        {
            return;
        }
        let valid = self.conversation_target(owner).ok().is_some_and(|target| {
            !target.remote
                && target.root.as_ref().is_some_and(|root| {
                    self.app_server
                        .goals
                        .view(owner, thread)
                        .is_some_and(|view| same_directory(root, &view.directory))
                })
        }) && self.native_access_selected(owner)
            && self
                .app_server
                .conversations
                .binding(owner)
                .is_some_and(|b| b.thread_id == thread && !b.deleted && !b.archived)
            && self.app_server.conversations.is_thread_loaded(owner)
            && !self.app_server.native_config_pending()
            && self.app_server.delivery_warning(owner).is_none();
        if !valid {
            if let Some(view) = self
                .app_server
                .goals
                .views
                .get_mut(owner)
                .filter(|v| v.thread_id == thread)
            {
                view.current = false;
                view.pending = None;
                view.busy = false;
                view.writing = false;
                view.error=Some("Goal scope changed. Refresh in its original conversation; no edit is replayed.".into());
            }
        } else if let Some((id, call)) = self.app_server.goals.reply(owner, thread, id, result) {
            self.native_goal_call(owner.into(), thread.into(), id, call);
        }
        self.emit_app_server_conversation(owner, "stream");
    }
}
