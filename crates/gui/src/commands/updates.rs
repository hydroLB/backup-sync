use crate::commands::error::ErrorEnvelope;
use crate::commands::io_policy::run_blocking_io;
use anyhow::Context;
use chrono::Utc;
use serde::Deserialize;
use std::cmp::Ordering;
use std::path::PathBuf;
use tracing::warn;

#[derive(Deserialize)]
struct UpdateFeed {
    latest_version: String,
    notes: Option<String>,
    published_at: Option<String>,
}

/// Allow update checks without network access.
fn update_feed_path() -> Result<PathBuf, ErrorEnvelope> {
    if let Some(path) = std::env::var_os("BACKUP_SYNC_UPDATE_FEED") {
        return Ok(PathBuf::from(path));
    }
    let mut base = dirs::config_dir().ok_or_else(|| {
        ErrorEnvelope::new(
            "UPDATE_PATH",
            "updates::update_feed_path no config directory available",
        )
    })?;
    base.push("backup_sync/update.json");
    Ok(base)
}

/// Allow deterministic version comparisons without external crates.
fn parse_version(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map(|part| match part.parse::<u64>() {
            Ok(val) => val,
            Err(e) => {
                warn!(
                    "updates::parse_version invalid version component {}: {}",
                    part, e
                );
                0
            }
        })
        .collect()
}

/// Determine whether an update is available.
fn compare_versions(latest: &str, current: &str) -> Ordering {
    let a = parse_version(latest);
    let b = parse_version(current);
    let len = std::cmp::max(a.len(), b.len());
    for i in 0..len {
        let av = *a.get(i).unwrap_or(&0);
        let bv = *b.get(i).unwrap_or(&0);
        match av.cmp(&bv) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    Ordering::Equal
}

/// Report the current version in update responses.
fn current_version() -> String {
    option_env!("CARGO_PKG_VERSION")
        .unwrap_or("0.0.0")
        .to_string()
}

#[tauri::command]
/// Provide update information without network access.
pub async fn check_updates_cmd() -> Result<String, ErrorEnvelope> {
    let version = current_version();
    let ts = Utc::now().format("%Y-%m-%d %H:%M:%S");
    let feed_path = update_feed_path()?;
    if !feed_path.exists() {
        return Ok(format!(
            "Backup Sync {}. No update feed configured (checked {} at {}).",
            version,
            feed_path.display(),
            ts
        ));
    }
    let raw = run_blocking_io("gui::updates::check_updates_cmd read update feed", || {
        std::fs::read_to_string(&feed_path).with_context(|| {
            format!(
                "updates::check_updates_cmd failed to read update feed {:?}",
                feed_path
            )
        })
    })
    .map_err(|e| {
        ErrorEnvelope::new(
            "UPDATE_READ",
            format!(
                "updates::check_updates_cmd failed to read {:?}: {}",
                feed_path, e
            ),
        )
    })?;
    let feed: UpdateFeed = serde_json::from_str(&raw).map_err(|e| {
        ErrorEnvelope::new(
            "UPDATE_PARSE",
            format!(
                "updates::check_updates_cmd failed to parse update feed: {}",
                e
            ),
        )
    })?;
    let ordering = compare_versions(&feed.latest_version, &version);
    let published = feed
        .published_at
        .as_deref()
        .unwrap_or("unknown publish time");
    let notes = feed
        .notes
        .unwrap_or_else(|| "No release notes provided.".to_string());
    match ordering {
        Ordering::Greater => Ok(format!(
            "Update available: {} -> {} (published {}). Notes: {}",
            version, feed.latest_version, published, notes
        )),
        Ordering::Equal => Ok(format!(
            "Backup Sync {} is up to date (checked {} at {}).",
            version,
            feed_path.display(),
            ts
        )),
        Ordering::Less => Ok(format!(
            "Backup Sync {} is newer than feed version {} (checked {} at {}).",
            version,
            feed.latest_version,
            feed_path.display(),
            ts
        )),
    }
}
