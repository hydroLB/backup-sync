import { Config } from '../domain/config';

export type StatusDto = {
  last_run_ts: number | null;
  last_files_backed_up: number;
  last_error: string | null;
  last_dirty_count: number;
  last_safety_warning?: SafetyWarningDto | null;
  uptime_secs: number | null;
  version: string | null;
  free_bytes: number | null;
  min_free_space_bytes?: number | null;
  last_verify_ts: number | null;
  last_verify_status: string | null;
  last_verify_issues: number | null;
  recent_activity: Array<{ path: string; bytes: number; ts: number }>;
  safe_mode?: boolean;
  destination_paused?: boolean;
  destination_pause_reason?: string | null;
  destination_unavailable_ids?: string[];
  destination_last_unavailable_ts?: number | null;
  destination_last_recovered_ts?: number | null;
  replication_last_run_ts?: number | null;
  replication_last_status?: string | null;
  replication_last_error?: string | null;
  replication_last_bytes_copied?: number;
  replication_last_blobs_copied?: number;
  replication_last_manifests_copied?: number;
  replication_last_manifests_deleted?: number;
  replication_last_pairs_ok?: number;
  replication_last_pairs_failed?: number;
  replication_last_targets_failed?: string[];
  destinations?: DestinationStatus[];
};

export type SafetyWarningDto = {
  ts: number;
  message: string;
  watched_path?: string | null;
  kept_version_id?: string | null;
};

export type DestinationStatus = {
  id: string;
  label?: string | null;
  path: string;
  reachable?: boolean;
  writable?: boolean;
  free_bytes: number | null;
  message?: string;
};

export type ServiceStatusDto = {
  installed: boolean;
  reachable: boolean;
  message: string;
  fix_command?: string;
  uptime_secs?: number | null;
  last_ipc_ts?: number | null;
};

export type SimulationResult = {
  items: number;
  bytes: number;
  sample?: string[];
  message?: string;
};

export type VerifyResult = {
  ok: number;
  bad: number;
  last_verify_ts?: number | null;
  last_verify_status?: string | null;
  last_verify_issues?: number | null;
};

export type VersionInfoDto = {
  id: string;
  created_at_unix: number;
};

export type FolderVersionsDto = {
  source_path: string;
  versions: VersionInfoDto[];
};

export type RestoreModeDto = 'in_place' | 'to_directory';

export type RestoreArgs = {
  source_path: string;
  version_id: string;
  mode: RestoreModeDto;
  target_dir?: string | null;
  /** Web showcase only: retain manifests newer than the selected recovery point. */
  keep_newer_versions?: boolean;
};

export type RestoreResultDto = {
  files_written: number;
  files_removed: number;
  dirs_created: number;
  newer_versions_removed?: number;
};

export type VersionFileInfoDto = {
  rel_path: string;
  len: number;
  mtime_unix: number;
  mtime_nanos: number;
  sha256: string;
};

export type ListVersionFilesArgs = {
  source_path: string;
  version_id: string;
  query?: string | null;
  limit?: number | null;
};

export type ListVersionFilesResultDto = {
  total_files: number;
  files: VersionFileInfoDto[];
};

export type RestoreFilesArgs = {
  source_path: string;
  version_id: string;
  rel_paths: string[];
  mode: RestoreModeDto;
  target_dir?: string | null;
};

export type DestinationCheck = { writable: boolean; free_bytes: number | null; message: string };

export type AccessProbe = {
  destination_writable: boolean;
  destination_message: string;
  watched_ok: string[];
  watched_missing: string[];
  watched_unwritable: string[];
};

/** Make hardening failures actionable and precise. */
export type HardeningWatchedIssue = {
  path: string;
  kind: string;
  issue: string;
};

/** Surface which destination is blocking scheduling. */
export type HardeningDestinationResult = {
  id: string;
  path: string;
  ok: boolean;
  free_bytes: number | null;
  required_free_bytes: number;
  message: string;
};

/** Allow optional validation of snapshot support before enabling scheduling. */
export type HardeningSnapshotResult = {
  checked: boolean;
  supported: boolean;
  message: string;
};

/** Scheduling should only be enabled once prerequisites are satisfied. */
export type HardeningReport = {
  ok: boolean;
  message: string;
  watched_ok: string[];
  watched_issues: HardeningWatchedIssue[];
  destinations: HardeningDestinationResult[];
  snapshots: HardeningSnapshotResult;
};

export type ConfigWithValidation = Config;
