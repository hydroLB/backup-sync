use backup_core::backup::versioned::{
    restore::{list_versions, restore_version, RestoreMode, RestoreRequest},
    run_backup_cycle, scrub_versioned_store, ScrubMode,
};
use backup_core::config::model::{
    Config, Destination, EncryptionConfig, ExecutionTuning, HashingTuning, PlanningTuning,
    RuntimeTuning, WatchedKind, WatchedPath,
};
use backup_core::encryption::keyfile;
use backup_core::validate;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn build_cfg(source: &Path, dest: &Path, key_path: &Path, key_id: &str) -> Config {
    let encryption = EncryptionConfig {
        enabled: true,
        key_path: Some(key_path.to_path_buf()),
        key_id: Some(key_id.to_string()),
        blob_chunk_bytes: 1024,
    };

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
        encryption,
        compression: backup_core::config::model::CompressionConfig::default(),
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

fn locate_one_blob(dest: &Path) -> PathBuf {
    let store_root = dest.join(".backup_sync").join("v1");
    let sources_root = store_root.join("sources");
    let src = fs::read_dir(&sources_root)
        .expect("encryption_blobs::locate_one_blob read sources")
        .next()
        .expect("encryption_blobs::locate_one_blob no sources")
        .expect("encryption_blobs::locate_one_blob readdir")
        .path();
    let manifests_root = src.join("manifests");
    let manifest_path = fs::read_dir(&manifests_root)
        .expect("encryption_blobs::locate_one_blob read manifests")
        .next()
        .expect("encryption_blobs::locate_one_blob no manifests")
        .expect("encryption_blobs::locate_one_blob readdir")
        .path();
    let raw = fs::read_to_string(&manifest_path)
        .expect("encryption_blobs::locate_one_blob read manifest");
    let v: serde_json::Value =
        serde_json::from_str(&raw).expect("encryption_blobs::locate_one_blob parse manifest");
    let entries = v
        .get("entries")
        .and_then(|e| e.as_object())
        .expect("encryption_blobs::locate_one_blob missing entries");
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
            .expect("encryption_blobs::locate_one_blob missing sha256");
        let prefix = &sha[0..2];
        return store_root
            .join("blobs")
            .join("sha256")
            .join(prefix)
            .join(sha);
    }
    panic!("encryption_blobs::locate_one_blob no file entry found");
}

#[test]
fn encrypted_blobs_restore_and_scrub() {
    let tmp = tempdir().expect("encryption_blobs::encrypted_blobs_restore_and_scrub tempdir");
    let source = tmp.path().join("source");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&source)
        .expect("encryption_blobs::encrypted_blobs_restore_and_scrub create source");
    fs::create_dir_all(&dest)
        .expect("encryption_blobs::encrypted_blobs_restore_and_scrub create dest");

    let key_path = tmp.path().join("key_v1.bin");
    let key_info = keyfile::create_key_file(&key_path, false)
        .expect("encryption_blobs::encrypted_blobs_restore_and_scrub keygen");

    let file = source.join("a.txt");
    fs::write(&file, "hello world")
        .expect("encryption_blobs::encrypted_blobs_restore_and_scrub write file");
    let cfg = build_cfg(&source, &dest, &key_path, &key_info.key_id);
    validate(&cfg).expect("encryption_blobs::encrypted_blobs_restore_and_scrub validate");
    run_backup_cycle(&cfg).expect("encryption_blobs::encrypted_blobs_restore_and_scrub backup");

    let blob = locate_one_blob(&dest);
    let mut prefix = [0u8; 8];
    let raw =
        fs::read(&blob).expect("encryption_blobs::encrypted_blobs_restore_and_scrub read blob");
    prefix.copy_from_slice(&raw[..8]);
    assert_eq!(&prefix, b"BSYNCENC", "expected encrypted blob magic");

    let versions = list_versions(&cfg)
        .expect("encryption_blobs::encrypted_blobs_restore_and_scrub list_versions");
    let (_src, versions) = versions
        .into_iter()
        .find(|(p, _)| p == &source)
        .expect("encryption_blobs::encrypted_blobs_restore_and_scrub missing source");
    let version_id = versions
        .last()
        .expect("encryption_blobs::encrypted_blobs_restore_and_scrub no versions")
        .id
        .clone();

    let restore_dir = tmp.path().join("restore");
    let req = RestoreRequest {
        source_path: source.clone(),
        version_id,
        mode: RestoreMode::ToDirectory,
        target_dir: Some(restore_dir.clone()),
    };
    restore_version(&cfg, &req)
        .expect("encryption_blobs::encrypted_blobs_restore_and_scrub restore");
    let restored = fs::read_to_string(restore_dir.join("a.txt"))
        .expect("encryption_blobs::encrypted_blobs_restore_and_scrub read restored file");
    assert_eq!(restored, "hello world");

    let scrub = scrub_versioned_store(&cfg, &cfg.hashing, ScrubMode::Full, 10, 10, 0)
        .expect("encryption_blobs::encrypted_blobs_restore_and_scrub scrub");
    assert_eq!(scrub.hash_mismatches, 0);
    assert_eq!(scrub.missing_blobs, 0);
    assert_eq!(scrub.manifests_bad, 0);
}

#[test]
fn encryption_key_id_mismatch_fails_backup() {
    let tmp = tempdir().expect("encryption_blobs::encryption_key_id_mismatch_fails_backup tempdir");
    let source = tmp.path().join("source");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&source)
        .expect("encryption_blobs::encryption_key_id_mismatch_fails_backup create source");
    fs::create_dir_all(&dest)
        .expect("encryption_blobs::encryption_key_id_mismatch_fails_backup create dest");

    let key_path = tmp.path().join("key_v1.bin");
    let _key_info = keyfile::create_key_file(&key_path, false)
        .expect("encryption_blobs::encryption_key_id_mismatch_fails_backup keygen");
    fs::write(source.join("a.txt"), "data")
        .expect("encryption_blobs::encryption_key_id_mismatch_fails_backup write file");

    let cfg = build_cfg(&source, &dest, &key_path, "deadbeef");
    let err = run_backup_cycle(&cfg)
        .expect_err("encryption_blobs::encryption_key_id_mismatch_fails_backup should error");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("key_id mismatch"),
        "expected key mismatch error, got: {msg}"
    );
}
