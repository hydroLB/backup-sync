pub mod formatter;
pub mod sinks;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tracing::warn;
use tracing_subscriber::EnvFilter;

static FILE_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

/// Purpose: Initializes stdout logging with a default info filter.
///
/// Inputs: none.
/// Outputs: `()` after attempting to register a subscriber.
/// Ties to: CLI and daemon startup logging.
/// Side effects: Registers a global tracing subscriber for stdout logging.
/// Why: provide a lightweight default logger without file output.
pub fn init() {
    if let Err(e) = tracing_subscriber::fmt()
        .with_timer(formatter::default_timer())
        .with_env_filter("info")
        .with_span_events(formatter::default_span_events())
        .with_target(false)
        .try_init()
    {
        eprintln!("logging::init failed to initialize subscriber: {}", e);
    }
}

/// Purpose: Initializes logging with optional file output and daily rotation.
///
/// Inputs: an optional log file path.
/// Outputs: `Ok(())` after the logger is configured.
/// Ties to: daemon and GUI background logging.
/// Side effects: Registers a global tracing subscriber and opens log sinks.
/// Why: keep logs persistent while bounding disk usage.
pub fn init_with_optional_file(log_path: Option<PathBuf>) -> Result<()> {
    let env_filter = match EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(e) => {
            eprintln!(
                "logging::init_with_optional_file failed to parse RUST_LOG, using info: {}",
                e
            );
            EnvFilter::new("info")
        }
    };
    if let Some(path) = log_path {
        let (writer, guard) = sinks::rolling_file_sink(&path)
            .context("logging::init_with_optional_file failed to build file sink")?;
        if FILE_GUARD.set(guard).is_err() {
            warn!("logging::init_with_optional_file failed to set file guard");
        }
        if let Err(e) = tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_writer(writer)
            .with_timer(formatter::default_timer())
            .with_span_events(formatter::default_span_events())
            .with_target(false)
            .try_init()
        {
            warn!(
                "logging::init_with_optional_file failed to initialize file subscriber: {}",
                e
            );
        }
    } else {
        if let Err(e) = tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_timer(formatter::default_timer())
            .with_span_events(formatter::default_span_events())
            .with_target(false)
            .try_init()
        {
            warn!(
                "logging::init_with_optional_file failed to initialize stdout subscriber: {}",
                e
            );
        }
    }
    Ok(())
}

/// Purpose: Redacts sensitive path prefixes for log safety.
///
/// Inputs: a filesystem path reference.
/// Outputs: a redacted path string.
/// Ties to: core logging call sites that emit filesystem paths.
/// Side effects: None.
/// Why: avoid leaking full user directory paths in logs.
pub fn redact_path(path: &Path) -> String {
    let raw = path.display().to_string();
    if let Some(home) = dirs::home_dir() {
        let home_str = home.display().to_string();
        if raw.starts_with(&home_str) {
            return raw.replacen(&home_str, "~", 1);
        }
    }
    raw
}
