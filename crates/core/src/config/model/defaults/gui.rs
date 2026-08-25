/// Keep tray refresh timing tunable via config.
pub(crate) fn default_tray_tooltip_refresh_seconds() -> u64 {
    10
}

/// Avoid running expensive health probes too frequently.
pub(crate) fn default_tray_full_check_min_interval_seconds() -> u64 {
    20
}

/// Keep tray status checks responsive.
pub(crate) fn default_tray_full_check_timeout_seconds() -> u64 {
    8
}

/// Keep low-space warning policy configurable.
pub(crate) fn default_tray_low_space_warning_bytes() -> u64 {
    2 * 1024 * 1024 * 1024
}

/// Keep log payload sizes bounded.
pub(crate) fn default_log_tail_lines() -> usize {
    200
}

/// Keep log tail read behavior configurable without hardcoded buffers.
pub(crate) fn default_log_tail_read_chunk_bytes() -> usize {
    8192
}

/// Bound log tail latency and memory usage for large log files.
pub(crate) fn default_log_tail_max_bytes() -> u64 {
    512 * 1024
}

/// Keep simulation previews concise.
pub(crate) fn default_simulation_sample_limit() -> usize {
    10
}

/// Keep first-run checks responsive even when snapshot tooling is slow.
pub(crate) fn default_hardening_snapshot_probe_timeout_seconds() -> u64 {
    5
}

/// Avoid duplicate work from rapid close/reopen interactions.
pub(crate) fn default_gui_close_final_backup_debounce_ms() -> u64 {
    300
}

/// Keep close-final backup scheduling bounded and predictable.
pub(crate) fn default_gui_close_final_backup_reset_delay_seconds() -> u64 {
    2
}

/// Quitting should not hang indefinitely on best-effort backup.
pub(crate) fn default_gui_quit_final_backup_timeout_seconds() -> u64 {
    3
}

/// Keep quit responsive when service management is slow.
pub(crate) fn default_gui_daemon_stop_timeout_seconds() -> u64 {
    3
}

/// Ensure explicit quit always terminates even if background tasks misbehave.
pub(crate) fn default_gui_quit_force_exit_timeout_seconds() -> u64 {
    8
}
