// Safe IPC wrapper for the desktop Tauri runtime.

import { UI_TUNING } from '../config/uiTuning';

/**
 * Summary: Provide a typed error shape for IPC failures.
 *
 * Inputs: Message, optional backend error code, and optional original payload.
 *
 * Outputs: An Error instance with a stable `code` field.
 *
 * Side effects: Captures a stack trace at construction time.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: `safeInvoke` and UI error handling that inspects `code`.
 *
 * Why this exists: Preserve backend error codes and avoid `[object Object]` failures in the UI.
 */
export class IpcError extends Error {
  public readonly code?: string;
  public readonly details?: unknown;

  constructor(message: string, code?: string, details?: unknown) {
    super(message);
    this.name = 'IpcError';
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
 * Summary: Narrow an unknown value to a record type.
 *
 * Inputs: Unknown value.
 *
 * Outputs: `true` when the value is a non-null object.
 *
 * Side effects: None.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: IPC error normalization.
 *
 * Why this exists: Avoid unsafe casts when reading error payload fields.
 */
function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null;
}

/**
 * Summary: Normalize an unknown IPC failure into a message and optional code.
 *
 * Inputs: Unknown error payload.
 *
 * Outputs: Normalized message and optional error code.
 *
 * Side effects: None.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: `withTimeout` and `safeInvoke` error mapping.
 *
 * Why this exists: Tauri command errors can arrive as plain objects, not `Error` instances.
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
    const code = typeof error.code === 'string' ? error.code : undefined;
    if (typeof error.message === 'string') {
      const out: { message: string; code?: string } = { message: error.message };
      if (code !== undefined) out.code = code;
      return out;
    }
    if (typeof error.error === 'string') {
      const out: { message: string; code?: string } = { message: error.error };
      if (code !== undefined) out.code = code;
      return out;
    }
    try {
      const out: { message: string; code?: string } = { message: JSON.stringify(error) };
      if (code !== undefined) out.code = code;
      return out;
    } catch (stringifyError) {
      const reason =
        stringifyError instanceof Error ? stringifyError.message : String(stringifyError);
      const out: { message: string; code?: string } = {
        message: `${String(error)} (JSON stringify failed: ${reason})`,
      };
      if (code !== undefined) out.code = code;
      return out;
    }
  }
  return { message: String(error) };
}

/**
 * Summary: Wrap an unknown error with additional context while preserving error codes.
 *
 * Inputs: Context label and an unknown error payload.
 *
 * Outputs: An `IpcError` carrying a normalized message and optional `code`.
 *
 * Side effects: None.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: Service layer error wrapping across the frontend.
 *
 * Why this exists: Keep error handling consistent and actionable without losing backend error metadata.
 */
export function wrapError(context: string, error: unknown): IpcError {
  const norm = normalizeFailure(error);
  return new IpcError(`${context}: ${norm.message}`, norm.code, error);
}

/**
 * Summary: Enforce a timeout for a promise-based operation.
 *
 * Inputs: Promise to execute, timeout duration, and error label.
 *
 * Outputs: Resolves with the promise result or rejects on timeout.
 *
 * Side effects: Schedules and clears timer callbacks.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: IPC invocations and UI responsiveness.
 *
 * Why this exists: Prevent hung IPC calls from stalling the UI.
 */
async function withTimeout<T>(promise: Promise<T>, timeoutMs: number, label: string): Promise<T> {
  if (timeoutMs <= 0) {
    return promise;
  }
  let timeoutId: ReturnType<typeof setTimeout> | null = null;
  const timeoutPromise = new Promise<T>((_, reject) => {
    timeoutId = setTimeout(() => {
      reject(new IpcError(`${label} timed out after ${timeoutMs}ms`, 'IPC_TIMEOUT'));
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
 * Summary: Determine if the Tauri IPC bridge is available.
 *
 * Inputs: Reads from the global `window` object.
 *
 * Outputs: `true` when the IPC bridge is present.
 *
 * Side effects: Reads global `window` state.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: `tauriAvailable` and `safeInvoke` guards in the service layer.
 *
 * Why this exists: Some startup paths need to delay work until the native bridge is ready.
 */
function hasTauri(): boolean {
  try {
    const bridge =
      typeof window !== 'undefined'
        ? (window as unknown as { __TAURI_IPC__?: unknown }).__TAURI_IPC__
        : undefined;
    return typeof bridge === 'function';
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[hasTauri] Failed to detect IPC availability: ${reason}`);
  }
}

/**
 * Summary: Expose IPC availability to calling code.
 *
 * Inputs: None.
 *
 * Outputs: `true` when the IPC bridge can be used.
 *
 * Side effects: Reads global `window` state.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: UI decisions that depend on native capabilities.
 *
 * Why this exists: Allows the UI to show or hide native only actions safely.
 */
export function tauriAvailable(): boolean {
  return hasTauri();
}

/**
 * Summary: Invoke a Tauri command with timeout and error guardrails.
 *
 * Inputs: `cmd` as the command name, `args` as an optional argument map.
 *
 * Outputs: A typed response from the backend IPC layer.
 *
 * Side effects: Invokes IPC calls and loads the Tauri API module.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: All backend IPC calls in the services layer.
 *
 * Why this exists: Provides a single IPC entry point with consistent error context.
 */
export async function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!hasTauri()) {
    throw new IpcError('Tauri IPC unavailable; run the desktop app build', 'TAURI_UNAVAILABLE');
  }
  const { invoke } = await loadTauriInvoke();
  const timeoutMs = UI_TUNING.system.ipcTimeoutMs;
  return await withTimeout(invoke<T>(cmd, args), timeoutMs, `IPC ${cmd}`);
}

/**
 * Summary: Invoke a Tauri command with a per-call timeout override.
 *
 * Inputs: `cmd` command name, `args` optional argument map, and `timeoutMs` override.
 *
 * Outputs: A typed response from the backend IPC layer.
 *
 * Side effects: Invokes IPC calls and loads the Tauri API module.
 *
 * Error handling: Throws an `IpcError` on timeout or IPC failure, preserving backend codes when possible.
 *
 * Ties to other methods: Used by long-running health checks like hardening probes.
 *
 * Why this exists: Some checks can legitimately take longer than the global IPC timeout without hanging the UI.
 */
export async function safeInvokeWithTimeout<T>(
  cmd: string,
  args: Record<string, unknown> | undefined,
  timeoutMs: number,
): Promise<T> {
  if (!hasTauri()) {
    throw new IpcError('Tauri IPC unavailable; run the desktop app build', 'TAURI_UNAVAILABLE');
  }
  const { invoke } = await loadTauriInvoke();
  const effectiveTimeout =
    Number.isFinite(timeoutMs) && timeoutMs > 0 ? timeoutMs : UI_TUNING.system.ipcTimeoutMs;
  return await withTimeout(invoke<T>(cmd, args), effectiveTimeout, `IPC ${cmd}`);
}

type TauriModule = typeof import('@tauri-apps/api/core');
let tauriModulePromise: Promise<TauriModule> | null = null;

/**
 * Summary: Load the Tauri invoke module with caching.
 *
 * Inputs: None.
 *
 * Outputs: The Tauri API module.
 *
 * Side effects: Dynamically imports the Tauri module on first use.
 *
 * Error handling: Throws a contextualized error when the module fails to load.
 *
 * Ties to other methods: Used by `safeInvoke` and `prewarmNativeApis`.
 *
 * Why this exists: Avoid repeated dynamic imports on hot IPC paths.
 */
export async function loadTauriInvoke(): Promise<TauriModule> {
  try {
    if (!tauriModulePromise) {
      tauriModulePromise = import('@tauri-apps/api/core');
    }
    return await tauriModulePromise;
  } catch (error) {
    tauriModulePromise = null;
    throw wrapError('[loadTauriInvoke] Failed to load Tauri invoke module', error);
  }
}
