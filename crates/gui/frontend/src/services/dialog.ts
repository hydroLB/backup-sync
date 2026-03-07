import { wrapError } from './ipc';

type DialogModule = typeof import('@tauri-apps/plugin-dialog');
type OpenOptions = import('@tauri-apps/plugin-dialog').OpenDialogOptions;
type OpenResult = Awaited<ReturnType<DialogModule['open']>>;

let dialogModulePromise: Promise<DialogModule> | null = null;

/**
 * Summary: Load the Tauri dialog module with caching.
 *
 * Inputs: None.
 *
 * Outputs: The dialog API module.
 *
 * Side effects: Dynamically imports the dialog module on first use.
 *
 * Error handling: Throws a contextualized error when the module fails to load.
 *
 * Ties to other methods: Used by `openDialog` and `prewarmDialog`.
 *
 * Why this exists: Avoid repeated module resolution and reduce click-to-dialog latency.
 */
export async function loadDialogModule(): Promise<DialogModule> {
  try {
    if (!dialogModulePromise) {
      dialogModulePromise = import('@tauri-apps/plugin-dialog');
    }
    return await dialogModulePromise;
  } catch (error) {
    dialogModulePromise = null;
    throw wrapError('[loadDialogModule] Failed to load dialog module', error);
  }
}

/**
 * Summary: Open a native file/folder picker dialog.
 *
 * Inputs: Dialog options, including `directory` and `title`.
 *
 * Outputs: The selection result (`string`, `string[]`, or `null`).
 *
 * Side effects: Triggers the OS-native picker UI via Tauri.
 *
 * Error handling: Wraps unknown failures with actionable context.
 *
 * Ties to other methods: Used by `usePickers` and restore flows.
 *
 * Why this exists: Centralize dialog calls so we can prewarm/cache for performance.
 */
export async function openDialog(options: OpenOptions): Promise<OpenResult> {
  try {
    const mod = await loadDialogModule();
    return await mod.open(options);
  } catch (error) {
    throw wrapError('[openDialog] Failed to open native picker dialog', error);
  }
}

/**
 * Summary: Prewarm the native dialog module to reduce first-use latency.
 *
 * Inputs: None.
 *
 * Outputs: Resolves when the module is loaded.
 *
 * Side effects: Loads the dialog module in the background.
 *
 * Error handling: Swallows errors to keep UI functional; callers handle picker failures.
 *
 * Ties to other methods: Called by `prewarmNativeApis`.
 *
 * Why this exists: Move module initialization cost off the critical click path.
 */
export async function prewarmDialog(): Promise<void> {
  try {
    await loadDialogModule();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.warn(`[prewarmDialog] Best-effort prewarm failed: ${reason}`);
  }
}
