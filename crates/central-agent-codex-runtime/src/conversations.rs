//! Client ownership and delivery correlation, not an agent implementation.
//! Only native thread/session IDs and unresolved delivery receipts are durable.
//! Transcript projection is display-only; Codex owns the actual conversation.
use crate::{
    api::{self, Access, Call, Profile},
    mirror::Mirror,
    transport::CallError,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
};
mod compaction;
mod delegation;
pub use delegation::Delegation;
mod management;
mod review_refresh;
pub use management::{GitConfirmation, RevertConfirmation, RevertReceipt, SectionMoveConfirmation};

pub struct TurnStart<'a> {
    pub owner: &'a str,
    pub message_id: &'a str,
    pub input: Vec<Value>,
    pub cwd: &'a Path,
    pub profile: &'a Profile,
    pub access: Access,
    pub options: &'a api::TurnOptions,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Binding {
    pub thread_id: String,
    pub session_id: Option<String>,
    pub archived: bool,
    /// Native deletion is terminal. Keep only an ID tombstone so queued input
    /// or a restart cannot silently create a replacement conversation.
    #[serde(default)]
    pub deleted: bool,
    /// Only newly opened sessions with no submitted turn are eligible for an
    /// explicit local-link reset. Older/imported bindings default conservatively.
    #[serde(default)]
    pub never_submitted: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Receipt {
    pub thread_id: String,
    pub message_id: String,
    pub steer: bool,
    #[serde(default)]
    pub review: bool,
    #[serde(default)]
    pub compact: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Saved {
    pub version: u32,
    pub bindings: BTreeMap<String, Binding>,
    pub unresolved: BTreeMap<String, Receipt>,
    /// Destination -> native source ID. No prompt or transcript is stored.
    #[serde(default)]
    pub fork_origins: BTreeMap<String, String>,
    /// Confirmed child native ID -> source native ID, for display-only ancestry.
    /// Separate from unresolved fork receipts; never grants replay authority.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub lineage: BTreeMap<String, String>,
    /// Native agent delegation, distinct from independent user-created forks.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub delegations: BTreeMap<String, String>,
    /// Write-ahead destructive-history intents. IDs only, never transcript text.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub reverts: BTreeMap<String, RevertReceipt>,
}
impl Default for Saved {
    fn default() -> Self {
        Self {
            version: 1,
            bindings: BTreeMap::new(),
            unresolved: BTreeMap::new(),
            fork_origins: BTreeMap::new(),
            lineage: BTreeMap::new(),
            delegations: BTreeMap::new(),
            reverts: BTreeMap::new(),
        }
    }
}
impl Saved {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported App Server binding store version".into());
        }
        let mut threads = HashSet::new();
        delegation::validate_links(&self.delegations)?;
        for (child, parent) in &self.lineage {
            if !lineage_id(child) || !lineage_id(parent) || child == parent {
                return Err("Invalid native conversation ancestry".into());
            }
        }
        for (destination, source) in &self.fork_origins {
            if !valid_owner(destination)
                || source.is_empty()
                || self.bindings.contains_key(destination)
            {
                return Err("Invalid pending native fork destination".into());
            }
        }
        for (owner, binding) in &self.bindings {
            if !valid_owner(owner)
                || binding.thread_id.is_empty()
                || !threads.insert(&binding.thread_id)
            {
                return Err("Invalid or duplicated App Server conversation binding".into());
            }
        }
        for (owner, receipt) in &self.unresolved {
            if receipt.message_id.is_empty()
                || u8::from(receipt.steer) + u8::from(receipt.review) + u8::from(receipt.compact)
                    > 1
                || self
                    .bindings
                    .get(owner)
                    .is_none_or(|b| b.thread_id != receipt.thread_id)
            {
                return Err("An App Server delivery receipt has no matching conversation".into());
            }
        }
        for (owner, receipt) in &self.reverts {
            receipt.validate()?;
            if self
                .bindings
                .get(owner)
                .is_none_or(|b| b.thread_id != receipt.thread_id || b.deleted)
                || self.unresolved.contains_key(owner)
            {
                return Err("A revert receipt has no exclusive matching conversation".into());
            }
        }
        Ok(())
    }
}

fn valid_owner(owner: &str) -> bool {
    ["chat:", "graph:", "draft:"]
        .iter()
        .any(|prefix| owner.strip_prefix(prefix).is_some_and(|id| !id.is_empty()))
}

#[derive(Clone, Debug)]
enum Kind {
    PrepareReview {
        cwd: String,
    },
    Review,
    DetachedReview {
        destination: String,
    },
    Import {
        source: String,
        archived: bool,
        fork: bool,
    },
    Open,
    Read,
    Start {
        message_id: String,
    },
    Steer {
        message_id: String,
        expected_turn: String,
    },
    Interrupt,
    Rename,
    Archive,
    Unarchive,
    Delete,
    Fork {
        destination: String,
    },
    Compact,
    GitMetadata {
        cwd: std::path::PathBuf,
        git: crate::thread_metadata::GitMetadata,
    },
    MoveSection,
    Revert,
}

/// Immutable destination captured before dispatch. It cannot be constructed by
/// website content or substituted with the currently selected conversation.
#[derive(Clone, Debug)]
pub struct Request {
    pub call: Call,
    pub owner: String,
    serial: u64,
    generation: u64,
    thread_id: Option<String>,
    revision: u64,
    lifecycle_revision: u64,
    kind: Kind,
}

impl Request {
    /// Local dispatch/acknowledgement correlation, not a native JSON-RPC ID.
    pub fn diagnostic_id(&self) -> String {
        format!("host:{}:{}", self.generation, self.serial)
    }
    /// Whether this operation is expected to publish the requested native
    /// history after its identity/session response has been validated.
    pub fn requests_history_hydration(&self) -> bool {
        matches!(
            self.kind,
            Kind::Open
                | Kind::Read
                | Kind::Fork { .. }
                | Kind::Import { fork: true, .. }
                | Kind::Revert
                | Kind::MoveSection
        )
    }

    /// Paginated threads first return metadata so a bounded page reader can be
    /// selected without ever requesting an unbounded transcript frame.
    pub fn omits_turns(&self) -> bool {
        self.call.params["includeTurns"] == false
            || self.call.params["excludeTurns"] == true
            || matches!(self.kind, Kind::Revert | Kind::MoveSection)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    ReviewReady {
        owner: String,
    },
    Opened {
        owner: String,
        thread_id: String,
    },
    Accepted {
        owner: String,
        message_id: String,
        turn_id: String,
    },
    Updated {
        owner: String,
    },
    Forked {
        owner: String,
        thread_id: String,
    },
    DetachedReview {
        source_owner: String,
        owner: String,
        thread_id: String,
    },
    Deleted {
        owner: String,
    },
}

/// Frozen local unlink intent. It removes only Supervisor ownership; native
/// history remains in Codex and unsubscribe is connection-local.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unlink {
    pub owner: String,
    pub thread_id: String,
    pub subscribed: bool,
}

#[derive(Clone, Debug)]
pub enum Action {
    Read,
    Rename(String),
    Archive,
    Unarchive,
    Delete,
    Fork {
        destination: String,
    },
    ForkWithOptions {
        destination: String,
        options: Box<api::ThreadForkOptions>,
    },
    Compact {
        operation_id: String,
    },
    Interrupt,
}

#[derive(Default)]
pub struct Conversations {
    saved: Saved,
    pub mirror: Mirror,
    generation: u64,
    serial: u64,
    pending: HashMap<u64, Request>,
    loaded: HashSet<String>,
    observed: HashSet<String>,
    lifecycle_revisions: HashMap<String, u64>,
    compactions: HashMap<String, compaction::Pending>,
    review_refreshes: HashMap<String, review_refresh::Refresh>,
    // Unowned thread/started snapshots observed while a detached review RPC is
    // pending. They are connection-local and become authoritative only when
    // the matching review/start response returns their exact ID.
    detached_threads: HashMap<String, Value>,
    initializing_delegates: HashSet<String>,
    unsubscribing: HashSet<String>,
    management: management::State,
}

impl Conversations {
    /// Connection-local native ownership, not a possibly stale display status.
    pub fn is_thread_loaded(&self, owner: &str) -> bool {
        self.binding(owner).is_some_and(|binding| {
            !binding.deleted && !binding.archived && self.loaded.contains(&binding.thread_id)
        })
    }
    pub fn ready_for_turn(&self, owner: &str) -> bool {
        self.binding(owner).is_some_and(|binding| {
            !binding.deleted
                && !binding.archived
                && self.loaded.contains(&binding.thread_id)
                && self.observed.contains(&binding.thread_id)
                && self
                    .mirror
                    .thread(&binding.thread_id)
                    .and_then(|thread| thread.status.as_ref())
                    .is_some_and(|status| status["type"] == "idle")
        }) && !self.busy(owner)
    }
    pub fn unused_link(&self, owner: &str) -> bool {
        self.binding(owner).is_some_and(|binding| {
            binding.never_submitted
                && !binding.deleted
                && !binding.archived
                && !self.loaded.contains(&binding.thread_id)
        })
    }
    /// Explicit local bookkeeping only. Never deletes or replaces native history.
    pub fn reset_unused_link(&mut self, owner: &str) -> Result<(), String> {
        if !self.unused_link(owner)
            || self.busy(owner)
            || self.pending.values().any(|request| request.owner == owner)
        {
            return Err("Only a disconnected, unused native link can be reset. Existing history is unchanged.".into());
        }
        let binding = self.saved.bindings.remove(owner).unwrap();
        self.mirror.forget(&binding.thread_id);
        self.observed.remove(&binding.thread_id);
        Ok(())
    }

    pub fn prepare_unlink(&self, owner: &str) -> Result<Option<Unlink>, String> {
        let Some(binding) = self.binding(owner) else {
            if self.saved.fork_origins.contains_key(owner) {
                return Err(
                    "Resolve this uncertain native fork before removing its local surface".into(),
                );
            }
            return Ok(None);
        };
        if self.busy(owner)
            || self.pending.values().any(|request| request.owner == owner)
            || self.saved.unresolved.contains_key(owner)
        {
            return Err(
                "Stop or reconcile this native conversation before removing its local surface"
                    .into(),
            );
        }
        Ok(Some(Unlink {
            owner: owner.into(),
            thread_id: binding.thread_id.clone(),
            subscribed: self.loaded.contains(&binding.thread_id),
        }))
    }

    /// Call only after the host has atomically persisted the matching Saved
    /// snapshot. Returns a best-effort connection-local unsubscribe request.
    pub fn commit_unlink(&mut self, unlink: &Unlink) -> Result<Option<Call>, String> {
        if self
            .binding(&unlink.owner)
            .is_none_or(|binding| binding.thread_id != unlink.thread_id)
        {
            return Err("The native conversation changed before it could be unlinked".into());
        }
        self.saved.bindings.remove(&unlink.owner);
        self.saved.lineage.remove(&unlink.thread_id);
        self.saved.delegations.remove(&unlink.thread_id);
        self.initializing_delegates.remove(&unlink.thread_id);
        self.saved.unresolved.remove(&unlink.owner);
        self.saved.reverts.remove(&unlink.owner);
        self.management.forget(&unlink.owner);
        self.saved.fork_origins.remove(&unlink.owner);
        self.loaded.remove(&unlink.thread_id);
        self.observed.remove(&unlink.thread_id);
        self.compactions.remove(&unlink.owner);
        self.review_refreshes.remove(&unlink.owner);
        self.mirror.forget(&unlink.thread_id);
        if unlink.subscribed {
            self.unsubscribing.insert(unlink.thread_id.clone());
            Ok(Some(api::unsubscribe_thread(&unlink.thread_id)))
        } else {
            Ok(None)
        }
    }

    pub fn complete_unsubscribe(
        &mut self,
        thread_id: &str,
        result: Result<Value, CallError>,
    ) -> Result<(), String> {
        if !self.unsubscribing.contains(thread_id) {
            return Err("Stale native unsubscribe response".into());
        }
        match result {
            Ok(value)
                if matches!(
                    value.get("status").and_then(Value::as_str),
                    Some("unsubscribed" | "notSubscribed" | "notLoaded")
                ) =>
            {
                self.unsubscribing.remove(thread_id);
                Ok(())
            }
            Ok(_) => {
                self.unsubscribing.remove(thread_id);
                Err("Codex returned an invalid unsubscribe status".into())
            }
            Err(CallError::Disconnected {
                delivery_unknown: true,
                reason,
            }) => Err(format!(
                "Codex disconnected while releasing a native subscription: {reason}"
            )),
            Err(error) => {
                self.unsubscribing.remove(thread_id);
                Err(format!(
                    "Codex could not release a native subscription: {error}"
                ))
            }
        }
    }

    /// Reconcile the atomic result of thread/loaded/list on a fresh connection.
    /// It never creates ownership or treats a loaded ID as observed history.
    pub fn reconcile_loaded(&mut self, ids: &HashSet<String>) -> Vec<String> {
        let bindings = self
            .saved
            .bindings
            .iter()
            .map(|(owner, binding)| (owner.clone(), binding.clone()))
            .collect::<Vec<_>>();
        let mut changed = Vec::new();
        for (owner, binding) in bindings {
            let before = self.loaded.contains(&binding.thread_id);
            let after = !binding.deleted && !binding.archived && ids.contains(&binding.thread_id);
            if after {
                self.loaded.insert(binding.thread_id.clone());
            } else {
                self.loaded.remove(&binding.thread_id);
                self.mirror.unload(&binding.thread_id);
            }
            if before != after {
                changed.push(owner);
            }
        }
        changed
    }
    pub fn prepare_review(&mut self, owner: &str, cwd: &Path) -> Result<Request, String> {
        validate_cwd(cwd)?;
        let binding = self
            .binding(owner)
            .ok_or("Load a native conversation before starting a review")?;
        if binding.deleted || binding.archived || self.busy(owner) || !self.observed(owner) {
            return Err("Load an idle, non-archived native conversation before reviewing".into());
        }
        let cwd = cwd.to_str().unwrap().to_owned();
        Ok(self.request(
            owner,
            api::prepare_review(&binding.thread_id, &cwd),
            Kind::PrepareReview { cwd },
        ))
    }
    pub fn start_review(
        &mut self,
        owner: &str,
        target: &api::ReviewTarget,
        receipt_id: &str,
    ) -> Result<Request, String> {
        target.validate()?;
        let binding = self.binding(owner).ok_or("No native review conversation")?;
        if receipt_id.is_empty()
            || binding.deleted
            || binding.archived
            || self.busy(owner)
            || !self.loaded.contains(&binding.thread_id)
        {
            return Err("Prepare an idle native conversation before reviewing".into());
        }
        let thread = binding.thread_id.clone();
        self.saved.bindings.get_mut(owner).unwrap().never_submitted = false;
        self.saved.unresolved.insert(
            owner.into(),
            Receipt {
                thread_id: thread.clone(),
                message_id: receipt_id.into(),
                steer: false,
                review: true,
                compact: false,
            },
        );
        Ok(self.request(owner, api::start_review(&thread, target), Kind::Review))
    }
    pub fn start_detached_review(
        &mut self,
        owner: &str,
        destination: &str,
        target: &api::ReviewTarget,
        receipt_id: &str,
    ) -> Result<Request, String> {
        target.validate()?;
        let binding = self.binding(owner).ok_or("No native review conversation")?;
        if receipt_id.is_empty()
            || !valid_owner(destination)
            || self.binding(destination).is_some()
            || self.saved.fork_origins.contains_key(destination)
            || self.pending.values().any(|request| {
                request.owner == destination
                    || matches!(&request.kind, Kind::Fork { destination: pending } | Kind::DetachedReview { destination: pending } if pending == destination)
            })
            || binding.deleted
            || binding.archived
            || self.busy(owner)
            || !self.loaded.contains(&binding.thread_id)
        {
            return Err("Prepare an idle native conversation and a new local destination before starting a separate review".into());
        }
        let thread = binding.thread_id.clone();
        self.saved.bindings.get_mut(owner).unwrap().never_submitted = false;
        self.saved.unresolved.insert(
            owner.into(),
            Receipt {
                thread_id: thread.clone(),
                message_id: receipt_id.into(),
                steer: false,
                review: true,
                compact: false,
            },
        );
        self.saved
            .fork_origins
            .insert(destination.into(), thread.clone());
        Ok(self.request(
            owner,
            api::start_detached_review(&thread, target),
            Kind::DetachedReview {
                destination: destination.into(),
            },
        ))
    }
    /// Host must require an explicit choice from a native list result. Import
    /// reads history without resuming it; fork asks Codex to copy it to a new ID.
    /// Neither operation submits a model prompt or replaces an existing binding.
    pub fn import(
        &mut self,
        owner: &str,
        source: &str,
        archived: bool,
        fork: bool,
    ) -> Result<Request, String> {
        if fork && archived {
            return Err("Codex requires an active source to fork. Link and explicitly restore the archived conversation first; nothing is restored automatically".into());
        }
        if !valid_owner(owner)
            || source.is_empty()
            || self.unsubscribing.contains(source)
            || self.binding(owner).is_some()
            || self.busy(owner)
            || self
                .pending
                .values()
                .any(|r| matches!(&r.kind, Kind::Fork {destination} if destination == owner))
        {
            return Err(
                "Choose a local conversation without a Codex binding or pending work".into(),
            );
        }
        if !fork && (self.owner_for_thread(source).is_some() || self.pending.values().any(|r| matches!(&r.kind, Kind::Import { source:pending, fork:false, ..} if pending == source))) {
            return Err("This native conversation is already linked here; create a fork instead".into());
        }
        if fork && self.saved.fork_origins.contains_key(owner) {
            return Err(
                "A previous fork needs native history review before creating another here".into(),
            );
        }
        let call = if fork {
            api::fork_thread_with_options(
                source,
                &api::ThreadForkOptions {
                    exclude_turns: true,
                    ..api::ThreadForkOptions::default()
                },
            )
        } else {
            // Linking needs identity and lifecycle metadata only. Reading a full
            // private transcript here provides no authority and can make a long
            // conversation exceed a single transport frame before it is loaded.
            api::read_thread_metadata(source)
        };
        if fork {
            self.saved.fork_origins.insert(owner.into(), source.into());
        }
        let mut request = self.request(
            owner,
            call,
            Kind::Import {
                source: source.into(),
                archived,
                fork,
            },
        );
        request.lifecycle_revision = self.lifecycle_revisions.get(source).copied().unwrap_or(0);
        self.pending.insert(request.serial, request.clone());
        Ok(request)
    }
    pub fn restore(saved: Saved) -> Result<Self, String> {
        saved.validate()?;
        Ok(Self {
            saved,
            ..Self::default()
        })
    }
    pub fn saved(&self) -> &Saved {
        &self.saved
    }
    pub fn owner_for_thread(&self, id: &str) -> Option<&str> {
        self.saved
            .bindings
            .iter()
            .find(|(_, binding)| binding.thread_id == id)
            .map(|(owner, _)| owner.as_str())
    }
    pub fn binding(&self, owner: &str) -> Option<&Binding> {
        self.saved.bindings.get(owner)
    }
    pub fn active_turn(&self, owner: &str) -> Option<&str> {
        let binding = self.binding(owner)?;
        if binding.deleted || binding.archived || !self.loaded.contains(&binding.thread_id) {
            return None;
        }
        self.mirror
            .thread(&binding.thread_id)?
            .active_turn()
            .map(|turn| turn.id.as_str())
    }
    pub fn busy(&self, owner: &str) -> bool {
        self.active_turn(owner).is_some()
            || self.binding(owner).is_some_and(|b| self.initializing_delegates.contains(&b.thread_id))
            || self
                .binding(owner)
                .filter(|b| !b.deleted && !b.archived && self.loaded.contains(&b.thread_id))
                .and_then(|binding| self.mirror.thread(&binding.thread_id))
                .and_then(|thread| thread.status.as_ref())
                .is_some_and(|status| status.get("type").and_then(Value::as_str) == Some("active"))
            || self.pending.values().any(|request| {
                (request.owner == owner && !matches!(request.kind, Kind::Read))
                    || matches!(&request.kind,Kind::Fork {destination} | Kind::DetachedReview {destination} if destination==owner)
            })
            || self.saved.unresolved.contains_key(owner)
            || self.saved.reverts.contains_key(owner)
            || self.management.refresh_required.contains(owner)
    }

    pub fn any_busy(&self) -> bool {
        self.pending.values().any(|r| !matches!(r.kind, Kind::Read))
            || self.saved.bindings.keys().any(|owner| self.busy(owner))
    }

    fn request(&mut self, owner: &str, call: Call, kind: Kind) -> Request {
        self.serial += 1;
        let request = Request {
            call,
            owner: owner.to_owned(),
            serial: self.serial,
            generation: self.generation,
            thread_id: self.binding(owner).map(|binding| binding.thread_id.clone()),
            revision: self.mirror.revision(),
            lifecycle_revision: self.lifecycle_revision(owner),
            kind,
        };
        self.pending.insert(request.serial, request.clone());
        request
    }

    /// A new binding must be saved successfully by the host before start_turn.
    /// This opens/resumes native history by ID, without supplying a transcript.
    pub fn open(
        &mut self,
        owner: &str,
        cwd: &Path,
        profile: &Profile,
        access: Access,
    ) -> Result<Request, String> {
        if self.saved.fork_origins.contains_key(owner) {
            return Err("A previous native fork has an uncertain result. Browse native history and link that fork before sending a prompt here.".into());
        }
        if !valid_owner(owner) {
            return Err("The conversation needs a stable local owner".into());
        }
        if self.pending.values().any(
            |request| matches!(&request.kind, Kind::Fork {destination} if destination == owner),
        ) {
            return Err("This conversation is the destination of a pending native fork".into());
        }
        if self.saved.unresolved.contains_key(owner)
            || self.saved.reverts.contains_key(owner)
            || self.management.refresh_required.contains(owner)
            || self
                .pending
                .values()
                .any(|r| r.owner == owner && !matches!(r.kind, Kind::Read))
            || (self
                .binding(owner)
                .is_some_and(|b| self.loaded.contains(&b.thread_id))
                && self.active_turn(owner).is_some())
        {
            return Err("This conversation is busy or needs delivery reconciliation".into());
        }
        validate_cwd(cwd)?;
        let call = if let Some(binding) = self.binding(owner) {
            if binding.deleted {
                return Err("This native conversation was deleted. Start a new local chat to use Codex again.".into());
            }
            if binding.archived {
                return Err("Restore the archived conversation before resuming it".into());
            }
            api::resume_thread_with_options(
                &binding.thread_id,
                &api::ThreadResumeOptions {
                    exclude_turns: true,
                    ..api::ThreadResumeOptions::default()
                },
            )
        } else {
            api::start_thread(&cwd.to_string_lossy(), profile, access)
        };
        Ok(self.request(owner, call, Kind::Open))
    }

    pub fn start_turn(
        &mut self,
        owner: &str,
        message_id: &str,
        input: Vec<Value>,
        cwd: &Path,
        profile: &Profile,
        access: Access,
    ) -> Result<Request, String> {
        self.start_turn_with_options(TurnStart {
            owner,
            message_id,
            input,
            cwd,
            profile,
            access,
            options: &api::TurnOptions::default(),
        })
    }

    pub fn start_turn_with_options(&mut self, request: TurnStart<'_>) -> Result<Request, String> {
        let TurnStart {
            owner,
            message_id,
            input,
            cwd,
            profile,
            access,
            options,
        } = request;
        if self.busy(owner) {
            return Err(
                "Resolve the active turn or uncertain delivery before submitting again".into(),
            );
        }
        validate_cwd(cwd)?;
        let binding = self
            .binding(owner)
            .ok_or("Open the native conversation first")?;
        if binding.deleted || binding.archived || !self.loaded.contains(&binding.thread_id) {
            return Err("Resume the native conversation before starting a turn".into());
        }
        if message_id.is_empty() || input.is_empty() {
            return Err("A turn requires input and a unique client message ID".into());
        }
        let call = api::start_turn_with_options(
            &binding.thread_id,
            message_id,
            input,
            &cwd.to_string_lossy(),
            profile,
            access,
            options,
        );
        let thread_id = binding.thread_id.clone();
        self.saved.bindings.get_mut(owner).unwrap().never_submitted = false;
        // Write-ahead metadata: if the host crashes after sending, it can reconcile
        // by clientUserMessageId. No prompt or credential is stored in this receipt.
        self.saved.unresolved.insert(
            owner.to_owned(),
            Receipt {
                thread_id,
                message_id: message_id.to_owned(),
                steer: false,
                review: false,
                compact: false,
            },
        );
        Ok(self.request(
            owner,
            call,
            Kind::Start {
                message_id: message_id.to_owned(),
            },
        ))
    }

    pub fn steer_target(&self, owner: &str) -> Option<&str> {
        let binding = self.binding(owner)?;
        if binding.deleted
            || binding.archived
            || !self.loaded.contains(&binding.thread_id)
            || !self.observed.contains(&binding.thread_id)
            || self.saved.unresolved.contains_key(owner)
            || self
                .pending
                .values()
                .any(|r| r.owner == owner && !matches!(r.kind, Kind::Read))
        {
            return None;
        }
        self.active_turn(owner)
    }

    pub fn steer(
        &mut self,
        owner: &str,
        expected_turn: &str,
        message_id: &str,
        input: Vec<Value>,
    ) -> Result<Request, String> {
        if self.saved.unresolved.contains_key(owner)
            || self
                .pending
                .values()
                .any(|r| r.owner == owner && !matches!(r.kind, Kind::Read))
        {
            return Err("Wait for the previous submission to be acknowledged".into());
        }
        if message_id.is_empty() || input.is_empty() {
            return Err("A follow-up requires input and a local receipt ID".into());
        }
        let turn = self
            .steer_target(owner)
            .ok_or("No active native turn to steer")?
            .to_owned();
        if turn != expected_turn {
            return Err(
                "The active native turn changed. Review the conversation before sending again."
                    .into(),
            );
        }
        let thread = self.binding(owner).unwrap().thread_id.clone();
        let call = api::steer_turn(&thread, &turn, message_id, input);
        // Both supported contracts echo this ID in the native user item. A lost
        // acknowledgement still requires explicit history review, never replay.
        self.saved.unresolved.insert(
            owner.into(),
            Receipt {
                thread_id: thread,
                message_id: message_id.into(),
                steer: true,
                review: false,
                compact: false,
            },
        );
        Ok(self.request(
            owner,
            call,
            Kind::Steer {
                message_id: message_id.into(),
                expected_turn: turn,
            },
        ))
    }

    /// Validate before allocating a local branch destination. Rechecked by
    /// `action` so callers cannot bypass the native lifecycle boundary.
    pub fn validate_fork_source(&self, owner: &str) -> Result<(), String> {
        let binding = self
            .binding(owner)
            .ok_or("No Supervisor-owned native conversation is bound here")?;
        if binding.deleted {
            return Err(
                "This native conversation was deleted; its history cannot be restored.".into(),
            );
        }
        if binding.archived {
            return Err("Codex cannot fork this archived conversation. Explicitly restore it first, then confirm a new fork.".into());
        }
        if self.busy(owner) {
            return Err("Finish or reconcile this conversation before creating a fork".into());
        }
        if !self.observed(owner) {
            return Err("Read native history after reconnecting before forking".into());
        }
        Ok(())
    }

    pub fn action(&mut self, owner: &str, action: Action) -> Result<Request, String> {
        if matches!(action, Action::Fork { .. } | Action::ForkWithOptions { .. }) {
            self.validate_fork_source(owner)?;
        }
        let binding = self
            .binding(owner)
            .ok_or("No Supervisor-owned native conversation is bound here")?;
        if binding.deleted {
            return Err(
                "This native conversation was deleted; its history cannot be restored.".into(),
            );
        }
        if !matches!(action, Action::Read | Action::Interrupt) && self.busy(owner) {
            return Err(
                "Finish or reconcile this conversation before changing its lifecycle".into(),
            );
        }
        let thread = binding.thread_id.clone();
        if !matches!(action, Action::Read | Action::Unarchive) && !self.observed.contains(&thread) {
            return Err(
                "Read or resume this conversation after reconnecting before changing it".into(),
            );
        }
        let (call, kind) = match action {
            Action::Read => {
                let call = if self
                    .mirror
                    .thread(&thread)
                    .and_then(|thread| thread.history_mode.as_deref())
                    == Some("legacy")
                {
                    api::read_thread(&thread)
                } else {
                    api::read_thread_metadata(&thread)
                };
                (call, Kind::Read)
            }
            Action::Rename(name) if !name.trim().is_empty() => {
                (api::rename_thread(&thread, &name), Kind::Rename)
            }
            Action::Rename(_) => return Err("The conversation name cannot be empty".into()),
            Action::Archive => (api::archive_thread(&thread), Kind::Archive),
            Action::Unarchive => (api::unarchive_thread(&thread), Kind::Unarchive),
            Action::Delete => (api::delete_thread(&thread), Kind::Delete),
            Action::Fork { destination } => {
                if !valid_owner(&destination) || self.binding(&destination).is_some() || self.pending.values().any(|r| r.owner == destination || matches!(&r.kind,Kind::Fork{destination:pending} if pending == &destination)) {
                    return Err("The fork requires a new, independent local conversation".into());
                }
                if self.saved.fork_origins.contains_key(&destination) {
                    return Err("The destination already has an unresolved fork".into());
                }
                self.saved
                    .fork_origins
                    .insert(destination.clone(), thread.clone());
                (
                    api::fork_thread_with_options(
                        &thread,
                        &api::ThreadForkOptions {
                            exclude_turns: true,
                            ..api::ThreadForkOptions::default()
                        },
                    ),
                    Kind::Fork { destination },
                )
            }
            Action::ForkWithOptions {
                destination,
                options,
            } => {
                if !valid_owner(&destination) || self.binding(&destination).is_some() || self.pending.values().any(|r| r.owner == destination || matches!(&r.kind,Kind::Fork{destination:pending} if pending == &destination)) {
                    return Err("The fork requires a new, independent local conversation".into());
                }
                if self.saved.fork_origins.contains_key(&destination) {
                    return Err("The destination already has an unresolved fork".into());
                }
                if let Some(last_turn_id) = options.last_turn_id.as_deref() {
                    let native = self
                        .mirror
                        .thread(&thread)
                        .and_then(|thread| thread.turns.iter().find(|turn| turn.id == last_turn_id))
                        .ok_or(
                            "The selected fork turn is no longer in the observed native history",
                        )?;
                    if native.active() {
                        return Err("Codex cannot fork through an in-progress turn".into());
                    }
                }
                self.saved
                    .fork_origins
                    .insert(destination.clone(), thread.clone());
                let mut options = *options;
                options.exclude_turns = true;
                (
                    api::fork_thread_with_options(&thread, &options),
                    Kind::Fork { destination },
                )
            }
            Action::Compact { operation_id } => {
                self.begin_compaction(owner, &operation_id)?;
                (api::compact_thread(&thread), Kind::Compact)
            }
            Action::Interrupt => (
                api::interrupt_turn(
                    &thread,
                    self.active_turn(owner)
                        .ok_or("No active turn to interrupt")?,
                ),
                Kind::Interrupt,
            ),
        };
        Ok(self.request(owner, call, kind))
    }

    /// Call only with the matching transport ticket result. Returns UI intent;
    /// it never starts another model turn or interprets tool commands.
    pub fn complete(
        &mut self,
        request: &Request,
        result: Result<Value, CallError>,
    ) -> Result<Outcome, String> {
        // Extract only IDs, but record nothing until the normal generation,
        // ownership and response checks have succeeded. Older histories are
        // backfilled by the existing explicit read/resume/import paths.
        let ancestry = result.as_ref().ok().and_then(|value| {
            if !matches!(
                request.call.method,
                "thread/start"
                    | "thread/resume"
                    | "thread/read"
                    | "thread/unarchive"
                    | "thread/fork"
            ) {
                return None;
            }
            let thread = value.get("thread")?;
            let child = thread.get("id")?.as_str()?;
            let reported = thread.get("forkedFromId").and_then(Value::as_str);
            let parent = if request.call.method == "thread/fork" {
                let expected = request.call.params.get("threadId")?.as_str()?;
                if reported.is_some_and(|parent| parent != expected) {
                    return None;
                }
                expected
            } else {
                reported?
            };
            (lineage_id(child) && lineage_id(parent) && child != parent)
                .then(|| (child.to_owned(), parent.to_owned()))
        });
        let outcome = self.complete_response(request, result)?;
        if let Some((child, parent)) = ancestry
            && self.owner_for_thread(&child).is_some()
            && !self.saved.delegations.contains_key(&child)
        {
            self.saved.lineage.insert(child, parent);
        }
        self.saved.lineage.retain(|child, _| {
            self.saved
                .bindings
                .values()
                .any(|binding| &binding.thread_id == child)
        });
        Ok(outcome)
    }

    fn complete_response(
        &mut self,
        request: &Request,
        result: Result<Value, CallError>,
    ) -> Result<Outcome, String> {
        let pending = self
            .pending
            .get(&request.serial)
            .ok_or("This request was already resolved or belongs to an old connection")?;
        if request.generation != self.generation
            || pending.owner != request.owner
            || pending.thread_id != request.thread_id
        {
            return Err("Stale conversation response".into());
        }
        self.pending.remove(&request.serial);
        if request.thread_id.as_deref()
            != self
                .binding(&request.owner)
                .map(|binding| binding.thread_id.as_str())
        {
            return Err("The conversation binding changed while the request was pending".into());
        }
        let value = match result {
            Ok(value) => value,
            Err(error) => {
                if matches!(request.kind, Kind::Compact) {
                    self.compactions.remove(&request.owner);
                }
                if !matches!(
                    &error,
                    CallError::Disconnected {
                        delivery_unknown: true,
                        ..
                    }
                ) {
                    // An internal RPC error may follow a durable destructive
                    // write. Only local/non-delivery rejection proves no send.
                    if matches!(request.kind, Kind::Revert)
                        && matches!(
                            error,
                            CallError::Rejected(_)
                                | CallError::Disconnected {
                                    delivery_unknown: false,
                                    ..
                                }
                        )
                    {
                        self.saved.reverts.remove(&request.owner);
                    }
                    match &request.kind {
                        Kind::Fork { destination } | Kind::DetachedReview { destination } => {
                            self.saved.fork_origins.remove(destination);
                        }
                        Kind::Import { fork: true, .. } => {
                            self.saved.fork_origins.remove(&request.owner);
                        }
                        _ => {}
                    }
                }
                if matches!(
                    request.kind,
                    Kind::Start { .. }
                        | Kind::Steer { .. }
                        | Kind::Review
                        | Kind::DetachedReview { .. }
                        | Kind::Compact
                ) && !matches!(
                    error,
                    CallError::Disconnected {
                        delivery_unknown: true,
                        ..
                    }
                ) {
                    self.saved.unresolved.remove(&request.owner);
                }
                return Err(error.to_string());
            }
        };
        // Deletion notifications can precede their RPC acknowledgement. No late
        // read/resume/start response may reconstruct the deleted native history.
        if self.binding(&request.owner).is_some_and(|b| b.deleted)
            && !matches!(request.kind, Kind::Delete)
        {
            return Err("Codex deleted this conversation while the request was pending.".into());
        }
        let lifecycle_changed =
            self.lifecycle_revision(&request.owner) != request.lifecycle_revision;
        match &request.kind {
            Kind::GitMetadata { .. } | Kind::MoveSection | Kind::Revert => {
                self.complete_management(request, value, lifecycle_changed)
            }
            Kind::PrepareReview { cwd } => {
                let mismatches: Vec<_> = [
                    (lifecycle_changed, "conversation changed during preparation"),
                    (
                        value["cwd"].as_str().is_none_or(|actual| {
                            !same_existing_directory(Path::new(actual), Path::new(cwd))
                        }),
                        "working directory",
                    ),
                    (
                        value["sandbox"]["type"] != "readOnly",
                        "read-only file access",
                    ),
                    (
                        value["sandbox"]["networkAccess"] != false,
                        "disabled network access",
                    ),
                    (
                        value["approvalPolicy"] != "untrusted",
                        "command approval policy",
                    ),
                    (
                        value["approvalsReviewer"] != "user",
                        "user approval reviewer",
                    ),
                ]
                .into_iter()
                .filter_map(|(mismatch, label)| mismatch.then_some(label))
                .collect();
                if !mismatches.is_empty() {
                    return Err(format!(
                        "Codex did not confirm the requested review settings: {}. No review was sent.",
                        mismatches.join(", ")
                    ));
                }
                let thread = value
                    .get("thread")
                    .ok_or("Native review preparation omitted its thread")?;
                let id = native_id(thread)?;
                if Some(id) != request.thread_id.as_deref() {
                    return Err("Review preparation returned another native conversation".into());
                }
                self.mirror
                    .hydrate_session_response(thread, &value, request.revision)?;
                self.loaded.insert(id.into());
                self.observed.insert(id.into());
                self.saved
                    .bindings
                    .get_mut(&request.owner)
                    .unwrap()
                    .session_id = thread["sessionId"].as_str().map(str::to_owned);
                Ok(Outcome::ReviewReady {
                    owner: request.owner.clone(),
                })
            }
            Kind::Review => {
                if value["reviewThreadId"].as_str() != request.thread_id.as_deref() {
                    return Err("Review returned another thread. Inspect native history; the request will not be replayed.".into());
                }
                let turn = value
                    .get("turn")
                    .ok_or("Review response omitted its turn")?;
                native_id(turn)?;
                self.mirror.notify(
                    "turn/started",
                    &json!({"threadId":request.thread_id,"turn":turn}),
                )?;
                self.saved.unresolved.remove(&request.owner);
                Ok(Outcome::Updated {
                    owner: request.owner.clone(),
                })
            }
            Kind::DetachedReview { destination } => {
                let review_thread = value
                    .get("reviewThreadId")
                    .and_then(Value::as_str)
                    .filter(|id| !id.is_empty())
                    .ok_or("Separate review response omitted its review thread")?;
                if Some(review_thread) == request.thread_id.as_deref()
                    || self.owner_for_thread(review_thread).is_some()
                    || self.binding(destination).is_some()
                {
                    return Err(
                        "Separate review did not return an independent native thread".into(),
                    );
                }
                let started = self.detached_threads.remove(review_thread);
                if let Some(started) = &started {
                    if native_id(started)? != review_thread
                        || started.get("forkedFromId").and_then(Value::as_str)
                            != request.thread_id.as_deref()
                    {
                        return Err(
                            "Separate review thread ownership did not match its source".into()
                        );
                    }
                    if let Err(error) = self.mirror.hydrate(started, request.revision) {
                        self.mirror.forget(review_thread);
                        return Err(error);
                    }
                }
                let turn = value
                    .get("turn")
                    .ok_or("Separate review response omitted its turn")?;
                native_id(turn)?;
                self.mirror.notify(
                    "turn/started",
                    &json!({"threadId":review_thread,"turn":turn}),
                )?;
                self.saved.bindings.insert(
                    destination.clone(),
                    Binding {
                        thread_id: review_thread.into(),
                        session_id: started
                            .as_ref()
                            .and_then(|thread| thread["sessionId"].as_str())
                            .map(str::to_owned),
                        archived: false,
                        deleted: false,
                        never_submitted: false,
                    },
                );
                self.loaded.insert(review_thread.into());
                self.observed.insert(review_thread.into());
                self.saved.fork_origins.remove(destination);
                self.saved.unresolved.remove(&request.owner);
                Ok(Outcome::DetachedReview {
                    source_owner: request.owner.clone(),
                    owner: destination.clone(),
                    thread_id: review_thread.into(),
                })
            }
            Kind::Import {
                source,
                archived,
                fork,
            } => {
                if self.binding(&request.owner).is_some()
                    || self.lifecycle_revisions.get(source).copied().unwrap_or(0)
                        != request.lifecycle_revision
                {
                    return Err("The import target or native history changed. Refresh before retrying; no request was replayed.".into());
                }
                let thread = value
                    .get("thread")
                    .ok_or("Native history response omitted its thread")?;
                let id = native_id(thread)?;
                if (!fork && id != source)
                    || (*fork && id == source)
                    || self.owner_for_thread(id).is_some()
                {
                    return Err(
                        "Native history returned an unexpected or already linked thread".into(),
                    );
                }
                // Validate/hydrate before installing durable ownership.
                if !fork
                    && let Some(origin) = self.saved.fork_origins.get(&request.owner)
                    && thread.get("forkedFromId").and_then(Value::as_str) != Some(origin.as_str())
                {
                    return Err("Choose the native fork of the original source; an unrelated history cannot resolve this pending branch".into());
                }
                let hydrated = if *fork {
                    self.mirror
                        .hydrate_session_response(thread, &value, request.revision)
                } else {
                    self.mirror.hydrate(thread, request.revision)
                };
                if let Err(error) = hydrated {
                    self.mirror.forget(id);
                    return Err(error);
                }
                self.saved.bindings.insert(
                    request.owner.clone(),
                    Binding {
                        thread_id: id.into(),
                        session_id: thread
                            .get("sessionId")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        archived: *archived && !fork,
                        deleted: false,
                        never_submitted: false,
                    },
                );
                self.observed.insert(id.into());
                self.saved.fork_origins.remove(&request.owner);
                if *fork {
                    self.loaded.insert(id.into());
                }
                Ok(if *fork {
                    Outcome::Forked {
                        owner: request.owner.clone(),
                        thread_id: id.into(),
                    }
                } else {
                    Outcome::Updated {
                        owner: request.owner.clone(),
                    }
                })
            }
            Kind::Open | Kind::Read | Kind::Unarchive => {
                let thread = value
                    .get("thread")
                    .ok_or("Native response omitted the thread")?;
                let id = native_id(thread)?;
                if request
                    .thread_id
                    .as_deref()
                    .is_some_and(|expected| expected != id)
                {
                    return Err("Native response targeted a different thread".into());
                }
                if self
                    .owner_for_thread(id)
                    .is_some_and(|owner| owner != request.owner)
                {
                    return Err(
                        "Native thread is already owned by another local conversation".into(),
                    );
                }
                let archived = (matches!(request.kind, Kind::Read) || lifecycle_changed)
                    && self.binding(&request.owner).is_some_and(|b| b.archived);
                let never_submitted = self
                    .binding(&request.owner)
                    .map_or(request.thread_id.is_none(), |binding| {
                        binding.never_submitted
                    })
                    && thread["turns"].as_array().is_some_and(Vec::is_empty);
                if matches!(request.kind, Kind::Open) {
                    self.mirror
                        .hydrate_session_response(thread, &value, request.revision)?;
                } else {
                    self.mirror.hydrate(thread, request.revision)?;
                }
                self.saved.bindings.insert(
                    request.owner.clone(),
                    Binding {
                        thread_id: id.into(),
                        session_id: thread
                            .get("sessionId")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        archived,
                        deleted: false,
                        never_submitted,
                    },
                );
                self.observed.insert(id.into());
                if matches!(request.kind, Kind::Open) && !lifecycle_changed {
                    self.loaded.insert(id.into());
                }
                self.reconcile_receipt(&request.owner, thread);
                self.reconcile_review_refresh(&request.owner);
                if matches!(request.kind, Kind::Open) {
                    Ok(Outcome::Opened {
                        owner: request.owner.clone(),
                        thread_id: id.into(),
                    })
                } else {
                    Ok(Outcome::Updated {
                        owner: request.owner.clone(),
                    })
                }
            }
            Kind::Start { message_id } => {
                let turn = value.get("turn").ok_or(
                    "Native response omitted the accepted turn; reconcile before retrying",
                )?;
                let id = native_id(turn)?.to_owned();
                let thread_id = request.thread_id.as_deref().unwrap();
                self.mirror
                    .notify("turn/started", &json!({"threadId":thread_id,"turn":turn}))?;
                self.saved.unresolved.remove(&request.owner);
                Ok(Outcome::Accepted {
                    owner: request.owner.clone(),
                    message_id: message_id.clone(),
                    turn_id: id,
                })
            }
            Kind::Steer {
                message_id,
                expected_turn,
            } => {
                if value.get("turnId").and_then(Value::as_str) != Some(expected_turn) {
                    return Err(
                        "The steering acknowledgement did not match the expected turn".into(),
                    );
                }
                self.saved.unresolved.remove(&request.owner);
                Ok(Outcome::Accepted {
                    owner: request.owner.clone(),
                    message_id: message_id.clone(),
                    turn_id: expected_turn.clone(),
                })
            }
            Kind::Fork { destination } => {
                let thread = value
                    .get("thread")
                    .ok_or("Native fork response omitted its thread")?;
                let id = native_id(thread)?;
                if self.owner_for_thread(id).is_some() || self.binding(destination).is_some() {
                    return Err("The fork is not an independent conversation".into());
                }
                if let Err(error) =
                    self.mirror
                        .hydrate_session_response(thread, &value, request.revision)
                {
                    self.mirror.forget(id);
                    return Err(error);
                }
                self.saved.bindings.insert(
                    destination.clone(),
                    Binding {
                        thread_id: id.into(),
                        session_id: thread
                            .get("sessionId")
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        archived: false,
                        deleted: false,
                        never_submitted: false,
                    },
                );
                self.loaded.insert(id.into());
                self.observed.insert(id.into());
                self.saved.fork_origins.remove(destination);
                Ok(Outcome::Forked {
                    owner: destination.clone(),
                    thread_id: id.into(),
                })
            }
            Kind::Archive => {
                if !lifecycle_changed {
                    self.notification("thread/archived", &json!({"threadId":request.thread_id}))?;
                }
                Ok(Outcome::Updated {
                    owner: request.owner.clone(),
                })
            }
            Kind::Delete => {
                self.notification("thread/deleted", &json!({"threadId":request.thread_id}))?;
                Ok(Outcome::Deleted {
                    owner: request.owner.clone(),
                })
            }
            Kind::Rename => {
                if let Some(name) = request.call.params.get("name").and_then(Value::as_str) {
                    self.mirror.rename(
                        request.thread_id.as_deref().unwrap(),
                        Some(name),
                        request.revision,
                    );
                }
                Ok(Outcome::Updated {
                    owner: request.owner.clone(),
                })
            }
            Kind::Compact => {
                if value != json!({}) {
                    self.compactions.remove(&request.owner);
                    return Err("Codex returned an unexpected compaction acknowledgement. Inspect native history before retrying.".into());
                }
                if let Some(pending) = self.compactions.get_mut(&request.owner) {
                    pending.accepted = true;
                }
                Ok(Outcome::Updated {
                    owner: request.owner.clone(),
                })
            }
            Kind::Interrupt => Ok(Outcome::Updated {
                owner: request.owner.clone(),
            }),
        }
    }

    pub fn notification(&mut self, method: &str, params: &Value) -> Result<bool, String> {
        if method == "thread/started"
            && let Some(thread) = params.get("thread")
        {
            let id = native_id(thread)?;
            let origin = thread.get("forkedFromId").and_then(Value::as_str);
            let pending_detached = self.pending.values().any(|request| {
                matches!(request.kind, Kind::DetachedReview { .. })
                    && request.thread_id.as_deref() == origin
            });
            if pending_detached
                && id != origin.unwrap_or_default()
                && self.owner_for_thread(id).is_none()
                && self.detached_threads.len() < 32
            {
                self.detached_threads.insert(id.into(), thread.clone());
                return Ok(false);
            }
            if let Some(owner) = self.owner_for_thread(id).map(str::to_owned) {
                let revision = self.mirror.revision();
                self.mirror.hydrate(thread, revision)?;
                if let Some(session_id) = thread["sessionId"].as_str() {
                    self.saved.bindings.get_mut(&owner).unwrap().session_id =
                        Some(session_id.into());
                }
                self.loaded.insert(id.into());
                self.observed.insert(id.into());
                return Ok(true);
            }
            return Ok(false);
        }
        let Some(id) = params.get("threadId").and_then(Value::as_str) else {
            return Ok(false);
        };
        let Some(owner) = self.owner_for_thread(id).map(str::to_owned) else {
            if matches!(
                method,
                "thread/archived" | "thread/unarchived" | "thread/deleted" | "thread/closed"
            ) && self
                .pending
                .values()
                .any(|r| matches!(&r.kind,Kind::Import {source,..} if source==id))
            {
                let revision = self.lifecycle_revisions.entry(id.into()).or_default();
                *revision = revision.saturating_add(1);
            }
            return Ok(false);
        };
        if self.binding(&owner).is_some_and(|b| b.deleted) {
            return Ok(false);
        }
        if matches!(
            method,
            "turn/started"
                | "turn/completed"
                | "thread/status/changed"
                | "thread/closed"
                | "thread/archived"
                | "thread/deleted"
        ) {
            self.initializing_delegates.remove(id);
        }
        if method == "thread/reverted" {
            self.invalidate_managed_history(&owner, id);
            return Ok(true);
        }
        // A late settings event cannot revalidate a closed/archived session.
        // It is observed state only, never a new native binding or consent.
        if method == "thread/settings/updated" && !self.is_thread_loaded(&owner) {
            return Ok(false);
        }
        if method == "turn/started"
            || method == "thread/status/changed" && params["status"]["type"] == "active"
        {
            self.saved.bindings.get_mut(&owner).unwrap().never_submitted = false;
        }
        if method == "thread/status/changed" && params["status"]["type"] == "notLoaded" {
            self.compactions.remove(&owner);
            self.review_refreshes.remove(&owner);
            let revision = self.lifecycle_revisions.entry(id.into()).or_default();
            *revision = revision.saturating_add(1);
            self.loaded.remove(id);
            return self.mirror.notify(method, params);
        }
        if matches!(
            method,
            "thread/archived" | "thread/unarchived" | "thread/deleted" | "thread/closed"
        ) {
            let revision = self.lifecycle_revisions.entry(id.into()).or_default();
            *revision = revision.saturating_add(1);
            let binding = self.saved.bindings.get_mut(&owner).unwrap();
            match method {
                "thread/archived" => binding.archived = true,
                "thread/unarchived" => binding.archived = false,
                "thread/deleted" => {
                    binding.deleted = true;
                    self.saved.unresolved.remove(&owner);
                    self.saved.reverts.remove(&owner);
                    self.management.forget(&owner);
                    self.observed.remove(id);
                    self.mirror.forget(id);
                }
                _ => {}
            }
            if method != "thread/unarchived" {
                self.compactions.remove(&owner);
                self.review_refreshes.remove(&owner);
                self.loaded.remove(id);
                self.mirror.unload(id);
            }
            return Ok(true);
        }
        let changed = self.mirror.notify(method, params)?;
        self.observe_compaction(&owner, method, params);
        self.observe_review_refresh(&owner, method, params);
        Ok(changed)
    }

    fn lifecycle_revision(&self, owner: &str) -> u64 {
        self.binding(owner)
            .and_then(|b| self.lifecycle_revisions.get(&b.thread_id))
            .copied()
            .unwrap_or(0)
    }

    pub fn observed(&self, owner: &str) -> bool {
        self.binding(owner)
            .is_some_and(|b| !b.deleted && self.observed.contains(&b.thread_id))
    }

    pub fn history_mode(&self, owner: &str) -> Option<&str> {
        let binding = self.binding(owner)?;
        self.mirror
            .thread(&binding.thread_id)?
            .history_mode
            .as_deref()
    }

    /// Atomically merge a complete, bounded sequence of paginated turn
    /// summaries into the display-only mirror. Native history remains owned by
    /// Codex and is never reconstructed or sent back to the server.
    pub fn hydrate_paginated_history(
        &mut self,
        owner: &str,
        thread_id: &str,
        turns: &[Value],
        requested_at: u64,
    ) -> Result<(), String> {
        let binding = self
            .binding(owner)
            .ok_or("The native conversation was unlinked while history was loading")?;
        if binding.deleted || binding.thread_id != thread_id {
            return Err("The native conversation changed while history was loading".into());
        }
        if self.history_mode(owner) != Some("paginated") {
            return Err("Codex changed the conversation history mode while it was loading".into());
        }
        self.mirror.hydrate_turns(thread_id, turns, requested_at)?;
        // A metadata-only thread/read cannot correlate an uncertain delivery.
        // Reconcile only after the complete bounded page sequence has supplied
        // the native user-message client ID; absence still proves nothing.
        self.reconcile_receipt(owner, &json!({"turns":turns}));
        self.observe_managed_history(owner, turns)?;
        self.reconcile_review_refresh(owner);
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.mirror.disconnect();
        self.generation += 1;
        self.pending.clear();
        self.loaded.clear();
        self.observed.clear();
        self.compactions.clear();
        self.review_refreshes.clear();
        self.detached_threads.clear();
        self.initializing_delegates.clear();
        self.unsubscribing.clear();
        self.management.disconnect();
        // Keep native IDs and uncertain message receipts. Never replay anything.
    }

    /// Explicit host-UI acknowledgement after inspecting native history. There
    /// is deliberately no retry here: this only clears the exact warning the
    /// user reviewed. A newer receipt or still-running RPC cannot be dismissed.
    pub fn resolve_delivery_after_user_review(
        &mut self,
        owner: &str,
        message_id: &str,
    ) -> Result<(), String> {
        if !self.observed(owner) {
            return Err("Read this conversation's native history on the current connection before dismissing its delivery warning".into());
        }
        if self.pending.values().any(|request| {
            request.owner == owner
                && matches!(
                    request.kind,
                    Kind::Start { .. } | Kind::Steer { .. } | Kind::Review | Kind::Compact
                )
        }) {
            return Err("The submission is still awaiting its native response".into());
        }
        if self.compaction_pending(owner)
            || self
                .saved
                .unresolved
                .get(owner)
                .is_some_and(|receipt| receipt.compact)
                && (!self.observed(owner)
                    || self
                        .binding(owner)
                        .and_then(|binding| self.mirror.thread(&binding.thread_id))
                        .is_some_and(|thread| {
                            thread.active_turn().is_some()
                                || thread
                                    .status
                                    .as_ref()
                                    .is_some_and(|status| status["type"] == "active")
                        }))
        {
            return Err(
                "Wait for the native operation to finish before dismissing its delivery warning"
                    .into(),
            );
        }
        if self
            .saved
            .unresolved
            .get(owner)
            .is_none_or(|receipt| receipt.message_id != message_id)
        {
            return Err(
                "The delivery warning has changed; inspect the current conversation again".into(),
            );
        }
        self.saved.unresolved.remove(owner);
        Ok(())
    }

    fn reconcile_receipt(&mut self, owner: &str, thread: &Value) {
        let Some(receipt) = self.saved.unresolved.get(owner) else {
            return;
        };
        if receipt.steer || receipt.review || receipt.compact {
            return;
        }
        let found = thread
            .get("turns")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .flat_map(|turn| {
                turn.get("items")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
            })
            .any(|item| {
                item.get("type").and_then(Value::as_str) == Some("userMessage")
                    && item.get("clientId").and_then(Value::as_str) == Some(&receipt.message_id)
            });
        if found {
            self.saved.unresolved.remove(owner);
        }
    }
}

fn native_id(value: &Value) -> Result<&str, String> {
    value
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| "Native response has no valid ID".into())
}
fn lineage_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 1024 && !id.chars().any(char::is_control)
}
// Native Windows responses can omit the verbatim prefix on our canonical cwd.
// Compare existing directory identity, never lowercase/strip arbitrary paths or
// accept an unresolved path. Native sandbox and approval fields are checked
// independently; an equivalent spelling must not broaden review permissions.
fn same_existing_directory(actual: &Path, expected: &Path) -> bool {
    actual.is_absolute()
        && expected.is_absolute()
        && actual.is_dir()
        && expected.is_dir()
        && std::fs::canonicalize(actual)
            .ok()
            .zip(std::fs::canonicalize(expected).ok())
            .is_some_and(|(actual, expected)| actual == expected)
}

fn validate_cwd(cwd: &Path) -> Result<(), String> {
    if !cwd.is_absolute() || !cwd.is_dir() {
        return Err("Select an existing absolute local working directory; remote paths are not local cwd values".into());
    }
    if cwd.to_str().is_none() {
        return Err(
            "The working directory cannot be represented in the App Server protocol".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmed_fork_ancestry_is_durable_and_separate_from_recovery_receipts() {
        let root = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:source", "native-source", root.path());
        let fork = book
            .action(
                "chat:source",
                Action::Fork {
                    destination: "chat:child".into(),
                },
            )
            .unwrap();
        assert!(book.saved().lineage.is_empty());
        book.complete(&fork, Ok(json!({"thread":thread("native-child")})))
            .unwrap();
        assert_eq!(book.saved().lineage["native-child"], "native-source");
        assert!(book.saved().fork_origins.is_empty());
        let stored = serde_json::to_string(book.saved()).unwrap();
        let mut restored = Conversations::restore(serde_json::from_str(&stored).unwrap()).unwrap();
        assert_eq!(restored.saved().lineage["native-child"], "native-source");
        let read = restored.action("chat:child", Action::Read).unwrap();
        restored
            .complete(&read, Ok(json!({"thread":thread("native-child")})))
            .unwrap();
        assert_eq!(
            restored.saved().lineage["native-child"],
            "native-source",
            "Omitted optional metadata must not erase known ancestry"
        );
        assert!(restored.saved().unresolved.is_empty());
    }

    #[test]
    fn older_bound_histories_backfill_ancestry_only_after_a_valid_explicit_read() {
        let root = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:old", "old-fork", root.path());
        let read = book.action("chat:old", Action::Read).unwrap();
        let mut native = thread("foreign-thread");
        native["forkedFromId"] = json!("source");
        assert!(book.complete(&read, Ok(json!({"thread":native}))).is_err());
        assert!(book.saved().lineage.is_empty());
        let read = book.action("chat:old", Action::Read).unwrap();
        native["id"] = json!("old-fork");
        book.complete(&read, Ok(json!({"thread":native}))).unwrap();
        assert_eq!(book.saved().lineage["old-fork"], "source");
        assert!(
            book.complete(&read, Ok(json!({"thread":thread("old-fork")})))
                .is_err()
        );
        assert_eq!(book.saved().lineage["old-fork"], "source");
    }

    #[test]
    fn ancestry_accepts_old_store_and_rejects_invalid_ids_without_changing_authority() {
        let old: Saved =
            serde_json::from_value(json!({"version":1,"bindings":{},"unresolved":{}})).unwrap();
        assert!(old.lineage.is_empty());
        assert!(old.validate().is_ok());
        for (child, parent) in [("same", "same"), ("child", ""), ("child", "line\nbreak")] {
            let mut saved = old.clone();
            saved.lineage.insert(child.into(), parent.into());
            assert!(saved.validate().is_err());
        }
    }

    #[test]
    fn open_response_reports_stable_session_settings_without_waiting_for_a_notification() {
        let root = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        let request = book
            .open("chat:a", root.path(), &Profile::default(), Access::ReadOnly)
            .unwrap();
        let response = json!({
            "thread": thread("native-a"),
            "cwd": root.path(),
            "model": "fixture-model",
            "modelProvider": "openai",
            "serviceTier": null,
            "reasoningEffort": "high",
            "approvalPolicy": "on-request",
            "approvalsReviewer": "user",
            "sandbox": {"type":"readOnly","networkAccess":false}
        });
        book.complete(&request, Ok(response)).unwrap();
        let reported = book.mirror.thread("native-a").unwrap();
        assert!(reported.settings_current);
        assert_eq!(reported.settings.as_ref().unwrap().model, "fixture-model");
        assert_eq!(
            reported.settings.as_ref().unwrap().effort.as_deref(),
            Some("high")
        );
    }

    #[test]
    fn reported_settings_are_owner_scoped_display_only_and_expire_with_the_session() {
        let root = tempfile::tempdir().unwrap();
        for owner in ["chat:a", "graph:a"] {
            let mut book = Conversations::default();
            opened(&mut book, owner, "native-a", root.path());
            opened(&mut book, "graph:b", "native-b", root.path());
            let saved = serde_json::to_value(book.saved()).unwrap();
            let event: Value =
                serde_json::from_str(include_str!("../tests/thread-settings.json")).unwrap();
            assert!(
                book.notification("thread/settings/updated", &event)
                    .unwrap()
            );
            let thread = book.mirror.thread("native-a").unwrap();
            assert!(thread.settings_current);
            assert!(thread.turns.is_empty());
            assert_eq!(thread.settings.as_ref().unwrap().model, "fixture-model");
            assert!(book.mirror.thread("native-b").unwrap().settings.is_none());
            assert_eq!(serde_json::to_value(book.saved()).unwrap(), saved);
            let revision = book.mirror.revision();
            let mut unknown = event.clone();
            unknown["threadId"] = json!("foreign");
            assert!(
                !book
                    .notification("thread/settings/updated", &unknown)
                    .unwrap()
            );
            assert_eq!(book.mirror.revision(), revision);
            // History hydration is not a source of loaded settings.
            book.mirror.hydrate(&self::thread("native-a"), 0).unwrap();
            assert!(book.mirror.thread("native-a").unwrap().settings_current);
            book.notification(
                "thread/status/changed",
                &json!({"threadId":"native-a","status":{"type":"notLoaded"}}),
            )
            .unwrap();
            assert!(!book.mirror.thread("native-a").unwrap().settings_current);
            assert!(
                !book
                    .notification("thread/settings/updated", &event)
                    .unwrap()
            );
            book.disconnect();
            assert!(
                !book
                    .notification("thread/settings/updated", &event)
                    .unwrap()
            );
            assert!(book.mirror.thread("native-a").unwrap().settings.is_some());
        }
    }

    #[test]
    fn malformed_thread_settings_invalidates_freshness_without_replacing_prior_report() {
        let root = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native-a", root.path());
        let mut event: Value =
            serde_json::from_str(include_str!("../tests/thread-settings.json")).unwrap();
        book.notification("thread/settings/updated", &event)
            .unwrap();
        event["threadSettings"]["model"] = Value::Null;
        assert!(
            book.notification("thread/settings/updated", &event)
                .is_err()
        );
        let thread = book.mirror.thread("native-a").unwrap();
        assert!(!thread.settings_current);
        assert_eq!(thread.settings.as_ref().unwrap().model, "fixture-model");
        assert!(thread.turns.is_empty());
    }

    #[test]
    fn idle_loaded_session_submits_directly_with_the_new_frozen_profile() {
        let initial = tempfile::tempdir().unwrap();
        let next = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native-a", initial.path());
        opened(&mut book, "graph:b", "native-b", initial.path());
        assert!(book.ready_for_turn("chat:a"));
        assert!(!book.unused_link("chat:a"));
        assert!(book.reset_unused_link("chat:a").is_err());
        assert!(book.pending.is_empty());
        let profile = Profile {
            model: Some("fixture-model".into()),
            effort: Some("high".into()),
            service_tier: Some("fast".into()),
            personality: None,
            summary: None,
        };
        let request = book
            .start_turn(
                "chat:a",
                "first-input",
                vec![api::text_input("Actual input")],
                next.path(),
                &profile,
                Access::WorkspaceWrite,
            )
            .unwrap();
        assert_eq!(request.call.method, "turn/start");
        assert_eq!(request.call.params["threadId"], "native-a");
        assert_eq!(request.call.params["cwd"], next.path().to_str().unwrap());
        assert_eq!(request.call.params["model"], "fixture-model");
        assert_eq!(request.call.params["effort"], "high");
        assert_eq!(request.call.params["serviceTier"], "fast");
        assert_eq!(
            request.call.params["sandboxPolicy"]["type"],
            "workspaceWrite"
        );
        assert_eq!(request.call.params["approvalPolicy"], "on-request");
        assert_eq!(
            request.call.params["input"],
            json!([api::text_input("Actual input")])
        );
        assert_eq!(book.pending.len(), 1);
        assert!(!book.ready_for_turn("chat:a"));
        assert!(book.ready_for_turn("graph:b"));
        assert!(!book.binding("chat:a").unwrap().never_submitted);
        book.complete(
            &request,
            Ok(json!({"turn":{"id":"turn-a","items":[],"status":"inProgress"}})),
        )
        .unwrap();
        assert!(!book.ready_for_turn("chat:a"));
        book.notification(
            "turn/completed",
            &json!({"threadId":"native-a","turn":{"id":"turn-a","items":[],"status":"completed"}}),
        )
        .unwrap();
        book.notification(
            "thread/status/changed",
            &json!({"threadId":"native-a","status":{"type":"idle"}}),
        )
        .unwrap();
        assert!(book.ready_for_turn("chat:a"));
    }

    #[test]
    fn typed_turn_options_reach_the_exact_owned_turn_start() {
        let root = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(
            &mut book,
            "graph:supervisor",
            "native-supervisor",
            root.path(),
        );
        let profile = Profile {
            model: Some("fixture-model".into()),
            effort: Some("low".into()),
            service_tier: None,
            personality: None,
            summary: None,
        };
        let schema = json!({"type":"object","properties":{"decision":{"type":"string"}},"required":["decision"],"additionalProperties":false});
        let options = api::TurnOptions {
            output_schema: Some(schema.clone()),
            ..api::TurnOptions::default()
        };
        let request = book
            .start_turn_with_options(TurnStart {
                owner: "graph:supervisor",
                message_id: "review-input",
                input: vec![api::text_input("Review")],
                cwd: root.path(),
                profile: &profile,
                access: Access::ReadOnly,
                options: &options,
            })
            .unwrap();
        assert_eq!(request.call.method, "turn/start");
        assert_eq!(request.call.params["threadId"], "native-supervisor");
        assert_eq!(request.call.params["outputSchema"], schema);
        assert_eq!(request.call.params["sandboxPolicy"]["type"], "readOnly");
    }

    #[test]
    fn unloaded_notification_prevents_reuse_and_late_resume_cannot_reload_it() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native-a", temp.path());
        assert!(book.is_thread_loaded("chat:a"));
        assert!(!book.is_thread_loaded("graph:foreign"));
        let pending = book
            .open("chat:a", temp.path(), &Profile::default(), Access::ReadOnly)
            .unwrap();
        assert!(!book.ready_for_turn("chat:a"));
        book.notification(
            "thread/status/changed",
            &json!({"threadId":"native-a","status":{"type":"notLoaded"}}),
        )
        .unwrap();
        assert!(!book.ready_for_turn("chat:a"));
        book.complete(&pending, Ok(json!({"thread":thread("native-a")})))
            .unwrap();
        assert!(!book.is_thread_loaded("chat:a"));
        assert!(!book.ready_for_turn("chat:a"));
        assert!(
            book.start_turn(
                "chat:a",
                "not-sent",
                vec![api::text_input("User input")],
                temp.path(),
                &Profile::default(),
                Access::ReadOnly
            )
            .is_err()
        );
        let resume = book
            .open("chat:a", temp.path(), &Profile::default(), Access::ReadOnly)
            .unwrap();
        assert_eq!(resume.call.method, "thread/resume");
        book.complete(&resume, Ok(json!({"thread":thread("native-a")})))
            .unwrap();
        assert!(book.is_thread_loaded("chat:a"));
        assert!(book.ready_for_turn("chat:a"));
        book.disconnect();
        assert!(!book.is_thread_loaded("chat:a"));
        assert!(!book.ready_for_turn("chat:a"));
    }

    #[test]
    fn unused_reset_removes_only_the_explicit_local_binding_without_rpc() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "unused-native", temp.path());
        opened(&mut book, "graph:b", "other-native", temp.path());
        let other = book.binding("graph:b").unwrap().clone();
        book.disconnect();
        let mut restored = Conversations::restore(
            serde_json::from_value(serde_json::to_value(book.saved()).unwrap()).unwrap(),
        )
        .unwrap();
        assert!(restored.unused_link("chat:a"));
        let read = restored.action("chat:a", Action::Read).unwrap();
        assert!(restored.reset_unused_link("chat:a").is_err());
        restored
            .complete(&read, Err(CallError::Rejected("No rollout".into())))
            .unwrap_err();
        restored.reset_unused_link("chat:a").unwrap();
        assert!(restored.binding("chat:a").is_none());
        assert_eq!(restored.binding("graph:b"), Some(&other));
        assert!(restored.pending.is_empty());
        assert!(restored.saved().unresolved.is_empty());
        assert!(restored.reset_unused_link("chat:a").is_err());
        // Only a subsequent, explicit user submission creates a new native thread.
        let next = restored
            .open("chat:a", temp.path(), &Profile::default(), Access::ReadOnly)
            .unwrap();
        assert_eq!(next.call.method, "thread/start");
        assert!(next.call.params.get("threadId").is_none());
    }

    #[test]
    fn submitted_or_observed_work_never_becomes_an_unused_link() {
        let temp = tempfile::tempdir().unwrap();
        for outcome in [
            "rejected",
            "uncertain",
            "review",
            "external_turn",
            "external_active",
            "history",
        ] {
            let mut book = Conversations::default();
            opened(&mut book, "chat:review", "native-review", temp.path());
            match outcome {
                "external_turn" => {
                    book.notification("turn/started", &json!({"threadId":"native-review","turn":{"id":"external","items":[],"status":"inProgress"}})).unwrap();
                }
                "external_active" => {
                    book.notification("thread/status/changed", &json!({"threadId":"native-review","status":{"type":"active","activeFlags":[]}})).unwrap();
                }
                "history" => {
                    let read = book.action("chat:review", Action::Read).unwrap();
                    let mut native = thread("native-review");
                    native["turns"] = json!([{"id":"prior","items":[],"status":"completed"}]);
                    book.complete(&read, Ok(json!({"thread":native}))).unwrap();
                }
                _ => {
                    let request = if outcome == "review" {
                        review_prepared(&mut book, temp.path());
                        book.start_review(
                            "chat:review",
                            &api::ReviewTarget::UncommittedChanges,
                            "review",
                        )
                        .unwrap()
                    } else {
                        start(&mut book, "chat:review", "input", temp.path())
                    };
                    let error = if outcome == "rejected" {
                        CallError::Rejected("rejected".into())
                    } else {
                        CallError::Disconnected {
                            reason: "lost response".into(),
                            delivery_unknown: true,
                        }
                    };
                    assert!(book.complete(&request, Err(error)).is_err());
                }
            }
            book.disconnect();
            assert!(
                !book.binding("chat:review").unwrap().never_submitted,
                "{outcome}"
            );
            assert!(!book.unused_link("chat:review"), "{outcome}");
            assert!(book.reset_unused_link("chat:review").is_err(), "{outcome}");
        }
    }

    #[test]
    fn legacy_imported_and_forked_history_is_not_an_unused_link() {
        let temp = tempfile::tempdir().unwrap();
        let binding: Binding =
            serde_json::from_value(json!({"threadId":"legacy","sessionId":null,"archived":false}))
                .unwrap();
        assert!(!binding.never_submitted);
        for fork in [false, true] {
            let mut book = Conversations::default();
            let import = book.import("chat:imported", "source", false, fork).unwrap();
            book.complete(
                &import,
                Ok(json!({"thread":thread(if fork {"fork"} else {"source"})})),
            )
            .unwrap();
            book.disconnect();
            assert!(!book.unused_link("chat:imported"));
            assert!(book.reset_unused_link("chat:imported").is_err());
        }
        for notification in ["thread/archived", "thread/deleted"] {
            let mut book = Conversations::default();
            opened(&mut book, "chat:a", "native-a", temp.path());
            book.notification(notification, &json!({"threadId":"native-a"}))
                .unwrap();
            book.disconnect();
            assert!(!book.unused_link("chat:a"));
            assert!(book.reset_unused_link("chat:a").is_err());
        }
    }

    fn review_prepared(book: &mut Conversations, cwd: &Path) {
        let request = book.prepare_review("chat:review", cwd).unwrap();
        assert_eq!(request.call.method, "thread/resume");
        assert_eq!(request.call.params["threadId"], "native-review");
        assert!(request.call.params.get("history").is_none());
        let result = json!({"thread":thread("native-review"),"cwd":cwd,"model":"fixture-model","modelProvider":"openai","serviceTier":null,"reasoningEffort":null,"sandbox":{"type":"readOnly","networkAccess":false},"approvalPolicy":"untrusted","approvalsReviewer":"user"});
        assert_eq!(
            book.complete(&request, Ok(result)).unwrap(),
            Outcome::ReviewReady {
                owner: "chat:review".into()
            }
        );
    }

    #[test]
    fn native_review_refresh_closes_worker_projection_only_from_native_history() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:review", "native-review", temp.path());
        opened(&mut book, "graph:other", "unrelated", temp.path());
        for turn in ["review", "worker"] {
            book.notification("turn/started", &json!({"threadId":"native-review","turn":{"id":turn,"status":"inProgress","items":[]}})).unwrap();
        }
        let final_item = json!({"threadId":"native-review","turnId":"review","item":{"type":"exitedReviewMode","id":"final","review":"Result"}});
        book.notification("item/completed", &final_item).unwrap();
        let request = book.take_review_refresh("chat:review").unwrap().unwrap();
        assert_eq!(request.call.method, "thread/read");
        assert_eq!(request.call.params["threadId"], "native-review");
        assert!(book.take_review_refresh("chat:review").unwrap().is_none());
        assert!(book.take_review_refresh("graph:other").unwrap().is_none());
        book.notification(
            "thread/status/changed",
            &json!({"threadId":"native-review","status":{"type":"idle"}}),
        )
        .unwrap();
        // A first read can race persistence: neither idle nor the final item
        // alone is allowed to invent successful completion.
        let mut early = thread("native-review");
        early["status"] = json!({"type":"idle"});
        early["turns"] =
            json!([{"id":"review","status":"inProgress","items":[final_item["item"]]}]);
        book.complete(&request, Ok(json!({"thread":early})))
            .unwrap();
        assert!(book.busy("chat:review"));
        let follow_up = book.take_review_refresh("chat:review").unwrap().unwrap();
        let mut complete = thread("native-review");
        complete["status"] = json!({"type":"idle"});
        complete["turns"] = json!([
            {"id":"worker","status":"interrupted","items":[]},
            {"id":"review","status":"completed","items":[final_item["item"]]}
        ]);
        book.complete(&follow_up, Ok(json!({"thread":complete})))
            .unwrap();
        assert!(!book.busy("chat:review"));
        assert!(book.review_refreshes.is_empty());
        assert!(book.take_review_refresh("chat:review").unwrap().is_none());
        assert!(!book.busy("graph:other"));
    }

    #[test]
    fn paginated_review_clears_refresh_only_after_terminal_native_pages() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:review", "native-review", temp.path());
        for turn in ["review", "worker"] {
            book.notification("turn/started", &json!({"threadId":"native-review","turn":{"id":turn,"status":"inProgress","items":[]}})).unwrap();
        }
        book.notification("item/completed", &json!({"threadId":"native-review","turnId":"review","item":{"type":"exitedReviewMode","id":"final","review":"Result"}})).unwrap();
        let read = book.take_review_refresh("chat:review").unwrap().unwrap();
        let mut metadata = thread("native-review");
        metadata["historyMode"] = json!("paginated");
        metadata["status"] = json!({"type":"idle"});
        metadata["turns"] = json!([]);
        book.complete(&read, Ok(json!({"thread":metadata})))
            .unwrap();
        let mut turns = vec![
            json!({"id":"worker","status":"inProgress","items":[]}),
            json!({"id":"review","status":"completed","items":[]}),
        ];
        book.hydrate_paginated_history(
            "chat:review",
            "native-review",
            &turns,
            book.mirror.revision(),
        )
        .unwrap();
        assert!(book.busy("chat:review"));
        assert!(!book.review_refreshes.is_empty());
        turns[0]["status"] = json!("interrupted");
        book.hydrate_paginated_history(
            "chat:review",
            "native-review",
            &turns,
            book.mirror.revision(),
        )
        .unwrap();
        assert!(!book.busy("chat:review"));
        assert!(book.review_refreshes.is_empty());
        assert!(book.take_review_refresh("chat:review").unwrap().is_none());
    }

    #[test]
    fn native_review_refresh_failure_does_not_loop_or_survive_disconnect() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:review", "native-review", temp.path());
        let event = json!({"threadId":"native-review","turnId":"review","item":{"type":"exitedReviewMode","id":"final","review":"Result"}});
        let mut foreign = event.clone();
        foreign["threadId"] = json!("foreign");
        assert!(!book.notification("item/completed", &foreign).unwrap());
        assert!(book.review_refreshes.is_empty());
        book.notification("item/completed", &event).unwrap();
        let request = book.take_review_refresh("chat:review").unwrap().unwrap();
        assert!(
            book.complete(
                &request,
                Err(CallError::Rejected("fixture read failed".into()))
            )
            .is_err()
        );
        assert!(book.take_review_refresh("chat:review").unwrap().is_none());
        book.disconnect();
        assert!(book.review_refreshes.is_empty());
        assert!(book.take_review_refresh("chat:review").unwrap().is_none());
    }

    #[test]
    fn native_review_refresh_never_clears_an_unknown_submission_receipt() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:review", "native-review", temp.path());
        review_prepared(&mut book, temp.path());
        let review = book
            .start_review(
                "chat:review",
                &api::ReviewTarget::UncommittedChanges,
                "receipt",
            )
            .unwrap();
        let item = json!({"type":"exitedReviewMode","id":"final","review":"Result"});
        book.notification(
            "item/completed",
            &json!({"threadId":"native-review","turnId":"review","item":item}),
        )
        .unwrap();
        let read = book.take_review_refresh("chat:review").unwrap().unwrap();
        let mut snapshot = thread("native-review");
        snapshot["status"] = json!({"type":"idle"});
        snapshot["turns"] = json!([{"id":"review","status":"completed","items":[item]}]);
        book.complete(&read, Ok(json!({"thread":snapshot})))
            .unwrap();
        assert!(book.saved().unresolved["chat:review"].review);
        assert!(book.busy("chat:review"));
        assert!(book.review_refreshes.is_empty());
        assert!(
            book.complete(
                &review,
                Err(CallError::Disconnected {
                    reason: "lost acknowledgement".into(),
                    delivery_unknown: true
                })
            )
            .is_err()
        );
        assert!(book.saved().unresolved["chat:review"].review);
        assert!(book.take_review_refresh("chat:review").unwrap().is_none());
    }

    #[test]
    fn native_model_notices_are_owner_bound_and_not_saved_as_history() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:notice", "notice-thread", temp.path());
        opened(&mut book, "graph:other", "unrelated", temp.path());
        let samples: Vec<Value> =
            serde_json::from_str(include_str!("../tests/native-model-notices.json")).unwrap();
        for sample in &samples[..3] {
            assert!(
                book.notification(sample["method"].as_str().unwrap(), &sample["params"])
                    .unwrap()
            );
        }
        assert_eq!(
            book.mirror.thread("notice-thread").unwrap().notices.len(),
            3
        );
        assert!(book.mirror.thread("unrelated").unwrap().notices.is_empty());
        assert!(!book.busy("chat:notice"));
        assert!(!book.busy("graph:other"));
        assert!(
            !serde_json::to_string(book.saved())
                .unwrap()
                .contains("trustedAccessForCyber")
        );
        let mut unknown = samples[0]["params"].clone();
        unknown["threadId"] = json!("not-owned");
        assert!(!book.notification("model/rerouted", &unknown).unwrap());
        assert!(book.mirror.thread("not-owned").is_none());
        book.notification("thread/deleted", &json!({"threadId":"notice-thread"}))
            .unwrap();
        assert!(
            !book
                .notification("model/rerouted", &samples[0]["params"])
                .unwrap()
        );
        assert!(book.mirror.thread("notice-thread").is_none());
    }

    #[test]
    fn native_review_preserves_owner_and_a_completed_turn_before_ack() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:review", "native-review", temp.path());
        opened(&mut book, "graph:other", "unrelated", temp.path());
        review_prepared(&mut book, temp.path());
        let request = book
            .start_review(
                "chat:review",
                &api::ReviewTarget::UncommittedChanges,
                "review-receipt",
            )
            .unwrap();
        assert_eq!(request.call.method, "review/start");
        assert_eq!(
            request.call.params,
            json!({"threadId":"native-review","delivery":"inline","target":{"type":"uncommittedChanges"}})
        );
        assert!(book.saved().unresolved["chat:review"].review);
        assert!(!book.busy("graph:other"));
        assert!(
            book.resolve_delivery_after_user_review("chat:review", "review-receipt")
                .is_err()
        );
        let samples: Vec<Value> =
            serde_json::from_str(include_str!("../tests/native-review.json")).unwrap();
        book.notification(
            "turn/started",
            &json!({"threadId":"native-review","turn":samples[0]["params"]["turn"]}),
        )
        .unwrap();
        for sample in &samples[1..] {
            book.notification(sample["method"].as_str().unwrap(), &sample["params"])
                .unwrap();
        }
        book.complete(&request, Ok(samples[0]["params"].clone()))
            .unwrap();
        assert!(!book.busy("chat:review"));
        assert!(book.saved().unresolved.is_empty());
        assert_eq!(
            book.mirror.thread("native-review").unwrap().turns[0].status,
            "completed"
        );
        assert!(book.mirror.thread("unrelated").unwrap().turns.is_empty());
    }

    #[test]
    fn detached_review_binds_only_the_returned_independent_thread() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:review", "native-review", temp.path());
        opened(&mut book, "graph:other", "unrelated", temp.path());
        review_prepared(&mut book, temp.path());
        let request = book
            .start_detached_review(
                "chat:review",
                "chat:separate",
                &api::ReviewTarget::UncommittedChanges,
                "detached-receipt",
            )
            .unwrap();
        assert_eq!(
            request.call.params,
            json!({"threadId":"native-review","delivery":"detached","target":{"type":"uncommittedChanges"}})
        );
        assert!(book.busy("chat:review"));
        assert!(book.busy("chat:separate"));
        assert_eq!(book.saved().fork_origins["chat:separate"], "native-review");
        let mut started = thread("native-detached");
        started["forkedFromId"] = json!("native-review");
        assert!(
            !book
                .notification("thread/started", &json!({"thread":started}))
                .unwrap()
        );
        let outcome = book
            .complete(
                &request,
                Ok(json!({
                    "reviewThreadId":"native-detached",
                    "turn":{"id":"review-turn","items":[],"itemsView":"full","status":"inProgress","error":null}
                })),
            )
            .unwrap();
        assert_eq!(
            outcome,
            Outcome::DetachedReview {
                source_owner: "chat:review".into(),
                owner: "chat:separate".into(),
                thread_id: "native-detached".into(),
            }
        );
        assert_eq!(
            book.binding("chat:separate").unwrap().thread_id,
            "native-detached"
        );
        assert_eq!(book.owner_for_thread("native-review"), Some("chat:review"));
        assert_eq!(
            book.owner_for_thread("native-detached"),
            Some("chat:separate")
        );
        assert!(book.saved().fork_origins.is_empty());
        assert!(book.saved().unresolved.is_empty());
        assert!(
            book.mirror
                .thread("native-review")
                .unwrap()
                .turns
                .is_empty()
        );
        assert_eq!(
            book.mirror.thread("native-detached").unwrap().turns[0].id,
            "review-turn"
        );
        assert!(book.mirror.thread("unrelated").unwrap().turns.is_empty());
    }

    #[test]
    fn detached_review_accepts_response_before_thread_started_notification() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:review", "native-review", temp.path());
        review_prepared(&mut book, temp.path());
        let request = book
            .start_detached_review(
                "chat:review",
                "chat:separate",
                &api::ReviewTarget::UncommittedChanges,
                "detached-receipt",
            )
            .unwrap();
        book.complete(
            &request,
            Ok(json!({
                "reviewThreadId":"native-detached",
                "turn":{"id":"review-turn","items":[],"itemsView":"full","status":"inProgress","error":null}
            })),
        )
        .unwrap();
        assert_eq!(
            book.mirror.thread("native-detached").unwrap().turns[0].id,
            "review-turn"
        );

        let mut started = thread("native-detached");
        started["forkedFromId"] = json!("native-review");
        started["sessionId"] = json!("late-session");
        assert!(
            book.notification("thread/started", &json!({"thread":started}))
                .unwrap()
        );
        assert_eq!(
            book.mirror.thread("native-detached").unwrap().turns[0].id,
            "review-turn"
        );
        assert_eq!(
            book.binding("chat:separate").unwrap().session_id.as_deref(),
            Some("late-session")
        );
    }

    #[test]
    fn uncertain_detached_review_is_not_replayed_and_recovers_by_fork_origin() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:review", "native-review", temp.path());
        review_prepared(&mut book, temp.path());
        let request = book
            .start_detached_review(
                "chat:review",
                "chat:separate",
                &api::ReviewTarget::UncommittedChanges,
                "detached-receipt",
            )
            .unwrap();
        assert!(
            book.complete(
                &request,
                Err(CallError::Disconnected {
                    reason: "lost acknowledgement".into(),
                    delivery_unknown: true,
                }),
            )
            .is_err()
        );
        assert!(book.saved().unresolved["chat:review"].review);
        assert_eq!(book.saved().fork_origins["chat:separate"], "native-review");
        let saved = serde_json::to_string(book.saved()).unwrap();
        let mut restored = Conversations::restore(serde_json::from_str(&saved).unwrap()).unwrap();
        assert!(
            restored
                .open(
                    "chat:separate",
                    temp.path(),
                    &Profile::default(),
                    Access::ReadOnly,
                )
                .is_err()
        );
        let recovery = restored
            .import("chat:separate", "native-detached", false, false)
            .unwrap();
        let mut recovered = thread("native-detached");
        recovered["forkedFromId"] = json!("native-review");
        restored
            .complete(&recovery, Ok(json!({"thread":recovered})))
            .unwrap();
        assert_eq!(
            restored.binding("chat:separate").unwrap().thread_id,
            "native-detached"
        );
        assert!(restored.saved().fork_origins.is_empty());

        let read = restored.action("chat:review", Action::Read).unwrap();
        restored
            .complete(&read, Ok(json!({"thread":thread("native-review")})))
            .unwrap();
        restored
            .resolve_delivery_after_user_review("chat:review", "detached-receipt")
            .unwrap();
        assert!(restored.saved().unresolved.is_empty());
    }

    #[test]
    fn review_preparation_rejects_foreign_or_broader_native_scope() {
        let temp = tempfile::tempdir().unwrap();
        for field in [
            "cwd",
            "thread",
            "sandbox",
            "approvalPolicy",
            "approvalsReviewer",
        ] {
            let mut book = Conversations::default();
            opened(&mut book, "chat:review", "native-review", temp.path());
            let request = book.prepare_review("chat:review", temp.path()).unwrap();
            let mut result = json!({"thread":thread("native-review"),"cwd":temp.path(),"sandbox":{"type":"readOnly","networkAccess":false},"approvalPolicy":"untrusted","approvalsReviewer":"user"});
            result[field] = match field {
                "thread" => thread("foreign"),
                "sandbox" => json!({"type":"dangerFullAccess"}),
                _ => json!("other"),
            };
            assert!(book.complete(&request, Ok(result)).is_err());
            assert!(book.saved().unresolved.is_empty());
            assert!(book.mirror.thread("foreign").is_none());
        }
    }

    #[test]
    fn review_directory_identity_requires_existing_absolute_same_directory() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let canonical = std::fs::canonicalize(first.path()).unwrap();
        assert!(same_existing_directory(first.path(), &canonical));
        assert!(!same_existing_directory(second.path(), &canonical));
        assert!(!same_existing_directory(Path::new("."), &canonical));
        assert!(!same_existing_directory(
            &first.path().join("missing"),
            &canonical
        ));
        let file = first.path().join("file");
        std::fs::write(&file, "fixture").unwrap();
        assert!(!same_existing_directory(&file, &file));
    }

    #[cfg(windows)]
    #[test]
    fn review_preparation_accepts_native_windows_cwd_without_verbatim_prefix() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = std::fs::canonicalize(temp.path()).unwrap();
        let native = canonical.to_str().unwrap().strip_prefix(r"\\?\").unwrap();
        assert_ne!(Path::new(native), canonical.as_path());
        for network in [false, true] {
            let mut book = Conversations::default();
            opened(&mut book, "chat:review", "native-review", &canonical);
            let request = book.prepare_review("chat:review", &canonical).unwrap();
            let result = json!({"thread":thread("native-review"),"cwd":native,
                "model":"fixture-model","modelProvider":"openai","serviceTier":null,"reasoningEffort":null,
                "sandbox":{"type":"readOnly","networkAccess":network},
                "approvalPolicy":"untrusted","approvalsReviewer":"user"});
            assert_eq!(book.complete(&request, Ok(result)).is_ok(), !network);
        }
    }

    #[test]
    fn unknown_review_delivery_survives_restart_without_replay() {
        let temp = tempfile::tempdir().unwrap();
        for outcome in ["unknown", "wrong_thread", "rejected"] {
            let mut book = Conversations::default();
            opened(&mut book, "chat:review", "native-review", temp.path());
            review_prepared(&mut book, temp.path());
            let target = api::ReviewTarget::Custom {
                instructions: "Review fixture errors".into(),
            };
            let request = book
                .start_review("chat:review", &target, "receipt")
                .unwrap();
            let result = match outcome {
                "unknown" => Err(CallError::Disconnected {
                    reason: "closed".into(),
                    delivery_unknown: true,
                }),
                "wrong_thread" => Ok(
                    json!({"reviewThreadId":"foreign","turn":{"id":"review-turn","items":[],"status":"inProgress"}}),
                ),
                _ => Err(CallError::Rejected("rejected".into())),
            };
            assert!(book.complete(&request, result).is_err());
            assert_eq!(
                book.saved().unresolved.contains_key("chat:review"),
                outcome != "rejected"
            );
            let saved = serde_json::to_string(book.saved()).unwrap();
            assert!(!saved.contains("Review fixture errors"));
            let mut restored =
                Conversations::restore(serde_json::from_str(&saved).unwrap()).unwrap();
            if outcome != "rejected" {
                assert!(
                    restored
                        .open(
                            "chat:review",
                            temp.path(),
                            &Profile::default(),
                            Access::ReadOnly
                        )
                        .is_err()
                );
                let read = restored.action("chat:review", Action::Read).unwrap();
                restored
                    .complete(&read, Ok(json!({"thread":thread("native-review")})))
                    .unwrap();
                assert!(restored.saved().unresolved.contains_key("chat:review"));
                restored
                    .resolve_delivery_after_user_review("chat:review", "receipt")
                    .unwrap();
                assert!(restored.saved().unresolved.is_empty());
            }
        }
    }
    #[test]
    fn uncertain_fork_survives_restart_and_requires_matching_native_history() {
        let temp = tempfile::tempdir().unwrap();
        for from_bound in [false, true] {
            let mut book = Conversations::default();
            opened(&mut book, "chat:source", "source", temp.path());
            let request = if from_bound {
                book.action(
                    "chat:source",
                    Action::Fork {
                        destination: "graph:branch".into(),
                    },
                )
                .unwrap()
            } else {
                book.import("graph:branch", "source", false, true).unwrap()
            };
            assert!(book.busy("graph:branch"));
            assert!(book.import("graph:branch", "other", false, false).is_err());
            assert!(
                book.open(
                    "graph:branch",
                    temp.path(),
                    &Profile::default(),
                    Access::ReadOnly
                )
                .is_err()
            );
            let stored = serde_json::to_string(book.saved()).unwrap();
            let mut restored =
                Conversations::restore(serde_json::from_str(&stored).unwrap()).unwrap();
            assert!(!restored.any_busy());
            assert!(
                restored
                    .open(
                        "graph:branch",
                        temp.path(),
                        &Profile::default(),
                        Access::ReadOnly
                    )
                    .is_err()
            );
            assert!(
                restored
                    .import("graph:branch", "source", false, true)
                    .is_err()
            );
            assert!(
                restored
                    .complete(&request, Ok(json!({"thread":thread("late")})))
                    .is_err()
            );
            let wrong = restored
                .import("graph:branch", "unrelated", false, false)
                .unwrap();
            assert!(
                restored
                    .complete(&wrong, Ok(json!({"thread":thread("unrelated")})))
                    .is_err()
            );
            assert!(restored.binding("graph:branch").is_none());
            assert_eq!(restored.saved().fork_origins["graph:branch"], "source");
            let recovery = restored
                .import("graph:branch", "branch", true, false)
                .unwrap();
            assert_eq!(recovery.call.method, "thread/read");
            let mut native = thread("branch");
            native["forkedFromId"] = json!("source");
            native["sessionId"] = json!("native-session-root");
            restored
                .complete(&recovery, Ok(json!({"thread":native})))
                .unwrap();
            assert!(restored.saved().fork_origins.is_empty());
            let binding = restored.binding("graph:branch").unwrap();
            assert_eq!(binding.thread_id, "branch");
            assert_eq!(binding.session_id.as_deref(), Some("native-session-root"));
            assert!(binding.archived);
            restored.saved().validate().unwrap();
            assert_eq!(restored.binding("chat:source").unwrap().thread_id, "source");
        }
    }

    #[test]
    fn fork_receipts_clear_only_on_definitive_rejection_or_valid_success() {
        let temp = tempfile::tempdir().unwrap();
        for from_bound in [false, true] {
            for outcome in ["rejected", "not_sent", "unknown", "malformed", "success"] {
                let mut book = Conversations::default();
                opened(&mut book, "chat:source", "source", temp.path());
                let request = if from_bound {
                    book.action(
                        "chat:source",
                        Action::Fork {
                            destination: "chat:branch".into(),
                        },
                    )
                    .unwrap()
                } else {
                    book.import("chat:branch", "source", false, true).unwrap()
                };
                let reply = match outcome {
                    "rejected" => Err(CallError::Rejected("rejected".into())),
                    "not_sent" | "unknown" => Err(CallError::Disconnected {
                        reason: "closed".into(),
                        delivery_unknown: outcome == "unknown",
                    }),
                    "malformed" => Ok(json!({"thread":{"id":"branch","turns":[{}]}})),
                    _ => Ok(json!({"thread":thread("branch")})),
                };
                assert_eq!(book.complete(&request, reply).is_ok(), outcome == "success");
                assert_eq!(
                    book.saved().fork_origins.contains_key("chat:branch"),
                    matches!(outcome, "unknown" | "malformed")
                );
                assert_eq!(book.binding("chat:branch").is_some(), outcome == "success");
                book.saved().validate().unwrap();
                if outcome == "malformed" {
                    assert!(book.mirror.thread("branch").is_none());
                }
            }
        }
    }

    #[test]
    fn fork_receipt_store_is_backward_compatible_and_rejects_invalid_owners() {
        let old: Saved =
            serde_json::from_value(json!({"version":1,"bindings":{},"unresolved":{}})).unwrap();
        assert!(old.fork_origins.is_empty());
        old.validate().unwrap();
        for (owner, source) in [("chat:", "source"), ("foreign", "source"), ("graph:a", "")] {
            let mut saved = old.clone();
            saved.fork_origins.insert(owner.into(), source.into());
            assert!(saved.validate().is_err());
        }
        let mut saved = old;
        saved.fork_origins.insert("chat:a".into(), "source".into());
        saved.bindings.insert(
            "chat:a".into(),
            Binding {
                thread_id: "already-bound".into(),
                session_id: None,
                archived: false,
                deleted: false,
                never_submitted: false,
            },
        );
        assert!(saved.validate().is_err());
    }

    #[test]
    fn explicit_history_import_reads_without_resuming_and_preserves_archive() {
        let mut book = Conversations::default();
        let thread: Value =
            serde_json::from_str(include_str!("../tests/native-history-thread.json")).unwrap();
        let id = thread["id"].as_str().unwrap();
        let request = book.import("chat:destination", id, true, false).unwrap();
        assert_eq!(request.call.method, "thread/read");
        assert!(book.any_busy());
        assert!(book.import("chat:other", id, true, false).is_err());
        book.complete(&request, Ok(json!({"thread":thread})))
            .unwrap();
        assert!(book.binding("chat:destination").unwrap().archived);
        assert!(book.observed("chat:destination"));
        assert!(!book.loaded.contains(id));
        assert!(
            !serde_json::to_string(book.saved())
                .unwrap()
                .contains("Inspect the example")
        );
        assert!(!book.any_busy());
    }
    #[test]
    fn explicit_fork_is_independent_and_never_replaces_a_binding() {
        let mut book = Conversations::default();
        let temp = tempfile::tempdir().unwrap();
        opened(&mut book, "chat:source", "source", temp.path());
        let request = book
            .import("graph:destination", "source", false, true)
            .unwrap();
        assert_eq!(request.call.method, "thread/fork");
        assert_eq!(
            request.call.params,
            json!({"threadId":"source","excludeTurns":true})
        );
        assert!(
            book.import("graph:destination", "other", false, false)
                .is_err()
        );
        book.complete(&request, Ok(json!({"thread":thread("fork")})))
            .unwrap();
        assert_eq!(book.binding("chat:source").unwrap().thread_id, "source");
        assert!(!book.binding("graph:destination").unwrap().archived);
        assert_eq!(book.binding("graph:destination").unwrap().thread_id, "fork");
        assert!(
            book.import("graph:destination", "replacement", false, false)
                .is_err()
        );
    }
    #[test]
    fn bound_archived_fork_is_rejected_before_local_allocation_or_native_receipt() {
        let temp = tempfile::tempdir().unwrap();
        for owner in ["chat:source", "graph:source"] {
            let mut book = Conversations::default();
            opened(&mut book, owner, "source", temp.path());
            assert!(book.validate_fork_source(owner).is_ok());
            book.notification("thread/archived", &json!({"threadId":"source"}))
                .unwrap();
            let before = serde_json::to_value(book.saved()).unwrap();
            assert!(
                book.validate_fork_source(owner)
                    .unwrap_err()
                    .contains("Explicitly restore")
            );
            assert!(
                book.action(
                    owner,
                    Action::Fork {
                        destination: "graph:branch".into()
                    }
                )
                .unwrap_err()
                .contains("Explicitly restore")
            );
            assert_eq!(serde_json::to_value(book.saved()).unwrap(), before);
            assert!(book.pending.is_empty());
            assert!(!book.any_busy());
            assert!(book.saved.fork_origins.is_empty());
            assert!(book.binding("graph:branch").is_none());
            let restore = book.action(owner, Action::Unarchive).unwrap();
            assert_eq!(restore.call.method, "thread/unarchive");
        }
    }

    #[test]
    fn branch_preflight_rejects_stale_deleted_and_busy_sources_without_mutation() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        assert!(book.validate_fork_source("chat:missing").is_err());
        opened(&mut book, "chat:source", "source", temp.path());
        let request = book
            .action("chat:source", Action::Rename("Renamed".into()))
            .unwrap();
        assert!(book.validate_fork_source("chat:source").is_err());
        book.complete(&request, Ok(json!({}))).unwrap();
        assert!(book.validate_fork_source("chat:source").is_ok());
        let mut restored = Conversations::restore(book.saved().clone()).unwrap();
        assert!(restored.validate_fork_source("chat:source").is_err());
        restored
            .notification("thread/deleted", &json!({"threadId":"source"}))
            .unwrap();
        assert!(
            restored
                .validate_fork_source("chat:source")
                .unwrap_err()
                .contains("deleted")
        );
        assert!(restored.pending.is_empty());
        assert!(restored.saved.fork_origins.is_empty());
    }

    #[test]
    fn archived_fork_is_rejected_before_any_request_or_pending_origin() {
        let mut book = Conversations::default();
        assert!(
            book.import("chat:destination", "archived-source", true, true)
                .unwrap_err()
                .contains("explicitly restore")
        );
        assert!(!book.any_busy());
        assert!(book.saved.bindings.is_empty());
        assert!(book.saved.fork_origins.is_empty());
        assert!(book.pending.is_empty());
        assert_eq!(
            book.import("chat:destination", "archived-source", true, false)
                .unwrap()
                .call
                .method,
            "thread/read"
        );
    }
    #[test]
    fn import_rejects_foreign_ids_malformed_history_and_lifecycle_races() {
        for method in [
            "thread/deleted",
            "thread/archived",
            "thread/unarchived",
            "thread/closed",
        ] {
            let mut book = Conversations::default();
            let r = book.import("chat:a", "source", false, false).unwrap();
            book.notification(method, &json!({"threadId":"source"}))
                .unwrap();
            assert!(
                book.complete(&r, Ok(json!({"thread":thread("source")})))
                    .is_err()
            );
            assert!(book.binding("chat:a").is_none());
        }
        let mut book = Conversations::default();
        let r = book.import("chat:a", "source", false, false).unwrap();
        assert!(
            book.complete(&r, Ok(json!({"thread":thread("different")})))
                .is_err()
        );
        let r = book.import("chat:a", "source", false, false).unwrap();
        assert!(
            book.complete(&r, Ok(json!({"thread":{"id":"source","turns":[{}]}})))
                .is_err()
        );
        assert!(book.binding("chat:a").is_none());
        assert!(book.mirror.thread("source").is_none());
    }
    #[test]
    fn disconnected_import_and_fork_are_not_replayed_or_bound_from_late_ack() {
        for fork in [true, false] {
            let mut book = Conversations::default();
            let r = book.import("chat:a", "source", false, fork).unwrap();
            book.disconnect();
            assert!(!book.any_busy());
            assert!(
                book.complete(
                    &r,
                    Ok(json!({"thread":thread(if fork {"fork"} else {"source"})}))
                )
                .is_err()
            );
            assert!(book.saved().bindings.is_empty());
        }
    }
    #[test]
    fn lifecycle_notifications_are_scoped_and_outrank_earlier_responses() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native", temp.path());
        opened(&mut book, "graph:b", "other", temp.path());
        let samples: Vec<Value> =
            serde_json::from_str(include_str!("../tests/thread-lifecycle.json")).unwrap();
        let read = book.action("chat:a", Action::Read).unwrap();
        assert!(
            book.notification(
                samples[0]["method"].as_str().unwrap(),
                &samples[0]["params"]
            )
            .unwrap()
        );
        let mut snapshot = thread("native");
        snapshot["name"] = json!("Older name");
        book.complete(&read, Ok(json!({"thread":snapshot})))
            .unwrap();
        assert_eq!(
            book.mirror.thread("native").unwrap().name.as_deref(),
            Some("Renamed remotely")
        );
        let rename = book
            .action("chat:a", Action::Rename("Requested name".into()))
            .unwrap();
        book.notification("thread/name/updated", &samples[1]["params"])
            .unwrap();
        book.complete(&rename, Ok(json!({}))).unwrap();
        assert!(book.mirror.thread("native").unwrap().name.is_none());
        let archive = book.action("chat:a", Action::Archive).unwrap();
        for sample in &samples[2..4] {
            assert!(
                book.notification(sample["method"].as_str().unwrap(), &sample["params"])
                    .unwrap()
            );
        }
        book.complete(&archive, Ok(json!({}))).unwrap();
        assert!(!book.binding("chat:a").unwrap().archived);
        assert!(!book.binding("graph:b").unwrap().archived);
        let resume = book
            .open("chat:a", temp.path(), &Profile::default(), Access::ReadOnly)
            .unwrap();
        book.notification("thread/closed", &samples[4]["params"])
            .unwrap();
        book.complete(&resume, Ok(json!({"thread":thread("native")})))
            .unwrap();
        assert!(
            book.start_turn(
                "chat:a",
                "late",
                vec![api::text_input("no")],
                temp.path(),
                &Profile::default(),
                Access::ReadOnly
            )
            .is_err()
        );
        assert!(!book.busy("chat:a"));
        assert!(
            !book
                .notification("thread/deleted", &json!({"threadId":"foreign"}))
                .unwrap()
        );
    }

    #[test]
    fn native_deletion_before_ack_is_terminal_across_restart_and_late_events() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native", temp.path());
        opened(&mut book, "graph:b", "other", temp.path());
        let read = book.action("chat:a", Action::Read).unwrap();
        let delete = book.action("chat:a", Action::Delete).unwrap();
        book.notification("thread/deleted", &json!({"threadId":"native"}))
            .unwrap();
        assert_eq!(
            book.complete(&delete, Ok(json!({}))).unwrap(),
            Outcome::Deleted {
                owner: "chat:a".into()
            }
        );
        assert!(
            book.complete(&read, Ok(json!({"thread":thread("native")})))
                .is_err()
        );
        assert!(!book.notification("turn/started", &json!({"threadId":"native","turn":{"id":"late","status":"inProgress","items":[]}})).unwrap());
        assert!(book.mirror.thread("native").is_none());
        assert!(!book.binding("graph:b").unwrap().deleted);
        let bytes = serde_json::to_vec(book.saved()).unwrap();
        let mut restored = Conversations::restore(serde_json::from_slice(&bytes).unwrap()).unwrap();
        assert!(restored.binding("chat:a").unwrap().deleted);
        assert!(!restored.busy("chat:a"));
        assert!(
            restored
                .open("chat:a", temp.path(), &Profile::default(), Access::ReadOnly)
                .is_err()
        );
        assert!(restored.action("chat:a", Action::Unarchive).is_err());
        assert!(restored.action("chat:a", Action::Read).is_err());
        // Old binding files remain readable without a destructive migration.
        let mut old: Value = serde_json::from_slice(&bytes).unwrap();
        old["bindings"]["graph:b"]
            .as_object_mut()
            .unwrap()
            .remove("deleted");
        assert!(
            !Conversations::restore(serde_json::from_value(old).unwrap())
                .unwrap()
                .binding("graph:b")
                .unwrap()
                .deleted
        );
    }

    #[test]
    fn deletion_discards_receipts_without_acknowledging_or_replaying_pending_input() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native", temp.path());
        let pending = start(&mut book, "chat:a", "message", temp.path());
        book.notification("thread/deleted", &json!({"threadId":"native"}))
            .unwrap();
        assert!(book.saved.unresolved.is_empty());
        assert!(
            book.complete(
                &pending,
                Ok(json!({"turn":{"id":"t","status":"inProgress","items":[]}}))
            )
            .is_err()
        );
        assert!(book.mirror.thread("native").is_none());
        assert!(!book.busy("chat:a"));
    }
    fn thread(id: &str) -> Value {
        json!({"id":id,"sessionId":format!("session-{id}"),"status":{"type":"idle"},"turns":[]})
    }
    fn opened(book: &mut Conversations, owner: &str, id: &str, cwd: &Path) {
        let request = book
            .open(owner, cwd, &Profile::default(), Access::ReadOnly)
            .unwrap();
        book.complete(&request, Ok(json!({"thread":thread(id)})))
            .unwrap();
    }
    fn start(book: &mut Conversations, owner: &str, message: &str, cwd: &Path) -> Request {
        book.start_turn(
            owner,
            message,
            vec![api::text_input("private user request")],
            cwd,
            &Profile::default(),
            Access::ReadOnly,
        )
        .unwrap()
    }
    #[test]
    fn concurrent_main_and_graph_owners_do_not_follow_selected_chat() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        let a = book
            .open("chat:a", temp.path(), &Profile::default(), Access::ReadOnly)
            .unwrap();
        let b = book
            .open(
                "graph:b",
                temp.path(),
                &Profile::default(),
                Access::ReadOnly,
            )
            .unwrap();
        book.complete(&b, Ok(json!({"thread":thread("thread-b")})))
            .unwrap();
        book.complete(&a, Ok(json!({"thread":thread("thread-a")})))
            .unwrap();
        assert_eq!(book.owner_for_thread("thread-b"), Some("graph:b"));
        let a = start(&mut book, "chat:a", "message-a", temp.path());
        let b = start(&mut book, "graph:b", "message-b", temp.path());
        assert_eq!(a.call.params["threadId"], "thread-a");
        assert_eq!(b.call.params["threadId"], "thread-b");
        assert_eq!(
            book.complete(
                &b,
                Ok(json!({"turn":{"id":"turn-b","status":"inProgress","items":[]}}))
            )
            .unwrap(),
            Outcome::Accepted {
                owner: "graph:b".into(),
                message_id: "message-b".into(),
                turn_id: "turn-b".into()
            }
        );
        book.complete(
            &a,
            Ok(json!({"turn":{"id":"turn-a","status":"inProgress","items":[]}})),
        )
        .unwrap();
        assert_eq!(book.active_turn("chat:a"), Some("turn-a"));
        assert_eq!(book.active_turn("graph:b"), Some("turn-b"));
        assert!(
            !book
                .notification(
                    "item/agentMessage/delta",
                    &json!({"threadId":"foreign","turnId":"t","itemId":"i","delta":"do not import"})
                )
                .unwrap()
        );
    }
    #[test]
    fn bindings_are_native_ids_and_do_not_store_or_replay_transcripts() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native", temp.path());
        let request = start(&mut book, "chat:a", "message-a", temp.path());
        let bytes = serde_json::to_vec(book.saved()).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("private user request"));
        assert!(
            !request
                .call
                .params
                .as_object()
                .unwrap()
                .contains_key("history")
        );
        let mut restored = Conversations::restore(serde_json::from_slice(&bytes).unwrap()).unwrap();
        assert!(
            restored
                .start_turn(
                    "chat:a",
                    "other",
                    vec![api::text_input("no")],
                    temp.path(),
                    &Profile::default(),
                    Access::ReadOnly
                )
                .is_err()
        );
        let read = restored.action("chat:a", Action::Read).unwrap();
        assert_eq!(read.call.method, "thread/read");
        let mut snapshot = thread("native");
        // A server item ID that merely matches our receipt is not correlation.
        snapshot["turns"] = json!([{"id":"t","status":"completed","items":[{"id":"message-a","clientId":null,"type":"userMessage","content":[]}]}]);
        restored
            .complete(&read, Ok(json!({"thread":snapshot})))
            .unwrap();
        assert!(!restored.saved.unresolved.is_empty());
        let read = restored.action("chat:a", Action::Read).unwrap();
        snapshot["turns"] = json!([{"id":"t","status":"completed","items":[{"id":"server-item","clientId":"message-a","type":"userMessage","content":[]}]}]);
        restored
            .complete(&read, Ok(json!({"thread":snapshot})))
            .unwrap();
        assert!(restored.saved.unresolved.is_empty());
        let resume = restored
            .open("chat:a", temp.path(), &Profile::default(), Access::ReadOnly)
            .unwrap();
        assert_eq!(resume.call.method, "thread/resume");
        assert_eq!(
            resume.call.params,
            json!({"threadId":"native","excludeTurns":true})
        );
    }
    #[test]
    fn paginated_history_reconciles_only_the_exact_native_client_id() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native", temp.path());
        let request = start(&mut book, "chat:a", "message-a", temp.path());
        let saved = book.saved().clone();
        drop(request);

        let mut restored = Conversations::restore(saved).unwrap();
        let read = restored.action("chat:a", Action::Read).unwrap();
        let mut metadata = thread("native");
        metadata["historyMode"] = json!("paginated");
        restored
            .complete(&read, Ok(json!({"thread":metadata})))
            .unwrap();
        assert!(restored.saved.unresolved.contains_key("chat:a"));

        let revision = restored.mirror.revision();
        restored
            .hydrate_paginated_history(
                "chat:a",
                "native",
                &[json!({"id":"wrong","status":"completed","items":[{"id":"user-wrong","clientId":"other-message","type":"userMessage","content":[]}]})],
                revision,
            )
            .unwrap();
        assert!(restored.saved.unresolved.contains_key("chat:a"));

        let revision = restored.mirror.revision();
        restored
            .hydrate_paginated_history(
                "chat:a",
                "native",
                &[json!({"id":"accepted","status":"completed","items":[{"id":"user-accepted","clientId":"message-a","type":"userMessage","content":[]}]})],
                revision,
            )
            .unwrap();
        assert!(!restored.saved.unresolved.contains_key("chat:a"));
    }
    #[test]
    fn disconnection_and_late_acknowledgement_never_replay_or_clear_a_new_draft() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native", temp.path());
        let request = start(&mut book, "chat:a", "message-a", temp.path());
        book.disconnect();
        assert!(
            book.complete(&request, Ok(json!({"turn":{"id":"t"}})))
                .is_err()
        );
        assert_eq!(book.saved.unresolved["chat:a"].message_id, "message-a");
        let read = book.action("chat:a", Action::Read).unwrap();
        assert!(
            book.resolve_delivery_after_user_review("chat:a", "message-a")
                .is_err()
        );
        assert_eq!(book.saved.unresolved["chat:a"].message_id, "message-a");
        book.complete(&read, Ok(json!({"thread":thread("native")})))
            .unwrap();
        assert!(book.busy("chat:a")); // Absence in one read is not proof of rejection.
        assert!(
            book.open("chat:a", temp.path(), &Profile::default(), Access::ReadOnly)
                .is_err()
        );
        assert!(
            book.resolve_delivery_after_user_review("chat:a", "another-message")
                .is_err()
        );
        book.resolve_delivery_after_user_review("chat:a", "message-a")
            .unwrap();
        assert!(book.saved.unresolved.is_empty());
        assert!(book.pending.is_empty());
    }
    #[test]
    fn restored_receipt_requires_successful_current_owner_history_before_dismissal() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native-a", temp.path());
        opened(&mut book, "graph:b", "native-b", temp.path());
        start(&mut book, "chat:a", "pending-a", temp.path());
        let saved = serde_json::from_slice(&serde_json::to_vec(book.saved()).unwrap()).unwrap();
        let mut restored = Conversations::restore(saved).unwrap();
        assert!(
            restored
                .resolve_delivery_after_user_review("chat:a", "pending-a")
                .is_err()
        );
        let other = restored.action("graph:b", Action::Read).unwrap();
        restored
            .complete(&other, Ok(json!({"thread":thread("native-b")})))
            .unwrap();
        assert!(
            restored
                .resolve_delivery_after_user_review("chat:a", "pending-a")
                .is_err()
        );
        let wrong = restored.action("chat:a", Action::Read).unwrap();
        assert!(
            restored
                .complete(&wrong, Ok(json!({"thread":thread("foreign")})))
                .is_err()
        );
        assert!(
            restored
                .resolve_delivery_after_user_review("chat:a", "pending-a")
                .is_err()
        );
        assert_eq!(restored.saved.unresolved["chat:a"].message_id, "pending-a");
        let read = restored.action("chat:a", Action::Read).unwrap();
        restored
            .complete(&read, Ok(json!({"thread":thread("native-a")})))
            .unwrap();
        restored
            .resolve_delivery_after_user_review("chat:a", "pending-a")
            .unwrap();
        assert!(restored.saved.unresolved.is_empty());
        assert!(restored.pending.is_empty());
        assert_eq!(restored.binding("chat:a").unwrap().thread_id, "native-a");
        assert_eq!(restored.binding("graph:b").unwrap().thread_id, "native-b");
    }
    #[test]
    fn fast_completion_preceding_acceptance_does_not_reopen_the_turn() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native", temp.path());
        let request = start(&mut book, "chat:a", "message-a", temp.path());
        book.notification(
            "turn/completed",
            &json!({"threadId":"native","turn":{"id":"t","status":"completed","items":[]}}),
        )
        .unwrap();
        book.complete(
            &request,
            Ok(json!({"turn":{"id":"t","status":"inProgress","items":[]}})),
        )
        .unwrap();
        assert_eq!(book.active_turn("chat:a"), None);
        assert!(!book.busy("chat:a"));
    }
    #[test]
    fn lifecycle_cannot_target_foreign_threads_or_rebind_a_response() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        assert!(book.action("chat:personal", Action::Delete).is_err());
        opened(&mut book, "chat:a", "native", temp.path());
        let b = book
            .open(
                "graph:b",
                temp.path(),
                &Profile::default(),
                Access::ReadOnly,
            )
            .unwrap();
        assert!(
            book.complete(&b, Ok(json!({"thread":thread("native")})))
                .is_err()
        );
        let read = book.action("chat:a", Action::Read).unwrap();
        assert!(
            book.complete(&read, Ok(json!({"thread":thread("foreign")})))
                .is_err()
        );
        assert_eq!(book.binding("chat:a").unwrap().thread_id, "native");
        let fork = book
            .action(
                "chat:a",
                Action::Fork {
                    destination: "graph:c".into(),
                },
            )
            .unwrap();
        book.complete(&fork, Ok(json!({"thread":thread("branch")})))
            .unwrap();
        assert_eq!(book.owner_for_thread("branch"), Some("graph:c"));
        let archive = book.action("chat:a", Action::Archive).unwrap();
        book.complete(&archive, Ok(json!({}))).unwrap();
        let read = book.action("chat:a", Action::Read).unwrap();
        book.complete(&read, Ok(json!({"thread":thread("native")})))
            .unwrap();
        assert!(book.binding("chat:a").unwrap().archived);
        assert!(
            book.open("chat:a", temp.path(), &Profile::default(), Access::ReadOnly)
                .is_err()
        );
        assert!(!book.binding("graph:c").unwrap().archived);
    }
    #[test]
    fn steering_is_native_and_an_uncertain_steer_needs_explicit_review() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native", temp.path());
        let request = start(&mut book, "chat:a", "message-a", temp.path());
        book.complete(
            &request,
            Ok(json!({"turn":{"id":"t","status":"inProgress","items":[]}})),
        )
        .unwrap();
        let steer = book
            .steer(
                "chat:a",
                "t",
                "followup",
                vec![api::text_input("new input")],
            )
            .unwrap();
        assert_eq!(steer.call.method, "turn/steer");
        assert_eq!(steer.call.params["expectedTurnId"], "t");
        assert_eq!(steer.call.params["clientUserMessageId"], "followup");
        assert!(steer.call.params.get("model").is_none());
        book.complete(
            &steer,
            Err(CallError::Disconnected {
                reason: "lost".into(),
                delivery_unknown: true,
            }),
        )
        .unwrap_err();
        let read = book.action("chat:a", Action::Read).unwrap();
        let mut snapshot = thread("native");
        snapshot["turns"] = json!([{"id":"t","status":"completed","items":[{"id":"followup","type":"userMessage","content":[]}]}]);
        book.complete(&read, Ok(json!({"thread":snapshot})))
            .unwrap();
        assert!(book.saved.unresolved.contains_key("chat:a"));
    }
    #[test]
    fn steering_target_cannot_be_replaced_by_a_new_turn_or_other_owner() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native", temp.path());
        opened(&mut book, "graph:b", "other-native", temp.path());
        let request = start(&mut book, "chat:a", "first", temp.path());
        assert!(book.steer_target("chat:a").is_none());
        book.complete(
            &request,
            Ok(json!({"turn":{"id":"t1","status":"inProgress","items":[]}})),
        )
        .unwrap();
        assert_eq!(book.steer_target("chat:a"), Some("t1"));
        assert!(
            book.steer("graph:b", "t1", "x", vec![api::text_input("Wrong owner")])
                .is_err()
        );
        assert!(
            book.steer("chat:a", "stale", "x", vec![api::text_input("Old view")])
                .is_err()
        );
        assert!(!book.saved.unresolved.contains_key("chat:a"));
        let steer = book
            .steer("chat:a", "t1", "follow", vec![api::text_input("Follow up")])
            .unwrap();
        assert!(book.steer_target("chat:a").is_none());
        assert!(
            book.steer(
                "chat:a",
                "t1",
                "duplicate",
                vec![api::text_input("Duplicate")]
            )
            .is_err()
        );
        book.notification(
            "turn/completed",
            &json!({"threadId":"native","turn":{"id":"t1","status":"completed","items":[]}}),
        )
        .unwrap();
        let outcome = book.complete(&steer, Ok(json!({"turnId":"t1"}))).unwrap();
        assert!(
            matches!(outcome, Outcome::Accepted { owner, message_id, .. } if owner=="chat:a" && message_id=="follow")
        );
        assert!(book.steer_target("chat:a").is_none());
        let next = start(&mut book, "chat:a", "second", temp.path());
        book.complete(
            &next,
            Ok(json!({"turn":{"id":"t2","status":"inProgress","items":[]}})),
        )
        .unwrap();
        assert!(
            book.steer("chat:a", "t1", "late", vec![api::text_input("Late click")])
                .is_err()
        );
        assert_eq!(book.steer_target("chat:a"), Some("t2"));
        book.disconnect();
        assert!(book.steer_target("chat:a").is_none());
    }
    #[test]
    fn rejected_steering_clears_receipt_but_a_wrong_acknowledgement_stays_uncertain() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native", temp.path());
        let request = start(&mut book, "chat:a", "first", temp.path());
        book.complete(
            &request,
            Ok(json!({"turn":{"id":"t","status":"inProgress","items":[]}})),
        )
        .unwrap();
        let steer = book
            .steer("chat:a", "t", "follow", vec![api::text_input("Follow up")])
            .unwrap();
        book.complete(&steer, Err(CallError::Rejected("Not sent".into())))
            .unwrap_err();
        assert!(!book.saved.unresolved.contains_key("chat:a"));
        let steer = book
            .steer(
                "chat:a",
                "t",
                "follow2",
                vec![api::text_input("Different input")],
            )
            .unwrap();
        book.complete(&steer, Ok(json!({"turnId":"wrong"})))
            .unwrap_err();
        assert!(book.saved.unresolved["chat:a"].steer);
        assert!(book.steer_target("chat:a").is_none());
    }
    #[test]
    fn duplicate_store_bindings_and_remote_cwd_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native", temp.path());
        let mut saved = book.saved().clone();
        saved
            .bindings
            .insert("graph:b".into(), saved.bindings["chat:a"].clone());
        assert!(Conversations::restore(saved).is_err());
        assert!(
            book.open(
                "chat:b",
                Path::new("remote:server/project"),
                &Profile::default(),
                Access::ReadOnly
            )
            .is_err()
        );
        let request = start(&mut book, "chat:a", "message-a", temp.path());
        book.complete(&request, Err(CallError::Rejected("not sent".into())))
            .unwrap_err();
        assert!(!book.busy("chat:a"));
    }
    #[test]
    fn native_active_status_blocks_duplicate_turn_before_items_arrive() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native", temp.path());
        book.notification(
            "thread/status/changed",
            &json!({"threadId":"native","status":{"type":"active","activeFlags":[]}}),
        )
        .unwrap();
        assert!(book.busy("chat:a"));
        assert!(
            book.start_turn(
                "chat:a",
                "m",
                vec![api::text_input("do not duplicate")],
                temp.path(),
                &Profile::default(),
                Access::ReadOnly
            )
            .is_err()
        );
    }

    #[test]
    fn explicit_local_unlink_releases_only_the_connection_subscription() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native", temp.path());

        let unlink = book.prepare_unlink("chat:a").unwrap().unwrap();
        assert_eq!(unlink.owner, "chat:a");
        assert_eq!(unlink.thread_id, "native");
        assert!(unlink.subscribed);
        assert!(book.binding("chat:a").is_some());

        let call = book.commit_unlink(&unlink).unwrap().unwrap();
        assert_eq!(call.method, "thread/unsubscribe");
        assert_eq!(call.params, json!({"threadId":"native"}));
        assert!(book.binding("chat:a").is_none());
        assert!(book.mirror.thread("native").is_none());
        assert!(book.import("chat:b", "native", false, false).is_err());

        book.complete_unsubscribe("native", Ok(json!({"status":"unsubscribed"})))
            .unwrap();
        let import = book.import("chat:b", "native", false, false).unwrap();
        assert_eq!(import.call.method, "thread/read");
        assert_eq!(
            import.call.params,
            json!({"threadId":"native","includeTurns":false})
        );
        assert!(
            book.complete_unsubscribe("native", Ok(json!({"status":"unsubscribed"})))
                .is_err()
        );

        let mut uncertain = Conversations::default();
        uncertain
            .saved
            .fork_origins
            .insert("chat:branch".into(), "native-source".into());
        assert!(
            uncertain
                .prepare_unlink("chat:branch")
                .unwrap_err()
                .contains("uncertain native fork")
        );
        assert_eq!(uncertain.saved.fork_origins["chat:branch"], "native-source");
    }

    #[test]
    fn loaded_inventory_reconciles_known_bindings_without_inventing_ownership() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:a", "native", temp.path());
        book.disconnect();
        assert!(!book.is_thread_loaded("chat:a"));
        assert!(!book.observed("chat:a"));

        let changed = book.reconcile_loaded(&HashSet::from([
            "native".to_owned(),
            "unowned-native-thread".to_owned(),
        ]));
        assert_eq!(changed, vec!["chat:a"]);
        assert!(book.is_thread_loaded("chat:a"));
        assert!(!book.observed("chat:a"));
        assert!(book.owner_for_thread("unowned-native-thread").is_none());

        let changed = book.reconcile_loaded(&HashSet::new());
        assert_eq!(changed, vec!["chat:a"]);
        assert!(!book.is_thread_loaded("chat:a"));
        assert!(book.mirror.thread("native").unwrap().status.is_none());
    }

    #[test]
    fn prompt_events_fork_and_restart_preserve_one_native_conversation_chain() {
        let temp = tempfile::tempdir().unwrap();
        let mut book = Conversations::default();
        opened(&mut book, "chat:source", "native-source", temp.path());

        let request = start(&mut book, "chat:source", "client-message", temp.path());
        assert_eq!(request.call.method, "turn/start");
        assert_eq!(request.call.params["clientUserMessageId"], "client-message");
        assert!(book.saved.unresolved.contains_key("chat:source"));
        assert!(matches!(
            book.complete(
                &request,
                Ok(json!({"turn":{"id":"turn-a","status":"inProgress","items":[]}}))
            )
            .unwrap(),
            Outcome::Accepted { message_id, turn_id, .. }
                if message_id == "client-message" && turn_id == "turn-a"
        ));
        assert!(!book.saved.unresolved.contains_key("chat:source"));

        let events = [
            (
                "item/started",
                json!({"threadId":"native-source","turnId":"turn-a","item":{"id":"user","type":"userMessage","clientId":"client-message","content":[{"type":"text","text":"Inspect"}]}}),
            ),
            (
                "item/completed",
                json!({"threadId":"native-source","turnId":"turn-a","item":{"id":"user","type":"userMessage","clientId":"client-message","content":[{"type":"text","text":"Inspect"}]}}),
            ),
            (
                "item/started",
                json!({"threadId":"native-source","turnId":"turn-a","item":{"id":"reasoning","type":"reasoning","summary":[],"content":[]}}),
            ),
            (
                "item/reasoning/summaryTextDelta",
                json!({"threadId":"native-source","turnId":"turn-a","itemId":"reasoning","summaryIndex":0,"delta":"Checking files"}),
            ),
            (
                "item/completed",
                json!({"threadId":"native-source","turnId":"turn-a","item":{"id":"reasoning","type":"reasoning","summary":["Checking files"],"content":["PRIVATE REASONING"]}}),
            ),
            (
                "item/started",
                json!({"threadId":"native-source","turnId":"turn-a","item":{"id":"command","type":"commandExecution","status":"inProgress","command":"cargo test","aggregatedOutput":""}}),
            ),
            (
                "item/commandExecution/outputDelta",
                json!({"threadId":"native-source","turnId":"turn-a","itemId":"command","delta":"tests passed"}),
            ),
            (
                "item/completed",
                json!({"threadId":"native-source","turnId":"turn-a","item":{"id":"command","type":"commandExecution","status":"completed","command":"cargo test","aggregatedOutput":"tests passed"}}),
            ),
            (
                "item/started",
                json!({"threadId":"native-source","turnId":"turn-a","item":{"id":"update","type":"agentMessage","phase":"commentary","text":""}}),
            ),
            (
                "item/agentMessage/delta",
                json!({"threadId":"native-source","turnId":"turn-a","itemId":"update","delta":"Verification complete."}),
            ),
            (
                "item/completed",
                json!({"threadId":"native-source","turnId":"turn-a","item":{"id":"update","type":"agentMessage","phase":"commentary","text":"Verification complete."}}),
            ),
            (
                "item/started",
                json!({"threadId":"native-source","turnId":"turn-a","item":{"id":"final","type":"agentMessage","phase":"final_answer","text":""}}),
            ),
            (
                "item/agentMessage/delta",
                json!({"threadId":"native-source","turnId":"turn-a","itemId":"final","delta":"Done"}),
            ),
            (
                "item/completed",
                json!({"threadId":"native-source","turnId":"turn-a","item":{"id":"final","type":"agentMessage","phase":"final_answer","text":"Done"}}),
            ),
            (
                "turn/completed",
                json!({"threadId":"native-source","turn":{"id":"turn-a","status":"completed","items":[]}}),
            ),
        ];
        for (method, params) in events {
            assert!(book.notification(method, &params).unwrap(), "{method}");
        }
        let history_items = {
            let turn = &book.mirror.thread("native-source").unwrap().turns[0];
            assert_eq!(
                turn.items
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect::<Vec<_>>(),
                ["user", "reasoning", "command", "update", "final"]
            );
            assert_eq!(turn.status, "completed");
            assert_eq!(turn.items[1].value["summary"][0], "Checking files");
            assert!(turn.items[1].value.get("content").is_none());
            assert_eq!(turn.items[2].value["aggregatedOutput"], "tests passed");
            assert_eq!(turn.items[4].value["phase"], "final_answer");
            turn.items
                .iter()
                .map(|item| item.value.clone())
                .collect::<Vec<_>>()
        };

        let fork = book
            .action(
                "chat:source",
                Action::Fork {
                    destination: "graph:child".into(),
                },
            )
            .unwrap();
        let mut child = thread("native-child");
        child["forkedFromId"] = json!("native-source");
        book.complete(&fork, Ok(json!({"thread":child}))).unwrap();
        assert_eq!(book.saved.lineage["native-child"], "native-source");

        let stored = serde_json::to_string(book.saved()).unwrap();
        let mut restored = Conversations::restore(serde_json::from_str(&stored).unwrap()).unwrap();
        assert_eq!(
            restored.binding("graph:child").unwrap().thread_id,
            "native-child"
        );
        assert_eq!(restored.saved.lineage["native-child"], "native-source");

        let resume = restored
            .open(
                "graph:child",
                temp.path(),
                &Profile::default(),
                Access::ReadOnly,
            )
            .unwrap();
        assert_eq!(resume.call.method, "thread/resume");
        let mut reloaded_child = thread("native-child");
        reloaded_child["forkedFromId"] = json!("native-source");
        reloaded_child["turns"] =
            json!([{"id":"turn-a","status":"completed","items":history_items}]);
        restored
            .complete(&resume, Ok(json!({"thread":reloaded_child})))
            .unwrap();
        let reloaded = &restored.mirror.thread("native-child").unwrap().turns[0];
        assert_eq!(
            reloaded
                .items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["user", "reasoning", "command", "update", "final"]
        );
        assert_eq!(restored.saved.lineage["native-child"], "native-source");
    }
}
