import { useCallback } from 'react';
import { tauriAvailable } from '../../../../services/ipc';

type AddWatched = (path: string, kind: 'File' | 'Directory', destId: string) => void;

type Params = {
  primaryDestinationId: string;
  addWatched: AddWatched;
  setStatus: (msg: string) => void;
  popup: (msg: string) => void;
  pickSinglePath: (kind: 'File' | 'Directory', title: string) => Promise<string | null>;
  setBackupRootPath: (path: string, label: string) => void;
};

/**
 * Summary: Build watched-path actions used by settings and onboarding.
 *
 * Inputs: Destination id, watched mutator, status/popup handlers, and picker helpers.
 * Outputs: Watch-related action handlers (pick/add/quick-add).
 * Side effects: Opens native pickers and mutates config via the provided callbacks.
 * Error handling: Emits contextual popup messages on failures.
 * Ties to other methods: Used by `useSettingsPanelActions` to compose the full action set.
 * Why this exists: Keep watch list and quick-add flows centralized and consistent.
 */
export function useWatchedActions({
  primaryDestinationId,
  addWatched,
  setStatus,
  popup,
  pickSinglePath,
  setBackupRootPath,
}: Params) {
  const pickPath = useCallback(
    async (kind: 'File' | 'Directory') => {
      const title = kind === 'Directory' ? 'Choose folder to protect' : 'Choose file to protect';
      const selection = await pickSinglePath(kind, title);
      if (!selection) {
        return;
      }
      addWatched(selection, kind, primaryDestinationId);
      setStatus(`Added ${selection}`);
    },
    [addWatched, pickSinglePath, primaryDestinationId, setStatus],
  );

  const addPathToDestination = useCallback(
    async (destId: string, kind: 'File' | 'Directory') => {
      const title = kind === 'Directory' ? 'Choose folder to protect' : 'Choose file to protect';
      const selection = await pickSinglePath(kind, title);
      if (!selection) {
        return;
      }
      addWatched(selection, kind, destId);
      setStatus(`Added ${selection}`);
    },
    [addWatched, pickSinglePath, setStatus],
  );

  const resolveSpecialPath = useCallback(
    async (
      getter: () => Promise<string>,
      label: string,
      onSuccess: (path: string) => void,
      statusPrefix: string,
    ) => {
      try {
        const path = await getter();
        if (!path) {
          popup(`Couldn't find ${label}. Pick a folder instead.`);
          return;
        }
        onSuccess(path);
        setStatus(`${statusPrefix} ${label}`);
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        popup(`[useWatchedActions::resolveSpecialPath] Failed to resolve ${label}: ${reason}`);
      }
    },
    [popup, setStatus],
  );

  const quickAdd = useCallback(
    async (getter: () => Promise<string>, label: string) => {
      try {
        if (!tauriAvailable()) {
          popup('IPC unavailable. Launch the app build to use quick add.');
          return;
        }
        await resolveSpecialPath(
          getter,
          label,
          (path) => addWatched(path, 'Directory', primaryDestinationId),
          'Added',
        );
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        popup(`[useWatchedActions::quickAdd] Quick add failed: ${reason}`);
      }
    },
    [addWatched, popup, primaryDestinationId, resolveSpecialPath],
  );

  const quickAddPathToBackup = useCallback(
    async (getter: () => Promise<string>, label: string) => {
      try {
        if (!tauriAvailable()) {
          popup('IPC unavailable. Launch the app build to set destination.');
          return;
        }
        await resolveSpecialPath(
          getter,
          label,
          (path) => setBackupRootPath(path, label),
          'Backup location set to',
        );
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        popup(`[useWatchedActions::quickAddPathToBackup] Quick destination set failed: ${reason}`);
      }
    },
    [popup, resolveSpecialPath, setBackupRootPath],
  );

  return {
    pickPath,
    addPathToDestination,
    quickAdd,
    quickAddPathToBackup,
  };
}
