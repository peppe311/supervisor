//! Opt-in acceptance against the real runtime using the production conversation
//! manager. Three tiny read-only turns; no personal history inspection, config
//! writes, tools, local transcript persistence, or automatic prompt replay.
use central_agent_codex_runtime::{
    api,
    conversations::{Action, Conversations, Outcome, Request},
    runtime::Runtime,
    transport::{Client, Event, Ticket},
};
use std::{collections::BTreeSet, path::Path, sync::mpsc, time::Duration};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const CHAT: &str = "chat:lifecycle-probe";
const GRAPH: &str = "graph:lifecycle-probe";
const BRANCH: &str = "chat:lifecycle-branch";
const ALPHA: &str = "ALPHA_73";
const BETA: &str = "BETA_29";

struct Probe {
    client: Client,
    events: mpsc::Receiver<Event>,
    book: Conversations,
    owned: BTreeSet<String>,
    streamed: BTreeSet<String>,
    completed: BTreeSet<(String, String)>,
}

impl Probe {
    fn connect(runtime: &Runtime, book: Conversations, owned: BTreeSet<String>) -> Result<Self> {
        let (tx, events) = mpsc::channel();
        let client = Client::spawn(runtime, env!("CARGO_PKG_VERSION"), move |event| {
            let _ = tx.send(event);
        })?;
        let mut probe = Self {
            client,
            events,
            book,
            owned,
            streamed: BTreeSet::new(),
            completed: BTreeSet::new(),
        };
        loop {
            let event = probe.events.recv()?;
            if matches!(event, Event::Ready { .. }) {
                break;
            }
            probe.event(event)?;
        }
        let requirements = api::requirements().send(&probe.client)?.wait()?;
        api::Access::ReadOnly.validate_requirements(requirements.get("requirements"))?;
        Ok(probe)
    }

    fn event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Notification { method, params } => {
                let thread = params["threadId"].as_str().unwrap_or_default();
                if self.owned.contains(thread) {
                    if method == "item/agentMessage/delta" {
                        self.streamed.insert(thread.into());
                    }
                    if matches!(method.as_str(), "item/started" | "item/completed") {
                        let kind = params["item"]["type"].as_str().unwrap_or_default();
                        if !matches!(
                            kind,
                            "userMessage"
                                | "agentMessage"
                                | "reasoning"
                                | "plan"
                                | "contextCompaction"
                        ) {
                            return Err(format!(
                                "No-tool probe received unexpected activity: {kind}"
                            )
                            .into());
                        }
                    }
                    if method == "turn/completed" {
                        if params["turn"]["status"] != "completed" {
                            return Err("Native probe turn failed or was interrupted".into());
                        }
                        let turn = params["turn"]["id"]
                            .as_str()
                            .ok_or("Missing completed turn ID")?;
                        self.completed.insert((thread.into(), turn.into()));
                    }
                }
                self.book.notification(&method, &params)?;
            }
            Event::ServerRequest { method, .. } => {
                return Err(
                    format!("Unexpected decision request {method}; no permission granted").into(),
                );
            }
            Event::Closed { reason } => return Err(reason.into()),
            Event::Ready { .. } => {}
        }
        Ok(())
    }

    fn pump(&mut self) -> Result<()> {
        match self.events.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => self.event(event),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(()), // Observation only, never cancels work.
            Err(error) => Err(error.into()),
        }
    }

    fn finish(&mut self, request: Request, ticket: Ticket) -> Result<Outcome> {
        loop {
            match ticket.try_result() {
                Ok(result) => {
                    // Record new IDs before projection so cleanup also runs if hydration fails.
                    if matches!(request.call.method, "thread/start" | "thread/fork")
                        && let Ok(value) = &result
                        && let Some(id) = value["thread"]["id"].as_str().filter(|id| !id.is_empty())
                    {
                        self.owned.insert(id.into());
                    }
                    return Ok(self.book.complete(&request, result)?);
                }
                Err(mpsc::TryRecvError::Empty) => self.pump()?,
                Err(error) => return Err(error.into()),
            }
        }
    }

    fn execute(&mut self, request: Request) -> Result<Outcome> {
        let ticket = request.call.clone().send(&self.client)?;
        self.finish(request, ticket)
    }

    fn open(&mut self, owner: &str, cwd: &Path) -> Result<String> {
        let request =
            self.book
                .open(owner, cwd, &api::Profile::default(), api::Access::ReadOnly)?;
        match self.execute(request)? {
            Outcome::Opened { thread_id, .. } => Ok(thread_id),
            _ => Err("Unexpected native open outcome".into()),
        }
    }

    fn start(
        &mut self,
        owner: &str,
        message: &str,
        text: &str,
        cwd: &Path,
    ) -> Result<(Request, Ticket)> {
        let request = self.book.start_turn(
            owner,
            message,
            vec![api::text_input(text)],
            cwd,
            &api::Profile::default(),
            api::Access::ReadOnly,
        )?;
        let ticket = request.call.clone().send(&self.client)?;
        Ok((request, ticket))
    }

    fn accept(&mut self, pending: (Request, Ticket)) -> Result<(String, String)> {
        let owner = pending.0.owner.clone();
        match self.finish(pending.0, pending.1)? {
            Outcome::Accepted { turn_id, .. } => {
                let thread = self
                    .book
                    .binding(&owner)
                    .ok_or("Missing accepted binding")?
                    .thread_id
                    .clone();
                Ok((thread, turn_id))
            }
            _ => Err("Unexpected native acceptance outcome".into()),
        }
    }

    fn wait_turns(&mut self, targets: &[(String, String)]) -> Result<()> {
        while !targets.iter().all(|target| self.completed.contains(target)) {
            self.pump()?;
        }
        Ok(())
    }

    fn check_history(
        &self,
        owner: &str,
        expected: &str,
        excluded: &str,
        turns: usize,
    ) -> Result<()> {
        let id = &self.book.binding(owner).ok_or("Missing owner")?.thread_id;
        let thread = self
            .book
            .mirror
            .thread(id)
            .ok_or("Missing native history projection")?;
        if thread.turns.len() != turns || thread.turns.iter().any(|turn| turn.status != "completed")
        {
            return Err(format!("Unexpected native turn history for {owner}").into());
        }
        for turn in &thread.turns {
            let replies: Vec<_> = turn
                .items
                .iter()
                .filter(|item| item.value["type"] == "agentMessage")
                .collect();
            if !replies.iter().any(|item| {
                item.completed
                    && item.value["text"]
                        .as_str()
                        .is_some_and(|text| text.contains(expected))
            }) {
                return Err(format!(
                    "Expected marker missing from authoritative answer for {owner}"
                )
                .into());
            }
            if turn
                .items
                .iter()
                .any(|item| item.value.to_string().contains(excluded))
            {
                return Err(format!("Cross-conversation marker leaked into {owner}").into());
            }
        }
        Ok(())
    }

    fn cleanup(&mut self) -> Result<()> {
        // All IDs originate in this probe's successful create/fork responses.
        // Continue cleaning the other owned IDs if a single deletion fails.
        let mut failures = vec![];
        for id in self.owned.clone() {
            if let Err(error) = api::delete_thread(&id)
                .send(&self.client)
                .and_then(|ticket| ticket.wait())
            {
                failures.push(format!("Test thread {id}: {error}"));
            } else {
                self.owned.remove(&id);
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; ").into())
        }
    }
}

fn main() -> Result<()> {
    if !std::env::args().any(|arg| arg == "--allow-test-inference") {
        return Err("Pass --allow-test-inference to create two disposable native conversations and one branch, submit three tiny prompts using account allowance, reconnect, and delete only these test conversations.".into());
    }
    let chat_dir = tempfile::tempdir()?;
    let graph_dir = tempfile::tempdir()?;
    let runtime = Runtime::discover(chat_dir.path())?;
    let mut probe = Probe::connect(&runtime, Conversations::default(), BTreeSet::new())?;
    let result = (|| -> Result<()> {
        let chat_id = probe.open(CHAT, chat_dir.path())?;
        let graph_id = probe.open(GRAPH, graph_dir.path())?;
        if chat_id == graph_id {
            return Err("Native owners share a thread".into());
        }
        let a = probe.start(CHAT, "probe-first-chat", "My marker is ALPHA_73. Reply exactly ALPHA_73. Do not use tools, access files or modify anything.", chat_dir.path())?;
        let b = probe.start(GRAPH, "probe-first-graph", "My marker is BETA_29. Reply exactly BETA_29. Do not use tools, access files or modify anything.", graph_dir.path())?;
        println!("Two native turn/start calls dispatched before awaiting either acknowledgement.");
        let a = probe.accept(a)?;
        let b = probe.accept(b)?;
        probe.wait_turns(&[a, b])?;
        probe.check_history(CHAT, ALPHA, BETA, 1)?;
        probe.check_history(GRAPH, BETA, ALPHA, 1)?;
        if ![&chat_id, &graph_id]
            .iter()
            .all(|id| probe.streamed.contains(*id))
        {
            return Err("Missing live agent-message deltas for an owner".into());
        }
        let saved = serde_json::to_string(probe.book.saved())?;
        if saved.contains(ALPHA) || saved.contains(BETA) {
            return Err("Local bindings contain transcript content".into());
        }
        println!("Both native turns completed; streamed answers and owner isolation verified.");
        let rename = probe
            .book
            .action(CHAT, Action::Rename("Lifecycle acceptance fixture".into()))?;
        probe.execute(rename)?;
        let read = probe.book.action(CHAT, Action::Read)?;
        probe.execute(read)?;
        if probe
            .book
            .mirror
            .thread(&chat_id)
            .and_then(|thread| thread.name.as_deref())
            != Some("Lifecycle acceptance fixture")
        {
            return Err("Native rename did not survive thread/read".into());
        }
        let fork = probe.book.action(
            CHAT,
            Action::Fork {
                destination: BRANCH.into(),
            },
        )?;
        let branch_id = match probe.execute(fork)? {
            Outcome::Forked { thread_id, .. } => thread_id,
            _ => return Err("Unexpected native fork result".into()),
        };
        if branch_id == chat_id || branch_id == graph_id {
            return Err("Fork did not create an independent ID".into());
        }
        probe.check_history(BRANCH, ALPHA, BETA, 1)?;
        let archive = probe.book.action(GRAPH, Action::Archive)?;
        probe.execute(archive)?;
        if !probe
            .book
            .binding(GRAPH)
            .is_some_and(|binding| binding.archived)
        {
            return Err("Native archive was not projected".into());
        }
        let unarchive = probe.book.action(GRAPH, Action::Unarchive)?;
        probe.execute(unarchive)?;
        if probe
            .book
            .binding(GRAPH)
            .is_none_or(|binding| binding.archived)
        {
            return Err("Native unarchive was not projected".into());
        }
        // Persist again after lifecycle operations, including the independent branch.
        let saved = serde_json::to_string(probe.book.saved())?;
        if saved.contains(ALPHA)
            || saved.contains(BETA)
            || !probe.book.saved().unresolved.is_empty()
        {
            return Err(
                "Saved bindings contain transcript data or unresolved accepted turns".into(),
            );
        }
        println!("Native rename/read, independent fork and archive/unarchive verified.");
        probe.book.disconnect();
        probe.client.shutdown();
        let restored = Conversations::restore(serde_json::from_str(&saved)?)?;
        let owned = probe.owned.clone();
        probe = Probe::connect(&runtime, restored, owned)?;
        for owner in [CHAT, GRAPH, BRANCH] {
            let read = probe.book.action(owner, Action::Read)?;
            probe.execute(read)?;
        }
        probe.check_history(CHAT, ALPHA, BETA, 1)?;
        probe.check_history(GRAPH, BETA, ALPHA, 1)?;
        probe.check_history(BRANCH, ALPHA, BETA, 1)?;
        if probe.open(CHAT, chat_dir.path())? != chat_id
            || probe.open(GRAPH, graph_dir.path())? != graph_id
        {
            return Err("Reconnect changed a native thread ID".into());
        }
        let pending = probe.start(CHAT, "probe-continued-chat", "Return only the marker from my first message in this conversation. Do not use tools, access files or modify anything.", chat_dir.path())?;
        let target = probe.accept(pending)?;
        probe.wait_turns(&[target])?;
        probe.check_history(CHAT, ALPHA, BETA, 2)?;
        probe.check_history(GRAPH, BETA, ALPHA, 1)?;
        probe.check_history(BRANCH, ALPHA, BETA, 1)?;
        println!(
            "Fresh connection restored both native histories; follow-up recovered its marker without transcript replay."
        );
        if std::env::args().any(|arg| arg == "--exercise-compaction") {
            while !probe.book.can_compact(CHAT) {
                probe.pump()?;
            }
            let compact = probe.book.action(
                CHAT,
                Action::Compact {
                    operation_id: "probe-compaction".into(),
                },
            )?;
            probe.execute(compact)?;
            while probe.book.compaction_pending(CHAT) {
                probe.pump()?;
            }
            if probe.book.saved().unresolved.contains_key(CHAT) {
                return Err("Compaction ended without confirmed native evidence".into());
            }
            if !probe.book.mirror.thread(&chat_id).is_some_and(|thread| {
                thread
                    .turns
                    .iter()
                    .flat_map(|turn| &turn.items)
                    .any(|item| item.completed && item.value["type"] == "contextCompaction")
            }) {
                return Err("Native compaction completed without its expected item".into());
            }
            probe.check_history(GRAPH, BETA, ALPHA, 1)?;
            probe.check_history(BRANCH, ALPHA, BETA, 1)?;
            println!(
                "Native compaction completed with its contextCompaction item; sibling histories are unchanged."
            );
        }
        let delete = probe.book.action(BRANCH, Action::Delete)?;
        if !matches!(probe.execute(delete)?, Outcome::Deleted { .. }) {
            return Err("Unexpected native deletion outcome".into());
        }
        probe.owned.remove(&branch_id);
        if !probe
            .book
            .binding(BRANCH)
            .is_some_and(|binding| binding.deleted)
            || probe.book.mirror.thread(&branch_id).is_some()
        {
            return Err("Native deletion did not leave a transcript-free terminal binding".into());
        }
        println!(
            "Native delete cleared only the branch transcript and retained its terminal binding."
        );
        Ok(())
    })();
    // Reconnect solely for cleanup if the private test connection failed.
    let cleanup: Result<()> = match probe.cleanup() {
        Ok(()) => Ok(()),
        Err(first) => {
            probe.client.shutdown();
            match Probe::connect(&runtime, Conversations::default(), probe.owned.clone()) {
                Ok(mut cleaner) => {
                    let cleanup = cleaner.cleanup();
                    cleaner.client.shutdown();
                    cleanup.map_err(|second| format!("Cleanup failed: {first}; {second}").into())
                }
                Err(second) => Err(format!("Cleanup unavailable: {first}; {second}").into()),
            }
        }
    };
    probe.client.shutdown();
    cleanup?;
    result?;
    println!(
        "Codex {} lifecycle probe passed; test conversations deleted. No personal history/config or Central Agent binding file changed.",
        runtime.version()
    );
    Ok(())
}
