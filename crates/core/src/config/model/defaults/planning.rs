/// Purpose: Supplies the default cadence for periodic hash checks on stable files.
///
/// Inputs: none.
/// Outputs: the number of stable cycles between hash checks.
/// Ties to: the change detector to avoid frequent rehashing.
/// Side effects: None.
/// Why: balance integrity validation with IO cost.
pub(crate) fn default_hash_check_interval() -> u32 {
    5
}

/// Purpose: Supplies the default maximum number of plan items per cycle.
///
/// Inputs: none.
/// Outputs: the maximum allowed planned items per cycle.
/// Ties to: planning limits to prevent runaway IO.
/// Side effects: None.
/// Why: keep backup cycles bounded for predictable performance.
pub(crate) fn default_max_plan_items() -> usize {
    20_000
}

/// Purpose: Supplies the default scan timeout in seconds.
///
/// Inputs: none.
/// Outputs: the scan timeout in seconds.
/// Ties to: filesystem scan timeouts.
/// Side effects: None.
/// Why: prevent scans from hanging indefinitely on slow filesystems.
pub(crate) fn default_scan_timeout_seconds() -> u64 {
    300
}

/// Purpose: Supplies the default scan capacity multiplier for scan result preallocation.
///
/// Inputs: none.
/// Outputs: a multiplier applied to watched path count.
/// Ties to: scan collection vector sizing.
/// Side effects: None.
/// Why: reduce reallocations on large scans.
pub(crate) fn default_scan_capacity_multiplier() -> usize {
    16
}
