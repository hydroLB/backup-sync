/// Purpose: Supplies the default interval for state pruning cycles.
///
/// Inputs: none.
/// Outputs: the number of cycles between prune passes.
/// Ties to: daemon state maintenance.
/// Side effects: None.
/// Why: keep state clean without pruning every cycle.
pub(crate) fn default_prune_interval_cycles() -> u64 {
    10
}

/// Purpose: Supplies the default interval for verification cycles.
///
/// Inputs: none.
/// Outputs: the verification interval in seconds.
/// Ties to: daemon verification scheduling.
/// Side effects: None.
/// Why: keep verification periodic without excessive IO.
pub(crate) fn default_verify_interval_seconds() -> u64 {
    24 * 3600
}

/// Purpose: Supplies the default interval between full blob scrubs.
///
/// Inputs: none.
/// Outputs: the full scrub interval in seconds.
/// Ties to: daemon scrub scheduling for bit-rot detection.
/// Side effects: None.
/// Why: full re-hash is IO-heavy; run it periodically while using sampled scrubs in between.
pub(crate) fn default_scrub_full_interval_seconds() -> u64 {
    7 * 24 * 3600
}

/// Purpose: Supplies the default number of blobs to hash in a sampled scrub.
///
/// Inputs: none.
/// Outputs: number of blobs to hash per sampled scrub run.
/// Ties to: daemon scrub scheduling for fast-path integrity checks.
/// Side effects: None.
/// Why: keep sampled scrubs bounded while still providing ongoing bit-rot detection coverage.
pub(crate) fn default_scrub_sample_blobs() -> usize {
    200
}

/// Purpose: Supplies the default number of versions per source to inspect in a sampled scrub.
///
/// Inputs: none.
/// Outputs: number of newest versions to inspect per source.
/// Ties to: sampled scrub manifest cross-checks.
/// Side effects: None.
/// Why: ensure the fast path still validates manifests without scanning the entire history each run.
pub(crate) fn default_scrub_sample_versions_per_source() -> usize {
    2
}

/// Purpose: Supplies the default watcher debounce duration.
///
/// Inputs: none.
/// Outputs: the debounce duration in seconds.
/// Ties to: file watcher debounce timing.
/// Side effects: None.
/// Why: reduce noisy watcher events before planning.
pub(crate) fn default_watcher_debounce_seconds() -> u64 {
    2
}

/// Purpose: Supplies the default IPC timeout in seconds.
///
/// Inputs: none.
/// Outputs: the IPC timeout in seconds.
/// Ties to: daemon IPC read and write operations.
/// Side effects: None.
/// Why: keep IPC requests bounded.
pub(crate) fn default_ipc_timeout_seconds() -> u64 {
    5
}

/// Purpose: Supplies the default service command timeout in seconds.
///
/// Inputs: none.
/// Outputs: the service command timeout in seconds.
/// Ties to: service installation and control commands.
/// Side effects: None.
/// Why: keep service operations bounded.
pub(crate) fn default_service_command_timeout_seconds() -> u64 {
    15
}

/// Purpose: Supplies the default service command retry delay in milliseconds.
///
/// Inputs: none.
/// Outputs: the retry delay in milliseconds.
/// Ties to: service enable and restart retries.
/// Side effects: None.
/// Why: avoid hammering system daemons on transient failures.
pub(crate) fn default_service_command_retry_delay_ms() -> u64 {
    300
}

/// Purpose: Supplies the default service command poll interval in milliseconds.
///
/// Inputs: None.
/// Outputs: The poll interval in milliseconds.
/// Ties to: service command execution polling loops.
/// Side effects: None.
/// Why: Avoid hardcoded sleep intervals during service command polling.
pub(crate) fn default_service_command_poll_interval_ms() -> u64 {
    50
}

/// Purpose: Supplies the default enablement flag for source snapshots.
///
/// Inputs: none.
/// Outputs: false by default.
/// Ties to: optional consistent reads during versioned backup scanning and blob writes.
/// Side effects: None.
/// Why: snapshots may require privileges and are not universally available; opt-in avoids surprises.
pub(crate) fn default_source_snapshots_enabled() -> bool {
    false
}

/// Purpose: Supplies the default timeout for source snapshot operations in seconds.
///
/// Inputs: none.
/// Outputs: the timeout in seconds.
/// Ties to: snapshot create, mount, and cleanup commands when enabled.
/// Side effects: None.
/// Why: keep OS-level snapshot operations bounded so backup cycles cannot hang indefinitely.
pub(crate) fn default_source_snapshot_timeout_seconds() -> u64 {
    20
}
