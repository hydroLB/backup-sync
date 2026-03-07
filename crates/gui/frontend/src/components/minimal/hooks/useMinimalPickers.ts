import { useCallback } from 'react';
import { openDialog } from '../../../services/dialog';

type EventKind = 'ok' | 'error' | 'info';

type Params = {
  onEvent: (msg: string, kind?: EventKind) => void;
  onPickerBusyChange: (next: boolean) => void;
};

type AddWatched = (
  path: string,
  kind: 'File' | 'Directory',
  destinationId?: string,
) => void | Promise<void>;

type MinimalPickers = {
  pickDestinationPath: () => Promise<string | null>;
  pickPathForDest: (
    destinationId: string,
    kind: 'File' | 'Directory',
    addWatched: AddWatched,
  ) => Promise<void>;
};

/**
 * Summary: Normalize a dialog selection into a single path string.
 *
 * Inputs: Result from the Tauri open dialog.
 *
 * Outputs: The selected path or `null`.
 *
 * Side effects: None.
 *
 * Error handling: Returns `null` for cancelled selections and empty arrays.
 *
 * Ties to other methods: Used by both destination and watched-path picker flows.
 *
 * Why this exists: Tauri dialog results can be either a string, a string array, or `null`.
 */
function firstPath(selection: string | string[] | null): string | null {
  if (typeof selection === 'string') return selection;
  if (Array.isArray(selection)) return selection[0] ?? null;
  return null;
}

/**
 * Summary: Build the desktop-only picker handlers used by the minimal shell.
 *
 * Inputs: Event callback and picker busy-state callback.
 *
 * Outputs: Stable picker handlers for destination and watched-path selection.
 *
 * Side effects: Opens native dialogs and updates picker busy state.
 *
 * Error handling: Reports actionable picker failures through `onEvent`.
 *
 * Ties to other methods: Used by `MinimalMain` and `useMinimalActions`.
 *
 * Why this exists: Keeps native picker wiring local to the active minimal UI after removing the legacy settings shell.
 */
export function useMinimalPickers({ onEvent, onPickerBusyChange }: Params): MinimalPickers {
  const pickDestinationPath = useCallback(async () => {
    onPickerBusyChange(true);
    try {
      const selection = await openDialog({
        directory: true,
        multiple: false,
        title: 'Choose destination',
      });
      return firstPath(selection);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      onEvent(`[useMinimalPickers] Failed to choose destination: ${reason}`, 'error');
      return null;
    } finally {
      onPickerBusyChange(false);
    }
  }, [onEvent, onPickerBusyChange]);

  const pickPathForDest = useCallback(
    async (
      destinationId: string,
      kind: 'File' | 'Directory',
      addWatched: AddWatched,
    ): Promise<void> => {
      onPickerBusyChange(true);
      try {
        const selection = await openDialog({
          directory: kind === 'Directory',
          multiple: false,
          title: kind === 'Directory' ? 'Choose protected folder' : 'Choose protected file',
        });
        const selectedPath = firstPath(selection);
        if (!selectedPath) return;
        await addWatched(selectedPath, kind, destinationId);
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        onEvent(`[useMinimalPickers] Failed to choose protected path: ${reason}`, 'error');
      } finally {
        onPickerBusyChange(false);
      }
    },
    [onEvent, onPickerBusyChange],
  );

  return {
    pickDestinationPath,
    pickPathForDest,
  };
}
