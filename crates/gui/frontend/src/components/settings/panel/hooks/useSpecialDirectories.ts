import { useCallback } from 'react';
import { tauriAvailable } from '../../../../services/ipc';

let pathModulePromise: Promise<typeof import('@tauri-apps/api/path')> | null = null;

/**
 * Summary: Load the Tauri path module once to avoid repeated dynamic import overhead.
 *
 * Inputs: None.
 * Outputs: Promise resolving to `@tauri-apps/api/path` exports.
 * Side effects: Performs a dynamic import on first call.
 * Error handling: Propagates module load errors to the caller.
 * Ties to other methods: Used by `useSpecialDirectories`.
 * Why this exists: Reduce perceived latency for quick-add and destination shortcuts.
 */
async function loadTauriPathModule() {
  if (!pathModulePromise) {
    pathModulePromise = import('@tauri-apps/api/path');
  }
  return pathModulePromise;
}

/**
 * Summary: Provide helpers for resolving OS special directories via Tauri.
 *
 * Inputs: None.
 * Outputs: A `specialDir` resolver function.
 * Side effects: Loads the Tauri path module on demand.
 * Error handling: Throws stable, contextual errors when IPC or resolution fails.
 * Ties to other methods: Used by settings onboarding quick actions.
 * Why this exists: Keep special-directory resolution centralized and cached.
 */
export function useSpecialDirectories() {
  const specialDir = useCallback(async (kind: 'desktop' | 'documents' | 'downloads') => {
    if (!tauriAvailable()) {
      throw new Error(
        '[useSpecialDirectories::specialDir] IPC unavailable. Launch the app build to pick paths.',
      );
    }
    const { desktopDir, documentDir, downloadDir } = await loadTauriPathModule();
    if (kind === 'desktop') return desktopDir();
    if (kind === 'documents') return documentDir();
    return downloadDir();
  }, []);

  return { specialDir };
}

