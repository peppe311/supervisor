//! Session-only observation of notifications with no projected UI update.
//! Only schema-owned names can be displayed. Unknown names are fingerprinted;
//! this API deliberately cannot receive payloads, thread IDs or credentials.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::VecDeque, sync::OnceLock};

const RECENT_METHODS: usize = 32;
const NOTIFICATIONS: &str =
    include_str!("../../../protocol/app-server/0.153.4/json/ServerNotification.json");

fn known_methods() -> &'static Vec<String> {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let schema: serde_json::Value = serde_json::from_str(NOTIFICATIONS)
            .expect("embedded official notification schema must be valid JSON");
        schema["oneOf"]
            .as_array()
            .expect("notification union")
            .iter()
            .filter_map(|branch| {
                branch["properties"]["method"]["enum"][0]
                    .as_str()
                    .map(str::to_owned)
            })
            .collect()
    })
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct Entry {
    fingerprint: String,
    method: Option<&'static str>,
    count: u32,
}

#[derive(Clone, Debug, Default, Serialize)]
pub(super) struct Diagnostics {
    total: u32,
    entries: VecDeque<Entry>,
}

impl Diagnostics {
    pub(super) fn record(&mut self, method: &str) {
        self.total = self.total.saturating_add(1);
        let fingerprint = format!("{:x}", Sha256::digest(method.as_bytes()));
        let entry = if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.fingerprint == fingerprint)
        {
            let mut entry = self.entries.remove(index).expect("existing entry");
            entry.count = entry.count.saturating_add(1);
            entry
        } else {
            Entry {
                fingerprint,
                method: known_methods()
                    .iter()
                    .map(String::as_str)
                    .find(|name| *name == method),
                count: 1,
            }
        };
        self.entries.push_front(entry);
        self.entries.truncate(RECENT_METHODS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displayed_names_match_the_generated_stable_json_contract_exactly() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("protocol/app-server/0.153.4/json/ServerNotification.json");
        let schema: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let expected: std::collections::BTreeSet<_> = schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .map(|branch| branch["properties"]["method"]["enum"][0].as_str().unwrap())
            .collect();
        assert!(!expected.is_empty());
        assert_eq!(
            known_methods()
                .iter()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            expected
        );
        assert!(
            known_methods()
                .iter()
                .any(|name| name == "thread/queue/changed")
        );
        // The TS union also contains rawResponse notifications not present in
        // the generated stable JSON contract. Do not use it as this allowlist.
        assert!(
            !known_methods()
                .iter()
                .any(|name| name == "rawResponse/completed")
        );
    }

    #[test]
    fn unknown_names_never_enter_serialized_or_debug_diagnostics() {
        let mut view = Diagnostics::default();
        let private = "future/private-token\n<script>secret</script>";
        view.record(private);
        view.record(private);
        view.record("thread/queue/changed");
        assert_eq!(view.total, 3);
        assert_eq!(view.entries[0].method, Some("thread/queue/changed"));
        assert_eq!(view.entries[1].method, None);
        assert_eq!(view.entries[1].count, 2);
        assert_eq!(
            view.entries[1].fingerprint,
            format!("{:x}", Sha256::digest(private))
        );
        for text in [serde_json::to_string(&view).unwrap(), format!("{view:?}")] {
            assert!(!text.contains("private-token"));
            assert!(!text.contains("secret"));
            assert!(!text.contains("<script>"));
        }
    }

    #[test]
    fn metadata_is_bounded_recent_and_reset_without_changing_conversations() {
        let mut host = super::super::State::default();
        let before = serde_json::to_value(host.conversations.saved()).unwrap();
        for index in 0..100 {
            host.view.diagnostics.record(&format!("future/{index}"));
        }
        host.view.diagnostics.record("future/99");
        assert_eq!(host.view.diagnostics.total, 101);
        assert_eq!(host.view.diagnostics.entries.len(), RECENT_METHODS);
        assert_eq!(host.view.diagnostics.entries[0].count, 2);
        assert_eq!(
            serde_json::to_value(host.conversations.saved()).unwrap(),
            before
        );
        assert!(!host.any_busy());
        host.view.diagnostics.total = u32::MAX;
        host.view.diagnostics.entries[0].count = u32::MAX;
        host.view.diagnostics.record("future/99");
        assert_eq!(host.view.diagnostics.entries[0].count, u32::MAX);
        assert_eq!(host.view.diagnostics.total, u32::MAX);
        host.view.diagnostics = Diagnostics::default();
        assert_eq!(host.view.diagnostics.total, 0);
        assert!(host.view.diagnostics.entries.is_empty());
    }
}
