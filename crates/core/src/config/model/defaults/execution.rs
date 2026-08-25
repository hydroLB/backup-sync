/// Keep default IO pressure low on typical machines.
pub(crate) fn default_max_parallel_copies() -> usize {
    1
}

/// Allow unlimited throughput unless the user opts in.
pub(crate) fn default_max_bytes_per_second() -> Option<u64> {
    None
}

/// Allow backups unless the user explicitly sets a floor.
pub(crate) fn default_min_free_space_bytes() -> Option<u64> {
    None
}

/// Balance throughput with memory footprint by default.
pub(crate) fn default_copy_buffer_bytes() -> usize {
    64 * 1024
}

/// Keep IO operations bounded by default.
pub(crate) fn default_copy_timeout_seconds() -> u64 {
    300
}

/// Reduce the chance of failing mid copy due to filesystem overhead.
pub(crate) fn default_free_space_safety_buffer_bytes() -> u64 {
    10 * 1024 * 1024
}

/// Keep state compact while preserving a short UI timeline.
pub(crate) fn default_recent_activity_cap() -> usize {
    50
}

/// Provide a conservative backoff without stalling the cycle.
pub(crate) fn default_retry_delays_ms() -> Vec<u64> {
    vec![100, 200, 400, 800]
}

/// Reduce coordinated retries on transient failures.
pub(crate) fn default_retry_jitter_pct() -> f64 {
    0.2
}

/// Avoid hardcoded poll intervals inside blocking I/O helpers.
pub(crate) fn default_blocking_io_backoff_poll_interval_ms() -> u64 {
    50
}
