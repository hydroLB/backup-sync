import { Config } from "../components/settings/types";

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
  uptime_secs: number | null;
  version: string | null;
  free_bytes: number | null;
  min_free_space_bytes?: number | null;
  last_verify_ts: number | null;
  last_verify_status: string | null;
  last_verify_issues: number | null;
  recent_activity: Array<{ path: string; bytes: number; ts: number }>;
  safe_mode?: boolean;
  destinations?: DestinationStatus[];
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
  free_bytes: number | null;
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
 * Purpose: Config payload that already satisfies validation rules.
 *
 * Inputs: loaded or validated config objects.
 * Outputs: a strongly typed config alias.
 * Ties to: settings state and validation hooks.
 * Side effects: None.
 * Why: document intent when a config is known to be valid.
 */
export type ConfigWithValidation = Config;
