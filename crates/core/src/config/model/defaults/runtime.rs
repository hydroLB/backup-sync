/// Summary: Supplies the default interval for state pruning cycles.
///
/// Inputs: none.
///
/// Outputs: the number of cycles between prune passes.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon state maintenance.
///
/// Why this exists: keep state clean without pruning every cycle.
pub(crate) fn default_prune_interval_cycles() -> u64 {
    10
}

/// Summary: Supplies the default interval (in cycles) for forcing a full scan even when watchers report no changes.
///
/// Inputs: none.
///
/// Outputs: the number of cycles between forced full scans.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon cycle skipping based on watcher dirty sets.
///
/// Why this exists: watchers can miss events; periodic full scans provide a safety net.
pub(crate) fn default_force_full_scan_interval_cycles() -> u64 {
    24
}

/// Summary: Supplies the default interval for verification cycles.
///
/// Inputs: none.
///
/// Outputs: the verification interval in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon verification scheduling.
///
/// Why this exists: keep verification periodic without excessive IO.
pub(crate) fn default_verify_interval_seconds() -> u64 {
    24 * 3600
}

/// Summary: Supplies the default interval between full blob scrubs.
///
/// Inputs: none.
///
/// Outputs: the full scrub interval in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon scrub scheduling for bit-rot detection.
///
/// Why this exists: full re-hash is IO-heavy; run it periodically while using sampled scrubs in between.
pub(crate) fn default_scrub_full_interval_seconds() -> u64 {
    7 * 24 * 3600
}

/// Summary: Supplies the default number of blobs to hash in a sampled scrub.
///
/// Inputs: none.
///
/// Outputs: number of blobs to hash per sampled scrub run.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon scrub scheduling for fast-path integrity checks.
///
/// Why this exists: keep sampled scrubs bounded while still providing ongoing bit-rot detection coverage.
pub(crate) fn default_scrub_sample_blobs() -> usize {
    200
}

/// Summary: Supplies the default number of versions per source to inspect in a sampled scrub.
///
/// Inputs: none.
///
/// Outputs: number of newest versions to inspect per source.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: sampled scrub manifest cross-checks.
///
/// Why this exists: ensure the fast path still validates manifests without scanning the entire history each run.
pub(crate) fn default_scrub_sample_versions_per_source() -> usize {
    2
}

/// Summary: Supplies the default watcher debounce duration.
///
/// Inputs: none.
///
/// Outputs: the debounce duration in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: file watcher debounce timing.
///
/// Why this exists: reduce noisy watcher events before planning.
pub(crate) fn default_watcher_debounce_seconds() -> u64 {
    2
}

/// Summary: Supplies the default IPC timeout in seconds.
///
/// Inputs: none.
///
/// Outputs: the IPC timeout in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon IPC read and write operations.
///
/// Why this exists: keep IPC requests bounded.
pub(crate) fn default_ipc_timeout_seconds() -> u64 {
    5
}

/// Summary: Supplies the default max request size for daemon IPC payloads.
///
/// Inputs: none.
///
/// Outputs: max IPC request size in bytes.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon IPC request parsing limits.
///
/// Why this exists: avoid unbounded request buffering on malformed clients.
pub(crate) fn default_ipc_request_max_bytes() -> usize {
    64 * 1024
}

/// Summary: Supplies the default max response size for IPC status payloads.
///
/// Inputs: none.
///
/// Outputs: max IPC response size in bytes.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI/CLI IPC response buffering limits.
///
/// Why this exists: avoid unbounded response buffering on malformed servers.
pub(crate) fn default_ipc_response_max_bytes() -> usize {
    4 * 1024 * 1024
}

/// Summary: Supplies the default read chunk size for IPC request and response streams.
///
/// Inputs: none.
///
/// Outputs: IPC read chunk size in bytes.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon and GUI IPC read loops.
///
/// Why this exists: keep stream processing efficient without hardcoded buffer literals.
pub(crate) fn default_ipc_read_chunk_bytes() -> usize {
    8192
}

/// Summary: Supplies the default shutdown join timeout used by daemon background tasks.
///
/// Inputs: none.
///
/// Outputs: task join timeout in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon runtime shutdown coordination.
///
/// Why this exists: bound graceful task joins without immediate aborts.
pub(crate) fn default_daemon_shutdown_join_timeout_seconds() -> u64 {
    30
}

/// Summary: Supplies the default service command timeout in seconds.
///
/// Inputs: none.
///
/// Outputs: the service command timeout in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: service installation and control commands.
///
/// Why this exists: keep service operations bounded.
pub(crate) fn default_service_command_timeout_seconds() -> u64 {
    15
}

/// Summary: Supplies the default service command retry delay in milliseconds.
///
/// Inputs: none.
///
/// Outputs: the retry delay in milliseconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: service enable and restart retries.
///
/// Why this exists: avoid hammering system daemons on transient failures.
pub(crate) fn default_service_command_retry_delay_ms() -> u64 {
    300
}

/// Summary: Supplies the default service command poll interval in milliseconds.
///
/// Inputs: None.
///
/// Outputs: The poll interval in milliseconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: service command execution polling loops.
///
/// Why this exists: Avoid hardcoded sleep intervals during service command polling.
pub(crate) fn default_service_command_poll_interval_ms() -> u64 {
    50
}

/// Summary: Supplies the default enablement flag for source snapshots.
///
/// Inputs: none.
///
/// Outputs: false by default.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: optional consistent reads during versioned backup scanning and blob writes.
///
/// Why this exists: snapshots may require privileges and are not universally available; opt-in avoids surprises.
pub(crate) fn default_source_snapshots_enabled() -> bool {
    false
}

/// Summary: Supplies the default timeout for source snapshot operations in seconds.
///
/// Inputs: none.
///
/// Outputs: the timeout in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: snapshot create, mount, and cleanup commands when enabled.
///
/// Why this exists: keep OS-level snapshot operations bounded so backup cycles cannot hang indefinitely.
pub(crate) fn default_source_snapshot_timeout_seconds() -> u64 {
    20
}

/// Summary: Supplies the default enablement flag for destination replication.
///
/// Inputs: none.
///
/// Outputs: true by default.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: versioned store replication for multi-destination and 3-2-1 workflows.
///
/// Why this exists: replication is opt-in per destination via `replicate_to`, but can be disabled globally for safety.
pub(crate) fn default_replication_enabled() -> bool {
    true
}

/// Summary: Supplies the default mirror behavior for replication manifest pruning.
///
/// Inputs: none.
///
/// Outputs: true by default.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: replication behavior when primary retention prunes old versions.
///
/// Why this exists: replicas should usually reflect the same visible version set as the primary store.
pub(crate) fn default_replication_mirror_manifests() -> bool {
    true
}

/// Summary: Supplies the default safety cap for manifest deletions per replication cycle.
///
/// Inputs: none.
///
/// Outputs: maximum number of manifest files to delete from a replica per cycle.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: replication mirror deletion safety.
///
/// Why this exists: prevent accidental large deletions when destination mappings are misconfigured.
pub(crate) fn default_replication_max_manifest_deletes_per_cycle() -> usize {
    500
}

/// Summary: Supplies the default "large deletion" threshold for pinning an extra version.
///
/// Inputs: none.
///
/// Outputs: A ratio in `[0.0, 1.0]` representing the minimum shrink to trigger pinning.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: versioned store pruning behavior.
///
/// Why this exists: Keep a safety baseline when a watched path suddenly shrinks dramatically.
pub(crate) fn default_large_deletion_keep_extra_threshold_ratio() -> f64 {
    0.5
}
