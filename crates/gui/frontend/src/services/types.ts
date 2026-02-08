import { Config } from '../components/settings/types';

/**
 * Purpose: Status payload returned by the backend.
 *
 * Inputs: deserialized from IPC responses.
 * Outputs: a typed status object for the UI.
 * Ties to: status panels and activity feeds.
 * Side effects: None.
 * Why: keep status data strongly typed in the frontend.
 */
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

/**
 * Purpose: Safety warning payload returned by the backend.
 *
 * Inputs: deserialized from IPC responses.
 * Outputs: a typed warning object for UI display.
 * Ties to: minimal warning banners and tray tooltip state.
 * Side effects: None.
 * Why: keep safety warnings strongly typed in the frontend.
 */
export type SafetyWarningDto = {
  ts: number;
  message: string;
  watched_path?: string | null;
  kept_version_id?: string | null;
};

/**
 * Purpose: Destination status payload returned by the backend.
 *
 * Inputs: deserialized from IPC responses.
 * Outputs: a typed destination status object.
 * Ties to: status panels and destination cards.
 * Side effects: None.
 * Why: keep destination metadata strongly typed in the frontend.
 */
export type DestinationStatus = {
  id: string;
  label?: string | null;
  path: string;
  reachable?: boolean;
  writable?: boolean;
  free_bytes: number | null;
  message?: string;
};

/**
 * Purpose: Service status payload returned by the backend.
 *
 * Inputs: deserialized from IPC responses.
 * Outputs: a typed service status object.
 * Ties to: service banners and diagnostics.
 * Side effects: None.
 * Why: keep service metadata strongly typed in the frontend.
 */
export type ServiceStatusDto = {
  installed: boolean;
  reachable: boolean;
  message: string;
  fix_command?: string;
  uptime_secs?: number | null;
  last_ipc_ts?: number | null;
};

/**
 * Purpose: Simulation result payload returned by the backend.
 *
 * Inputs: deserialized from IPC responses.
 * Outputs: a typed simulation result.
 * Ties to: simulation previews.
 * Side effects: None.
 * Why: keep simulation output strongly typed in the frontend.
 */
export type SimulationResult = {
  items: number;
  bytes: number;
  sample?: string[];
  message?: string;
};

/**
 * Purpose: Verify result payload returned by the backend.
 *
 * Inputs: deserialized from IPC responses.
 * Outputs: a typed verification result.
 * Ties to: verification status displays.
 * Side effects: None.
 * Why: keep verification output strongly typed in the frontend.
 */
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
};

export type RestoreResultDto = {
  files_written: number;
  files_removed: number;
  dirs_created: number;
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

/**
 * Purpose: Destination validation payload returned by the backend.
 *
 * Inputs: deserialized from IPC responses.
 * Outputs: a typed destination check result.
 * Ties to: destination pickers.
 * Side effects: None.
 * Why: keep destination validation output strongly typed in the frontend.
 */
export type DestinationCheck = { writable: boolean; free_bytes: number | null; message: string };

/**
 * Purpose: Access probe payload returned by the backend.
 *
 * Inputs: deserialized from IPC responses.
 * Outputs: a typed access probe structure.
 * Ties to: access tests and diagnostics views.
 * Side effects: None.
 * Why: keep access test results strongly typed in the frontend.
 */
export type AccessProbe = {
  destination_writable: boolean;
  destination_message: string;
  watched_ok: string[];
  watched_missing: string[];
  watched_unwritable: string[];
};

/**
 * Purpose: Watched-path issue returned by hardening checks.
 *
 * Inputs: Deserialized from IPC.
 * Outputs: A typed issue object suitable for UI display.
 * Ties to: First-run hardening wizard.
 * Side effects: None.
 * Why: Make hardening failures actionable and precise.
 */
export type HardeningWatchedIssue = {
  path: string;
  kind: string;
  issue: string;
};

/**
 * Purpose: Destination hardening result returned by hardening checks.
 *
 * Inputs: Deserialized from IPC.
 * Outputs: A typed destination result.
 * Ties to: First-run hardening wizard.
 * Side effects: None.
 * Why: Surface which destination is blocking scheduling.
 */
export type HardeningDestinationResult = {
  id: string;
  path: string;
  ok: boolean;
  free_bytes: number | null;
  required_free_bytes: number;
  message: string;
};

/**
 * Purpose: Snapshot capability result returned by hardening checks.
 *
 * Inputs: Deserialized from IPC.
 * Outputs: A typed snapshot result.
 * Ties to: First-run hardening wizard.
 * Side effects: None.
 * Why: Allow optional validation of snapshot support before enabling scheduling.
 */
export type HardeningSnapshotResult = {
  checked: boolean;
  supported: boolean;
  message: string;
};

/**
 * Purpose: Hardening report returned by the backend.
 *
 * Inputs: Deserialized from IPC.
 * Outputs: A typed hardening report.
 * Ties to: First-run wizard gating in minimal/settings UIs.
 * Side effects: None.
 * Why: Scheduling should only be enabled once prerequisites are satisfied.
 */
export type HardeningReport = {
  ok: boolean;
  message: string;
  watched_ok: string[];
  watched_issues: HardeningWatchedIssue[];
  destinations: HardeningDestinationResult[];
  snapshots: HardeningSnapshotResult;
};

/**
 * Purpose: Config payload that already satisfies validation rules.
 *
 * Inputs: loaded or validated config objects.
 * Outputs: a strongly typed config alias.
 * Ties to: settings state and validation hooks.
 * Side effects: None.
 * Why: document intent when a config is known to be valid.
 */
export type ConfigWithValidation = Config;
