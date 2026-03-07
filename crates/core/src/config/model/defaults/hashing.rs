/// Summary: Supplies the default hash buffer size used during hashing.
///
/// Inputs: none.
///
/// Outputs: the hash buffer byte size.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: hash file IO chunk sizing.
///
/// Why this exists: keep hashing throughput balanced with memory usage.
pub(crate) fn default_hash_buffer_bytes() -> usize {
    64 * 1024
}

/// Summary: Supplies the default hash timeout in seconds.
///
/// Inputs: none.
///
/// Outputs: the hash timeout in seconds.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: hashing timeouts across planning and verification.
///
/// Why this exists: avoid hung hashing operations on problematic filesystems.
pub(crate) fn default_hash_timeout_seconds() -> u64 {
    30
}
