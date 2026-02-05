use backup_core::backup::versioned::{run_backup_cycle, scrub_versioned_store, ScrubMode};
use backup_core::config::model::{
    Config, Destination, ExecutionTuning, HashingTuning, PlanningTuning, RuntimeTuning,
    WatchedKind, WatchedPath,
};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn build_cfg(source: &Path, dest: &Path) -> Config {
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
        }],
    }
}

fn locate_one_blob(dest: &Path) -> PathBuf {
    let store_root = dest.join(".backup_sync").join("v1");
    let sources_root = store_root.join("sources");
    let src = fs::read_dir(&sources_root)
        .expect("versioned_scrub::locate_one_blob read sources")
        .next()
        .expect("versioned_scrub::locate_one_blob no sources")
        .expect("versioned_scrub::locate_one_blob readdir")
        .path();
    let manifests_root = src.join("manifests");
    let manifest_path = fs::read_dir(&manifests_root)
        .expect("versioned_scrub::locate_one_blob read manifests")
        .next()
        .expect("versioned_scrub::locate_one_blob no manifests")
        .expect("versioned_scrub::locate_one_blob readdir")
        .path();
    let raw =
        fs::read_to_string(&manifest_path).expect("versioned_scrub::locate_one_blob read manifest");
    let v: serde_json::Value =
        serde_json::from_str(&raw).expect("versioned_scrub::locate_one_blob parse manifest");
    let entries = v
        .get("entries")
        .and_then(|e| e.as_object())
        .expect("versioned_scrub::locate_one_blob missing entries");
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
            .expect("versioned_scrub::locate_one_blob missing sha256");
        let prefix = &sha[0..2];
        return store_root
            .join("blobs")
            .join("sha256")
            .join(prefix)
            .join(sha);
    }
    panic!("versioned_scrub::locate_one_blob no file entry found");
}

#[test]
fn scrub_detects_blob_hash_mismatch() {
    let tmp = tempdir().expect("versioned_scrub::scrub_detects_blob_hash_mismatch tempdir");
    let source = tmp.path().join("source");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&source)
        .expect("versioned_scrub::scrub_detects_blob_hash_mismatch create source");
    fs::create_dir_all(&dest)
        .expect("versioned_scrub::scrub_detects_blob_hash_mismatch create dest");
    fs::write(source.join("a.txt"), "v1")
        .expect("versioned_scrub::scrub_detects_blob_hash_mismatch write v1");

    let cfg = build_cfg(&source, &dest);
    run_backup_cycle(&cfg).expect("versioned_scrub::scrub_detects_blob_hash_mismatch backup");

    let blob = locate_one_blob(&dest);
    fs::write(&blob, "corrupted")
        .expect("versioned_scrub::scrub_detects_blob_hash_mismatch corrupt");

    let res = scrub_versioned_store(&cfg, &cfg.hashing, ScrubMode::Full, 1, 1, 0)
        .expect("versioned_scrub::scrub_detects_blob_hash_mismatch scrub");
    assert!(
        res.hash_mismatches >= 1,
        "expected at least one hash mismatch"
    );

    let sampled = scrub_versioned_store(&cfg, &cfg.hashing, ScrubMode::Sampled, 100, 10, 0)
        .expect("versioned_scrub::scrub_detects_blob_hash_mismatch scrub sampled");
    assert!(
        sampled.hash_mismatches >= 1,
        "expected sampled scrub to catch mismatch when sample covers all blobs"
    );
}

#[test]
fn scrub_detects_missing_blob() {
    let tmp = tempdir().expect("versioned_scrub::scrub_detects_missing_blob tempdir");
    let source = tmp.path().join("source");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&source).expect("versioned_scrub::scrub_detects_missing_blob create source");
    fs::create_dir_all(&dest).expect("versioned_scrub::scrub_detects_missing_blob create dest");
    fs::write(source.join("a.txt"), "v1")
        .expect("versioned_scrub::scrub_detects_missing_blob write v1");

    let cfg = build_cfg(&source, &dest);
    run_backup_cycle(&cfg).expect("versioned_scrub::scrub_detects_missing_blob backup");

    let blob = locate_one_blob(&dest);
    fs::remove_file(&blob).expect("versioned_scrub::scrub_detects_missing_blob remove blob");

    let res = scrub_versioned_store(&cfg, &cfg.hashing, ScrubMode::Full, 1, 1, 0)
        .expect("versioned_scrub::scrub_detects_missing_blob scrub");
    assert!(res.missing_blobs >= 1, "expected missing blob count");
}
