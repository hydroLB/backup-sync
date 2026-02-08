import { safeInvoke, wrapError } from '../../../services/ipc';

/**
 * Summary: Toggle safe mode using the backend command so the daemon is updated when reachable.
 *
 * Inputs: Desired safe mode value.
 * Outputs: The resulting safe mode value.
 * Side effects: Writes config and may update the running daemon via IPC.
 * Error handling: Wraps IPC failures with a stable, actionable context message.
 * Ties to other methods: Used by `MinimalMain` running toggle flow.
 * Why this exists: Keep Start/Stop semantics simple while avoiding daemon restarts when possible.
 */
export async function setSafeMode(desired: boolean): Promise<boolean> {
  try {
    return await safeInvoke<boolean>('toggle_safe_mode_cmd', { desired });
  } catch (error) {
    throw wrapError('[setSafeMode] Failed to toggle safe mode', error);
  }
}
