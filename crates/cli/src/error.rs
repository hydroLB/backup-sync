use anyhow::Error;
use backup_core::boundary_error::{classify_anyhow, CanonicalErrorCode};
use backup_core::logging::redact_text;

/// Summary: Typed CLI boundary error codes exposed in logs and exit handling.
///
/// Inputs: mapped from canonical boundary error codes.
///
/// Outputs: stable CLI error codes.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: CLI entrypoint boundary mapping.
///
/// Why this exists: keep CLI failure contracts explicit and machine-readable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliErrorCode {
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

impl CliErrorCode {
    /// Summary: Returns a stable code string for logs and terminal output.
    ///
    /// Inputs: none.
    ///
    /// Outputs: code string.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: CLI structured logging and user-facing errors.
    ///
    /// Why this exists: enforce deterministic error identifiers for automation.
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

    /// Summary: Returns deterministic process exit code for this failure class.
    ///
    /// Inputs: none.
    ///
    /// Outputs: POSIX-style non-zero exit code.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: CLI main process termination contract.
    ///
    /// Why this exists: provide reproducible exit semantics for scripts and CI jobs.
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

impl From<CanonicalErrorCode> for CliErrorCode {
    /// Summary: Maps canonical codes into CLI boundary-specific codes.
    ///
    /// Inputs: canonical boundary code.
    ///
    /// Outputs: CLI code.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: CLI boundary mapper.
    ///
    /// Why this exists: preserve one canonical classifier with boundary-local representation.
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

/// Summary: Structured CLI boundary failure payload.
///
/// Inputs: created from caught command/panic errors.
///
/// Outputs: typed boundary details for logging and process exit.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: CLI entrypoint failure handling.
///
/// Why this exists: centralize human and machine-facing CLI failure rendering.
#[derive(Debug, Clone)]
pub struct CliBoundaryError {
    pub code: CliErrorCode,
    pub message: String,
    pub hint: &'static str,
    pub retryable: bool,
}

impl CliBoundaryError {
    /// Summary: Returns the deterministic CLI process exit code for this error.
    ///
    /// Inputs: none.
    ///
    /// Outputs: non-zero exit code.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: CLI main process termination.
    ///
    /// Why this exists: keep process exit behavior centralized and testable.
    pub fn exit_code(&self) -> i32 {
        self.code.exit_code()
    }

    /// Summary: Renders a concise actionable error for terminal users.
    ///
    /// Inputs: none.
    ///
    /// Outputs: user-facing message string.
    ///
    /// Side effects: None.
    ///
    /// Error handling: Propagates contextual errors to the caller when operations fail.
    ///
    /// Ties to other methods: stderr reporting in CLI main.
    ///
    /// Why this exists: keep user guidance consistent for all top-level command failures.
    pub fn render_for_user(&self) -> String {
        format!(
            "{}: {}\nHint: {}",
            self.code.as_str(),
            self.message,
            self.hint
        )
    }
}

/// Summary: Maps an anyhow command failure into a structured CLI boundary error.
///
/// Inputs: command failure and boundary context label.
///
/// Outputs: typed CLI boundary error payload.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: CLI main command failure handler.
///
/// Why this exists: centralize CLI boundary mapping so all commands behave consistently.
pub fn map_anyhow(error: &Error, context: &'static str) -> CliBoundaryError {
    let classified = classify_anyhow(error);
    let code = CliErrorCode::from(classified.code);
    let message = format!("{context}: {}", redact_text(&format!("{error:#}")));
    CliBoundaryError {
        code,
        message,
        hint: classified.hint,
        retryable: classified.retryable,
    }
}

/// Summary: Maps panic payload text into a structured CLI boundary error.
///
/// Inputs: panic text from the process panic hook.
///
/// Outputs: typed internal CLI boundary error payload.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: CLI panic hook.
///
/// Why this exists: panics should emit the same structured boundary model as command failures.
pub fn map_panic(panic_text: &str) -> CliBoundaryError {
    CliBoundaryError {
        code: CliErrorCode::Internal,
        message: redact_text(panic_text),
        hint: "Inspect CLI logs and file an issue with the correlation id.",
        retryable: false,
    }
}

#[cfg(test)]
mod tests {
    use super::{map_anyhow, CliErrorCode};

    #[test]
    fn map_anyhow_sets_config_exit_code() {
        let error = anyhow::anyhow!("config::load_validated_config invalid configuration");
        let mapped = map_anyhow(&error, "cli::run failed");
        assert_eq!(mapped.code, CliErrorCode::ConfigInvalid);
        assert_eq!(mapped.exit_code(), 2);
    }

    #[test]
    fn map_anyhow_sets_permission_denied_code() {
        let error = anyhow::Error::new(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "permission denied",
        ));
        let mapped = map_anyhow(&error, "cli::run failed");
        assert_eq!(mapped.code, CliErrorCode::PermissionDenied);
        assert_eq!(mapped.exit_code(), 5);
    }

    #[test]
    fn map_anyhow_sets_retryable_timeout() {
        let error = anyhow::anyhow!("daemon IPC request timed out");
        let mapped = map_anyhow(&error, "cli::status failed");
        assert_eq!(mapped.code, CliErrorCode::Timeout);
        assert!(mapped.retryable);
    }
}
