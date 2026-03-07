use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::fmt::time::ChronoUtc;

/// Summary: Builds the default timer used by tracing logs.
///
/// Inputs: none.
///
/// Outputs: a `ChronoUtc` timer configured for RFC 3339 timestamps.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: log formatting across core and daemon.
///
/// Why this exists: keep timestamp formatting consistent across logs.
pub fn default_timer() -> ChronoUtc {
    ChronoUtc::rfc_3339()
}

/// Summary: Provides the default span event configuration for logs.
///
/// Inputs: none.
///
/// Outputs: a `FmtSpan` configuration value.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: log formatting configuration.
///
/// Why this exists: control span lifecycle events without extra noise.
pub fn default_span_events() -> FmtSpan {
    FmtSpan::CLOSE
}
