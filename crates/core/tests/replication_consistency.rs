mod support;

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
use tempfile::{tempdir, TempDir};

struct Fixture {
    _tmp: TempDir,
    source: PathBuf,
    primary: PathBuf,
    mirror: PathBuf,
    cfg: Config,
}

impl Fixture {
    fn new() -> Self {
        let tmp = tempdir().expect("replication_consistency tempdir");
        let source = tmp.path().join("source");
        let primary = tmp.path().join("primary");
        let mirror = tmp.path().join("mirror");
        for path in [&source, &primary, &mirror] {
            fs::create_dir_all(path).expect("replication_consistency create fixture directory");
        }
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
                    label: None,
                    max_backups_per_file: None,
                    replicate_to: vec!["mirror".into()],
                },
                Destination {
                    id: "mirror".into(),
                    path: mirror.clone(),
                    label: None,
                    max_backups_per_file: None,
                    replicate_to: vec![],
                },
            ],
        };
        validate(&cfg).expect("replication_consistency validate config");
        Self {
            _tmp: tmp,
            source,
            primary,
            mirror,
            cfg,
        }
    }

    fn write_revision(&self, revision: i64, contents: &str) {
        support::write_fixture_revision(&self.source.join("a.txt"), revision, contents);
    }
}

fn source_root(destination: &Path) -> PathBuf {
    let sources = destination.join(".backup_sync/v1/sources");
    let mut roots: Vec<_> = fs::read_dir(sources)
        .expect("replication_consistency read sources")
        .map(|entry| entry.expect("replication_consistency source entry").path())
        .collect();
    assert_eq!(roots.len(), 1);
    roots.pop().expect("replication_consistency source root")
}

fn index(root: &Path) -> VersionIndex {
    serde_json::from_slice(
        &fs::read(root.join("index.json")).expect("replication_consistency read index"),
    )
    .expect("replication_consistency parse index")
}

fn manifest(root: &Path, version_id: &str) -> Manifest {
    serde_json::from_slice(
        &fs::read(root.join("manifests").join(format!("{version_id}.json")))
            .expect("replication_consistency read manifest"),
    )
    .expect("replication_consistency parse manifest")
}

fn referenced_blob(root: &Path, version_id: &str) -> PathBuf {
    let hash = manifest(root, version_id)
        .entries
        .get("a.txt")
        .and_then(|entry| entry.sha256.as_deref())
        .expect("replication_consistency file hash")
        .to_string();
    root.parent()
        .and_then(Path::parent)
        .expect("replication_consistency store root")
        .join("blobs/sha256")
        .join(&hash[..2])
        .join(hash)
}

#[test]
fn missing_source_blob_fails_without_replacing_replica_index() {
    let fixture = Fixture::new();
    fixture.write_revision(1, "first");
    run_backup_cycle(&fixture.cfg).expect("replication_consistency initial backup");
    replicate_configured_stores(&fixture.cfg).expect("replication_consistency initial replicate");

    let replica_root = source_root(&fixture.mirror);
    let prior_index = fs::read(replica_root.join("index.json"))
        .expect("replication_consistency read prior replica index");

    fixture.write_revision(2, "second revision");
    run_backup_cycle(&fixture.cfg).expect("replication_consistency second backup");
    let primary_root = source_root(&fixture.primary);
    let primary_index = index(&primary_root);
    let latest = &primary_index
        .versions
        .last()
        .expect("replication_consistency latest version")
        .id;
    fs::remove_file(referenced_blob(&primary_root, latest))
        .expect("replication_consistency remove primary blob");

    let summary = replicate_configured_stores(&fixture.cfg)
        .expect("replication_consistency failed pair is summarized");
    assert_eq!(summary.pairs_failed, 1);
    assert_eq!(
        fs::read(replica_root.join("index.json"))
            .expect("replication_consistency reread replica index"),
        prior_index
    );
}

#[test]
fn indexed_version_with_missing_replica_blob_is_repaired() {
    let fixture = Fixture::new();
    fixture.write_revision(1, "repair me");
    run_backup_cycle(&fixture.cfg).expect("replication_consistency backup");
    replicate_configured_stores(&fixture.cfg).expect("replication_consistency initial replicate");

    let replica_root = source_root(&fixture.mirror);
    let version_id = index(&replica_root).versions[0].id.clone();
    let blob = referenced_blob(&replica_root, &version_id);
    fs::remove_file(&blob).expect("replication_consistency remove replica blob");

    let summary = replicate_configured_stores(&fixture.cfg)
        .expect("replication_consistency repair replicate");
    assert_eq!(summary.pairs_failed, 0);
    assert_eq!(summary.blobs_copied, 1);
    assert!(blob.is_file());
}

#[test]
fn indexed_version_with_corrupt_replica_blob_is_repaired() {
    let mut fixture = Fixture::new();
    fixture.cfg.compression.enabled = true;
    validate(&fixture.cfg).expect("replication_consistency validate compressed config");
    fixture.write_revision(1, "repair corrupt bytes");
    run_backup_cycle(&fixture.cfg).expect("replication_consistency backup");
    replicate_configured_stores(&fixture.cfg).expect("replication_consistency initial replicate");

    let primary_root = source_root(&fixture.primary);
    let replica_root = source_root(&fixture.mirror);
    let version_id = index(&replica_root).versions[0].id.clone();
    let primary_blob = referenced_blob(&primary_root, &version_id);
    let replica_blob = referenced_blob(&replica_root, &version_id);
    fs::write(&replica_blob, b"present but corrupt")
        .expect("replication_consistency corrupt replica blob");

    let summary = replicate_configured_stores(&fixture.cfg)
        .expect("replication_consistency repair corrupt blob");
    assert_eq!(summary.pairs_failed, 0);
    assert_eq!(summary.blobs_copied, 1);
    assert_eq!(
        fs::read(replica_blob).expect("replication_consistency read repaired blob"),
        fs::read(primary_blob).expect("replication_consistency read source blob")
    );
}

#[test]
fn stale_valid_replica_manifest_is_replaced_from_source() {
    let fixture = Fixture::new();
    fixture.write_revision(1, "authoritative manifest");
    run_backup_cycle(&fixture.cfg).expect("replication_consistency backup");
    replicate_configured_stores(&fixture.cfg).expect("replication_consistency initial replicate");

    let primary_root = source_root(&fixture.primary);
    let replica_root = source_root(&fixture.mirror);
    let version_id = index(&replica_root).versions[0].id.clone();
    let primary_manifest = primary_root
        .join("manifests")
        .join(format!("{version_id}.json"));
    let replica_manifest = replica_root
        .join("manifests")
        .join(format!("{version_id}.json"));
    let mut stale = manifest(&replica_root, &version_id);
    stale.source_path = "stale-but-valid-source".to_string();
    fs::write(
        &replica_manifest,
        serde_json::to_vec_pretty(&stale)
            .expect("replication_consistency serialize stale manifest"),
    )
    .expect("replication_consistency write stale manifest");

    let summary = replicate_configured_stores(&fixture.cfg)
        .expect("replication_consistency replace stale manifest");
    assert_eq!(summary.pairs_failed, 0);
    assert_eq!(summary.manifests_copied, 1);
    assert_eq!(
        fs::read(replica_manifest).expect("replication_consistency read repaired manifest"),
        fs::read(primary_manifest).expect("replication_consistency read source manifest")
    );
}

#[cfg(unix)]
#[test]
fn replica_blob_symlink_is_rejected_without_touching_target() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    fixture.write_revision(1, "symlink defense");
    run_backup_cycle(&fixture.cfg).expect("replication_consistency backup");
    replicate_configured_stores(&fixture.cfg).expect("replication_consistency initial replicate");

    let replica_root = source_root(&fixture.mirror);
    let prior_index = fs::read(replica_root.join("index.json"))
        .expect("replication_consistency read prior replica index");
    let version_id = index(&replica_root).versions[0].id.clone();
    let replica_blob = referenced_blob(&replica_root, &version_id);
    let outside_target = fixture._tmp.path().join("outside-target");
    fs::write(&outside_target, b"must remain unchanged")
        .expect("replication_consistency write symlink target");
    fs::remove_file(&replica_blob).expect("replication_consistency remove replica blob");
    symlink(&outside_target, &replica_blob).expect("replication_consistency create blob symlink");

    let summary = replicate_configured_stores(&fixture.cfg)
        .expect("replication_consistency symlink rejection is summarized");
    assert_eq!(summary.pairs_failed, 1);
    assert_eq!(
        fs::read(&outside_target).expect("replication_consistency read symlink target"),
        b"must remain unchanged"
    );
    assert!(fs::symlink_metadata(&replica_blob)
        .expect("replication_consistency inspect blob symlink")
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::read(replica_root.join("index.json"))
            .expect("replication_consistency reread replica index"),
        prior_index
    );
}

#[test]
fn invalid_manifest_hash_fails_before_replica_index_publication() {
    let fixture = Fixture::new();
    fixture.write_revision(1, "invalid hash");
    run_backup_cycle(&fixture.cfg).expect("replication_consistency backup");

    let primary_root = source_root(&fixture.primary);
    let version_id = index(&primary_root).versions[0].id.clone();
    let manifest_path = primary_root
        .join("manifests")
        .join(format!("{version_id}.json"));
    let mut source_manifest = manifest(&primary_root, &version_id);
    source_manifest
        .entries
        .get_mut("a.txt")
        .expect("replication_consistency a.txt entry")
        .sha256 = Some("A".repeat(64));
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&source_manifest)
            .expect("replication_consistency serialize manifest"),
    )
    .expect("replication_consistency replace manifest");

    let summary = replicate_configured_stores(&fixture.cfg)
        .expect("replication_consistency invalid hash is summarized");
    assert_eq!(summary.pairs_failed, 1);
    assert!(
        !fixture.mirror.join(".backup_sync/v1/sources").exists()
            || fs::read_dir(fixture.mirror.join(".backup_sync/v1/sources"))
                .expect("replication_consistency read empty replica sources")
                .all(|entry| !entry
                    .expect("replication_consistency replica entry")
                    .path()
                    .join("index.json")
                    .exists())
    );
}
