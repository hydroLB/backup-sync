import { prewarmDialog } from './dialog';
import { loadTauriInvoke, tauriAvailable } from './ipc';

type PrewarmState = 'idle' | 'in_progress' | 'done';
let prewarmState: PrewarmState = 'idle';

/**
 * Summary: Prewarm native Tauri modules used by common UI interactions.
 *
 * Inputs: None.
 *
 * Outputs: Resolves after best-effort prewarm completes.
 *
 * Side effects: Dynamically imports Tauri modules in the background.
 *
 * Error handling: Best-effort; never throws to avoid breaking startup.
 *
 * Ties to other methods: Improves responsiveness for pickers and IPC-heavy actions.
 *
 * Why this exists: Reduce perceived lag by shifting module load off user click paths.
 */
export async function prewarmNativeApis(): Promise<void> {
  try {
    if (prewarmState !== 'idle') return;
    // Do not permanently "burn" prewarming if the IPC bridge is not
    // available yet. Tauri can attach __TAURI_IPC__ slightly after first paint.
    if (!tauriAvailable()) return;
    prewarmState = 'in_progress';
    await Promise.all([loadTauriInvoke(), prewarmDialog()]);
    prewarmState = 'done';
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.warn(`[prewarmNativeApis] Native prewarm failed: ${reason}`);
    prewarmState = 'idle';
  }
}
