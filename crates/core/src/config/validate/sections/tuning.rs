use crate::config::model::Config;
use crate::config::validate::ValidationLimits;
use anyhow::{bail, Result};

use super::{ensure_nonzero_u64, ensure_nonzero_usize};

/// Summary: Validates non-path configuration fields and tuning blocks.
///
/// Inputs: the config, validation limits, and a label prefix.
///
/// Outputs: `Ok(())` when all numeric and tuning invariants hold.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: config loading and backup runtime behavior.
///
/// Why this exists: keep the top-level validation function readable by grouping numeric checks.
pub(crate) fn validate_tuning(cfg: &Config, limits: &ValidationLimits, label: &str) -> Result<()> {
    validate_backup_root(cfg, label)?;
    validate_interval(cfg, limits, label)?;
    validate_execution_limits(cfg, limits, label)?;
    validate_hashing_limits(cfg, limits, label)?;
    validate_planning_limits(cfg, limits, label)?;
    validate_runtime_limits(cfg, limits, label)?;
    Ok(())
}

/// Summary: Validates required path settings that are structurally invalid when empty.
///
/// Inputs: the config and label prefix.
///
/// Outputs: `Ok(())` when required paths are present.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: destination and state file layout.
///
/// Why this exists: avoid running a cycle when basic config paths are missing.
fn validate_backup_root(cfg: &Config, label: &str) -> Result<()> {
    if cfg.backup_root.as_os_str().is_empty() {
        bail!("{label} backup_root is missing");
    }
    Ok(())
}

/// Summary: Validates scheduling-related fields.
///
/// Inputs: the config, limits, and label prefix.
///
/// Outputs: `Ok(())` when intervals are within guardrails.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon cycle scheduling.
///
/// Why this exists: prevent accidental busy loops or multi-month stalls.
fn validate_interval(cfg: &Config, limits: &ValidationLimits, label: &str) -> Result<()> {
    if cfg.interval_seconds < limits.min_interval_seconds {
        bail!(
            "{label} interval_seconds must be >= {} seconds to avoid busy looping",
            limits.min_interval_seconds
        );
    }
    if cfg.interval_seconds > limits.max_interval_seconds {
        bail!(
            "{label} interval_seconds too large (>{} seconds); pick a shorter schedule",
            limits.max_interval_seconds
        );
    }
    Ok(())
}

/// Summary: Validates top-level and execution tuning fields.
///
/// Inputs: the config, limits, and label prefix.
///
/// Outputs: `Ok(())` when execution tuning is within limits.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: `BackupExecutor` copy buffering, retry behavior, and state updates.
///
/// Why this exists: cap memory usage and enforce sane retry policies.
fn validate_execution_limits(cfg: &Config, limits: &ValidationLimits, label: &str) -> Result<()> {
    ensure_nonzero_usize(label, "max_backups_per_file", cfg.max_backups_per_file)?;
    if cfg.max_backups_per_file > limits.max_backups_per_file {
        bail!(
            "{label} max_backups_per_file too large (>{})",
            limits.max_backups_per_file
        );
    }
    ensure_nonzero_usize(label, "max_parallel_copies", cfg.max_parallel_copies)?;
    if cfg.max_parallel_copies > limits.max_parallel_copies {
        bail!(
            "{label} max_parallel_copies too high (>{}); reduce to avoid overwhelming the system",
            limits.max_parallel_copies
        );
    }
    if let Some(bps) = cfg.max_bytes_per_second {
        if bps == 0 {
            bail!("{label} max_bytes_per_second must be > 0 when set");
        }
    }
    if let Some(min_free) = cfg.min_free_space_bytes {
        if min_free == 0 {
            bail!("{label} min_free_space_bytes must be > 0 when set");
        }
    }

    ensure_nonzero_usize(
        label,
        "execution.copy_buffer_bytes",
        cfg.execution.copy_buffer_bytes,
    )?;
    if cfg.execution.copy_buffer_bytes > limits.max_copy_buffer_bytes {
        bail!(
            "{label} execution.copy_buffer_bytes too large (>{} bytes); reduce to limit memory usage",
            limits.max_copy_buffer_bytes
        );
    }

    ensure_nonzero_u64(
        label,
        "execution.copy_timeout_seconds",
        cfg.execution.copy_timeout_seconds,
    )?;
    if cfg.execution.copy_timeout_seconds > limits.max_copy_timeout_seconds {
        bail!(
            "{label} execution.copy_timeout_seconds too large (>{} seconds)",
            limits.max_copy_timeout_seconds
        );
    }
    if cfg.execution.free_space_safety_buffer_bytes > limits.max_free_space_safety_buffer_bytes {
        bail!(
            "{label} execution.free_space_safety_buffer_bytes too large (>{} bytes)",
            limits.max_free_space_safety_buffer_bytes
        );
    }

    ensure_nonzero_usize(
        label,
        "execution.recent_activity_cap",
        cfg.execution.recent_activity_cap,
    )?;
    if cfg.execution.recent_activity_cap > limits.max_recent_activity_cap {
        bail!(
            "{label} execution.recent_activity_cap too large (>{})",
            limits.max_recent_activity_cap
        );
    }

    if cfg.execution.retry_delays_ms.is_empty() {
        bail!("{label} execution.retry_delays_ms must contain at least one delay");
    }
    if cfg.execution.retry_delays_ms.len() > limits.max_retry_delays {
        bail!(
            "{label} execution.retry_delays_ms has too many entries (>{})",
            limits.max_retry_delays
        );
    }
    let mut last_delay = 0u64;
    for (idx, delay_ms) in cfg.execution.retry_delays_ms.iter().enumerate() {
        if *delay_ms == 0 {
            bail!("{label} execution.retry_delays_ms[{}] must be > 0", idx);
        }
        if *delay_ms > limits.max_retry_delay_ms {
            bail!(
                "{label} execution.retry_delays_ms[{}] too large (>{}ms)",
                idx,
                limits.max_retry_delay_ms
            );
        }
        if idx > 0 && *delay_ms < last_delay {
            bail!("{label} execution.retry_delays_ms must be non-decreasing to preserve backoff");
        }
        last_delay = *delay_ms;
    }
    if cfg.execution.retry_jitter_pct < 0.0 {
        bail!("{label} execution.retry_jitter_pct must be >= 0");
    }
    if cfg.execution.retry_jitter_pct > limits.max_retry_jitter_pct {
        bail!(
            "{label} execution.retry_jitter_pct too large (>{})",
            limits.max_retry_jitter_pct
        );
    }
    ensure_nonzero_u64(
        label,
        "execution.blocking_io_backoff_poll_interval_ms",
        cfg.execution.blocking_io_backoff_poll_interval_ms,
    )?;
    if cfg.execution.blocking_io_backoff_poll_interval_ms
        > limits.max_blocking_io_backoff_poll_interval_ms
    {
        bail!(
            "{label} execution.blocking_io_backoff_poll_interval_ms too large (>{}ms)",
            limits.max_blocking_io_backoff_poll_interval_ms
        );
    }
    Ok(())
}

/// Summary: Validates hashing tuning fields.
///
/// Inputs: the config, limits, and label prefix.
///
/// Outputs: `Ok(())` when hashing tuning is within limits.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: hashing in planning, execution, and verification.
///
/// Why this exists: bound hashing timeouts and memory usage.
fn validate_hashing_limits(cfg: &Config, limits: &ValidationLimits, label: &str) -> Result<()> {
    ensure_nonzero_usize(label, "hashing.buffer_bytes", cfg.hashing.buffer_bytes)?;
    if cfg.hashing.buffer_bytes > limits.max_hash_buffer_bytes {
        bail!(
            "{label} hashing.buffer_bytes too large (>{} bytes)",
            limits.max_hash_buffer_bytes
        );
    }
    ensure_nonzero_u64(
        label,
        "hashing.timeout_seconds",
        cfg.hashing.timeout_seconds,
    )?;
    if cfg.hashing.timeout_seconds > limits.max_hash_timeout_seconds {
        bail!(
            "{label} hashing.timeout_seconds too large (>{} seconds)",
            limits.max_hash_timeout_seconds
        );
    }
    Ok(())
}

/// Summary: Validates planning tuning fields.
///
/// Inputs: the config, limits, and label prefix.
///
/// Outputs: `Ok(())` when planning tuning is within limits.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: scan planning and guardrail enforcement.
///
/// Why this exists: prevent runaway planning or hung scans.
fn validate_planning_limits(cfg: &Config, limits: &ValidationLimits, label: &str) -> Result<()> {
    if cfg.planning.hash_check_interval == 0 {
        bail!("{label} planning.hash_check_interval must be > 0");
    }
    ensure_nonzero_usize(
        label,
        "planning.max_plan_items",
        cfg.planning.max_plan_items,
    )?;
    ensure_nonzero_u64(
        label,
        "planning.scan_timeout_seconds",
        cfg.planning.scan_timeout_seconds,
    )?;
    if cfg.planning.scan_timeout_seconds > limits.max_scan_timeout_seconds {
        bail!(
            "{label} planning.scan_timeout_seconds too large (>{} seconds)",
            limits.max_scan_timeout_seconds
        );
    }
    ensure_nonzero_usize(
        label,
        "planning.scan_capacity_multiplier",
        cfg.planning.scan_capacity_multiplier,
    )?;
    if cfg.planning.scan_capacity_multiplier > limits.max_scan_capacity_multiplier {
        bail!(
            "{label} planning.scan_capacity_multiplier too large (>{})",
            limits.max_scan_capacity_multiplier
        );
    }
    Ok(())
}

/// Summary: Validates runtime tuning fields.
///
/// Inputs: the config, limits, and label prefix.
///
/// Outputs: `Ok(())` when runtime tuning is within limits.
///
/// Side effects: None.
///
/// Error handling: Propagates contextual errors to the caller when operations fail.
///
/// Ties to other methods: daemon loop cadence and IPC behavior.
///
/// Why this exists: bound timeouts and keep daemon responsiveness predictable.
fn validate_runtime_limits(cfg: &Config, limits: &ValidationLimits, label: &str) -> Result<()> {
    ensure_nonzero_u64(
        label,
        "runtime.prune_interval_cycles",
        cfg.runtime.prune_interval_cycles,
    )?;
    if cfg.runtime.prune_interval_cycles > limits.max_prune_interval_cycles {
        bail!(
            "{label} runtime.prune_interval_cycles too large (>{})",
            limits.max_prune_interval_cycles
        );
    }

    ensure_nonzero_u64(
        label,
        "runtime.force_full_scan_interval_cycles",
        cfg.runtime.force_full_scan_interval_cycles,
    )?;
    if cfg.runtime.force_full_scan_interval_cycles > limits.max_force_full_scan_interval_cycles {
        bail!(
            "{label} runtime.force_full_scan_interval_cycles too large (>{})",
            limits.max_force_full_scan_interval_cycles
        );
    }

    if cfg.runtime.verify_interval_seconds < limits.min_verify_interval_seconds {
        bail!(
            "{label} runtime.verify_interval_seconds must be >= {}",
            limits.min_verify_interval_seconds
        );
    }
    if cfg.runtime.verify_interval_seconds > limits.max_verify_interval_seconds {
        bail!(
            "{label} runtime.verify_interval_seconds too large (>{})",
            limits.max_verify_interval_seconds
        );
    }
    ensure_nonzero_u64(
        label,
        "runtime.scrub_full_interval_seconds",
        cfg.runtime.scrub_full_interval_seconds,
    )?;
    if cfg.runtime.scrub_full_interval_seconds > limits.max_verify_interval_seconds {
        bail!(
            "{label} runtime.scrub_full_interval_seconds too large (>{})",
            limits.max_verify_interval_seconds
        );
    }
    ensure_nonzero_usize(
        label,
        "runtime.scrub_sample_blobs",
        cfg.runtime.scrub_sample_blobs,
    )?;
    if cfg.runtime.scrub_sample_blobs > limits.max_scrub_sample_blobs {
        bail!(
            "{label} runtime.scrub_sample_blobs too large (>{})",
            limits.max_scrub_sample_blobs
        );
    }
    ensure_nonzero_usize(
        label,
        "runtime.scrub_sample_versions_per_source",
        cfg.runtime.scrub_sample_versions_per_source,
    )?;
    if cfg.runtime.scrub_sample_versions_per_source > limits.max_scrub_sample_versions_per_source {
        bail!(
            "{label} runtime.scrub_sample_versions_per_source too large (>{})",
            limits.max_scrub_sample_versions_per_source
        );
    }
    ensure_nonzero_u64(
        label,
        "runtime.watcher_debounce_seconds",
        cfg.runtime.watcher_debounce_seconds,
    )?;
    if cfg.runtime.watcher_debounce_seconds > limits.max_watcher_debounce_seconds {
        bail!(
            "{label} runtime.watcher_debounce_seconds too large (>{})",
            limits.max_watcher_debounce_seconds
        );
    }

    ensure_nonzero_u64(
        label,
        "runtime.ipc_timeout_seconds",
        cfg.runtime.ipc_timeout_seconds,
    )?;
    if cfg.runtime.ipc_timeout_seconds > limits.max_ipc_timeout_seconds {
        bail!(
            "{label} runtime.ipc_timeout_seconds too large (>{} seconds)",
            limits.max_ipc_timeout_seconds
        );
    }
    ensure_nonzero_usize(
        label,
        "runtime.ipc_request_max_bytes",
        cfg.runtime.ipc_request_max_bytes,
    )?;
    if cfg.runtime.ipc_request_max_bytes > limits.max_ipc_request_max_bytes {
        bail!(
            "{label} runtime.ipc_request_max_bytes too large (>{} bytes)",
            limits.max_ipc_request_max_bytes
        );
    }
    ensure_nonzero_usize(
        label,
        "runtime.ipc_response_max_bytes",
        cfg.runtime.ipc_response_max_bytes,
    )?;
    if cfg.runtime.ipc_response_max_bytes > limits.max_ipc_response_max_bytes {
        bail!(
            "{label} runtime.ipc_response_max_bytes too large (>{} bytes)",
            limits.max_ipc_response_max_bytes
        );
    }
    ensure_nonzero_usize(
        label,
        "runtime.ipc_read_chunk_bytes",
        cfg.runtime.ipc_read_chunk_bytes,
    )?;
    if cfg.runtime.ipc_read_chunk_bytes > limits.max_ipc_read_chunk_bytes {
        bail!(
            "{label} runtime.ipc_read_chunk_bytes too large (>{} bytes)",
            limits.max_ipc_read_chunk_bytes
        );
    }
    ensure_nonzero_u64(
        label,
        "runtime.daemon_shutdown_join_timeout_seconds",
        cfg.runtime.daemon_shutdown_join_timeout_seconds,
    )?;
    if cfg.runtime.daemon_shutdown_join_timeout_seconds
        > limits.max_daemon_shutdown_join_timeout_seconds
    {
        bail!(
            "{label} runtime.daemon_shutdown_join_timeout_seconds too large (>{} seconds)",
            limits.max_daemon_shutdown_join_timeout_seconds
        );
    }

    ensure_nonzero_u64(
        label,
        "runtime.service_command_timeout_seconds",
        cfg.runtime.service_command_timeout_seconds,
    )?;
    if cfg.runtime.service_command_timeout_seconds > limits.max_service_command_timeout_seconds {
        bail!(
            "{label} runtime.service_command_timeout_seconds too large (>{} seconds)",
            limits.max_service_command_timeout_seconds
        );
    }
    ensure_nonzero_u64(
        label,
        "runtime.service_command_retry_delay_ms",
        cfg.runtime.service_command_retry_delay_ms,
    )?;
    if cfg.runtime.service_command_retry_delay_ms > limits.max_service_command_retry_delay_ms {
        bail!(
            "{label} runtime.service_command_retry_delay_ms too large (>{}ms)",
            limits.max_service_command_retry_delay_ms
        );
    }
    ensure_nonzero_u64(
        label,
        "runtime.service_command_poll_interval_ms",
        cfg.runtime.service_command_poll_interval_ms,
    )?;
    if cfg.runtime.service_command_poll_interval_ms > limits.max_service_command_poll_interval_ms {
        bail!(
            "{label} runtime.service_command_poll_interval_ms too large (>{}ms)",
            limits.max_service_command_poll_interval_ms
        );
    }

    ensure_nonzero_u64(
        label,
        "runtime.source_snapshot_timeout_seconds",
        cfg.runtime.source_snapshot_timeout_seconds,
    )?;
    if cfg.runtime.source_snapshot_timeout_seconds > limits.max_source_snapshot_timeout_seconds {
        bail!(
            "{label} runtime.source_snapshot_timeout_seconds too large (>{} seconds)",
            limits.max_source_snapshot_timeout_seconds
        );
    }

    ensure_nonzero_u64(
        label,
        "runtime.tray_tooltip_refresh_seconds",
        cfg.runtime.tray_tooltip_refresh_seconds,
    )?;
    if cfg.runtime.tray_tooltip_refresh_seconds > limits.max_tray_tooltip_refresh_seconds {
        bail!(
            "{label} runtime.tray_tooltip_refresh_seconds too large (>{} seconds)",
            limits.max_tray_tooltip_refresh_seconds
        );
    }
    ensure_nonzero_u64(
        label,
        "runtime.tray_full_check_min_interval_seconds",
        cfg.runtime.tray_full_check_min_interval_seconds,
    )?;
    if cfg.runtime.tray_full_check_min_interval_seconds
        > limits.max_tray_full_check_min_interval_seconds
    {
        bail!(
            "{label} runtime.tray_full_check_min_interval_seconds too large (>{} seconds)",
            limits.max_tray_full_check_min_interval_seconds
        );
    }
    ensure_nonzero_u64(
        label,
        "runtime.tray_full_check_timeout_seconds",
        cfg.runtime.tray_full_check_timeout_seconds,
    )?;
    if cfg.runtime.tray_full_check_timeout_seconds > limits.max_tray_full_check_timeout_seconds {
        bail!(
            "{label} runtime.tray_full_check_timeout_seconds too large (>{} seconds)",
            limits.max_tray_full_check_timeout_seconds
        );
    }
    ensure_nonzero_u64(
        label,
        "runtime.tray_low_space_warning_bytes",
        cfg.runtime.tray_low_space_warning_bytes,
    )?;
    if cfg.runtime.tray_low_space_warning_bytes > limits.max_tray_low_space_warning_bytes {
        bail!(
            "{label} runtime.tray_low_space_warning_bytes too large (>{} bytes)",
            limits.max_tray_low_space_warning_bytes
        );
    }

    ensure_nonzero_usize(label, "runtime.log_tail_lines", cfg.runtime.log_tail_lines)?;
    if cfg.runtime.log_tail_lines > limits.max_log_tail_lines {
        bail!(
            "{label} runtime.log_tail_lines too large (>{})",
            limits.max_log_tail_lines
        );
    }
    ensure_nonzero_usize(
        label,
        "runtime.log_tail_read_chunk_bytes",
        cfg.runtime.log_tail_read_chunk_bytes,
    )?;
    if cfg.runtime.log_tail_read_chunk_bytes > limits.max_log_tail_read_chunk_bytes {
        bail!(
            "{label} runtime.log_tail_read_chunk_bytes too large (>{} bytes)",
            limits.max_log_tail_read_chunk_bytes
        );
    }
    ensure_nonzero_u64(
        label,
        "runtime.log_tail_max_bytes",
        cfg.runtime.log_tail_max_bytes,
    )?;
    if cfg.runtime.log_tail_max_bytes > limits.max_log_tail_max_bytes {
        bail!(
            "{label} runtime.log_tail_max_bytes too large (>{} bytes)",
            limits.max_log_tail_max_bytes
        );
    }

    ensure_nonzero_usize(
        label,
        "runtime.simulation_sample_limit",
        cfg.runtime.simulation_sample_limit,
    )?;
    if cfg.runtime.simulation_sample_limit > limits.max_simulation_sample_limit {
        bail!(
            "{label} runtime.simulation_sample_limit too large (>{})",
            limits.max_simulation_sample_limit
        );
    }
    ensure_nonzero_u64(
        label,
        "runtime.hardening_snapshot_probe_timeout_seconds",
        cfg.runtime.hardening_snapshot_probe_timeout_seconds,
    )?;
    if cfg.runtime.hardening_snapshot_probe_timeout_seconds
        > limits.max_hardening_snapshot_probe_timeout_seconds
    {
        bail!(
            "{label} runtime.hardening_snapshot_probe_timeout_seconds too large (>{} seconds)",
            limits.max_hardening_snapshot_probe_timeout_seconds
        );
    }
    ensure_nonzero_u64(
        label,
        "runtime.gui_close_final_backup_debounce_ms",
        cfg.runtime.gui_close_final_backup_debounce_ms,
    )?;
    if cfg.runtime.gui_close_final_backup_debounce_ms
        > limits.max_gui_close_final_backup_debounce_ms
    {
        bail!(
            "{label} runtime.gui_close_final_backup_debounce_ms too large (>{}ms)",
            limits.max_gui_close_final_backup_debounce_ms
        );
    }
    ensure_nonzero_u64(
        label,
        "runtime.gui_close_final_backup_reset_delay_seconds",
        cfg.runtime.gui_close_final_backup_reset_delay_seconds,
    )?;
    if cfg.runtime.gui_close_final_backup_reset_delay_seconds
        > limits.max_gui_close_final_backup_reset_delay_seconds
    {
        bail!(
            "{label} runtime.gui_close_final_backup_reset_delay_seconds too large (>{} seconds)",
            limits.max_gui_close_final_backup_reset_delay_seconds
        );
    }
    ensure_nonzero_u64(
        label,
        "runtime.gui_quit_final_backup_timeout_seconds",
        cfg.runtime.gui_quit_final_backup_timeout_seconds,
    )?;
    if cfg.runtime.gui_quit_final_backup_timeout_seconds
        > limits.max_gui_quit_final_backup_timeout_seconds
    {
        bail!(
            "{label} runtime.gui_quit_final_backup_timeout_seconds too large (>{} seconds)",
            limits.max_gui_quit_final_backup_timeout_seconds
        );
    }
    ensure_nonzero_u64(
        label,
        "runtime.gui_daemon_stop_timeout_seconds",
        cfg.runtime.gui_daemon_stop_timeout_seconds,
    )?;
    if cfg.runtime.gui_daemon_stop_timeout_seconds > limits.max_gui_daemon_stop_timeout_seconds {
        bail!(
            "{label} runtime.gui_daemon_stop_timeout_seconds too large (>{} seconds)",
            limits.max_gui_daemon_stop_timeout_seconds
        );
    }
    ensure_nonzero_u64(
        label,
        "runtime.gui_quit_force_exit_timeout_seconds",
        cfg.runtime.gui_quit_force_exit_timeout_seconds,
    )?;
    if cfg.runtime.gui_quit_force_exit_timeout_seconds
        > limits.max_gui_quit_force_exit_timeout_seconds
    {
        bail!(
            "{label} runtime.gui_quit_force_exit_timeout_seconds too large (>{} seconds)",
            limits.max_gui_quit_force_exit_timeout_seconds
        );
    }

    if cfg.runtime.replication_max_manifest_deletes_per_cycle
        > limits.max_replication_max_manifest_deletes_per_cycle
    {
        bail!(
            "{label} runtime.replication_max_manifest_deletes_per_cycle too large (>{})",
            limits.max_replication_max_manifest_deletes_per_cycle
        );
    }

    let threshold = cfg.runtime.large_deletion_keep_extra_threshold_ratio;
    if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
        bail!(
            "{label} runtime.large_deletion_keep_extra_threshold_ratio must be within [0.0, 1.0]"
        );
    }
    Ok(())
}
