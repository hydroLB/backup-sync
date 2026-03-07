use chrono::Utc;
use std::path::Path;
use tracing::field::Empty;
use tracing::{info, warn};

/// Summary: Generates a correlation id with a prefix and current timestamp.
///
/// Inputs: a string prefix for the id.
///
/// Outputs: a timestamped correlation id string.
///
/// Side effects: Reads system time.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon logging and tracing.
///
/// Why this exists: provide a consistent identifier for log grouping.
pub fn cid(prefix: &str) -> String {
    format!("{}-{}", prefix, Utc::now().timestamp_millis())
}

/// Summary: Redacts sensitive path prefixes for log safety.
///
/// Inputs: an optional filesystem path.
///
/// Outputs: an optional redacted path string.
///
/// Side effects: Reads the user home directory.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: structured logging for daemon events.
///
/// Why this exists: avoid leaking full user directory paths in logs.
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

/// Summary: Logs a structured info line with action metadata.
///
/// Inputs: an action name, correlation id, optional path, and message.
///
/// Outputs: `()` after emitting a log line.
///
/// Side effects: Emits structured info logs.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon logging for informational events.
///
/// Why this exists: standardize log output for parsing and troubleshooting.
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

/// Summary: Logs a structured warning line with action metadata.
///
/// Inputs: an action name, correlation id, optional path, and message.
///
/// Outputs: `()` after emitting a log line.
///
/// Side effects: Emits structured warning logs.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon logging for warning events.
///
/// Why this exists: standardize warning output for troubleshooting.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Summary: Ensures correlation ids include the provided prefix.
    ///
    /// Inputs: a test prefix.
    ///
    /// Outputs: an id string that starts with the prefix.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: cid generation behavior.
    ///
    /// Why this exists: keep log ids traceable to their origin.
    fn cid_has_prefix() {
        let id = cid("test");
        assert!(id.starts_with("test-"));
    }
}
