use chrono::Utc;
use std::sync::atomic::{AtomicU64, Ordering};

static CID_SEQ: AtomicU64 = AtomicU64::new(0);

/// Purpose: Builds a correlation id for GUI command tracing.
///
/// Inputs: a prefix and optional incoming id.
/// Outputs: a correlation id string.
/// Ties to: logging and diagnostics across GUI commands.
/// Side effects: Reads system time and increments a process-local counter.
/// Why: provide collision-resistant correlation ids without requiring external randomness.
pub fn cid(prefix: &str, incoming: Option<String>) -> String {
    incoming.unwrap_or_else(|| {
        let ts = Utc::now().timestamp_millis();
        let seq = CID_SEQ.fetch_add(1, Ordering::Relaxed);
        format!("{prefix}-{ts}-{seq}")
    })
}
