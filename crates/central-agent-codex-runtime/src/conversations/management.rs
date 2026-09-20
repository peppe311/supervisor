//! Backend-only P3 intents. The host must persist `Saved` before dispatching a
//! revert and after reconciling it. No operation here replays a mutation.
use super::*;
use crate::{
    thread_metadata::{GitMetadata, NativeMetadata, RepositorySnapshot, text},
    thread_sections::Sections,
};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_SCOPE: AtomicU64 = AtomicU64::new(1);

pub(super) struct State {
    scope: u64,
    histories: HashMap<String, History>,
    pub(super) refresh_required: HashSet<String>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            scope: NEXT_SCOPE.fetch_add(1, Ordering::Relaxed),
            histories: HashMap::new(),
            refresh_required: HashSet::new(),
        }
    }
}
impl State {
    pub(super) fn forget(&mut self, owner: &str) {
        self.histories.remove(owner);
        self.refresh_required.remove(owner);
    }
    pub(super) fn disconnect(&mut self) {
        self.histories.clear();
        // A failed/uncertain destructive operation still requires a fresh read.
    }
}
struct History {
    revision: u64,
    ids: Vec<String>,
}

struct Stamp {
    scope: u64,
    owner: String,
    thread_id: String,
    generation: u64,
    serial: u64,
    revision: u64,
    lifecycle: u64,
    cwd: PathBuf,
}

/// These capabilities cannot be deserialized from IPC or cloned for replay.
pub struct GitConfirmation {
    stamp: Stamp,
    repository: RepositorySnapshot,
}
impl GitConfirmation {
    pub fn metadata(&self) -> &GitMetadata {
        self.repository.metadata()
    }
    pub fn directory(&self) -> &Path {
        self.repository.directory()
    }
}
pub struct SectionMoveConfirmation {
    stamp: Stamp,
    inventory: (u64, u64),
    section_id: Option<String>,
    before_thread_id: Option<String>,
}
pub struct RevertConfirmation {
    stamp: Stamp,
    receipt: RevertReceipt,
}
impl RevertConfirmation {
    pub fn before_turn_id(&self) -> &str {
        &self.receipt.before_turn_id
    }
    pub fn removed_turn_count(&self) -> usize {
        self.receipt.removed_turn_count
    }
    pub fn retained_turn_count(&self) -> usize {
        self.receipt.retained_turn_ids.len()
    }
    pub fn thread_id(&self) -> &str {
        &self.receipt.thread_id
    }
}

/// Minimal write-ahead receipt: identities and counts only, no conversation
/// text, credentials, Git URL, repository path or replacement transcript.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RevertReceipt {
    pub thread_id: String,
    pub operation_id: String,
    pub before_turn_id: String,
    pub retained_turn_ids: Vec<String>,
    pub removed_turn_count: usize,
}
impl RevertReceipt {
    pub(super) fn validate(&self) -> Result<(), String> {
        let unique: HashSet<_> = self.retained_turn_ids.iter().collect();
        if !text(&self.thread_id, 256)
            || !text(&self.operation_id, 256)
            || !text(&self.before_turn_id, 256)
            || self.removed_turn_count == 0
            || self.removed_turn_count > 4096
            || self
                .retained_turn_ids
                .len()
                .saturating_add(self.removed_turn_count)
                > 4096
            || unique.len() != self.retained_turn_ids.len()
            || self
                .retained_turn_ids
                .iter()
                .any(|id| !text(id, 256) || id == &self.before_turn_id)
        {
            return Err("Invalid protected revert receipt".into());
        }
        Ok(())
    }
}

impl Conversations {
    pub fn native_metadata(&self, owner: &str) -> Option<&NativeMetadata> {
        let binding = self.binding(owner)?;
        if binding.deleted || !self.observed(owner) {
            return None;
        }
        let thread = self.mirror.thread(&binding.thread_id)?;
        thread
            .metadata_current
            .then_some(thread.metadata.as_ref())
            .flatten()
    }
    pub fn revert_pending(&self, owner: &str) -> bool {
        self.pending
            .values()
            .any(|r| r.owner == owner && matches!(r.kind, Kind::Revert))
    }
    fn management_stamp(&self, owner: &str, cwd: &Path) -> Result<Stamp, String> {
        if !self.ready_for_turn(owner) || self.pending.values().any(|r| r.owner == owner) {
            return Err(
                "Read/resume an idle native conversation before preparing this operation".into(),
            );
        }
        let metadata = self
            .native_metadata(owner)
            .ok_or("Refresh native thread metadata first")?;
        if !same_existing_directory(&metadata.cwd, cwd) {
            return Err("The selected directory does not match this native conversation".into());
        }
        Ok(Stamp {
            scope: self.management.scope,
            owner: owner.into(),
            thread_id: self.binding(owner).unwrap().thread_id.clone(),
            generation: self.generation,
            serial: self.serial,
            revision: self.mirror.revision(),
            lifecycle: self.lifecycle_revision(owner),
            cwd: cwd.canonicalize().map_err(|_| "Directory is unavailable")?,
        })
    }
    fn validate_management_stamp(&self, stamp: &Stamp) -> Result<(), String> {
        let current = self.management_stamp(&stamp.owner, &stamp.cwd)?;
        if (
            current.scope,
            current.generation,
            current.serial,
            current.revision,
            current.lifecycle,
            &current.thread_id,
        ) != (
            stamp.scope,
            stamp.generation,
            stamp.serial,
            stamp.revision,
            stamp.lifecycle,
            &stamp.thread_id,
        ) {
            return Err("The native conversation changed; prepare a new confirmation".into());
        }
        Ok(())
    }
    pub fn prepare_git_metadata(&self, owner: &str, cwd: &Path) -> Result<GitConfirmation, String> {
        let stamp = self.management_stamp(owner, cwd)?;
        let repository = RepositorySnapshot::capture(&stamp.cwd)?;
        Ok(GitConfirmation { stamp, repository })
    }
    /// Re-read the same bound worktree immediately before creating the request.
    /// This updates native metadata only; it never runs checkout/reset/revert.
    pub fn confirm_git_metadata(
        &mut self,
        confirmation: GitConfirmation,
    ) -> Result<Request, String> {
        self.validate_management_stamp(&confirmation.stamp)?;
        confirmation
            .repository
            .revalidate(&confirmation.stamp.cwd)?;
        let git = confirmation.repository.metadata().clone();
        let stamp = confirmation.stamp;
        self.mirror.invalidate_metadata(&stamp.thread_id);
        Ok(self.request(
            &stamp.owner,
            api::update_thread_git_info(&stamp.thread_id, &git.update()),
            Kind::GitMetadata {
                cwd: stamp.cwd,
                git,
            },
        ))
    }
    pub fn prepare_section_move(
        &self,
        owner: &str,
        cwd: &Path,
        sections: &Sections,
        section_id: Option<&str>,
        before_owner: Option<&str>,
    ) -> Result<SectionMoveConfirmation, String> {
        let stamp = self.management_stamp(owner, cwd)?;
        let inventory = sections.version()?;
        if let Some(section_id) = section_id {
            sections.section(section_id)?;
        }
        let before_thread_id = if let Some(before_owner) = before_owner {
            if before_owner == owner {
                return Err("A thread cannot be ordered before itself".into());
            }
            let binding = self
                .binding(before_owner)
                .ok_or("The ordering anchor is not locally owned")?;
            let metadata = self
                .native_metadata(before_owner)
                .ok_or("Read the ordering anchor's current metadata first")?;
            if binding.archived
                || binding.deleted
                || metadata.section.as_ref().map(|s| s.id.as_str()) != section_id
            {
                return Err("The ordering anchor is not in the requested section".into());
            }
            Some(binding.thread_id.clone())
        } else {
            None
        };
        Ok(SectionMoveConfirmation {
            stamp,
            inventory,
            section_id: section_id.map(str::to_owned),
            before_thread_id,
        })
    }
    pub fn confirm_section_move(
        &mut self,
        confirmation: SectionMoveConfirmation,
        sections: &Sections,
    ) -> Result<Request, String> {
        self.validate_management_stamp(&confirmation.stamp)?;
        if sections.version()? != confirmation.inventory {
            return Err("The section inventory changed; confirm again".into());
        }
        let stamp = confirmation.stamp;
        self.mirror.invalidate_metadata(&stamp.thread_id);
        Ok(self.request(
            &stamp.owner,
            api::move_thread_section(
                &stamp.thread_id,
                confirmation.section_id.as_deref(),
                confirmation.before_thread_id.as_deref(),
            ),
            Kind::MoveSection,
        ))
    }
    pub fn prepare_revert(
        &self,
        owner: &str,
        cwd: &Path,
        before_turn_id: &str,
        operation_id: &str,
    ) -> Result<RevertConfirmation, String> {
        let stamp = self.management_stamp(owner, cwd)?;
        if self.history_mode(owner) != Some("paginated") {
            return Err(
                "Protected revert requires paginated native history; no rollback fallback is used"
                    .into(),
            );
        }
        let history = self
            .management
            .histories
            .get(owner)
            .filter(|h| h.revision == stamp.revision)
            .ok_or("Read the complete current paginated history before preparing a revert")?;
        let index = history
            .ids
            .iter()
            .position(|id| id == before_turn_id)
            .ok_or("The excluded turn is not in the observed history")?;
        let receipt = RevertReceipt {
            thread_id: stamp.thread_id.clone(),
            operation_id: operation_id.into(),
            before_turn_id: before_turn_id.into(),
            retained_turn_ids: history.ids[..index].to_vec(),
            removed_turn_count: history.ids.len() - index,
        };
        receipt.validate()?;
        Ok(RevertConfirmation { stamp, receipt })
    }
    /// WRITE-AHEAD: persist `saved()` successfully before sending the returned
    /// request. On persistence failure complete it as Rejected without sending.
    pub fn confirm_revert(&mut self, confirmation: RevertConfirmation) -> Result<Request, String> {
        self.validate_management_stamp(&confirmation.stamp)?;
        let stamp = confirmation.stamp;
        let receipt = confirmation.receipt;
        let call = api::revert_thread(&stamp.thread_id, &receipt.before_turn_id);
        self.saved.reverts.insert(stamp.owner.clone(), receipt);
        self.invalidate_managed_history(&stamp.owner, &stamp.thread_id);
        Ok(self.request(&stamp.owner, call, Kind::Revert))
    }
    pub(super) fn invalidate_managed_history(&mut self, owner: &str, thread: &str) {
        self.mirror.invalidate_history(thread);
        self.management.histories.remove(owner);
        self.management.refresh_required.insert(owner.into());
    }
    pub(super) fn observe_managed_history(
        &mut self,
        owner: &str,
        turns: &[Value],
    ) -> Result<(), String> {
        if turns.len() > 4096 {
            return Err("Protected history exceeded its bound".into());
        }
        let mut ids = Vec::with_capacity(turns.len());
        let mut unique = HashSet::new();
        for turn in turns {
            let id = turn["id"]
                .as_str()
                .filter(|id| text(id, 256))
                .ok_or("Invalid protected turn identity")?;
            if !unique.insert(id) {
                return Err("Protected history repeated a turn".into());
            }
            // Live/unknown turns are displayable, but are not a revert checkpoint.
            if !matches!(
                turn["status"].as_str(),
                Some("completed" | "interrupted" | "failed")
            ) {
                self.management.histories.remove(owner);
                return Ok(());
            }
            ids.push(id.into());
        }
        if self
            .pending
            .values()
            .any(|r| r.owner == owner && matches!(r.kind, Kind::Revert))
        {
            return Ok(());
        }
        let matches_mirror = self
            .binding(owner)
            .and_then(|b| self.mirror.thread(&b.thread_id))
            .is_some_and(|thread| {
                thread.turns.len() == ids.len()
                    && thread
                        .turns
                        .iter()
                        .zip(&ids)
                        .all(|(t, id)| &t.id == id && !t.active())
            });
        if !matches_mirror {
            self.management.histories.remove(owner);
            return Ok(());
        }
        self.management.refresh_required.remove(owner);
        if self
            .saved
            .reverts
            .get(owner)
            .is_some_and(|r| r.retained_turn_ids == ids)
        {
            self.saved.reverts.remove(owner);
        }
        self.management.histories.insert(
            owner.into(),
            History {
                revision: self.mirror.revision(),
                ids,
            },
        );
        Ok(())
    }
    /// Explicit host confirmation after inspecting fresh native history. A
    /// nonmatching read never proves failure and never triggers automatic retry.
    pub fn resolve_revert_after_user_review(
        &mut self,
        owner: &str,
        operation_id: &str,
    ) -> Result<(), String> {
        if !self.observed(owner)
            || self
                .management
                .histories
                .get(owner)
                .is_none_or(|h| h.revision != self.mirror.revision())
            || self.pending.values().any(|r| r.owner == owner)
            || self
                .saved
                .reverts
                .get(owner)
                .is_none_or(|r| r.operation_id != operation_id)
        {
            return Err(
                "Inspect the complete current native history and its exact revert warning first"
                    .into(),
            );
        }
        self.saved.reverts.remove(owner);
        Ok(())
    }
    pub(super) fn complete_management(
        &mut self,
        request: &Request,
        value: Value,
        lifecycle_changed: bool,
    ) -> Result<Outcome, String> {
        let id = request
            .thread_id
            .as_deref()
            .ok_or("Missing management thread identity")?;
        // 0.153.4 releases the old loaded session during a successful revert.
        // Closed/notLoaded may precede its ACK; never resurrect that session.
        // Archive/delete are not evidence of the requested history replacement.
        if lifecycle_changed
            && (!matches!(request.kind, Kind::Revert)
                || self
                    .binding(&request.owner)
                    .is_none_or(|b| b.archived || b.deleted))
        {
            return Err(
                "Native lifecycle changed; read the current state before another operation".into(),
            );
        }
        match &request.kind {
            Kind::GitMetadata { cwd, git } => {
                let thread = &value["thread"];
                let metadata = NativeMetadata::read(thread)?;
                if thread["id"] != id
                    || !same_existing_directory(&metadata.cwd, cwd)
                    || metadata.git.as_ref() != Some(git)
                    || thread["gitInfo"]["originUrl"].as_str() != git.origin_url.as_deref()
                {
                    return Err("Native metadata did not match the confirmed repository".into());
                }
                self.mirror.hydrate_metadata(thread, request.revision)?;
            }
            Kind::MoveSection => {
                if value != json!({}) {
                    return Err("Unexpected section move acknowledgement; refresh metadata before another operation".into());
                }
                // ACK is not a metadata snapshot. Follow with thread/read.
            }
            Kind::Revert => {
                let thread = &value["thread"];
                let metadata = NativeMetadata::read(thread)?;
                let expected = self
                    .mirror
                    .thread(id)
                    .and_then(|t| t.metadata.as_ref())
                    .ok_or("Revert metadata is unavailable")?;
                if thread["id"] != id
                    || thread["historyMode"] != "paginated"
                    || !thread["turns"].as_array().is_some_and(Vec::is_empty)
                    || !same_existing_directory(&metadata.cwd, &expected.cwd)
                {
                    return Err(
                        "Unexpected revert result; inspect native history without replaying".into(),
                    );
                }
                for field in ["turnsBackwardsCursor", "itemsBackwardsCursor"] {
                    if value.get(field).is_none() {
                        return Err("Revert result omitted hydration cursors".into());
                    }
                    crate::thread_metadata::optional_text(&value, field, 16 * 1024)?;
                }
                self.invalidate_managed_history(&request.owner, id);
                // A preceding thread/reverted notification invalidates older
                // snapshots; the correlated ACK starts a new read boundary.
                self.mirror.hydrate(thread, self.mirror.revision())?;
                // Receipt is cleared only by a complete exact retained-prefix read.
            }
            _ => return Err("Unexpected backend management operation".into()),
        }
        Ok(Outcome::Updated {
            owner: request.owner.clone(),
        })
    }
}

#[cfg(test)]
mod tests;
