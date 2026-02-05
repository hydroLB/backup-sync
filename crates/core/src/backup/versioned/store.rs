use super::model::{
    Manifest, ManifestEntry, ManifestEntryKind, ReadFailure, ReadFailurePhase, VersionIndex,
    VersionInfo,
};
use crate::config::model::{Config, Destination, WatchedKind, WatchedPath};
use crate::fs::snapshots::prepare_source_view;
use crate::logging::redact_path;
use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub struct FolderDescriptor {
    pub source_path: PathBuf,
    pub destination_root: PathBuf,
    pub keep_versions: usize,
}

#[derive(Debug, Default, Clone)]
pub struct FolderBackupResult {
    pub changed: bool,
    pub version_id: Option<String>,
    pub blobs_written: usize,
    pub bytes_written: u64,
}

#[derive(Debug, Default, Clone)]
pub struct BackupCycleResult {
    pub folders_scanned: usize,
    pub versions_created: usize,
    pub blobs_written: usize,
    pub bytes_written: u64,
}

pub(crate) const STORE_DIR: &str = ".backup_sync";
pub(crate) const STORE_SCHEMA_VERSION: u32 = 1;

/**
 * Summary: Best-effort directory fsync helper for crash-consistent commits.
 *
 * Inputs: `path` directory path to sync.
 * Outputs: `Ok(())` when the directory metadata is durably flushed.
 * Side effects: Opens a directory handle and invokes `sync_all` on it.
 * Error handling: On Unix, returns contextual errors; on non-Unix, it is a no-op.
 * Ties to other methods: Used by `create_dir_all_durable`, `write_json_atomic_durable`, and `write_blob_durable`.
 * Why this exists: Atomic rename is not durable across power loss without syncing the parent directory.
 */
#[cfg(target_family = "unix")]
fn sync_dir(path: &Path) -> Result<()> {
    let dir = fs::File::open(path)
        .with_context(|| format!("versioned::sync_dir failed to open directory {:?}", path))?;
    dir.sync_all()
        .with_context(|| format!("versioned::sync_dir failed to sync directory {:?}", path))?;
    Ok(())
}

/**
 * Summary: Best-effort directory fsync helper for crash-consistent commits.
 *
 * Inputs: `path` directory path to sync.
 * Outputs: `Ok(())` always.
 * Side effects: None.
 * Error handling: None.
 * Ties to other methods: Used by `create_dir_all_durable`, `write_json_atomic_durable`, and `write_blob_durable`.
 * Why this exists: Some platforms do not support opening directories with `std::fs::File` reliably.
 */
#[cfg(not(target_family = "unix"))]
fn sync_dir(_path: &Path) -> Result<()> {
    Ok(())
}

/**
 * Summary: Create a directory tree and fsync its parent directories for durability.
 *
 * Inputs: `path` directory to create; `durability_root` root directory to stop syncing at.
 * Outputs: `Ok(())` when the directory exists and parent entries are durably flushed.
 * Side effects: Creates directories and performs directory fsync operations.
 * Error handling: Returns contextual errors when create or sync steps fail.
 * Ties to other methods: Used to prepare `blobs_root` and `manifests_root` before durable writes.
 * Why this exists: Directory creation and atomic renames are not crash-safe without syncing parents.
 */
fn create_dir_all_durable(path: &Path, durability_root: &Path) -> Result<()> {
    if path != durability_root && !path.starts_with(durability_root) {
        anyhow::bail!(
            "versioned::create_dir_all_durable invalid durability root; path {:?} is not within {:?}",
            path,
            durability_root
        );
    }
    if path.exists() {
        if !path.is_dir() {
            anyhow::bail!(
                "versioned::create_dir_all_durable expected directory at {:?}",
                path
            );
        }
        return Ok(());
    }
    fs::create_dir_all(path).with_context(|| {
        format!(
            "versioned::create_dir_all_durable failed to create directory {:?}",
            path
        )
    })?;

    // Sync the created directory itself and each ancestor up to `durability_root` so
    // directory entries become durable (power-loss safe) on filesystems that require it.
    let mut cur: Option<&Path> = Some(path);
    while let Some(p) = cur {
        if p.exists() {
            sync_dir(p)?;
        }
        if p == durability_root {
            break;
        }
        cur = p.parent();
    }
    Ok(())
}

/**
 * Summary: Retry helper that runs an operation with backoff and optional overall timeout.
 *
 * Inputs: `label` used for error context, `timeout_seconds` as an overall time bound, `retry_delays`
 * as backoff schedule, and `op` as the fallible operation.
 * Outputs: On success, returns `(value, attempts_used)`. On failure, returns the last error.
 * Side effects: Sleeps between attempts when retries are configured.
 * Error handling: Preserves the last error and adds context including attempt count and timeout status.
 * Ties to other methods: Used by file hashing, blob writing, and metadata reads for glitch resilience.
 * Why this exists: Make transient IO failures first-class without failing an entire backup cycle.
 */
fn retry_with_backoff<T, F>(
    label: &str,
    timeout_seconds: u64,
    retry_delays: &[Duration],
    mut op: F,
) -> Result<(T, u32)>
where
    F: FnMut() -> Result<T>,
{
    let start = Instant::now();
    let mut last_err: Option<anyhow::Error> = None;
    let max_attempts = retry_delays.len() + 1;
    for attempt_idx in 0..max_attempts {
        if timeout_seconds > 0 && start.elapsed().as_secs() > timeout_seconds {
            let attempts = attempt_idx as u32;
            anyhow::bail!(
                "{label} timed out after {}s (attempts={})",
                timeout_seconds,
                attempts.max(1)
            );
        }
        match op() {
            Ok(v) => return Ok((v, (attempt_idx + 1) as u32)),
            Err(e) => {
                last_err = Some(e);
                if attempt_idx < retry_delays.len() {
                    std::thread::sleep(retry_delays[attempt_idx]);
                    continue;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("{label} failed (no error captured)")))
        .with_context(|| format!("{label} failed after {max_attempts} attempts"))
}

pub fn run_backup_cycle(cfg: &Config) -> Result<BackupCycleResult> {
    let destinations_by_id: HashMap<&str, &Destination> = cfg
        .destinations
        .iter()
        .map(|d| (d.id.as_str(), d))
        .collect();

    let mut cycle = BackupCycleResult::default();
    for watched in cfg.watched.iter().filter(|w| w.enabled) {
        let dest = destinations_by_id
            .get(watched.destination_id.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "versioned::run_backup_cycle missing destination id {} for watched path {}",
                    watched.destination_id,
                    redact_path(&watched.path)
                )
            })?;
        let keep_versions = watched
            .max_backups_per_file
            .or(dest.max_backups_per_file)
            .unwrap_or(cfg.max_backups_per_file);
        let descriptor = FolderDescriptor {
            source_path: watched.path.clone(),
            destination_root: dest.path.clone(),
            keep_versions,
        };
        let result = backup_one_folder(cfg, watched, &descriptor)?;
        cycle.folders_scanned += 1;
        if result.changed {
            cycle.versions_created += 1;
        }
        cycle.blobs_written += result.blobs_written;
        cycle.bytes_written += result.bytes_written;
    }
    Ok(cycle)
}

fn backup_one_folder(
    cfg: &Config,
    watched: &WatchedPath,
    desc: &FolderDescriptor,
) -> Result<FolderBackupResult> {
    let retry_delays = cfg.execution.retry_delays();
    let store_root = store_root(&desc.destination_root);
    let blobs_root = blobs_root(&store_root);
    create_dir_all_durable(&blobs_root, &desc.destination_root).with_context(|| {
        format!(
            "versioned::backup_one_folder failed to create durable blob directory {:?}",
            blobs_root
        )
    })?;

    let source_id = sha256_hex(desc.source_path.to_string_lossy().as_bytes());
    let source_root = sources_root(&store_root).join(&source_id);
    let manifests_root = source_root.join("manifests");
    create_dir_all_durable(&manifests_root, &desc.destination_root).with_context(|| {
        format!(
            "versioned::backup_one_folder failed to create durable manifests directory {:?}",
            manifests_root
        )
    })?;

    let mut index = load_index(&source_root, &desc.source_path)?;
    let prev = latest_manifest(&manifests_root, &index)?;
    let source_view = prepare_source_view(
        &watched.path,
        cfg.runtime.source_snapshots_enabled,
        cfg.runtime.source_snapshot_timeout_seconds,
    )?;
    if let Some(err) = source_view.snapshot_error() {
        tracing::warn!(
            source_path = %redact_path(&watched.path),
            error = %err,
            "versioned backup could not obtain a snapshot view; scanning live filesystem"
        );
    }
    let mut snapshot = scan_snapshot(cfg, watched, source_view.scan_path(), &prev, &retry_delays)?;
    snapshot.source_snapshot = source_view.snapshot().cloned();
    snapshot.source_snapshot_error = source_view.snapshot_error().map(|s| s.to_string());
    let mut had_read_failures = !snapshot.read_failures.is_empty();
    let changed = match &prev {
        None => true,
        Some(prev_manifest) => !manifests_equivalent(prev_manifest, &snapshot),
    };
    if !changed {
        write_scan_report(&source_root, &snapshot, None, &desc.destination_root)?;
        return Ok(FolderBackupResult {
            changed: false,
            ..Default::default()
        });
    }

    let mut version_id = chrono::Utc::now().format("%Y%m%d-%H%M%S-%f").to_string();
    let mut manifest_path = manifests_root.join(format!("{version_id}.json"));
    let mut collision = 0u32;
    while manifest_path.exists() {
        collision += 1;
        version_id = format!("{}-{}", version_id, collision);
        manifest_path = manifests_root.join(format!("{version_id}.json"));
    }

    let mut written = FolderBackupResult {
        changed: true,
        version_id: Some(version_id.clone()),
        ..Default::default()
    };

    let file_rel_paths: Vec<String> = snapshot
        .entries
        .values()
        .filter(|e| e.kind == ManifestEntryKind::File)
        .map(|e| e.rel_path.clone())
        .collect();

    for rel_path in file_rel_paths {
        let entry = match snapshot.entries.get(&rel_path).cloned() {
            None => continue,
            Some(e) => e,
        };
        let Some(hash) = entry.sha256.as_deref() else {
            had_read_failures = true;
            snapshot.read_failures.push(ReadFailure {
                rel_path: rel_path.clone(),
                phase: ReadFailurePhase::BlobWrite,
                attempts: 1,
                message: "missing sha256 for file entry".to_string(),
            });
            snapshot.entries.remove(&rel_path);
            continue;
        };
        let blob_path = blob_path(&blobs_root, hash);
        if blob_path.exists() {
            continue;
        }
        let src_path = match watched.kind {
            WatchedKind::File => source_view.scan_path().to_path_buf(),
            WatchedKind::Directory => source_view.scan_path().join(Path::new(&rel_path)),
        };
        match write_blob(
            &src_path,
            &blob_path,
            cfg.hashing.timeout_seconds,
            &retry_delays,
            &desc.destination_root,
        ) {
            Ok((count, bytes)) => {
                written.blobs_written += count;
                written.bytes_written += bytes;
            }
            Err(e) => {
                had_read_failures = true;
                snapshot.read_failures.push(ReadFailure {
                    rel_path: rel_path.clone(),
                    phase: ReadFailurePhase::BlobWrite,
                    attempts: (retry_delays.len() + 1) as u32,
                    message: format!("{e:#}"),
                });
                if let Some(prev_manifest) = prev.as_ref() {
                    if let Some(prev_entry) = prev_manifest.entries.get(&rel_path) {
                        snapshot
                            .entries
                            .insert(rel_path.clone(), prev_entry.clone());
                    } else {
                        snapshot.entries.remove(&rel_path);
                    }
                } else {
                    snapshot.entries.remove(&rel_path);
                }
            }
        }
    }

    let changed_after_failures = match &prev {
        None => true,
        Some(prev_manifest) => !manifests_equivalent(prev_manifest, &snapshot),
    };
    if !changed_after_failures {
        write_scan_report(&source_root, &snapshot, None, &desc.destination_root)?;
        return Ok(FolderBackupResult {
            changed: false,
            blobs_written: written.blobs_written,
            bytes_written: written.bytes_written,
            ..Default::default()
        });
    }

    write_json_atomic_durable(&manifest_path, &snapshot, &desc.destination_root).with_context(
        || {
            format!(
                "versioned::backup_one_folder failed to write manifest {:?}",
                manifest_path
            )
        },
    )?;

    index.schema_version = STORE_SCHEMA_VERSION;
    index.source_path = desc.source_path.to_string_lossy().into_owned();
    index.versions.push(VersionInfo {
        id: version_id.clone(),
        created_at_unix: snapshot.created_at_unix,
    });
    index.versions.sort_by(|a, b| {
        a.created_at_unix
            .cmp(&b.created_at_unix)
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut deleted_any = false;
    if !had_read_failures {
        while index.versions.len() > desc.keep_versions {
            if let Some(oldest) = index.versions.first().cloned() {
                let path = manifests_root.join(format!("{}.json", oldest.id));
                let _ = fs::remove_file(&path);
                index.versions.remove(0);
                deleted_any = true;
            } else {
                break;
            }
        }
    }

    write_index(&source_root, &index, &desc.destination_root)?;
    write_scan_report(
        &source_root,
        &snapshot,
        Some(&version_id),
        &desc.destination_root,
    )?;

    if deleted_any {
        gc_unreferenced_blobs(&store_root, &blobs_root)?;
    }

    Ok(written)
}

pub(crate) fn store_root(destination_root: &Path) -> PathBuf {
    destination_root
        .join(STORE_DIR)
        .join(format!("v{STORE_SCHEMA_VERSION}"))
}

pub(crate) fn sources_root(store_root: &Path) -> PathBuf {
    store_root.join("sources")
}

pub(crate) fn blobs_root(store_root: &Path) -> PathBuf {
    store_root.join("blobs").join("sha256")
}

pub(crate) fn blob_path(blobs_root: &Path, hash: &str) -> PathBuf {
    let prefix = &hash[0..2];
    blobs_root.join(prefix).join(hash)
}

/**
 * Summary: Write a content-addressed blob durably (fsync file and parent directory).
 *
 * Inputs: Source file path, destination blob path, timeout in seconds, and `durability_root`.
 * Outputs: `(blobs_written, bytes_written)` for accounting.
 * Side effects: Reads from the source filesystem and writes a new blob file under the destination store.
 * Error handling: Returns contextual errors for create, IO, timeout, fsync, and rename failures.
 * Ties to other methods: Called by `backup_one_folder` before writing the manifest that references blobs.
 * Why this exists: A blob referenced by a manifest must be durable across power loss before the manifest commit.
 */
fn write_blob(
    src_path: &Path,
    blob_path: &Path,
    timeout_seconds: u64,
    retry_delays: &[Duration],
    durability_root: &Path,
) -> Result<(usize, u64)> {
    let parent = blob_path
        .parent()
        .context("versioned::write_blob missing blob parent")?;
    create_dir_all_durable(parent, durability_root).with_context(|| {
        format!(
            "versioned::write_blob failed to create durable blob parent directory {:?}",
            parent
        )
    })?;

    let label = format!("versioned::write_blob {:?} -> {:?}", src_path, blob_path);
    let (written, _) = retry_with_backoff(&label, timeout_seconds, retry_delays, || {
        write_blob_once(src_path, blob_path, timeout_seconds)
    })?;
    Ok((1, written))
}

/**
 * Summary: Single-attempt blob write (used by retry wrapper).
 *
 * Inputs: Source file path, destination blob path, and timeout in seconds.
 * Outputs: Number of bytes written to the blob temp file.
 * Side effects: Reads from source, writes temp file, fsyncs, renames, and fsyncs the parent directory.
 * Error handling: Returns contextual errors for IO, timeout, fsync, and rename operations.
 * Ties to other methods: Called by `write_blob` via `retry_with_backoff`.
 * Why this exists: Allow clean retry semantics without partial blob files surviving failed attempts.
 */
fn write_blob_once(src_path: &Path, blob_path: &Path, timeout_seconds: u64) -> Result<u64> {
    let start = Instant::now();
    let mut file = fs::File::open(src_path).with_context(|| {
        format!(
            "versioned::write_blob_once failed to open source file {:?}",
            src_path
        )
    })?;
    let parent = blob_path
        .parent()
        .context("versioned::write_blob_once missing blob parent")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .context("versioned::write_blob_once temp create")?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut written: u64 = 0;
    loop {
        if timeout_seconds > 0 && start.elapsed().as_secs() > timeout_seconds {
            anyhow::bail!(
                "versioned::write_blob_once timed out after {}s writing {:?}",
                timeout_seconds,
                src_path
            );
        }
        let n = file
            .read(&mut buf)
            .with_context(|| format!("versioned::write_blob_once failed reading {:?}", src_path))?;
        if n == 0 {
            break;
        }
        temp.write_all(&buf[..n]).with_context(|| {
            format!(
                "versioned::write_blob_once failed writing temp for {:?}",
                src_path
            )
        })?;
        written += n as u64;
    }
    temp.flush().with_context(|| {
        format!(
            "versioned::write_blob_once failed to flush temp for {:?}",
            src_path
        )
    })?;
    temp.as_file().sync_all().with_context(|| {
        format!(
            "versioned::write_blob_once failed to fsync temp file for {:?}",
            src_path
        )
    })?;
    temp.persist(blob_path).map_err(|e| {
        anyhow::anyhow!(
            "versioned::write_blob_once failed to persist blob {:?}: {}",
            blob_path,
            e
        )
    })?;
    sync_dir(parent)?;
    Ok(written)
}

fn scan_snapshot(
    cfg: &Config,
    watched: &WatchedPath,
    scan_path: &Path,
    prev: &Option<Manifest>,
    retry_delays: &[Duration],
) -> Result<Manifest> {
    let prev_entries: HashMap<&str, &ManifestEntry> = prev
        .as_ref()
        .map(|m| {
            m.entries
                .values()
                .map(|e| (e.rel_path.as_str(), e))
                .collect()
        })
        .unwrap_or_default();

    let ignore = build_ignore_set(&cfg.ignore_patterns)
        .context("versioned::scan_snapshot failed to build ignore set")?;

    let mut entries: BTreeMap<String, ManifestEntry> = BTreeMap::new();
    let mut read_failures: Vec<ReadFailure> = Vec::new();
    let prev_manifest = prev.as_ref();

    match watched.kind {
        WatchedKind::File => {
            let name = watched
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "file".to_string());
            match scan_one_file(
                scan_path,
                &name,
                cfg.hashing.timeout_seconds,
                retry_delays,
                &prev_entries,
            ) {
                Ok(e) => {
                    entries.insert(name.clone(), e);
                }
                Err(error) => {
                    let phase = classify_scan_failure(&error);
                    read_failures.push(ReadFailure {
                        rel_path: name.clone(),
                        phase,
                        attempts: (retry_delays.len() + 1) as u32,
                        message: format!("{error:#}"),
                    });
                    if let Some(prev_manifest) = prev_manifest {
                        if let Some(prev_entry) = prev_manifest.entries.get(&name) {
                            entries.insert(name.clone(), prev_entry.clone());
                        }
                    }
                }
            }
        }
        WatchedKind::Directory => {
            for item in walkdir::WalkDir::new(scan_path)
                .follow_links(false)
                .into_iter()
            {
                let item = match item {
                    Ok(i) => i,
                    Err(e) => {
                        let rel_str = e
                            .path()
                            .and_then(|p| p.strip_prefix(scan_path).ok())
                            .map(normalize_rel)
                            .unwrap_or_else(|| "<walk_error>".to_string());
                        read_failures.push(ReadFailure {
                            rel_path: rel_str.clone(),
                            phase: ReadFailurePhase::Walk,
                            attempts: 1,
                            message: e.to_string(),
                        });
                        carry_forward_prev_prefix(&mut entries, prev_manifest, &rel_str);
                        continue;
                    }
                };
                let path = item.path();
                let rel = path.strip_prefix(scan_path).unwrap_or(path);
                if rel.as_os_str().is_empty() {
                    continue;
                }
                if cfg.skip_hidden && is_hidden(rel) {
                    continue;
                }
                if ignore.is_match(rel) {
                    continue;
                }
                if item.file_type().is_dir() {
                    let rel_str = normalize_rel(rel);
                    entries.insert(
                        rel_str.clone(),
                        ManifestEntry {
                            kind: ManifestEntryKind::Dir,
                            rel_path: rel_str,
                            len: 0,
                            mtime_unix: 0,
                            mtime_nanos: 0,
                            sha256: None,
                        },
                    );
                    continue;
                }
                if item.file_type().is_file() {
                    let rel_str = normalize_rel(rel);
                    match scan_one_file(
                        path,
                        &rel_str,
                        cfg.hashing.timeout_seconds,
                        retry_delays,
                        &prev_entries,
                    ) {
                        Ok(entry) => {
                            entries.insert(rel_str.clone(), entry);
                        }
                        Err(error) => {
                            let phase = classify_scan_failure(&error);
                            read_failures.push(ReadFailure {
                                rel_path: rel_str.clone(),
                                phase,
                                attempts: (retry_delays.len() + 1) as u32,
                                message: format!("{error:#}"),
                            });
                            if let Some(prev_manifest) = prev_manifest {
                                if let Some(prev_entry) = prev_manifest.entries.get(&rel_str) {
                                    entries.insert(rel_str.clone(), prev_entry.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(Manifest {
        schema_version: STORE_SCHEMA_VERSION,
        source_path: watched.path.to_string_lossy().into_owned(),
        created_at_unix: chrono::Utc::now().timestamp(),
        source_snapshot: None,
        source_snapshot_error: None,
        read_failures,
        entries,
    })
}

/**
 * Summary: Carry forward previous manifest entries for an unreadable subtree.
 *
 * Inputs: Current `entries` map, optional previous manifest, and a relative path prefix.
 * Outputs: Adds any missing prior entries under the prefix to `entries`.
 * Side effects: Mutates the provided `entries` map.
 * Error handling: None.
 * Ties to other methods: Used by `scan_snapshot` when `walkdir` yields permission or IO errors.
 * Why this exists: Prevent transient directory read failures from being interpreted as deletions.
 */
fn carry_forward_prev_prefix(
    entries: &mut BTreeMap<String, ManifestEntry>,
    prev: Option<&Manifest>,
    rel_prefix: &str,
) {
    let Some(prev) = prev else { return };
    if rel_prefix.is_empty() || rel_prefix == "<walk_error>" {
        return;
    }
    let prefix = if rel_prefix.ends_with('/') {
        rel_prefix.to_string()
    } else {
        format!("{rel_prefix}/")
    };
    for e in prev.entries.values() {
        if e.rel_path == rel_prefix || e.rel_path.starts_with(&prefix) {
            entries
                .entry(e.rel_path.clone())
                .or_insert_with(|| e.clone());
        }
    }
}

/**
 * Summary: Classify a scan failure into a stable phase for reporting.
 *
 * Inputs: A scan-related error.
 * Outputs: A `ReadFailurePhase` value indicating the most likely failure phase.
 * Side effects: None.
 * Error handling: None.
 * Ties to other methods: Used by `scan_snapshot` when recording `read_failures`.
 * Why this exists: Keep failure reporting useful without plumbing custom error types everywhere.
 */
fn classify_scan_failure(error: &anyhow::Error) -> ReadFailurePhase {
    let msg = format!("{error:#}");
    if msg.contains("failed to stat file") || msg.contains("metadata") {
        ReadFailurePhase::Metadata
    } else {
        ReadFailurePhase::Hash
    }
}

fn scan_one_file(
    abs_path: &Path,
    rel_path: &str,
    timeout_seconds: u64,
    retry_delays: &[Duration],
    prev: &HashMap<&str, &ManifestEntry>,
) -> Result<ManifestEntry> {
    let label = format!("versioned::scan_one_file metadata {:?}", abs_path);
    let (meta, _) = retry_with_backoff(&label, timeout_seconds, retry_delays, || {
        fs::metadata(abs_path).with_context(|| {
            format!(
                "versioned::scan_one_file failed to stat file {:?}",
                abs_path
            )
        })
    })?;
    let (mtime_unix, mtime_nanos) = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| (d.as_secs() as i64, d.subsec_nanos()))
        .unwrap_or((0, 0));
    let len = meta.len();
    if let Some(prev) = prev.get(rel_path) {
        if prev.kind == ManifestEntryKind::File
            && prev.len == len
            && prev.mtime_unix == mtime_unix
            && prev.mtime_nanos == mtime_nanos
            && prev.sha256.is_some()
        {
            return Ok(ManifestEntry {
                kind: ManifestEntryKind::File,
                rel_path: rel_path.to_string(),
                len,
                mtime_unix,
                mtime_nanos,
                sha256: prev.sha256.clone(),
            });
        }
    }
    let hash = sha256_file(abs_path, timeout_seconds, retry_delays)?;
    Ok(ManifestEntry {
        kind: ManifestEntryKind::File,
        rel_path: rel_path.to_string(),
        len,
        mtime_unix,
        mtime_nanos,
        sha256: Some(hash),
    })
}

fn manifests_equivalent(prev: &Manifest, next: &Manifest) -> bool {
    if prev.entries.len() != next.entries.len() {
        return false;
    }
    for (k, v) in next.entries.iter() {
        let prev_e = match prev.entries.get(k) {
            None => return false,
            Some(e) => e,
        };
        if prev_e.kind != v.kind {
            return false;
        }
        if v.kind == ManifestEntryKind::File && prev_e.sha256 != v.sha256 {
            return false;
        }
    }
    true
}

fn load_index(source_root: &Path, source_path: &Path) -> Result<VersionIndex> {
    let path = source_root.join("index.json");
    if !path.exists() {
        return Ok(VersionIndex {
            schema_version: STORE_SCHEMA_VERSION,
            source_path: source_path.to_string_lossy().into_owned(),
            versions: Vec::new(),
        });
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("versioned::load_index failed to read {:?}", path))?;
    let index: VersionIndex = serde_json::from_str(&raw)
        .with_context(|| format!("versioned::load_index failed to parse {:?}", path))?;
    Ok(index)
}

/**
 * Summary: Persist the version index durably (atomic rename plus fsync).
 *
 * Inputs: `source_root` folder, `index` payload, and `durability_root` stop directory for fsync.
 * Outputs: `Ok(())` when the updated index is durably persisted.
 * Side effects: Writes `index.json` under the source root.
 * Error handling: Returns contextual errors from serialization, IO, fsync, and rename operations.
 * Ties to other methods: Called after `write_json_atomic_durable` writes the manifest for a new version.
 * Why this exists: The index is the commit pointer; it must not reference versions that are not durable on disk.
 */
fn write_index(source_root: &Path, index: &VersionIndex, durability_root: &Path) -> Result<()> {
    let path = source_root.join("index.json");
    write_json_atomic_durable(&path, index, durability_root).with_context(|| {
        format!(
            "versioned::write_index failed to write version index {:?}",
            path
        )
    })
}

#[derive(serde::Serialize)]
struct LastScanReport<'a> {
    schema_version: u32,
    source_path: &'a str,
    created_at_unix: i64,
    committed_version_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_snapshot: Option<&'a crate::fs::snapshots::SourceSnapshotInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_snapshot_error: Option<&'a str>,
    read_failures: &'a [ReadFailure],
}

/**
 * Summary: Persist the last scan report for a watched source.
 *
 * Inputs: `source_root` destination store source directory, scanned `snapshot`, optional committed
 * version id, and `durability_root` for directory fsync bounds.
 * Outputs: `Ok(())` when the report is durably written.
 * Side effects: Writes `last_scan_report.json` under the source directory.
 * Error handling: Returns contextual errors for serialization, IO, fsync, and rename operations.
 * Ties to other methods: Called by `backup_one_folder` after scans and commits (or skipped commits).
 * Why this exists: Make read failures visible and inspectable even when a version cannot be safely created.
 */
fn write_scan_report(
    source_root: &Path,
    snapshot: &Manifest,
    committed_version_id: Option<&str>,
    durability_root: &Path,
) -> Result<()> {
    let path = source_root.join("last_scan_report.json");
    let report = LastScanReport {
        schema_version: snapshot.schema_version,
        source_path: snapshot.source_path.as_str(),
        created_at_unix: snapshot.created_at_unix,
        committed_version_id,
        source_snapshot: snapshot.source_snapshot.as_ref(),
        source_snapshot_error: snapshot.source_snapshot_error.as_deref(),
        read_failures: snapshot.read_failures.as_slice(),
    };
    write_json_atomic_durable(&path, &report, durability_root).with_context(|| {
        format!(
            "versioned::write_scan_report failed to write scan report {:?}",
            path
        )
    })
}

fn latest_manifest(manifests_root: &Path, index: &VersionIndex) -> Result<Option<Manifest>> {
    let last = match index.versions.last() {
        None => return Ok(None),
        Some(v) => v,
    };
    let path = manifests_root.join(format!("{}.json", last.id));
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("versioned::latest_manifest failed to read {:?}", path))?;
    let manifest: Manifest = serde_json::from_str(&raw)
        .with_context(|| format!("versioned::latest_manifest failed to parse {:?}", path))?;
    Ok(Some(manifest))
}

/**
 * Summary: Atomically write JSON and make it durable with fsync.
 *
 * Inputs: `path` destination file path, `value` JSON-serializable payload, and `durability_root`.
 * Outputs: `Ok(())` when the file is atomically replaced and durably committed.
 * Side effects: Writes a temp file, fsyncs it, renames it into place, and fsyncs the parent directory.
 * Error handling: Returns contextual errors for create, serialize, write, fsync, and rename failures.
 * Ties to other methods: Used for both manifests and `index.json` writes in the commit pipeline.
 * Why this exists: Without fsync, a power loss can leave a missing or truncated file after an atomic rename.
 */
fn write_json_atomic_durable<T: serde::Serialize>(
    path: &Path,
    value: &T,
    durability_root: &Path,
) -> Result<()> {
    let parent = path
        .parent()
        .context("versioned::write_json_atomic_durable missing parent directory")?;
    create_dir_all_durable(parent, durability_root).with_context(|| {
        format!(
            "versioned::write_json_atomic_durable failed to create durable parent {:?}",
            parent
        )
    })?;

    let raw = serde_json::to_string_pretty(value)
        .context("versioned::write_json_atomic_durable failed to serialize JSON")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .context("versioned::write_json_atomic_durable failed to create temp file")?;
    temp.write_all(raw.as_bytes())
        .context("versioned::write_json_atomic_durable failed writing JSON bytes")?;
    temp.flush()
        .context("versioned::write_json_atomic_durable failed to flush temp file")?;
    temp.as_file()
        .sync_all()
        .context("versioned::write_json_atomic_durable failed to fsync temp file")?;
    temp.persist(path).map_err(|e| {
        anyhow::anyhow!(
            "versioned::write_json_atomic_durable failed to persist {:?}: {}",
            path,
            e
        )
    })?;
    sync_dir(parent)?;
    Ok(())
}

fn sha256_file(path: &Path, timeout_seconds: u64, retry_delays: &[Duration]) -> Result<String> {
    let label = format!("versioned::sha256_file {:?}", path);
    let (hash, _) = retry_with_backoff(&label, timeout_seconds, retry_delays, || {
        sha256_file_once(path, timeout_seconds)
    })?;
    Ok(hash)
}

/**
 * Summary: Compute a SHA-256 hash for a file in a single attempt.
 *
 * Inputs: File path and timeout in seconds.
 * Outputs: Hex-encoded SHA-256 hash string.
 * Side effects: Reads file contents from disk.
 * Error handling: Returns contextual errors on open/read failures and a timeout error when exceeded.
 * Ties to other methods: Used by `sha256_file` which applies retry/backoff around this function.
 * Why this exists: Allow retry to restart hashing cleanly after transient read errors.
 */
fn sha256_file_once(path: &Path, timeout_seconds: u64) -> Result<String> {
    let start = Instant::now();
    let mut file = fs::File::open(path)
        .with_context(|| format!("versioned::sha256_file_once failed to open {:?}", path))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        if timeout_seconds > 0 && start.elapsed().as_secs() > timeout_seconds {
            anyhow::bail!(
                "versioned::sha256_file_once timed out after {}s hashing {:?}",
                timeout_seconds,
                path
            );
        }
        let n = file
            .read(&mut buf)
            .with_context(|| format!("versioned::sha256_file_once failed reading {:?}", path))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(sha256_hex(&hasher.finalize()))
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{:02x}", b);
    }
    out
}

pub(crate) fn normalize_rel(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    if s.starts_with("./") {
        s.trim_start_matches("./").to_string()
    } else {
        s
    }
}

fn is_hidden(rel: &Path) -> bool {
    rel.components().any(|c| {
        let name = c.as_os_str().to_string_lossy();
        name.starts_with('.')
    })
}

fn build_ignore_set(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for p in patterns {
        if let Ok(g) = Glob::new(p) {
            let _ = builder.add(g);
        }
    }
    builder
        .build()
        .context("versioned::build_ignore_set failed to build ignore set")
}

fn gc_unreferenced_blobs(store_root: &Path, blobs_root: &Path) -> Result<()> {
    let sources = sources_root(store_root);
    if !sources.exists() {
        return Ok(());
    }
    let mut referenced: HashSet<String> = HashSet::new();
    for src in fs::read_dir(&sources).with_context(|| {
        format!(
            "versioned::gc_unreferenced_blobs failed to read {:?}",
            sources
        )
    })? {
        let src = src?;
        let manifests = src.path().join("manifests");
        if !manifests.exists() {
            continue;
        }
        for entry in fs::read_dir(&manifests)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let raw = fs::read_to_string(&path).with_context(|| {
                format!("versioned::gc_unreferenced_blobs failed to read {:?}", path)
            })?;
            let manifest: Manifest = serde_json::from_str(&raw).with_context(|| {
                format!(
                    "versioned::gc_unreferenced_blobs failed to parse {:?}",
                    path
                )
            })?;
            for e in manifest.entries.values() {
                if let Some(h) = e.sha256.as_ref() {
                    referenced.insert(h.clone());
                }
            }
        }
    }

    if !blobs_root.exists() {
        return Ok(());
    }
    for prefix in fs::read_dir(blobs_root).with_context(|| {
        format!(
            "versioned::gc_unreferenced_blobs failed to read {:?}",
            blobs_root
        )
    })? {
        let prefix = prefix?;
        if !prefix.file_type()?.is_dir() {
            continue;
        }
        for blob in fs::read_dir(prefix.path())? {
            let blob = blob?;
            let path = blob.path();
            if !blob.file_type()?.is_file() {
                continue;
            }
            let name = match path.file_name().and_then(|n| n.to_str()) {
                None => continue,
                Some(n) => n,
            };
            if !referenced.contains(name) {
                let _ = fs::remove_file(&path);
            }
        }
    }
    Ok(())
}
