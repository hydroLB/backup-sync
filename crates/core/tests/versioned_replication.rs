use backup_core::backup::versioned::{
    replicate_configured_stores, run_backup_cycle, Manifest, VersionIndex,
};
use backup_core::config::model::{
    Config, Destination, ExecutionTuning, HashingTuning, PlanningTuning, RuntimeTuning,
    WatchedKind, WatchedPath,
};
use backup_core::validate;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

/// Summary: locate_one_source_root orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn locate_one_source_root(dest: &Path) -> PathBuf {
    let store_root = dest.join(".backup_sync").join("v1");
    let sources_root = store_root.join("sources");
    let mut sources: Vec<_> = fs::read_dir(&sources_root)
        .expect("versioned_replication::locate_one_source_root read sources")
        .map(|e| {
            e.expect("versioned_replication::locate_one_source_root readdir")
                .path()
        })
        .collect();
    sources.sort();
    assert_eq!(
        sources.len(),
        1,
        "versioned_replication expected one source root"
    );
    sources[0].clone()
}

/// Summary: read_index orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn read_index(source_root: &Path) -> VersionIndex {
    let index_path = source_root.join("index.json");
    let raw =
        fs::read_to_string(&index_path).expect("versioned_replication::read_index read index.json");
    serde_json::from_str(&raw).expect("versioned_replication::read_index parse index.json")
}

/// Summary: read_manifest orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn read_manifest(source_root: &Path, version_id: &str) -> Manifest {
    let path = source_root
        .join("manifests")
        .join(format!("{version_id}.json"));
    let raw =
        fs::read_to_string(&path).expect("versioned_replication::read_manifest read manifest");
    serde_json::from_str(&raw).expect("versioned_replication::read_manifest parse manifest")
}

/// Summary: replication_copies_new_versions_and_blobs_to_mirror_destination orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
#[test]
fn replication_copies_new_versions_and_blobs_to_mirror_destination() {
    let tmp = tempdir().expect("versioned_replication tempdir");
    let source = tmp.path().join("source");
    let primary = tmp.path().join("primary");
    let mirror = tmp.path().join("mirror");
    fs::create_dir_all(&source).expect("versioned_replication create source");
    fs::create_dir_all(&primary).expect("versioned_replication create primary");
    fs::create_dir_all(&mirror).expect("versioned_replication create mirror");

    let file = source.join("a.txt");
    fs::write(&file, "hello").expect("versioned_replication write source file");

    let cfg = Config {
        backup_root: primary.clone(),
        interval_seconds: 1800,
        max_backups_per_file: 5,
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
            path: source.clone(),
            kind: WatchedKind::Directory,
            enabled: true,
            destination_id: "primary".into(),
            max_backups_per_file: Some(5),
        }],
        destinations: vec![
            Destination {
                id: "primary".into(),
                path: primary.clone(),
                label: Some("Primary".into()),
                max_backups_per_file: None,
                replicate_to: vec!["mirror".into()],
            },
            Destination {
                id: "mirror".into(),
                path: mirror.clone(),
                label: Some("Mirror".into()),
                max_backups_per_file: None,
                replicate_to: vec![],
            },
        ],
    };

    validate(&cfg).expect("versioned_replication validate config");
    run_backup_cycle(&cfg).expect("versioned_replication run primary backup");

    let rep = replicate_configured_stores(&cfg).expect("versioned_replication replicate");
    assert_eq!(rep.pairs_failed, 0, "replication should succeed");
    assert!(
        rep.manifests_copied >= 1,
        "expected at least one manifest copied"
    );
    assert!(rep.blobs_copied >= 1, "expected at least one blob copied");

    let mirror_source_root = locate_one_source_root(&mirror);
    let index = read_index(&mirror_source_root);
    assert_eq!(
        index.versions.len(),
        1,
        "mirror should have one version after replication"
    );
    let version_id = index.versions[0].id.clone();
    let manifest = read_manifest(&mirror_source_root, &version_id);
    let entry = manifest
        .entries
        .get("a.txt")
        .expect("versioned_replication expected a.txt entry");
    let blob_hash = entry
        .sha256
        .as_deref()
        .expect("versioned_replication expected sha256 for a.txt");

    let prefix = &blob_hash[0..2];
    let blob_path = mirror
        .join(".backup_sync")
        .join("v1")
        .join("blobs")
        .join("sha256")
        .join(prefix)
        .join(blob_hash);
    assert!(
        blob_path.exists(),
        "expected mirrored blob at {:?}",
        blob_path
    );

    assert!(
        index.source_path == source.to_string_lossy(),
        "expected mirror index to preserve source_path"
    );
}
