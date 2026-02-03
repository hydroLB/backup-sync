import { safeInvoke, wrapError } from "./ipc";

/**
 * Purpose: Fetch the recent log tail from the backend.
 *
 * Inputs: None.
 * Outputs: The log tail string.
 * Ties to: Log viewer panels and diagnostic widgets.
 * Side effects: Invokes IPC calls to the backend.
 * Why: Shows recent daemon activity in the UI.
 */
export async function getLogTail(): Promise<string> {
  try {
    return await safeInvoke("log_tail_cmd");
  } catch (error) {
    throw wrapError("[getLogTail] Failed to fetch log tail", error);
  }
}

/**
 * Purpose: Export logs to the Desktop directory.
 *
 * Inputs: None.
 * Outputs: The path to the exported log file.
 * Ties to: Log export actions and support workflows.
 * Side effects: Invokes IPC calls that write log files.
 * Why: Allows users to share logs for diagnostics.
 */
export async function exportLogs(): Promise<string> {
  try {
    return await safeInvoke("export_logs_cmd");
  } catch (error) {
    throw wrapError("[exportLogs] Failed to export logs", error);
  }
}
