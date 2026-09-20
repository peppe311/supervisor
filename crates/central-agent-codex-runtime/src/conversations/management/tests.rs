use super::*;

fn metadata(cwd: &Path) -> Value {
    json!({"id":"thread-p3", "cwd":cwd, "historyMode":"paginated", "status":{"type":"idle"}, "turns":[], "gitInfo":null, "section":null})
}
fn turns(ids: &[&str]) -> Vec<Value> {
    ids.iter()
        .map(|id| json!({"id":id,"status":"completed","items":[]}))
        .collect()
}
fn book(cwd: &Path) -> Conversations {
    let mut book = Conversations::default();
    let open = book
        .open("chat:p3", cwd, &Profile::default(), Access::ReadOnly)
        .unwrap();
    book.complete(&open, Ok(json!({"thread":metadata(cwd)})))
        .unwrap();
    hydrate(&mut book, &["one", "two", "three"]);
    book
}
fn hydrate(book: &mut Conversations, ids: &[&str]) {
    let revision = book.mirror.revision();
    book.hydrate_paginated_history("chat:p3", "thread-p3", &turns(ids), revision)
        .unwrap();
}
fn revert(book: &mut Conversations, cwd: &Path) -> Request {
    let intent = book
        .prepare_revert("chat:p3", cwd, "two", "revert-test-1")
        .unwrap();
    assert_eq!(intent.removed_turn_count(), 2);
    assert_eq!(intent.retained_turn_count(), 1);
    book.confirm_revert(intent).unwrap()
}
fn ack(cwd: &Path) -> Value {
    json!({"thread":metadata(cwd), "turnsBackwardsCursor":"one", "itemsBackwardsCursor":null})
}

#[test]
fn revert_receipt_blocks_until_exact_retained_history_and_rejects_late_pages() {
    let directory = tempfile::tempdir().unwrap();
    let mut book = book(directory.path());
    let old_revision = book.mirror.revision();
    let request = revert(&mut book, directory.path());
    assert_eq!(
        request.call.params,
        json!({"threadId":"thread-p3","beforeTurnId":"two"})
    );
    assert!(request.requests_history_hydration() && request.omits_turns());
    assert!(book.saved().reverts.contains_key("chat:p3"));
    assert!(book.busy("chat:p3"));
    assert!(
        book.hydrate_paginated_history(
            "chat:p3",
            "thread-p3",
            &turns(&["one", "two", "three"]),
            old_revision
        )
        .is_err()
    );
    book.notification("thread/reverted", &json!({"threadId":"thread-p3"}))
        .unwrap();
    book.complete(&request, Ok(ack(directory.path()))).unwrap();
    assert!(book.saved().reverts.contains_key("chat:p3"));
    hydrate(&mut book, &["one"]);
    assert!(book.saved().reverts.is_empty());
    assert!(book.ready_for_turn("chat:p3"));
    assert_eq!(book.mirror.thread("thread-p3").unwrap().turns.len(), 1);
    assert!(book.complete(&request, Ok(ack(directory.path()))).is_err());
}

#[test]
fn uncertain_revert_survives_restart_and_never_replays_or_infers_failure() {
    let directory = tempfile::tempdir().unwrap();
    let mut book = book(directory.path());
    let request = revert(&mut book, directory.path());
    assert!(
        book.complete(
            &request,
            Err(CallError::Disconnected {
                reason: "fixture".into(),
                delivery_unknown: true
            })
        )
        .is_err()
    );
    let encoded = serde_json::to_string(book.saved()).unwrap();
    assert!(!encoded.contains("cwd") && !encoded.contains("items") && !encoded.contains("gitInfo"));
    let mut restored = Conversations::restore(serde_json::from_str(&encoded).unwrap()).unwrap();
    assert!(
        restored
            .open(
                "chat:p3",
                directory.path(),
                &Profile::default(),
                Access::ReadOnly
            )
            .is_err()
    );
    let read = restored.action("chat:p3", Action::Read).unwrap();
    restored
        .complete(&read, Ok(json!({"thread":metadata(directory.path())})))
        .unwrap();
    hydrate(&mut restored, &["one", "two", "three"]);
    assert!(restored.saved().reverts.contains_key("chat:p3"));
    assert!(
        restored
            .resolve_revert_after_user_review("chat:p3", "wrong-warning")
            .is_err()
    );
    restored
        .resolve_revert_after_user_review("chat:p3", "revert-test-1")
        .unwrap();
    assert!(restored.saved().reverts.is_empty());
    assert!(restored.pending.is_empty());
}

#[test]
fn restart_can_reconcile_success_without_acknowledgement() {
    let directory = tempfile::tempdir().unwrap();
    let mut book = book(directory.path());
    revert(&mut book, directory.path());
    let mut restored = Conversations::restore(book.saved().clone()).unwrap();
    let read = restored.action("chat:p3", Action::Read).unwrap();
    restored
        .complete(&read, Ok(json!({"thread":metadata(directory.path())})))
        .unwrap();
    hydrate(&mut restored, &["one"]);
    assert!(restored.saved().reverts.is_empty());
    assert!(restored.pending.is_empty());
}

#[test]
fn revert_rejects_unobserved_foreign_busy_legacy_and_changed_confirmations() {
    let directory = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let mut book = book(directory.path());
    assert!(
        book.prepare_revert("chat:other", directory.path(), "two", "x")
            .is_err()
    );
    assert!(
        book.prepare_revert("chat:p3", elsewhere.path(), "two", "x")
            .is_err()
    );
    assert!(
        book.prepare_revert("chat:p3", directory.path(), "missing", "x")
            .is_err()
    );
    assert!(
        book.prepare_revert("chat:p3", directory.path(), "two", "")
            .is_err()
    );
    let confirmation = book
        .prepare_revert("chat:p3", directory.path(), "two", "x")
        .unwrap();
    book.notification(
        "thread/status/changed",
        &json!({"threadId":"thread-p3","status":{"type":"idle"}}),
    )
    .unwrap();
    assert!(book.confirm_revert(confirmation).is_err());
    hydrate(&mut book, &["one", "two", "three"]);
    let confirmation = book
        .prepare_revert("chat:p3", directory.path(), "two", "x")
        .unwrap();
    book.disconnect();
    assert!(book.confirm_revert(confirmation).is_err());
    assert!(book.saved().reverts.is_empty());
}

#[test]
fn no_full_history_or_concurrent_turn_cannot_be_a_revert_checkpoint() {
    let directory = tempfile::tempdir().unwrap();
    let mut book = book(directory.path());
    let revision = book.mirror.revision();
    book.notification(
        "turn/started",
        &json!({"threadId":"thread-p3","turn":{"id":"four","status":"inProgress","items":[]}}),
    )
    .unwrap();
    book.hydrate_paginated_history(
        "chat:p3",
        "thread-p3",
        &turns(&["one", "two", "three"]),
        revision,
    )
    .unwrap();
    assert!(
        book.prepare_revert("chat:p3", directory.path(), "two", "x")
            .is_err()
    );
    assert!(!book.management.histories.contains_key("chat:p3"));
}

#[test]
fn definite_rejection_clears_receipt_but_still_requires_fresh_history() {
    let directory = tempfile::tempdir().unwrap();
    let mut book = book(directory.path());
    let request = revert(&mut book, directory.path());
    book.complete(
        &request,
        Err(CallError::Rejected("write-ahead save failed".into())),
    )
    .unwrap_err();
    assert!(book.saved().reverts.is_empty());
    assert!(book.busy("chat:p3"));
    hydrate(&mut book, &["one", "two", "three"]);
    assert!(book.ready_for_turn("chat:p3"));
}

#[test]
fn malformed_or_foreign_revert_ack_keeps_receipt_and_cannot_restore_old_history() {
    let directory = tempfile::tempdir().unwrap();
    for invalid in [json!({}), json!({"thread":{"id":"foreign"}})] {
        let mut book = book(directory.path());
        let request = revert(&mut book, directory.path());
        assert!(book.complete(&request, Ok(invalid)).is_err());
        assert!(book.saved().reverts.contains_key("chat:p3"));
        assert!(book.mirror.thread("thread-p3").unwrap().turns.is_empty());
    }
}

#[test]
fn receipts_validate_and_old_binding_files_still_restore() {
    let old = json!({"version":1,"bindings":{},"unresolved":{}});
    assert!(Conversations::restore(serde_json::from_value(old).unwrap()).is_ok());
    let directory = tempfile::tempdir().unwrap();
    let mut book = book(directory.path());
    revert(&mut book, directory.path());
    let mut invalid = book.saved().clone();
    invalid
        .reverts
        .get_mut("chat:p3")
        .unwrap()
        .retained_turn_ids
        .push("two".into());
    assert!(Conversations::restore(invalid).is_err());
    book.notification("thread/deleted", &json!({"threadId":"thread-p3"}))
        .unwrap();
    assert!(book.saved().reverts.is_empty());
    book.saved().validate().unwrap();
}

#[test]
fn git_confirmation_revalidates_worktree_and_updates_only_backend_metadata() {
    let directory = tempfile::tempdir().unwrap();
    let mut command = std::process::Command::new("git");
    crate::runtime::hide_window(&mut command);
    assert!(
        command
            .current_dir(directory.path())
            .args(["init", "--quiet", "-b", "p3-fixture"])
            .output()
            .unwrap()
            .status
            .success()
    );
    let mut book = book(directory.path());
    let intent = book
        .prepare_git_metadata("chat:p3", directory.path())
        .unwrap();
    assert_eq!(intent.metadata().branch.as_deref(), Some("p3-fixture"));
    let request = book.confirm_git_metadata(intent).unwrap();
    assert_eq!(request.call.method, "thread/metadata/update");
    assert_eq!(
        request.call.params["gitInfo"],
        json!({"sha":null,"branch":"p3-fixture","originUrl":null})
    );
    let mut updated = metadata(directory.path());
    updated["gitInfo"] = request.call.params["gitInfo"].clone();
    book.complete(&request, Ok(json!({"thread":updated})))
        .unwrap();
    assert_eq!(
        book.native_metadata("chat:p3")
            .unwrap()
            .git
            .as_ref()
            .unwrap()
            .branch
            .as_deref(),
        Some("p3-fixture")
    );
    let projected = serde_json::to_value(book.mirror.thread("thread-p3").unwrap()).unwrap();
    assert!(projected.get("metadata").is_none() && projected.get("gitInfo").is_none());
    let intent = book
        .prepare_git_metadata("chat:p3", directory.path())
        .unwrap();
    let mut command = std::process::Command::new("git");
    crate::runtime::hide_window(&mut command);
    assert!(
        command
            .current_dir(directory.path())
            .args(["symbolic-ref", "HEAD", "refs/heads/changed"])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(book.confirm_git_metadata(intent).is_err());
    assert!(book.pending.is_empty());
}

#[test]
fn section_moves_require_current_inventory_and_owned_matching_anchor() {
    let directory = tempfile::tempdir().unwrap();
    let mut book = book(directory.path());
    let mut sections = Sections::default();
    assert!(
        book.prepare_section_move("chat:p3", directory.path(), &sections, None, None)
            .is_err()
    );
    let request = sections.refresh().unwrap();
    sections
        .complete(
            &request,
            Ok(json!({"data":[{"id":"s1","name":"Section"}],"nextCursor":null})),
        )
        .unwrap();
    assert!(
        book.prepare_section_move(
            "chat:p3",
            directory.path(),
            &sections,
            Some("missing"),
            None
        )
        .is_err()
    );
    assert!(
        book.prepare_section_move(
            "chat:p3",
            directory.path(),
            &sections,
            Some("s1"),
            Some("chat:p3")
        )
        .is_err()
    );
    assert!(
        book.prepare_section_move(
            "chat:p3",
            directory.path(),
            &sections,
            Some("s1"),
            Some("chat:foreign")
        )
        .is_err()
    );
    let intent = book
        .prepare_section_move("chat:p3", directory.path(), &sections, Some("s1"), None)
        .unwrap();
    let request = book.confirm_section_move(intent, &sections).unwrap();
    assert_eq!(
        request.call.params,
        json!({"threadId":"thread-p3","sectionId":"s1","beforeThreadId":null})
    );
    book.complete(&request, Ok(json!({}))).unwrap();
    assert!(book.native_metadata("chat:p3").is_none());
    let read = book.action("chat:p3", Action::Read).unwrap();
    let mut moved = metadata(directory.path());
    moved["section"] = json!({"id":"s1","name":"Section"});
    book.complete(&read, Ok(json!({"thread":moved}))).unwrap();
    assert_eq!(
        book.native_metadata("chat:p3")
            .unwrap()
            .section
            .as_ref()
            .unwrap()
            .id,
        "s1"
    );
    let intent = book
        .prepare_section_move("chat:p3", directory.path(), &sections, None, None)
        .unwrap();
    sections.disconnect();
    assert!(book.confirm_section_move(intent, &sections).is_err());
}

#[test]
fn protected_confirmation_cannot_cross_instances_or_legacy_archive_boundaries() {
    let directory = tempfile::tempdir().unwrap();
    let source = book(directory.path());
    let intent = source
        .prepare_revert("chat:p3", directory.path(), "two", "x")
        .unwrap();
    let mut foreign = book(directory.path());
    assert!(foreign.confirm_revert(intent).is_err());
    let read = foreign.action("chat:p3", Action::Read).unwrap();
    let mut legacy = metadata(directory.path());
    legacy["historyMode"] = json!("legacy");
    foreign
        .complete(&read, Ok(json!({"thread":legacy})))
        .unwrap();
    assert!(
        foreign
            .prepare_revert("chat:p3", directory.path(), "two", "x")
            .is_err()
    );
    foreign
        .notification("thread/archived", &json!({"threadId":"thread-p3"}))
        .unwrap();
    assert!(
        foreign
            .prepare_revert("chat:p3", directory.path(), "two", "x")
            .is_err()
    );
}

#[test]
fn native_session_release_before_revert_ack_is_valid_but_does_not_resume_it() {
    let directory = tempfile::tempdir().unwrap();
    let mut book = book(directory.path());
    let request = revert(&mut book, directory.path());
    book.notification("thread/closed", &json!({"threadId":"thread-p3"}))
        .unwrap();
    book.notification("thread/reverted", &json!({"threadId":"thread-p3"}))
        .unwrap();
    book.complete(&request, Ok(ack(directory.path()))).unwrap();
    hydrate(&mut book, &["one"]);
    assert!(!book.busy("chat:p3"));
    assert!(!book.is_thread_loaded("chat:p3"));
    assert!(!book.ready_for_turn("chat:p3"));
}

#[test]
fn internal_rpc_error_is_not_proof_that_destructive_history_was_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let mut book = book(directory.path());
    let request = revert(&mut book, directory.path());
    let error = crate::wire::RpcError {
        code: -32603,
        message: "Fixture storage error".into(),
        data: None,
    };
    assert!(book.complete(&request, Err(CallError::Rpc(error))).is_err());
    assert!(book.saved().reverts.contains_key("chat:p3"));
    hydrate(&mut book, &["one"]);
    assert!(book.saved().reverts.is_empty());
}

#[test]
fn first_turn_revert_reconciles_an_empty_prefix_and_external_revert_blocks_old_reads() {
    let directory = tempfile::tempdir().unwrap();
    let mut book = book(directory.path());
    let intent = book
        .prepare_revert("chat:p3", directory.path(), "one", "empty-prefix")
        .unwrap();
    assert_eq!(intent.retained_turn_count(), 0);
    let request = book.confirm_revert(intent).unwrap();
    book.complete(&request, Ok(ack(directory.path()))).unwrap();
    hydrate(&mut book, &[]);
    assert!(book.saved().reverts.is_empty());
    let read = book.action("chat:p3", Action::Read).unwrap();
    book.notification("thread/reverted", &json!({"threadId":"thread-p3"}))
        .unwrap();
    assert!(
        book.complete(&read, Ok(json!({"thread":metadata(directory.path())})))
            .is_err()
    );
    assert!(book.busy("chat:p3"));
}
