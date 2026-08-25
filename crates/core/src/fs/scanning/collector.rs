use super::metadata::FileMeta;
use crate::config::model::{Config, WatchedKind};
use crate::logging::redact_path;
use anyhow::{anyhow, Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::fs;
use std::time::{Duration, Instant, UNIX_EPOCH};
use tracing::warn;
use walkdir::{DirEntry, WalkDir};

/// Keep timestamps comparable across runs.
fn mtime_i64(meta: &fs::Metadata) -> Result<i64> {
    let m = meta
        .modified()
        .context("fs::scanning::mtime_i64 failed to read modified time")?;
    let nanos = m
        .duration_since(UNIX_EPOCH)
        .context("fs::scanning::mtime_i64 failed to compute duration since epoch")?
        .as_nanos();
    let ts: i64 = nanos
        .try_into()
        .map_err(|_| anyhow!("fs::scanning::mtime_i64 mtime too large to store"))?;
    Ok(ts)
}

/// Optionally skip hidden files when configured.
fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// Keep ignore matching efficient during scans.
fn build_ignore_set(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for p in patterns {
        if let Ok(g) = Glob::new(p) {
            builder.add(g);
        } else {
            warn!(
                "fs::scanning::build_ignore_set invalid ignore pattern skipped: {}",
                p
            );
        }
    }
    builder
        .build()
        .context("fs::scanning::build_ignore_set failed to build ignore set")
}

/// Keep scan operations bounded on slow or problematic filesystems.
fn check_scan_timeout(start: Instant, timeout: Duration, context: &str) -> Result<()> {
    if start.elapsed() > timeout {
        anyhow::bail!(
            "fs::scanning::collect_targets timed out after {}s while {}",
            timeout.as_secs(),
            context
        );
    }
    Ok(())
}

/// Provide a consistent snapshot of watched files for planning.
pub fn collect_targets(cfg: &Config) -> Result<Vec<FileMeta>> {
    if cfg.watched.is_empty() {
        anyhow::bail!("fs::scanning::collect_targets no watched paths configured; add a folder/file to continue");
    }
    let timeout = Duration::from_secs(cfg.planning.scan_timeout_seconds);
    let start = Instant::now();
    let dest_map: std::collections::HashMap<_, _> = cfg
        .destinations
        .iter()
        .map(|d| (d.id.as_str(), d))
        .collect();
    // Preallocate to cut down on reallocations during large scans.
    let mut out = Vec::with_capacity(
        cfg.watched
            .len()
            .saturating_mul(cfg.planning.scan_capacity_multiplier),
    );
    let mut skipped = 0usize;
    let ignore = build_ignore_set(&cfg.ignore_patterns)?;
    let mut scanned = 0usize;
    for w in cfg.watched.iter().filter(|w| w.enabled) {
        check_scan_timeout(start, timeout, "scanning watched paths")?;
        let dest = dest_map.get(w.destination_id.as_str()).ok_or_else(|| {
            anyhow!(
                "fs::scanning::collect_targets missing destination {}",
                w.destination_id
            )
        })?;
        let effective_max = w
            .max_backups_per_file
            .or(dest.max_backups_per_file)
            .unwrap_or(cfg.max_backups_per_file);
        if w.path.starts_with(&cfg.backup_root) {
            warn!(
                "fs::scanning::collect_targets watched path inside backup_root, skipping: {}",
                redact_path(&w.path)
            );
            skipped += 1;
            continue;
        }
        match w.kind {
            WatchedKind::File => {
                if cfg.skip_hidden
                    && w.path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .map(|s| s.starts_with('.'))
                        .unwrap_or(false)
                {
                    continue;
                }
                if ignore.is_match(&w.path) {
                    continue;
                }
                match fs::metadata(&w.path) {
                    Ok(meta) => {
                        match mtime_i64(&meta) {
                            Ok(mtime) => out.push(FileMeta {
                                key: w.path.to_string_lossy().into_owned(),
                                path: w.path.clone(),
                                len: meta.len(),
                                mtime,
                                destination_root: dest.path.clone(),
                                max_copies: effective_max,
                            }),
                            Err(e) => {
                                skipped += 1;
                                warn!(
                                    "fs::scanning::collect_targets failed to read mtime for {}: {}",
                                    redact_path(&w.path),
                                    e
                                );
                            }
                        };
                    }
                    Err(e) => {
                        skipped += 1;
                        warn!(
                            "fs::scanning::collect_targets failed to stat watched file {}: {}",
                            redact_path(&w.path),
                            e
                        );
                    }
                };
            }
            WatchedKind::Directory => {
                for entry in WalkDir::new(&w.path).into_iter().filter_entry(|e| {
                    if cfg.skip_hidden && is_hidden(e) {
                        return false;
                    }
                    !ignore.is_match(e.path())
                }) {
                    scanned = scanned.saturating_add(1);
                    if scanned.is_multiple_of(1024) {
                        check_scan_timeout(start, timeout, "walking directory entries")?;
                    }
                    match entry {
                        Ok(entry) => {
                            if entry.file_type().is_symlink() {
                                skipped += 1;
                                warn!(
                                    "fs::scanning::collect_targets skipping symlink inside {}: {}",
                                    redact_path(&w.path),
                                    redact_path(entry.path())
                                );
                                continue;
                            }
                            if entry.file_type().is_file() {
                                let path = entry.path().to_path_buf();
                                if ignore.is_match(&path) {
                                    continue;
                                }
                                match entry.metadata() {
                                    Ok(meta) => match mtime_i64(&meta) {
                                        Ok(mtime) => {
                                            let key = path.to_string_lossy().into_owned();
                                            out.push(FileMeta {
                                                key,
                                                path,
                                                len: meta.len(),
                                                mtime,
                                                destination_root: dest.path.clone(),
                                                max_copies: effective_max,
                                            })
                                        }
                                        Err(e) => {
                                            skipped += 1;
                                            warn!(
                                                "fs::scanning::collect_targets failed to read mtime for {}: {}",
                                                redact_path(&path),
                                                e
                                            );
                                        }
                                    },
                                    Err(e) => {
                                        skipped += 1;
                                        warn!(
                                            "fs::scanning::collect_targets failed to read metadata for {} in {}: {}",
                                            redact_path(&path),
                                            redact_path(&w.path),
                                            e
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            skipped += 1;
                            warn!(
                                "fs::scanning::collect_targets failed to enumerate entry under {}: {}",
                                redact_path(&w.path),
                                e
                            );
                        }
                    }
                }
            }
        }
    }
    if skipped > 0 {
        warn!(
            "fs::scanning::collect_targets skipped {} items during scan due to IO errors or filters; continuing with remaining files",
            skipped
        );
    }
    Ok(out)
}
