use anyhow::{Context, Result};
use backup_core::backup::versioned::restore::{
    list_versions, restore_version, RestoreMode, RestoreRequest,
};
use backup_core::{
    backup::versioned,
    config::{
        model::{Destination, WatchedKind, WatchedPath},
        registry::config_defaults,
    },
    validate,
};
use std::fs;
use tempfile::tempdir;

#[test]
/// Ensure the production engine stays healthy across refactors.
fn e2e_smoke_versioned_backup_and_restore() -> Result<()> {
    let dir = tempdir().context("e2e_smoke::versioned failed to create temp root")?;
    let watched_dir = dir.path().join("watched");
    let destination = dir.path().join("dest");
    fs::create_dir_all(&watched_dir).context("e2e_smoke::versioned failed to create watched")?;
    fs::create_dir_all(&destination).context("e2e_smoke::versioned failed to create dest")?;
    let file_path = watched_dir.join("note.txt");
    fs::write(&file_path, "hello world\n").context("e2e_smoke::versioned write watched file")?;

    let mut cfg = config_defaults().context("e2e_smoke::versioned defaults")?;
    cfg.backup_root = destination.clone();
    cfg.runtime.source_snapshots_enabled = false;
    cfg.destinations = vec![Destination {
        id: "primary".into(),
        path: destination.clone(),
        label: Some("Primary".into()),
        max_backups_per_file: None,
        replicate_to: vec![],
    }];
    cfg.watched = vec![WatchedPath {
        path: watched_dir.clone(),
        kind: WatchedKind::Directory,
        enabled: true,
        destination_id: "primary".into(),
        max_backups_per_file: Some(5),
    }];
    validate(&cfg).context("e2e_smoke::versioned config validation")?;

    let result = versioned::run_backup_cycle(&cfg).context("e2e_smoke::versioned run backup")?;
    assert_eq!(result.versions_created, 1, "expected one version created");

    let versions = list_versions(&cfg).context("e2e_smoke::versioned list versions")?;
    let (source_path, infos) = versions
        .into_iter()
        .find(|(p, _)| p == &watched_dir)
        .context("e2e_smoke::versioned missing watched folder in list_versions")?;
    let latest = infos
        .last()
        .context("e2e_smoke::versioned expected at least one version")?;

    let restore_dir = dir.path().join("restore_out");
    let restore_res = restore_version(
        &cfg,
        &RestoreRequest {
            source_path,
            version_id: latest.id.clone(),
            mode: RestoreMode::ToDirectory,
            target_dir: Some(restore_dir.clone()),
        },
    )
    .context("e2e_smoke::versioned restore_version")?;
    assert!(
        restore_res.files_written >= 1,
        "expected at least one restored file"
    );

    let restored = restore_dir.join("note.txt");
    let body = fs::read_to_string(&restored)
        .with_context(|| format!("e2e_smoke::versioned read restored file {:?}", restored))?;
    assert_eq!(body, "hello world\n");
    Ok(())
}
