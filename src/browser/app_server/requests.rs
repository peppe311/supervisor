//! Owner-scoped native decisions. No request content or secret answer is saved.
use super::*;
use central_agent_codex_runtime::{requests::Decision, wire::ServerRequestKey};

#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum RequestAction {
    Answer { ticket: String, decision: Decision },
    OpenUrl { ticket: String },
}

impl std::fmt::Debug for RequestAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // AgentPanelMessage derives Debug. Do not expose passwords/form answers
        // if a caller logs a rejected IPC message.
        f.write_str("AppServerRequestAction(<redacted>)")
    }
}

impl BrowserApp {
    pub(in crate::browser) fn app_server_request_views(&self, owner: &str) -> Value {
        let views = self
            .app_server
            .requests
            .views(owner)
            .into_iter()
            .map(|view| {
                let item = view
                    .params
                    .get("itemId")
                    .and_then(Value::as_str)
                    .and_then(|id| {
                        self.app_server
                            .conversations
                            .mirror
                            .thread(&view.thread_id)?
                            .turns
                            .iter()
                            .find(|t| Some(t.id.as_str()) == view.turn_id.as_deref())?
                            .items
                            .iter()
                            .find(|i| i.id == id)
                            .map(|i| &i.value)
                    });
                let mut value = json!(view);
                if let Some(item) = item {
                    value["item"] = item.clone();
                }
                value
            })
            .collect::<Vec<_>>();
        json!(views)
    }

    pub(super) fn emit_app_server_requests(&self, owner: &str, error: Option<String>) {
        let supervision_state = self
            .is_supervised_project_chat_owner(owner)
            .then(|| self.conversation_state(owner));
        self.conversation_event(
            "central-agent:app-server-requests",
            json!({
                "owner":owner,"nativeRequests":self.app_server_request_views(owner),
                "conversationState":supervision_state,"error":error
            }),
        );
    }

    pub(in crate::browser) fn app_server_request(&mut self, owner: String, action: RequestAction) {
        let result = self.prepare_app_server_answer(&owner, action);
        self.emit_app_server_requests(&owner, result.err());
    }

    fn prepare_app_server_answer(
        &mut self,
        owner: &str,
        action: RequestAction,
    ) -> Result<(), String> {
        // Authority is the exact live request + its owner, not the current project.
        // Inactive main chats may still be answered after reopening their panel.
        let client = self
            .app_server
            .client
            .clone()
            .filter(|_| self.app_server.view.connected)
            .ok_or("Codex is disconnected; this decision was not sent")?;
        match action {
            RequestAction::OpenUrl { ticket } => {
                let url = self.app_server.requests.url(owner, &ticket)?;
                let url = safe_elicitation_url(url)?;
                // Explicit user gesture only; URL never becomes a shell command.
                crate::external_browser::open(url.as_str())
            }
            RequestAction::Answer { ticket, decision } => {
                let (key, result) = self.app_server.requests.answer(owner, &ticket, decision)?;
                let epoch = self.app_server.epoch;
                let owner = owner.to_owned();
                let proxy = self.proxy.clone();
                std::thread::spawn(move || {
                    let result = client.answer(&key, Ok(result));
                    let _ = proxy.send_event(BrowserEvent::AppServer(Event::Answered {
                        epoch,
                        owner,
                        key,
                        result,
                    }));
                });
                Ok(())
            }
        }
    }

    pub(super) fn receive_app_server_request(
        &mut self,
        key: ServerRequestKey,
        method: &str,
        params: Value,
    ) {
        self.record_native_event(
            None,
            "request",
            method,
            &params,
            Some(match &key.id {
                central_agent_codex_runtime::wire::RequestId::Text(id) => id.clone(),
                central_agent_codex_runtime::wire::RequestId::Integer(id) => id.to_string(),
            }),
        );
        let owner = params
            .get("threadId")
            .and_then(Value::as_str)
            .and_then(|id| self.app_server.conversations.owner_for_thread(id))
            .map(str::to_owned);
        let result = match owner.as_deref() {
            Some(_) if self.app_server.binding_store_error.is_some() => Err("Native conversation ownership could not be saved; this request cannot be answered yet".into()),
            Some(owner) => self
                .app_server
                .requests
                .insert(owner, key.clone(), method, params),
            None => Err("Native request has no conversation owned by this client".into()),
        };
        match result {
            Ok(()) => {
                if let Some(owner) = owner {
                    self.emit_app_server_requests(&owner, None);
                    self.note_supervision_request(&owner);
                    if let Some(binding) = self.app_server.conversations.binding(&owner) {
                        self.emit_native_delegate_relatives(&binding.thread_id);
                    }
                }
            }
            Err(error) => {
                if let Some(owner) = owner {
                    self.emit_app_server_requests(&owner, Some(error.clone()));
                }
                if let Some(client) = self.app_server.client.clone() {
                    std::thread::spawn(move || {
                        let _ = client.answer(
                            &key,
                            Err(RpcError {
                                code: -32601,
                                message: error,
                                data: None,
                            }),
                        );
                    });
                }
            }
        }
    }
}

fn safe_elicitation_url(value: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(value).map_err(|_| "Invalid MCP authorization URL")?;
    if !matches!(url.scheme(), "https" | "http")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(
            "MCP authorization must use an HTTP(S) URL without embedded credentials".into(),
        );
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn url_open_cannot_launch_a_local_program_or_credentials() {
        for bad in [
            "file:///C:/tool.exe",
            "javascript:alert(1)",
            "ssh://host",
            "https://user:secret@example.test",
        ] {
            assert!(safe_elicitation_url(bad).is_err());
        }
        assert!(safe_elicitation_url("https://example.test/oauth?state=opaque").is_ok());
        assert!(safe_elicitation_url("http://localhost:3210/authorize").is_ok());
    }
    #[test]
    fn ipc_never_accepts_raw_rpc_or_permission_grants() {
        assert!(serde_json::from_value::<RequestAction>(json!({"kind":"answer","ticket":"known","decision":{"kind":"accept"},"threadId":"other"})).is_err());
        assert!(serde_json::from_value::<RequestAction>(json!({"kind":"answer","ticket":"known","decision":{"kind":"permissions","grant":true,"session":false,"permissions":{"fileSystem":{"write":["C:/"]}}}})).is_err());
    }
}
