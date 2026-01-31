/// Purpose: Supplies the default parallel copy limit for backup execution.
///
/// Inputs: none.
/// Outputs: the maximum number of concurrent copies.
/// Ties to: the executor setup.
/// Side effects: None.
/// Why: keep default IO pressure low on typical machines.
pub(crate) fn default_max_parallel_copies() -> usize {
    1
}

/// Purpose: Supplies the default throughput cap for backup execution.
///
/// Inputs: none.
/// Outputs: an optional bytes per second cap.
/// Ties to: the executor copy throttle.
/// Side effects: None.
/// Why: allow unlimited throughput unless the user opts in.
pub(crate) fn default_max_bytes_per_second() -> Option<u64> {
    None
}

/// Purpose: Supplies the default minimum free space guard for backups.
///
/// Inputs: none.
/// Outputs: an optional byte threshold.
/// Ties to: the executor free space check.
/// Side effects: None.
/// Why: allow backups unless the user explicitly sets a floor.
pub(crate) fn default_min_free_space_bytes() -> Option<u64> {
    None
}

/// Purpose: Supplies the copy buffer size used by the executor.
///
/// Inputs: none.
/// Outputs: a byte count for the IO buffer.
/// Ties to: `copy_with_throttle` for read and write chunking.
/// Side effects: None.
/// Why: balance throughput with memory footprint by default.
pub(crate) fn default_copy_buffer_bytes() -> usize {
    64 * 1024
}

/// Purpose: Supplies the default copy timeout in seconds.
///
/// Inputs: none.
/// Outputs: the copy timeout in seconds.
/// Ties to: executor copy timeouts.
/// Side effects: None.
/// Why: keep IO operations bounded by default.
pub(crate) fn default_copy_timeout_seconds() -> u64 {
    300
}

/// Purpose: Supplies the free space safety buffer used during execution.
///
/// Inputs: none.
/// Outputs: a byte count added to the file size requirement.
/// Ties to: `BackupExecutor` free space checks.
/// Side effects: None.
/// Why: reduce the chance of failing mid copy due to filesystem overhead.
pub(crate) fn default_free_space_safety_buffer_bytes() -> u64 {
    10 * 1024 * 1024
}

/// Purpose: Supplies the maximum number of activity entries kept in state.
///
/// Inputs: none.
/// Outputs: the maximum recent activity count.
/// Ties to: executor state updates after successful backups.
/// Side effects: None.
/// Why: keep state compact while preserving a short UI timeline.
pub(crate) fn default_recent_activity_cap() -> usize {
    50
}

/// Purpose: Supplies the retry delay schedule for transient IO operations.
///
/// Inputs: none.
/// Outputs: a list of millisecond delays.
/// Ties to: executor hashing and copy retries.
/// Side effects: None.
/// Why: provide a conservative backoff without stalling the cycle.
pub(crate) fn default_retry_delays_ms() -> Vec<u64> {
    vec![100, 200, 400, 800]
}

/// Purpose: Supplies the default retry jitter percentage.
///
/// Inputs: none.
/// Outputs: a jitter percentage between 0 and 1.
/// Ties to: retry backoff variance to avoid thundering herds.
/// Side effects: None.
/// Why: reduce coordinated retries on transient failures.
pub(crate) fn default_retry_jitter_pct() -> f64 {
    0.2
}
