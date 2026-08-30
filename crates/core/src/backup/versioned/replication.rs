use super::browse::sync_source_readable_view;
use super::model::{Manifest, ManifestEntryKind, VersionIndex};
use super::operation_lock::acquire_store_leases;
use super::store::{blob_path, blobs_root, sources_root, store_root};
use super::validation::validate_version_index;
use crate::config::model::{Config, Destination};
use crate::encryption::blobs::BlobCodec;
use crate::io::BlockingIoPolicy;
use crate::logging::redact_path;
use crate::scheduling::throttling::Throttle;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
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

#[derive(Debug)]
struct BlobReplicationPlan {
    src: PathBuf,
    dst: PathBuf,
    expected_sha256: String,
}

#[derive(Debug)]
struct VersionReplicationPlan {
    version_id: String,
    src_manifest: PathBuf,
    dst_manifest: PathBuf,
    blobs: Vec<BlobReplicationPlan>,
}

/// A second destination provides an immediate redundant copy without changing the manifest model.
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

    // Resolve the complete attempt set before touching any store. Missing target
    // IDs remain in the set so their existing per-pair summary behavior is kept,
    // while every source and resolvable target path is leased exactly once.
    let mut pairs = Vec::new();
    for src in cfg.destinations.iter() {
        if src.replicate_to.is_empty() {
            continue;
        }
        for target_id in src.replicate_to.iter() {
            pairs.push((
                src,
                target_id.as_str(),
                by_id.get(target_id.as_str()).copied(),
            ));
        }
    }

    let mut involved_destinations = Vec::with_capacity(pairs.len().saturating_mul(2));
    for (src, _target_id, dst) in &pairs {
        involved_destinations.push(src.path.as_path());
        if let Some(dst) = dst {
            involved_destinations.push(dst.path.as_path());
        }
    }
    let lock_policy = BlockingIoPolicy::from_config(cfg);
    let _operation_leases = acquire_store_leases(involved_destinations, &lock_policy)
        .context("versioned::replicate_configured_stores could not serialize destination stores")?;

    for (src, target_id, dst) in pairs {
        summary.pairs_attempted += 1;
        let Some(dst) = dst else {
            summary.pairs_failed += 1;
            summary.targets_failed.push(target_id.to_string());
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

/// Keep pair replication logic isolated for easier testing and observability.
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
    let blob_codec = BlobCodec::from_config(cfg)
        .context("versioned::replicate_one_pair failed to initialize blob codec")?;
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
        match regular_file_state(&src_index_path)? {
            RegularFileState::Missing => continue,
            RegularFileState::Regular => {}
        }
        let src_index: VersionIndex = read_json(&src_index_path)
            .with_context(|| format!("versioned::replicate_one_pair parse {:?}", src_index_path))?;
        validate_version_index(&src_index).with_context(|| {
            format!("versioned::replicate_one_pair validate source index {source_id}")
        })?;

        let dst_source_root = dst_sources.join(source_id.as_str());
        let dst_index_path = dst_source_root.join("index.json");
        fs::create_dir_all(&dst_source_root).with_context(|| {
            format!(
                "versioned::replicate_one_pair failed to create replica source root {}",
                redact_path(&dst_source_root)
            )
        })?;

        if regular_file_state(&dst_index_path)? == RegularFileState::Regular {
            let dst_index: VersionIndex = read_json(&dst_index_path).with_context(|| {
                format!(
                    "versioned::replicate_one_pair parse replica index {:?}",
                    dst_index_path
                )
            })?;
            validate_version_index(&dst_index).with_context(|| {
                format!("versioned::replicate_one_pair validate replica index {source_id}")
            })?;
        }

        let src_manifests_root = source_root.join("manifests");
        let dst_manifests_root = dst_source_root.join("manifests");
        fs::create_dir_all(&dst_manifests_root).with_context(|| {
            format!(
                "versioned::replicate_one_pair failed to create replica manifests root {}",
                redact_path(&dst_manifests_root)
            )
        })?;

        // Validate every artifact referenced by the source index before mutating the
        // replica. In particular, a missing late manifest/blob must not allow a new
        // replica index to be published.
        let plans = build_replication_plans(
            &src_index,
            &src_manifests_root,
            &dst_manifests_root,
            &src_blobs,
            &dst_blobs,
            &blob_codec,
            cfg.hashing.timeout_seconds,
        )
        .with_context(|| {
            format!("versioned::replicate_one_pair source preflight failed for {source_id}")
        })?;

        for plan in plans.iter() {
            let manifest_needs_copy = match regular_file_state(&plan.dst_manifest)? {
                RegularFileState::Missing => true,
                RegularFileState::Regular => {
                    !files_have_identical_contents(&plan.src_manifest, &plan.dst_manifest)?
                }
            };
            if manifest_needs_copy {
                let bytes = copy_file_atomic(cfg, &plan.src_manifest, &plan.dst_manifest)
                    .with_context(|| {
                        format!(
                            "versioned::replicate_one_pair failed to copy manifest {} -> {}",
                            redact_path(&plan.src_manifest),
                            redact_path(&plan.dst_manifest)
                        )
                    })?;
                pair.bytes_copied = pair.bytes_copied.saturating_add(bytes);
                pair.manifests_copied += 1;
            }

            // Repair missing blobs regardless of whether this version was already in
            // the replica index. The index is evidence of intent, not of completeness.
            for blob in plan.blobs.iter() {
                let blob_needs_copy = match regular_file_state(&blob.dst)? {
                    RegularFileState::Missing => true,
                    RegularFileState::Regular => !blob_matches_plaintext_hash(
                        &blob_codec,
                        &blob.dst,
                        &blob.expected_sha256,
                        cfg.hashing.timeout_seconds,
                    ),
                };
                if !blob_needs_copy {
                    continue;
                }
                let bytes = copy_file_atomic(cfg, &blob.src, &blob.dst).with_context(|| {
                    format!(
                        "versioned::replicate_one_pair failed to copy blob {} -> {}",
                        redact_path(&blob.src),
                        redact_path(&blob.dst)
                    )
                })?;
                pair.bytes_copied = pair.bytes_copied.saturating_add(bytes);
                pair.blobs_copied += 1;
                require_plaintext_hash(
                    &blob_codec,
                    &blob.dst,
                    &blob.expected_sha256,
                    cfg.hashing.timeout_seconds,
                    "replica blob after repair",
                )?;
            }
        }

        // Copy last_scan_report.json if present for UI diagnostics parity.
        let src_scan_report = source_root.join("last_scan_report.json");
        if regular_file_state(&src_scan_report)? == RegularFileState::Regular {
            let dst_scan_report = dst_source_root.join("last_scan_report.json");
            let _ = regular_file_state(&dst_scan_report)?;
            if let Ok(bytes) = copy_file_atomic(cfg, &src_scan_report, &dst_scan_report) {
                pair.bytes_copied = pair.bytes_copied.saturating_add(bytes);
            }
        }

        // Recheck both sides immediately before publication, parsing the current
        // manifests so the check covers exactly the hashes they now reference. The
        // surrounding store lease will make this boundary stable once lock
        // integration wraps this call.
        validate_replication_plans(
            &plans,
            &src_blobs,
            &dst_blobs,
            &blob_codec,
            cfg.hashing.timeout_seconds,
        )?;

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

    for watched in cfg
        .watched
        .iter()
        .filter(|watched| watched.enabled && watched.destination_id == src.id)
    {
        sync_source_readable_view(cfg, watched, &dst.path).with_context(|| {
            format!(
                "versioned::replicate_one_pair failed to synchronize readable view for replica {}",
                dst.id
            )
        })?;
    }

    Ok(pair)
}

fn build_replication_plans(
    src_index: &VersionIndex,
    src_manifests_root: &Path,
    dst_manifests_root: &Path,
    src_blobs_root: &Path,
    dst_blobs_root: &Path,
    blob_codec: &BlobCodec,
    hash_timeout_seconds: u64,
) -> Result<Vec<VersionReplicationPlan>> {
    let mut plans = Vec::with_capacity(src_index.versions.len());
    for version in src_index.versions.iter() {
        let src_manifest = src_manifests_root.join(format!("{}.json", version.id));
        require_regular_file(&src_manifest, "source manifest", &version.id, None)?;
        let manifest: Manifest = read_json(&src_manifest).with_context(|| {
            format!(
                "versioned::build_replication_plans failed to parse manifest {}",
                redact_path(&src_manifest)
            )
        })?;

        let mut seen_hashes = HashSet::new();
        let mut blobs = Vec::new();
        for entry in manifest.entries.values() {
            if entry.kind != ManifestEntryKind::File {
                continue;
            }
            let Some(hash) = entry.sha256.as_deref() else {
                continue;
            };
            validate_blob_hash(hash).with_context(|| {
                format!(
                    "versioned::build_replication_plans invalid blob hash in version {}",
                    version.id
                )
            })?;
            if !seen_hashes.insert(hash.to_string()) {
                continue;
            }
            let src_blob = blob_path(src_blobs_root, hash);
            require_regular_file(&src_blob, "source blob", &version.id, Some(hash))?;
            require_plaintext_hash(
                blob_codec,
                &src_blob,
                hash,
                hash_timeout_seconds,
                "source blob",
            )?;
            blobs.push(BlobReplicationPlan {
                src: src_blob,
                dst: blob_path(dst_blobs_root, hash),
                expected_sha256: hash.to_string(),
            });
        }

        plans.push(VersionReplicationPlan {
            version_id: version.id.clone(),
            src_manifest,
            dst_manifest: dst_manifests_root.join(format!("{}.json", version.id)),
            blobs,
        });
    }
    Ok(plans)
}

fn validate_replication_plans(
    plans: &[VersionReplicationPlan],
    src_blobs_root: &Path,
    dst_blobs_root: &Path,
    blob_codec: &BlobCodec,
    hash_timeout_seconds: u64,
) -> Result<()> {
    for plan in plans {
        if !files_have_identical_contents(&plan.src_manifest, &plan.dst_manifest)? {
            anyhow::bail!(
                "versioned replication replica manifest differs from authoritative source for version {}",
                plan.version_id
            );
        }
        validate_manifest_artifacts(
            &plan.src_manifest,
            "source manifest",
            &plan.version_id,
            src_blobs_root,
            blob_codec,
            hash_timeout_seconds,
        )?;
        validate_manifest_artifacts(
            &plan.dst_manifest,
            "replica manifest",
            &plan.version_id,
            dst_blobs_root,
            blob_codec,
            hash_timeout_seconds,
        )?;
    }
    Ok(())
}

fn validate_manifest_artifacts(
    manifest_path: &Path,
    artifact: &str,
    version_id: &str,
    blobs_root: &Path,
    blob_codec: &BlobCodec,
    hash_timeout_seconds: u64,
) -> Result<()> {
    require_regular_file(manifest_path, artifact, version_id, None)?;
    let manifest: Manifest = read_json(manifest_path).with_context(|| {
        format!(
            "versioned::validate_manifest_artifacts failed to parse {artifact} {}",
            redact_path(manifest_path)
        )
    })?;
    for entry in manifest.entries.values() {
        if entry.kind != ManifestEntryKind::File {
            continue;
        }
        let Some(hash) = entry.sha256.as_deref() else {
            continue;
        };
        validate_blob_hash(hash).with_context(|| {
            format!(
                "versioned::validate_manifest_artifacts invalid hash in {artifact} for version {version_id}"
            )
        })?;
        let blob = blob_path(blobs_root, hash);
        require_regular_file(&blob, "referenced blob", version_id, Some(hash))?;
        require_plaintext_hash(
            blob_codec,
            &blob,
            hash,
            hash_timeout_seconds,
            "referenced blob",
        )?;
    }
    Ok(())
}

fn validate_blob_hash(hash: &str) -> Result<()> {
    if hash.len() != 64
        || !hash
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        anyhow::bail!("versioned replication expected a 64-character lowercase hex sha256");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegularFileState {
    Missing,
    Regular,
}

fn regular_file_state(path: &Path) -> Result<RegularFileState> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => anyhow::bail!(
            "versioned replication refuses symbolic-link artifact at {}",
            redact_path(path)
        ),
        Ok(metadata) if metadata.is_file() => Ok(RegularFileState::Regular),
        Ok(_) => anyhow::bail!(
            "versioned replication expected a regular-file artifact at {}",
            redact_path(path)
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(RegularFileState::Missing),
        Err(error) => Err(error).with_context(|| {
            format!(
                "versioned::regular_file_state failed to inspect {}",
                redact_path(path)
            )
        }),
    }
}

fn require_regular_file(
    path: &Path,
    artifact: &str,
    version_id: &str,
    hash: Option<&str>,
) -> Result<()> {
    if regular_file_state(path)? != RegularFileState::Regular {
        anyhow::bail!(
            "versioned replication missing {artifact} for version {version_id}{} at {}",
            hash.map(|value| format!(" (sha256 {value})"))
                .unwrap_or_default(),
            redact_path(path)
        );
    }
    Ok(())
}

fn files_have_identical_contents(left: &Path, right: &Path) -> Result<bool> {
    require_regular_file(left, "comparison source", "<comparison>", None)?;
    require_regular_file(right, "comparison target", "<comparison>", None)?;
    let left_file = fs::File::open(left).with_context(|| {
        format!(
            "versioned::files_have_identical_contents failed opening {}",
            redact_path(left)
        )
    })?;
    let right_file = fs::File::open(right).with_context(|| {
        format!(
            "versioned::files_have_identical_contents failed opening {}",
            redact_path(right)
        )
    })?;
    if left_file.metadata()?.len() != right_file.metadata()?.len() {
        return Ok(false);
    }
    let mut left_reader = BufReader::new(left_file);
    let mut right_reader = BufReader::new(right_file);
    let mut left_buffer = [0_u8; 64 * 1024];
    let mut right_buffer = [0_u8; 64 * 1024];
    loop {
        let left_read = left_reader
            .read(&mut left_buffer)
            .context("versioned::files_have_identical_contents failed reading source")?;
        let right_read = right_reader
            .read(&mut right_buffer)
            .context("versioned::files_have_identical_contents failed reading target")?;
        if left_read != right_read || left_buffer[..left_read] != right_buffer[..right_read] {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
    }
}

fn blob_matches_plaintext_hash(
    blob_codec: &BlobCodec,
    blob_path: &Path,
    expected_sha256: &str,
    timeout_seconds: u64,
) -> bool {
    blob_codec
        .sha256_plaintext_blob(blob_path, timeout_seconds)
        .is_ok_and(|actual| actual == expected_sha256)
}

fn require_plaintext_hash(
    blob_codec: &BlobCodec,
    blob_path: &Path,
    expected_sha256: &str,
    timeout_seconds: u64,
    artifact: &str,
) -> Result<()> {
    let actual = blob_codec
        .sha256_plaintext_blob(blob_path, timeout_seconds)
        .with_context(|| {
            format!(
                "versioned replication failed decoding {artifact} {}",
                redact_path(blob_path)
            )
        })?;
    if actual != expected_sha256 {
        anyhow::bail!(
            "versioned replication plaintext hash mismatch for {artifact} {} (expected {}, got {})",
            redact_path(blob_path),
            expected_sha256,
            actual
        );
    }
    Ok(())
}

/// Retention pruning on the primary should be reflected in replicas to keep versions aligned.
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

/// Replicas must not observe partial blobs that could break restore verification.
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

#[cfg(not(target_family = "unix"))]
fn sync_dir_best_effort(_path: &Path) {}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("versioned::read_json failed to read {}", redact_path(path)))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("versioned::read_json failed to parse {}", redact_path(path)))
}
