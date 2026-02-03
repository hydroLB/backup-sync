// Safe IPC wrapper for browser mode and native desktop mode.

import { UI_TUNING } from "../config/uiTuning";

/**
 * Purpose: Provide a typed error shape for IPC failures.
 *
 * Inputs: Message, optional backend error code, and optional original payload.
 * Outputs: An Error instance with a stable `code` field.
 * Ties to: `safeInvoke` and UI error handling that inspects `code`.
 * Side effects: Captures a stack trace at construction time.
 * Why: Preserve backend error codes and avoid `[object Object]` failures in the UI.
 */
export class IpcError extends Error {
  public readonly code?: string;
  public readonly details?: unknown;

  constructor(message: string, code?: string, details?: unknown) {
    super(message);
    this.name = "IpcError";
    if (code !== undefined) {
      this.code = code;
    }
    if (details !== undefined) {
      this.details = details;
    }
  }
}

type UnknownRecord = Record<string, unknown>;

/**
 * Purpose: Narrow an unknown value to a record type.
 *
 * Inputs: Unknown value.
 * Outputs: `true` when the value is a non-null object.
 * Ties to: IPC error normalization.
 * Side effects: None.
 * Why: Avoid unsafe casts when reading error payload fields.
 */
function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null;
}

/**
 * Purpose: Normalize an unknown IPC failure into a message and optional code.
 *
 * Inputs: Unknown error payload.
 * Outputs: Normalized message and optional error code.
 * Ties to: `withTimeout` and `safeInvoke` error mapping.
 * Side effects: None.
 * Why: Tauri command errors can arrive as plain objects, not `Error` instances.
 */
function normalizeFailure(error: unknown): { message: string; code?: string } {
  if (error instanceof IpcError) {
    const out: { message: string; code?: string } = { message: error.message };
    if (error.code !== undefined) out.code = error.code;
    return out;
  }
  if (error instanceof Error) {
    return { message: error.message };
  }
  if (isRecord(error)) {
    const code = typeof error.code === "string" ? error.code : undefined;
    if (typeof error.message === "string") {
      const out: { message: string; code?: string } = { message: error.message };
      if (code !== undefined) out.code = code;
      return out;
    }
    if (typeof error.error === "string") {
      const out: { message: string; code?: string } = { message: error.error };
      if (code !== undefined) out.code = code;
      return out;
    }
    try {
      const out: { message: string; code?: string } = { message: JSON.stringify(error) };
      if (code !== undefined) out.code = code;
      return out;
    } catch {
      const out: { message: string; code?: string } = { message: String(error) };
      if (code !== undefined) out.code = code;
      return out;
    }
  }
  return { message: String(error) };
}

/**
 * Purpose: Wrap an unknown error with additional context while preserving error codes.
 *
 * Inputs: Context label and an unknown error payload.
 * Outputs: An `IpcError` carrying a normalized message and optional `code`.
 * Ties to: Service layer error wrapping across the frontend.
 * Side effects: None.
 * Why: Keep error handling consistent and actionable without losing backend error metadata.
 */
export function wrapError(context: string, error: unknown): IpcError {
  const norm = normalizeFailure(error);
  return new IpcError(`${context}: ${norm.message}`, norm.code, error);
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
      reject(new IpcError(`${label} timed out after ${timeoutMs}ms`, "IPC_TIMEOUT"));
    }, timeoutMs);
  });
  try {
    return await Promise.race([promise, timeoutPromise]);
  } catch (error) {
    throw wrapError(`${label} failed`, error);
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
    const bridge = typeof window !== "undefined" ? (window as unknown as { __TAURI_IPC__?: unknown }).__TAURI_IPC__ : undefined;
    return typeof bridge === "function";
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
  return hasTauri();
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
  if (!hasTauri()) {
    throw new IpcError("Tauri IPC unavailable; run the desktop app build", "TAURI_UNAVAILABLE");
  }
  const { invoke } = await import("@tauri-apps/api/tauri");
  const timeoutMs = UI_TUNING.system.ipcTimeoutMs;
  return await withTimeout(invoke<T>(cmd, args), timeoutMs, `IPC ${cmd}`);
}
