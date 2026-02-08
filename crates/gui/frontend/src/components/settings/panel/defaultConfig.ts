import { Config } from '../types';

/**
 * Summary: Build the default GUI configuration object used during first-run initialization.
 *
 * Inputs: None.
 * Outputs: A fresh `Config` object with safe defaults and runtime tuning values.
 * Side effects: None.
 * Error handling: None.
 * Ties to other methods: Passed into `useSettingsState` by `SettingsPanel` as the initial config seed.
 * Why this exists: Keep the default config definition centralized and avoid bloating the panel component.
 */
export function buildDefaultConfig(): Config {
  return {
    backup_root: '',
    interval_seconds: 30 * 60,
    max_backups_per_file: 3,
    skip_hidden: true,
    ignore_patterns: [],
    max_parallel_copies: 2,
    max_bytes_per_second: null,
    min_free_space_bytes: null,
    hashing: {
      buffer_bytes: 64 * 1024,
      timeout_seconds: 30,
    },
    execution: {
      copy_buffer_bytes: 64 * 1024,
      copy_timeout_seconds: 300,
      free_space_safety_buffer_bytes: 10 * 1024 * 1024,
      recent_activity_cap: 50,
      retry_delays_ms: [100, 200, 400, 800],
      retry_jitter_pct: 0.2,
    },
    planning: {
      hash_check_interval: 5,
      max_plan_items: 20_000,
      scan_timeout_seconds: 300,
      scan_capacity_multiplier: 16,
    },
    runtime: {
      prune_interval_cycles: 10,
      verify_interval_seconds: 24 * 3600,
      scrub_full_interval_seconds: 7 * 24 * 3600,
      scrub_sample_blobs: 200,
      scrub_sample_versions_per_source: 2,
      watcher_debounce_seconds: 2,
      ipc_timeout_seconds: 5,
      service_command_timeout_seconds: 15,
      service_command_retry_delay_ms: 300,
      service_command_poll_interval_ms: 50,
      source_snapshots_enabled: false,
      source_snapshot_timeout_seconds: 20,
      tray_tooltip_refresh_seconds: 10,
      log_tail_lines: 200,
      simulation_sample_limit: 10,
    },
    safe_mode: false,
    watched: [],
    destinations: [{ id: 'default', label: 'Primary', path: '', max_backups_per_file: null }],
  };
}
