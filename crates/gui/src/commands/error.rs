use serde::Serialize;

/// Purpose: Error payload returned by GUI commands.
///
/// Inputs: an error code and message.
/// Outputs: a serializable error envelope.
/// Ties to: Tauri command error responses.
/// Side effects: None.
/// Why: provide structured errors for the frontend.
#[derive(Serialize, Debug, Clone)]
pub struct ErrorEnvelope {
    pub code: &'static str,
    pub message: String,
}

impl ErrorEnvelope {
    /// Purpose: Creates a new error envelope with a code and message.
    ///
    /// Inputs: a static error code and a message.
    /// Outputs: an `ErrorEnvelope`.
    /// Ties to: GUI command error handling.
    /// Side effects: None.
    /// Why: standardize error payload creation.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ErrorEnvelope {
    /// Purpose: Formats the error envelope for display.
    ///
    /// Inputs: the formatter and the error envelope.
    /// Outputs: a formatted string.
    /// Ties to: logging and debug output.
    /// Side effects: None.
    /// Why: provide readable error output when logged.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
