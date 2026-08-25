/// Keep hashing throughput balanced with memory usage.
pub(crate) fn default_hash_buffer_bytes() -> usize {
    64 * 1024
}

/// Avoid hung hashing operations on problematic filesystems.
pub(crate) fn default_hash_timeout_seconds() -> u64 {
    30
}
