//! Local surfaces for native child agents; all execution stays in Codex.
use super::*;
use central_agent_codex_runtime::conversations::Delegation;

impl BrowserApp {
    pub(super) fn reconcile_native_delegates(&mut self, owner: &str) {
        let children = self.app_server.conversations.historical_delegations(owner);
        self.adopt_native_delegates(children);
    }

    pub(super) fn adopt_native_delegates(&mut self, children: Vec<Delegation>) {
        for child in children {
            if self
                .app_server
                .conversations
                .owner_for_thread(&child.child_thread)
                .is_some()
            {
                continue;
            }
            let result = self.adopt_native_delegate(&child);
            if let Err(error) = result {
                self.app_server
                    .view
                    .errors
                    .insert("native-delegation", error.clone());
                self.conversation_event(
                    "central-agent:app-server-conversation",
                    json!({"owner":child.parent_owner,"error":error}),
                );
            }
        }
    }

    fn adopt_native_delegate(&mut self, child: &Delegation) -> Result<(), String> {
        if let Some(error) = &self.app_server.binding_store_error {
            return Err(error.clone());
        }
        let target = self.conversation_target(&child.parent_owner)?;
        let root = target
            .root
            .filter(|root| !target.remote && root.is_absolute() && root.is_dir())
            .ok_or("The native child has no local project destination")?;
        let count = self
            .app_server
            .conversations
            .saved()
            .delegations
            .values()
            .filter(|parent| **parent == child.parent_thread)
            .count();
        let destination = self.create_native_destination(
            &child.parent_owner,
            &format!("Delegated agent {}", count + 1),
            &root,
        )?;
        self.app_server
            .conversations
            .adopt_delegation(child, &destination.owner)?;
        // Synchronous durability before the next native event can route an approval.
        self.save_app_server_bindings()?;
        self.app_server
            .roots
            .insert(destination.owner.clone(), root);
        self.app_server.view.errors.remove("native-delegation");
        self.emit_app_server_conversation(&child.parent_owner, "delegate_linked");
        self.emit_app_server_conversation(&destination.owner, "delegate_linked");
        Ok(())
    }

    pub(super) fn native_delegate_views(&self, owner: &str) -> Vec<Value> {
        let book = &self.app_server.conversations;
        let Some(binding) = book.binding(owner) else {
            return vec![];
        };
        book.saved().delegations.iter().filter_map(|(child, parent)| {
            let (thread, relation) = if parent == &binding.thread_id { (child, "child") }
                else if child == &binding.thread_id { (parent, "parent") } else { return None; };
            let destination = book.owner_for_thread(thread)?;
            let name = if let Some(id) = destination.strip_prefix("chat:") {
                self.project_chats.iter().find(|chat| chat.id == id).map(|chat| chat.title.clone())
            } else {
                self.agent_graph_bindings.iter().find(|binding| Some(binding.node_key().as_str()) == destination.strip_prefix("graph:")).map(|binding| binding.name.clone())
            }?;
            let native = book.mirror.thread(thread);
            let requests = self.app_server.requests.views(destination).len();
            let status = if book.binding(destination).is_some_and(|b| b.deleted) { "deleted" }
                else if requests > 0 { "needs_attention" }
                else if !book.is_thread_loaded(destination) { "not_loaded" }
                else if book.busy(destination) { "running" }
                else { native.and_then(|t| t.turns.last()).map(|turn| turn.status.as_str()).unwrap_or("idle") };
            Some(json!({"owner":destination,"threadId":thread,"name":name,"relation":relation,"status":status,"pendingRequests":requests}))
        }).collect()
    }

    pub(super) fn open_native_delegate(
        &mut self,
        owner: &str,
        expected_thread: &str,
        destination: &str,
    ) -> Result<(), String> {
        self.conversation_target(owner)?;
        let book = &self.app_server.conversations;
        let target = book
            .binding(destination)
            .ok_or("The delegated conversation is no longer linked")?;
        let related = book
            .saved()
            .delegations
            .get(&target.thread_id)
            .is_some_and(|parent| parent == expected_thread)
            || book
                .saved()
                .delegations
                .get(expected_thread)
                .is_some_and(|parent| parent == &target.thread_id);
        if !related {
            return Err("This conversation is not part of the selected native delegation".into());
        }
        self.open_native_destination(owner, destination)?;
        self.emit_app_server_conversation(owner, "delegate_opened");
        Ok(())
    }

    pub(super) fn emit_native_delegate_relatives(&self, thread: &str) {
        let book = &self.app_server.conversations;
        for (child, parent) in &book.saved().delegations {
            let related = if child == thread {
                parent
            } else if parent == thread {
                child
            } else {
                continue;
            };
            if let Some(owner) = book.owner_for_thread(related) {
                self.emit_app_server_conversation(owner, "stream");
            }
        }
    }
}
