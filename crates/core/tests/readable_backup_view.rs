use backup_core::backup::versioned::{
    list_versions, remove_source_history_with_commit, replicate_configured_stores, restore_version,
    run_backup_cycle, version_index_path, RestoreMode, RestoreRequest,
};
use backup_core::config::model::{
    Config, Destination, ExecutionTuning, HashingTuning, PlanningTuning, RuntimeTuning,
    WatchedKind, WatchedPath,
};
use backup_core::validate;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn config(source: &Path, primary: &Path, mirror: Option<&Path>, keep: usize) -> Config {
    let mut destinations = vec![Destination {
        id: "primary".into(),
        path: primary.to_path_buf(),
        label: Some("Main storage".into()),
        max_backups_per_file: None,
        replicate_to: mirror.map(|_| vec!["mirror".into()]).unwrap_or_default(),
    }];
    if let Some(mirror) = mirror {
        destinations.push(Destination {
            id: "mirror".into(),
            path: mirror.to_path_buf(),
            label: Some("Secondary backup location".into()),
            max_backups_per_file: None,
            replicate_to: vec![],
        });
    }
    Config {
        backup_root: primary.to_path_buf(),
        interval_seconds: 1800,
        max_backups_per_file: keep,
        skip_hidden: true,
        ignore_patterns: vec![],
        max_parallel_copies: 1,
        max_bytes_per_second: None,
        min_free_space_bytes: None,
        hashing: HashingTuning::default(),
        execution: ExecutionTuning::default(),
        planning: PlanningTuning::default(),
        runtime: RuntimeTuning::default(),
        encryption: backup_core::config::model::EncryptionConfig::default(),
        compression: backup_core::config::model::CompressionConfig::default(),
        safe_mode: false,
        watched: vec![WatchedPath {
            path: source.to_path_buf(),
            kind: WatchedKind::Directory,
            enabled: true,
            destination_id: "primary".into(),
            max_backups_per_file: Some(keep),
        }],
        destinations,
    }
}

fn visible_version_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(root)
        .expect("readable_backup_view read version directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

fn recursive_file_count(root: &Path) -> usize {
    if !root.is_dir() {
        return 0;
    }
    fs::read_dir(root)
        .expect("read recursive file-count directory")
        .map(|entry| entry.expect("read recursive file-count entry").path())
        .map(|path| {
            if path.is_dir() {
                recursive_file_count(&path)
            } else if path.is_file() {
                1
            } else {
                0
            }
        })
        .sum()
}

#[test]
fn readable_view_exposes_latest_and_previous_real_files_without_trusting_them() {
    let temp = tempdir().expect("readable_backup_view tempdir");
    let source = temp.path().join("project-alpha");
    let destination = temp.path().join("backup");
    fs::create_dir_all(source.join("notes")).expect("create source");
    fs::create_dir_all(&destination).expect("create destination");
    fs::write(source.join("notes/plan.txt"), "first").expect("write v1");
    let cfg = config(&source, &destination, None, 3);
    validate(&cfg).expect("validate config");

    run_backup_cycle(&cfg).expect("backup v1");
    let readable = destination.join("project-alpha");
    assert_eq!(
        fs::read_to_string(readable.join("Latest/notes/plan.txt")).expect("read latest v1"),
        "first"
    );
    assert!(readable.join("Previous Versions").is_dir());

    fs::write(source.join("notes/plan.txt"), "second").expect("write v2");
    run_backup_cycle(&cfg).expect("backup v2");
    assert_eq!(
        fs::read_to_string(readable.join("Latest/notes/plan.txt")).expect("read latest v2"),
        "second"
    );
    let previous = visible_version_dirs(&readable.join("Previous Versions"));
    assert_eq!(previous.len(), 1);
    assert_eq!(
        fs::read_to_string(previous[0].join("notes/plan.txt")).expect("read previous v1"),
        "first"
    );

    // A readable tree is deliberately independent from the authoritative blob database.
    fs::write(
        readable.join("Latest/notes/plan.txt"),
        "tampered browse copy",
    )
    .expect("edit readable copy");
    let versions = list_versions(&cfg).expect("list versions");
    let version_id = versions[0].1.last().expect("latest version").id.clone();
    let restore_target = temp.path().join("restored");
    restore_version(
        &cfg,
        &RestoreRequest {
            source_path: source.clone(),
            version_id,
            mode: RestoreMode::ToDirectory,
            target_dir: Some(restore_target.clone()),
        },
    )
    .expect("restore authoritative version");
    assert_eq!(
        fs::read_to_string(restore_target.join("notes/plan.txt")).expect("read restored file"),
        "second"
    );
}

#[test]
fn readable_view_tracks_retention_and_preserves_unowned_folders() {
    let temp = tempdir().expect("readable_backup_view retention tempdir");
    let source = temp.path().join("portfolio");
    let destination = temp.path().join("backup");
    fs::create_dir_all(&source).expect("create source");
    fs::create_dir_all(&destination).expect("create destination");
    let cfg = config(&source, &destination, None, 2);
    validate(&cfg).expect("validate config");

    fs::write(source.join("index.txt"), "one").expect("write one");
    run_backup_cycle(&cfg).expect("backup one");
    fs::write(source.join("index.txt"), "two").expect("write two");
    run_backup_cycle(&cfg).expect("backup two");

    let previous_root = destination.join("portfolio/Previous Versions");
    let personal = previous_root.join("My notes - do not delete");
    fs::create_dir(&personal).expect("create unowned folder");
    fs::write(personal.join("note.txt"), "mine").expect("write unowned file");

    fs::write(source.join("index.txt"), "three").expect("write three");
    run_backup_cycle(&cfg).expect("backup three");
    assert_eq!(
        fs::read_to_string(destination.join("portfolio/Latest/index.txt"))
            .expect("read retained latest"),
        "three"
    );
    assert_eq!(visible_version_dirs(&previous_root).len(), 2);
    assert_eq!(
        fs::read_to_string(personal.join("note.txt")).expect("read preserved unowned file"),
        "mine"
    );
    let managed: Vec<PathBuf> = visible_version_dirs(&previous_root)
        .into_iter()
        .filter(|path| path.join(".backup-sync-version.json").is_file())
        .collect();
    assert_eq!(
        managed.len(),
        1,
        "one retained previous version plus Latest"
    );
}

#[test]
fn readable_view_is_created_in_secondary_storage_too() {
    let temp = tempdir().expect("readable_backup_view replication tempdir");
    let source = temp.path().join("client-work");
    let primary = temp.path().join("primary");
    let mirror = temp.path().join("mirror");
    fs::create_dir_all(&source).expect("create source");
    fs::create_dir_all(&primary).expect("create primary");
    fs::create_dir_all(&mirror).expect("create mirror");
    fs::write(source.join("deliverable.txt"), "ready").expect("write source");
    let cfg = config(&source, &primary, Some(&mirror), 3);
    validate(&cfg).expect("validate config");

    run_backup_cycle(&cfg).expect("backup primary");
    let summary = replicate_configured_stores(&cfg).expect("replicate");
    assert_eq!(summary.pairs_failed, 0);
    assert_eq!(
        fs::read_to_string(mirror.join("client-work/Latest/deliverable.txt"))
            .expect("read mirror browse view"),
        "ready"
    );
}

#[test]
fn removing_protection_deletes_saved_history_everywhere_but_not_original_files() {
    let temp = tempdir().expect("readable_backup_view removal tempdir");
    let source = temp.path().join("portfolio");
    let primary = temp.path().join("primary");
    let mirror = temp.path().join("mirror");
    fs::create_dir_all(&source).expect("create source");
    fs::create_dir_all(&primary).expect("create primary");
    fs::create_dir_all(&mirror).expect("create mirror");
    fs::write(source.join("work.txt"), "do not delete the original").expect("write source");
    let cfg = config(&source, &primary, Some(&mirror), 3);

    run_backup_cycle(&cfg).expect("backup primary");
    replicate_configured_stores(&cfg).expect("replicate mirror");
    assert!(version_index_path(&primary, &source).is_file());
    assert!(version_index_path(&mirror, &source).is_file());
    assert!(primary.join("portfolio/Latest/work.txt").is_file());
    assert!(mirror.join("portfolio/Latest/work.txt").is_file());

    let mut committed = false;
    remove_source_history_with_commit(&cfg, &source, || {
        committed = true;
        Ok(())
    })
    .expect("remove source history");

    assert!(committed, "config commit boundary must run after cleanup");
    assert!(!version_index_path(&primary, &source).exists());
    assert!(!version_index_path(&mirror, &source).exists());
    assert!(!primary.join("portfolio").exists());
    assert!(!mirror.join("portfolio").exists());
    assert_eq!(
        recursive_file_count(&primary.join(".backup_sync/v1/blobs")),
        0,
        "unreferenced primary blob files must be physically deleted"
    );
    assert_eq!(
        recursive_file_count(&mirror.join(".backup_sync/v1/blobs")),
        0,
        "unreferenced mirror blob files must be physically deleted"
    );
    assert_eq!(
        fs::read_to_string(source.join("work.txt")).expect("read untouched original"),
        "do not delete the original"
    );
}

#[test]
fn removing_protection_preserves_unowned_files_in_the_readable_view() {
    let temp = tempdir().expect("readable_backup_view unowned removal tempdir");
    let source = temp.path().join("portfolio");
    let destination = temp.path().join("backup");
    fs::create_dir_all(&source).expect("create source");
    fs::create_dir_all(&destination).expect("create destination");
    fs::write(source.join("work.txt"), "backed up").expect("write source");
    let cfg = config(&source, &destination, None, 3);
    run_backup_cycle(&cfg).expect("backup source");

    let personal = destination.join("portfolio/Previous Versions/My notes - keep");
    fs::create_dir_all(&personal).expect("create unowned readable content");
    fs::write(personal.join("note.txt"), "mine").expect("write unowned note");

    remove_source_history_with_commit(&cfg, &source, || Ok(()))
        .expect("remove source history while preserving unowned content");

    assert!(!version_index_path(&destination, &source).exists());
    assert!(!destination.join("portfolio/Latest").exists());
    assert_eq!(
        fs::read_to_string(personal.join("note.txt")).expect("read preserved note"),
        "mine"
    );
}

#[test]
fn disconnected_destination_blocks_removal_before_any_history_is_deleted() {
    let temp = tempdir().expect("readable_backup_view disconnected removal tempdir");
    let source = temp.path().join("portfolio");
    let primary = temp.path().join("primary");
    let missing = temp.path().join("disconnected-mirror");
    fs::create_dir_all(&source).expect("create source");
    fs::create_dir_all(&primary).expect("create primary");
    fs::write(source.join("work.txt"), "retain on failure").expect("write source");
    let mut cfg = config(&source, &primary, None, 3);
    run_backup_cycle(&cfg).expect("backup source");
    cfg.destinations.push(Destination {
        id: "mirror".into(),
        path: missing,
        label: Some("Disconnected mirror".into()),
        max_backups_per_file: None,
        replicate_to: vec![],
    });

    let mut committed = false;
    let error = remove_source_history_with_commit(&cfg, &source, || {
        committed = true;
        Ok(())
    })
    .expect_err("disconnected destination must block deletion");

    assert!(error.to_string().contains("serialize destination stores"));
    assert!(!committed);
    assert!(version_index_path(&primary, &source).is_file());
    assert!(primary.join("portfolio/Latest/work.txt").is_file());
}
