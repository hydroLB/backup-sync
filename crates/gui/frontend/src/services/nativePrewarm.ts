import { prewarmDialog } from './dialog';
import { loadTauriInvoke, tauriAvailable } from './ipc';

let prewarmed = false;

/**
 * Purpose: Prewarm native Tauri modules used by common UI interactions.
 *
 * Inputs: None.
 * Outputs: Resolves after best-effort prewarm completes.
 * Side effects: Dynamically imports Tauri modules in the background.
 * Error handling: Best-effort; never throws to avoid breaking startup.
 * Ties to other methods: Improves responsiveness for pickers and IPC-heavy actions.
 * Why this exists: Reduce perceived lag by shifting module load off user click paths.
 */
export async function prewarmNativeApis(): Promise<void> {
  try {
    if (prewarmed) return;
    prewarmed = true;
    if (!tauriAvailable()) return;
    await Promise.all([loadTauriInvoke(), prewarmDialog()]);
  } catch {
    // best-effort prewarm
  }
}

