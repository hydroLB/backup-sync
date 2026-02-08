import { IpcError } from '../../../../services/ipc';

/**
 * Summary: Normalize unknown errors into a safe message string.
 *
 * Inputs: Unknown error value.
 * Outputs: A string message suitable for UI and logs.
 * Side effects: None.
 * Error handling: Never throws; falls back to `String(error)`.
 * Ties to other methods: Used by status refresh and action error handling.
 * Why this exists: Avoid unsafe casts while preserving error context.
 */
export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return String(error);
}

/**
 * Summary: Convert known IPC error codes into an actionable user-facing message.
 *
 * Inputs: IPC error or unknown error payload.
 * Outputs: A user-friendly message string.
 * Side effects: None.
 * Error handling: Never throws; falls back to `errorMessage`.
 * Ties to other methods: Used by status polling and offline banners.
 * Why this exists: Keep UI messages helpful without exposing internal stack context.
 */
export function displayError(error: unknown): string {
  if (error instanceof IpcError) {
    if (error.code === 'DAEMON_OFFLINE') {
      return 'Not connected yet. Finish setup, or enable Start on login to keep the daemon running.';
    }
    if (error.code === 'IPC_TIMEOUT') {
      return 'Daemon did not respond in time. If this persists, restart the daemon or check logs.';
    }
    if (error.code === 'TAURI_UNAVAILABLE') {
      return 'IPC unavailable. Launch the desktop app (./start) instead of a browser.';
    }
    return error.message;
  }
  return errorMessage(error);
}

