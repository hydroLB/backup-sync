use backup_core::backup::versioned::{
    list_versions, restore_files, restore_version, run_backup_cycle, RestoreFilesRequest,
    RestoreMode, RestoreRequest,
};
use backup_core::config::model::{
    Config, Destination, ExecutionTuning, HashingTuning, PlanningTuning, RuntimeTuning,
    WatchedKind, WatchedPath,
};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn build_cfg(source: &Path, destination: &Path) -> Config {
    Config {
        backup_root: destination.to_path_buf(),
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
            path: source.to_path_buf(),
            kind: WatchedKind::Directory,
            enabled: true,
            destination_id: "default".into(),
            max_backups_per_file: Some(5),
        }],
        destinations: vec![Destination {
            id: "default".into(),
            path: destination.to_path_buf(),
            label: None,
            max_backups_per_file: None,
            replicate_to: vec![],
        }],
    }
}

fn create_version() -> (tempfile::TempDir, Config, PathBuf, PathBuf, String) {
    let temp = tempdir().expect("restore_security failed to create temp directory");
    let source = temp.path().join("source");
    let destination = temp.path().join("destination");
    fs::create_dir_all(&source).expect("restore_security failed to create source");
    fs::create_dir_all(&destination).expect("restore_security failed to create destination");
    fs::write(source.join("safe.txt"), "trusted")
        .expect("restore_security failed to write source file");

    let cfg = build_cfg(&source, &destination);
    run_backup_cycle(&cfg).expect("restore_security failed to create version");
    let version_id = list_versions(&cfg)
        .expect("restore_security failed to list versions")
        .into_iter()
        .next()
        .and_then(|(_, versions)| versions.into_iter().next())
        .expect("restore_security missing created version")
        .id;

    (temp, cfg, source, destination, version_id)
}

fn manifest_path(destination: &Path, source: &Path, version_id: &str) -> PathBuf {
    let source_id = backup_core::sha256_hex(source.to_string_lossy().as_bytes());
    destination
        .join(".backup_sync/v1/sources")
        .join(source_id)
        .join("manifests")
        .join(format!("{version_id}.json"))
}

fn restore_in_place(cfg: &Config, source: &Path, version_id: &str) -> anyhow::Result<()> {
    restore_version(
        cfg,
        &RestoreRequest {
            source_path: source.to_path_buf(),
            version_id: version_id.to_string(),
            mode: RestoreMode::InPlace,
            target_dir: None,
        },
    )?;
    Ok(())
}

#[test]
fn rejects_untrusted_version_identifiers() {
    let (_temp, cfg, source, _destination, _version_id) = create_version();

    for invalid in [
        "",
        ".",
        "..",
        "../outside",
        "/absolute",
        "C:\\outside",
        "id.json",
    ] {
        let error = restore_in_place(&cfg, &source, invalid)
            .expect_err("restore_security accepted an untrusted version identifier");
        assert!(
            error.to_string().contains("invalid version identifier"),
            "unexpected error for {invalid:?}: {error:#}"
        );
    }
}

#[test]
fn rejects_escaping_and_incoherent_manifest_paths_before_restore() {
    for (map_key, rel_path) in [
        ("../escaped.txt", "../escaped.txt"),
        ("/absolute.txt", "/absolute.txt"),
        ("C:\\escaped.txt", "C:\\escaped.txt"),
        ("C:/escaped.txt", "C:/escaped.txt"),
        ("safe.txt", "different.txt"),
    ] {
        let (temp, cfg, source, destination, version_id) = create_version();
        fs::write(source.join("safe.txt"), "current")
            .expect("restore_security failed to update source");
        let path = manifest_path(&destination, &source, &version_id);
        let mut manifest: Value = serde_json::from_slice(
            &fs::read(&path).expect("restore_security failed to read manifest"),
        )
        .expect("restore_security failed to parse manifest");
        let entry = manifest["entries"]["safe.txt"].take();
        manifest["entries"]
            .as_object_mut()
            .expect("restore_security entries was not an object")
            .remove("safe.txt");
        let mut entry = entry;
        entry["rel_path"] = Value::String(rel_path.to_string());
        manifest["entries"][map_key] = entry;
        fs::write(
            &path,
            serde_json::to_vec(&manifest).expect("restore_security failed to encode manifest"),
        )
        .expect("restore_security failed to replace manifest");

        restore_in_place(&cfg, &source, &version_id)
            .expect_err("restore_security accepted an unsafe manifest path");
        assert_eq!(
            fs::read_to_string(source.join("safe.txt"))
                .expect("restore_security failed to read current source"),
            "current"
        );
        assert!(
            !temp.path().join("escaped.txt").exists(),
            "unsafe restore wrote outside its root"
        );
    }
}

#[test]
fn plaintext_hash_mismatch_never_replaces_target() {
    let (_temp, cfg, source, destination, version_id) = create_version();
    let path = manifest_path(&destination, &source, &version_id);
    let manifest: Value =
        serde_json::from_slice(&fs::read(&path).expect("restore_security failed to read manifest"))
            .expect("restore_security failed to parse manifest");
    let hash = manifest["entries"]["safe.txt"]["sha256"]
        .as_str()
        .expect("restore_security missing manifest hash");
    let blob = destination
        .join(".backup_sync/v1/blobs/sha256")
        .join(&hash[..2])
        .join(hash);
    fs::write(blob, "corrupt").expect("restore_security failed to corrupt blob");
    fs::write(source.join("safe.txt"), "current")
        .expect("restore_security failed to update source");

    let error = restore_in_place(&cfg, &source, &version_id)
        .expect_err("restore_security restored plaintext with a mismatched hash");
    assert!(error.to_string().contains("plaintext sha256 mismatch"));
    assert_eq!(
        fs::read_to_string(source.join("safe.txt"))
            .expect("restore_security failed to read current source"),
        "current"
    );
}

#[test]
fn selective_restore_stages_every_file_before_mutating_targets() {
    let temp = tempdir().expect("restore_security failed to create temp directory");
    let source = temp.path().join("source");
    let destination = temp.path().join("destination");
    fs::create_dir_all(&source).expect("restore_security failed to create source");
    fs::create_dir_all(&destination).expect("restore_security failed to create destination");
    fs::write(source.join("a.txt"), "trusted-a")
        .expect("restore_security failed to write first source file");
    fs::write(source.join("z.txt"), "trusted-z")
        .expect("restore_security failed to write second source file");

    let cfg = build_cfg(&source, &destination);
    run_backup_cycle(&cfg).expect("restore_security failed to create version");
    let version_id = list_versions(&cfg)
        .expect("restore_security failed to list versions")
        .into_iter()
        .next()
        .and_then(|(_, versions)| versions.into_iter().next())
        .expect("restore_security missing created version")
        .id;
    let path = manifest_path(&destination, &source, &version_id);
    let manifest: Value =
        serde_json::from_slice(&fs::read(&path).expect("restore_security failed to read manifest"))
            .expect("restore_security failed to parse manifest");
    let corrupt_hash = manifest["entries"]["z.txt"]["sha256"]
        .as_str()
        .expect("restore_security missing second file hash");
    let corrupt_blob = destination
        .join(".backup_sync/v1/blobs/sha256")
        .join(&corrupt_hash[..2])
        .join(corrupt_hash);
    fs::write(corrupt_blob, "corrupt-late")
        .expect("restore_security failed to corrupt second blob");
    fs::write(source.join("a.txt"), "current-a")
        .expect("restore_security failed to update first target");
    fs::write(source.join("z.txt"), "current-z")
        .expect("restore_security failed to update second target");

    let error = restore_files(
        &cfg,
        &RestoreFilesRequest {
            source_path: source.clone(),
            version_id,
            rel_paths: vec!["a.txt".into(), "z.txt".into()],
            mode: RestoreMode::InPlace,
            target_dir: None,
        },
    )
    .expect_err("restore_security accepted a corrupt artifact in a selective restore");
    assert!(error.to_string().contains("plaintext sha256 mismatch"));
    assert_eq!(
        fs::read_to_string(source.join("a.txt"))
            .expect("restore_security failed to read first current target"),
        "current-a"
    );
    assert_eq!(
        fs::read_to_string(source.join("z.txt"))
            .expect("restore_security failed to read second current target"),
        "current-z"
    );
}

#[cfg(unix)]
#[test]
fn selective_restore_rejects_preexisting_directory_symlink_escape() {
    use std::os::unix::fs::symlink;

    let (temp, cfg, source, destination, version_id) = create_version();
    let path = manifest_path(&destination, &source, &version_id);
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&path).expect("restore_security failed to read manifest"))
            .expect("restore_security failed to parse manifest");
    let mut entry = manifest["entries"]["safe.txt"].take();
    manifest["entries"]
        .as_object_mut()
        .expect("restore_security entries was not an object")
        .remove("safe.txt");
    entry["rel_path"] = Value::String("escape/safe.txt".into());
    manifest["entries"]["escape/safe.txt"] = entry;
    fs::write(
        &path,
        serde_json::to_vec(&manifest).expect("restore_security failed to encode manifest"),
    )
    .expect("restore_security failed to replace manifest");

    let outside = temp.path().join("outside");
    fs::create_dir_all(&outside).expect("restore_security failed to create outside directory");
    fs::write(outside.join("safe.txt"), "outside-current")
        .expect("restore_security failed to write outside sentinel");
    symlink(&outside, source.join("escape"))
        .expect("restore_security failed to create directory symlink");

    let error = restore_files(
        &cfg,
        &RestoreFilesRequest {
            source_path: source,
            version_id,
            rel_paths: vec!["escape/safe.txt".into()],
            mode: RestoreMode::InPlace,
            target_dir: None,
        },
    )
    .expect_err("restore_security followed a pre-existing target directory symlink");
    assert!(error.to_string().contains("symlink component"));
    assert_eq!(
        fs::read_to_string(outside.join("safe.txt"))
            .expect("restore_security failed to read outside sentinel"),
        "outside-current"
    );
}
