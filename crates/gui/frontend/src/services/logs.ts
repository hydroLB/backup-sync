import { safeInvoke, wrapError } from './ipc';

/**
 * Summary: Fetch the recent log tail from the backend.
 *
 * Inputs: None.
 *
 * Outputs: The log tail string.
 *
 * Side effects: Invokes IPC calls to the backend.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: Log viewer panels and diagnostic widgets.
 *
 * Why this exists: Shows recent daemon activity in the UI.
 */
export async function getLogTail(): Promise<string> {
  try {
    return await safeInvoke('log_tail_cmd');
  } catch (error) {
    throw wrapError('[getLogTail] Failed to fetch log tail', error);
  }
}

/**
 * Summary: Export logs to the Desktop directory.
 *
 * Inputs: None.
 *
 * Outputs: The path to the exported log file.
 *
 * Side effects: Invokes IPC calls that write log files.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: Log export actions and support workflows.
 *
 * Why this exists: Allows users to share logs for diagnostics.
 */
export async function exportLogs(): Promise<string> {
  try {
    return await safeInvoke('export_logs_cmd');
  } catch (error) {
    throw wrapError('[exportLogs] Failed to export logs', error);
  }
}
