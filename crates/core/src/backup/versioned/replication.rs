use super::model::{Manifest, ManifestEntryKind, VersionIndex};
use super::store::{blob_path, blobs_root, sources_root, store_root};
use crate::config::model::{Config, Destination};
use crate::logging::redact_path;
use crate::scheduling::throttling::Throttle;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};
use walkdir::WalkDir;

#[derive(Debug, Default, Clone)]
pub struct ReplicationSummary {
    pub pairs_attempted: usize,
    pub pairs_ok: usize,
    pub pairs_failed: usize,
    pub manifests_copied: usize,
    pub blobs_copied: usize,
    pub bytes_copied: u64,
    pub manifests_deleted: usize,
    pub targets_failed: Vec<String>,
    pub message: String,
}

#[derive(Debug, Default, Clone)]
struct PairCounters {
    manifests_copied: usize,
    blobs_copied: usize,
    bytes_copied: u64,
    manifests_deleted: usize,
}

/// Summary: Replicate versioned store data from configured destinations to their replicas.
///
/// Inputs: loaded config containing destinations with optional `replicate_to` lists.
///
/// Outputs: a `ReplicationSummary` for UI and logs.
///
/// Side effects: Copies manifests, blobs, and index files into replica destinations.
///
/// Error handling: Best-effort per replica; returns `Ok` with failure counts unless a non-recoverable internal error occurs.
///
/// Ties to other methods: Intended to run after `run_backup_cycle` in the daemon and CLI for 3-2-1 workflows.
///
/// Why this exists: A second destination provides an immediate redundant copy without changing the manifest model.
pub fn replicate_configured_stores(cfg: &Config) -> Result<ReplicationSummary> {
    let by_id: HashMap<&str, &Destination> = cfg
        .destinations
        .iter()
        .map(|d| (d.id.as_str(), d))
        .collect();
    let mut summary = ReplicationSummary::default();
    let mut counters = PairCounters::default();

    if !cfg.runtime.replication_enabled {
        summary.message = "replication disabled (runtime.replication_enabled=false)".to_string();
        return Ok(summary);
    }

    for src in cfg.destinations.iter() {
        if src.replicate_to.is_empty() {
            continue;
        }
        for target_id in src.replicate_to.iter() {
            summary.pairs_attempted += 1;
            let Some(dst) = by_id.get(target_id.as_str()).copied() else {
                summary.pairs_failed += 1;
                summary.targets_failed.push(target_id.clone());
                continue;
            };
            match replicate_one_pair(cfg, src, dst) {
                Ok(pair) => {
                    summary.pairs_ok += 1;
                    counters.manifests_copied += pair.manifests_copied;
                    counters.blobs_copied += pair.blobs_copied;
                    counters.bytes_copied = counters.bytes_copied.saturating_add(pair.bytes_copied);
                    counters.manifests_deleted += pair.manifests_deleted;
                }
                Err(e) => {
                    summary.pairs_failed += 1;
                    summary.targets_failed.push(dst.id.clone());
                    tracing::warn!(
                        source_id = %src.id,
                        target_id = %dst.id,
                        error = %e,
                        "versioned replication failed for destination pair"
                    );
                }
            }
        }
    }

    summary.manifests_copied = counters.manifests_copied;
    summary.blobs_copied = counters.blobs_copied;
    summary.bytes_copied = counters.bytes_copied;
    summary.manifests_deleted = counters.manifests_deleted;
    summary.message = format!(
        "replication pairs_ok={}/{} manifests_copied={} blobs_copied={} bytes_copied={} manifests_deleted={} targets_failed={}",
        summary.pairs_ok,
        summary.pairs_attempted,
        summary.manifests_copied,
        summary.blobs_copied,
        summary.bytes_copied,
        summary.manifests_deleted,
        summary.targets_failed.len()
    );
    Ok(summary)
}

/// Summary: Replicate one source destination store into one replica destination store.
///
/// Inputs: config plus a source and destination entry.
///
/// Outputs: per-pair replication counters.
///
/// Side effects: Reads from the source store and writes missing content into the replica store.
///
/// Error handling: Returns contextual errors when the replica is unreachable or when store reads are inconsistent.
///
/// Ties to other methods: Called by `replicate_configured_stores`.
///
/// Why this exists: Keep pair replication logic isolated for easier testing and observability.
fn replicate_one_pair(cfg: &Config, src: &Destination, dst: &Destination) -> Result<PairCounters> {
    if !dst.path.exists() || !dst.path.is_dir() {
        anyhow::bail!(
            "versioned::replicate_one_pair replica destination not available at {}",
            redact_path(&dst.path)
        );
    }

    let src_store = store_root(&src.path);
    if !src_store.exists() || !src_store.is_dir() {
        // Nothing to replicate yet.
        return Ok(PairCounters::default());
    }
    let dst_store = store_root(&dst.path);
    fs::create_dir_all(&dst_store).with_context(|| {
        format!(
            "versioned::replicate_one_pair failed to create replica store root {}",
            redact_path(&dst_store)
        )
    })?;

    let src_sources = sources_root(&src_store);
    let src_blobs = blobs_root(&src_store);
    let dst_sources = sources_root(&dst_store);
    let dst_blobs = blobs_root(&dst_store);
    fs::create_dir_all(&dst_sources).with_context(|| {
        format!(
            "versioned::replicate_one_pair failed to create replica sources root {}",
            redact_path(&dst_sources)
        )
    })?;
    fs::create_dir_all(&dst_blobs).with_context(|| {
        format!(
            "versioned::replicate_one_pair failed to create replica blobs root {}",
            redact_path(&dst_blobs)
        )
    })?;

    let mut pair = PairCounters::default();
    for entry in WalkDir::new(&src_sources)
        .min_depth(1)
        .max_depth(1)
        .follow_links(false)
    {
        let entry = entry.context("versioned::replicate_one_pair walk source roots")?;
        if !entry.file_type().is_dir() {
            continue;
        }
        let source_root = entry.path();
        let source_id = source_root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "<unknown>".to_string());
        let src_index_path = source_root.join("index.json");
        if !src_index_path.exists() {
            continue;
        }
        let src_index: VersionIndex = read_json(&src_index_path)
            .with_context(|| format!("versioned::replicate_one_pair parse {:?}", src_index_path))?;

        let dst_source_root = dst_sources.join(source_id.as_str());
        let dst_index_path = dst_source_root.join("index.json");
        fs::create_dir_all(&dst_source_root).with_context(|| {
            format!(
                "versioned::replicate_one_pair failed to create replica source root {}",
                redact_path(&dst_source_root)
            )
        })?;

        let dst_index: Option<VersionIndex> = if dst_index_path.exists() {
            Some(read_json(&dst_index_path).with_context(|| {
                format!(
                    "versioned::replicate_one_pair parse replica index {:?}",
                    dst_index_path
                )
            })?)
        } else {
            None
        };
        let mut dst_versions: HashSet<String> = HashSet::new();
        if let Some(di) = dst_index.as_ref() {
            for v in di.versions.iter() {
                dst_versions.insert(v.id.clone());
            }
        }

        let src_manifests_root = source_root.join("manifests");
        let dst_manifests_root = dst_source_root.join("manifests");
        fs::create_dir_all(&dst_manifests_root).with_context(|| {
            format!(
                "versioned::replicate_one_pair failed to create replica manifests root {}",
                redact_path(&dst_manifests_root)
            )
        })?;

        for v in src_index.versions.iter() {
            if dst_versions.contains(v.id.as_str()) {
                continue;
            }
            let src_manifest_path = src_manifests_root.join(format!("{}.json", v.id));
            if !src_manifest_path.exists() {
                tracing::warn!(
                    source_id = %source_id,
                    version_id = %v.id,
                    "versioned replication skipping missing source manifest"
                );
                continue;
            }
            let dst_manifest_path = dst_manifests_root.join(format!("{}.json", v.id));
            let bytes = copy_file_atomic(cfg, &src_manifest_path, &dst_manifest_path)
                .with_context(|| {
                    format!(
                        "versioned::replicate_one_pair failed to copy manifest {} -> {}",
                        redact_path(&src_manifest_path),
                        redact_path(&dst_manifest_path)
                    )
                })?;
            pair.bytes_copied = pair.bytes_copied.saturating_add(bytes);
            pair.manifests_copied += 1;

            let manifest: Manifest = read_json(&src_manifest_path).with_context(|| {
                format!(
                    "versioned::replicate_one_pair failed to parse manifest {:?}",
                    src_manifest_path
                )
            })?;
            for e in manifest.entries.values() {
                if e.kind != ManifestEntryKind::File {
                    continue;
                }
                let Some(hash) = e.sha256.as_deref() else {
                    continue;
                };
                let src_blob = blob_path(&src_blobs, hash);
                if !src_blob.exists() {
                    tracing::warn!(
                        source_id = %source_id,
                        version_id = %v.id,
                        blob = %hash,
                        "versioned replication skipping missing source blob"
                    );
                    continue;
                }
                let dst_blob = blob_path(&dst_blobs, hash);
                if dst_blob.exists() {
                    continue;
                }
                let bytes = copy_file_atomic(cfg, &src_blob, &dst_blob).with_context(|| {
                    format!(
                        "versioned::replicate_one_pair failed to copy blob {} -> {}",
                        redact_path(&src_blob),
                        redact_path(&dst_blob)
                    )
                })?;
                pair.bytes_copied = pair.bytes_copied.saturating_add(bytes);
                pair.blobs_copied += 1;
            }
        }

        // Copy last_scan_report.json if present for UI diagnostics parity.
        let src_scan_report = source_root.join("last_scan_report.json");
        if src_scan_report.exists() {
            let dst_scan_report = dst_source_root.join("last_scan_report.json");
            if let Ok(bytes) = copy_file_atomic(cfg, &src_scan_report, &dst_scan_report) {
                pair.bytes_copied = pair.bytes_copied.saturating_add(bytes);
            }
        }

        // Mirror index last to keep the replica consistent for restores.
        let bytes = copy_file_atomic(cfg, &src_index_path, &dst_index_path).with_context(|| {
            format!(
                "versioned::replicate_one_pair failed to copy index {} -> {}",
                redact_path(&src_index_path),
                redact_path(&dst_index_path)
            )
        })?;
        pair.bytes_copied = pair.bytes_copied.saturating_add(bytes);

        if cfg.runtime.replication_mirror_manifests {
            pair.manifests_deleted =
                pair.manifests_deleted
                    .saturating_add(mirror_manifest_deletions(
                        cfg,
                        &src_index,
                        &dst_manifests_root,
                    )?);
        }
    }

    Ok(pair)
}

/// Summary: Delete replica manifest files that are not present in the source index.
///
/// Inputs: config, source index, and replica manifests directory.
///
/// Outputs: number of manifest files deleted.
///
/// Side effects: Removes manifest files from the replica store.
///
/// Error handling: Returns contextual errors on filesystem failures.
///
/// Ties to other methods: Used by `replicate_one_pair` when mirror mode is enabled.
///
/// Why this exists: Retention pruning on the primary should be reflected in replicas to keep versions aligned.
fn mirror_manifest_deletions(
    cfg: &Config,
    src_index: &VersionIndex,
    dst_manifests_root: &Path,
) -> Result<usize> {
    let keep: HashSet<&str> = src_index.versions.iter().map(|v| v.id.as_str()).collect();
    let mut deleted: usize = 0;
    let mut remaining = cfg.runtime.replication_max_manifest_deletes_per_cycle;
    if remaining == 0 {
        return Ok(0);
    }
    if !dst_manifests_root.exists() {
        return Ok(0);
    }
    for entry in WalkDir::new(dst_manifests_root)
        .min_depth(1)
        .max_depth(1)
        .follow_links(false)
    {
        let entry = entry.context("versioned::mirror_manifest_deletions walk manifests")?;
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(name) = entry
            .path()
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
        else {
            continue;
        };
        if !name.ends_with(".json") {
            continue;
        }
        let id = name.trim_end_matches(".json");
        if keep.contains(id) {
            continue;
        }
        if remaining == 0 {
            break;
        }
        fs::remove_file(entry.path()).with_context(|| {
            format!(
                "versioned::mirror_manifest_deletions failed removing {}",
                redact_path(entry.path())
            )
        })?;
        deleted += 1;
        remaining = remaining.saturating_sub(1);
    }
    Ok(deleted)
}

/// Summary: Atomically copy a file to the replica store with throttling and time bounds.
///
/// Inputs: config (for buffer sizing and throttling), source path, and destination path.
///
/// Outputs: bytes written to the destination.
///
/// Side effects: Writes a temp file, fsyncs it, renames it into place, and best-effort syncs the parent directory.
///
/// Error handling: Returns contextual errors for IO, timeouts, and rename failures.
///
/// Ties to other methods: Used by replication for manifests, indexes, scan reports, and blobs.
///
/// Why this exists: Replicas must not observe partial blobs that could break restore verification.
fn copy_file_atomic(cfg: &Config, src: &Path, dst: &Path) -> Result<u64> {
    let parent = dst
        .parent()
        .context("versioned::copy_file_atomic missing destination parent")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "versioned::copy_file_atomic failed to create parent {}",
            redact_path(parent)
        )
    })?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .context("versioned::copy_file_atomic failed to create temp file")?;

    let start = Instant::now();
    let timeout = Duration::from_secs(cfg.execution.copy_timeout_seconds.max(1));
    let bytes = copy_stream_with_throttle(
        src,
        temp.as_file_mut(),
        cfg.max_bytes_per_second,
        cfg.execution.copy_buffer_bytes,
        timeout,
        start,
    )?;
    temp.as_file_mut()
        .sync_all()
        .context("versioned::copy_file_atomic failed to fsync temp file")?;
    temp.persist(dst).map_err(|e| {
        anyhow::anyhow!(
            "versioned::copy_file_atomic failed to persist {}: {}",
            redact_path(dst),
            e
        )
    })?;
    sync_dir_best_effort(parent);
    Ok(bytes)
}

/// Summary: copy_stream_with_throttle orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
fn copy_stream_with_throttle(
    src: &Path,
    out: &mut dyn Write,
    max_bps: Option<u64>,
    buffer_bytes: usize,
    timeout: Duration,
    start: Instant,
) -> Result<u64> {
    if buffer_bytes == 0 {
        anyhow::bail!("versioned::copy_stream_with_throttle buffer_bytes must be > 0");
    }
    let mut reader = BufReader::with_capacity(
        buffer_bytes,
        fs::File::open(src).with_context(|| {
            format!(
                "versioned::copy_stream_with_throttle failed to open source {}",
                redact_path(src)
            )
        })?,
    );
    let mut writer = BufWriter::with_capacity(buffer_bytes, out);
    let mut buf = vec![0u8; buffer_bytes];
    let mut total: u64 = 0;
    let mut throttle = match max_bps {
        Some(limit) => Some(Throttle::new(limit)?),
        None => None,
    };
    loop {
        if start.elapsed() > timeout {
            anyhow::bail!(
                "versioned::copy_stream_with_throttle timed out after {:?} copying {}",
                timeout,
                redact_path(src)
            );
        }
        let n = reader.read(&mut buf).with_context(|| {
            format!(
                "versioned::copy_stream_with_throttle read {}",
                redact_path(src)
            )
        })?;
        if n == 0 {
            break;
        }
        writer
            .write_all(&buf[..n])
            .context("versioned::copy_stream_with_throttle write")?;
        total = total.saturating_add(n as u64);
        if let Some(throttle) = throttle.as_mut() {
            throttle.record(n as u64);
        }
    }
    writer
        .flush()
        .context("versioned::copy_stream_with_throttle flush")?;
    Ok(total)
}

/// Summary: sync_dir_best_effort orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
#[cfg(target_family = "unix")]
fn sync_dir_best_effort(path: &Path) {
    match fs::File::open(path) {
        Ok(dir) => {
            if let Err(error) = dir.sync_all() {
                tracing::warn!(
                    path = %redact_path(path),
                    error = %error,
                    "versioned::sync_dir_best_effort failed syncing directory"
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                path = %redact_path(path),
                error = %error,
                "versioned::sync_dir_best_effort failed opening directory"
            );
        }
    }
}

/// Summary: sync_dir_best_effort orchestrates this method's core behavior.
///
/// Inputs: Method parameters and required receiver state.
///
/// Outputs: Return value and observable result for callers.
///
/// Side effects: None beyond this method's explicit operations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: Invoked by and composes with adjacent module methods.
///
/// Why this exists: Keeps this behavior isolated, testable, and reusable.
#[cfg(not(target_family = "unix"))]
fn sync_dir_best_effort(_path: &Path) {}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("versioned::read_json failed to read {}", redact_path(path)))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("versioned::read_json failed to parse {}", redact_path(path)))
}
