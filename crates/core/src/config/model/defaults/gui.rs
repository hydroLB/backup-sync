/// Purpose: Supplies the default tray tooltip refresh interval in seconds.
///
/// Inputs: None.
/// Outputs: The tray tooltip refresh interval in seconds.
/// Ties to: GUI tray tooltip refresh loops.
/// Side effects: None.
/// Why: Keep tray refresh timing tunable via config.
pub(crate) fn default_tray_tooltip_refresh_seconds() -> u64 {
    10
}

/// Purpose: Supplies the default log tail line count for GUI log viewing.
///
/// Inputs: none.
/// Outputs: the default number of lines.
/// Ties to: GUI log tail responses.
/// Side effects: None.
/// Why: keep log payload sizes bounded.
pub(crate) fn default_log_tail_lines() -> usize {
    200
}

/// Purpose: Supplies the default simulation sample limit for GUI preview output.
///
/// Inputs: none.
/// Outputs: the maximum number of sample items.
/// Ties to: GUI simulation responses.
/// Side effects: None.
/// Why: keep simulation previews concise.
pub(crate) fn default_simulation_sample_limit() -> usize {
    10
}
