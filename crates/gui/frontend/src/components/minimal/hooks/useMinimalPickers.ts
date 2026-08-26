import { useCallback } from 'react';
import { openDialog } from '../../../services/dialog';

type EventKind = 'ok' | 'error' | 'info';

export type PickerBusyScope = 'source' | 'destination' | null;

type Params = {
  onEvent: (msg: string, kind?: EventKind) => void;
  onPickerBusyChange: (next: PickerBusyScope) => void;
  pickWebStoragePath?: () => Promise<string | null>;
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

/** Tauri dialog results can be either a string, a string array, or `null`. */
function firstPath(selection: string | string[] | null): string | null {
  if (typeof selection === 'string') return selection;
  if (Array.isArray(selection)) return selection[0] ?? null;
  return null;
}

/** Keeps native picker wiring local to the active minimal UI after removing the legacy settings shell. */
export function useMinimalPickers({
  onEvent,
  onPickerBusyChange,
  pickWebStoragePath,
}: Params): MinimalPickers {
  const pickDestinationPath = useCallback(async () => {
    onPickerBusyChange('destination');
    try {
      if (pickWebStoragePath) return await pickWebStoragePath();
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
      onPickerBusyChange(null);
    }
  }, [onEvent, onPickerBusyChange, pickWebStoragePath]);

  const pickPathForDest = useCallback(
    async (
      destinationId: string,
      kind: 'File' | 'Directory',
      addWatched: AddWatched,
    ): Promise<void> => {
      onPickerBusyChange('source');
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
        onPickerBusyChange(null);
      }
    },
    [onEvent, onPickerBusyChange],
  );

  return {
    pickDestinationPath,
    pickPathForDest,
  };
}
