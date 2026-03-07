use tracing::warn;

/// Summary: Emits a warning when skipped planning items are detected.
///
/// Inputs: the count of skipped items.
///
/// Outputs: `()` after the warning is emitted.
///
/// Side effects: Emits warning logs when skips are detected.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: planning and scanning to surface user visible warnings.
///
/// Why this exists: make skipped items visible during troubleshooting.
pub fn log_skipped_count(skipped: usize) {
    if skipped > 0 {
        warn!(
            "planning::log_skipped_count skipped {} items during planning or scanning; see logs for details",
            skipped
        );
    }
}
