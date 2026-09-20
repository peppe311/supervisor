use super::*;

fn chatgpt() -> Value {
    json!({"account":{"type":"chatgpt","email":"fixture@example.test","planType":"pro"}})
}

fn preference(data_dir: &Path) -> Value {
    serde_json::from_slice(&fs::read(data_dir.join(FILE_NAME)).unwrap()).unwrap()
}

fn remember(data_dir: &Path) {
    let mut state = State::load(data_dir);
    state.connection.begin_connect();
    state.accept_account(data_dir, &chatgpt()).unwrap();
    assert_eq!(preference(data_dir), json!({"version":1,"reconnect":true}));
}

#[test]
fn new_profile_stays_explicit_and_saves_no_authentication_guess() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = State::load(dir.path());
    assert!(!state.take_startup_connection());
    assert!(!state.take_startup_connection());
    assert!(!dir.path().join(FILE_NAME).exists());
    state.accept_account(dir.path(), &chatgpt()).unwrap();
    assert!(!dir.path().join(FILE_NAME).exists());
}

#[test]
fn confirmed_chatgpt_restores_once_without_credentials_or_permissions() {
    let dir = tempfile::tempdir().unwrap();
    remember(dir.path());
    let mut state = State::load(dir.path());
    assert!(state.take_startup_connection());
    assert!(!state.take_startup_connection());
    assert!(state.view.account.is_none());
    assert!(!state.configuration().0.can_run());
    assert_eq!(state.access(), api::Access::ReadOnly);
    assert!(state.submissions.is_empty());
    assert!(state.reviews.is_empty());
    assert!(state.client.is_none());
    assert!(!state.view.connected);
    assert!(state.view.login.is_none());
    assert!(!state.view.requirements_loaded);
    assert_eq!(preference(dir.path()).as_object().unwrap().len(), 2);
}

#[test]
fn explicit_connect_consumes_the_startup_attempt_too() {
    let dir = tempfile::tempdir().unwrap();
    remember(dir.path());
    let mut state = State::load(dir.path());
    state.connection.begin_connect();
    assert!(!state.take_startup_connection());
}

#[test]
fn legacy_model_profile_gets_a_single_account_check_not_assumed_authentication() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("app-server-profile.json"),
        serde_json::to_vec(&AgentSelection {
            model: "fixture-model".into(),
            ..AgentSelection::default()
        })
        .unwrap(),
    )
    .unwrap();
    let mut state = State::load(dir.path());
    assert!(state.take_startup_connection());
    assert!(state.view.account.is_none());
    assert!(!dir.path().join(FILE_NAME).exists());
}

#[test]
fn signed_out_bindings_still_load_local_history_without_restoring_authentication() {
    let dir = tempfile::tempdir().unwrap();
    let mut saved = Saved::default();
    saved.bindings.insert(
        "chat:fixture".into(),
        central_agent_codex_runtime::conversations::Binding {
            thread_id: "fixture-thread".into(),
            session_id: None,
            archived: false,
            deleted: false,
            never_submitted: false,
        },
    );
    fs::write(
        dir.path().join("app-server-threads.json"),
        serde_json::to_vec(&saved).unwrap(),
    )
    .unwrap();
    let mut state = State::load(dir.path());
    assert!(state.take_startup_connection());
    assert_eq!(state.conversations.saved().bindings.len(), 1);
    assert!(state.forget_connection_for_logout(dir.path()));
    drop(state);
    let mut next = State::load(dir.path());
    assert!(next.take_startup_connection());
    assert!(!next.take_startup_connection());
    assert!(!next.connection.remember_allowed);
    assert!(next.view.account.is_none());
    assert!(next.view.login.is_none());
    assert_eq!(preference(dir.path())["reconnect"], false);
    assert_eq!(next.conversations.saved().bindings.len(), 1);
}

#[test]
fn logout_is_durable_before_rpc_and_late_account_reads_cannot_rearm_it() {
    let dir = tempfile::tempdir().unwrap();
    remember(dir.path());
    let mut state = State::load(dir.path());
    state.connection.begin_connect();
    assert!(state.forget_connection_for_logout(dir.path()));
    state.accept_account(dir.path(), &chatgpt()).unwrap();
    assert_eq!(preference(dir.path())["reconnect"], false);
    drop(state);
    let mut next = State::load(dir.path());
    assert!(!next.take_startup_connection());
    // A new explicit sign-in can remember the account again after validation.
    next.connection.begin_login();
    next.accept_account(dir.path(), &chatgpt()).unwrap();
    assert_eq!(preference(dir.path())["reconnect"], true);
}

#[test]
fn temporary_failure_or_invalid_account_does_not_forget_the_saved_connection() {
    let dir = tempfile::tempdir().unwrap();
    remember(dir.path());
    let mut state = State::load(dir.path());
    assert!(state.take_startup_connection());
    state.connection.begin_connect();
    for value in [json!({}), json!({"account":{"type":"unknown"}})] {
        assert!(state.accept_account(dir.path(), &value).is_err());
        assert_eq!(preference(dir.path())["reconnect"], true);
    }
    // Closing a process is not account/logout and doesn't alter intent.
    state.shutdown_owned_process();
    drop(state);
    assert!(State::load(dir.path()).take_startup_connection());
}

#[test]
fn definite_missing_or_unsupported_account_disables_automatic_connection() {
    for account in [
        Value::Null,
        json!({"type":"apiKey"}),
        json!({"type":"amazonBedrock","usesCodexManagedCredentials":false}),
    ] {
        let dir = tempfile::tempdir().unwrap();
        remember(dir.path());
        let mut state = State::load(dir.path());
        state.connection.begin_connect();
        state
            .accept_account(dir.path(), &json!({"account":account}))
            .unwrap();
        assert_eq!(preference(dir.path())["reconnect"], false);
    }
}

#[test]
fn invalid_preferences_are_preserved_and_do_not_restore_an_older_backup() {
    for contents in [
        "{broken".to_owned(),
        r#"{"version":2,"reconnect":true}"#.to_owned(),
        r#"{"version":1,"reconnect":true,"token":"must-not-be-used"}"#.to_owned(),
        " ".repeat(4097),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        fs::write(&path, &contents).unwrap();
        fs::write(backup_path_for(&path), r#"{"version":1,"reconnect":true}"#).unwrap();
        let mut state = State::load(dir.path());
        assert!(!state.take_startup_connection());
        assert!(state.view.errors.contains_key(ERROR_KEY));
        assert_eq!(fs::read_to_string(&path).unwrap(), contents);
    }
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        backup_path_for(&dir.path().join(FILE_NAME)),
        r#"{"version":1,"reconnect":true}"#,
    )
    .unwrap();
    let mut state = State::load(dir.path());
    assert!(!state.take_startup_connection());
    assert!(state.view.errors.contains_key(ERROR_KEY));
}

#[test]
fn second_instance_cannot_restore_or_modify_the_preference() {
    let dir = tempfile::tempdir().unwrap();
    remember(dir.path());
    let _first = State::load(dir.path());
    let mut second = State::load(dir.path());
    assert!(!second.take_startup_connection());
    assert!(!second.forget_connection_for_logout(dir.path()));
    assert!(second.view.errors.contains_key(ERROR_KEY));
    assert_eq!(preference(dir.path())["reconnect"], true);
}

#[test]
fn corrupt_bindings_do_not_trigger_automatic_runtime_start() {
    let dir = tempfile::tempdir().unwrap();
    remember(dir.path());
    fs::write(dir.path().join("app-server-threads.json"), "{broken").unwrap();
    let mut state = State::load(dir.path());
    assert!(!state.take_startup_connection());
    assert!(state.binding_store_error.is_some());
    assert_eq!(preference(dir.path())["reconnect"], true);
}

#[test]
fn failed_persistence_is_visible_and_blocks_logout_until_an_explicit_retry() {
    let dir = tempfile::tempdir().unwrap();
    let mut state = State::load(dir.path());
    state.connection.begin_connect();
    // A directory in place of the preference makes the atomic write fail.
    fs::create_dir(dir.path().join(FILE_NAME)).unwrap();
    state.accept_account(dir.path(), &chatgpt()).unwrap();
    assert!(state.view.account.as_ref().unwrap().supported);
    assert!(state.view.errors.contains_key(ERROR_KEY));
    assert!(!state.forget_connection_for_logout(dir.path()));
    assert!(!state.connection.remember_allowed);
    assert_eq!(state.connection.saved, None);
    fs::remove_dir(dir.path().join(FILE_NAME)).unwrap();
    assert!(state.forget_connection_for_logout(dir.path()));
    assert!(!state.view.errors.contains_key(ERROR_KEY));
    assert_eq!(preference(dir.path())["reconnect"], false);
}

#[test]
fn startup_and_refresh_details_do_not_request_another_login_while_checking() {
    let mut state = State::default();
    state.view.connecting = true;
    state.sync_profile();
    assert_eq!(
        state.configuration().0.phase,
        crate::provider_types::AgentProviderPhase::Checking
    );
    assert_eq!(state.configuration().0.detail, "Connecting to Codex…");
    assert!(!state.configuration().0.authenticated);
    state.view.connected = true;
    state.view.connecting = false;
    state.begin_refresh();
    state.sync_profile();
    assert_eq!(
        state.configuration().0.phase,
        crate::provider_types::AgentProviderPhase::Checking
    );
    assert!(
        state
            .configuration()
            .0
            .detail
            .starts_with("Checking the saved ChatGPT sign-in")
    );
    assert!(!state.configuration().0.can_run());
}
