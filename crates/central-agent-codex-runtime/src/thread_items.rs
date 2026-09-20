//! Prepared item pagination decoder. No production transport can send this method.
use crate::thread_metadata::text;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug)]
pub struct Entry {
    pub turn_id: String,
    pub item: Value,
}

/// Native values remain Rust-side. Do not serialize this into a WebView or feed it
/// back to a model. A future host must use the existing allowlisted item renderer.
#[derive(Default)]
pub struct Pages {
    entries: Vec<Entry>,
    identities: HashSet<(String, String)>,
    cursors: HashSet<String>,
    next: Option<String>,
    turn: Option<String>,
    pages: usize,
    bytes: usize,
    finished: bool,
    failed: bool,
}

impl Pages {
    pub fn new(turn_id: Option<&str>) -> Result<Self, String> {
        if turn_id.is_some_and(|id| !text(id, 256)) {
            return Err("Invalid item turn scope".into());
        }
        Ok(Self {
            turn: turn_id.map(str::to_owned),
            ..Self::default()
        })
    }
    pub fn accept(&mut self, cursor: Option<&str>, value: Value) -> Result<bool, String> {
        if self.failed || self.finished {
            return Err("Item pagination is no longer active".into());
        }
        let result = self.accept_page(cursor, value);
        if result.is_err() {
            self.failed = true;
            self.entries.clear();
        }
        result
    }
    fn accept_page(&mut self, cursor: Option<&str>, value: Value) -> Result<bool, String> {
        if cursor != self.next.as_deref() {
            return Err("Item page has a different cursor".into());
        }
        let data = value["data"].as_array().ok_or("Missing item list")?;
        let next = crate::thread_sections::cursor(&value)?;
        self.bytes = self.bytes.saturating_add(value.to_string().len());
        self.pages += 1;
        if data.len() > 64
            || self.pages > 512
            || self.bytes > 16 * 1024 * 1024
            || self.entries.len() + data.len() > 8192
        {
            return Err("Item pagination exceeded its bound".into());
        }
        for raw in data {
            let turn = raw["turnId"]
                .as_str()
                .filter(|id| text(id, 256))
                .ok_or("Invalid item turn identity")?;
            let id = raw["item"]["id"]
                .as_str()
                .filter(|id| text(id, 256))
                .ok_or("Invalid item identity")?;
            if !raw["item"]["type"]
                .as_str()
                .is_some_and(|kind| text(kind, 128))
                || self
                    .turn
                    .as_deref()
                    .is_some_and(|expected| expected != turn)
                || !self.identities.insert((turn.into(), id.into()))
            {
                return Err("Invalid, repeated or foreign thread item".into());
            }
            self.entries.push(Entry {
                turn_id: turn.into(),
                item: raw["item"].clone(),
            });
        }
        if next
            .as_ref()
            .is_some_and(|next| !self.cursors.insert(next.clone()))
            || next.is_some() && self.pages == 512
        {
            return Err("Item pagination did not terminate".into());
        }
        self.finished = next.is_none();
        self.next = next;
        Ok(self.finished)
    }
    pub fn next_cursor(&self) -> Option<&str> {
        self.next.as_deref()
    }
    pub fn finish(self) -> Result<Vec<Entry>, String> {
        if !self.finished || self.failed {
            return Err("Item pagination is incomplete".into());
        }
        Ok(self.entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn entry(turn: &str, id: &str) -> Value {
        json!({"turnId":turn,"item":{"type":"agentMessage","id":id,"text":"Fixture"}})
    }
    #[test]
    fn prepared_item_pages_preserve_order_and_turn_scopes_without_projection() {
        let mut pages = Pages::new(Some("t1")).unwrap();
        assert!(
            !pages
                .accept(None, json!({"data":[entry("t1","i1")],"nextCursor":"next"}))
                .unwrap()
        );
        assert!(
            pages
                .accept(
                    Some("next"),
                    json!({"data":[entry("t1","i2")],"nextCursor":null})
                )
                .unwrap()
        );
        let entries = pages.finish().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].item["id"], "i2");
    }
    #[test]
    fn item_pages_reject_incomplete_foreign_oversized_or_duplicate_data() {
        assert!(Pages::default().finish().is_err());
        for value in [
            json!({"data":[]}),
            json!({"data":[entry("foreign","i1")],"nextCursor":null}),
            json!({"data":[entry("t1","i1"),entry("t1","i1")],"nextCursor":null}),
            json!({"data":vec![entry("t1","i1");65],"nextCursor":null}),
        ] {
            let mut pages = Pages::new(Some("t1")).unwrap();
            assert!(pages.accept(None, value).is_err());
            assert!(pages.finish().is_err());
        }
    }
    #[test]
    fn item_cursor_loops_and_wrong_request_cursor_poison_the_sequence() {
        let mut pages = Pages::default();
        pages
            .accept(None, json!({"data":[],"nextCursor":"next"}))
            .unwrap();
        assert!(
            pages
                .accept(Some("next"), json!({"data":[],"nextCursor":"next"}))
                .is_err()
        );
        assert!(pages.finish().is_err());
        let mut pages = Pages::default();
        assert!(
            pages
                .accept(Some("foreign"), json!({"data":[],"nextCursor":null}))
                .is_err()
        );
    }
}
