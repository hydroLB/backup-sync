import { prewarmDialog } from './dialog';
import { loadTauriInvoke, tauriAvailable } from './ipc';
import { IS_WEB_RUNTIME } from '../runtime/mode';

type PrewarmState = 'idle' | 'in_progress' | 'done';
let prewarmState: PrewarmState = 'idle';

/** Reduce perceived lag by shifting module load off user click paths. */
export async function prewarmNativeApis(): Promise<void> {
  try {
    if (IS_WEB_RUNTIME) return;
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
