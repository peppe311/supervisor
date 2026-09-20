//! Bound native MCP inventory and explicit service OAuth; never a prompt,
//! implicit resume, tool execution or custom server registration.
use super::*;

#[derive(Default)]
pub(super) struct Inventories {
    owners: BTreeMap<String, (String, mcp::McpState)>,
}
impl Inventories {
    fn action(
        &mut self,
        owner: &str,
        thread: &str,
        action: McpAction,
    ) -> Result<(String, Call), String> {
        self.owners
            .get_mut(owner)
            .filter(|entry| entry.0 == thread)
            .ok_or("Refresh this conversation's MCP inventory first.")?
            .1
            .action(action)
    }
    pub(super) fn login_url(
        &self,
        owner: &str,
        thread: &str,
        attempt: &str,
    ) -> Result<&url::Url, String> {
        self.owners
            .get(owner)
            .filter(|entry| entry.0 == thread)
            .ok_or("This native MCP scope is no longer available.")?
            .1
            .login_url(attempt)
    }
    fn refresh(&mut self, owner: &str, thread: &str) -> Result<(String, Call), String> {
        let entry = self
            .owners
            .entry(owner.into())
            .or_insert_with(|| (thread.into(), mcp::McpState::for_thread(thread)));
        if entry.0 != thread {
            *entry = (thread.into(), mcp::McpState::for_thread(thread));
        }
        entry.1.refresh()
    }
    pub(super) fn view(&self, owner: &str, thread: &str) -> Option<&mcp::View> {
        self.owners
            .get(owner)
            .filter(|entry| entry.0 == thread)
            .map(|entry| &entry.1.view)
    }
    fn reply(
        &mut self,
        owner: &str,
        thread: &str,
        id: &str,
        result: Result<Value, CallError>,
    ) -> Option<(String, Call)> {
        let entry = self
            .owners
            .get_mut(owner)
            .filter(|entry| entry.0 == thread)?;
        entry.1.reply(id, result)
    }
    pub(super) fn invalidate_all(&mut self, notice: &str) -> Vec<String> {
        self.owners
            .iter_mut()
            .map(|(owner, (_, state))| {
                state.invalidate(notice);
                owner.clone()
            })
            .collect()
    }
    pub(super) fn invalidate_inventories(&mut self, notice: &str) -> Vec<String> {
        self.owners
            .iter_mut()
            .map(|(owner, (_, state))| {
                state.invalidate_inventory(notice);
                owner.clone()
            })
            .collect()
    }
    pub(super) fn notify(&mut self, method: &str, params: &Value) -> Vec<String> {
        if method == "mcpServer/oauthLogin/completed" {
            // Credentials can be shared, but only the exact originating scope
            // may consume this completion. All inventories become stale.
            return self.owners.iter_mut().map(|(owner, (_, state))| {
                state.invalidate_inventory("Native service authentication changed. Refresh this conversation's MCP inventory.");
                state.completed(params);
                owner.clone()
            }).collect();
        }
        let invalidates = matches!(
            method,
            "mcpServer/startupStatus/updated"
                | "thread/closed"
                | "thread/deleted"
                | "thread/archived"
        ) || method == "thread/status/changed"
            && params["status"]["type"] == "notLoaded";
        if !invalidates {
            return vec![];
        }
        let Some(thread) = params.get("threadId").and_then(Value::as_str) else {
            return vec![];
        };
        self.owners.iter_mut().filter(|(_, (id, _))| id == thread).map(|(owner, (_, state))| {
            state.invalidate_inventory("The native conversation or its MCP servers changed. Refresh to inspect the current tool inventory.");
            owner.clone()
        }).collect()
    }
}

impl BrowserApp {
    pub(super) fn control_thread_mcp(
        &mut self,
        owner: &str,
        thread: &str,
        action: McpAction,
    ) -> Result<(), String> {
        if matches!(action, McpAction::Refresh {}) {
            return self.refresh_thread_mcp(owner, thread);
        }
        if matches!(action, McpAction::Reload { .. }) {
            return Err("Shared MCP reload belongs in Codex Settings.".into());
        }
        if !self.app_server.conversations.is_thread_loaded(owner) {
            return Err("Resume this native conversation before starting or opening its service authorization.".into());
        }
        let result = (|| {
            if let McpAction::OpenLogin { attempt_id } = action {
                let url = self
                    .app_server
                    .thread_mcp
                    .login_url(owner, thread, &attempt_id)?;
                return crate::external_browser::open(url.as_str());
            }
            let (id, call) = self.app_server.thread_mcp.action(owner, thread, action)?;
            self.thread_mcp_call(owner.into(), thread.into(), id, call);
            Ok(())
        })();
        if let Err(error) = result {
            if let Some((_, state)) = self
                .app_server
                .thread_mcp
                .owners
                .get_mut(owner)
                .filter(|entry| entry.0 == thread)
            {
                state.view.error = Some(error);
            } else {
                return Err(error);
            }
        }
        self.emit_app_server_conversation(owner, "stream");
        Ok(())
    }
    pub(super) fn refresh_thread_mcp(&mut self, owner: &str, thread: &str) -> Result<(), String> {
        self.app_server
            .conversations
            .binding(owner)
            .filter(|b| b.thread_id == thread && !b.deleted && !b.archived)
            .ok_or("Select an active native conversation first.")?;
        if !self.app_server.conversations.is_thread_loaded(owner) {
            return Err("Resume the native conversation before inspecting its MCP tools. No conversation was resumed automatically.".into());
        }
        let (id, call) = self.app_server.thread_mcp.refresh(owner, thread)?;
        self.thread_mcp_call(owner.into(), thread.into(), id, call);
        // A metadata update must not acknowledge a different pending chat action.
        self.emit_app_server_conversation(owner, "stream");
        Ok(())
    }
    fn thread_mcp_call(&self, owner: String, thread: String, id: String, call: Call) {
        let Some(client) = self.app_server.client.clone() else {
            return;
        };
        let epoch = self.app_server.epoch;
        let proxy = self.proxy.clone();
        std::thread::spawn(move || {
            let result = call.send(&client).and_then(|ticket| ticket.wait());
            let _ = proxy.send_event(BrowserEvent::AppServer(Event::ThreadMcpReply {
                epoch,
                owner,
                thread,
                id,
                result,
            }));
        });
    }
    pub(super) fn thread_mcp_reply(
        &mut self,
        owner: &str,
        thread: &str,
        id: &str,
        result: Result<Value, CallError>,
    ) {
        if self.conversation_target(owner).is_err()
            || self
                .app_server
                .conversations
                .binding(owner)
                .is_none_or(|b| b.thread_id != thread || b.deleted)
        {
            return;
        }
        if let Some((id, call)) = self.app_server.thread_mcp.reply(owner, thread, id, result) {
            self.thread_mcp_call(owner.into(), thread.into(), id, call);
        }
        self.emit_app_server_conversation(owner, "stream");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn page(next: Value) -> Value {
        json!({"data":[],"nextCursor":next})
    }
    fn scoped_login(inventories: &mut Inventories, owner: &str, thread: &str) -> String {
        let (id, _) = inventories.refresh(owner, thread).unwrap();
        inventories.reply(owner, thread, &id, Ok(json!({"data":[{"name":"fixture","authStatus":"notLoggedIn","runtimeStatus":"authenticationRequired","tools":{},"resources":[],"resourceTemplates":[]}],"nextCursor":null})));
        let inventory_id = inventories
            .view(owner, thread)
            .unwrap()
            .inventory_id
            .clone()
            .unwrap();
        let (id, call) = inventories
            .action(
                owner,
                thread,
                McpAction::Login {
                    inventory_id,
                    name: "fixture".into(),
                },
            )
            .unwrap();
        assert_eq!(call.params, json!({"threadId":thread,"name":"fixture"}));
        id
    }
    #[test]
    fn oauth_scope_early_completion_and_inventory_changes_preserve_exact_attempt() {
        for early in [false, true] {
            let mut inventories = Inventories::default();
            let a = scoped_login(&mut inventories, "chat:a", "a");
            let b = scoped_login(&mut inventories, "graph:b", "b");
            inventories.notify(
                "mcpServer/startupStatus/updated",
                &json!({"threadId":"a","name":"fixture","status":"starting"}),
            );
            inventories.notify(
                "mcpServer/oauthLogin/completed",
                &json!({"threadId":null,"name":"fixture","success":true}),
            );
            assert!(inventories.view("chat:a", "a").unwrap().login.is_some());
            let completed = json!({"threadId":"a","name":"fixture","success":true});
            if early {
                inventories.notify("mcpServer/oauthLogin/completed", &completed);
            }
            inventories.reply(
                "chat:a",
                "a",
                &a,
                Ok(json!({"authorizationUrl":"https://example.test/authorize?state=private"})),
            );
            if !early {
                assert!(inventories.login_url("chat:a", "a", &a).is_ok());
                assert!(inventories.login_url("graph:b", "b", &a).is_err());
                assert!(
                    !serde_json::to_string(inventories.view("chat:a", "a").unwrap())
                        .unwrap()
                        .contains("private")
                );
                inventories.notify("mcpServer/oauthLogin/completed", &completed);
            }
            assert!(inventories.login_url("chat:a", "a", &a).is_err());
            assert!(inventories.view("chat:a", "a").unwrap().login.is_none());
            assert!(!inventories.view("chat:a", "a").unwrap().current);
            assert!(
                inventories
                    .view("chat:a", "a")
                    .unwrap()
                    .notice
                    .as_deref()
                    .unwrap()
                    .contains("completed")
            );
            assert!(inventories.view("graph:b", "b").unwrap().login.is_some());
            inventories.reply(
                "graph:b",
                "b",
                &b,
                Ok(json!({"authorizationUrl":"https://example.test/b"})),
            );
            assert!(inventories.login_url("graph:b", "b", &b).is_ok());
            inventories.invalidate_all("Disconnected");
            inventories.notify(
                "mcpServer/oauthLogin/completed",
                &json!({"threadId":"b","name":"fixture","success":true}),
            );
            assert!(inventories.login_url("graph:b", "b", &b).is_err());
            assert!(inventories.view("graph:b", "b").unwrap().login.is_none());
        }
    }
    #[test]
    fn stale_inventory_and_unsupported_service_cannot_start_a_scoped_login() {
        let mut inventories = Inventories::default();
        let (id, _) = inventories.refresh("chat:a", "a").unwrap();
        inventories.reply("chat:a", "a", &id, Ok(page(Value::Null)));
        for (thread, inventory_id) in [
            ("b", "stale".to_owned()),
            ("a", "stale".to_owned()),
            (
                "a",
                inventories
                    .view("chat:a", "a")
                    .unwrap()
                    .inventory_id
                    .clone()
                    .unwrap(),
            ),
        ] {
            assert!(
                inventories
                    .action(
                        "chat:a",
                        thread,
                        McpAction::Login {
                            inventory_id,
                            name: "missing".into()
                        }
                    )
                    .is_err()
            );
        }
    }
    #[test]
    fn pages_keep_exact_owner_and_thread_and_never_fall_back_to_global() {
        let mut inventories = Inventories::default();
        let (id, call) = inventories.refresh("graph:a", "thread-a").unwrap();
        assert_eq!(call.method, "mcpServerStatus/list");
        assert_eq!(
            call.params,
            json!({"threadId":"thread-a","cursor":null,"detail":"full"})
        );
        assert!(inventories.refresh("graph:a", "thread-a").is_err());
        let (sibling, _) = inventories.refresh("chat:b", "thread-b").unwrap();
        assert!(
            inventories
                .reply("chat:b", "thread-b", &id, Ok(page(Value::Null)))
                .is_none()
        );
        assert!(inventories.view("chat:b", "thread-b").unwrap().busy);
        let (next, call) = inventories
            .reply("graph:a", "thread-a", &id, Ok(page(json!("page-2"))))
            .unwrap();
        assert_eq!(call.params["threadId"], "thread-a");
        assert_eq!(call.params["cursor"], "page-2");
        assert!(!inventories.view("graph:a", "thread-a").unwrap().current);
        inventories.reply("graph:a", "thread-a", &next, Ok(page(Value::Null)));
        assert!(inventories.view("graph:a", "thread-a").unwrap().current);
        inventories.reply("chat:b", "thread-b", &sibling, Ok(page(Value::Null)));
        assert!(inventories.view("chat:b", "thread-b").unwrap().current);
        assert!(inventories.view("graph:a", "thread-b").is_none());
    }
    #[test]
    fn scope_replacement_disconnect_and_native_changes_reject_late_pages() {
        let mut inventories = Inventories::default();
        let (old, _) = inventories.refresh("chat:a", "old").unwrap();
        let (new, _) = inventories.refresh("chat:a", "new").unwrap();
        inventories.reply("chat:a", "old", &old, Ok(page(Value::Null)));
        assert!(inventories.view("chat:a", "old").is_none());
        assert!(inventories.view("chat:a", "new").unwrap().busy);
        assert!(
            inventories
                .notify("mcpServer/startupStatus/updated", &json!({"threadId":null}))
                .is_empty()
        );
        assert!(
            inventories
                .notify(
                    "mcpServer/startupStatus/updated",
                    &json!({"threadId":"foreign"})
                )
                .is_empty()
        );
        assert_eq!(
            inventories.notify(
                "mcpServer/startupStatus/updated",
                &json!({"threadId":"new"})
            ),
            vec!["chat:a"]
        );
        inventories.reply("chat:a", "new", &new, Ok(page(Value::Null)));
        assert!(!inventories.view("chat:a", "new").unwrap().current);
        assert!(!inventories.view("chat:a", "new").unwrap().busy);
        let (new, _) = inventories.refresh("chat:a", "new").unwrap();
        inventories.invalidate_all("Disconnected");
        inventories.reply("chat:a", "new", &new, Ok(page(Value::Null)));
        assert!(!inventories.view("chat:a", "new").unwrap().current);
    }
    #[test]
    fn error_does_not_publish_partial_inventory_or_start_a_retry() {
        let mut inventories = Inventories::default();
        let (id, _) = inventories.refresh("graph:a", "thread-a").unwrap();
        inventories.reply("graph:a", "thread-a", &id, Ok(json!({"data":[]})));
        let view = inventories.view("graph:a", "thread-a").unwrap();
        assert!(!view.current && !view.busy && view.error.is_some());
    }
    #[test]
    fn lifecycle_and_auth_updates_invalidate_only_the_affected_scope() {
        for (method, params, all) in [
            ("thread/closed", json!({"threadId":"a"}), false),
            ("thread/archived", json!({"threadId":"a"}), false),
            ("thread/deleted", json!({"threadId":"a"}), false),
            (
                "thread/status/changed",
                json!({"threadId":"a","status":{"type":"notLoaded"}}),
                false,
            ),
            (
                "mcpServer/oauthLogin/completed",
                json!({"threadId":null}),
                true,
            ),
        ] {
            let mut inventories = Inventories::default();
            for scope in ["a", "b"] {
                let (id, _) = inventories.refresh(scope, scope).unwrap();
                inventories.reply(scope, scope, &id, Ok(page(Value::Null)));
            }
            inventories.notify(method, &params);
            assert!(!inventories.view("a", "a").unwrap().current);
            assert_eq!(inventories.view("b", "b").unwrap().current, !all);
        }
    }
}
