export type WatchedPath = {
  path: string;
  kind: 'File' | 'Directory';
  enabled: boolean;
  destination_id?: string;
  max_backups_per_file?: number | null;
};

export type Destination = {
  id: string;
  label?: string | null;
  path: string;
  max_backups_per_file?: number | null;
};

export type HashingTuning = {
  buffer_bytes: number;
  timeout_seconds: number;
};

export type ExecutionTuning = {
  copy_buffer_bytes: number;
  copy_timeout_seconds: number;
  free_space_safety_buffer_bytes: number;
  recent_activity_cap: number;
  retry_delays_ms: number[];
  retry_jitter_pct: number;
};

export type PlanningTuning = {
  hash_check_interval: number;
  max_plan_items: number;
  scan_timeout_seconds: number;
  scan_capacity_multiplier: number;
};

export type RuntimeTuning = {
  prune_interval_cycles: number;
  verify_interval_seconds: number;
  watcher_debounce_seconds: number;
  ipc_timeout_seconds: number;
  service_command_timeout_seconds: number;
  service_command_retry_delay_ms: number;
  service_command_poll_interval_ms: number;
  auth_unlock_seconds: number;
  tray_tooltip_refresh_seconds: number;
  log_tail_lines: number;
  simulation_sample_limit: number;
};

export type Config = {
  backup_root: string;
  interval_seconds: number;
  max_backups_per_file: number;
  skip_hidden: boolean;
  ignore_patterns: string[];
  max_parallel_copies: number;
  max_bytes_per_second: number | null;
  min_free_space_bytes: number | null;
  hashing: HashingTuning;
  execution: ExecutionTuning;
  planning: PlanningTuning;
  runtime: RuntimeTuning;
  safe_mode: boolean;
  watched: WatchedPath[];
  destinations: Destination[];
};
