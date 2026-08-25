use anyhow::Error;
use backup_core::boundary_error::{classify_anyhow, CanonicalErrorCode};
use backup_core::logging::redact_text;

/// Keep daemon failure contracts explicit for operators and automation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DaemonErrorCode {
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

impl DaemonErrorCode {
    /// Preserve deterministic error taxonomy at daemon boundaries.
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

    /// Keep daemon lifecycle semantics reproducible across environments.
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Internal => 1,
            Self::ConfigInvalid | Self::ConfigUnavailable => 2,
            Self::InvalidInput => 3,
            Self::NotFound => 4,
            Self::PermissionDenied => 5,
            Self::Timeout | Self::Cancelled | Self::DependencyUnavailable => 6,
            Self::Unsupported => 7,
            Self::Conflict => 8,
        }
    }
}

impl From<CanonicalErrorCode> for DaemonErrorCode {
    /// Ensure daemon code taxonomy stays aligned with canonical classifier output.
    fn from(value: CanonicalErrorCode) -> Self {
        match value {
            CanonicalErrorCode::ConfigInvalid => Self::ConfigInvalid,
            CanonicalErrorCode::ConfigUnavailable => Self::ConfigUnavailable,
            CanonicalErrorCode::InvalidInput => Self::InvalidInput,
            CanonicalErrorCode::NotFound => Self::NotFound,
            CanonicalErrorCode::PermissionDenied => Self::PermissionDenied,
            CanonicalErrorCode::Timeout => Self::Timeout,
            CanonicalErrorCode::Cancelled => Self::Cancelled,
            CanonicalErrorCode::DependencyUnavailable => Self::DependencyUnavailable,
            CanonicalErrorCode::Unsupported => Self::Unsupported,
            CanonicalErrorCode::Conflict => Self::Conflict,
            CanonicalErrorCode::Internal => Self::Internal,
        }
    }
}

/// Centralize daemon operator-facing error rendering.
#[derive(Debug, Clone)]
pub struct DaemonBoundaryError {
    pub code: DaemonErrorCode,
    pub message: String,
    pub hint: &'static str,
    pub retryable: bool,
}

impl DaemonBoundaryError {
    /// Keep daemon exits reproducible for service managers.
    pub fn exit_code(&self) -> i32 {
        self.code.exit_code()
    }

    /// Preserve operator guidance when logs are unavailable.
    pub fn render_for_user(&self) -> String {
        format!(
            "{}: {}\nHint: {}",
            self.code.as_str(),
            self.message,
            self.hint
        )
    }
}

/// Keep daemon boundary mapping centralized and testable.
pub fn map_anyhow(error: &Error, context: &'static str) -> DaemonBoundaryError {
    let classified = classify_anyhow(error);
    DaemonBoundaryError {
        code: DaemonErrorCode::from(classified.code),
        message: format!("{context}: {}", redact_text(&format!("{error:#}"))),
        hint: classified.hint,
        retryable: classified.retryable,
    }
}

/// Keep panic outputs consistent with runtime boundary failures.
pub fn map_panic(panic_text: &str) -> DaemonBoundaryError {
    DaemonBoundaryError {
        code: DaemonErrorCode::Internal,
        message: redact_text(panic_text),
        hint: "Inspect daemon logs with correlation id and restart after resolving the root cause.",
        retryable: false,
    }
}

#[cfg(test)]
mod tests {
    use super::{map_anyhow, DaemonErrorCode};

    #[test]
    fn map_anyhow_sets_config_code() {
        let error = anyhow::anyhow!("daemon::main failed to load configuration from config file");
        let mapped = map_anyhow(&error, "daemon startup failed");
        assert_eq!(mapped.code, DaemonErrorCode::ConfigUnavailable);
        assert_eq!(mapped.exit_code(), 2);
    }

    #[test]
    fn map_anyhow_sets_dependency_unavailable() {
        let error = anyhow::anyhow!("daemon IPC connection refused");
        let mapped = map_anyhow(&error, "daemon runtime failed");
        assert_eq!(mapped.code, DaemonErrorCode::DependencyUnavailable);
        assert!(mapped.retryable);
    }
}
