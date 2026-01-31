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
    let data = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
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
    let max_lines = resolve_tail_limit(limit)?;
    Ok(tail_lines(&data, max_lines))
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
