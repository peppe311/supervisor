//! Explicit follow-up to one observed turn. Never fallback to start or queue.
use super::*;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SteerInput {
    expected_turn_id: String,
    message: String,
    #[serde(default)]
    timing: Option<delivery::ClientTiming>,
    #[serde(default)]
    tab_ids: Vec<u64>,
    #[serde(default)]
    terminal_session_ids: Vec<u64>,
    #[serde(default)]
    file_ids: Vec<String>,
}
impl std::fmt::Debug for SteerInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SteerInput").finish_non_exhaustive()
    }
}
impl SteerInput {
    fn validate(&self) -> Result<(), String> {
        if (self.message.trim().is_empty()
            && self.tab_ids.is_empty()
            && self.terminal_session_ids.is_empty()
            && self.file_ids.is_empty())
            || self.expected_turn_id.is_empty()
        {
            return Err(
                "A follow-up requires text or an attachment and the original active turn. Your draft was retained."
                    .into(),
            );
        }
        Ok(())
    }

    pub(in crate::browser) fn supervised(
        expected_turn_id: String,
        message: String,
        timing: Option<delivery::ClientTiming>,
    ) -> Self {
        Self {
            expected_turn_id,
            message,
            timing,
            tab_ids: Vec::new(),
            terminal_session_ids: Vec::new(),
            file_ids: Vec::new(),
        }
    }
}
pub(super) struct Steering {
    skills: Vec<SelectedSkill>,
    apps: Vec<SelectedApp>,
    pub(super) message_id: String,
    input: String,
    snapshots: SubmissionSnapshots,
    pub(super) receipt_owner: String,
}
impl State {
    pub(in crate::browser) fn steer_target(&self, owner: &str) -> Option<&str> {
        if !self.view.connected
            || self.client.is_none()
            || self.binding_store_error.is_some()
            || self.view.sandbox_setup.busy()
            || self.submissions.contains_key(owner)
            || self.steering.contains_key(owner)
        {
            return None;
        }
        self.conversations.steer_target(owner)
    }
}
impl BrowserApp {
    pub(in crate::browser) fn app_server_steer(&mut self, owner: String, input: SteerInput) {
        self.app_server_steer_for(owner.clone(), input, owner);
    }

    pub(in crate::browser) fn app_server_steer_for(
        &mut self,
        owner: String,
        input: SteerInput,
        receipt_owner: String,
    ) {
        let trace = delivery::Trace::new(input.timing.clone());
        let result = (|| -> Result<(), String> {
            input.validate()?;
            if !self.native_access_selected(&owner) {
                return Err(
                    "Select the original Codex conversation before sending this follow-up.".into(),
                );
            }
            let target = self.conversation_target(&owner)?;
            if target.remote {
                return Err("Native steering requires the original local conversation.".into());
            }
            let root = target
                .root
                .clone()
                .unwrap_or_else(|| self.data_dir.join("codex-workspace"));
            let root = fs::canonicalize(&root).map_err(|e| e.to_string())?;
            let original = self
                .app_server
                .roots
                .get(&owner)
                .ok_or("The native working directory is not loaded")?;
            let original = fs::canonicalize(original).map_err(|e| e.to_string())?;
            if root != original {
                return Err("This conversation's directory changed. Stop the old turn before starting work in a different project.".into());
            }
            if self.app_server.steer_target(&owner) != Some(input.expected_turn_id.as_str()) {
                return Err("The original Codex turn is no longer available for steering. Your draft was retained; no new turn or queue entry was created.".into());
            }
            let message_id = Uuid::new_v4().to_string();
            let mut snapshots =
                self.freeze_native_contexts(&owner, &input.tab_ids, &input.terminal_session_ids)?;
            snapshots.files = self.capture_native_files(&owner, &input.file_ids)?;
            let skills = self.app_server.capture_skills_for_prompt(
                &owner,
                Some(target.root.as_deref().unwrap_or(&root)),
                &input.message,
            )?;
            let thread = self
                .app_server
                .conversations
                .binding(&owner)
                .map(|binding| binding.thread_id.clone())
                .ok_or("The native conversation changed before Apps were captured")?;
            let apps = self.app_server.apps.capture(&owner, Some(&thread))?;
            let mut native_input = attachments::native_inputs(
                &input.message,
                &input.tab_ids,
                &snapshots.tabs,
                &input.terminal_session_ids,
                &snapshots.terminals,
                &input.file_ids,
                &snapshots.files,
            )?;
            skills::append_skill_inputs(
                &mut native_input,
                &skills,
                target.root.as_deref().unwrap_or(&root),
            )?;
            apps::append_app_inputs(&mut native_input, &apps)?;
            if let Some(context) = owner
                .strip_prefix("graph:")
                .and_then(|node_key| self.supervised_chat_context(node_key))
            {
                native_input.push(api::text_input(&context));
            }
            let request = self.app_server.conversations.steer(
                &owner,
                &input.expected_turn_id,
                &message_id,
                native_input,
            )?;
            trace.prepared();
            // Write-ahead uncertainty marker, without persisting the prompt again.
            if let Err(error) = self.save_app_server_bindings() {
                let _ = self
                    .app_server
                    .conversations
                    .complete(&request, Err(CallError::Rejected(error.clone())));
                return Err(error);
            }
            let after = self
                .app_server_messages(&owner)
                .last()
                .and_then(|row| row["id"].as_str())
                .map(str::to_owned);
            self.app_server.pending_inputs.begin(
                &owner,
                &message_id,
                &input.message,
                &snapshots,
                Some(&input.expected_turn_id),
                after,
            );
            self.app_server.timings.register(
                &owner,
                &message_id,
                Some(&thread),
                trace.clone(),
                "steer",
            );
            self.app_server.steering.insert(
                owner.clone(),
                Steering {
                    skills,
                    apps,
                    message_id,
                    input: input.message,
                    snapshots,
                    receipt_owner: receipt_owner.clone(),
                },
            );
            self.dispatch_app_server_thread(request);
            Ok(())
        })();
        if let Err(error) = result {
            self.conversation_event(
                "central-agent:conversation-error",
                json!({"owner":receipt_owner,"error":error}),
            );
        }
        self.render_agent_panel();
    }
    pub(super) fn accept_app_server_steering(&mut self, owner: &str, message_id: &str) -> bool {
        if self
            .app_server
            .steering
            .get(owner)
            .is_none_or(|s| s.message_id != message_id)
        {
            return false;
        }
        let pending = self.app_server.steering.remove(owner).unwrap();
        self.app_server.skills.consume(owner, &pending.skills);
        self.app_server.apps.consume(owner, &pending.apps);
        self.emit_app_server_skills(owner, None);
        self.consume_snapshot_attachments(owner, &pending.snapshots);
        self.conversation_event(
            "central-agent:submission-accepted",
            json!({"owner":pending.receipt_owner,"input":pending.input}),
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn steering_intent_cannot_override_native_configuration_or_drop_attachments() {
        let base = json!({"expected_turn_id":"turn-a","message":"Follow up"});
        assert!(
            serde_json::from_value::<SteerInput>(base.clone())
                .unwrap()
                .validate()
                .is_ok()
        );
        for (key, value) in [
            ("model", json!("other")),
            ("cwd", json!("C:/")),
            ("sandboxPolicy", json!({})),
        ] {
            let mut payload = base.clone();
            payload[key] = value;
            assert!(serde_json::from_value::<SteerInput>(payload).is_err());
        }
        for (key, value) in [("message", json!(" ")), ("expected_turn_id", json!(""))] {
            let mut payload = base.clone();
            payload[key] = value;
            assert!(
                serde_json::from_value::<SteerInput>(payload)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
        for key in ["tab_ids", "terminal_session_ids", "file_ids"] {
            let mut payload = json!({"expected_turn_id":"turn-a","message":" "});
            payload[key] = json!([if key == "file_ids" {
                json!("file")
            } else {
                json!(1)
            }]);
            assert!(
                serde_json::from_value::<SteerInput>(payload)
                    .unwrap()
                    .validate()
                    .is_ok()
            );
        }
        assert!(
            !format!("{:?}", serde_json::from_value::<SteerInput>(base).unwrap())
                .contains("Follow up")
        );
    }
}
