//! Validates backup destination paths without performing side-effectful writes.
use crate::api::{config_api, status_api};
use crate::commands::error::ErrorEnvelope;
use crate::commands::io_policy::run_blocking_io;
use anyhow::{bail, Context, Result};
use backup_core::backup::versioned::{self, ScrubMode, VersionIndex};
use fs2::free_space;
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
#[cfg(target_os = "macos")]
use std::path::Component;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize)]
/// Communicate validation feedback to the frontend.
pub struct DestinationCheck {
    pub writable: bool,
    pub free_bytes: Option<u64>,
    pub message: String,
}

#[derive(Serialize)]
/// Report the completed, verified storage relocation and the active configuration.
pub struct DestinationRelocationResult {
    pub config: backup_core::Config,
    pub operation: &'static str,
    pub files_moved: usize,
    pub bytes_moved: u64,
    pub old_location_removed: bool,
    pub warning: Option<String>,
}

/// Open one configured storage directory with the platform file manager.
fn open_directory(path: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer.exe");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    #[cfg(not(any(target_os = "macos", target_os = "windows", unix)))]
    bail!("Opening storage folders is not supported on this platform.");

    let status = command
        .arg(path)
        .status()
        .with_context(|| format!("failed to launch the file manager for {:?}", path))?;
    if !status.success() {
        bail!("The file manager could not open the configured storage folder.");
    }
    Ok(())
}

#[tauri::command]
/// Resolve storage by id so the frontend cannot use this command to open arbitrary paths.
pub fn open_destination_cmd(destination_id: String) -> Result<(), ErrorEnvelope> {
    let cfg = config_api::get_config().map_err(|error| {
        relocation_error(
            "CONFIG_LOAD",
            "destination::open_destination_cmd failed to load config",
            &error,
        )
    })?;
    let destination = cfg
        .destinations
        .iter()
        .find(|candidate| candidate.id == destination_id)
        .ok_or_else(|| ErrorEnvelope::new("NOT_FOUND", "Storage location no longer exists."))?;
    if !destination.path.is_dir() {
        return Err(ErrorEnvelope::new(
            "NOT_FOUND",
            "The configured storage folder is missing or unavailable.",
        ));
    }
    open_directory(&destination.path).map_err(|error| {
        relocation_error(
            "DEPENDENCY_UNAVAILABLE",
            "destination::open_destination_cmd failed to open storage folder",
            &error,
        )
    })
}

#[derive(Debug)]
struct CopyStats {
    files: usize,
    bytes: u64,
}

#[derive(Debug)]
struct PromotionStats {
    manifests_verified: usize,
}

/// Compare existing paths by filesystem identity when possible, not only by spelling.
fn paths_refer_to_same_location(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

/// Swap physical stores while preserving destination ids and every watched-source mapping.
fn swap_main_with_secondary(
    cfg: &backup_core::Config,
    secondary_index: usize,
) -> Result<backup_core::Config> {
    if secondary_index == 0 || secondary_index >= cfg.destinations.len() {
        bail!("Only an existing secondary backup location can become Main storage.");
    }
    let mut next = cfg.clone();
    let main_path = next.destinations[0].path.clone();
    let secondary_path = next.destinations[secondary_index].path.clone();
    next.destinations[0].path = secondary_path.clone();
    next.destinations[secondary_index].path = main_path;
    next.backup_root = secondary_path;
    backup_core::validate(&next).context("promoted storage configuration is invalid")?;
    Ok(next)
}

fn read_version_index(destination_root: &Path, source_path: &Path) -> Result<Option<VersionIndex>> {
    let path = versioned::version_index_path(destination_root, source_path);
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read recovery index {:?}", path))?;
    let index = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse recovery index {:?}", path))?;
    Ok(Some(index))
}

/// Require the candidate to be current and internally recoverable before changing roles.
fn verify_secondary_can_be_main(
    cfg: &backup_core::Config,
    secondary_index: usize,
) -> Result<(backup_core::Config, PromotionStats)> {
    let main = cfg
        .destinations
        .first()
        .context("Main storage is not configured.")?;
    let secondary = cfg
        .destinations
        .get(secondary_index)
        .context("The selected secondary backup location no longer exists.")?;
    if !main.path.is_dir() {
        bail!("Main storage is missing or unavailable; no storage roles were changed.");
    }
    if !secondary.path.is_dir() {
        bail!("The selected secondary backup location is missing or unavailable.");
    }
    if paths_refer_to_same_location(&main.path, &secondary.path) {
        bail!("Main and secondary storage resolve to the same folder.");
    }

    let mut checked_sources = HashSet::new();
    for watched in cfg
        .watched
        .iter()
        .filter(|watched| watched.enabled && watched.destination_id == main.id)
    {
        if !checked_sources.insert(watched.path.clone()) {
            continue;
        }
        let current = read_version_index(&main.path, &watched.path)?;
        let candidate = read_version_index(&secondary.path, &watched.path)?;
        let Some(current_latest) = current.as_ref().and_then(|index| {
            index
                .versions
                .iter()
                .max_by_key(|version| version.created_at_unix)
        }) else {
            continue;
        };
        let candidate_latest = candidate
            .as_ref()
            .and_then(|index| {
                index
                    .versions
                    .iter()
                    .max_by_key(|version| version.created_at_unix)
            })
            .with_context(|| {
                format!(
                    "The selected secondary location has no recovery version for {:?}.",
                    watched.path
                )
            })?;
        if candidate_latest.created_at_unix < current_latest.created_at_unix {
            bail!(
                "The selected secondary location is behind Main storage for {:?}; run a backup and try again.",
                watched.path
            );
        }
    }

    let next = swap_main_with_secondary(cfg, secondary_index)?;
    let scrub = versioned::scrub_versioned_store(
        &next,
        &next.hashing,
        ScrubMode::Full,
        next.runtime.scrub_sample_blobs,
        next.runtime.scrub_sample_versions_per_source,
        0,
    )
    .context("full integrity verification of the selected storage location failed")?;
    let issues = scrub
        .manifests_bad
        .saturating_add(scrub.missing_blobs)
        .saturating_add(scrub.hash_mismatches);
    if issues > 0 {
        bail!(
            "The selected secondary location has {issues} integrity issue(s); storage roles were not changed."
        );
    }
    Ok((
        next,
        PromotionStats {
            manifests_verified: scrub.manifests_checked,
        },
    ))
}

/// Copy without following symlinks, then hash every file before the old store can be removed.
fn copy_and_verify_store(
    old_root: &Path,
    new_root: &Path,
    safety_buffer: u64,
) -> Result<CopyStats> {
    if !old_root.is_dir() {
        bail!("The current backup storage folder is missing or unavailable.");
    }
    if new_root.exists() && !new_root.is_dir() {
        bail!("The new storage location points to a file.");
    }
    fs::create_dir_all(new_root)
        .with_context(|| format!("failed to create new storage folder {:?}", new_root))?;
    if fs::read_dir(new_root)
        .with_context(|| format!("failed to inspect new storage folder {:?}", new_root))?
        .next()
        .is_some()
    {
        bail!("The new storage folder must be empty so existing files cannot be overwritten.");
    }

    let old_canonical = old_root
        .canonicalize()
        .with_context(|| format!("failed to resolve current storage folder {:?}", old_root))?;
    let new_canonical = new_root
        .canonicalize()
        .with_context(|| format!("failed to resolve new storage folder {:?}", new_root))?;
    if old_canonical == new_canonical
        || old_canonical.starts_with(&new_canonical)
        || new_canonical.starts_with(&old_canonical)
    {
        bail!("The new storage folder cannot be the current folder or one nested inside it.");
    }

    let mut files = Vec::<PathBuf>::new();
    let mut pending_dirs = vec![old_canonical.clone()];
    let mut total_bytes = 0_u64;
    while let Some(directory) = pending_dirs.pop() {
        for entry in fs::read_dir(&directory)
            .with_context(|| format!("failed to read backup directory {:?}", directory))?
        {
            let entry =
                entry.with_context(|| format!("failed to read entry in {:?}", directory))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .with_context(|| format!("failed to inspect backup entry {:?}", path))?;
            if metadata.file_type().is_symlink() {
                bail!("Backup storage contains a symbolic link, so it was left unchanged.");
            }
            if metadata.is_dir() {
                pending_dirs.push(path);
            } else if metadata.is_file() {
                total_bytes = total_bytes.saturating_add(metadata.len());
                files.push(path);
            }
        }
    }

    let available = free_space(&new_canonical)
        .with_context(|| format!("failed to read free space at {:?}", new_canonical))?;
    let required = total_bytes.saturating_add(safety_buffer);
    if available < required {
        bail!(
            "The new storage location needs at least {} bytes free; only {} bytes are available.",
            required,
            available
        );
    }

    for source in &files {
        let relative = source
            .strip_prefix(&old_canonical)
            .with_context(|| format!("failed to resolve backup entry {:?}", source))?;
        let destination = new_canonical.join(relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create backup directory {:?}", parent))?;
        }
        fs::copy(source, &destination).with_context(|| {
            format!(
                "failed to copy backup file {:?} to {:?}",
                source, destination
            )
        })?;
        fs::File::open(&destination)
            .and_then(|file| file.sync_all())
            .with_context(|| format!("failed to flush copied backup file {:?}", destination))?;
        let source_hash = backup_core::sha256_file_hex(source)
            .with_context(|| format!("failed to verify current backup file {:?}", source))?;
        let destination_hash = backup_core::sha256_file_hex(&destination)
            .with_context(|| format!("failed to verify copied backup file {:?}", destination))?;
        if source_hash != destination_hash {
            bail!("Verification failed for copied backup file {:?}.", relative);
        }
    }

    Ok(CopyStats {
        files: files.len(),
        bytes: total_bytes,
    })
}

fn relocation_error(code: &str, context: &'static str, error: &anyhow::Error) -> ErrorEnvelope {
    ErrorEnvelope::from_anyhow_with_code(code, context, error)
}

#[tauri::command]
/// Pause writes, copy and verify the store, switch config, restart, then delete the old copy.
pub async fn relocate_destination_cmd(
    destination_id: String,
    new_path: String,
) -> Result<DestinationRelocationResult, ErrorEnvelope> {
    let original = config_api::get_config().map_err(|error| {
        relocation_error(
            "CONFIG_LOAD",
            "destination::relocate_destination_cmd failed to load config",
            &error,
        )
    })?;
    let destination_index = original
        .destinations
        .iter()
        .position(|destination| destination.id == destination_id)
        .ok_or_else(|| ErrorEnvelope::new("NOT_FOUND", "Storage location no longer exists."))?;
    let old_path = original.destinations[destination_index].path.clone();
    let new_path = PathBuf::from(new_path);
    if old_path == new_path {
        return Err(ErrorEnvelope::new(
            "INVALID_INPUT",
            "Choose a different storage location.",
        ));
    }
    if new_path.parent().is_none() {
        return Err(ErrorEnvelope::new(
            "INVALID_INPUT",
            "Cannot use a filesystem root as backup storage.",
        ));
    }
    let existing_destination_index = original
        .destinations
        .iter()
        .enumerate()
        .find(|(index, destination)| {
            *index != destination_index
                && paths_refer_to_same_location(&destination.path, &new_path)
        })
        .map(|(index, _)| index);
    if existing_destination_index.is_some() && destination_index != 0 {
        return Err(ErrorEnvelope::new(
            "CONFLICT",
            "Only Main storage can be switched with an existing secondary backup location.",
        ));
    }

    let mut paused = original.clone();
    paused.safe_mode = true;
    config_api::save_config(&paused).map_err(|error| {
        relocation_error(
            "CONFIG_SAVE",
            "destination::relocate_destination_cmd failed to pause configuration",
            &error,
        )
    })?;
    if let Err(error) = status_api::set_safe_mode_with_correlation(true, None).await {
        let daemon_offline = error.to_string().contains("socket not found");
        if !daemon_offline {
            let _ = config_api::save_config(&original);
            return Err(relocation_error(
                "STATUS_UNAVAILABLE",
                "destination::relocate_destination_cmd could not pause active backups",
                &error,
            ));
        }
    }

    if let Some(secondary_index) = existing_destination_index {
        let verification_config = original.clone();
        let verification = tokio::task::spawn_blocking(move || {
            verify_secondary_can_be_main(&verification_config, secondary_index)
        })
        .await;
        let (next, stats) = match verification {
            Ok(Ok(verified)) => verified,
            Ok(Err(error)) => {
                let _ = config_api::save_config(&original);
                let _ = status_api::set_safe_mode_with_correlation(original.safe_mode, None).await;
                return Err(relocation_error(
                    "STORAGE_PROMOTION_FAILED",
                    "The existing secondary copy could not be verified; storage roles are unchanged",
                    &error,
                ));
            }
            Err(error) => {
                let _ = config_api::save_config(&original);
                let _ = status_api::set_safe_mode_with_correlation(original.safe_mode, None).await;
                return Err(ErrorEnvelope::new(
                    "BLOCKING_TASK_FAILED",
                    format!(
                        "Storage verification stopped unexpectedly; storage roles are unchanged: {error}"
                    ),
                ));
            }
        };

        if let Err(error) = config_api::save_config(&next) {
            let _ = config_api::save_config(&original);
            let _ = status_api::set_safe_mode_with_correlation(original.safe_mode, None).await;
            return Err(relocation_error(
                "CONFIG_SAVE",
                "The secondary copy was verified but could not be promoted; storage roles are unchanged",
                &error,
            ));
        }

        let restart_result =
            tokio::task::spawn_blocking(|| crate::commands::service::restart_daemon_cmd(None))
                .await;
        if !matches!(restart_result, Ok(Ok(_))) {
            let rollback_save = config_api::save_config(&original);
            let rollback_restart =
                tokio::task::spawn_blocking(|| crate::commands::service::restart_daemon_cmd(None))
                    .await;
            let restart_reason = match restart_result {
                Ok(Err(error)) => error.message,
                Err(error) => error.to_string(),
                Ok(Ok(_)) => unreachable!(),
            };
            let rollback_warning = if rollback_save.is_err()
                || !matches!(rollback_restart, Ok(Ok(_)))
            {
                " Automatic rollback also needs attention; leave Backup Sync paused and inspect the configured paths before continuing."
            } else {
                " The original storage roles were restored."
            };
            return Err(ErrorEnvelope::new(
                "ACTIVATION_FAILED",
                format!(
                    "The selected copy verified correctly, but the background service did not accept the role change: {restart_reason}.{rollback_warning}"
                ),
            ));
        }

        return Ok(DestinationRelocationResult {
            config: next,
            operation: "promoted_existing_secondary",
            files_moved: 0,
            bytes_moved: 0,
            old_location_removed: false,
            warning: Some(format!(
                "Verified {} recovery manifest(s). The selected secondary copy is now Main storage; the previous Main storage remains intact as a secondary backup.",
                stats.manifests_verified
            )),
        });
    }

    let safety_buffer = original.execution.free_space_safety_buffer_bytes;
    let copy_old = old_path.clone();
    let copy_new = new_path.clone();
    let copy_result = tokio::task::spawn_blocking(move || {
        copy_and_verify_store(&copy_old, &copy_new, safety_buffer)
    })
    .await;
    let stats = match copy_result {
        Ok(Ok(stats)) => stats,
        Ok(Err(error)) => {
            let _ = config_api::save_config(&original);
            let _ = status_api::set_safe_mode_with_correlation(original.safe_mode, None).await;
            return Err(relocation_error(
                "STORAGE_MOVE_FAILED",
                "New storage could not be copied and verified; the old location is unchanged",
                &error,
            ));
        }
        Err(error) => {
            let _ = config_api::save_config(&original);
            let _ = status_api::set_safe_mode_with_correlation(original.safe_mode, None).await;
            return Err(ErrorEnvelope::new(
                "BLOCKING_TASK_FAILED",
                format!(
                    "Storage move stopped unexpectedly; the old location is unchanged: {error}"
                ),
            ));
        }
    };

    let mut next = original.clone();
    next.destinations[destination_index].path = new_path.clone();
    if destination_index == 0 {
        next.backup_root = new_path.clone();
    }
    if let Err(error) = config_api::save_config(&next) {
        let _ = config_api::save_config(&original);
        let _ = status_api::set_safe_mode_with_correlation(original.safe_mode, None).await;
        return Err(relocation_error(
            "CONFIG_SAVE",
            "New storage was verified but could not be activated; the old location is unchanged",
            &error,
        ));
    }

    let restart_result =
        tokio::task::spawn_blocking(|| crate::commands::service::restart_daemon_cmd(None)).await;
    if !matches!(restart_result, Ok(Ok(_))) {
        let restart_reason = match restart_result {
            Ok(Err(error)) => error.message,
            Err(error) => error.to_string(),
            Ok(Ok(_)) => unreachable!(),
        };
        let rollback_save = config_api::save_config(&original);
        let rollback_restart =
            tokio::task::spawn_blocking(|| crate::commands::service::restart_daemon_cmd(None))
                .await;
        let rollback_warning = if rollback_save.is_err() || !matches!(rollback_restart, Ok(Ok(_))) {
            " Automatic rollback also needs attention; leave Backup Sync paused and inspect the configured paths before continuing."
        } else {
            " The original storage location was restored. The verified new copy was kept and can be removed manually after inspection."
        };
        return Err(ErrorEnvelope::new(
            "ACTIVATION_FAILED",
            format!(
                "The new copy verified correctly, but the background service did not accept the storage change: {restart_reason}.{rollback_warning}"
            ),
        ));
    }

    let mut old_location_removed = false;
    let remove_old = old_path.clone();
    let warning = match tokio::task::spawn_blocking(move || fs::remove_dir_all(&remove_old)).await {
        Ok(Ok(())) => {
            old_location_removed = true;
            None
        }
        Ok(Err(error)) => Some(format!(
            "The new location is active and verified. The old backup files could not be removed automatically and remain at {}: {}",
            old_path.display(),
            error
        )),
        Err(error) => Some(format!(
            "The new location is active and verified. The old backup files remain at {} because cleanup did not complete: {}",
            old_path.display(),
            error
        )),
    };

    Ok(DestinationRelocationResult {
        config: next,
        operation: "moved_to_new_location",
        files_moved: stats.files,
        bytes_moved: stats.bytes,
        old_location_removed,
        warning,
    })
}

#[cfg(target_os = "macos")]
/// Prevent auto-creating fake mount folders when an external drive is disconnected.
fn missing_macos_mount_root(path: &Path) -> Option<PathBuf> {
    let volumes_root = Path::new("/Volumes");
    let relative = match path.strip_prefix(volumes_root) {
        Ok(relative) => relative,
        Err(_) => return None,
    };
    let mut components = relative.components();
    let volume_name = match components.next()? {
        Component::Normal(name) => name,
        _ => return None,
    };
    let mount_root = volumes_root.join(volume_name);
    if mount_root.exists() {
        None
    } else {
        Some(mount_root)
    }
}

#[tauri::command]
/// Preflight destination settings before saving config.
pub fn check_destination_cmd(path: String) -> Result<DestinationCheck, ErrorEnvelope> {
    if path.trim().is_empty() {
        return Ok(DestinationCheck {
            writable: false,
            free_bytes: None,
            message: "Pick a backup destination.".into(),
        });
    }
    let p = PathBuf::from(path);
    if p.parent().is_none() {
        return Ok(DestinationCheck {
            writable: false,
            free_bytes: None,
            message: "Cannot use filesystem root. Pick a folder inside your home directory.".into(),
        });
    }
    if p.exists() && p.is_file() {
        return Ok(DestinationCheck {
            writable: false,
            free_bytes: None,
            message: "Destination points to a file; choose a folder.".into(),
        });
    }
    if !p.exists() {
        #[cfg(target_os = "macos")]
        if let Some(missing_mount_root) = missing_macos_mount_root(&p) {
            return Ok(DestinationCheck {
                writable: false,
                free_bytes: None,
                message: format!(
                    "Destination drive is not mounted at {}. Reconnect it and try again.",
                    missing_mount_root.display()
                ),
            });
        }

        if let Err(error) = run_blocking_io(
            "gui::destination::check_destination_cmd create destination",
            || {
                std::fs::create_dir_all(&p).with_context(|| {
                    format!(
                        "destination::check_destination_cmd failed creating destination {:?}",
                        p
                    )
                })
            },
        ) {
            return Ok(DestinationCheck {
                writable: false,
                free_bytes: None,
                message: format!(
                    "Could not create destination folder automatically: {}",
                    error
                ),
            });
        }
    }
    if !p.is_dir() {
        return Ok(DestinationCheck {
            writable: false,
            free_bytes: None,
            message: "Destination must be a folder.".into(),
        });
    }
    match free_space(&p) {
        Ok(free) => Ok(DestinationCheck {
            writable: true,
            free_bytes: Some(free),
            message: format!("Writable. Free space: {} bytes", free),
        }),
        Err(e) => Ok(DestinationCheck {
            writable: false,
            free_bytes: None,
            message: format!("Cannot read free space: {}", e),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        check_destination_cmd, copy_and_verify_store, swap_main_with_secondary,
        verify_secondary_can_be_main,
    };
    use backup_core::backup::versioned::{version_index_path, VersionIndex, VersionInfo};
    use backup_core::config::model::{Destination, WatchedKind, WatchedPath};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Keep tests deterministic without adding external dependencies.
    fn unique_temp_path(label: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("backup-sync-gui-{label}-{timestamp}"))
    }

    #[test]
    fn creates_missing_destination_folder_automatically() {
        let destination = unique_temp_path("destination-create")
            .join("missing")
            .join("nested");
        if destination.exists() {
            if let Err(error) = std::fs::remove_dir_all(&destination) {
                panic!(
                    "destination::tests::creates_missing_destination_folder_automatically failed to clear destination {:?}: {}",
                    destination,
                    error
                );
            }
        }

        let result =
            check_destination_cmd(destination.display().to_string()).expect("check should succeed");
        assert!(
            result.writable,
            "expected writable result, got: {}",
            result.message
        );
        assert!(
            destination.is_dir(),
            "destination directory should be created"
        );

        let cleanup_root = destination
            .ancestors()
            .nth(2)
            .map(PathBuf::from)
            .unwrap_or(destination);
        if let Err(error) = std::fs::remove_dir_all(&cleanup_root) {
            panic!(
                "destination::tests::creates_missing_destination_folder_automatically failed to cleanup {:?}: {}",
                cleanup_root,
                error
            );
        }
    }

    #[test]
    fn copies_and_verifies_every_backup_file_before_cleanup_is_allowed() {
        let root = unique_temp_path("relocation-copy");
        let old = root.join("old");
        let new = root.join("new");
        std::fs::create_dir_all(old.join("nested")).expect("old tree should be created");
        std::fs::write(old.join("manifest.json"), b"manifest").expect("manifest should be written");
        std::fs::write(old.join("nested").join("blob"), b"backup-bytes")
            .expect("blob should be written");

        let stats = copy_and_verify_store(&old, &new, 0).expect("copy should verify");

        assert_eq!(stats.files, 2);
        assert_eq!(stats.bytes, 20);
        assert_eq!(
            std::fs::read(new.join("nested").join("blob")).expect("copied blob should exist"),
            b"backup-bytes"
        );
        assert!(
            old.exists(),
            "copy verification must never delete the old store"
        );
        std::fs::remove_dir_all(root).expect("test tree should be removed");
    }

    #[test]
    fn refuses_to_overwrite_a_nonempty_new_storage_folder() {
        let root = unique_temp_path("relocation-conflict");
        let old = root.join("old");
        let new = root.join("new");
        std::fs::create_dir_all(&old).expect("old directory should be created");
        std::fs::create_dir_all(&new).expect("new directory should be created");
        std::fs::write(old.join("backup"), b"safe").expect("backup should be written");
        std::fs::write(new.join("unrelated"), b"keep").expect("target file should be written");

        let error = copy_and_verify_store(&old, &new, 0).expect_err("nonempty target must fail");

        assert!(error.to_string().contains("must be empty"));
        assert_eq!(
            std::fs::read(new.join("unrelated")).expect("target file must remain"),
            b"keep"
        );
        std::fs::remove_dir_all(root).expect("test tree should be removed");
    }

    fn two_destination_config(root: &std::path::Path) -> backup_core::Config {
        let main = root.join("main");
        let secondary = root.join("secondary");
        let source = root.join("source");
        std::fs::create_dir_all(&main).expect("main storage should be created");
        std::fs::create_dir_all(&secondary).expect("secondary storage should be created");
        std::fs::create_dir_all(&source).expect("source should be created");
        let mut cfg = backup_core::config::load::default_config().expect("default config");
        cfg.backup_root = main.clone();
        cfg.destinations = vec![
            Destination {
                id: "main".into(),
                path: main,
                label: Some("Main".into()),
                max_backups_per_file: None,
                replicate_to: vec!["secondary".into()],
            },
            Destination {
                id: "secondary".into(),
                path: secondary,
                label: Some("Secondary".into()),
                max_backups_per_file: None,
                replicate_to: vec![],
            },
        ];
        cfg.watched = vec![
            WatchedPath {
                path: source.clone(),
                kind: WatchedKind::Directory,
                enabled: true,
                destination_id: "main".into(),
                max_backups_per_file: None,
            },
            WatchedPath {
                path: source,
                kind: WatchedKind::Directory,
                enabled: true,
                destination_id: "secondary".into(),
                max_backups_per_file: None,
            },
        ];
        cfg
    }

    #[test]
    fn swaps_only_paths_and_preserves_ids_source_mappings_and_files() {
        let root = unique_temp_path("promotion-swap");
        let cfg = two_destination_config(&root);
        std::fs::write(cfg.destinations[0].path.join("main-sentinel"), b"main")
            .expect("main sentinel");
        std::fs::write(
            cfg.destinations[1].path.join("secondary-sentinel"),
            b"secondary",
        )
        .expect("secondary sentinel");

        let next = swap_main_with_secondary(&cfg, 1).expect("role swap should validate");

        assert_eq!(next.destinations[0].id, "main");
        assert_eq!(next.destinations[1].id, "secondary");
        assert_eq!(next.destinations[0].path, cfg.destinations[1].path);
        assert_eq!(next.destinations[1].path, cfg.destinations[0].path);
        assert_eq!(next.backup_root, cfg.destinations[1].path);
        assert_eq!(next.watched[0].destination_id, "main");
        assert_eq!(next.watched[1].destination_id, "secondary");
        assert_eq!(next.destinations[0].replicate_to, vec!["secondary"]);
        assert_eq!(
            std::fs::read(cfg.destinations[0].path.join("main-sentinel")).expect("main remains"),
            b"main"
        );
        assert_eq!(
            std::fs::read(cfg.destinations[1].path.join("secondary-sentinel"))
                .expect("secondary remains"),
            b"secondary"
        );
        std::fs::remove_dir_all(root).expect("test tree should be removed");
    }

    #[test]
    fn verifies_empty_stores_without_moving_or_deleting_either_location() {
        let root = unique_temp_path("promotion-verify");
        let cfg = two_destination_config(&root);

        let (next, stats) =
            verify_secondary_can_be_main(&cfg, 1).expect("empty initialized stores are safe");

        assert_eq!(stats.manifests_verified, 0);
        assert_eq!(next.destinations[0].path, cfg.destinations[1].path);
        assert!(cfg.destinations[0].path.is_dir());
        assert!(cfg.destinations[1].path.is_dir());
        std::fs::remove_dir_all(root).expect("test tree should be removed");
    }

    #[test]
    fn rejects_a_secondary_copy_older_than_main_without_touching_either_store() {
        let root = unique_temp_path("promotion-stale");
        let cfg = two_destination_config(&root);
        let source = &cfg.watched[0].path;
        let main_index = version_index_path(&cfg.destinations[0].path, source);
        let secondary_index = version_index_path(&cfg.destinations[1].path, source);
        std::fs::create_dir_all(main_index.parent().expect("main index parent"))
            .expect("main index tree");
        std::fs::create_dir_all(secondary_index.parent().expect("secondary index parent"))
            .expect("secondary index tree");
        let write_index = |path: &std::path::Path, timestamp| {
            let index = VersionIndex {
                schema_version: 1,
                source_path: source.display().to_string(),
                versions: vec![VersionInfo {
                    id: format!("version-{timestamp}"),
                    created_at_unix: timestamp,
                }],
                ..VersionIndex::default()
            };
            std::fs::write(path, serde_json::to_vec(&index).expect("serialize index"))
                .expect("write index");
        };
        write_index(&main_index, 200);
        write_index(&secondary_index, 100);

        let error = verify_secondary_can_be_main(&cfg, 1).expect_err("stale copy must fail");

        assert!(error.to_string().contains("behind Main storage"));
        assert!(main_index.is_file());
        assert!(secondary_index.is_file());
        std::fs::remove_dir_all(root).expect("test tree should be removed");
    }
}
