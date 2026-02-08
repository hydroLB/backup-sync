import { useCallback } from 'react';
import { Config, Destination } from '../../types';

type PickerHelpers = {
  pickDestination: (onChosen: (path: string) => void) => Promise<void>;
};

type Params = {
  cfg: Config;
  setCfg: (cfg: Config) => void;
  saveCfg: (cfg: Config, message?: string) => void;
  setStatus: (msg: string) => void;
  popup: (msg: string) => void;
  pickerHelpers: PickerHelpers;
};

/**
 * Summary: Build destination-related settings actions.
 *
 * Inputs: Current config, state/persistence callbacks, picker helpers, and popup handler.
 * Outputs: Destination action handlers used by settings and onboarding views.
 * Side effects: Persists config changes and updates status messages.
 * Error handling: Emits contextual popup messages on failures.
 * Ties to other methods: Used by `useSettingsPanelActions` to compose the full action set.
 * Why this exists: Keep destination mutation logic centralized and separate from watch actions.
 */
export function useDestinationActions({ cfg, setCfg, saveCfg, setStatus, popup, pickerHelpers }: Params) {
  const setBackupRootPath = useCallback(
    (path: string, label: string) => {
      try {
        const existing: Destination[] =
          cfg.destinations && cfg.destinations.length > 0 ? cfg.destinations : [];
        const primary: Destination = existing[0] ?? {
          id: 'default',
          label: 'Primary',
          path,
          max_backups_per_file: null,
        };
        const nextDests: Destination[] = [{ ...primary, path }, ...existing.slice(1)];
        saveCfg(
          { ...cfg, backup_root: path, destinations: nextDests },
          `Backup location set${label ? ` to ${label}` : ''}`,
        );
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        popup(`[useDestinationActions::setBackupRootPath] Failed to set backup root: ${reason}`);
      }
    },
    [cfg, popup, saveCfg],
  );

  const pickBackupRoot = useCallback(async () => {
    try {
      await pickerHelpers.pickDestination((path) => setBackupRootPath(path, ''));
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      popup(`[useDestinationActions::pickBackupRoot] Failed to pick backup root: ${reason}`);
    }
  }, [pickerHelpers, popup, setBackupRootPath]);

  const addDestination = useCallback(async () => {
    try {
      await pickerHelpers.pickDestination((selection) => {
        const id = `dest-${Date.now()}`;
        const nextDests: Destination[] = [
          ...(cfg.destinations || []),
          { id, path: selection, max_backups_per_file: null },
        ];
        saveCfg(
          { ...cfg, destinations: nextDests, backup_root: nextDests[0]?.path || selection },
          'Destination added',
        );
        setStatus(`Destination added: ${selection}`);
      });
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      popup(`[useDestinationActions::addDestination] Failed to add destination: ${reason}`);
    }
  }, [cfg, pickerHelpers, popup, saveCfg, setStatus]);

  const setDestinationRetention = useCallback(
    (destId: string, v: number) => {
      try {
        const nextDests = (cfg.destinations || []).map((d) =>
          d.id === destId ? { ...d, max_backups_per_file: v } : d,
        );
        saveCfg({ ...cfg, destinations: nextDests }, 'Destination retention updated');
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        popup(
          `[useDestinationActions::setDestinationRetention] Failed to update retention: ${reason}`,
        );
      }
    },
    [cfg, popup, saveCfg],
  );

  const setDestinationLabel = useCallback(
    (destId: string, label: string) => {
      try {
        const nextDests = (cfg.destinations || []).map((d) =>
          d.id === destId ? { ...d, label } : d,
        );
        setCfg({ ...cfg, destinations: nextDests });
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        popup(`[useDestinationActions::setDestinationLabel] Failed to set label: ${reason}`);
      }
    },
    [cfg, popup, setCfg],
  );

  return {
    setBackupRootPath,
    pickBackupRoot,
    addDestination,
    setDestinationRetention,
    setDestinationLabel,
  };
}

