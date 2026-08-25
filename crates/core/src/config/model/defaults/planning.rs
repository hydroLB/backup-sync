/// Balance integrity validation with IO cost.
pub(crate) fn default_hash_check_interval() -> u32 {
    5
}

/// Keep backup cycles bounded for predictable performance.
pub(crate) fn default_max_plan_items() -> usize {
    20_000
}

/// Prevent scans from hanging indefinitely on slow filesystems.
pub(crate) fn default_scan_timeout_seconds() -> u64 {
    300
}

/// Reduce reallocations on large scans.
pub(crate) fn default_scan_capacity_multiplier() -> usize {
    16
}
