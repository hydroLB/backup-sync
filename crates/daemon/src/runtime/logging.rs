use chrono::Utc;
use std::path::Path;
use tracing::field::Empty;
use tracing::{error, info, warn};

/// Purpose: Generates a correlation id with a prefix and current timestamp.
///
/// Inputs: a string prefix for the id.
/// Outputs: a timestamped correlation id string.
/// Ties to: daemon logging and tracing.
/// Side effects: Reads system time.
/// Why: provide a consistent identifier for log grouping.
pub fn cid(prefix: &str) -> String {
    format!("{}-{}", prefix, Utc::now().timestamp_millis())
}

/// Purpose: Redacts sensitive path prefixes for log safety.
///
/// Inputs: an optional filesystem path.
/// Outputs: an optional redacted path string.
/// Ties to: structured logging for daemon events.
/// Side effects: Reads the user home directory.
/// Why: avoid leaking full user directory paths in logs.
fn redact_path(path: Option<&Path>) -> Option<String> {
    let raw = path.map(|p| p.display().to_string())?;
    if let Some(home) = dirs::home_dir() {
        let home_str = home.display().to_string();
        if raw.starts_with(&home_str) {
            return Some(raw.replacen(&home_str, "~", 1));
        }
    }
    Some(raw)
}

/// Purpose: Logs a structured info line with action metadata.
///
/// Inputs: an action name, correlation id, optional path, and message.
/// Outputs: `()` after emitting a log line.
/// Ties to: daemon logging for informational events.
/// Side effects: Emits structured info logs.
/// Why: standardize log output for parsing and troubleshooting.
pub fn log_info(action: &str, cid: &str, path: Option<&Path>, message: &str) {
    let redacted = redact_path(path);
    match redacted {
        Some(p) => info!(
            component = "daemon",
            cid = cid,
            action = action,
            path = %p,
            message = message,
            "daemon_event"
        ),
        None => info!(
            component = "daemon",
            cid = cid,
            action = action,
            path = Empty,
            message = message,
            "daemon_event"
        ),
    }
}

/// Purpose: Logs a structured warning line with action metadata.
///
/// Inputs: an action name, correlation id, optional path, and message.
/// Outputs: `()` after emitting a log line.
/// Ties to: daemon logging for warning events.
/// Side effects: Emits structured warning logs.
/// Why: standardize warning output for troubleshooting.
pub fn log_warn(action: &str, cid: &str, path: Option<&Path>, message: &str) {
    let redacted = redact_path(path);
    match redacted {
        Some(p) => warn!(
            component = "daemon",
            cid = cid,
            action = action,
            path = %p,
            message = message,
            "daemon_event"
        ),
        None => warn!(
            component = "daemon",
            cid = cid,
            action = action,
            path = Empty,
            message = message,
            "daemon_event"
        ),
    }
}

/// Purpose: Logs a structured error line with action metadata.
///
/// Inputs: an action name, correlation id, optional path, and message.
/// Outputs: `()` after emitting a log line.
/// Ties to: daemon logging for error events.
/// Side effects: Emits structured error logs.
/// Why: standardize error output for troubleshooting.
pub fn log_error(action: &str, cid: &str, path: Option<&Path>, message: &str) {
    let redacted = redact_path(path);
    match redacted {
        Some(p) => error!(
            component = "daemon",
            cid = cid,
            action = action,
            path = %p,
            message = message,
            "daemon_event"
        ),
        None => error!(
            component = "daemon",
            cid = cid,
            action = action,
            path = Empty,
            message = message,
            "daemon_event"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Purpose: Ensures correlation ids include the provided prefix.
    ///
    /// Inputs: a test prefix.
    /// Outputs: an id string that starts with the prefix.
    /// Ties to: cid generation behavior.
    /// Side effects: None.
    /// Why: keep log ids traceable to their origin.
    fn cid_has_prefix() {
        let id = cid("test");
        assert!(id.starts_with("test-"));
    }
}
