use backup_core::backup::versioned::{run_backup_cycle, Manifest, VersionIndex};
use backup_core::config::model::{
    Config, Destination, ExecutionTuning, HashingTuning, PlanningTuning, RuntimeTuning,
    WatchedKind, WatchedPath,
};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn build_cfg(source: &Path, dest: &Path) -> Config {
    Config {
        backup_root: dest.to_path_buf(),
        interval_seconds: 1800,
        max_backups_per_file: 10,
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
            max_backups_per_file: Some(10),
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

fn assert_index_and_manifests_refer_to_existing_blobs(dest: &Path) {
    let store_root = dest.join(".backup_sync").join("v1");
    let sources_root = store_root.join("sources");
    let mut sources: Vec<_> = fs::read_dir(&sources_root)
        .expect("durable_commits::assert_index_and_manifests_refer_to_existing_blobs read sources")
        .map(|e| {
            e.expect("durable_commits::assert_index_and_manifests_refer_to_existing_blobs readdir")
                .path()
        })
        .collect();
    sources.sort();
    assert_eq!(
        sources.len(),
        1,
        "expected exactly one source root for this test"
    );
    let source_root = sources[0].clone();
    let index_path = source_root.join("index.json");
    let raw = fs::read_to_string(&index_path)
        .expect("durable_commits::assert_index_and_manifests_refer_to_existing_blobs read index");
    let index: VersionIndex = serde_json::from_str(&raw)
        .expect("durable_commits::assert_index_and_manifests_refer_to_existing_blobs parse index");

    let blobs_root = store_root.join("blobs").join("sha256");
    let manifests_root = source_root.join("manifests");
    for v in index.versions.iter() {
        let manifest_path = manifests_root.join(format!("{}.json", v.id));
        assert!(
            manifest_path.exists(),
            "index points at missing manifest: {:?}",
            manifest_path
        );
        let raw = fs::read_to_string(&manifest_path).expect(
            "durable_commits::assert_index_and_manifests_refer_to_existing_blobs read manifest",
        );
        let manifest: Manifest = serde_json::from_str(&raw).expect(
            "durable_commits::assert_index_and_manifests_refer_to_existing_blobs parse manifest",
        );
        for entry in manifest.entries.values() {
            let Some(h) = entry.sha256.as_ref() else {
                continue;
            };
            let prefix = &h[0..2];
            let blob_path = blobs_root.join(prefix).join(h);
            assert!(
                blob_path.exists(),
                "manifest references missing blob: {:?}",
                blob_path
            );
        }
    }
}

#[test]
fn versioned_commit_is_structurally_durable() {
    let tmp = tempdir().expect("durable_commits::versioned_commit_is_structurally_durable tempdir");
    let source = tmp.path().join("source");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&source)
        .expect("durable_commits::versioned_commit_is_structurally_durable create source dir");
    fs::create_dir_all(&dest)
        .expect("durable_commits::versioned_commit_is_structurally_durable create dest dir");

    fs::write(source.join("a.txt"), "v1")
        .expect("durable_commits::versioned_commit_is_structurally_durable write a v1");
    fs::write(source.join("b.txt"), "v1")
        .expect("durable_commits::versioned_commit_is_structurally_durable write b v1");

    let cfg = build_cfg(&source, &dest);
    run_backup_cycle(&cfg)
        .expect("durable_commits::versioned_commit_is_structurally_durable cycle 1");

    fs::write(source.join("a.txt"), "v2")
        .expect("durable_commits::versioned_commit_is_structurally_durable write a v2");
    run_backup_cycle(&cfg)
        .expect("durable_commits::versioned_commit_is_structurally_durable cycle 2");

    assert_index_and_manifests_refer_to_existing_blobs(&dest);
}
