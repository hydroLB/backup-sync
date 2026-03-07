/// Summary: Supplies the default cadence for periodic hash checks on stable files.
///
/// Inputs: none.
///
/// Outputs: the number of stable cycles between hash checks.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: the change detector to avoid frequent rehashing.
///
/// Why this exists: balance integrity validation with IO cost.
pub(crate) fn default_hash_check_interval() -> u32 {
    5
}

/// Summary: Supplies the default maximum number of plan items per cycle.
///
/// Inputs: none.
///
/// Outputs: the maximum allowed planned items per cycle.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: planning limits to prevent runaway IO.
///
/// Why this exists: keep backup cycles bounded for predictable performance.
pub(crate) fn default_max_plan_items() -> usize {
    20_000
}

/// Summary: Supplies the default scan timeout in seconds.
///
/// Inputs: none.
///
/// Outputs: the scan timeout in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: filesystem scan timeouts.
///
/// Why this exists: prevent scans from hanging indefinitely on slow filesystems.
pub(crate) fn default_scan_timeout_seconds() -> u64 {
    300
}

/// Summary: Supplies the default scan capacity multiplier for scan result preallocation.
///
/// Inputs: none.
///
/// Outputs: a multiplier applied to watched path count.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: scan collection vector sizing.
///
/// Why this exists: reduce reallocations on large scans.
pub(crate) fn default_scan_capacity_multiplier() -> usize {
    16
}
