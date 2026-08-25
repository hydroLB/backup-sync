use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeTuning {
    #[serde(default = "crate::config::model::defaults::default_prune_interval_cycles")]
    pub prune_interval_cycles: u64,
    #[serde(default = "crate::config::model::defaults::default_force_full_scan_interval_cycles")]
    pub force_full_scan_interval_cycles: u64,
    #[serde(default = "crate::config::model::defaults::default_verify_interval_seconds")]
    pub verify_interval_seconds: u64,
    #[serde(default = "crate::config::model::defaults::default_scrub_full_interval_seconds")]
    pub scrub_full_interval_seconds: u64,
    #[serde(default = "crate::config::model::defaults::default_scrub_sample_blobs")]
    pub scrub_sample_blobs: usize,
    #[serde(default = "crate::config::model::defaults::default_scrub_sample_versions_per_source")]
    pub scrub_sample_versions_per_source: usize,
    #[serde(default = "crate::config::model::defaults::default_watcher_debounce_seconds")]
    pub watcher_debounce_seconds: u64,
    #[serde(default = "crate::config::model::defaults::default_ipc_timeout_seconds")]
    pub ipc_timeout_seconds: u64,
    #[serde(default = "crate::config::model::defaults::default_ipc_request_max_bytes")]
    pub ipc_request_max_bytes: usize,
    #[serde(default = "crate::config::model::defaults::default_ipc_response_max_bytes")]
    pub ipc_response_max_bytes: usize,
    #[serde(default = "crate::config::model::defaults::default_ipc_read_chunk_bytes")]
    pub ipc_read_chunk_bytes: usize,
    #[serde(
        default = "crate::config::model::defaults::default_daemon_shutdown_join_timeout_seconds"
    )]
    pub daemon_shutdown_join_timeout_seconds: u64,
    #[serde(default = "crate::config::model::defaults::default_service_command_timeout_seconds")]
    pub service_command_timeout_seconds: u64,
    #[serde(default = "crate::config::model::defaults::default_service_command_retry_delay_ms")]
    pub service_command_retry_delay_ms: u64,
    #[serde(default = "crate::config::model::defaults::default_service_command_poll_interval_ms")]
    pub service_command_poll_interval_ms: u64,
    #[serde(default = "crate::config::model::defaults::default_source_snapshots_enabled")]
    pub source_snapshots_enabled: bool,
    #[serde(default = "crate::config::model::defaults::default_source_snapshot_timeout_seconds")]
    pub source_snapshot_timeout_seconds: u64,
    #[serde(default = "crate::config::model::defaults::default_tray_tooltip_refresh_seconds")]
    pub tray_tooltip_refresh_seconds: u64,
    #[serde(
        default = "crate::config::model::defaults::default_tray_full_check_min_interval_seconds"
    )]
    pub tray_full_check_min_interval_seconds: u64,
    #[serde(default = "crate::config::model::defaults::default_tray_full_check_timeout_seconds")]
    pub tray_full_check_timeout_seconds: u64,
    #[serde(default = "crate::config::model::defaults::default_tray_low_space_warning_bytes")]
    pub tray_low_space_warning_bytes: u64,
    #[serde(default = "crate::config::model::defaults::default_log_tail_lines")]
    pub log_tail_lines: usize,
    #[serde(default = "crate::config::model::defaults::default_log_tail_read_chunk_bytes")]
    pub log_tail_read_chunk_bytes: usize,
    #[serde(default = "crate::config::model::defaults::default_log_tail_max_bytes")]
    pub log_tail_max_bytes: u64,
    #[serde(default = "crate::config::model::defaults::default_simulation_sample_limit")]
    pub simulation_sample_limit: usize,
    #[serde(
        default = "crate::config::model::defaults::default_hardening_snapshot_probe_timeout_seconds"
    )]
    pub hardening_snapshot_probe_timeout_seconds: u64,
    #[serde(
        default = "crate::config::model::defaults::default_gui_close_final_backup_debounce_ms"
    )]
    pub gui_close_final_backup_debounce_ms: u64,
    #[serde(
        default = "crate::config::model::defaults::default_gui_close_final_backup_reset_delay_seconds"
    )]
    pub gui_close_final_backup_reset_delay_seconds: u64,
    #[serde(
        default = "crate::config::model::defaults::default_gui_quit_final_backup_timeout_seconds"
    )]
    pub gui_quit_final_backup_timeout_seconds: u64,
    #[serde(default = "crate::config::model::defaults::default_gui_daemon_stop_timeout_seconds")]
    pub gui_daemon_stop_timeout_seconds: u64,
    #[serde(
        default = "crate::config::model::defaults::default_gui_quit_force_exit_timeout_seconds"
    )]
    pub gui_quit_force_exit_timeout_seconds: u64,
    #[serde(default = "crate::config::model::defaults::default_replication_enabled")]
    pub replication_enabled: bool,
    #[serde(default = "crate::config::model::defaults::default_replication_mirror_manifests")]
    pub replication_mirror_manifests: bool,
    #[serde(
        default = "crate::config::model::defaults::default_replication_max_manifest_deletes_per_cycle"
    )]
    pub replication_max_manifest_deletes_per_cycle: usize,
    #[serde(
        default = "crate::config::model::defaults::default_large_deletion_keep_extra_threshold_ratio"
    )]
    pub large_deletion_keep_extra_threshold_ratio: f64,
    #[serde(default)]
    pub gui_start_hidden: bool,
}

impl Default for RuntimeTuning {
    /// Centralize runtime knobs so defaults stay aligned across the codebase.
    fn default() -> Self {
        Self {
            prune_interval_cycles: crate::config::model::defaults::default_prune_interval_cycles(),
            force_full_scan_interval_cycles:
                crate::config::model::defaults::default_force_full_scan_interval_cycles(),
            verify_interval_seconds:
                crate::config::model::defaults::default_verify_interval_seconds(),
            scrub_full_interval_seconds:
                crate::config::model::defaults::default_scrub_full_interval_seconds(),
            scrub_sample_blobs: crate::config::model::defaults::default_scrub_sample_blobs(),
            scrub_sample_versions_per_source:
                crate::config::model::defaults::default_scrub_sample_versions_per_source(),
            watcher_debounce_seconds:
                crate::config::model::defaults::default_watcher_debounce_seconds(),
            ipc_timeout_seconds: crate::config::model::defaults::default_ipc_timeout_seconds(),
            ipc_request_max_bytes: crate::config::model::defaults::default_ipc_request_max_bytes(),
            ipc_response_max_bytes: crate::config::model::defaults::default_ipc_response_max_bytes(
            ),
            ipc_read_chunk_bytes: crate::config::model::defaults::default_ipc_read_chunk_bytes(),
            daemon_shutdown_join_timeout_seconds:
                crate::config::model::defaults::default_daemon_shutdown_join_timeout_seconds(),
            service_command_timeout_seconds:
                crate::config::model::defaults::default_service_command_timeout_seconds(),
            service_command_retry_delay_ms:
                crate::config::model::defaults::default_service_command_retry_delay_ms(),
            service_command_poll_interval_ms:
                crate::config::model::defaults::default_service_command_poll_interval_ms(),
            source_snapshots_enabled:
                crate::config::model::defaults::default_source_snapshots_enabled(),
            source_snapshot_timeout_seconds:
                crate::config::model::defaults::default_source_snapshot_timeout_seconds(),
            tray_tooltip_refresh_seconds:
                crate::config::model::defaults::default_tray_tooltip_refresh_seconds(),
            tray_full_check_min_interval_seconds:
                crate::config::model::defaults::default_tray_full_check_min_interval_seconds(),
            tray_full_check_timeout_seconds:
                crate::config::model::defaults::default_tray_full_check_timeout_seconds(),
            tray_low_space_warning_bytes:
                crate::config::model::defaults::default_tray_low_space_warning_bytes(),
            log_tail_lines: crate::config::model::defaults::default_log_tail_lines(),
            log_tail_read_chunk_bytes:
                crate::config::model::defaults::default_log_tail_read_chunk_bytes(),
            log_tail_max_bytes: crate::config::model::defaults::default_log_tail_max_bytes(),
            simulation_sample_limit:
                crate::config::model::defaults::default_simulation_sample_limit(),
            hardening_snapshot_probe_timeout_seconds:
                crate::config::model::defaults::default_hardening_snapshot_probe_timeout_seconds(),
            gui_close_final_backup_debounce_ms:
                crate::config::model::defaults::default_gui_close_final_backup_debounce_ms(),
            gui_close_final_backup_reset_delay_seconds:
                crate::config::model::defaults::default_gui_close_final_backup_reset_delay_seconds(),
            gui_quit_final_backup_timeout_seconds:
                crate::config::model::defaults::default_gui_quit_final_backup_timeout_seconds(),
            gui_daemon_stop_timeout_seconds:
                crate::config::model::defaults::default_gui_daemon_stop_timeout_seconds(),
            gui_quit_force_exit_timeout_seconds:
                crate::config::model::defaults::default_gui_quit_force_exit_timeout_seconds(),
            replication_enabled: crate::config::model::defaults::default_replication_enabled(),
            replication_mirror_manifests:
                crate::config::model::defaults::default_replication_mirror_manifests(),
            replication_max_manifest_deletes_per_cycle:
                crate::config::model::defaults::default_replication_max_manifest_deletes_per_cycle(),
            large_deletion_keep_extra_threshold_ratio:
                crate::config::model::defaults::default_large_deletion_keep_extra_threshold_ratio(),
            gui_start_hidden: false,
        }
    }
}
