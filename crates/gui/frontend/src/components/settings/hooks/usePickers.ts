import { open } from '@tauri-apps/api/dialog';
import { tauriAvailable } from '../../../services/ipc';

/**
 * Purpose: Provide picker helpers for settings actions.
 *
 * Inputs: Current config and popup handler.
 * Outputs: Picker helper functions.
 * Ties to: Settings panel destination and watch pickers.
 * Side effects: Invokes IPC-backed file pickers and emits popup messages.
 * Why: Centralize picker behavior and IPC checks.
 */
export function usePickers(popup: (msg: string) => void) {
  /**
   * Purpose: Pick a file or directory and attach it to a destination.
   *
   * Inputs: Destination id, item kind, and watch handler.
   * Outputs: Updates watch list and status messages.
   * Ties to: Destination board add path actions.
   * Side effects: Opens native pickers, updates config via callbacks, and emits status messages.
   * Why: Ensure consistent picker behavior across destinations.
   */
  const pickPathForDest = async (
    destId: string,
    kind: 'File' | 'Directory',
    addWatched: (path: string, kind: 'File' | 'Directory', destId?: string) => void,
  ) => {
    try {
      if (!tauriAvailable()) {
        popup('Picker unavailable (IPC). Launch the app build to select paths.');
        return;
      }
      const selection = await open({
        directory: kind === 'Directory',
        multiple: false,
        title: kind === 'Directory' ? 'Choose folder to protect' : 'Choose file to protect',
      });
      if (typeof selection === 'string') {
        addWatched(selection, kind, destId);
        popup(`Added ${selection}`);
      } else {
        popup('No selection made or picker was closed.');
      }
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      popup(`[usePickers::pickPathForDest] Picker failed: ${reason}`);
    }
  };

  /**
   * Purpose: Pick a backup destination directory.
   *
   * Inputs: Callback invoked with the chosen path.
   * Outputs: Updates destination and status messaging.
   * Ties to: Destination picker actions.
   * Side effects: Opens native pickers, updates config via callbacks, and emits status messages.
   * Why: Keep destination selection logic consistent.
   */
  const pickDestination = async (onChosen: (path: string) => void) => {
    try {
      if (!tauriAvailable()) {
        popup('Picker unavailable (IPC). Launch the app build to choose destination.');
        return;
      }
      const selection = await open({
        directory: true,
        multiple: false,
        title: 'Choose a backup destination',
      });
      if (typeof selection === 'string') {
        onChosen(selection);
        popup(`Destination set to ${selection}`);
      } else {
        popup('No destination selected or picker was closed.');
      }
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      popup(`[usePickers::pickDestination] Picker failed: ${reason}`);
    }
  };

  return { pickPathForDest, pickDestination };
}
