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
