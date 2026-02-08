import { useCallback } from 'react';
import { openDialog } from '../../../../services/dialog';
import { tauriAvailable } from '../../../../services/ipc';

type Params = {
  popup: (msg: string) => void;
};

/**
 * Summary: Provide panel-scoped native picker helpers with consistent IPC error handling.
 *
 * Inputs: A `popup` function for surfacing actionable errors to the UI.
 * Outputs: A `pickSinglePath` function for file/directory selection.
 * Side effects: Opens OS-native pickers via Tauri.
 * Error handling: Emits a popup message and returns null when the picker fails.
 * Ties to other methods: Used by settings destination and watch selection actions.
 * Why this exists: Avoid duplicating picker wiring across settings handlers.
 */
export function usePanelPickers({ popup }: Params) {
  const pickSinglePath = useCallback(
    async (kind: 'File' | 'Directory', title: string): Promise<string | null> => {
      try {
        if (!tauriAvailable()) {
          popup('Picker unavailable (IPC). Launch the Tauri app build to select paths.');
          return null;
        }
        const selection = await openDialog({
          directory: kind === 'Directory',
          multiple: false,
          title,
        });
        return typeof selection === 'string' ? selection : null;
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        popup(`[usePanelPickers::pickSinglePath] Picker failed: ${reason}`);
        return null;
      }
    },
    [popup],
  );

  return { pickSinglePath };
}

