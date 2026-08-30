//! Human-readable, app-managed views of committed backup versions.
//!
//! The content-addressed `.backup_sync` store remains authoritative for integrity,
//! deduplication, and recovery. This module materializes one independent, readable tree per
//! committed version so opening a configured destination does not expose implementation details.

use super::model::{VersionIndex, VersionInfo};
use super::restore::build_version_tree_from_store;
use super::store::{sources_root, store_root};
use super::validation::validate_version_index;
use crate::config::model::{Config, WatchedKind, WatchedPath};
use crate::hashing;
use anyhow::{Context, Result};
use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const SOURCE_MARKER_NAME: &str = ".backup-sync-source.json";
const VERSION_MARKER_NAME: &str = ".backup-sync-version.json";
const LATEST_NAME: &str = "Latest";
const PREVIOUS_NAME: &str = "Previous Versions";
const MARKER_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct SourceMarker {
    schema_version: u32,
    source_path: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct VersionMarker {
    schema_version: u32,
    version_id: String,
}

/// Synchronize the readable view for one watched directory against its committed index.
///
/// The caller must already hold the destination's operation lease. Materialized trees contain
/// ordinary decoded files and never hard-link into the authoritative blob store.
pub(crate) fn sync_source_readable_view(
    cfg: &Config,
    watched: &WatchedPath,
    destination_root: &Path,
) -> Result<Option<PathBuf>> {
    if !watched.enabled || !matches!(watched.kind, WatchedKind::Directory) {
        return Ok(None);
    }
    // A decoded browse tree would silently defeat the product's at-rest encryption promise.
    // Encrypted stores remain available through the in-app recovery browser only.
    if cfg.encryption.enabled {
        remove_generated_plaintext_view(destination_root, &watched.path)?;
        return Ok(None);
    }

    let source_id = hashing::sha256_hex(watched.path.to_string_lossy().as_bytes());
    let source_store = sources_root(&store_root(destination_root)).join(source_id);
    let index_path = source_store.join("index.json");
    if !index_path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&index_path).with_context(|| {
        format!(
            "versioned::sync_source_readable_view failed to read {:?}",
            index_path
        )
    })?;
    let index: VersionIndex = serde_json::from_str(&raw).with_context(|| {
        format!(
            "versioned::sync_source_readable_view failed to parse {:?}",
            index_path
        )
    })?;
    validate_version_index(&index).with_context(|| {
        format!(
            "versioned::sync_source_readable_view rejected unsafe {:?}",
            index_path
        )
    })?;
    if index.source_path != watched.path.to_string_lossy() {
        anyhow::bail!(
            "versioned::sync_source_readable_view source index does not match configured source"
        );
    }
    if index.versions.is_empty() {
        return Ok(None);
    }

    fs::create_dir_all(destination_root).with_context(|| {
        format!(
            "versioned::sync_source_readable_view failed to create destination {:?}",
            destination_root
        )
    })?;
    let source_view = find_or_create_source_view(destination_root, &watched.path)?;
    let previous_root = source_view.join(PREVIOUS_NAME);
    ensure_real_directory(&previous_root, "previous versions")?;

    let latest = index
        .versions
        .last()
        .context("versioned::sync_source_readable_view committed index unexpectedly empty")?;
    let previous: Vec<&VersionInfo> = index
        .versions
        .iter()
        .filter(|version| version.id != latest.id)
        .collect();
    let previous_ids: HashSet<&str> = previous.iter().map(|version| version.id.as_str()).collect();

    // Move the old readable latest into history before materializing the new latest. This keeps
    // exactly one readable copy of each version rather than duplicating the newest snapshot.
    let latest_path = source_view.join(LATEST_NAME);
    if latest_path.exists() {
        match read_version_marker(&latest_path)? {
            Some(marker) if marker.version_id == latest.id => {}
            Some(marker) if previous_ids.contains(marker.version_id.as_str()) => {
                let version = previous
                    .iter()
                    .find(|version| version.id == marker.version_id)
                    .copied()
                    .context(
                        "versioned::sync_source_readable_view missing prior version metadata",
                    )?;
                let target = available_version_path(&previous_root, version)?;
                fs::rename(&latest_path, &target).with_context(|| {
                    format!(
                        "versioned::sync_source_readable_view failed to archive {:?} as {:?}",
                        latest_path, target
                    )
                })?;
            }
            Some(_) => remove_managed_version_tree(&latest_path)?,
            None => {
                anyhow::bail!(
                    "versioned::sync_source_readable_view cannot replace unowned {:?}",
                    latest_path
                );
            }
        }
    }

    let mut existing_previous = previous_versions_by_id(&previous_root)?;
    for version in previous {
        if let Some(existing) = existing_previous.get(version.id.as_str()).cloned() {
            let preferred = previous_root.join(readable_version_name(version));
            if existing != preferred && !preferred.exists() {
                fs::rename(&existing, &preferred).with_context(|| {
                    format!(
                        "versioned::sync_source_readable_view failed to normalize readable version name {:?} -> {:?}",
                        existing, preferred
                    )
                })?;
                existing_previous.insert(version.id.clone(), preferred);
            }
            continue;
        }
        let target = available_version_path(&previous_root, version)?;
        materialize_managed_version(cfg, destination_root, &watched.path, version, &target)?;
        existing_previous.insert(version.id.clone(), target);
    }

    for (version_id, path) in existing_previous {
        if !previous_ids.contains(version_id.as_str()) {
            remove_managed_version_tree(&path)?;
        }
    }

    if !latest_path.exists() {
        materialize_managed_version(cfg, destination_root, &watched.path, latest, &latest_path)?;
    }

    Ok(Some(source_view))
}

/// Remove only the readable trees owned by Backup Sync for one source.
///
/// User-created files inside the source view are deliberately preserved. When no unowned content
/// remains, the now-empty source container is removed too so deleting protection does not leave a
/// misleading backup folder behind.
pub(crate) fn remove_generated_plaintext_view(
    destination_root: &Path,
    source_path: &Path,
) -> Result<()> {
    if !destination_root.is_dir() {
        return Ok(());
    }
    let Some(source_view) = find_source_view(destination_root, source_path)? else {
        return Ok(());
    };

    let latest = source_view.join(LATEST_NAME);
    if latest.exists() && read_version_marker(&latest)?.is_some() {
        remove_managed_version_tree(&latest)?;
    }
    let previous = source_view.join(PREVIOUS_NAME);
    if previous.is_dir() {
        for path in previous_versions_by_id(&previous)?.into_values() {
            remove_managed_version_tree(&path)?;
        }
        if fs::read_dir(&previous)?.next().is_none() {
            fs::remove_dir(&previous).with_context(|| {
                format!(
                    "versioned::remove_generated_plaintext_view failed to remove empty {:?}",
                    previous
                )
            })?;
        }
    }

    let remaining: Vec<PathBuf> = fs::read_dir(&source_view)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<_>>()?;
    let marker = source_view.join(SOURCE_MARKER_NAME);
    if remaining.iter().all(|path| path == &marker) {
        fs::remove_file(&marker).with_context(|| {
            format!(
                "versioned::remove_generated_plaintext_view failed to remove marker {:?}",
                marker
            )
        })?;
        fs::remove_dir(&source_view).with_context(|| {
            format!(
                "versioned::remove_generated_plaintext_view failed to remove empty {:?}",
                source_view
            )
        })?;
    }
    Ok(())
}

fn find_source_view(destination_root: &Path, source_path: &Path) -> Result<Option<PathBuf>> {
    for entry in fs::read_dir(destination_root)? {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            continue;
        }
        if read_source_marker(&path)?
            .map(|marker| marker.source_path == source_path.to_string_lossy())
            .unwrap_or(false)
        {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn find_or_create_source_view(destination_root: &Path, source_path: &Path) -> Result<PathBuf> {
    if let Some(existing) = find_source_view(destination_root, source_path).with_context(|| {
        format!(
            "versioned::find_or_create_source_view failed to inspect {:?}",
            destination_root
        )
    })? {
        return Ok(existing);
    }

    let base_name = readable_source_name(source_path);
    let source_hash = hashing::sha256_hex(source_path.to_string_lossy().as_bytes());
    let short_hash = source_hash.get(..8).unwrap_or(source_hash.as_str());
    let preferred = destination_root.join(&base_name);
    let target = if preferred.exists() {
        destination_root.join(format!("{base_name} — {short_hash}"))
    } else {
        preferred
    };
    if target.exists() {
        anyhow::bail!(
            "versioned::find_or_create_source_view readable source path is already occupied: {:?}",
            target
        );
    }

    let stage = destination_root.join(format!(
        ".backup-sync-source-stage-{}-{}",
        std::process::id(),
        short_hash
    ));
    if stage.exists() {
        anyhow::bail!(
            "versioned::find_or_create_source_view source staging path already exists: {:?}",
            stage
        );
    }
    fs::create_dir(&stage).with_context(|| {
        format!(
            "versioned::find_or_create_source_view failed to create staging directory {:?}",
            stage
        )
    })?;
    let marker = SourceMarker {
        schema_version: MARKER_SCHEMA_VERSION,
        source_path: source_path.to_string_lossy().into_owned(),
    };
    if let Err(error) = write_json_marker(&stage.join(SOURCE_MARKER_NAME), &marker)
        .and_then(|_| fs::rename(&stage, &target).map_err(anyhow::Error::from))
    {
        let _ = fs::remove_dir_all(&stage);
        return Err(error).with_context(|| {
            format!(
                "versioned::find_or_create_source_view failed to publish {:?}",
                target
            )
        });
    }
    Ok(target)
}

fn ensure_real_directory(path: &Path, label: &str) -> Result<()> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            anyhow::bail!(
                "versioned::ensure_real_directory {label} path is not a real directory: {:?}",
                path
            );
        }
        return Ok(());
    }
    fs::create_dir(path).with_context(|| {
        format!(
            "versioned::ensure_real_directory failed to create {label} directory {:?}",
            path
        )
    })
}

fn materialize_managed_version(
    cfg: &Config,
    destination_root: &Path,
    source_path: &Path,
    version: &VersionInfo,
    target: &Path,
) -> Result<()> {
    if target.exists() {
        anyhow::bail!(
            "versioned::materialize_managed_version target already exists: {:?}",
            target
        );
    }
    let parent = target
        .parent()
        .context("versioned::materialize_managed_version target has no parent")?;
    let source_hash = hashing::sha256_hex(source_path.to_string_lossy().as_bytes());
    let short_source = source_hash.get(..8).unwrap_or(source_hash.as_str());
    let short_version = version.id.get(..15).unwrap_or(version.id.as_str());
    let stage = parent.join(format!(
        ".backup-sync-version-stage-{}-{short_source}-{short_version}",
        std::process::id()
    ));
    if stage.exists() {
        anyhow::bail!(
            "versioned::materialize_managed_version staging path already exists: {:?}",
            stage
        );
    }

    let outcome = (|| -> Result<()> {
        build_version_tree_from_store(cfg, destination_root, source_path, &version.id, &stage)?;
        let marker = VersionMarker {
            schema_version: MARKER_SCHEMA_VERSION,
            version_id: version.id.clone(),
        };
        write_json_marker(&stage.join(VERSION_MARKER_NAME), &marker)?;
        fs::rename(&stage, target).with_context(|| {
            format!(
                "versioned::materialize_managed_version failed to publish {:?}",
                target
            )
        })?;
        Ok(())
    })();
    if outcome.is_err() && stage.exists() {
        let _ = fs::remove_dir_all(&stage);
    }
    outcome
}

fn previous_versions_by_id(previous_root: &Path) -> Result<HashMap<String, PathBuf>> {
    let mut versions = HashMap::new();
    for entry in fs::read_dir(previous_root)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
            continue;
        }
        if let Some(marker) = read_version_marker(&path)? {
            if versions.insert(marker.version_id.clone(), path).is_some() {
                anyhow::bail!(
                    "versioned::previous_versions_by_id duplicate readable version {}",
                    marker.version_id
                );
            }
        }
    }
    Ok(versions)
}

fn available_version_path(previous_root: &Path, version: &VersionInfo) -> Result<PathBuf> {
    let display_name = readable_version_name(version);
    let preferred = previous_root.join(&display_name);
    if !preferred.exists() {
        return Ok(preferred);
    }
    if read_version_marker(&preferred)?
        .map(|marker| marker.version_id == version.id)
        .unwrap_or(false)
    {
        return Ok(preferred);
    }
    let short_id = version.id.get(..15).unwrap_or(version.id.as_str());
    let alternate = previous_root.join(format!("{display_name} — {short_id}"));
    if alternate.exists() {
        anyhow::bail!(
            "versioned::available_version_path readable version path is occupied: {:?}",
            alternate
        );
    }
    Ok(alternate)
}

fn remove_managed_version_tree(path: &Path) -> Result<()> {
    if read_version_marker(path)?.is_none() {
        anyhow::bail!(
            "versioned::remove_managed_version_tree refused to remove unowned directory {:?}",
            path
        );
    }
    fs::remove_dir_all(path).with_context(|| {
        format!(
            "versioned::remove_managed_version_tree failed to remove {:?}",
            path
        )
    })
}

fn read_source_marker(root: &Path) -> Result<Option<SourceMarker>> {
    read_json_marker(&root.join(SOURCE_MARKER_NAME))
}

fn read_version_marker(root: &Path) -> Result<Option<VersionMarker>> {
    read_json_marker(&root.join(VERSION_MARKER_NAME))
}

fn read_json_marker<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        anyhow::bail!(
            "versioned::read_json_marker marker is not a regular file: {:?}",
            path
        );
    }
    let raw = fs::read_to_string(path)?;
    let marker = serde_json::from_str(&raw)
        .with_context(|| format!("versioned::read_json_marker failed to parse {:?}", path))?;
    Ok(Some(marker))
}

fn write_json_marker<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .context("versioned::write_json_marker marker has no parent")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(temp.as_file_mut(), value)?;
    use std::io::Write;
    temp.as_file_mut().write_all(b"\n")?;
    temp.as_file_mut().sync_all()?;
    temp.persist(path).map_err(|error| {
        anyhow::anyhow!(
            "versioned::write_json_marker failed to persist {:?}: {}",
            path,
            error
        )
    })?;
    Ok(())
}

fn readable_source_name(source_path: &Path) -> String {
    let raw = source_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("Backup");
    let cleaned: String = raw
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            other if other.is_control() => '-',
            other => other,
        })
        .collect();
    let cleaned = cleaned.trim_matches(|character: char| character == '.' || character == ' ');
    if cleaned.is_empty() {
        "Backup".to_string()
    } else {
        cleaned.to_string()
    }
}

fn readable_version_name(version: &VersionInfo) -> String {
    Local
        .timestamp_opt(version.created_at_unix, 0)
        .single()
        .map(|timestamp| timestamp.format("%Y-%m-%d at %H.%M.%S").to_string())
        .unwrap_or_else(|| version.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readable_source_name_uses_only_the_useful_final_component() {
        assert_eq!(
            readable_source_name(Path::new("/workspace/example/Projects/portfolio")),
            "portfolio"
        );
        assert_eq!(readable_source_name(Path::new("/")), "Backup");
    }
}
