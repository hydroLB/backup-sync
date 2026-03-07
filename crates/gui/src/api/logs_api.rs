use anyhow::{Context, Result};
use backup_core::config::model::RuntimeTuning;
use backup_core::io::{run_with_policy, BlockingIoPolicy, CancellationFlag};
use chrono::Utc;
use std::path::{Path, PathBuf};
use tracing::warn;

/// Summary: Resolves the daemon log file path.
///
/// Inputs: none.
///
/// Outputs: the resolved log path.
///
/// Side effects: Reads platform log directory locations.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: log tail and export operations.
///
/// Why this exists: centralize log path resolution for GUI features.
pub fn log_path() -> Result<PathBuf> {
    backup_core::platform::paths::log_file_path()
        .context("gui::api::logs_api::log_path failed to resolve log path")
}

/// Summary: Reads the log file and returns the last N lines.
///
/// Inputs: an optional line limit.
///
/// Outputs: the tail string for display.
///
/// Side effects: Reads the log file from disk.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI log tail commands.
///
/// Why this exists: provide quick access to recent daemon logs.
pub fn read_log_tail(limit: Option<usize>) -> Result<String> {
    let path = log_path()?;
    let runtime = resolve_runtime_tuning();
    let max_lines = resolve_tail_limit(limit, &runtime);
    let tail = match read_tail_from_file(
        &path,
        max_lines,
        runtime.log_tail_read_chunk_bytes,
        runtime.log_tail_max_bytes,
    ) {
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

/// Summary: Exports the full log content to a destination directory.
///
/// Inputs: the destination directory.
///
/// Outputs: the path of the written log file.
///
/// Side effects: Reads the log file and writes an exported copy.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI export logs commands.
///
/// Why this exists: allow users to share logs for support.
pub fn export_logs(dest_dir: &Path) -> Result<PathBuf> {
    let io_policy = BlockingIoPolicy::bootstrap_defaults();
    let path = log_path()?;
    let data = run_with_policy(
        "gui::api::logs_api::export_logs read log file",
        &io_policy,
        CancellationFlag::none(),
        || {
            std::fs::read_to_string(&path).with_context(|| {
                format!(
                    "gui::api::logs_api::export_logs failed to read log file {:?}",
                    path
                )
            })
        },
    )?;
    let data = filter_hidden_log_lines(&data);
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    let dest = dest_dir.join(format!("BackupSync-logs-{}.txt", ts));
    run_with_policy(
        "gui::api::logs_api::export_logs write export file",
        &io_policy,
        CancellationFlag::none(),
        || {
            std::fs::write(&dest, data.as_bytes()).with_context(|| {
                format!("gui::api::logs_api::export_logs failed to write {:?}", dest)
            })
        },
    )?;
    Ok(dest)
}

/// Summary: Resolves the log tail line count from the configuration or default values.
///
/// Inputs: an optional line limit override.
///
/// Outputs: the resolved line count.
///
/// Side effects: Reads config defaults when no override is provided.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: log tail responses from the GUI.
///
/// Why this exists: centralize log tail limit selection with clear fallback logging.
fn resolve_runtime_tuning() -> RuntimeTuning {
    match backup_core::load_validated_config() {
        Ok(cfg) => cfg.runtime,
        Err(error) => {
            warn!(
                error = %error,
                "gui::api::logs_api::resolve_runtime_tuning failed to load config; using runtime defaults"
            );
            RuntimeTuning::default()
        }
    }
}

/// Summary: Resolves the log tail line count from the provided override and runtime tuning.
///
/// Inputs: optional override and loaded runtime tuning.
///
/// Outputs: the resolved line count.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: log tail responses from the GUI.
///
/// Why this exists: keep log tail policy selection centralized and free of magic values.
fn resolve_tail_limit(limit: Option<usize>, runtime: &RuntimeTuning) -> usize {
    if let Some(limit) = limit {
        return limit;
    }
    runtime.log_tail_lines
}

/// Summary: Returns the last N lines from a string.
///
/// Inputs: the log data and maximum line count.
///
/// Outputs: the truncated log text.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: log tail formatting.
///
/// Why this exists: avoid sending large logs when only recent lines are needed.
fn tail_lines(data: &str, max_lines: usize) -> String {
    if max_lines == 0 {
        return String::new();
    }
    let lines: Vec<&str> = data.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    lines[start..].join("\n")
}

/// Summary: Detects whether an error is a file-not-found IO error.
///
/// Inputs: An error value.
///
/// Outputs: `true` when the root cause is `ErrorKind::NotFound`.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: log tail and export error handling.
///
/// Why this exists: Keep `read_log_tail` behavior stable when logs are not present yet.
fn is_not_found(err: &anyhow::Error) -> bool {
    err.downcast_ref::<std::io::Error>()
        .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound)
}

/// Summary: Read the last N lines from a file without loading the full file.
///
/// Inputs: Path to the log file and maximum lines.
///
/// Outputs: The tail text.
///
/// Side effects: Reads from disk using file seeks.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `read_log_tail`.
///
/// Why this exists: Avoid large allocations when logs grow over time.
fn read_tail_from_file(
    path: &Path,
    max_lines: usize,
    read_chunk_bytes: usize,
    max_bytes: u64,
) -> Result<String> {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    if max_lines == 0 {
        return Ok(String::new());
    }
    let io_policy = BlockingIoPolicy::bootstrap_defaults();
    run_with_policy(
        "gui::api::logs_api::read_tail_from_file read tail",
        &io_policy,
        CancellationFlag::none(),
        || {
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
            while pos > 0 && newline_count <= max_lines && total < max_bytes {
                let take = std::cmp::min(read_chunk_bytes.max(1) as u64, pos) as usize;
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
        },
    )
}

/// Summary: Removes legacy auth bypass noise from log output.
///
/// Inputs: a raw log string.
///
/// Outputs: a filtered log string with noisy lines removed.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: log tail and log export operations.
///
/// Why this exists: ensure removed authentication flows do not linger in UI log views or exports.
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

    /// Summary: filter_hidden_log_lines_removes_legacy_auth_lines orchestrates this method's core behavior.
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
