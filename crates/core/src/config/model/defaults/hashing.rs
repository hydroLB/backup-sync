/// Purpose: Supplies the default hash buffer size used during hashing.
///
/// Inputs: none.
/// Outputs: the hash buffer byte size.
/// Ties to: hash file IO chunk sizing.
/// Side effects: None.
/// Why: keep hashing throughput balanced with memory usage.
pub(crate) fn default_hash_buffer_bytes() -> usize {
    64 * 1024
}

/// Purpose: Supplies the default hash timeout in seconds.
///
/// Inputs: none.
/// Outputs: the hash timeout in seconds.
/// Ties to: hashing timeouts across planning and verification.
/// Side effects: None.
/// Why: avoid hung hashing operations on problematic filesystems.
pub(crate) fn default_hash_timeout_seconds() -> u64 {
    30
}
