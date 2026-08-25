use backup_core::{StateStore, StoredState};
use std::fs;

#[test]
fn persist_creates_parents_and_atomically_replaces_state() {
    let directory = tempfile::tempdir().expect("create temporary directory");
    let state_path = directory.path().join("nested").join("state.json");
    let store = StateStore::new(state_path.clone());

    let first = StoredState {
        last_run_ts: Some(1),
        ..StoredState::default()
    };
    store.persist(&first).expect("persist initial state");

    let replacement = StoredState {
        last_run_ts: Some(2),
        ..StoredState::default()
    };
    store.persist(&replacement).expect("replace existing state");

    let (loaded, _) = StateStore::load_or_default(state_path).expect("reload state");
    assert_eq!(loaded.last_run_ts, Some(2));

    let entries = fs::read_dir(directory.path().join("nested"))
        .expect("read state directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect state directory entries");
    assert_eq!(entries.len(), 1, "temporary state files must be cleaned up");
}

#[test]
fn failed_replace_does_not_leave_a_temporary_file() {
    let directory = tempfile::tempdir().expect("create temporary directory");
    let state_path = directory.path().join("state.json");
    fs::create_dir(&state_path).expect("create conflicting destination directory");

    let error = StateStore::new(state_path)
        .persist(&StoredState::default())
        .expect_err("replacing a directory with a state file must fail");
    assert!(
        error.to_string().contains("replace state file"),
        "unexpected error: {error}"
    );

    let entries = fs::read_dir(directory.path())
        .expect("read state directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect state directory entries");
    assert_eq!(entries.len(), 1, "failed writes must clean up temp files");
}
