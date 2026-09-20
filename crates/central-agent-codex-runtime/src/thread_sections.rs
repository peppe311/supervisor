//! Bounded server-owned section inventory and explicit, single-use mutations.
//! This is a backend API only. There is no IPC route.
use crate::{
    api::{self, Call},
    thread_metadata::{optional_text, text},
    transport::CallError,
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::HashSet,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_SCOPE: AtomicU64 = AtomicU64::new(1);
const MAX_SECTIONS: usize = 4096;
const MAX_PAGES: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Section {
    pub id: String,
    pub name: String,
    // Appearance is intentionally not projected until a UI owns its token mapping.
}
impl Section {
    pub fn read(value: &Value) -> Result<Self, String> {
        let id = optional_text(value, "id", 256)?.ok_or("Section identity is missing")?;
        let name = optional_text(value, "name", 160)?.ok_or("Section name is missing")?;
        Ok(Self { id, name })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Stale,
    Loading,
    Ready,
    Sending,
    Failed,
    Uncertain,
}

#[derive(Clone)]
enum Operation {
    List,
    Create(String),
    Rename(String, String),
    Delete,
}

pub struct Request {
    scope: u64,
    revision: u64,
    call: Call,
}
impl Request {
    pub fn call(&self) -> Call {
        self.call.clone()
    }
}

/// Held by the trusted host between presenting and accepting an explicit intent.
/// No Deserialize implementation; website payloads cannot create confirmations.
pub struct Confirmation {
    scope: u64,
    revision: u64,
    call: Call,
    operation: Operation,
}

pub struct Sections {
    scope: u64,
    revision: u64,
    status: Status,
    entries: Vec<Section>,
    staged: Vec<Section>,
    cursors: HashSet<String>,
    pages: usize,
    pending: Option<(u64, Operation)>,
}
impl Default for Sections {
    fn default() -> Self {
        Self {
            scope: NEXT_SCOPE.fetch_add(1, Ordering::Relaxed),
            revision: 0,
            status: Status::Stale,
            entries: vec![],
            staged: vec![],
            cursors: HashSet::new(),
            pages: 0,
            pending: None,
        }
    }
}
impl Sections {
    pub fn status(&self) -> Status {
        self.status
    }
    pub fn entries(&self) -> Option<&[Section]> {
        (self.status == Status::Ready).then_some(self.entries.as_slice())
    }
    pub fn section(&self, id: &str) -> Result<&Section, String> {
        self.entries()
            .and_then(|entries| entries.iter().find(|s| s.id == id))
            .ok_or_else(|| "Refresh the section inventory before choosing this section".into())
    }
    pub(crate) fn version(&self) -> Result<(u64, u64), String> {
        if self.status != Status::Ready {
            return Err("Refresh the section inventory first".into());
        }
        Ok((self.scope, self.revision))
    }
    pub fn disconnect(&mut self) {
        self.revision += 1;
        self.pending = None;
        self.entries.clear();
        self.staged.clear();
        self.cursors.clear();
        self.status = if matches!(self.status, Status::Sending | Status::Uncertain) {
            Status::Uncertain
        } else {
            Status::Stale
        };
    }
    fn request(&mut self, call: Call, operation: Operation) -> Request {
        self.revision += 1;
        self.pending = Some((self.revision, operation));
        Request {
            scope: self.scope,
            revision: self.revision,
            call,
        }
    }
    pub fn refresh(&mut self) -> Result<Request, String> {
        if self.pending.is_some() {
            return Err("A section request is already pending".into());
        }
        self.status = Status::Loading;
        self.staged.clear();
        self.cursors.clear();
        self.pages = 0;
        Ok(self.request(api::list_thread_sections(None), Operation::List))
    }
    pub fn prepare_create(&self, name: &str) -> Result<Confirmation, String> {
        self.prepare(
            api::create_thread_section(name),
            Operation::Create(valid_name(name)?),
        )
    }
    pub fn prepare_rename(&self, id: &str, name: &str) -> Result<Confirmation, String> {
        self.section(id)?;
        self.prepare(
            api::rename_thread_section(id, name),
            Operation::Rename(id.into(), valid_name(name)?),
        )
    }
    pub fn prepare_delete(&self, id: &str) -> Result<Confirmation, String> {
        self.section(id)?;
        self.prepare(api::delete_thread_section(id), Operation::Delete)
    }
    fn prepare(&self, call: Call, operation: Operation) -> Result<Confirmation, String> {
        self.version()?;
        Ok(Confirmation {
            scope: self.scope,
            revision: self.revision,
            call,
            operation,
        })
    }
    pub fn confirm(&mut self, intent: Confirmation) -> Result<Request, String> {
        if self.version()? != (intent.scope, intent.revision) {
            return Err("The section inventory changed; prepare a new confirmation".into());
        }
        self.status = Status::Sending;
        Ok(self.request(intent.call, intent.operation))
    }
    /// A successful mutation always refreshes the inventory. Continuations are
    /// reads only; malformed/uncertain mutation replies are never replayed.
    pub fn complete(
        &mut self,
        request: &Request,
        result: Result<Value, CallError>,
    ) -> Result<Option<Request>, String> {
        if request.scope != self.scope
            || self
                .pending
                .as_ref()
                .is_none_or(|(revision, _)| *revision != request.revision)
        {
            return Err("Stale section response".into());
        }
        let (_, operation) = self.pending.take().unwrap();
        let mutation = !matches!(operation, Operation::List);
        let value = match result {
            Ok(value) => value,
            Err(error) => {
                self.status = if mutation
                    && matches!(
                        error,
                        CallError::Disconnected {
                            delivery_unknown: true,
                            ..
                        }
                    ) {
                    Status::Uncertain
                } else {
                    Status::Failed
                };
                self.staged.clear();
                return Err("Section request failed; refresh before another operation".into());
            }
        };
        let outcome = self.accept(operation, value);
        if outcome.is_err() {
            self.status = if mutation {
                Status::Uncertain
            } else {
                Status::Failed
            };
            self.staged.clear();
        }
        outcome
    }
    fn accept(&mut self, operation: Operation, value: Value) -> Result<Option<Request>, String> {
        match operation {
            Operation::List => {
                let data = value["data"].as_array().ok_or("Missing section page")?;
                let next = cursor(&value)?;
                self.pages += 1;
                if data.len() > 64
                    || self.pages > MAX_PAGES
                    || self.staged.len() + data.len() > MAX_SECTIONS
                {
                    return Err("Section inventory exceeded its bound".into());
                }
                for raw in data {
                    let section = Section::read(raw)?;
                    if self.staged.iter().any(|s| s.id == section.id) {
                        return Err("Section pagination repeated an identity".into());
                    }
                    self.staged.push(section);
                }
                if let Some(next) = next {
                    if !self.cursors.insert(next.clone()) || self.pages == MAX_PAGES {
                        return Err("Section pagination did not terminate".into());
                    }
                    Ok(Some(self.request(
                        api::list_thread_sections(Some(&next)),
                        Operation::List,
                    )))
                } else {
                    self.entries = std::mem::take(&mut self.staged);
                    self.status = Status::Ready;
                    Ok(None)
                }
            }
            Operation::Create(name) => {
                let created = Section::read(&value["section"])?;
                if created.name != name || self.entries.iter().any(|s| s.id == created.id) {
                    return Err("Section creation did not return the requested new section".into());
                }
                self.refresh().map(Some)
            }
            Operation::Rename(id, name) => {
                let updated = Section::read(&value["section"])?;
                if updated.id != id || updated.name != name {
                    return Err("Section rename returned a different section".into());
                }
                self.refresh().map(Some)
            }
            Operation::Delete => {
                if value != serde_json::json!({}) {
                    return Err("Unexpected section deletion result".into());
                }
                self.refresh().map(Some)
            }
        }
    }
}
fn valid_name(name: &str) -> Result<String, String> {
    if !text(name, 160) {
        return Err("Section names must contain 1–160 bytes of visible text".into());
    }
    Ok(name.into())
}
pub(crate) fn cursor(value: &Value) -> Result<Option<String>, String> {
    if value.get("nextCursor").is_none() {
        return Err("Missing pagination cursor".into());
    }
    optional_text(value, "nextCursor", 16 * 1024)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn ready() -> Sections {
        let mut sections = Sections::default();
        let request = sections.refresh().unwrap();
        sections
            .complete(
                &request,
                Ok(json!({"data":[{"id":"s1","name":"One","appearance":null}],"nextCursor":null})),
            )
            .unwrap();
        sections
    }
    #[test]
    fn inventory_is_atomic_and_confirmations_are_scoped_and_single_use() {
        let mut sections = Sections::default();
        let first = sections.refresh().unwrap();
        let next = sections
            .complete(
                &first,
                Ok(json!({"data":[{"id":"s1","name":"One"}],"nextCursor":"next"})),
            )
            .unwrap()
            .unwrap();
        assert!(sections.entries().is_none());
        assert!(sections.prepare_delete("s1").is_err());
        sections
            .complete(&next, Ok(json!({"data":[],"nextCursor":null})))
            .unwrap();
        assert_eq!(sections.entries().unwrap().len(), 1);
        assert!(sections.complete(&first, Ok(json!({}))).is_err());
        let foreign = ready().prepare_delete("s1").unwrap();
        assert!(sections.confirm(foreign).is_err());
        let old = sections.prepare_rename("s1", "New").unwrap();
        sections.disconnect();
        assert!(sections.confirm(old).is_err());
    }
    #[test]
    fn section_mutation_acks_refresh_reads_and_never_replay_uncertain_writes() {
        let mut sections = ready();
        let intent = sections.prepare_create("Two").unwrap();
        let request = sections.confirm(intent).unwrap();
        let next = sections
            .complete(&request, Ok(json!({"section":{"id":"s2","name":"Two"}})))
            .unwrap()
            .unwrap();
        assert_eq!(next.call().method, "threadSection/list");
        sections
            .complete(
                &next,
                Ok(json!({"data":[{"id":"s2","name":"Two"}],"nextCursor":null})),
            )
            .unwrap();
        let intent = sections.prepare_delete("s2").unwrap();
        let request = sections.confirm(intent).unwrap();
        assert!(
            sections
                .complete(
                    &request,
                    Err(CallError::Disconnected {
                        reason: "fixture".into(),
                        delivery_unknown: true
                    })
                )
                .is_err()
        );
        assert_eq!(sections.status(), Status::Uncertain);
        assert!(sections.entries().is_none());
        assert!(sections.prepare_create("Two").is_err());
        assert_eq!(
            sections.refresh().unwrap().call().method,
            "threadSection/list"
        );
    }
    #[test]
    fn section_pages_fail_closed_on_duplicates_loops_and_missing_cursor() {
        for value in [
            json!({"data":[]}),
            json!({"data":[{"id":"same","name":"A"},{"id":"same","name":"B"}],"nextCursor":null}),
            json!({"data":[{"id":"x","name":"\n"}],"nextCursor":null}),
        ] {
            let mut sections = Sections::default();
            let request = sections.refresh().unwrap();
            assert!(sections.complete(&request, Ok(value)).is_err());
            assert!(sections.entries().is_none());
        }
        let mut sections = Sections::default();
        let request = sections.refresh().unwrap();
        let next = sections
            .complete(&request, Ok(json!({"data":[],"nextCursor":"loop"})))
            .unwrap()
            .unwrap();
        assert!(
            sections
                .complete(&next, Ok(json!({"data":[],"nextCursor":"loop"})))
                .is_err()
        );
    }
    #[test]
    fn rename_preserves_native_appearance_and_foreign_ack_is_uncertain() {
        let mut sections = ready();
        let confirmation = sections.prepare_rename("s1", "Renamed").unwrap();
        let request = sections.confirm(confirmation).unwrap();
        assert!(request.call().params.get("appearance").is_none());
        assert!(
            sections
                .complete(
                    &request,
                    Ok(json!({"section":{"id":"other","name":"Renamed"}}))
                )
                .is_err()
        );
        assert_eq!(sections.status(), Status::Uncertain);
    }
}
