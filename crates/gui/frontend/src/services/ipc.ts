// Safe IPC wrapper for browser mode and native desktop mode.

import { UI_TUNING } from "../config/uiTuning";

declare global {
  interface Window {
    __TAURI_IPC__?: unknown;
  }
}

/**
 * Purpose: Enforce a timeout for a promise-based operation.
 *
 * Inputs: Promise to execute, timeout duration, and error label.
 * Outputs: Resolves with the promise result or rejects on timeout.
 * Ties to: IPC invocations and UI responsiveness.
 * Side effects: Schedules and clears timer callbacks.
 * Why: Prevent hung IPC calls from stalling the UI.
 */
async function withTimeout<T>(promise: Promise<T>, timeoutMs: number, label: string): Promise<T> {
  if (timeoutMs <= 0) {
    return promise;
  }
  let timeoutId: ReturnType<typeof setTimeout> | null = null;
  const timeoutPromise = new Promise<T>((_, reject) => {
    timeoutId = setTimeout(() => {
      reject(new Error(`[withTimeout] ${label} timed out after ${timeoutMs}ms`));
    }, timeoutMs);
  });
  try {
    return await Promise.race([promise, timeoutPromise]);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[withTimeout] ${label} failed: ${reason}`);
  } finally {
    if (timeoutId) {
      clearTimeout(timeoutId);
    }
  }
}

/**
 * Purpose: Determine if the Tauri IPC bridge is available.
 *
 * Inputs: Reads from the global `window` object.
 * Outputs: `true` when the IPC bridge is present.
 * Ties to: `tauriAvailable` and `safeInvoke` guards in the service layer.
 * Side effects: Reads global `window` state.
 * Why: Allows services to short circuit in browser mode.
 */
function hasTauri(): boolean {
  try {
    return typeof window !== "undefined" && typeof window.__TAURI_IPC__ === "function";
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[hasTauri] Failed to detect IPC availability: ${reason}`);
  }
}

/**
 * Purpose: Expose IPC availability to calling code.
 *
 * Inputs: None.
 * Outputs: `true` when the IPC bridge can be used.
 * Ties to: UI decisions that depend on native capabilities.
 * Side effects: Reads global `window` state.
 * Why: Allows the UI to show or hide native only actions safely.
 */
export function tauriAvailable(): boolean {
  try {
    return hasTauri();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[tauriAvailable] Failed to determine IPC availability: ${reason}`);
  }
}

/**
 * Purpose: Invoke a Tauri command with guardrails for browser mode.
 *
 * Inputs: `cmd` as the command name, `args` as an optional argument map.
 * Outputs: A typed response from the backend IPC layer.
 * Ties to: All backend IPC calls in the services layer.
 * Side effects: Invokes IPC calls and loads the Tauri API module.
 * Why: Provides a single IPC entry point with consistent error context.
 */
export async function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    if (!hasTauri()) {
      throw new Error("Tauri IPC unavailable; run the desktop app build");
    }
    const { invoke } = await import("@tauri-apps/api/tauri");
    const timeoutMs = UI_TUNING.system.ipcTimeoutMs;
    return await withTimeout(invoke<T>(cmd, args), timeoutMs, `IPC ${cmd}`);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`[safeInvoke] IPC invoke failed for ${cmd}: ${message}`);
  }
}
