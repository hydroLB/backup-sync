// Safe IPC wrapper for the desktop Tauri runtime.

import { UI_TUNING } from '../config/uiTuning';
import { IS_WEB_RUNTIME } from '../runtime/mode';
import { invokeWebCommand } from '../runtime/web/engine';

/** Preserve backend error codes and avoid `[object Object]` failures in the UI. */
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

/** Avoid unsafe casts when reading error payload fields. */
function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === 'object' && value !== null;
}

/** Tauri command errors can arrive as plain objects, not `Error` instances. */
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

/** Keep error handling consistent and actionable without losing backend error metadata. */
export function wrapError(context: string, error: unknown): IpcError {
  const norm = normalizeFailure(error);
  return new IpcError(`${context}: ${norm.message}`, norm.code, error);
}

/** Prevent hung IPC calls from stalling the UI. */
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

/** Some startup paths need to delay work until the native bridge is ready. */
function hasTauri(): boolean {
  try {
    if (typeof window === 'undefined') return false;
    const nativeWindow = window as unknown as {
      __TAURI_INTERNALS__?: { invoke?: unknown };
      __TAURI_IPC__?: unknown;
    };
    return (
      typeof nativeWindow.__TAURI_INTERNALS__?.invoke === 'function' ||
      typeof nativeWindow.__TAURI_IPC__ === 'function'
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[hasTauri] Failed to detect IPC availability: ${reason}`);
  }
}

/** Allows the UI to show or hide native only actions safely. */
export function tauriAvailable(): boolean {
  return hasTauri();
}

/** Provides a single IPC entry point with consistent error context. */
export async function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (IS_WEB_RUNTIME) {
    return await withTimeout(
      invokeWebCommand<T>(cmd, args),
      UI_TUNING.system.ipcTimeoutMs,
      `Web ${cmd}`,
    );
  }
  if (!hasTauri()) {
    throw new IpcError('Tauri IPC unavailable; run the desktop app build', 'TAURI_UNAVAILABLE');
  }
  const { invoke } = await loadTauriInvoke();
  const timeoutMs = UI_TUNING.system.ipcTimeoutMs;
  return await withTimeout(invoke<T>(cmd, args), timeoutMs, `IPC ${cmd}`);
}

/** Some checks can legitimately take longer than the global IPC timeout without hanging the UI. */
export async function safeInvokeWithTimeout<T>(
  cmd: string,
  args: Record<string, unknown> | undefined,
  timeoutMs: number,
): Promise<T> {
  if (IS_WEB_RUNTIME) {
    const effectiveTimeout =
      Number.isFinite(timeoutMs) && timeoutMs >= 0 ? timeoutMs : UI_TUNING.system.ipcTimeoutMs;
    return await withTimeout(invokeWebCommand<T>(cmd, args), effectiveTimeout, `Web ${cmd}`);
  }
  if (!hasTauri()) {
    throw new IpcError('Tauri IPC unavailable; run the desktop app build', 'TAURI_UNAVAILABLE');
  }
  const { invoke } = await loadTauriInvoke();
  const effectiveTimeout =
    Number.isFinite(timeoutMs) && timeoutMs >= 0 ? timeoutMs : UI_TUNING.system.ipcTimeoutMs;
  return await withTimeout(invoke<T>(cmd, args), effectiveTimeout, `IPC ${cmd}`);
}

type TauriModule = typeof import('@tauri-apps/api/core');
let tauriModulePromise: Promise<TauriModule> | null = null;

/** Avoid repeated dynamic imports on hot IPC paths. */
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
