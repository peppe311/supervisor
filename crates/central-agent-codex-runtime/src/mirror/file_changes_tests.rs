use super::*;

fn samples() -> Vec<Value> {
    serde_json::from_str(include_str!("../../tests/native-patch-updates.json")).unwrap()
}

#[test]
fn native_patch_updates_replace_snapshots_and_preserve_owner_and_item_identity() {
    let mut mirror = Mirror::default();
    let values = samples();
    let mut sibling = values[0].clone();
    sibling["threadId"] = json!("patch-graph");
    mirror
        .notify("item/fileChange/patchUpdated", &sibling)
        .unwrap();
    mirror.notify("item/started", &json!({"threadId":"patch-main","turnId":"turn","item":{"id":"patch","type":"fileChange","status":"inProgress","changes":[]}})).unwrap();
    let read_revision = mirror.revision();
    for sample in &values {
        assert!(
            mirror
                .notify("item/fileChange/patchUpdated", sample)
                .unwrap()
        );
        let turn = &mirror.thread("patch-main").unwrap().turns[0];
        assert_eq!(turn.items.len(), 1);
        assert!(!turn.items[0].completed);
        assert_eq!(turn.items[0].value["status"], "inProgress");
        assert_eq!(turn.items[0].value["changes"], sample["changes"]);
    }
    mirror.hydrate(&json!({"id":"patch-main","turns":[{"id":"turn","status":"inProgress","items":[{"id":"patch","type":"fileChange","status":"inProgress","changes":values[0]["changes"]}]}]}), read_revision).unwrap();
    assert_eq!(
        mirror.thread("patch-main").unwrap().turns[0].items[0].value["changes"],
        json!([])
    );
    assert_eq!(
        mirror.thread("patch-graph").unwrap().turns[0].items[0].value["changes"],
        values[0]["changes"]
    );
}

#[test]
fn native_patch_completion_is_authoritative_and_late_updates_cannot_change_it() {
    for status in ["completed", "declined", "failed"] {
        let mut mirror = Mirror::default();
        let values = samples();
        mirror
            .notify("item/fileChange/patchUpdated", &values[0])
            .unwrap();
        mirror.notify("item/completed", &json!({"threadId":"patch-main","turnId":"turn","item":{"id":"patch","type":"fileChange","status":status,"changes":values[1]["changes"]}})).unwrap();
        mirror
            .notify("item/fileChange/patchUpdated", &values[2])
            .unwrap();
        let item = &mirror.thread("patch-main").unwrap().turns[0].items[0];
        assert!(item.completed);
        assert_eq!(item.value["status"], status);
        assert_eq!(item.value["changes"], values[1]["changes"]);
    }
}

#[test]
fn native_patch_malformed_updates_and_wrong_item_kinds_do_not_replace_content() {
    let mut mirror = Mirror::default();
    let sample = samples().remove(0);
    for changes in [Value::Null, json!({}), json!([{"path":"x"}])] {
        let mut invalid = sample.clone();
        invalid["changes"] = changes;
        assert!(
            mirror
                .notify("item/fileChange/patchUpdated", &invalid)
                .is_err()
        );
        assert!(mirror.thread("patch-main").is_none());
    }
    mirror
        .notify(
            "item/agentMessage/delta",
            &json!({"threadId":"patch-main","turnId":"turn","itemId":"patch","delta":"Original"}),
        )
        .unwrap();
    assert!(
        mirror
            .notify("item/fileChange/patchUpdated", &sample)
            .is_err()
    );
    let item = &mirror.thread("patch-main").unwrap().turns[0].items[0];
    assert_eq!(item.value["text"], "Original");
    assert!(item.value.get("changes").is_none());
}
