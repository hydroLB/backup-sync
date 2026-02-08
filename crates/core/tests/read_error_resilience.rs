use backup_core::backup::versioned::{run_backup_cycle, Manifest, VersionIndex};
use backup_core::config::model::{
    Config, Destination, ExecutionTuning, HashingTuning, PlanningTuning, RuntimeTuning,
    WatchedKind, WatchedPath,
};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

fn build_cfg(source: &Path, dest: &Path, keep_versions: usize) -> Config {
    let mut execution = ExecutionTuning::default();
    execution.retry_delays_ms = vec![];
    execution.retry_jitter_pct = 0.0;
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
        execution,
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

fn load_index_and_manifests(
    dest: &Path,
) -> (VersionIndex, Vec<(String, Manifest)>, std::path::PathBuf) {
    let store_root = dest.join(".backup_sync").join("v1");
    let sources_root = store_root.join("sources");
    let mut sources: Vec<_> = fs::read_dir(&sources_root)
        .expect("read_error_resilience::load_index_and_manifests read sources")
        .map(|e| {
            e.expect("read_error_resilience::load_index_and_manifests readdir")
                .path()
        })
        .collect();
    sources.sort();
    assert_eq!(sources.len(), 1, "expected one watched source");
    let source_root = sources[0].clone();
    let index_path = source_root.join("index.json");
    let raw = fs::read_to_string(&index_path)
        .expect("read_error_resilience::load_index_and_manifests read index");
    let index: VersionIndex =
        serde_json::from_str(&raw).expect("read_error_resilience::load_index_and_manifests parse");

    let manifests_root = source_root.join("manifests");
    let mut manifests: Vec<(String, Manifest)> = Vec::new();
    for v in index.versions.iter() {
        let path = manifests_root.join(format!("{}.json", v.id));
        let raw = fs::read_to_string(&path)
            .expect("read_error_resilience::load_index_and_manifests read manifest");
        let m: Manifest = serde_json::from_str(&raw)
            .expect("read_error_resilience::load_index_and_manifests parse manifest");
        manifests.push((v.id.clone(), m));
    }
    let last_scan_report = source_root.join("last_scan_report.json");
    (index, manifests, last_scan_report)
}

#[cfg(target_family = "unix")]
#[test]
fn versioned_preserves_last_good_on_read_errors() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempdir()
        .expect("read_error_resilience::versioned_preserves_last_good_on_read_errors tempdir");
    let source = tmp.path().join("source");
    let dest = tmp.path().join("dest");
    fs::create_dir_all(&source).expect(
        "read_error_resilience::versioned_preserves_last_good_on_read_errors create source",
    );
    fs::create_dir_all(&dest)
        .expect("read_error_resilience::versioned_preserves_last_good_on_read_errors create dest");

    let good = source.join("good.txt");
    let bad = source.join("bad.txt");
    fs::write(&good, "v1").expect(
        "read_error_resilience::versioned_preserves_last_good_on_read_errors write good v1",
    );
    fs::write(&bad, "v1")
        .expect("read_error_resilience::versioned_preserves_last_good_on_read_errors write bad v1");

    let cfg = build_cfg(&source, &dest, 2);

    run_backup_cycle(&cfg)
        .expect("read_error_resilience::versioned_preserves_last_good_on_read_errors cycle 1");
    fs::write(&good, "v2").expect(
        "read_error_resilience::versioned_preserves_last_good_on_read_errors write good v2",
    );
    run_backup_cycle(&cfg)
        .expect("read_error_resilience::versioned_preserves_last_good_on_read_errors cycle 2");

    fs::write(&bad, "v2")
        .expect("read_error_resilience::versioned_preserves_last_good_on_read_errors write bad v2");
    let mut perms = fs::metadata(&bad)
        .expect("read_error_resilience::versioned_preserves_last_good_on_read_errors stat bad")
        .permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&bad, perms)
        .expect("read_error_resilience::versioned_preserves_last_good_on_read_errors chmod bad");
    fs::write(&good, "v3").expect(
        "read_error_resilience::versioned_preserves_last_good_on_read_errors write good v3",
    );
    run_backup_cycle(&cfg)
        .expect("read_error_resilience::versioned_preserves_last_good_on_read_errors cycle 3");

    let (index, manifests, report_path) = load_index_and_manifests(&dest);
    assert_eq!(
        index.versions.len(),
        3,
        "retention should be skipped when read errors occur"
    );

    let last = manifests
        .last()
        .expect("read_error_resilience::versioned_preserves_last_good_on_read_errors missing last")
        .1
        .clone();
    assert!(
        last.read_failures.iter().any(|f| f.rel_path == "bad.txt"),
        "expected a read failure recorded for bad.txt"
    );

    let prev = manifests
        .get(1)
        .expect("read_error_resilience::versioned_preserves_last_good_on_read_errors missing prev")
        .1
        .clone();
    let prev_bad = prev
        .entries
        .get("bad.txt")
        .and_then(|e| e.sha256.clone())
        .expect("read_error_resilience::versioned_preserves_last_good_on_read_errors prev sha");
    let last_bad = last
        .entries
        .get("bad.txt")
        .and_then(|e| e.sha256.clone())
        .expect("read_error_resilience::versioned_preserves_last_good_on_read_errors last sha");
    assert_eq!(
        prev_bad, last_bad,
        "unreadable file should carry forward last known-good entry"
    );

    let raw = fs::read_to_string(&report_path).expect(
        "read_error_resilience::versioned_preserves_last_good_on_read_errors read scan report",
    );
    let v: serde_json::Value = serde_json::from_str(&raw).expect(
        "read_error_resilience::versioned_preserves_last_good_on_read_errors parse scan report",
    );
    assert!(
        v.get("read_failures")
            .and_then(|x| x.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false),
        "scan report should include read_failures"
    );
}

#[cfg(not(target_family = "unix"))]
#[test]
fn versioned_preserves_last_good_on_read_errors() {
    // Permission-based read failure simulation is Unix-specific.
}
