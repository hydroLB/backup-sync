use anyhow::Error;
use backup_core::logging::redact_text;
use serde::Serialize;

/// Make GUI boundary error contracts explicit and consistent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiErrorCode {
    ConfigLoad,
    ConfigSave,
    ConfigInvalid,
    DaemonOffline,
    StatusUnavailable,
    LogTail,
    LogExport,
    NoDesktop,
    NotFound,
    PermissionDenied,
    Timeout,
    Cancelled,
    InvalidInput,
    DependencyUnavailable,
    Unsupported,
    Conflict,
    Internal,
}

impl GuiErrorCode {
    /// Ensure GUI command failures are actionable by default.
    fn hint(self) -> &'static str {
        match self {
            Self::ConfigLoad => "Fix config path or permissions, then reload settings.",
            Self::ConfigSave => "Resolve validation or permission issues before saving again.",
            Self::ConfigInvalid => "Correct invalid configuration values and retry.",
            Self::DaemonOffline => "Start the daemon service, then retry the action.",
            Self::StatusUnavailable => {
                "Confirm daemon reachability and destination availability, then retry."
            }
            Self::LogTail => "Confirm log path availability and permissions, then retry log tail.",
            Self::LogExport => {
                "Confirm Desktop destination and log readability, then retry export."
            }
            Self::NoDesktop => "Set a writable Desktop directory for log and diagnostic exports.",
            Self::NotFound => "Ensure required files and directories exist.",
            Self::PermissionDenied => "Grant required filesystem/service permissions.",
            Self::Timeout => "Retry after dependency recovery or increase timeout settings.",
            Self::Cancelled => "Retry when cancellation and shutdown conditions are cleared.",
            Self::InvalidInput => "Adjust inputs to satisfy command constraints.",
            Self::DependencyUnavailable => "Check dependent services and retry.",
            Self::Unsupported => "Use a supported platform or feature combination.",
            Self::Conflict => "Resolve conflicting state and retry.",
            Self::Internal => "Inspect logs and report the issue if it persists.",
        }
    }

    /// Preserve backward compatibility while migrating to typed codes.
    fn from_legacy_code(code: &str) -> Self {
        match code {
            "CONFIG_LOAD" => Self::ConfigLoad,
            "CONFIG_SAVE" => Self::ConfigSave,
            "CONFIG_INVALID" => Self::ConfigInvalid,
            "DAEMON_OFFLINE" => Self::DaemonOffline,
            "STATUS_UNAVAILABLE" => Self::StatusUnavailable,
            "LOG_TAIL" => Self::LogTail,
            "LOG_EXPORT" => Self::LogExport,
            "NO_DESKTOP" => Self::NoDesktop,
            "NOT_FOUND" => Self::NotFound,
            "PERMISSION_DENIED" => Self::PermissionDenied,
            "TIMEOUT" => Self::Timeout,
            "CANCELLED" => Self::Cancelled,
            "INVALID_INPUT" => Self::InvalidInput,
            "DEPENDENCY_UNAVAILABLE" => Self::DependencyUnavailable,
            "UNSUPPORTED" => Self::Unsupported,
            "CONFLICT" => Self::Conflict,
            _ => Self::Internal,
        }
    }
}

/// Provide structured errors for the frontend.
#[derive(Serialize, Debug, Clone)]
pub struct ErrorEnvelope {
    pub code: String,
    pub message: String,
}

impl ErrorEnvelope {
    /// Preserve compatibility while normalizing actionable error messages.
    pub fn new(code: impl AsRef<str>, message: impl Into<String>) -> Self {
        let code = code.as_ref().to_string();
        let kind = GuiErrorCode::from_legacy_code(&code);
        Self {
            code,
            message: ensure_hint(message.into(), kind.hint()),
        }
    }

    /// Allow centralized mapping without breaking existing frontend code contracts.
    pub fn from_anyhow_with_code(
        code: impl AsRef<str>,
        context: &'static str,
        error: &Error,
    ) -> Self {
        let code = code.as_ref().to_string();
        let kind = GuiErrorCode::from_legacy_code(&code);
        let raw_message = format!("{context}: {}", redact_text(&format!("{error:#}")));
        Self {
            code,
            message: ensure_hint(raw_message, kind.hint()),
        }
    }
}

/// Keep GUI boundary errors actionable without duplicating hint text.
fn ensure_hint(message: String, hint: &str) -> String {
    if message.contains("Hint:") {
        return message;
    }
    format!("{message} Hint: {hint}")
}

impl std::fmt::Display for ErrorEnvelope {
    /// Provide readable error output when logged.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorEnvelope, GuiErrorCode};

    #[test]
    fn new_maps_legacy_code_and_adds_hint() {
        let envelope = ErrorEnvelope::new("CONFIG_LOAD", "failed loading config");
        assert_eq!(
            GuiErrorCode::from_legacy_code(&envelope.code),
            GuiErrorCode::ConfigLoad
        );
        assert!(envelope.message.contains("Hint:"));
    }

    #[test]
    fn from_anyhow_with_code_preserves_legacy_code() {
        let error = anyhow::anyhow!("ipc request timed out");
        let envelope = ErrorEnvelope::from_anyhow_with_code(
            "STATUS_UNAVAILABLE",
            "status fetch failed",
            &error,
        );
        assert_eq!(envelope.code, "STATUS_UNAVAILABLE");
        assert!(envelope.message.contains("Hint:"));
    }
}
