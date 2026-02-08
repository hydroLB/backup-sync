use backup_core::backup::versioned::{
    restore::{list_versions, restore_version, RestoreMode, RestoreRequest},
    run_backup_cycle,
};
use backup_core::config::model::{
    Config, Destination, ExecutionTuning, HashingTuning, PlanningTuning, RuntimeTuning,
    WatchedKind, WatchedPath,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for b in digest {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

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
        encryption: backup_core::config::model::EncryptionConfig::default(),
        compression: backup_core::config::model::CompressionConfig::default(),
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
            replicate_to: vec![],
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
fn large_deletion_pins_extra_version_beyond_retention() {
    let tmp = tempdir().expect(
        "versioned_backup::large_deletion_pins_extra_version_beyond_retention failed to create temp",
    );
    let source = tmp.path().join("source");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&source).expect(
        "versioned_backup::large_deletion_pins_extra_version_beyond_retention failed to create source dir",
    );
    fs::create_dir_all(&dest).expect(
        "versioned_backup::large_deletion_pins_extra_version_beyond_retention failed to create dest",
    );

    for i in 0..4 {
        fs::write(source.join(format!("file-{i}.txt")), "data").expect(
            "versioned_backup::large_deletion_pins_extra_version_beyond_retention failed to write seed file",
        );
    }

    let cfg = build_cfg(&source, &dest, 1);
    let cycle1 = run_backup_cycle(&cfg).expect(
        "versioned_backup::large_deletion_pins_extra_version_beyond_retention cycle 1 failed",
    );
    assert_eq!(
        cycle1.versions_created, 1,
        "expected an initial version to be created"
    );
    let baseline_version_id = versions_for(&cfg, &source)
        .first()
        .cloned()
        .expect("versioned_backup::large_deletion_pins_extra_version_beyond_retention missing baseline id");

    for i in 0..3 {
        fs::remove_file(source.join(format!("file-{i}.txt"))).expect(
            "versioned_backup::large_deletion_pins_extra_version_beyond_retention failed to delete file",
        );
    }

    let cycle2 = run_backup_cycle(&cfg).expect(
        "versioned_backup::large_deletion_pins_extra_version_beyond_retention cycle 2 failed",
    );
    assert_eq!(
        cycle2.versions_created, 1,
        "expected a new version after deletions"
    );
    assert!(
        !cycle2.safety_warnings.is_empty(),
        "expected a safety warning on large deletion"
    );
    assert!(
        cycle2.safety_warnings[0].message.contains("75%"),
        "expected a ~75% shrink warning, got: {}",
        cycle2.safety_warnings[0].message
    );

    // Keep versions=1 would normally retain only one manifest, but the pinned baseline should
    // remain as an extra safety version.
    let source_id = sha256_hex(source.to_string_lossy().as_bytes());
    let index_path = dest
        .join(".backup_sync")
        .join("v1")
        .join("sources")
        .join(source_id)
        .join("index.json");
    let raw = fs::read_to_string(&index_path).expect(
        "versioned_backup::large_deletion_pins_extra_version_beyond_retention failed to read index.json",
    );
    let index: backup_core::backup::versioned::VersionIndex = serde_json::from_str(&raw).expect(
        "versioned_backup::large_deletion_pins_extra_version_beyond_retention failed to parse index.json",
    );
    assert_eq!(
        index.safety_pending_version_id.as_deref(),
        Some(baseline_version_id.as_str()),
        "expected baseline version to be kept pending"
    );
    assert_eq!(
        index.safety_pinned_version_id.as_deref(),
        None,
        "expected pending to be promoted only after a second clean scan"
    );
    assert_eq!(
        versions_for(&cfg, &source).len(),
        2,
        "expected keep(1)+pinned(1) versions"
    );

    // A second clean scan with no further changes should promote the pending baseline to a pin.
    let cycle3 = run_backup_cycle(&cfg).expect(
        "versioned_backup::large_deletion_pins_extra_version_beyond_retention cycle 3 failed",
    );
    assert_eq!(
        cycle3.versions_created, 0,
        "expected no new version when no changes occurred"
    );
    assert!(
        cycle3.safety_warnings.is_empty(),
        "expected no additional safety warning on promotion"
    );

    let raw = fs::read_to_string(&index_path).expect(
        "versioned_backup::large_deletion_pins_extra_version_beyond_retention failed to read index.json after promotion",
    );
    let index: backup_core::backup::versioned::VersionIndex = serde_json::from_str(&raw).expect(
        "versioned_backup::large_deletion_pins_extra_version_beyond_retention failed to parse index.json after promotion",
    );
    assert_eq!(
        index.safety_pending_version_id.as_deref(),
        None,
        "expected pending baseline to be cleared after promotion"
    );
    assert_eq!(
        index.safety_pinned_version_id.as_deref(),
        Some(baseline_version_id.as_str()),
        "expected baseline version to be pinned after a second clean scan"
    );
    assert_eq!(
        versions_for(&cfg, &source).len(),
        2,
        "expected keep(1)+pinned(1) versions after promotion"
    );
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

#[test]
fn restore_preflight_missing_blob_does_not_mutate_in_place_target() {
    let tmp = tempdir().expect(
        "versioned_backup::restore_preflight_missing_blob_does_not_mutate_in_place_target failed to create",
    );
    let source = tmp.path().join("source");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&source).expect(
        "versioned_backup::restore_preflight_missing_blob_does_not_mutate_in_place_target failed to create source",
    );
    fs::create_dir_all(&dest).expect(
        "versioned_backup::restore_preflight_missing_blob_does_not_mutate_in_place_target failed to create dest",
    );

    let file = source.join("a.txt");
    fs::write(&file, "v1").expect(
        "versioned_backup::restore_preflight_missing_blob_does_not_mutate_in_place_target failed to write v1",
    );
    let cfg = build_cfg(&source, &dest, 5);
    run_backup_cycle(&cfg).expect(
        "versioned_backup::restore_preflight_missing_blob_does_not_mutate_in_place_target cycle 1 failed",
    );

    fs::write(&file, "v2").expect(
        "versioned_backup::restore_preflight_missing_blob_does_not_mutate_in_place_target failed to write v2",
    );
    let extra = source.join("extra.txt");
    fs::write(&extra, "extra").expect(
        "versioned_backup::restore_preflight_missing_blob_does_not_mutate_in_place_target failed to write extra",
    );
    run_backup_cycle(&cfg).expect(
        "versioned_backup::restore_preflight_missing_blob_does_not_mutate_in_place_target cycle 2 failed",
    );

    let versions = versions_for(&cfg, &source);
    let v1_id = versions.first().cloned().expect(
        "versioned_backup::restore_preflight_missing_blob_does_not_mutate_in_place_target missing v1 id",
    );

    let store_root = dest.join(".backup_sync").join("v1");
    let hash = sha256_hex(b"v1");
    let blob_path = store_root
        .join("blobs")
        .join("sha256")
        .join(&hash[0..2])
        .join(hash);
    fs::remove_file(&blob_path).expect(
        "versioned_backup::restore_preflight_missing_blob_does_not_mutate_in_place_target failed to delete blob",
    );

    let req = RestoreRequest {
        source_path: source.clone(),
        version_id: v1_id,
        mode: RestoreMode::InPlace,
        target_dir: None,
    };
    assert!(
        restore_version(&cfg, &req).is_err(),
        "restore should fail when a required blob is missing"
    );

    assert!(
        extra.exists(),
        "in-place restore preflight failure must not remove existing files"
    );
    let still = fs::read_to_string(&file).expect(
        "versioned_backup::restore_preflight_missing_blob_does_not_mutate_in_place_target failed to read a.txt",
    );
    assert_eq!(still, "v2");
}

#[test]
fn restore_preflight_space_check_does_not_mutate_to_directory_target() {
    let tmp = tempdir().expect(
        "versioned_backup::restore_preflight_space_check_does_not_mutate_to_directory_target failed to create",
    );
    let source = tmp.path().join("source");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&source).expect(
        "versioned_backup::restore_preflight_space_check_does_not_mutate_to_directory_target failed to create source",
    );
    fs::create_dir_all(&dest).expect(
        "versioned_backup::restore_preflight_space_check_does_not_mutate_to_directory_target failed to create dest",
    );

    let file = source.join("a.txt");
    fs::write(&file, "v1").expect(
        "versioned_backup::restore_preflight_space_check_does_not_mutate_to_directory_target failed to write v1",
    );
    let mut cfg = build_cfg(&source, &dest, 5);
    run_backup_cycle(&cfg).expect(
        "versioned_backup::restore_preflight_space_check_does_not_mutate_to_directory_target cycle 1 failed",
    );

    // Force a preflight space failure by requiring an impossible minimum.
    cfg.min_free_space_bytes = Some(u64::MAX);

    let versions = versions_for(&cfg, &source);
    let v1_id = versions.first().cloned().expect(
        "versioned_backup::restore_preflight_space_check_does_not_mutate_to_directory_target missing v1 id",
    );

    let out_dir = tmp.path().join("restore-out");
    fs::create_dir_all(&out_dir).expect(
        "versioned_backup::restore_preflight_space_check_does_not_mutate_to_directory_target failed to create out dir",
    );
    let sentinel = out_dir.join("keep.txt");
    fs::write(&sentinel, "keep").expect(
        "versioned_backup::restore_preflight_space_check_does_not_mutate_to_directory_target failed to write sentinel",
    );

    let req = RestoreRequest {
        source_path: source.clone(),
        version_id: v1_id,
        mode: RestoreMode::ToDirectory,
        target_dir: Some(out_dir.clone()),
    };
    assert!(
        restore_version(&cfg, &req).is_err(),
        "restore should fail when free space check fails"
    );

    let still = fs::read_to_string(&sentinel).expect(
        "versioned_backup::restore_preflight_space_check_does_not_mutate_to_directory_target failed to read sentinel",
    );
    assert_eq!(still, "keep");
}
