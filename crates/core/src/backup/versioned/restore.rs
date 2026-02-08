use super::model::{Manifest, ManifestEntryKind, VersionInfo};
use super::store::{blob_path, blobs_root, sources_root, store_root};
use crate::config::model::{Config, Destination, WatchedKind};
use crate::encryption::blobs::BlobCodec;
use crate::hashing;
use anyhow::{Context, Result};
use filetime::{set_file_mtime, FileTime};
use fs2::free_space;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

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
    mtime_unix: i64,
    mtime_nanos: u32,
}

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
        out.push((watched.path.clone(), index.versions));
    }
    Ok(out)
}

/**
 * Summary: Load a versioned manifest for a watched directory and compute restore plan inputs.
 *
 * Inputs: Store roots plus a restore request.
 * Outputs: Parsed manifest object.
 * Side effects: Reads the manifest JSON from the destination store.
 * Error handling: Returns contextual errors for missing watched paths, destinations, and parse failures.
 * Ties to other methods: Called by `restore_version` before preflight and restore execution.
 * Why this exists: Keep `restore_version` readable while centralizing store path calculations.
 */
fn load_manifest_for_request(cfg: &Config, req: &RestoreRequest) -> Result<(Manifest, PathBuf)> {
    let watched = cfg
        .watched
        .iter()
        .find(|w| w.enabled && w.path == req.source_path)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "versioned::load_manifest_for_request source path not configured: {:?}",
                req.source_path
            )
        })?;
    if !matches!(watched.kind, WatchedKind::Directory) {
        anyhow::bail!(
            "versioned::load_manifest_for_request only directory watched paths are supported: {:?}",
            watched.path
        );
    }

    let destinations_by_id: HashMap<&str, &Destination> = cfg
        .destinations
        .iter()
        .map(|d| (d.id.as_str(), d))
        .collect();
    let dest = destinations_by_id
        .get(watched.destination_id.as_str())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "versioned::load_manifest_for_request missing destination id {} for {:?}",
                watched.destination_id,
                watched.path
            )
        })?;

    let store_root = store_root(&dest.path);
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
    Ok((manifest, store_root))
}

/**
 * Summary: Preflight a restore by verifying blob availability and destination free space.
 *
 * Inputs: Manifest to restore, blob store root, free-space check directory, and config.
 * Outputs: `(planned_files, total_bytes)` for subsequent restore execution.
 * Side effects: Reads filesystem metadata for blobs and free space.
 * Error handling: Returns a detailed error listing missing blobs or insufficient space.
 * Ties to other methods: Called by `restore_version` before any filesystem mutations.
 * Why this exists: Prevent partial restores and ensure predictable failures before touching the target.
 */
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

/**
 * Summary: Enforce restore free-space guardrails before writing.
 *
 * Inputs: Config for thresholds, target directory for free-space probing, and required bytes.
 * Outputs: `Ok(())` when sufficient space is available.
 * Side effects: Reads filesystem free-space statistics.
 * Error handling: Returns a clear error that includes required bytes and configured thresholds.
 * Ties to other methods: Used by `preflight_restore` to ensure restores cannot fill disks unexpectedly.
 * Why this exists: Restores can write large amounts of data; failing mid-way due to disk full is unsafe.
 */
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

/**
 * Summary: Restore a manifest into a staging directory tree.
 *
 * Inputs: Stage root, manifest directory entries, and planned files with blob pointers.
 * Outputs: Restore counters for directories created and files written.
 * Side effects: Creates directories, writes files from blobs, and sets mtimes.
 * Error handling: Returns contextual errors for directory creation, blob reads, and atomic writes.
 * Ties to other methods: Called by transactional restore flows before swapping into place.
 * Why this exists: Building a complete restore tree first enables atomic swap and prevents partial restores.
 */
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
        write_file_atomic_from_blob(&pf.blob_path, &out_path, blob_codec, blob_timeout_seconds)?;
        restore_mtime(&out_path, pf.mtime_unix, pf.mtime_nanos)?;
        result.files_written += 1;
    }

    Ok(result)
}

/**
 * Summary: Count files in a target directory that are not present in the manifest.
 *
 * Inputs: Target root and manifest describing the desired version contents.
 * Outputs: Count of extraneous files relative to the manifest.
 * Side effects: Walks the target directory tree and reads filesystem metadata.
 * Error handling: Returns contextual errors for directory walks.
 * Ties to other methods: Used by `restore_version` to populate `files_removed` without mutating the target.
 * Why this exists: Transactional restore swaps whole trees; this preserves useful reporting without risky deletes.
 */
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

/**
 * Summary: Transactionally swap a staged directory tree into the target location.
 *
 * Inputs: The staged directory path, the final target path, and a token for backup naming.
 * Outputs: The path of any backup directory created (so callers can delete it after success).
 * Side effects: Renames directories to perform an atomic swap where supported by the OS/filesystem.
 * Error handling: Attempts rollback if the final rename fails after moving the original aside.
 * Ties to other methods: Called by `restore_version` for both in-place and restore-to-dir flows.
 * Why this exists: Renaming a fully-built tree into place is the closest thing to transactional restore.
 */
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
            let _ = fs::rename(&backup_root, target_root);
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

/**
 * Summary: Generate a token suitable for unique staging and backup directory names.
 *
 * Inputs: none.
 * Outputs: A token string.
 * Side effects: Reads time and process id.
 * Error handling: None.
 * Ties to other methods: Used by transactional restore flows to avoid collisions.
 * Why this exists: Restore operations must not collide across concurrent runs.
 */
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
    /*
     * Summary: List file entries for a specific version with optional substring filtering.
     *
     * Inputs: Config, watched directory path, version id, optional query substring, and a result limit.
     * Outputs: A `ListVersionFilesResult` with total file count and up to `limit` matches.
     * Side effects: Reads the manifest JSON from the destination store.
     * Error handling: Returns contextual errors for missing config mappings and manifest parse failures.
     * Ties to other methods: Used by GUI file-level restore UX to browse and search version contents.
     * Why this exists: Restoring individual files requires discoverability of paths inside manifests.
     */
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
    /*
     * Summary: Restore a specific set of files from a version without swapping whole directory trees.
     *
     * Inputs: Config plus a request containing watched folder, version id, file rel paths, and mode.
     * Outputs: A `RestoreResult` with counts for files written and directories created.
     * Side effects: Creates directories and writes files atomically from blobs.
     * Error handling: Fails before any writes if any requested file is missing or blobs are absent.
     * Ties to other methods: Uses manifest load, per-file preflight, `write_file_atomic_from_blob`, and mtime restore.
     * Why this exists: File-level restore is a common UX need without the risk of full in-place swaps.
     */
    if req.rel_paths.is_empty() {
        anyhow::bail!("versioned::restore_files requires at least one rel_path");
    }

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
    if restore_root.exists() && !restore_root.is_dir() {
        anyhow::bail!(
            "versioned::restore_files restore_root exists and is not a directory: {:?}",
            restore_root
        );
    }
    fs::create_dir_all(&restore_root).with_context(|| {
        format!(
            "versioned::restore_files failed to create restore root {:?}",
            restore_root
        )
    })?;

    let mut missing: Vec<String> = Vec::new();
    let mut planned: Vec<PlannedFile> = Vec::new();
    let mut total_bytes: u64 = 0;

    for rel_path in req.rel_paths.iter() {
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
    ensure_restore_free_space(cfg, &restore_root, total_bytes)?;

    let blob_codec = BlobCodec::from_config(cfg)
        .context("versioned::restore_files failed to initialize blob codec")?;

    let mut result = RestoreResult::default();
    for pf in planned.iter() {
        let out_path = restore_root.join(&pf.rel_path);
        if let Some(parent) = out_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).with_context(|| {
                    format!(
                        "versioned::restore_files failed to create parent dir {:?}",
                        parent
                    )
                })?;
                result.dirs_created += 1;
            }
        }
        write_file_atomic_from_blob(
            &pf.blob_path,
            &out_path,
            &blob_codec,
            cfg.hashing.timeout_seconds,
        )?;
        restore_mtime(&out_path, pf.mtime_unix, pf.mtime_nanos)?;
        result.files_written += 1;
    }

    Ok(result)
}

pub fn restore_version(cfg: &Config, req: &RestoreRequest) -> Result<RestoreResult> {
    /*
     * Summary: Restore a watched directory to a specific version with preflight + transactional swap.
     *
     * Inputs: Parsed config and a restore request describing source, version, mode, and optional target.
     * Outputs: A `RestoreResult` summarizing files written/removed and directories created.
     * Side effects: Creates staging directories, reads blobs from the destination store, and renames target trees.
     * Error handling: Fails before any target mutation if blobs are missing or space is insufficient; swap failures
     * attempt rollback and return contextual errors.
     * Ties to other methods: Uses `preflight_restore`, `build_restore_tree`, and `swap_staged_tree_into_place`.
     * Why this exists: Restores are high-risk; preflight + transactional swap prevents partial restores.
     */
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
            let _ = fs::remove_dir_all(&stage_root);
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

    blob_codec
        .copy_blob_plaintext_to_writer(blob_path, temp.as_file_mut(), blob_timeout_seconds)
        .with_context(|| {
            format!(
                "versioned::write_file_atomic_from_blob failed decoding blob {:?}",
                blob_path
            )
        })?;
    temp.flush().ok();

    let _ = fs::remove_file(out_path);
    temp.persist(out_path).map_err(|e| {
        anyhow::anyhow!(
            "versioned::write_file_atomic_from_blob failed to persist {:?}: {}",
            out_path,
            e
        )
    })?;
    Ok(())
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
