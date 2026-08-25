use anyhow::Error;

/// Keep boundary error classification centralized and consistent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalErrorCode {
    ConfigInvalid,
    ConfigUnavailable,
    InvalidInput,
    NotFound,
    PermissionDenied,
    Timeout,
    Cancelled,
    DependencyUnavailable,
    Unsupported,
    Conflict,
    Internal,
}

impl CanonicalErrorCode {
    /// Preserve deterministic code values at process boundaries.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ConfigInvalid => "CONFIG_INVALID",
            Self::ConfigUnavailable => "CONFIG_UNAVAILABLE",
            Self::InvalidInput => "INVALID_INPUT",
            Self::NotFound => "NOT_FOUND",
            Self::PermissionDenied => "PERMISSION_DENIED",
            Self::Timeout => "TIMEOUT",
            Self::Cancelled => "CANCELLED",
            Self::DependencyUnavailable => "DEPENDENCY_UNAVAILABLE",
            Self::Unsupported => "UNSUPPORTED",
            Self::Conflict => "CONFLICT",
            Self::Internal => "INTERNAL",
        }
    }

    /// Attach deterministic retry guidance to structured failures.
    pub fn retryable(self) -> bool {
        matches!(
            self,
            Self::Timeout | Self::Cancelled | Self::DependencyUnavailable
        )
    }

    /// Keep failure guidance consistent without duplicating strings.
    pub fn hint(self) -> &'static str {
        match self {
            Self::ConfigInvalid => {
                "Fix the invalid configuration values, then rerun the command or restart the process."
            }
            Self::ConfigUnavailable => {
                "Create or repair the config file path and verify read permissions."
            }
            Self::InvalidInput => {
                "Review command inputs and config constraints, then provide valid values."
            }
            Self::NotFound => "Check that required files, sockets, and paths exist before retrying.",
            Self::PermissionDenied => {
                "Run with the required filesystem/service permissions for this operation."
            }
            Self::Timeout => {
                "Retry after the dependency recovers or increase configured operation timeouts."
            }
            Self::Cancelled => {
                "Retry once cancellation conditions clear, and confirm shutdown is not in progress."
            }
            Self::DependencyUnavailable => {
                "Verify dependent services and destination paths are reachable, then retry."
            }
            Self::Unsupported => {
                "Use a supported platform/feature combination for this operation."
            }
            Self::Conflict => {
                "Resolve conflicting state first, then retry the operation."
            }
            Self::Internal => {
                "Inspect logs with correlation id and open an issue if the failure persists."
            }
        }
    }
}

/// Provide one classifier contract for all boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassifiedBoundaryError {
    pub code: CanonicalErrorCode,
    pub hint: &'static str,
    pub retryable: bool,
}

/// Enforce one classification pipeline across process boundaries.
pub fn classify_anyhow(error: &Error) -> ClassifiedBoundaryError {
    let code = classify_from_chain(error).unwrap_or_else(|| classify_from_message(error));
    ClassifiedBoundaryError {
        hint: code.hint(),
        retryable: code.retryable(),
        code,
    }
}

/// Prefer typed root-cause signals over string matching.
fn classify_from_chain(error: &Error) -> Option<CanonicalErrorCode> {
    for cause in error.chain() {
        if let Some(io_error) = cause.downcast_ref::<std::io::Error>() {
            return Some(match io_error.kind() {
                std::io::ErrorKind::NotFound => CanonicalErrorCode::NotFound,
                std::io::ErrorKind::PermissionDenied => CanonicalErrorCode::PermissionDenied,
                std::io::ErrorKind::TimedOut => CanonicalErrorCode::Timeout,
                std::io::ErrorKind::Interrupted => CanonicalErrorCode::Cancelled,
                std::io::ErrorKind::WouldBlock => CanonicalErrorCode::Timeout,
                std::io::ErrorKind::ConnectionRefused
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::NotConnected
                | std::io::ErrorKind::BrokenPipe => CanonicalErrorCode::DependencyUnavailable,
                std::io::ErrorKind::AlreadyExists => CanonicalErrorCode::Conflict,
                std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => {
                    CanonicalErrorCode::InvalidInput
                }
                _ => CanonicalErrorCode::Internal,
            });
        }
    }
    None
}

/// Preserve deterministic mapping even when errors are context wrappers.
fn classify_from_message(error: &Error) -> CanonicalErrorCode {
    let msg = format!("{error:#}").to_ascii_lowercase();
    if msg.contains("invalid configuration")
        || (msg.contains("config") && msg.contains("validation"))
        || (msg.contains("config") && msg.contains("must be"))
    {
        return CanonicalErrorCode::ConfigInvalid;
    }
    if msg.contains("failed to load config")
        || msg.contains("config file")
        || (msg.contains("config") && msg.contains("not found"))
    {
        return CanonicalErrorCode::ConfigUnavailable;
    }
    if msg.contains("timed out") || msg.contains("timeout") {
        return CanonicalErrorCode::Timeout;
    }
    if msg.contains("cancelled") || msg.contains("canceled") {
        return CanonicalErrorCode::Cancelled;
    }
    if msg.contains("permission denied") {
        return CanonicalErrorCode::PermissionDenied;
    }
    if msg.contains("not found") || msg.contains("no such file") || msg.contains("missing") {
        return CanonicalErrorCode::NotFound;
    }
    if msg.contains("connection refused")
        || msg.contains("unreachable")
        || msg.contains("offline")
        || msg.contains("temporarily unavailable")
    {
        return CanonicalErrorCode::DependencyUnavailable;
    }
    if msg.contains("unsupported") {
        return CanonicalErrorCode::Unsupported;
    }
    if msg.contains("already exists") || msg.contains("conflict") {
        return CanonicalErrorCode::Conflict;
    }
    if msg.contains("invalid") || msg.contains("must be") {
        return CanonicalErrorCode::InvalidInput;
    }
    CanonicalErrorCode::Internal
}

#[cfg(test)]
mod tests {
    use super::{classify_anyhow, CanonicalErrorCode};

    #[test]
    fn classify_anyhow_detects_config_invalid() {
        let error = anyhow::anyhow!(
            "config::load_validated_config invalid configuration from default path"
        );
        let classified = classify_anyhow(&error);
        assert_eq!(classified.code, CanonicalErrorCode::ConfigInvalid);
        assert!(!classified.retryable);
    }

    #[test]
    fn classify_anyhow_detects_permission_denied_from_io_chain() {
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let error = anyhow::Error::new(io).context("daemon startup failed");
        let classified = classify_anyhow(&error);
        assert_eq!(classified.code, CanonicalErrorCode::PermissionDenied);
    }

    #[test]
    fn classify_anyhow_detects_timeout_message() {
        let error = anyhow::anyhow!("ipc request timed out while waiting for daemon");
        let classified = classify_anyhow(&error);
        assert_eq!(classified.code, CanonicalErrorCode::Timeout);
        assert!(classified.retryable);
    }

    #[test]
    fn classify_anyhow_falls_back_to_internal() {
        let error = anyhow::anyhow!("unexpected failure in state reducer");
        let classified = classify_anyhow(&error);
        assert_eq!(classified.code, CanonicalErrorCode::Internal);
    }
}
