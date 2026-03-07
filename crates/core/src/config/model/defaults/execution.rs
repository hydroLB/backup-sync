/// Summary: Supplies the default parallel copy limit for backup execution.
///
/// Inputs: none.
///
/// Outputs: the maximum number of concurrent copies.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: the executor setup.
///
/// Why this exists: keep default IO pressure low on typical machines.
pub(crate) fn default_max_parallel_copies() -> usize {
    1
}

/// Summary: Supplies the default throughput cap for backup execution.
///
/// Inputs: none.
///
/// Outputs: an optional bytes per second cap.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: the executor copy throttle.
///
/// Why this exists: allow unlimited throughput unless the user opts in.
pub(crate) fn default_max_bytes_per_second() -> Option<u64> {
    None
}

/// Summary: Supplies the default minimum free space guard for backups.
///
/// Inputs: none.
///
/// Outputs: an optional byte threshold.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: the executor free space check.
///
/// Why this exists: allow backups unless the user explicitly sets a floor.
pub(crate) fn default_min_free_space_bytes() -> Option<u64> {
    None
}

/// Summary: Supplies the copy buffer size used by the executor.
///
/// Inputs: none.
///
/// Outputs: a byte count for the IO buffer.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `copy_with_throttle` for read and write chunking.
///
/// Why this exists: balance throughput with memory footprint by default.
pub(crate) fn default_copy_buffer_bytes() -> usize {
    64 * 1024
}

/// Summary: Supplies the default copy timeout in seconds.
///
/// Inputs: none.
///
/// Outputs: the copy timeout in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: executor copy timeouts.
///
/// Why this exists: keep IO operations bounded by default.
pub(crate) fn default_copy_timeout_seconds() -> u64 {
    300
}

/// Summary: Supplies the free space safety buffer used during execution.
///
/// Inputs: none.
///
/// Outputs: a byte count added to the file size requirement.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `BackupExecutor` free space checks.
///
/// Why this exists: reduce the chance of failing mid copy due to filesystem overhead.
pub(crate) fn default_free_space_safety_buffer_bytes() -> u64 {
    10 * 1024 * 1024
}

/// Summary: Supplies the maximum number of activity entries kept in state.
///
/// Inputs: none.
///
/// Outputs: the maximum recent activity count.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: executor state updates after successful backups.
///
/// Why this exists: keep state compact while preserving a short UI timeline.
pub(crate) fn default_recent_activity_cap() -> usize {
    50
}

/// Summary: Supplies the retry delay schedule for transient IO operations.
///
/// Inputs: none.
///
/// Outputs: a list of millisecond delays.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: executor hashing and copy retries.
///
/// Why this exists: provide a conservative backoff without stalling the cycle.
pub(crate) fn default_retry_delays_ms() -> Vec<u64> {
    vec![100, 200, 400, 800]
}

/// Summary: Supplies the default retry jitter percentage.
///
/// Inputs: none.
///
/// Outputs: a jitter percentage between 0 and 1.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: retry backoff variance to avoid thundering herds.
///
/// Why this exists: reduce coordinated retries on transient failures.
pub(crate) fn default_retry_jitter_pct() -> f64 {
    0.2
}

/// Summary: Supplies the poll interval for blocking I/O timeout/cancellation checks.
///
/// Inputs: none.
///
/// Outputs: poll interval in milliseconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `BlockingIoPolicy` backoff sleeps and cancellation responsiveness.
///
/// Why this exists: avoid hardcoded poll intervals inside blocking I/O helpers.
pub(crate) fn default_blocking_io_backoff_poll_interval_ms() -> u64 {
    50
}
