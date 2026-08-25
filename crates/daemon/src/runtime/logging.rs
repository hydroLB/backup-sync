use chrono::Utc;
use std::path::Path;
use tracing::field::Empty;
use tracing::{info, warn};

/// Provide a consistent identifier for log grouping.
pub fn cid(prefix: &str) -> String {
    format!("{}-{}", prefix, Utc::now().timestamp_millis())
}

/// Avoid leaking full user directory paths in logs.
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

/// Standardize log output for parsing and troubleshooting.
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

/// Standardize warning output for troubleshooting.
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
    /// Keep log ids traceable to their origin.
    fn cid_has_prefix() {
        let id = cid("test");
        assert!(id.starts_with("test-"));
    }
}
