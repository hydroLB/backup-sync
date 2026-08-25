import { safeInvoke, wrapError } from './ipc';

/** Shows recent daemon activity in the UI. */
export async function getLogTail(): Promise<string> {
  try {
    return await safeInvoke('log_tail_cmd');
  } catch (error) {
    throw wrapError('[getLogTail] Failed to fetch log tail', error);
  }
}

/** Allows users to share logs for diagnostics. */
export async function exportLogs(): Promise<string> {
  try {
    return await safeInvoke('export_logs_cmd');
  } catch (error) {
    throw wrapError('[exportLogs] Failed to export logs', error);
  }
}
