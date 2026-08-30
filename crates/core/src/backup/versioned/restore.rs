use super::model::{Manifest, ManifestEntryKind, VersionInfo};
use super::operation_lock::acquire_store_lease;
use super::store::{blob_path, blobs_root, sources_root, store_root};
use super::validation::{validate_version_id, validate_version_index};
use crate::config::model::{Config, Destination, WatchedKind};
use crate::encryption::blobs::BlobCodec;
use crate::hashing;
use crate::io::BlockingIoPolicy;
use anyhow::{Context, Result};
use filetime::{set_file_mtime, FileTime};
use fs2::free_space;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreMode {
    InPlace,
    ToDirectory,
}

#[derive(Debug, Clone)]
pub struct RestoreRequest {
    pub source_path: PathBuf,
    pub version_id: String,
    pub mode: RestoreMode,
    pub target_dir: Option<PathBuf>,
}

#[derive(Debug, Default, Clone)]
pub struct RestoreResult {
    pub files_written: usize,
    pub files_removed: usize,
    pub dirs_created: usize,
}

#[derive(Debug, Clone)]
pub struct VersionFileInfo {
    pub rel_path: String,
    pub len: u64,
    pub mtime_unix: i64,
    pub mtime_nanos: u32,
    pub sha256: String,
}

#[derive(Debug, Clone)]
pub struct ListVersionFilesResult {
    pub total_files: usize,
    pub files: Vec<VersionFileInfo>,
}

#[derive(Debug, Clone)]
pub struct RestoreFilesRequest {
    pub source_path: PathBuf,
    pub version_id: String,
    pub rel_paths: Vec<String>,
    pub mode: RestoreMode,
    pub target_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct PlannedFile {
    rel_path: String,
    blob_path: PathBuf,
    expected_sha256: String,
    mtime_unix: i64,
    mtime_nanos: u32,
}

fn validate_relative_path(rel_path: &str) -> Result<()> {
    let bytes = rel_path.as_bytes();
    let has_windows_drive_prefix =
        bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if rel_path.is_empty() || rel_path.contains('\\') || has_windows_drive_prefix {
        anyhow::bail!(
            "versioned::validate_relative_path unsafe manifest path {:?}",
            rel_path
        );
    }

    let path = Path::new(rel_path);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::Prefix(_)
                    | Component::RootDir
                    | Component::ParentDir
                    | Component::CurDir
            )
        })
    {
        anyhow::bail!(
            "versioned::validate_relative_path unsafe manifest path {:?}",
            rel_path
        );
    }
    Ok(())
}

fn validate_content_hash(hash: &str, rel_path: &str) -> Result<()> {
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        anyhow::bail!(
            "versioned::validate_content_hash invalid sha256 for manifest path {:?}",
            rel_path
        );
    }
    Ok(())
}

pub(crate) fn validate_manifest(manifest: &Manifest) -> Result<()> {
    for (key, entry) in &manifest.entries {
        validate_relative_path(key)?;
        validate_relative_path(&entry.rel_path)?;
        if key != &entry.rel_path {
            anyhow::bail!(
                "versioned::validate_manifest entry key {:?} does not match rel_path {:?}",
                key,
                entry.rel_path
            );
        }
        if let Some(hash) = entry.sha256.as_deref() {
            validate_content_hash(hash, &entry.rel_path)?;
        }
    }
    Ok(())
}

/// Resolve the configured destination without touching store contents.
fn destination_root_for_source<'a>(cfg: &'a Config, source_path: &Path) -> Result<&'a Path> {
    let watched = cfg
        .watched
        .iter()
        .find(|watched| watched.enabled && watched.path == source_path)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "versioned::load_manifest_for_request source path not configured: {:?}",
                source_path
            )
        })?;
    if !matches!(watched.kind, WatchedKind::Directory) {
        anyhow::bail!(
            "versioned::load_manifest_for_request only directory watched paths are supported: {:?}",
            watched.path
        );
    }

    cfg.destinations
        .iter()
        .find(|destination| destination.id == watched.destination_id)
        .map(|destination| destination.path.as_path())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "versioned::load_manifest_for_request missing destination id {} for {:?}",
                watched.destination_id,
                watched.path
            )
        })
}

/// List committed versions for each enabled watched directory.
pub fn list_versions(cfg: &Config) -> Result<Vec<(PathBuf, Vec<VersionInfo>)>> {
    let destinations_by_id: HashMap<&str, &Destination> = cfg
        .destinations
        .iter()
        .map(|d| (d.id.as_str(), d))
        .collect();

    let mut out: Vec<(PathBuf, Vec<VersionInfo>)> = Vec::new();
    for watched in cfg.watched.iter().filter(|w| w.enabled) {
        if !matches!(watched.kind, WatchedKind::Directory) {
            continue;
        }
        let dest = destinations_by_id
            .get(watched.destination_id.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "versioned::list_versions missing destination id {} for watched path {}",
                    watched.destination_id,
                    watched.path.display()
                )
            })?;
        let store_root = store_root(&dest.path);
        let source_id = hashing::sha256_hex(watched.path.to_string_lossy().as_bytes());
        let index_path = sources_root(&store_root)
            .join(&source_id)
            .join("index.json");
        if !index_path.exists() {
            out.push((watched.path.clone(), Vec::new()));
            continue;
        }
        let raw = fs::read_to_string(&index_path)
            .with_context(|| format!("versioned::list_versions failed to read {:?}", index_path))?;
        let index: super::model::VersionIndex = serde_json::from_str(&raw).with_context(|| {
            format!("versioned::list_versions failed to parse {:?}", index_path)
        })?;
        validate_version_index(&index).with_context(|| {
            format!("versioned::list_versions rejected unsafe {:?}", index_path)
        })?;
        out.push((watched.path.clone(), index.versions));
    }
    Ok(out)
}

/// Resolve and load the manifest requested by a restore operation.
fn load_manifest_for_request(cfg: &Config, req: &RestoreRequest) -> Result<(Manifest, PathBuf)> {
    validate_version_id(&req.version_id)?;

    let destination_root = destination_root_for_source(cfg, &req.source_path)?;
    let store_root = store_root(destination_root);
    let source_id = hashing::sha256_hex(req.source_path.to_string_lossy().as_bytes());
    let manifests_root = sources_root(&store_root).join(&source_id).join("manifests");
    let manifest_path = manifests_root.join(format!("{}.json", req.version_id));
    let raw = fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "versioned::load_manifest_for_request failed to read manifest {:?}",
            manifest_path
        )
    })?;
    let manifest: Manifest = serde_json::from_str(&raw).with_context(|| {
        format!(
            "versioned::load_manifest_for_request failed to parse manifest {:?}",
            manifest_path
        )
    })?;
    validate_manifest(&manifest)?;
    Ok((manifest, store_root))
}

/// Build one committed version into an empty staging directory without taking a store lease.
///
/// This is intentionally crate-private: callers such as the human-readable backup view already
/// hold the destination lease and must not deadlock by entering the public restore path again.
/// The caller owns the final atomic rename of `stage_root`.
pub(crate) fn build_version_tree_from_store(
    cfg: &Config,
    destination_root: &Path,
    source_path: &Path,
    version_id: &str,
    stage_root: &Path,
) -> Result<RestoreResult> {
    validate_version_id(version_id)?;
    if stage_root.exists() {
        anyhow::bail!(
            "versioned::build_version_tree_from_store staging directory already exists: {:?}",
            stage_root
        );
    }

    let managed_store = store_root(destination_root);
    let source_id = hashing::sha256_hex(source_path.to_string_lossy().as_bytes());
    let manifest_path = sources_root(&managed_store)
        .join(source_id)
        .join("manifests")
        .join(format!("{version_id}.json"));
    let raw = fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "versioned::build_version_tree_from_store failed to read manifest {:?}",
            manifest_path
        )
    })?;
    let manifest: Manifest = serde_json::from_str(&raw).with_context(|| {
        format!(
            "versioned::build_version_tree_from_store failed to parse manifest {:?}",
            manifest_path
        )
    })?;
    validate_manifest(&manifest)?;
    if manifest.source_path != source_path.to_string_lossy() {
        anyhow::bail!(
            "versioned::build_version_tree_from_store manifest source does not match requested source"
        );
    }

    let parent = stage_root
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "versioned::build_version_tree_from_store failed to create staging parent {:?}",
            parent
        )
    })?;
    let managed_blobs = blobs_root(&managed_store);
    let (planned_files, _bytes) = preflight_restore(cfg, &manifest, &managed_blobs, parent)?;
    let blob_codec = BlobCodec::from_config(cfg)
        .context("versioned::build_version_tree_from_store failed to initialize blob codec")?;

    build_restore_tree(
        stage_root,
        &manifest,
        &planned_files,
        &blob_codec,
        cfg.hashing.timeout_seconds,
    )
}

/// Preflight a restore by checking blob availability and free-space requirements.
fn preflight_restore(
    cfg: &Config,
    manifest: &Manifest,
    blobs_root: &Path,
    free_space_dir: &Path,
) -> Result<(Vec<PlannedFile>, u64)> {
    let mut missing: Vec<String> = Vec::new();
    let mut planned: Vec<PlannedFile> = Vec::new();
    let mut total_bytes: u64 = 0;

    for entry in manifest.entries.values() {
        if entry.kind != ManifestEntryKind::File {
            continue;
        }
        let hash = match entry.sha256.as_deref() {
            None => {
                missing.push(format!("{}: missing sha256 in manifest", entry.rel_path));
                continue;
            }
            Some(h) => h,
        };
        let blob = blob_path(blobs_root, hash);
        if !blob.exists() {
            missing.push(format!("{}: missing blob {}", entry.rel_path, hash));
            continue;
        }
        total_bytes = total_bytes.saturating_add(entry.len);
        planned.push(PlannedFile {
            rel_path: entry.rel_path.clone(),
            blob_path: blob,
            expected_sha256: hash.to_string(),
            mtime_unix: entry.mtime_unix,
            mtime_nanos: entry.mtime_nanos,
        });
    }

    if !missing.is_empty() {
        anyhow::bail!(
            "versioned::preflight_restore cannot restore due to missing content:\n{}",
            missing.join("\n")
        );
    }

    ensure_restore_free_space(cfg, free_space_dir, total_bytes)?;
    Ok((planned, total_bytes))
}

/** Restores can write large amounts of data; failing mid-way due to disk full is unsafe. */
fn ensure_restore_free_space(cfg: &Config, dir: &Path, required_bytes: u64) -> Result<()> {
    let free = free_space(dir).with_context(|| {
        format!(
            "versioned::ensure_restore_free_space failed to read free space for {:?}",
            dir
        )
    })?;
    let needed = required_bytes.saturating_add(cfg.execution.free_space_safety_buffer_bytes);
    if free < needed {
        anyhow::bail!(
            "versioned::ensure_restore_free_space insufficient space at {:?} (need {} bytes incl. safety buffer, have {})",
            dir,
            needed,
            free
        );
    }
    if let Some(min_free) = cfg.min_free_space_bytes {
        if free < min_free {
            anyhow::bail!(
                "versioned::ensure_restore_free_space free space {} below configured minimum {} at {:?}",
                free,
                min_free,
                dir
            );
        }
    }
    Ok(())
}

/** Building a complete restore tree first enables atomic swap and prevents partial restores. */
fn build_restore_tree(
    stage_root: &Path,
    manifest: &Manifest,
    planned_files: &[PlannedFile],
    blob_codec: &BlobCodec,
    blob_timeout_seconds: u64,
) -> Result<RestoreResult> {
    let mut result = RestoreResult::default();
    fs::create_dir_all(stage_root).with_context(|| {
        format!(
            "versioned::build_restore_tree failed to create stage root {:?}",
            stage_root
        )
    })?;

    for entry in manifest.entries.values() {
        if entry.kind != ManifestEntryKind::Dir {
            continue;
        }
        let dir_path = stage_root.join(&entry.rel_path);
        if !dir_path.exists() {
            fs::create_dir_all(&dir_path).with_context(|| {
                format!(
                    "versioned::build_restore_tree failed to create directory {:?}",
                    dir_path
                )
            })?;
            result.dirs_created += 1;
        }
    }

    for pf in planned_files {
        let out_path = stage_root.join(&pf.rel_path);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "versioned::build_restore_tree failed to create parent dir {:?}",
                    parent
                )
            })?;
        }
        write_file_atomic_from_blob(
            &pf.blob_path,
            &out_path,
            &pf.expected_sha256,
            blob_codec,
            blob_timeout_seconds,
        )?;
        restore_mtime(&out_path, pf.mtime_unix, pf.mtime_nanos)?;
        result.files_written += 1;
    }

    Ok(result)
}

/** Transactional restore swaps whole trees; this preserves useful reporting without risky deletes. */
fn count_extraneous_files(root: &Path, manifest: &Manifest) -> Result<usize> {
    let expected_files: HashSet<&str> = manifest
        .entries
        .values()
        .filter(|e| e.kind == ManifestEntryKind::File)
        .map(|e| e.rel_path.as_str())
        .collect();

    let mut extraneous = 0usize;
    for item in walkdir::WalkDir::new(root).min_depth(1).follow_links(false) {
        let item = item.with_context(|| {
            format!(
                "versioned::count_extraneous_files failed walking restore root {:?}",
                root
            )
        })?;
        if !item.file_type().is_file() {
            continue;
        }
        let rel = item.path().strip_prefix(root).unwrap_or(item.path());
        let rel_s = rel.to_string_lossy().replace('\\', "/");
        if !expected_files.contains(rel_s.as_str()) {
            extraneous += 1;
        }
    }
    Ok(extraneous)
}

/** Renaming a fully-built tree into place is the closest thing to transactional restore. */
fn swap_staged_tree_into_place(
    stage_root: &Path,
    target_root: &Path,
    token: &str,
) -> Result<Option<PathBuf>> {
    let parent = target_root
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let backup_root = parent.join(format!(".backup_sync_restore_prev_{token}"));

    if target_root.exists() {
        fs::rename(target_root, &backup_root).with_context(|| {
            format!(
                "versioned::swap_staged_tree_into_place failed to move existing target {:?} to {:?}",
                target_root, backup_root
            )
        })?;
    }

    if let Err(e) = fs::rename(stage_root, target_root) {
        // Best-effort rollback if we already moved the original aside.
        if backup_root.exists() && !target_root.exists() {
            if let Err(rollback_error) = fs::rename(&backup_root, target_root) {
                tracing::warn!(
                    backup_root = %backup_root.display(),
                    target_root = %target_root.display(),
                    error = %rollback_error,
                    "versioned::swap_staged_tree_into_place failed rollback rename after staging move failure"
                );
            }
        }
        return Err(anyhow::anyhow!(
            "versioned::swap_staged_tree_into_place failed to move staged restore {:?} into {:?}: {}",
            stage_root,
            target_root,
            e
        ));
    }

    if backup_root.exists() {
        Ok(Some(backup_root))
    } else {
        Ok(None)
    }
}

/** Restore operations must not collide across concurrent runs. */
fn restore_token() -> String {
    format!(
        "{}-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S-%f"),
        std::process::id()
    )
}

pub fn list_version_files(
    cfg: &Config,
    source_path: &Path,
    version_id: &str,
    query: Option<&str>,
    limit: usize,
) -> Result<ListVersionFilesResult> {
    // Restoring individual files requires discoverability of paths inside manifests.
    let req = RestoreRequest {
        source_path: source_path.to_path_buf(),
        version_id: version_id.to_string(),
        mode: RestoreMode::InPlace,
        target_dir: None,
    };
    let (manifest, _store_root) = load_manifest_for_request(cfg, &req)?;

    let norm_query = query
        .map(|q| q.trim())
        .filter(|q| !q.is_empty())
        .map(|q| q.to_ascii_lowercase());

    let mut total_files: usize = 0;
    let mut files: Vec<VersionFileInfo> = Vec::new();
    let cap = limit.max(1);

    for entry in manifest.entries.values() {
        if entry.kind != ManifestEntryKind::File {
            continue;
        }
        let Some(sha256) = entry.sha256.as_deref() else {
            continue;
        };
        total_files = total_files.saturating_add(1);
        if let Some(q) = norm_query.as_deref() {
            if !entry.rel_path.to_ascii_lowercase().contains(q) {
                continue;
            }
        }
        if files.len() >= cap {
            continue;
        }
        files.push(VersionFileInfo {
            rel_path: entry.rel_path.clone(),
            len: entry.len,
            mtime_unix: entry.mtime_unix,
            mtime_nanos: entry.mtime_nanos,
            sha256: sha256.to_string(),
        });
    }

    Ok(ListVersionFilesResult { total_files, files })
}

pub fn restore_files(cfg: &Config, req: &RestoreFilesRequest) -> Result<RestoreResult> {
    // File-level restore is a common UX need without the risk of full in-place swaps.
    if req.rel_paths.is_empty() {
        anyhow::bail!("versioned::restore_files requires at least one rel_path");
    }
    validate_version_id(&req.version_id)?;

    let destination_root = destination_root_for_source(cfg, &req.source_path)?;
    let lock_policy = BlockingIoPolicy::from_config(cfg);
    let _operation_lease = acquire_store_lease(destination_root, &lock_policy)
        .context("versioned::restore_files could not serialize the destination store")?;

    let manifest_req = RestoreRequest {
        source_path: req.source_path.clone(),
        version_id: req.version_id.clone(),
        mode: RestoreMode::InPlace,
        target_dir: None,
    };
    let (manifest, store_root) = load_manifest_for_request(cfg, &manifest_req)?;
    let blobs_root = blobs_root(&store_root);

    let restore_root = match req.mode {
        RestoreMode::InPlace => req.source_path.clone(),
        RestoreMode::ToDirectory => req
            .target_dir
            .clone()
            .context("versioned::restore_files missing target_dir for ToDirectory")?,
    };
    validate_restore_root(&restore_root)?;

    let mut missing: Vec<String> = Vec::new();
    let mut planned: Vec<PlannedFile> = Vec::new();
    let mut total_bytes: u64 = 0;

    let mut seen_paths = HashSet::new();
    for rel_path in req.rel_paths.iter() {
        validate_relative_path(rel_path)?;
        if !seen_paths.insert(rel_path.as_str()) {
            continue;
        }
        let entry = match manifest.entries.get(rel_path) {
            None => {
                missing.push(format!("{rel_path}: not present in manifest"));
                continue;
            }
            Some(e) => e,
        };
        if entry.kind != ManifestEntryKind::File {
            missing.push(format!("{rel_path}: not a file entry"));
            continue;
        }
        let hash = match entry.sha256.as_deref() {
            None => {
                missing.push(format!("{rel_path}: missing sha256 in manifest"));
                continue;
            }
            Some(h) => h,
        };
        let blob = blob_path(&blobs_root, hash);
        if !blob.exists() {
            missing.push(format!("{rel_path}: missing blob {hash}"));
            continue;
        }
        total_bytes = total_bytes.saturating_add(entry.len);
        planned.push(PlannedFile {
            rel_path: rel_path.clone(),
            blob_path: blob,
            expected_sha256: hash.to_string(),
            mtime_unix: entry.mtime_unix,
            mtime_nanos: entry.mtime_nanos,
        });
    }

    if !missing.is_empty() {
        anyhow::bail!(
            "versioned::restore_files cannot restore due to missing content:\n{}",
            missing.join("\n")
        );
    }
    let restore_parent_path = restore_root
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let restore_parent = nearest_existing_directory(restore_parent_path)?;
    ensure_restore_free_space(cfg, &restore_parent, total_bytes)?;

    let blob_codec = BlobCodec::from_config(cfg)
        .context("versioned::restore_files failed to initialize blob codec")?;

    // Decode and authenticate every selected artifact before touching the target tree. The
    // temporary directory lives beside (or above) the target so the later per-file replacements
    // remain on the same filesystem.
    let stage = tempfile::Builder::new()
        .prefix(".backup_sync_restore_files_stage_")
        .tempdir_in(&restore_parent)
        .with_context(|| {
            format!(
                "versioned::restore_files failed to create staging directory in {:?}",
                restore_parent
            )
        })?;
    for pf in planned.iter() {
        let staged_path = stage.path().join(&pf.rel_path);
        if let Some(parent) = staged_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "versioned::restore_files failed to create staged parent {:?}",
                    parent
                )
            })?;
        }
        write_file_atomic_from_blob(
            &pf.blob_path,
            &staged_path,
            &pf.expected_sha256,
            &blob_codec,
            cfg.hashing.timeout_seconds,
        )?;
        restore_mtime(&staged_path, pf.mtime_unix, pf.mtime_nanos)?;
    }

    // Resolve the complete selected set before committing any file. In particular, an existing
    // symlink anywhere beneath the restore root must not redirect a later replacement outside it.
    for pf in &planned {
        validate_target_path(&restore_root, &pf.rel_path)?;
    }

    let mut result = RestoreResult::default();
    for pf in &planned {
        result.dirs_created += create_safe_target_parents(&restore_root, &pf.rel_path)?;
        let staged_path = stage.path().join(&pf.rel_path);
        let out_path = restore_root.join(&pf.rel_path);
        replace_file_from_staged(&staged_path, &out_path, pf.mtime_unix, pf.mtime_nanos)?;
        result.files_written += 1;
    }

    Ok(result)
}

fn validate_restore_root(restore_root: &Path) -> Result<()> {
    match fs::symlink_metadata(restore_root) {
        Ok(metadata) if metadata.file_type().is_symlink() => anyhow::bail!(
            "versioned::restore_files restore root must not be a symlink: {:?}",
            restore_root
        ),
        Ok(metadata) if !metadata.is_dir() => anyhow::bail!(
            "versioned::restore_files restore_root exists and is not a directory: {:?}",
            restore_root
        ),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| {
            format!(
                "versioned::restore_files failed to inspect restore root {:?}",
                restore_root
            )
        }),
    }
}

fn nearest_existing_directory(path: &Path) -> Result<PathBuf> {
    let mut candidate = path;
    loop {
        match fs::symlink_metadata(candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() => anyhow::bail!(
                "versioned::restore_files staging ancestor must not be a symlink: {:?}",
                candidate
            ),
            Ok(metadata) if metadata.is_dir() => return Ok(candidate.to_path_buf()),
            Ok(_) => anyhow::bail!(
                "versioned::restore_files staging ancestor is not a directory: {:?}",
                candidate
            ),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                candidate = candidate.parent().with_context(|| {
                    format!(
                        "versioned::restore_files could not find an existing ancestor for {:?}",
                        path
                    )
                })?;
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "versioned::restore_files failed to inspect staging ancestor {:?}",
                        candidate
                    )
                });
            }
        }
    }
}

fn validate_target_path(restore_root: &Path, rel_path: &str) -> Result<()> {
    validate_restore_root(restore_root)?;
    let relative = Path::new(rel_path);
    let mut current = restore_root.to_path_buf();
    if let Some(parent) = relative.parent() {
        for component in parent.components() {
            current.push(component.as_os_str());
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => anyhow::bail!(
                    "versioned::restore_files target path contains a symlink component: {:?}",
                    current
                ),
                Ok(metadata) if !metadata.is_dir() => anyhow::bail!(
                    "versioned::restore_files target parent is not a directory: {:?}",
                    current
                ),
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => break,
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "versioned::restore_files failed to inspect target component {:?}",
                            current
                        )
                    });
                }
            }
        }
    }

    let target = restore_root.join(relative);
    match fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.file_type().is_symlink() => anyhow::bail!(
            "versioned::restore_files target file must not be a symlink: {:?}",
            target
        ),
        Ok(metadata) if metadata.is_dir() => anyhow::bail!(
            "versioned::restore_files target file is a directory: {:?}",
            target
        ),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| {
            format!(
                "versioned::restore_files failed to inspect target file {:?}",
                target
            )
        }),
    }
}

fn create_safe_target_parents(restore_root: &Path, rel_path: &str) -> Result<usize> {
    let mut created = create_safe_directory_path(restore_root)?;
    validate_restore_root(restore_root)?;

    let mut current = restore_root.to_path_buf();
    if let Some(parent) = Path::new(rel_path).parent() {
        for component in parent.components() {
            current.push(component.as_os_str());
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => anyhow::bail!(
                    "versioned::restore_files target path contains a symlink component: {:?}",
                    current
                ),
                Ok(metadata) if !metadata.is_dir() => anyhow::bail!(
                    "versioned::restore_files target parent is not a directory: {:?}",
                    current
                ),
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    fs::create_dir(&current).with_context(|| {
                        format!(
                            "versioned::restore_files failed to create target directory {:?}",
                            current
                        )
                    })?;
                    created += 1;
                }
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "versioned::restore_files failed to inspect target directory {:?}",
                            current
                        )
                    });
                }
            }
        }
    }
    validate_target_path(restore_root, rel_path)?;
    Ok(created)
}

fn create_safe_directory_path(path: &Path) -> Result<usize> {
    let mut missing = Vec::new();
    let mut candidate = path;
    loop {
        match fs::symlink_metadata(candidate) {
            Ok(metadata) if metadata.file_type().is_symlink() => anyhow::bail!(
                "versioned::restore_files directory path contains a symlink component: {:?}",
                candidate
            ),
            Ok(metadata) if metadata.is_dir() => break,
            Ok(_) => anyhow::bail!(
                "versioned::restore_files directory path component is not a directory: {:?}",
                candidate
            ),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                missing.push(candidate.to_path_buf());
                candidate = candidate.parent().with_context(|| {
                    format!(
                        "versioned::restore_files could not resolve directory path {:?}",
                        path
                    )
                })?;
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "versioned::restore_files failed to inspect directory path {:?}",
                        candidate
                    )
                });
            }
        }
    }

    for directory in missing.iter().rev() {
        fs::create_dir(directory).with_context(|| {
            format!(
                "versioned::restore_files failed to create target directory {:?}",
                directory
            )
        })?;
        let metadata = fs::symlink_metadata(directory).with_context(|| {
            format!(
                "versioned::restore_files failed to verify target directory {:?}",
                directory
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            anyhow::bail!(
                "versioned::restore_files created target path is not a safe directory: {:?}",
                directory
            );
        }
    }
    Ok(missing.len())
}

fn replace_file_from_staged(
    staged_path: &Path,
    out_path: &Path,
    mtime_unix: i64,
    mtime_nanos: u32,
) -> Result<()> {
    let parent = out_path
        .parent()
        .context("versioned::replace_file_from_staged missing parent")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).with_context(|| {
        format!(
            "versioned::replace_file_from_staged failed to create temp file for {:?}",
            out_path
        )
    })?;
    let mut staged = fs::File::open(staged_path).with_context(|| {
        format!(
            "versioned::replace_file_from_staged failed to open staged file {:?}",
            staged_path
        )
    })?;
    io::copy(&mut staged, temp.as_file_mut()).with_context(|| {
        format!(
            "versioned::replace_file_from_staged failed to prepare {:?}",
            out_path
        )
    })?;
    restore_mtime(temp.path(), mtime_unix, mtime_nanos)?;
    temp.as_file_mut().sync_all().with_context(|| {
        format!(
            "versioned::replace_file_from_staged failed to sync temp file for {:?}",
            out_path
        )
    })?;
    // `persist` uses the platform's atomic rename primitive. Crucially, do not unlink an existing
    // target first: on platforms that cannot replace it atomically, returning an error leaves the
    // existing file intact.
    temp.persist(out_path).map_err(|error| {
        anyhow::anyhow!(
            "versioned::replace_file_from_staged failed to atomically replace {:?}: {}",
            out_path,
            error
        )
    })?;
    Ok(())
}

/// Restore one watched directory version using preflight checks and a staged swap.
pub fn restore_version(cfg: &Config, req: &RestoreRequest) -> Result<RestoreResult> {
    // Restores are high-risk; preflight + transactional swap prevents partial restores.
    validate_version_id(&req.version_id)?;
    let destination_root = destination_root_for_source(cfg, &req.source_path)?;
    let lock_policy = BlockingIoPolicy::from_config(cfg);
    let _operation_lease = acquire_store_lease(destination_root, &lock_policy)
        .context("versioned::restore_version could not serialize the destination store")?;
    let (manifest, store_root) = load_manifest_for_request(cfg, req)?;
    let blobs_root = blobs_root(&store_root);

    let restore_root = match req.mode {
        RestoreMode::InPlace => req.source_path.clone(),
        RestoreMode::ToDirectory => req
            .target_dir
            .clone()
            .context("versioned::restore_version missing target_dir for ToDirectory")?,
    };

    if restore_root.exists() && !restore_root.is_dir() {
        anyhow::bail!(
            "versioned::restore_version restore_root exists and is not a directory: {:?}",
            restore_root
        );
    }

    let parent = restore_root
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "versioned::restore_version failed to create restore parent dir {:?}",
            parent
        )
    })?;

    let (planned_files, _bytes) = preflight_restore(cfg, &manifest, &blobs_root, parent)?;

    let blob_codec = BlobCodec::from_config(cfg)
        .context("versioned::restore_version failed to initialize blob codec")?;

    let token = restore_token();
    let stage_root = parent.join(format!(".backup_sync_restore_stage_{token}"));

    // Ensure we do not accidentally collide with a prior failed restore.
    if stage_root.exists() {
        anyhow::bail!(
            "versioned::restore_version staging directory already exists: {:?}",
            stage_root
        );
    }

    let files_removed = if req.mode == RestoreMode::InPlace && restore_root.exists() {
        count_extraneous_files(&restore_root, &manifest)?
    } else {
        0
    };

    let mut result = match build_restore_tree(
        &stage_root,
        &manifest,
        &planned_files,
        &blob_codec,
        cfg.hashing.timeout_seconds,
    ) {
        Ok(r) => r,
        Err(e) => {
            if let Err(cleanup_error) = fs::remove_dir_all(&stage_root) {
                tracing::warn!(
                    stage_root = %stage_root.display(),
                    error = %cleanup_error,
                    "versioned::restore_version failed cleaning staging directory after build failure"
                );
            }
            return Err(e);
        }
    };

    result.files_removed = files_removed;

    let backup_root = swap_staged_tree_into_place(&stage_root, &restore_root, &token)?;
    if let Some(backup_root) = backup_root {
        if req.mode == RestoreMode::InPlace {
            match fs::remove_dir_all(&backup_root) {
                Ok(()) => {}
                Err(e) => {
                    tracing::warn!(
                        backup_dir = %backup_root.display(),
                        error = %e,
                        "versioned in-place restore succeeded but failed to remove backup directory"
                    );
                }
            }
        }
    }

    Ok(result)
}

fn write_file_atomic_from_blob(
    blob_path: &Path,
    out_path: &Path,
    expected_sha256: &str,
    blob_codec: &BlobCodec,
    blob_timeout_seconds: u64,
) -> Result<()> {
    let parent = out_path
        .parent()
        .context("versioned::write_file_atomic_from_blob missing parent")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "versioned::write_file_atomic_from_blob failed to create parent {:?}",
            parent
        )
    })?;

    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .context("versioned::write_file_atomic_from_blob failed to create temp file")?;

    let mut hasher = Sha256::new();
    {
        let mut hashing_writer = HashingWriter {
            inner: temp.as_file_mut(),
            hasher: &mut hasher,
        };
        blob_codec
            .copy_blob_plaintext_to_writer(blob_path, &mut hashing_writer, blob_timeout_seconds)
            .with_context(|| {
                format!(
                    "versioned::write_file_atomic_from_blob failed decoding blob {:?}",
                    blob_path
                )
            })?;
    }
    temp.flush().with_context(|| {
        format!(
            "versioned::write_file_atomic_from_blob failed to flush temp file for {:?}",
            out_path
        )
    })?;

    let actual_sha256 = format!("{:x}", hasher.finalize());
    if actual_sha256 != expected_sha256 {
        anyhow::bail!(
            "versioned::write_file_atomic_from_blob plaintext sha256 mismatch for {:?} (expected {}, got {})",
            out_path,
            expected_sha256,
            actual_sha256
        );
    }

    temp.persist(out_path).map_err(|e| {
        anyhow::anyhow!(
            "versioned::write_file_atomic_from_blob failed to persist {:?}: {}",
            out_path,
            e
        )
    })?;
    Ok(())
}

struct HashingWriter<'a, W> {
    inner: &'a mut W,
    hasher: &'a mut Sha256,
}

impl<W: Write> Write for HashingWriter<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(bytes)?;
        self.hasher.update(&bytes[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn restore_mtime(path: &Path, mtime_unix: i64, mtime_nanos: u32) -> Result<()> {
    if mtime_unix <= 0 {
        return Ok(());
    }
    let ft = FileTime::from_unix_time(mtime_unix, mtime_nanos);
    set_file_mtime(path, ft).with_context(|| {
        format!(
            "versioned::restore_mtime failed to set mtime for {:?}",
            path
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::replace_file_from_staged;
    use std::fs;

    #[test]
    fn existing_target_survives_commit_preparation_failure() {
        let temp = tempfile::tempdir().expect("restore test failed to create temp directory");
        let target = temp.path().join("target.txt");
        fs::write(&target, "current").expect("restore test failed to write target");

        replace_file_from_staged(&temp.path().join("missing-stage"), &target, 0, 0)
            .expect_err("restore test unexpectedly prepared a missing staged file");

        assert_eq!(
            fs::read_to_string(&target).expect("restore test failed to read target"),
            "current"
        );
    }
}
