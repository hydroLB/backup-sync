use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::fmt::time::ChronoUtc;

/// Purpose: Builds the default timer used by tracing logs.
///
/// Inputs: none.
/// Outputs: a `ChronoUtc` timer configured for RFC 3339 timestamps.
/// Ties to: log formatting across core and daemon.
/// Side effects: None.
/// Why: keep timestamp formatting consistent across logs.
pub fn default_timer() -> ChronoUtc {
    ChronoUtc::rfc_3339()
}

/// Purpose: Provides the default span event configuration for logs.
///
/// Inputs: none.
/// Outputs: a `FmtSpan` configuration value.
/// Ties to: log formatting configuration.
/// Side effects: None.
/// Why: control span lifecycle events without extra noise.
pub fn default_span_events() -> FmtSpan {
    FmtSpan::CLOSE
}
