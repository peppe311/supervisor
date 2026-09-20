use super::*;
use central_agent_codex_runtime::conversations::Binding;

const PARENT: &str = "00000000-0000-7000-8000-000000000001";
const CHILD: &str = "00000000-0000-7000-8000-000000000002";
const FOREIGN: &str = "00000000-0000-7000-8000-000000000003";

fn book() -> Saved {
    let mut saved = Saved::default();
    saved.bindings.insert(
        "chat:parent".into(),
        Binding {
            thread_id: PARENT.into(),
            session_id: Some("native-session".into()),
            archived: false,
            deleted: false,
            never_submitted: false,
        },
    );
    saved.delegations.insert(CHILD.into(), PARENT.into());
    saved
}

fn rollout(root: &Path, id: &str, archive: bool) -> PathBuf {
    let relative = PathBuf::from(if archive {
        "archived_sessions"
    } else {
        "sessions/2026/09/10"
    })
    .join(format!("rollout-2026-09-10T17-39-27-{id}.jsonl"));
    let path = root.join(&relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, format!("{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{id}\"}}}}\n{{\"opaqueNativeHistory\":\"preserve exactly\"}}\n")).unwrap();
    relative
}

#[test]
fn transfers_only_owned_histories_and_unopened_children_without_importing_account() {
    let source = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let parent = rollout(source.path(), PARENT, false);
    let child = rollout(source.path(), CHILD, true);
    let foreign = rollout(source.path(), FOREIGN, false);
    for name in [
        "auth.json",
        "config.toml",
        "state_5.sqlite",
        "thread_history_1.sqlite",
    ] {
        fs::write(source.path().join(name), "source only").unwrap();
    }
    let saved = book();
    let before = serde_json::to_value(&saved).unwrap();
    let home = prepare_from(data.path(), &saved, Some(source.path())).unwrap();
    for relative in [&parent, &child] {
        assert_eq!(
            fs::read(home.join(relative)).unwrap(),
            fs::read(source.path().join(relative)).unwrap()
        );
    }
    assert!(!home.join(foreign).exists());
    for name in [
        "auth.json",
        "config.toml",
        "state_5.sqlite",
        "thread_history_1.sqlite",
    ] {
        assert!(!home.join(name).exists());
        assert_eq!(
            fs::read_to_string(source.path().join(name)).unwrap(),
            "source only"
        );
    }
    assert_eq!(serde_json::to_value(saved).unwrap(), before);
    let report: Report = serde_json::from_slice(&fs::read(home.join(MARKER)).unwrap()).unwrap();
    assert_eq!(report.copied.len(), 2);
    assert!(report.missing.is_empty());
    assert_eq!(
        report.copied[PARENT].sha256,
        digest(&source.path().join(parent)).unwrap()
    );
}

#[test]
fn subsequent_connections_never_restore_deleted_history_or_old_account() {
    let source = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let relative = rollout(source.path(), PARENT, false);
    let home = prepare_from(data.path(), &book(), Some(source.path())).unwrap();
    fs::remove_file(home.join(&relative)).unwrap();
    fs::write(source.path().join("auth.json"), "must not restore").unwrap();
    fs::write(source.path().join(relative), "source has since changed").unwrap();
    assert_eq!(
        prepare_from(data.path(), &book(), Some(source.path())).unwrap(),
        home
    );
    assert!(!home.join("auth.json").exists());
    assert_eq!(
        fs::read_dir(home.join("sessions/2026/09/10"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn missing_and_deleted_bindings_are_preserved_without_recreating_conversations() {
    let source = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let parent = rollout(source.path(), PARENT, false);
    let mut saved = book();
    saved.bindings.get_mut("chat:parent").unwrap().deleted = true;
    let before = serde_json::to_value(&saved).unwrap();
    let home = prepare_from(data.path(), &saved, Some(source.path())).unwrap();
    let report: Report = serde_json::from_slice(&fs::read(home.join(MARKER)).unwrap()).unwrap();
    assert!(report.copied.is_empty());
    assert_eq!(report.missing, [CHILD]);
    assert!(!home.join(parent).exists());
    assert_eq!(serde_json::to_value(saved).unwrap(), before);
}

#[test]
fn mismatch_or_duplicate_rollouts_never_publish_a_partial_profile() {
    for duplicate in [false, true] {
        let source = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let relative = rollout(source.path(), PARENT, false);
        if duplicate {
            rollout(source.path(), PARENT, true);
        } else {
            fs::write(
                source.path().join(&relative),
                format!("{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{FOREIGN}\"}}}}\n"),
            )
            .unwrap();
        }
        let before = fs::read(source.path().join(&relative)).unwrap();
        assert!(prepare_from(data.path(), &book(), Some(source.path())).is_err());
        assert!(!data.path().join("codex").exists());
        assert_eq!(fs::read(source.path().join(relative)).unwrap(), before);
    }
}

#[test]
fn unmarked_or_corrupt_profile_never_falls_back_to_shared_state() {
    let source = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    fs::create_dir(data.path().join("codex")).unwrap();
    assert!(prepare_from(data.path(), &book(), Some(source.path())).is_err());
    fs::write(data.path().join("codex").join(MARKER), "corrupt").unwrap();
    assert!(prepare_from(data.path(), &book(), Some(source.path())).is_err());
}

#[test]
fn rejects_overlapping_profiles_and_invalid_native_ids() {
    let source = tempfile::tempdir().unwrap();
    let data = source.path().join("nested-data");
    fs::create_dir(&data).unwrap();
    assert!(prepare_from(&data, &book(), Some(source.path())).is_err());
    let data = tempfile::tempdir().unwrap();
    let mut saved = book();
    saved.bindings.get_mut("chat:parent").unwrap().thread_id = "../foreign".into();
    assert!(prepare_from(data.path(), &saved, Some(source.path())).is_err());
    assert!(!data.path().join("codex").exists());
}

#[test]
fn fresh_install_creates_separate_empty_profile_without_a_legacy_directory() {
    let data = tempfile::tempdir().unwrap();
    let home = prepare_from(data.path(), &Saved::default(), None).unwrap();
    let report: Report = serde_json::from_slice(&fs::read(home.join(MARKER)).unwrap()).unwrap();
    assert!(report.source.is_none());
    assert!(report.copied.is_empty());
    assert!(report.missing.is_empty());
}
