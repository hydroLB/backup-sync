use backup_core::backup::versioned::{
    restore::{list_versions, restore_version, RestoreMode, RestoreRequest},
    run_backup_cycle, scrub_versioned_store, ScrubMode,
};
use backup_core::config::model::{
    CompressionConfig, Config, Destination, EncryptionConfig, ExecutionTuning, HashingTuning,
    PlanningTuning, RuntimeTuning, WatchedKind, WatchedPath,
};
use backup_core::encryption::keyfile;
use backup_core::validate;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

/// Summary: base_cfg orchestrates this method's core behavior.
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
fn base_cfg(source: &Path, dest: &Path) -> Config {
    Config {
        backup_root: dest.to_path_buf(),
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
        encryption: EncryptionConfig::default(),
        compression: CompressionConfig::default(),
        safe_mode: false,
        watched: vec![WatchedPath {
            path: source.to_path_buf(),
            kind: WatchedKind::Directory,
            enabled: true,
            destination_id: "default".into(),
            max_backups_per_file: Some(5),
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

/// Summary: locate_one_blob orchestrates this method's core behavior.
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
fn locate_one_blob(dest: &Path) -> PathBuf {
    let store_root = dest.join(".backup_sync").join("v1");
    let sources_root = store_root.join("sources");
    let src = fs::read_dir(&sources_root)
        .expect("compression_blobs::locate_one_blob read sources")
        .next()
        .expect("compression_blobs::locate_one_blob no sources")
        .expect("compression_blobs::locate_one_blob readdir")
        .path();
    let manifests_root = src.join("manifests");
    let manifest_path = fs::read_dir(&manifests_root)
        .expect("compression_blobs::locate_one_blob read manifests")
        .next()
        .expect("compression_blobs::locate_one_blob no manifests")
        .expect("compression_blobs::locate_one_blob readdir")
        .path();
    let raw = fs::read_to_string(&manifest_path)
        .expect("compression_blobs::locate_one_blob read manifest");
    let v: serde_json::Value =
        serde_json::from_str(&raw).expect("compression_blobs::locate_one_blob parse manifest");
    let entries = v
        .get("entries")
        .and_then(|e| e.as_object())
        .expect("compression_blobs::locate_one_blob missing entries");
    for (_k, entry) in entries.iter() {
        let kind = entry
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or_default();
        if kind != "file" {
            continue;
        }
        let sha = entry
            .get("sha256")
            .and_then(|s| s.as_str())
            .expect("compression_blobs::locate_one_blob missing sha256");
        let prefix = &sha[0..2];
        return store_root
            .join("blobs")
            .join("sha256")
            .join(prefix)
            .join(sha);
    }
    panic!("compression_blobs::locate_one_blob no file entry found");
}

/// Summary: latest_version_id orchestrates this method's core behavior.
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
fn latest_version_id(cfg: &Config, source: &Path) -> String {
    let listed = list_versions(cfg).expect("compression_blobs::latest_version_id list_versions");
    let (_, versions) = listed
        .into_iter()
        .find(|(p, _)| p == source)
        .expect("compression_blobs::latest_version_id missing watched path in list");
    versions
        .last()
        .expect("compression_blobs::latest_version_id no versions")
        .id
        .clone()
}

/// Summary: compression_only_writes_compressed_blobs_and_restores orchestrates this method's core behavior.
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
fn compression_only_writes_compressed_blobs_and_restores() {
    let tmp = tempdir().expect("compression_blobs::compression_only tempdir");
    let source = tmp.path().join("source");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&source).expect("compression_blobs::compression_only create source");
    fs::create_dir_all(&dest).expect("compression_blobs::compression_only create dest");

    fs::write(source.join("a.txt"), "hello world")
        .expect("compression_blobs::compression_only write file");

    let mut cfg = base_cfg(&source, &dest);
    cfg.compression.enabled = true;
    cfg.compression.blob_chunk_bytes = 1024;
    cfg.compression.zstd_level = 3;

    validate(&cfg).expect("compression_blobs::compression_only validate");
    run_backup_cycle(&cfg).expect("compression_blobs::compression_only backup");

    let blob = locate_one_blob(&dest);
    let raw = fs::read(&blob).expect("compression_blobs::compression_only read blob");
    assert_eq!(&raw[..8], b"BSYNCCMP", "expected compressed blob magic");

    let restore_dir = tmp.path().join("restore");
    let req = RestoreRequest {
        source_path: source.clone(),
        version_id: latest_version_id(&cfg, &source),
        mode: RestoreMode::ToDirectory,
        target_dir: Some(restore_dir.clone()),
    };
    restore_version(&cfg, &req).expect("compression_blobs::compression_only restore");
    let restored = fs::read_to_string(restore_dir.join("a.txt"))
        .expect("compression_blobs::compression_only read restored file");
    assert_eq!(restored, "hello world");

    let scrub = scrub_versioned_store(&cfg, &cfg.hashing, ScrubMode::Full, 10, 10, 0)
        .expect("compression_blobs::compression_only scrub");
    assert_eq!(scrub.hash_mismatches, 0);
    assert_eq!(scrub.missing_blobs, 0);
    assert_eq!(scrub.manifests_bad, 0);
}

/// Summary: encryption_plus_compression_restores_and_scrubs orchestrates this method's core behavior.
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
fn encryption_plus_compression_restores_and_scrubs() {
    let tmp = tempdir().expect("compression_blobs::enc_plus_comp tempdir");
    let source = tmp.path().join("source");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&source).expect("compression_blobs::enc_plus_comp create source");
    fs::create_dir_all(&dest).expect("compression_blobs::enc_plus_comp create dest");

    let key_path = tmp.path().join("key_v1.bin");
    let key_info = keyfile::create_key_file(&key_path, false)
        .expect("compression_blobs::enc_plus_comp keygen");

    fs::write(source.join("a.txt"), "hello world")
        .expect("compression_blobs::enc_plus_comp write file");

    let mut cfg = base_cfg(&source, &dest);
    cfg.compression.enabled = true;
    cfg.compression.blob_chunk_bytes = 1024;
    cfg.compression.zstd_level = 3;

    cfg.encryption.enabled = true;
    cfg.encryption.key_path = Some(key_path);
    cfg.encryption.key_id = Some(key_info.key_id);
    cfg.encryption.blob_chunk_bytes = 1024;

    validate(&cfg).expect("compression_blobs::enc_plus_comp validate");
    run_backup_cycle(&cfg).expect("compression_blobs::enc_plus_comp backup");

    let blob = locate_one_blob(&dest);
    let raw = fs::read(&blob).expect("compression_blobs::enc_plus_comp read blob");
    assert_eq!(&raw[..8], b"BSYNCENC", "expected encrypted blob magic");
    assert_eq!(raw[8], 1, "expected encryption blob version 1");
    assert_eq!(raw[9], 2, "expected encryption alg 2 (zstd chunked)");

    let restore_dir = tmp.path().join("restore");
    let req = RestoreRequest {
        source_path: source.clone(),
        version_id: latest_version_id(&cfg, &source),
        mode: RestoreMode::ToDirectory,
        target_dir: Some(restore_dir.clone()),
    };
    restore_version(&cfg, &req).expect("compression_blobs::enc_plus_comp restore");
    let restored = fs::read_to_string(restore_dir.join("a.txt"))
        .expect("compression_blobs::enc_plus_comp read restored file");
    assert_eq!(restored, "hello world");

    let scrub = scrub_versioned_store(&cfg, &cfg.hashing, ScrubMode::Full, 10, 10, 0)
        .expect("compression_blobs::enc_plus_comp scrub");
    assert_eq!(scrub.hash_mismatches, 0);
    assert_eq!(scrub.missing_blobs, 0);
    assert_eq!(scrub.manifests_bad, 0);
}
