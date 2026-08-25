/// Keep state clean without pruning every cycle.
pub(crate) fn default_prune_interval_cycles() -> u64 {
    10
}

/// Watchers can miss events; periodic full scans provide a safety net.
pub(crate) fn default_force_full_scan_interval_cycles() -> u64 {
    24
}

/// Keep verification periodic without excessive IO.
pub(crate) fn default_verify_interval_seconds() -> u64 {
    24 * 3600
}

/// Full re-hash is IO-heavy; run it periodically while using sampled scrubs in between.
pub(crate) fn default_scrub_full_interval_seconds() -> u64 {
    7 * 24 * 3600
}

/// Keep sampled scrubs bounded while still providing ongoing bit-rot detection coverage.
pub(crate) fn default_scrub_sample_blobs() -> usize {
    200
}

/// Ensure the fast path still validates manifests without scanning the entire history each run.
pub(crate) fn default_scrub_sample_versions_per_source() -> usize {
    2
}

/// Reduce noisy watcher events before planning.
pub(crate) fn default_watcher_debounce_seconds() -> u64 {
    2
}

/// Keep IPC requests bounded.
pub(crate) fn default_ipc_timeout_seconds() -> u64 {
    5
}

/// Avoid unbounded request buffering on malformed clients.
pub(crate) fn default_ipc_request_max_bytes() -> usize {
    64 * 1024
}

/// Avoid unbounded response buffering on malformed servers.
pub(crate) fn default_ipc_response_max_bytes() -> usize {
    4 * 1024 * 1024
}

/// Keep stream processing efficient without hardcoded buffer literals.
pub(crate) fn default_ipc_read_chunk_bytes() -> usize {
    8192
}

/// Bound graceful task joins without immediate aborts.
pub(crate) fn default_daemon_shutdown_join_timeout_seconds() -> u64 {
    30
}

/// Keep service operations bounded.
pub(crate) fn default_service_command_timeout_seconds() -> u64 {
    15
}

/// Avoid hammering system daemons on transient failures.
pub(crate) fn default_service_command_retry_delay_ms() -> u64 {
    300
}

/// Avoid hardcoded sleep intervals during service command polling.
pub(crate) fn default_service_command_poll_interval_ms() -> u64 {
    50
}

/// Snapshots may require privileges and are not universally available; opt-in avoids surprises.
pub(crate) fn default_source_snapshots_enabled() -> bool {
    false
}

/// Keep OS-level snapshot operations bounded so backup cycles cannot hang indefinitely.
pub(crate) fn default_source_snapshot_timeout_seconds() -> u64 {
    20
}

/// Replication is opt-in per destination via `replicate_to`, but can be disabled globally for safety.
pub(crate) fn default_replication_enabled() -> bool {
    true
}

/// Replicas should usually reflect the same visible version set as the primary store.
pub(crate) fn default_replication_mirror_manifests() -> bool {
    true
}

/// Prevent accidental large deletions when destination mappings are misconfigured.
pub(crate) fn default_replication_max_manifest_deletes_per_cycle() -> usize {
    500
}

/// Keep a safety baseline when a watched path suddenly shrinks dramatically.
pub(crate) fn default_large_deletion_keep_extra_threshold_ratio() -> f64 {
    0.5
}
