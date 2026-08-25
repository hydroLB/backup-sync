use tracing::warn;

/// Make skipped items visible during troubleshooting.
pub fn log_skipped_count(skipped: usize) {
    if skipped > 0 {
        warn!(
            "planning::log_skipped_count skipped {} items during planning or scanning; see logs for details",
            skipped
        );
    }
}
