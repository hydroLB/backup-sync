use super::model::{
    Manifest, ManifestEntry, ManifestEntryKind, ReadFailure, ReadFailurePhase, VersionIndex,
    VersionInfo,
};
use crate::config::model::{Config, Destination, WatchedKind, WatchedPath};
use crate::encryption::blobs::BlobCodec;
use crate::fs::snapshots::prepare_source_view;
use crate::hashing;
use crate::logging::redact_path;
use crate::state::models::SafetyWarning;
use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub struct FolderDescriptor {
    pub source_path: PathBuf,
    pub destination_root: PathBuf,
    pub keep_versions: usize,
}

#[derive(Debug, Default, Clone, Copy)]
struct ManifestSize {
    file_count: u64,
    file_bytes: u64,
}

#[derive(Debug, Default, Clone)]
pub struct FolderBackupResult {
    pub changed: bool,
    pub version_id: Option<String>,
    pub blobs_written: usize,
    pub bytes_written: u64,
    pub safety_warning: Option<SafetyWarning>,
}

#[derive(Debug, Default, Clone)]
pub struct BackupCycleResult {
    pub folders_scanned: usize,
    pub versions_created: usize,
    pub blobs_written: usize,
    pub bytes_written: u64,
    pub safety_warnings: Vec<SafetyWarning>,
}

#[derive(Debug, Default, Clone)]
pub struct SimulationSummary {
    pub watched: usize,
    pub versions_would_create: usize,
    pub adds: usize,
    pub modifies: usize,
    pub deletes: usize,
    pub items: usize,
    pub blobs_to_write: usize,
    pub bytes_to_write: u64,
    pub sample: Vec<String>,
    pub read_failures: usize,
    pub snapshot_errors: usize,
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

    let blob_codec = BlobCodec::from_config(cfg)
        .context("versioned::run_backup_cycle failed to initialize blob codec")?;

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
        let result = backup_one_folder(cfg, watched, &descriptor, &blob_codec)?;
        cycle.folders_scanned += 1;
        if result.changed {
            cycle.versions_created += 1;
        }
        if let Some(w) = result.safety_warning {
            cycle.safety_warnings.push(w);
        }
        cycle.blobs_written += result.blobs_written;
        cycle.bytes_written += result.bytes_written;
    }
    Ok(cycle)
}

/**
 * Summary: Removes a kept safety version (pending or pinned baseline) for a watched source.
 *
 * Inputs: Destination root containing the versioned store and the watched source path.
 * Outputs: `Ok(Some(version_id))` when a kept version was removed, `Ok(None)` when none existed.
 * Side effects: Updates `index.json`, deletes the pinned manifest file, and runs blob GC.
 * Error handling: Returns contextual errors for index IO/serialization and manifest deletion failures.
 * Ties to other methods: Clears pins set by `maybe_pin_large_deletion_baseline` and relies on `gc_unreferenced_blobs`.
 * Why this exists: Let users explicitly drop the extra baseline when a shrink was expected.
 */
pub fn remove_kept_safety_version(
    destination_root: &Path,
    source_path: &Path,
) -> Result<Option<String>> {
    let store_root = store_root(destination_root);
    let blobs_root = blobs_root(&store_root);

    let source_id = hashing::sha256_hex(source_path.to_string_lossy().as_bytes());
    let source_root = sources_root(&store_root).join(&source_id);
    let manifests_root = source_root.join("manifests");

    let mut index = load_index(&source_root, source_path)?;
    let kept = match index
        .safety_pinned_version_id
        .clone()
        .or(index.safety_pending_version_id.clone())
    {
        None => return Ok(None),
        Some(v) => v,
    };

    let was_listed = index.versions.iter().any(|v| v.id == kept);
    if !was_listed {
        index.safety_pinned_version_id = None;
        index.safety_pending_version_id = None;
        index.safety_pending_first_seen_unix = None;
        write_index(&source_root, &index, destination_root)?;
        return Ok(None);
    }

    index.versions.retain(|v| v.id != kept);
    index.safety_pinned_version_id = None;
    index.safety_pending_version_id = None;
    index.safety_pending_first_seen_unix = None;
    write_index(&source_root, &index, destination_root)?;

    let manifest_path = manifests_root.join(format!("{kept}.json"));
    fs::remove_file(&manifest_path).with_context(|| {
        format!(
            "versioned::remove_kept_safety_version failed to delete manifest {:?}",
            manifest_path
        )
    })?;
    let _ = sync_dir(&manifests_root);
    gc_unreferenced_blobs(&store_root, &blobs_root)?;

    Ok(Some(kept))
}

/**
 * Summary: Compute a stable "size" summary for a manifest for safety comparisons.
 *
 * Inputs: A manifest representing a watched file or directory state.
 * Outputs: `ManifestSize` containing file count and total file bytes.
 * Side effects: None.
 * Error handling: None.
 * Ties to other methods: Used by `maybe_pin_large_deletion_baseline` to detect large shrink events.
 * Why this exists: Provide a deterministic, cheap metric to detect suspicious mass deletions.
 */
fn manifest_size(manifest: &Manifest) -> ManifestSize {
    let mut out = ManifestSize::default();
    for e in manifest.entries.values() {
        if e.kind != ManifestEntryKind::File {
            continue;
        }
        out.file_count = out.file_count.saturating_add(1);
        out.file_bytes = out.file_bytes.saturating_add(e.len);
    }
    out
}

/**
 * Summary: Compute the maximum shrink ratio between two manifest sizes.
 *
 * Inputs: previous and next manifest size summaries.
 * Outputs: Optional shrink ratio in `[0.0, 1.0]` when a baseline is available.
 * Side effects: None.
 * Error handling: None.
 * Ties to other methods: Used by `maybe_pin_large_deletion_baseline`.
 * Why this exists: Detect suspicious deletions using both byte and file-count heuristics.
 */
fn shrink_ratio(prev: ManifestSize, next: ManifestSize) -> Option<f64> {
    let bytes_ratio = if prev.file_bytes > 0 && next.file_bytes <= prev.file_bytes {
        (prev.file_bytes - next.file_bytes) as f64 / prev.file_bytes as f64
    } else {
        0.0
    };
    let count_ratio = if prev.file_count > 0 && next.file_count <= prev.file_count {
        (prev.file_count - next.file_count) as f64 / prev.file_count as f64
    } else {
        0.0
    };
    if prev.file_bytes == 0 && prev.file_count == 0 {
        None
    } else {
        Some(bytes_ratio.max(count_ratio).clamp(0.0, 1.0))
    }
}

/**
 * Summary: Pin a pre-change version when a watched path shrinks dramatically.
 *
 * Inputs: Config, watched path, previous manifest, next manifest, and a mutable version index.
 * Outputs: A `SafetyWarning` when pinning occurred; otherwise `None`.
 * Side effects: Mutates the index to set `safety_pinned_version_id` for retention.
 * Error handling: Best-effort, returns `None` when required metadata is unavailable.
 * Ties to other methods: Called by `backup_one_folder` before pruning, and persisted via `write_index`.
 * Why this exists: Prevent silent "bad runs" from pruning the last known-good full version.
 */
fn maybe_pin_large_deletion_baseline(
    cfg: &Config,
    watched: &WatchedPath,
    prev: &Manifest,
    next: &Manifest,
    index: &mut VersionIndex,
    manifests_root: &Path,
    had_read_failures: bool,
) -> (Option<SafetyWarning>, bool) {
    let threshold = cfg.runtime.large_deletion_keep_extra_threshold_ratio;
    if had_read_failures {
        return (None, false);
    }
    if !(threshold > 0.0) {
        return (None, false);
    }

    let mut index_changed = false;

    if let Some(pinned) = index.safety_pinned_version_id.as_deref() {
        let still_present = index.versions.iter().any(|v| v.id == pinned);
        if !still_present {
            index.safety_pinned_version_id = None;
            index_changed = true;
        }
    }
    if let Some(pending) = index.safety_pending_version_id.as_deref() {
        let still_present = index.versions.iter().any(|v| v.id == pending);
        if !still_present {
            index.safety_pending_version_id = None;
            index.safety_pending_first_seen_unix = None;
            index_changed = true;
        }
    }

    if index.safety_pinned_version_id.is_some() {
        return (None, index_changed);
    }

    let baseline_manifest = if let Some(pending_id) = index.safety_pending_version_id.as_deref() {
        let path = manifests_root.join(format!("{pending_id}.json"));
        match fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<Manifest>(&raw) {
                Ok(m) => Some(m),
                Err(e) => {
                    tracing::warn!(
                        watched_path = %redact_path(&watched.path),
                        error = %e,
                        "versioned safety baseline pending manifest parse failed; clearing pending"
                    );
                    index.safety_pending_version_id = None;
                    index.safety_pending_first_seen_unix = None;
                    index_changed = true;
                    None
                }
            },
            Err(e) => {
                tracing::warn!(
                    watched_path = %redact_path(&watched.path),
                    error = %e,
                    "versioned safety baseline pending manifest read failed; clearing pending"
                );
                index.safety_pending_version_id = None;
                index.safety_pending_first_seen_unix = None;
                index_changed = true;
                None
            }
        }
    } else {
        None
    };

    let prev_size = manifest_size(baseline_manifest.as_ref().unwrap_or(prev));
    let next_size = manifest_size(next);
    let Some(ratio) = shrink_ratio(prev_size, next_size) else {
        return (None, index_changed);
    };
    if ratio < threshold {
        if index.safety_pending_version_id.is_some() {
            index.safety_pending_version_id = None;
            index.safety_pending_first_seen_unix = None;
            index_changed = true;
        }
        return (None, index_changed);
    }

    if let Some(pending_id) = index.safety_pending_version_id.clone() {
        // Second consecutive clean scan with the shrink still present: promote to a permanent pin.
        index.safety_pinned_version_id = Some(pending_id);
        index.safety_pending_version_id = None;
        index.safety_pending_first_seen_unix = None;
        index_changed = true;
        return (None, index_changed);
    }

    let baseline_id = index
        .versions
        .iter()
        .max_by(|a, b| {
            a.created_at_unix
                .cmp(&b.created_at_unix)
                .then_with(|| a.id.cmp(&b.id))
        })
        .map(|v| v.id.clone());
    let Some(baseline_id) = baseline_id else {
        return (None, index_changed);
    };

    // First detection: keep the baseline immediately (pending) so pruning cannot delete it.
    index.safety_pending_version_id = Some(baseline_id.clone());
    index.safety_pending_first_seen_unix = Some(chrono::Utc::now().timestamp());
    index_changed = true;

    let pct = (ratio * 100.0).round().clamp(0.0, 100.0) as u32;
    (
        Some(SafetyWarning {
            ts: chrono::Utc::now().timestamp(),
            message: format!(
                "Protected path is ~{pct}% smaller than before. Extra version kept while we confirm. If this was expected, remove it."
            ),
            watched_path: Some(watched.path.to_string_lossy().into_owned()),
            kept_version_id: Some(baseline_id),
        }),
        index_changed,
    )
}

/// Summary: Simulate a versioned backup cycle without writing to the destination store.
///
/// Inputs: a validated config with watched paths and destinations.
/// Outputs: a `SimulationSummary` describing what would change and what would be written.
/// Side effects: Reads filesystem metadata and file contents for hashing; reads existing store manifests/blobs.
/// Error handling: Returns contextual errors for invalid config references, scan/hashing failures, and store reads.
/// Ties to other methods: Mirrors the scan and equivalence logic used by `run_backup_cycle` and `backup_one_folder`.
/// Why this exists: Provide a safe preview of changes and expected IO before running a real backup.
pub fn simulate_backup_cycle(cfg: &Config) -> Result<SimulationSummary> {
    let sample_limit = cfg.runtime.simulation_sample_limit;
    simulate_backup_cycle_with_sample_limit(cfg, sample_limit)
}

/// Summary: Simulate a versioned backup cycle with an explicit sample limit override.
///
/// Inputs: config and a maximum number of sample paths to return.
/// Outputs: a `SimulationSummary` describing planned changes.
/// Side effects: Same as `simulate_backup_cycle`.
/// Error handling: Same as `simulate_backup_cycle`.
/// Ties to other methods: Used by tests and callers that need a smaller sample cap.
/// Why this exists: Allow deterministic and bounded simulation output.
pub fn simulate_backup_cycle_with_sample_limit(
    cfg: &Config,
    sample_limit: usize,
) -> Result<SimulationSummary> {
    let destinations_by_id: HashMap<&str, &Destination> = cfg
        .destinations
        .iter()
        .map(|d| (d.id.as_str(), d))
        .collect();

    let retry_delays = cfg.execution.retry_delays();
    let mut summary = SimulationSummary::default();
    let mut remaining_sample = sample_limit;

    for watched in cfg.watched.iter().filter(|w| w.enabled) {
        let dest = destinations_by_id
            .get(watched.destination_id.as_str())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "versioned::simulate_backup_cycle missing destination id {} for watched path {}",
                    watched.destination_id,
                    redact_path(&watched.path)
                )
            })?;

        let store_root = store_root(&dest.path);
        let blobs_root = blobs_root(&store_root);
        let source_id = hashing::sha256_hex(watched.path.to_string_lossy().as_bytes());
        let source_root = sources_root(&store_root).join(&source_id);
        let manifests_root = source_root.join("manifests");
        let index = load_index(&source_root, &watched.path)?;
        let prev = latest_manifest(&manifests_root, &index)?;

        let source_view = prepare_source_view(
            &watched.path,
            cfg.runtime.source_snapshots_enabled,
            cfg.runtime.source_snapshot_timeout_seconds,
        )?;
        let snapshot_error = source_view.snapshot_error().map(|s| s.to_string());

        let mut next = scan_snapshot(cfg, watched, source_view.scan_path(), &prev, &retry_delays)?;
        next.source_snapshot = source_view.snapshot().cloned();
        next.source_snapshot_error = snapshot_error.clone();

        summary.watched += 1;
        summary.read_failures += next.read_failures.len();
        if snapshot_error.is_some() {
            summary.snapshot_errors += 1;
        }

        let would_create_version = match prev.as_ref() {
            None => true,
            Some(prev_manifest) => !manifests_equivalent(prev_manifest, &next),
        };
        if would_create_version {
            summary.versions_would_create += 1;
        }

        let (adds, modifies, deletes, blobs_to_write, bytes_to_write, sample_used) =
            diff_for_simulation(watched, prev.as_ref(), &next, &blobs_root, remaining_sample);

        summary.adds += adds;
        summary.modifies += modifies;
        summary.deletes += deletes;
        summary.items += adds + modifies + deletes;
        summary.blobs_to_write += blobs_to_write;
        summary.bytes_to_write += bytes_to_write;

        if remaining_sample > 0 && !sample_used.is_empty() {
            let to_take = remaining_sample.min(sample_used.len());
            summary.sample.extend(sample_used.into_iter().take(to_take));
            remaining_sample = remaining_sample.saturating_sub(to_take);
        }
    }

    Ok(summary)
}

fn backup_one_folder(
    cfg: &Config,
    watched: &WatchedPath,
    desc: &FolderDescriptor,
    blob_codec: &BlobCodec,
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

    let source_id = hashing::sha256_hex(desc.source_path.to_string_lossy().as_bytes());
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
    let snapshot_error = source_view.snapshot_error().map(|s| s.to_string());
    if let Some(err) = source_view.snapshot_error() {
        tracing::warn!(
            source_path = %redact_path(&watched.path),
            error = %err,
            "versioned backup could not obtain a snapshot view; scanning live filesystem"
        );
    }
    let mut snapshot = scan_snapshot(cfg, watched, source_view.scan_path(), &prev, &retry_delays)?;
    snapshot.source_snapshot = source_view.snapshot().cloned();
    snapshot.source_snapshot_error = snapshot_error.clone();
    // Snapshot availability is best-effort and does not imply the scan is incomplete.
    // Only gate pruning/safety on real read failures (missing hashes, IO failures).
    let mut had_read_failures = !snapshot.read_failures.is_empty();
    let changed = match &prev {
        None => true,
        Some(prev_manifest) => !manifests_equivalent(prev_manifest, &snapshot),
    };
    if !changed {
        let mut safety_warning: Option<SafetyWarning> = None;
        if let Some(prev_manifest) = prev.as_ref() {
            let (warn, index_changed) = maybe_pin_large_deletion_baseline(
                cfg,
                watched,
                prev_manifest,
                &snapshot,
                &mut index,
                &manifests_root,
                had_read_failures,
            );
            safety_warning = warn;
            if index_changed {
                write_index(&source_root, &index, &desc.destination_root)?;
            }
        }
        write_scan_report(&source_root, &snapshot, None, &desc.destination_root)?;
        return Ok(FolderBackupResult {
            changed: false,
            safety_warning,
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
            blob_codec,
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
        let mut safety_warning: Option<SafetyWarning> = None;
        if let Some(prev_manifest) = prev.as_ref() {
            let (warn, index_changed) = maybe_pin_large_deletion_baseline(
                cfg,
                watched,
                prev_manifest,
                &snapshot,
                &mut index,
                &manifests_root,
                had_read_failures,
            );
            safety_warning = warn;
            if index_changed {
                write_index(&source_root, &index, &desc.destination_root)?;
            }
        }
        write_scan_report(&source_root, &snapshot, None, &desc.destination_root)?;
        return Ok(FolderBackupResult {
            changed: false,
            blobs_written: written.blobs_written,
            bytes_written: written.bytes_written,
            safety_warning,
            ..Default::default()
        });
    }

    if let Some(prev_manifest) = prev.as_ref() {
        let (warn, _index_changed) = maybe_pin_large_deletion_baseline(
            cfg,
            watched,
            prev_manifest,
            &snapshot,
            &mut index,
            &manifests_root,
            had_read_failures,
        );
        written.safety_warning = warn;
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

    let mut pruned: Vec<VersionInfo> = Vec::new();
    if !had_read_failures {
        let mut protected: Vec<&str> = Vec::new();
        if let Some(pinned) = index.safety_pinned_version_id.as_deref() {
            protected.push(pinned);
        }
        if let Some(pending) = index.safety_pending_version_id.as_deref() {
            if !protected.iter().any(|p| *p == pending) {
                protected.push(pending);
            }
        }
        let allowed = desc.keep_versions + protected.len();
        while index.versions.len() > allowed {
            let remove_pos = index
                .versions
                .iter()
                .position(|v| !protected.iter().any(|p| *p == v.id.as_str()));
            let Some(pos) = remove_pos else { break };
            pruned.push(index.versions.remove(pos));
        }
    }

    write_index(&source_root, &index, &desc.destination_root)?;
    write_scan_report(
        &source_root,
        &snapshot,
        Some(&version_id),
        &desc.destination_root,
    )?;

    if !pruned.is_empty() {
        for v in pruned.iter() {
            let path = manifests_root.join(format!("{}.json", v.id));
            let _ = fs::remove_file(&path);
        }
        let _ = sync_dir(&manifests_root);
        gc_unreferenced_blobs(&store_root, &blobs_root)?;
    }

    Ok(written)
}

fn diff_for_simulation(
    watched: &WatchedPath,
    prev: Option<&Manifest>,
    next: &Manifest,
    blobs_root: &Path,
    sample_limit: usize,
) -> (usize, usize, usize, usize, u64, Vec<String>) {
    let mut adds = 0usize;
    let mut modifies = 0usize;
    let mut deletes = 0usize;
    let mut blobs_to_write = 0usize;
    let mut bytes_to_write = 0u64;
    let mut sample: Vec<String> = Vec::new();

    for (k, entry) in next.entries.iter() {
        let change = match prev.and_then(|p| p.entries.get(k)) {
            None => Some('+'),
            Some(prev_e) => {
                if prev_e.kind != entry.kind {
                    Some('~')
                } else if entry.kind == ManifestEntryKind::File && prev_e.sha256 != entry.sha256 {
                    Some('~')
                } else {
                    None
                }
            }
        };
        if let Some(prefix) = change {
            if prefix == '+' {
                adds += 1;
            } else {
                modifies += 1;
            }
            if entry.kind == ManifestEntryKind::File {
                if let Some(hash) = entry.sha256.as_deref() {
                    let blob = blob_path(blobs_root, hash);
                    if !blob.exists() {
                        blobs_to_write += 1;
                        bytes_to_write = bytes_to_write.saturating_add(entry.len);
                    }
                }
            }
            if sample.len() < sample_limit {
                sample.push(format_change_sample(
                    prefix,
                    watched,
                    entry.rel_path.as_str(),
                ));
            }
        }
    }

    if let Some(prev_manifest) = prev {
        for (k, prev_entry) in prev_manifest.entries.iter() {
            if next.entries.contains_key(k) {
                continue;
            }
            deletes += 1;
            if sample.len() < sample_limit {
                sample.push(format_change_sample(
                    '-',
                    watched,
                    prev_entry.rel_path.as_str(),
                ));
            }
        }
    }

    (
        adds,
        modifies,
        deletes,
        blobs_to_write,
        bytes_to_write,
        sample,
    )
}

fn format_change_sample(prefix: char, watched: &WatchedPath, rel_path: &str) -> String {
    let full = match watched.kind {
        WatchedKind::File => watched.path.clone(),
        WatchedKind::Directory => watched.path.join(Path::new(rel_path)),
    };
    format!("{prefix} {}", redact_path(&full))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::model::{WatchedKind, WatchedPath};
    use crate::config::registry::config_defaults;
    use std::io::Write;

    /**
     * Summary: Build a minimal config for versioned backup tests.
     *
     * Inputs: Source and destination paths and keep versions count.
     * Outputs: A config suitable for calling `run_backup_cycle`.
     * Side effects: None.
     * Error handling: Propagates default config creation failures.
     * Ties to other methods: Used by durability and pruning tests in this module.
     * Why this exists: Avoid duplicating verbose config initialization in each test.
     */
    fn test_config(source: &Path, destination: &Path, keep_versions: usize) -> Result<Config> {
        let mut cfg = config_defaults()?;
        cfg.backup_root = destination.to_path_buf();
        cfg.destinations = vec![Destination {
            id: "default".to_string(),
            path: destination.to_path_buf(),
            label: Some("Primary".to_string()),
            max_backups_per_file: None,
            replicate_to: vec![],
        }];
        cfg.watched = vec![WatchedPath {
            path: source.to_path_buf(),
            kind: WatchedKind::Directory,
            enabled: true,
            destination_id: "default".to_string(),
            max_backups_per_file: Some(keep_versions),
        }];
        Ok(cfg)
    }

    /**
     * Summary: Ensure pruning does not delete manifests unless the index commit succeeds.
     *
     * Inputs: None.
     * Outputs: None.
     * Side effects: Creates temp directories, writes files, runs backups, and toggles permissions.
     * Error handling: Fails the test with actionable context on unexpected backup behavior.
     * Ties to other methods: Exercises `backup_one_folder` prune pipeline and `write_index` durability.
     * Why this exists: A crash or write failure during `index.json` update must not strand an index that
     * points at a deleted manifest.
     */
    #[test]
    #[cfg(target_family = "unix")]
    fn pruning_is_deferred_until_index_commit() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let src_dir = tempfile::tempdir().context("test temp src")?;
        let dst_dir = tempfile::tempdir().context("test temp dst")?;

        let file_path = src_dir.path().join("a.txt");
        fs::write(&file_path, "one\n").context("write initial source")?;

        let cfg = test_config(src_dir.path(), dst_dir.path(), 1)?;
        run_backup_cycle(&cfg).context("first backup cycle")?;

        let source_id =
            crate::hashing::sha256_hex(cfg.watched[0].path.to_string_lossy().as_bytes());
        let store = store_root(dst_dir.path());
        let source_root = sources_root(&store).join(&source_id);
        let manifests_root = source_root.join("manifests");

        let index_path = source_root.join("index.json");
        let raw = fs::read_to_string(&index_path).context("read index after first run")?;
        let index: VersionIndex =
            serde_json::from_str(&raw).context("parse index after first run")?;
        let first_version = index
            .versions
            .last()
            .context("expected at least one version")?
            .id
            .clone();
        let first_manifest = manifests_root.join(format!("{}.json", first_version));
        assert!(
            first_manifest.exists(),
            "expected first manifest to exist at {:?}",
            first_manifest
        );

        // Block index updates without blocking manifest writes by removing write permission on
        // `source_root` while leaving `manifests_root` writable.
        fs::set_permissions(&source_root, fs::Permissions::from_mode(0o555))
            .context("chmod source_root read-only")?;

        let mut f = fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&file_path)
            .context("open source for modification")?;
        writeln!(f, "two").context("mutate source content")?;
        f.flush().context("flush mutated source content")?;

        let err = run_backup_cycle(&cfg).expect_err("second backup should fail due to index write");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("write_json_atomic_durable") || msg.contains("write_index"),
            "expected write_index failure, got: {msg}"
        );

        // The pre-existing committed version must remain restorable via its manifest.
        assert!(
            first_manifest.exists(),
            "expected committed manifest to remain when index commit fails: {:?}",
            first_manifest
        );

        Ok(())
    }
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
    blob_codec: &BlobCodec,
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
        write_blob_once(src_path, blob_path, timeout_seconds, blob_codec)
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
fn write_blob_once(
    src_path: &Path,
    blob_path: &Path,
    timeout_seconds: u64,
    blob_codec: &BlobCodec,
) -> Result<u64> {
    let start = Instant::now();
    let parent = blob_path
        .parent()
        .context("versioned::write_blob_once missing blob parent")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .context("versioned::write_blob_once temp create")?;

    if timeout_seconds > 0 && start.elapsed().as_secs() > timeout_seconds {
        anyhow::bail!(
            "versioned::write_blob_once timed out after {}s writing {:?}",
            timeout_seconds,
            src_path
        );
    }
    blob_codec
        .write_blob_from_file_to_writer(src_path, temp.as_file_mut(), timeout_seconds)
        .with_context(|| {
            format!(
                "versioned::write_blob_once failed encoding blob from {:?}",
                src_path
            )
        })?;
    let written = temp.as_file().metadata().map(|m| m.len()).unwrap_or(0);
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
            match scan_one_file(scan_path, &name, &cfg.hashing, retry_delays, &prev_entries) {
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
                    match scan_one_file(path, &rel_str, &cfg.hashing, retry_delays, &prev_entries) {
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
    tuning: &crate::config::model::HashingTuning,
    retry_delays: &[Duration],
    prev: &HashMap<&str, &ManifestEntry>,
) -> Result<ManifestEntry> {
    let timeout_seconds = tuning.timeout_seconds;
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
    let hash = sha256_file(abs_path, tuning, retry_delays)?;
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
            safety_pinned_version_id: None,
            safety_pending_version_id: None,
            safety_pending_first_seen_unix: None,
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

fn sha256_file(
    path: &Path,
    tuning: &crate::config::model::HashingTuning,
    retry_delays: &[Duration],
) -> Result<String> {
    let label = format!("versioned::sha256_file {:?}", path);
    let (hash, _) = retry_with_backoff(&label, tuning.timeout_seconds, retry_delays, || {
        hashing::sha256_file_hex_with_tuning(path, tuning)
    })?;
    Ok(hash)
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
