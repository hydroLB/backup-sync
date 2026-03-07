/// Summary: Supplies the default tray tooltip refresh interval in seconds.
///
/// Inputs: None.
///
/// Outputs: The tray tooltip refresh interval in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI tray tooltip refresh loops.
///
/// Why this exists: Keep tray refresh timing tunable via config.
pub(crate) fn default_tray_tooltip_refresh_seconds() -> u64 {
    10
}

/// Summary: Supplies the minimum interval between expensive tray health refreshes.
///
/// Inputs: none.
///
/// Outputs: the minimum refresh interval in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: tray health cache refresh policy.
///
/// Why this exists: avoid running expensive health probes too frequently.
pub(crate) fn default_tray_full_check_min_interval_seconds() -> u64 {
    20
}

/// Summary: Supplies the timeout for tray health refresh probes.
///
/// Inputs: none.
///
/// Outputs: the refresh timeout in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: tray health probe `tokio::time::timeout` bounds.
///
/// Why this exists: keep tray status checks responsive.
pub(crate) fn default_tray_full_check_timeout_seconds() -> u64 {
    8
}

/// Summary: Supplies the free-space warning threshold used in tray status.
///
/// Inputs: none.
///
/// Outputs: a byte threshold for low-space warnings.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: tray severity and low-space indicator rendering.
///
/// Why this exists: keep low-space warning policy configurable.
pub(crate) fn default_tray_low_space_warning_bytes() -> u64 {
    2 * 1024 * 1024 * 1024
}

/// Summary: Supplies the default log tail line count for GUI log viewing.
///
/// Inputs: none.
///
/// Outputs: the default number of lines.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI log tail responses.
///
/// Why this exists: keep log payload sizes bounded.
pub(crate) fn default_log_tail_lines() -> usize {
    200
}

/// Summary: Supplies the chunk size for reverse tail reads of log files.
///
/// Inputs: none.
///
/// Outputs: log tail read chunk size in bytes.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `logs_api::read_tail_from_file`.
///
/// Why this exists: keep log tail read behavior configurable without hardcoded buffers.
pub(crate) fn default_log_tail_read_chunk_bytes() -> usize {
    8192
}

/// Summary: Supplies the max bytes scanned while reading log tail content.
///
/// Inputs: none.
///
/// Outputs: max log bytes scanned per tail request.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `logs_api::read_tail_from_file`.
///
/// Why this exists: bound log tail latency and memory usage for large log files.
pub(crate) fn default_log_tail_max_bytes() -> u64 {
    512 * 1024
}

/// Summary: Supplies the default simulation sample limit for GUI preview output.
///
/// Inputs: none.
///
/// Outputs: the maximum number of sample items.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI simulation responses.
///
/// Why this exists: keep simulation previews concise.
pub(crate) fn default_simulation_sample_limit() -> usize {
    10
}

/// Summary: Supplies the max timeout used when probing snapshot support in hardening checks.
///
/// Inputs: none.
///
/// Outputs: timeout cap in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI hardening snapshot probe flow.
///
/// Why this exists: keep first-run checks responsive even when snapshot tooling is slow.
pub(crate) fn default_hardening_snapshot_probe_timeout_seconds() -> u64 {
    5
}

/// Summary: Supplies the debounce delay before triggering close-final backup work.
///
/// Inputs: none.
///
/// Outputs: debounce duration in milliseconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI close-to-tray backup trigger flow.
///
/// Why this exists: avoid duplicate work from rapid close/reopen interactions.
pub(crate) fn default_gui_close_final_backup_debounce_ms() -> u64 {
    300
}

/// Summary: Supplies the cooldown period before allowing another close-final backup trigger.
///
/// Inputs: none.
///
/// Outputs: cooldown duration in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI close-to-tray backup trigger guard.
///
/// Why this exists: keep close-final backup scheduling bounded and predictable.
pub(crate) fn default_gui_close_final_backup_reset_delay_seconds() -> u64 {
    2
}

/// Summary: Supplies the timeout for the final backup attempt during quit.
///
/// Inputs: none.
///
/// Outputs: timeout in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: quit sequence final backup flow.
///
/// Why this exists: quitting should not hang indefinitely on best-effort backup.
pub(crate) fn default_gui_quit_final_backup_timeout_seconds() -> u64 {
    3
}

/// Summary: Supplies the timeout for best-effort daemon stop during GUI quit.
///
/// Inputs: none.
///
/// Outputs: timeout in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: macOS launchctl stop flow in GUI quit sequence.
///
/// Why this exists: keep quit responsive when service management is slow.
pub(crate) fn default_gui_daemon_stop_timeout_seconds() -> u64 {
    3
}

/// Summary: Supplies the force-exit guard timeout after user-confirmed quit.
///
/// Inputs: none.
///
/// Outputs: timeout in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: GUI quit sequence hard-stop fallback.
///
/// Why this exists: ensure explicit quit always terminates even if background tasks misbehave.
pub(crate) fn default_gui_quit_force_exit_timeout_seconds() -> u64 {
    8
}
