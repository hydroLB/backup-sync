/// Purpose: Holds validation thresholds for config guardrails.
///
/// Inputs: set by defaults or tests.
/// Outputs: a collection of numeric thresholds.
/// Ties to: config validation and policy enforcement.
/// Side effects: None.
/// Why: centralize limits to make tuning and audits straightforward.
#[derive(Debug, Clone)]
pub struct ValidationLimits {
    pub min_interval_seconds: u64,
    pub max_interval_seconds: u64,
    pub min_verify_interval_seconds: u64,
    pub max_verify_interval_seconds: u64,
    pub max_backups_per_file: usize,
    pub max_parallel_copies: usize,
    pub max_copy_buffer_bytes: usize,
    pub max_copy_timeout_seconds: u64,
    pub max_free_space_safety_buffer_bytes: u64,
    pub max_prune_interval_cycles: u64,
    pub max_force_full_scan_interval_cycles: u64,
    pub max_watcher_debounce_seconds: u64,
    pub max_recent_activity_cap: usize,
    pub max_retry_delays: usize,
    pub max_retry_delay_ms: u64,
    pub max_retry_jitter_pct: f64,
    pub max_hash_buffer_bytes: usize,
    pub max_hash_timeout_seconds: u64,
    pub max_ipc_timeout_seconds: u64,
    pub max_scan_timeout_seconds: u64,
    pub max_service_command_timeout_seconds: u64,
    pub max_service_command_retry_delay_ms: u64,
    pub max_service_command_poll_interval_ms: u64,
    pub max_source_snapshot_timeout_seconds: u64,
    pub max_tray_tooltip_refresh_seconds: u64,
    pub max_scan_capacity_multiplier: usize,
    pub max_log_tail_lines: usize,
    pub max_simulation_sample_limit: usize,
    pub max_replication_max_manifest_deletes_per_cycle: usize,
    pub max_blob_encryption_chunk_bytes: usize,
    pub max_blob_compression_chunk_bytes: usize,
    pub min_blob_compression_zstd_level: i32,
    pub max_blob_compression_zstd_level: i32,
    pub max_scrub_sample_blobs: usize,
    pub max_scrub_sample_versions_per_source: usize,
    pub max_ignore_patterns: usize,
    pub max_watched: usize,
}

impl Default for ValidationLimits {
    /// Purpose: Builds the default validation thresholds for config guardrails.
    ///
    /// Inputs: none.
    /// Outputs: a populated `ValidationLimits` instance.
    /// Ties to: config validation and guardrail enforcement.
    /// Side effects: None.
    /// Why: centralize limits so tuning is consistent and discoverable.
    fn default() -> Self {
        Self {
            min_interval_seconds: 5,
            max_interval_seconds: 86400 * 30,
            min_verify_interval_seconds: 60,
            max_verify_interval_seconds: 86400 * 365,
            max_backups_per_file: 1000,
            max_parallel_copies: 64,
            max_copy_buffer_bytes: 8 * 1024 * 1024,
            max_copy_timeout_seconds: 3600,
            max_free_space_safety_buffer_bytes: 10 * 1024 * 1024 * 1024,
            max_prune_interval_cycles: 1000,
            max_force_full_scan_interval_cycles: 100_000,
            max_watcher_debounce_seconds: 60,
            max_recent_activity_cap: 1000,
            max_retry_delays: 10,
            max_retry_delay_ms: 30_000,
            max_retry_jitter_pct: 1.0,
            max_hash_buffer_bytes: 8 * 1024 * 1024,
            max_hash_timeout_seconds: 3600,
            max_ipc_timeout_seconds: 60,
            max_scan_timeout_seconds: 3600,
            max_service_command_timeout_seconds: 300,
            max_service_command_retry_delay_ms: 60_000,
            max_service_command_poll_interval_ms: 5000,
            max_source_snapshot_timeout_seconds: 300,
            max_tray_tooltip_refresh_seconds: 3600,
            max_scan_capacity_multiplier: 128,
            max_log_tail_lines: 5000,
            max_simulation_sample_limit: 1000,
            max_replication_max_manifest_deletes_per_cycle: 1_000_000,
            max_blob_encryption_chunk_bytes: 8 * 1024 * 1024,
            max_blob_compression_chunk_bytes: 8 * 1024 * 1024,
            min_blob_compression_zstd_level: 1,
            max_blob_compression_zstd_level: 22,
            max_scrub_sample_blobs: 50_000,
            max_scrub_sample_versions_per_source: 1000,
            max_ignore_patterns: 200,
            max_watched: 500,
        }
    }
}
