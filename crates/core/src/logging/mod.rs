pub mod formatter;
pub mod sinks;

use anyhow::{Context, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tracing::warn;
use tracing_subscriber::EnvFilter;

static FILE_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

/// Keep bootstrap logging events traceable before normal runtime cids exist.
fn bootstrap_cid(prefix: &str) -> String {
    format!("{prefix}-{}", Utc::now().timestamp_millis())
}

/// Provide a shared correlation id helper across crates without cross-layer deps.
pub fn cid(prefix: &str) -> String {
    bootstrap_cid(prefix)
}

/// Prevent log injection and keep correlation ids stable for indexing.
pub fn sanitize_correlation_id(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "unknown".to_string();
    }
    let mut out = String::with_capacity(trimmed.len().min(96));
    for c in trimmed.chars().take(96) {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/' | '#') {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "unknown".to_string()
    } else {
        out
    }
}

/// Keep correlation id generation and sanitation consistent across boundaries.
pub fn correlation_id(prefix: &str, incoming: Option<&str>) -> String {
    match incoming {
        Some(raw) => sanitize_correlation_id(raw),
        None => sanitize_correlation_id(&cid(prefix)),
    }
}

/// Provide a lightweight default logger without file output.
pub fn init() {
    let cid = bootstrap_cid("core-log-init");
    if let Err(e) = tracing_subscriber::fmt()
        .json()
        .flatten_event(true)
        .with_current_span(false)
        .with_span_list(false)
        .with_timer(formatter::default_timer())
        .with_env_filter("info")
        .with_span_events(formatter::default_span_events())
        .with_target(true)
        .try_init()
    {
        warn!(
            cid = %cid,
            error = %e,
            "logging::init failed to initialize subscriber"
        );
    }
}

/// Keep logs persistent while bounding disk usage.
pub fn init_with_optional_file(log_path: Option<PathBuf>) -> Result<()> {
    let cid = bootstrap_cid("core-log-init");
    let env_filter = match EnvFilter::try_from_default_env() {
        Ok(filter) => filter,
        Err(e) => {
            warn!(
                cid = %cid,
                error = %e,
                "logging::init_with_optional_file failed to parse RUST_LOG; using info"
            );
            EnvFilter::new("info")
        }
    };
    if let Some(path) = log_path {
        let (writer, guard) = sinks::rolling_file_sink(&path)
            .context("logging::init_with_optional_file failed to build file sink")?;
        if FILE_GUARD.set(guard).is_err() {
            warn!(
                cid = %cid,
                action = "file_guard_set_failed",
                "logging::init_with_optional_file failed to set file guard"
            );
        }
        if let Err(e) = tracing_subscriber::fmt()
            .json()
            .flatten_event(true)
            .with_current_span(false)
            .with_span_list(false)
            .with_env_filter(env_filter)
            .with_writer(writer)
            .with_timer(formatter::default_timer())
            .with_span_events(formatter::default_span_events())
            .with_target(true)
            .try_init()
        {
            warn!(
                cid = %cid,
                error = %e,
                "logging::init_with_optional_file failed to initialize file subscriber"
            );
        }
    } else if let Err(e) = tracing_subscriber::fmt()
        .json()
        .flatten_event(true)
        .with_current_span(false)
        .with_span_list(false)
        .with_env_filter(env_filter)
        .with_timer(formatter::default_timer())
        .with_span_events(formatter::default_span_events())
        .with_target(true)
        .try_init()
    {
        warn!(
            cid = %cid,
            error = %e,
            "logging::init_with_optional_file failed to initialize stdout subscriber"
        );
    }
    Ok(())
}

/// Avoid leaking full user directory paths in logs.
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

/// Some error chains include embedded paths and still need safe logging.
pub fn redact_text(text: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let home_str = home.display().to_string();
        if !home_str.is_empty() {
            return text.replace(&home_str, "~");
        }
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::{correlation_id, sanitize_correlation_id};

    #[test]
    fn sanitize_correlation_id_keeps_safe_ascii() {
        let id = sanitize_correlation_id("gui-run/123:abc.def");
        assert_eq!(id, "gui-run/123:abc.def");
    }

    #[test]
    fn sanitize_correlation_id_replaces_unsafe_chars() {
        let id = sanitize_correlation_id("gui\nrun\tid");
        assert_eq!(id, "gui_run_id");
    }

    #[test]
    fn correlation_id_uses_incoming_when_present() {
        let id = correlation_id("prefix", Some(" incoming-id "));
        assert_eq!(id, "incoming-id");
    }
}
