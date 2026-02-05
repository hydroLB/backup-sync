use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeTuning {
    #[serde(default = "crate::config::model::defaults::default_prune_interval_cycles")]
    pub prune_interval_cycles: u64,
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
    #[serde(default = "crate::config::model::defaults::default_log_tail_lines")]
    pub log_tail_lines: usize,
    #[serde(default = "crate::config::model::defaults::default_simulation_sample_limit")]
    pub simulation_sample_limit: usize,
    #[serde(default)]
    pub gui_start_hidden: bool,
}

impl Default for RuntimeTuning {
    /// Purpose: Builds a baseline runtime tuning profile for daemon scheduling.
    ///
    /// Inputs: the default functions in `config::model::defaults`.
    /// Outputs: a fully populated runtime tuning profile.
    /// Ties to: daemon cycle pruning, verification, and watcher debounce.
    /// Side effects: None.
    /// Why: centralize runtime knobs so defaults stay aligned across the codebase.
    fn default() -> Self {
        Self {
            prune_interval_cycles: crate::config::model::defaults::default_prune_interval_cycles(),
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
            log_tail_lines: crate::config::model::defaults::default_log_tail_lines(),
            simulation_sample_limit:
                crate::config::model::defaults::default_simulation_sample_limit(),
            gui_start_hidden: false,
        }
    }
}
