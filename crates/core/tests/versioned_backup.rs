use backup_core::backup::versioned::{
    restore::{list_versions, restore_version, RestoreMode, RestoreRequest},
    run_backup_cycle,
};
use backup_core::config::model::{
    Config, Destination, ExecutionTuning, HashingTuning, PlanningTuning, RuntimeTuning,
    WatchedKind, WatchedPath,
};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn build_cfg(source: &Path, dest: &Path, keep_versions: usize) -> Config {
    Config {
        backup_root: dest.to_path_buf(),
        interval_seconds: 1800,
        max_backups_per_file: keep_versions,
        skip_hidden: true,
        ignore_patterns: vec![],
        max_parallel_copies: 1,
        max_bytes_per_second: None,
        min_free_space_bytes: None,
        hashing: HashingTuning::default(),
        execution: ExecutionTuning::default(),
        planning: PlanningTuning::default(),
        runtime: RuntimeTuning::default(),
        safe_mode: false,
        watched: vec![WatchedPath {
            path: source.to_path_buf(),
            kind: WatchedKind::Directory,
            enabled: true,
            destination_id: "default".into(),
            max_backups_per_file: Some(keep_versions),
        }],
        destinations: vec![Destination {
            id: "default".into(),
            path: dest.to_path_buf(),
            label: None,
            max_backups_per_file: None,
        }],
    }
}

fn versions_for(cfg: &Config, source: &Path) -> Vec<String> {
    let listed =
        list_versions(cfg).expect("versioned_backup::versions_for failed to list versions");
    let (_, versions) = listed
        .into_iter()
        .find(|(p, _)| p == source)
        .expect("versioned_backup::versions_for missing watched path in list");
    versions.into_iter().map(|v| v.id).collect()
}

#[test]
fn versioned_creates_version_only_on_change() {
    let tmp = tempdir()
        .expect("versioned_backup::versioned_creates_version_only_on_change failed to create temp");
    let source = tmp.path().join("source");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&source).expect(
        "versioned_backup::versioned_creates_version_only_on_change failed to create source dir",
    );
    fs::create_dir_all(&dest)
        .expect("versioned_backup::versioned_creates_version_only_on_change failed to create dest");

    let file = source.join("a.txt");
    fs::write(&file, "v1").expect(
        "versioned_backup::versioned_creates_version_only_on_change failed to write initial file",
    );

    let cfg = build_cfg(&source, &dest, 5);

    let cycle1 = run_backup_cycle(&cfg)
        .expect("versioned_backup::versioned_creates_version_only_on_change initial cycle failed");
    assert_eq!(cycle1.versions_created, 1);
    assert_eq!(versions_for(&cfg, &source).len(), 1);

    let cycle2 = run_backup_cycle(&cfg)
        .expect("versioned_backup::versioned_creates_version_only_on_change second cycle failed");
    assert_eq!(cycle2.versions_created, 0);
    assert_eq!(versions_for(&cfg, &source).len(), 1);

    fs::write(&file, "v2")
        .expect("versioned_backup::versioned_creates_version_only_on_change failed to modify file");
    let cycle3 = run_backup_cycle(&cfg)
        .expect("versioned_backup::versioned_creates_version_only_on_change modify cycle failed");
    assert_eq!(cycle3.versions_created, 1);
    assert_eq!(versions_for(&cfg, &source).len(), 2);

    fs::remove_file(&file)
        .expect("versioned_backup::versioned_creates_version_only_on_change failed to delete file");
    let cycle4 = run_backup_cycle(&cfg)
        .expect("versioned_backup::versioned_creates_version_only_on_change delete cycle failed");
    assert_eq!(cycle4.versions_created, 1);
    assert_eq!(versions_for(&cfg, &source).len(), 3);
}

#[test]
fn versioned_enforces_retention_limit() {
    let tmp = tempdir()
        .expect("versioned_backup::versioned_enforces_retention_limit failed to create temp");
    let source = tmp.path().join("source");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&source)
        .expect("versioned_backup::versioned_enforces_retention_limit failed to create source dir");
    fs::create_dir_all(&dest)
        .expect("versioned_backup::versioned_enforces_retention_limit failed to create dest");

    let file = source.join("a.txt");
    fs::write(&file, "v1").expect(
        "versioned_backup::versioned_enforces_retention_limit failed to write initial file",
    );
    let cfg = build_cfg(&source, &dest, 2);

    run_backup_cycle(&cfg)
        .expect("versioned_backup::versioned_enforces_retention_limit cycle 1 failed");
    fs::write(&file, "v2")
        .expect("versioned_backup::versioned_enforces_retention_limit failed to modify file v2");
    run_backup_cycle(&cfg)
        .expect("versioned_backup::versioned_enforces_retention_limit cycle 2 failed");
    fs::write(&file, "v3")
        .expect("versioned_backup::versioned_enforces_retention_limit failed to modify file v3");
    run_backup_cycle(&cfg)
        .expect("versioned_backup::versioned_enforces_retention_limit cycle 3 failed");

    assert_eq!(versions_for(&cfg, &source).len(), 2);
}

#[test]
fn versioned_restore_to_directory_and_in_place() {
    let tmp = tempdir()
        .expect("versioned_backup::versioned_restore_to_directory_and_in_place failed to create");
    let source = tmp.path().join("source");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&source).expect(
        "versioned_backup::versioned_restore_to_directory_and_in_place failed to create source",
    );
    fs::create_dir_all(&dest)
        .expect("versioned_backup::versioned_restore_to_directory_and_in_place failed to create");

    let file = source.join("a.txt");
    fs::write(&file, "v1")
        .expect("versioned_backup::versioned_restore_to_directory_and_in_place failed to write v1");
    let cfg = build_cfg(&source, &dest, 5);
    run_backup_cycle(&cfg)
        .expect("versioned_backup::versioned_restore_to_directory_and_in_place cycle 1 failed");

    fs::write(&file, "v2")
        .expect("versioned_backup::versioned_restore_to_directory_and_in_place failed to write v2");
    run_backup_cycle(&cfg)
        .expect("versioned_backup::versioned_restore_to_directory_and_in_place cycle 2 failed");

    let versions = versions_for(&cfg, &source);
    let v1_id = versions
        .first()
        .cloned()
        .expect("versioned_backup::versioned_restore_to_directory_and_in_place missing v1 id");

    let out_dir = tmp.path().join("restore-out");
    let req = RestoreRequest {
        source_path: source.clone(),
        version_id: v1_id.clone(),
        mode: RestoreMode::ToDirectory,
        target_dir: Some(out_dir.clone()),
    };
    restore_version(&cfg, &req).expect(
        "versioned_backup::versioned_restore_to_directory_and_in_place restore-to-dir failed",
    );
    let restored = fs::read_to_string(out_dir.join("a.txt")).expect(
        "versioned_backup::versioned_restore_to_directory_and_in_place failed to read restored file",
    );
    assert_eq!(restored, "v1");

    let extra = source.join("extra.txt");
    fs::write(&extra, "extra").expect(
        "versioned_backup::versioned_restore_to_directory_and_in_place failed to write extra file",
    );
    let req = RestoreRequest {
        source_path: source.clone(),
        version_id: v1_id,
        mode: RestoreMode::InPlace,
        target_dir: None,
    };
    restore_version(&cfg, &req).expect(
        "versioned_backup::versioned_restore_to_directory_and_in_place in-place restore failed",
    );

    assert!(
        !extra.exists(),
        "in-place restore should remove files not present in selected version"
    );
    let restored_in_place = fs::read_to_string(source.join("a.txt")).expect(
        "versioned_backup::versioned_restore_to_directory_and_in_place failed to read restored",
    );
    assert_eq!(restored_in_place, "v1");
}
