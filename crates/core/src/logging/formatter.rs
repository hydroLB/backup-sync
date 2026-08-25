use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::fmt::time::ChronoUtc;

/// Keep timestamp formatting consistent across logs.
pub fn default_timer() -> ChronoUtc {
    ChronoUtc::rfc_3339()
}

/// Control span lifecycle events without extra noise.
pub fn default_span_events() -> FmtSpan {
    FmtSpan::CLOSE
}
