use anyhow::{Context, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};
use tracing::warn;

/// Purpose: Resolves the daemon log file path.
///
/// Inputs: none.
/// Outputs: the resolved log path.
/// Ties to: log tail and export operations.
/// Side effects: Reads platform log directory locations.
/// Why: centralize log path resolution for GUI features.
pub fn log_path() -> Result<PathBuf> {
    backup_core::platform::paths::log_file_path()
        .context("gui::api::logs_api::log_path failed to resolve log path")
}

/// Purpose: Reads the log file and returns the last N lines.
///
/// Inputs: an optional line limit.
/// Outputs: the tail string for display.
/// Ties to: GUI log tail commands.
/// Side effects: Reads the log file from disk.
/// Why: provide quick access to recent daemon logs.
pub fn read_log_tail(limit: Option<usize>) -> Result<String> {
    let path = log_path()?;
    let max_lines = resolve_tail_limit(limit)?;
    let tail = match read_tail_from_file(&path, max_lines) {
        Ok(tail) => tail,
        Err(e) if is_not_found(&e) => {
            warn!(
                "gui::api::logs_api::read_log_tail log file not found at {:?}",
                path
            );
            String::new()
        }
        Err(e) => {
            return Err(anyhow::anyhow!(
                "gui::api::logs_api::read_log_tail failed to read log file {:?}: {}",
                path,
                e
            ));
        }
    };
    Ok(filter_hidden_log_lines(&tail))
}

/// Purpose: Exports the full log content to a destination directory.
///
/// Inputs: the destination directory.
/// Outputs: the path of the written log file.
/// Ties to: GUI export logs commands.
/// Side effects: Reads the log file and writes an exported copy.
/// Why: allow users to share logs for support.
pub fn export_logs(dest_dir: &Path) -> Result<PathBuf> {
    let path = log_path()?;
    let data = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "gui::api::logs_api::export_logs failed to read log file {:?}",
            path
        )
    })?;
    let data = filter_hidden_log_lines(&data);
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    let dest = dest_dir.join(format!("BackupSync-logs-{}.txt", ts));
    std::fs::write(&dest, data)
        .with_context(|| format!("gui::api::logs_api::export_logs failed to write {:?}", dest))?;
    Ok(dest)
}

/// Purpose: Resolves the log tail line count from the configuration or default values.
///
/// Inputs: an optional line limit override.
/// Outputs: the resolved line count.
/// Ties to: log tail responses from the GUI.
/// Side effects: Reads config defaults when no override is provided.
/// Why: centralize log tail limit selection with clear fallback logging.
fn resolve_tail_limit(limit: Option<usize>) -> Result<usize> {
    if let Some(limit) = limit {
        return Ok(limit);
    }
    match backup_core::load_config() {
        Ok(cfg) => Ok(cfg.runtime.log_tail_lines),
        Err(e) => {
            warn!(
                "gui::api::logs_api::resolve_tail_limit failed to load config, using defaults: {}",
                e
            );
            let defaults = backup_core::config::config_defaults()
                .context("gui::api::logs_api::resolve_tail_limit failed to load defaults")?;
            Ok(defaults.runtime.log_tail_lines)
        }
    }
}

/// Purpose: Returns the last N lines from a string.
///
/// Inputs: the log data and maximum line count.
/// Outputs: the truncated log text.
/// Ties to: log tail formatting.
/// Side effects: None.
/// Why: avoid sending large logs when only recent lines are needed.
fn tail_lines(data: &str, max_lines: usize) -> String {
    if max_lines == 0 {
        return String::new();
    }
    let lines: Vec<&str> = data.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

/// Purpose: Detects whether an error is a file-not-found IO error.
///
/// Inputs: An error value.
/// Outputs: `true` when the root cause is `ErrorKind::NotFound`.
/// Ties to: log tail and export error handling.
/// Side effects: None.
/// Why: Keep `read_log_tail` behavior stable when logs are not present yet.
fn is_not_found(err: &anyhow::Error) -> bool {
    err.downcast_ref::<std::io::Error>()
        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
}

/// Purpose: Read the last N lines from a file without loading the full file.
///
/// Inputs: Path to the log file and maximum lines.
/// Outputs: The tail text.
/// Ties to: `read_log_tail`.
/// Side effects: Reads from disk using file seeks.
/// Why: Avoid large allocations when logs grow over time.
fn read_tail_from_file(path: &Path, max_lines: usize) -> Result<String> {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    const CHUNK: usize = 8192;
    const MAX_BYTES: u64 = 512 * 1024; // cap tail reads to keep UI snappy

    if max_lines == 0 {
        return Ok(String::new());
    }

    let mut f = File::open(path).with_context(|| {
        format!(
            "gui::api::logs_api::read_tail_from_file failed to open {:?}",
            path
        )
    })?;
    let mut pos = f
        .metadata()
        .with_context(|| {
            format!(
                "gui::api::logs_api::read_tail_from_file failed to stat {:?}",
                path
            )
        })?
        .len();

    let mut chunks: Vec<Vec<u8>> = Vec::new();
    let mut newline_count: usize = 0;
    let mut total: u64 = 0;
    while pos > 0 && newline_count <= max_lines && total < MAX_BYTES {
        let take = std::cmp::min(CHUNK as u64, pos) as usize;
        pos -= take as u64;
        f.seek(SeekFrom::Start(pos)).with_context(|| {
            format!(
                "gui::api::logs_api::read_tail_from_file failed to seek {:?}",
                path
            )
        })?;
        let mut buf = vec![0u8; take];
        f.read_exact(&mut buf).with_context(|| {
            format!(
                "gui::api::logs_api::read_tail_from_file failed to read {:?}",
                path
            )
        })?;
        newline_count += buf.iter().filter(|b| **b == b'\n').count();
        total += buf.len() as u64;
        chunks.push(buf);
    }

    chunks.reverse();
    let mut bytes = Vec::with_capacity(total as usize);
    for c in chunks {
        bytes.extend_from_slice(&c);
    }

    let s = String::from_utf8_lossy(&bytes).to_string();
    Ok(tail_lines(&s, max_lines))
}

/// Purpose: Removes legacy auth bypass noise from log output.
///
/// Inputs: a raw log string.
/// Outputs: a filtered log string with noisy lines removed.
/// Ties to: log tail and log export operations.
/// Side effects: None.
/// Why: ensure removed authentication flows do not linger in UI log views or exports.
fn filter_hidden_log_lines(data: &str) -> String {
    const NEEDLES: [&str; 2] = ["AUTH_BYPASS", "allowing without unlock"];
    if !NEEDLES.iter().any(|needle| data.contains(needle)) {
        return data.to_string();
    }
    let mut out = String::with_capacity(data.len());
    for line in data.lines() {
        if NEEDLES.iter().any(|needle| line.contains(needle)) {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::filter_hidden_log_lines;

    #[test]
    fn filter_hidden_log_lines_removes_legacy_auth_lines() {
        let input = "\
2026-02-04T00:00:00Z INFO normal\n\
S allowing without unlock\n\
[cid=save-miqzosxg-e476] AUTH_BYPASS allowing without unlock\n\
2026-02-04T00:00:01Z INFO still here\n";
        let filtered = filter_hidden_log_lines(input);
        assert!(filtered.contains("INFO normal"));
        assert!(filtered.contains("INFO still here"));
        assert!(!filtered.contains("AUTH_BYPASS"));
        assert!(!filtered.contains("allowing without unlock"));
    }
}
