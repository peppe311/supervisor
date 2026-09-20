//! Explicit project-scoped inspection of shared native Codex preferences.
use super::*;
use central_agent_codex_runtime::configuration::{Snapshot, Target, WriteOutcome};

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum PreferencesAction {
    Refresh {
        request_id: String,
        expected_directory: String,
    },
    Save {
        view_id: String,
        key: String,
        value: String,
    },
    Clear {
        view_id: String,
        key: String,
    },
    Close {
        request_id: String,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct View {
    request_id: String,
    view_id: String,
    #[serde(serialize_with = "serialize_directory")]
    directory: PathBuf,
    snapshot: Option<Snapshot>,
    loading: bool,
    current: bool,
    error: Option<String>,
    #[serde(skip)]
    pending: Option<String>,
}

fn serialize_directory<S>(directory: &Path, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&display_path(directory))
}
struct Write {
    id: String,
    owner: String,
    target: Target,
    key: String,
    clearing: bool,
}
#[derive(Default)]
pub(super) struct Preferences {
    views: BTreeMap<String, View>,
    write: Option<Write>,
    notice: Option<String>,
}
impl Preferences {
    pub(super) fn writing(&self) -> bool {
        self.write.is_some()
    }
    fn refresh(
        &mut self,
        owner: &str,
        request_id: String,
        directory: PathBuf,
    ) -> Result<(String, Call), String> {
        if self.writing() || request_id.is_empty() {
            return Err("Wait for the pending native configuration write, then refresh".into());
        }
        let id = Uuid::new_v4().to_string();
        let call = api::config_read(
            directory
                .to_str()
                .ok_or("The project directory must be UTF-8")?,
        );
        self.views.insert(
            owner.into(),
            View {
                request_id,
                view_id: Uuid::new_v4().to_string(),
                directory,
                snapshot: None,
                loading: true,
                current: false,
                error: None,
                pending: Some(id.clone()),
            },
        );
        Ok((id, call))
    }
    fn save(
        &mut self,
        owner: &str,
        view_id: &str,
        directory: &Path,
        key: &str,
        value: &str,
    ) -> Result<(String, Call), String> {
        self.change(owner, view_id, directory, key, Some(value))
    }
    fn clear(
        &mut self,
        owner: &str,
        view_id: &str,
        directory: &Path,
        key: &str,
    ) -> Result<(String, Call), String> {
        self.change(owner, view_id, directory, key, None)
    }
    fn change(
        &mut self,
        owner: &str,
        view_id: &str,
        directory: &Path,
        key: &str,
        value: Option<&str>,
    ) -> Result<(String, Call), String> {
        if self.writing() {
            return Err("A native configuration write is already pending".into());
        }
        let view = self
            .views
            .get(owner)
            .filter(|v| v.current && !v.loading && v.view_id == view_id && v.directory == directory)
            .ok_or("Refresh native preferences for this project before saving")?;
        let snapshot = view.snapshot.as_ref().ok_or("Missing native preferences")?;
        let call = match value {
            Some(value) => snapshot.edit(key, value)?,
            None => snapshot.clear(key)?,
        };
        let id = Uuid::new_v4().to_string();
        self.write = Some(Write {
            id: id.clone(),
            owner: owner.into(),
            target: snapshot.target.clone().expect("Validated write target"),
            key: key.into(),
            clearing: value.is_none(),
        });
        self.notice = None;
        self.invalidate();
        Ok((id, call))
    }
    fn close(&mut self, owner: &str, request_id: &str) {
        if self
            .views
            .get(owner)
            .is_some_and(|v| v.request_id == request_id)
        {
            self.views.remove(owner);
        }
    }
    pub(super) fn invalidate(&mut self) {
        for view in self.views.values_mut() {
            view.current = false;
            view.loading = false;
            view.pending = None;
        }
    }
    fn reply(&mut self, owner: &str, id: &str, result: Result<Value, CallError>) {
        if self
            .write
            .as_ref()
            .is_some_and(|w| w.owner == owner && w.id == id)
        {
            let write = self.write.take().unwrap();
            self.invalidate();
            self.notice = Some(match result {
                Ok(value) => match WriteOutcome::read(&value, &write.target, &write.key) {
                    Ok(outcome) => format!(
                        "Codex {} {}. {}Refresh to inspect the effective project configuration. Existing conversations were not reloaded; explicit per-turn selections can override these defaults.",
                        if write.clearing {
                            "removed the saved user override for"
                        } else {
                            "saved"
                        },
                        write.key,
                        if outcome.overridden {
                            "A higher-priority configuration overrides this value. "
                        } else {
                            ""
                        }
                    ),
                    Err(error) => format!(
                        "{error}. The write may have completed. Nothing will be retried automatically."
                    ),
                },
                Err(error) => format!(
                    "Native configuration result: {error}. Refresh before saving again; no automatic retry was sent."
                ),
            });
            return;
        }
        let Some(view) = self
            .views
            .get_mut(owner)
            .filter(|v| v.pending.as_deref() == Some(id))
        else {
            return;
        };
        view.pending = None;
        view.loading = false;
        match result
            .map_err(|e| e.to_string())
            .and_then(|v| Snapshot::read(&v))
        {
            Ok(snapshot) => {
                view.snapshot = Some(snapshot);
                view.current = true;
                view.error = None;
            }
            Err(error) => {
                view.current = false;
                view.error = Some(error);
            }
        }
    }
    pub(super) fn disconnect(&mut self) {
        self.invalidate();
        if self.write.take().is_some() {
            self.notice = Some("Codex disconnected during a configuration write. Reconnect and refresh to inspect its outcome; nothing will be replayed.".into());
        }
    }
}

impl BrowserApp {
    pub(in crate::browser) fn app_server_preferences(
        &mut self,
        owner: String,
        action: PreferencesAction,
    ) {
        let result = (|| {
            if let PreferencesAction::Close { request_id } = &action {
                self.app_server.preferences.close(&owner, request_id);
                return Ok(());
            }
            if !self.app_server.view.connected || self.app_server.client.is_none() {
                return Err("Connect Codex App Server first".into());
            }
            if !self.native_access_selected(&owner) {
                return Err("Select Codex in this conversation first".into());
            }
            let target = self.conversation_target(&owner)?;
            let root = target
                .root
                .filter(|r| !target.remote && r.is_absolute() && r.is_dir())
                .ok_or("Choose an existing local project or graph directory")?;
            if matches!(
                &action,
                PreferencesAction::Save { .. } | PreferencesAction::Clear { .. }
            ) && (self.app_server.any_busy()
                || self.app_server.view.sandbox_setup.busy()
                || self
                    .agent_submission_queue
                    .iter()
                    .chain(&self.pending_agent_submissions)
                    .any(|i| i.provider == AgentProviderKind::CodexAppServer))
            {
                return Err("Finish active or queued Codex work and sandbox setup before changing shared preferences".into());
            }
            let (id, call) = match action {
                PreferencesAction::Refresh {
                    request_id,
                    expected_directory,
                } => {
                    if !same_directory(Path::new(&expected_directory), &root) {
                        return Err("The project changed; reopen its native preferences".into());
                    }
                    if self.app_server.native_config_pending() {
                        return Err(
                            "Wait for the shared Codex configuration write before refreshing"
                                .into(),
                        );
                    }
                    self.app_server
                        .preferences
                        .refresh(&owner, request_id, root)?
                }
                PreferencesAction::Save {
                    view_id,
                    key,
                    value,
                } => {
                    let prepared = self
                        .app_server
                        .preferences
                        .save(&owner, &view_id, &root, &key, &value)?;
                    self.invalidate_app_server_skills();
                    prepared
                }
                PreferencesAction::Clear { view_id, key } => {
                    let prepared = self
                        .app_server
                        .preferences
                        .clear(&owner, &view_id, &root, &key)?;
                    self.invalidate_app_server_skills();
                    prepared
                }
                PreferencesAction::Close { .. } => unreachable!(),
            };
            let client = self.app_server.client.clone().unwrap();
            let proxy = self.proxy.clone();
            let epoch = self.app_server.epoch;
            let reply_owner = owner.clone();
            std::thread::spawn(move || {
                let result = call.send(&client).and_then(|ticket| ticket.wait());
                let _ = proxy.send_event(BrowserEvent::AppServer(Event::PreferencesReply {
                    epoch,
                    owner: reply_owner,
                    id,
                    result,
                }));
            });
            Ok(())
        })();
        self.emit_app_server_preferences(&owner, result.err());
        self.emit_other_preference_views(&owner);
    }
    pub(super) fn app_server_preferences_reply(
        &mut self,
        owner: &str,
        id: &str,
        result: Result<Value, CallError>,
    ) {
        let was_writing = self.app_server.preferences.writing();
        self.app_server.preferences.reply(owner, id, result);
        if was_writing && !self.app_server.preferences.writing() {
            self.invalidate_app_server_skills();
        }
        self.emit_app_server_preferences(owner, None);
        self.emit_other_preference_views(owner);
    }
    pub(super) fn invalidate_app_server_preferences(&mut self) {
        self.app_server.mcp.stale_configuration();
        self.app_server.preferences.invalidate();
        self.emit_other_preference_views("");
    }
    fn emit_other_preference_views(&self, excluding: &str) {
        for owner in self
            .app_server
            .preferences
            .views
            .keys()
            .filter(|o| o.as_str() != excluding)
        {
            self.emit_app_server_preferences(owner, None);
        }
    }
    fn emit_app_server_preferences(&self, owner: &str, error: Option<String>) {
        self.conversation_event("central-agent:app-server-preferences", json!({"owner":owner,"view":self.app_server.preferences.views.get(owner),"writing":self.app_server.native_config_pending(),"notice":self.app_server.preferences.notice,"error":error}));
    }
}

fn same_directory(a: &Path, b: &Path) -> bool {
    a == b
        || fs::canonicalize(a)
            .ok()
            .zip(fs::canonicalize(b).ok())
            .is_some_and(|(a, b)| a == b)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(root: &Path) -> Value {
        json!({"config":{"model_verbosity":"medium"},"origins":{},"layers":[{"name":{"type":"user","file":root.join("config.toml"),"profile":null},"version":"native-v1","disabledReason":null,"config":{}}]})
    }
    fn ready(root: &Path) -> Preferences {
        let mut state = Preferences::default();
        let (id, _) = state.refresh("chat:a", "ui-a".into(), root.into()).unwrap();
        state.reply("chat:a", &id, Ok(fixture(root)));
        assert!(state.views["chat:a"].current);
        state
    }
    #[test]
    fn preference_reads_are_view_owned_and_invalidation_expires_pending_reads() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let mut state = ready(root);
        let (old, _) = state.refresh("chat:a", "old".into(), root.into()).unwrap();
        let (new, _) = state.refresh("chat:a", "new".into(), root.into()).unwrap();
        state.close("chat:a", "old");
        state.reply("chat:a", &old, Ok(fixture(root)));
        state.reply("graph:b", &new, Ok(fixture(root)));
        assert!(state.views["chat:a"].loading);
        state.invalidate();
        state.reply("chat:a", &new, Ok(fixture(root)));
        assert!(!state.views["chat:a"].current);
        state.close("chat:a", "new");
        assert!(state.views.is_empty());
    }

    #[test]
    fn preference_view_serializes_the_same_display_directory_as_the_conversation() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = fs::canonicalize(temp.path()).unwrap();
        let state = ready(&canonical);
        let value = serde_json::to_value(&state.views["chat:a"]).unwrap();
        assert_eq!(value["directory"], display_path(&canonical));
    }
    #[test]
    fn preference_write_requires_exact_view_owner_directory_and_single_inflight() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let mut state = ready(root);
        let view = state.views["chat:a"].view_id.clone();
        assert!(
            state
                .save("graph:b", &view, root, "model_verbosity", "high")
                .is_err()
        );
        assert!(
            state
                .save("chat:a", "old", root, "model_verbosity", "high")
                .is_err()
        );
        assert!(
            state
                .save(
                    "chat:a",
                    &view,
                    &root.join("other"),
                    "model_verbosity",
                    "high"
                )
                .is_err()
        );
        let (id, call) = state
            .save("chat:a", &view, root, "model_verbosity", "high")
            .unwrap();
        assert_eq!(call.params["expectedVersion"], "native-v1");
        assert!(
            state
                .save("chat:a", &view, root, "model_verbosity", "low")
                .is_err()
        );
        state.close("chat:a", "ui-a");
        assert!(state.writing());
        let ack = json!({"status":"ok","version":"native-v2","filePath":root.join("config.toml"),"overriddenMetadata":null});
        state.reply("graph:b", &id, Ok(ack.clone()));
        assert!(state.writing());
        state.reply("chat:a", &id, Ok(ack));
        assert!(!state.writing());
        assert!(state.notice.as_ref().unwrap().contains("not reloaded"));
        assert!(state.views.is_empty());
    }
    #[test]
    fn unknown_configuration_write_is_not_replayed_or_reported_successful() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        for disconnect in [false, true] {
            let mut state = ready(root);
            let view = state.views["chat:a"].view_id.clone();
            let (id, _) = state
                .save("chat:a", &view, root, "model_verbosity", "high")
                .unwrap();
            if disconnect {
                state.disconnect();
            } else {
                state.reply("chat:a", &id, Ok(json!({})));
            }
            assert!(!state.writing());
            assert!(!state.views["chat:a"].current);
            assert!(
                state
                    .save("chat:a", &view, root, "model_verbosity", "high")
                    .is_err()
            );
            assert!(!state.notice.as_ref().unwrap().contains("Codex saved"));
            let notice = state.notice.clone();
            state.reply("chat:a", &id, Ok(json!({})));
            assert_eq!(state.notice, notice);
        }
    }

    #[test]
    fn native_revision_conflicts_require_refresh_without_overwriting_or_retrying() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let mut state = ready(root);
        let view = state.views["chat:a"].view_id.clone();
        let (id, _) = state
            .save("chat:a", &view, root, "model_verbosity", "high")
            .unwrap();
        state.reply(
            "chat:a",
            &id,
            Err(CallError::Rpc(RpcError {
                code: -32600,
                message: "Native config version conflict".into(),
                data: None,
            })),
        );
        assert!(!state.writing());
        assert!(state.notice.as_ref().unwrap().contains("version conflict"));
        assert!(!state.views["chat:a"].current);
        assert!(
            state
                .save("chat:a", &view, root, "model_verbosity", "high")
                .is_err()
        );
        let (read, _) = state
            .refresh("chat:a", "refreshed".into(), root.into())
            .unwrap();
        let mut next = fixture(root);
        next["layers"][0]["version"] = json!("native-v2");
        state.reply("chat:a", &read, Ok(next));
        assert!(!state.writing());
        let refreshed_view = state.views["chat:a"].view_id.clone();
        let (_, call) = state
            .save("chat:a", &refreshed_view, root, "model_verbosity", "high")
            .unwrap();
        assert_eq!(call.params["expectedVersion"], "native-v2");
    }

    #[test]
    fn shared_preference_write_blocks_native_work_until_result() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let mut state = State {
            preferences: ready(root),
            ..Default::default()
        };
        let view = state.preferences.views["chat:a"].view_id.clone();
        let (id, _) = state
            .preferences
            .save("chat:a", &view, root, "model_verbosity", "high")
            .unwrap();
        assert!(state.native_config_pending());
        assert!(state.any_busy());
        state.preferences.reply("chat:a", &id, Ok(json!({"status":"ok","version":"native-v2","filePath":root.join("config.toml"),"overriddenMetadata":null})));
        assert!(!state.native_config_pending());
        assert!(!state.any_busy());
    }

    #[test]
    fn clearing_preferences_uses_the_same_frozen_authority_and_write_lock() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let mut state = ready(root);
        let view = state.views["chat:a"].view_id.clone();
        assert!(
            state
                .clear("chat:a", &view, root, "model_verbosity")
                .is_err()
        );
        state
            .views
            .get_mut("chat:a")
            .unwrap()
            .snapshot
            .as_mut()
            .unwrap()
            .preferences
            .iter_mut()
            .find(|p| p.key == "model_verbosity")
            .unwrap()
            .user_value = Some("medium".into());
        assert!(
            state
                .clear("graph:other", &view, root, "model_verbosity")
                .is_err()
        );
        assert!(
            state
                .clear("chat:a", "old-view", root, "model_verbosity")
                .is_err()
        );
        assert!(
            state
                .clear("chat:a", &view, &root.join("other"), "model_verbosity")
                .is_err()
        );
        let (id, call) = state
            .clear("chat:a", &view, root, "model_verbosity")
            .unwrap();
        assert_eq!(
            call.params["edits"],
            json!([{"keyPath":"model_verbosity","value":null,"mergeStrategy":"upsert"}])
        );
        assert_eq!(call.params["expectedVersion"], "native-v1");
        assert!(
            state
                .save("chat:a", &view, root, "model_verbosity", "high")
                .is_err()
        );
        state.close("chat:a", "ui-a");
        assert!(state.writing());
        state.reply("chat:a", &id, Ok(json!({"status":"ok","version":"native-v2","filePath":root.join("config.toml"),"overriddenMetadata":null})));
        assert!(
            state
                .notice
                .as_ref()
                .unwrap()
                .contains("removed the saved user override")
        );
        assert!(state.notice.as_ref().unwrap().contains("not reloaded"));
        assert!(!state.writing());
    }

    #[test]
    fn clear_preference_disconnect_never_replays_or_reports_removal() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let mut state = ready(root);
        let view = state.views["chat:a"].view_id.clone();
        state
            .views
            .get_mut("chat:a")
            .unwrap()
            .snapshot
            .as_mut()
            .unwrap()
            .preferences
            .iter_mut()
            .find(|p| p.key == "model_verbosity")
            .unwrap()
            .user_value = Some("medium".into());
        let (id, _) = state
            .clear("chat:a", &view, root, "model_verbosity")
            .unwrap();
        state.disconnect();
        let notice = state.notice.clone();
        assert!(!notice.as_ref().unwrap().contains("removed the saved"));
        state.reply(
            "chat:a",
            &id,
            Ok(json!({"status":"ok","version":"native-v2","filePath":root.join("config.toml")})),
        );
        assert_eq!(state.notice, notice);
        assert!(
            state
                .clear("chat:a", &view, root, "model_verbosity")
                .is_err()
        );
        assert!(serde_json::from_value::<PreferencesAction>(json!({"kind":"clear","view_id":view,"key":"model_verbosity","file":"untrusted.toml"})).is_err());
    }
}
