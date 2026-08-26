import { useCallback } from 'react';
import { Config, WatchedPath } from '../../../domain/config';
import { ensurePrimaryDestination, normalizeWatched } from '../helpers/config';

type EventKind = 'ok' | 'error' | 'info';

type Pickers = {
  pickDestinationPath: () => Promise<string | null>;
  pickPathForDest: (
    destId: string,
    kind: 'File' | 'Directory',
    addWatched: (path: string, kind: 'File' | 'Directory', destId?: string) => void | Promise<void>,
  ) => Promise<void>;
};

type Params = {
  cfg: Config | null;
  primaryId: string | null;
  persist: (next: Config, successMessage?: string) => Promise<void>;
  pickers: Pickers;
  onEvent: (msg: string, kind?: EventKind) => void;
};

type MinimalActions = {
  chooseDestination: () => Promise<void>;
  addDestination: () => Promise<void>;
  removeDestination: (destinationId: string) => Promise<void>;
  addFolder: () => Promise<void>;
  addFile: () => Promise<void>;
  changePath: (path: string, kind: 'File' | 'Directory') => Promise<void>;
  removePath: (
    path: string,
    kind: 'File' | 'Directory',
    sourceDestinationId: string,
  ) => Promise<void>;
  updateKeep: (
    path: string,
    kind: 'File' | 'Directory',
    sourceDestinationId: string,
    keep: number,
  ) => Promise<void>;
  updateDestination: (
    path: string,
    kind: 'File' | 'Directory',
    sourceDestinationId: string,
    destinationId: string,
  ) => Promise<void>;
};

/** Keep minimal success messages concise and grammatically correct. */
function destinationCountLabel(count: number): string {
  return `${count} destination${count === 1 ? '' : 's'}`;
}

/** Older configs can omit destination ids and still need deterministic behavior. */
function resolveWatchedDestinationId(watched: WatchedPath, fallbackDestinationId: string): string {
  return watched.destination_id ?? fallbackDestinationId;
}

/** A source path should be uniquely tracked per kind when expanding to all destinations. */
function sourceKey(path: string, kind: 'File' | 'Directory'): string {
  return `${kind}:${path}`;
}

/** Keep destination creation deterministic and avoid id conflicts. */
function nextDestinationId(cfg: Config): string {
  const base = 'dest';
  const used = new Set((cfg.destinations || []).map((destination) => destination.id));
  if (!used.has(base)) return base;
  for (let i = 2; i < 1000; i += 1) {
    const id = `${base}-${i}`;
    if (!used.has(id)) return id;
  }
  return `${base}-${Date.now()}`;
}

/** Centralize config mutation logic so UI components remain presentational. */
export function useMinimalActions({
  cfg,
  primaryId,
  persist,
  pickers,
  onEvent,
}: Params): MinimalActions {
  const chooseDestination = useCallback(async () => {
    if (!cfg) return;
    try {
      const picked = await pickers.pickDestinationPath();
      if (!picked) return;
      const { cfg: normalized, dest } = ensurePrimaryDestination(cfg);
      const next: Config = {
        ...normalized,
        backup_root: picked,
        destinations: [{ ...dest, path: picked }, ...normalized.destinations.slice(1)],
      };
      await persist(next, `Primary destination set to ${picked}.`);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      onEvent(`[useMinimalActions] Failed to choose destination: ${reason}`, 'error');
    }
  }, [cfg, onEvent, persist, pickers]);

  const addDestination = useCallback(async () => {
    if (!cfg) return;
    try {
      const picked = await pickers.pickDestinationPath();
      if (!picked) return;
      const { cfg: normalized } = ensurePrimaryDestination(cfg);
      const exists = normalized.destinations.some((destination) => destination.path === picked);
      if (exists) {
        onEvent('That destination is already configured.', 'info');
        return;
      }
      const newDestinationId = nextDestinationId(normalized);
      const nextDestinations = [
        ...normalized.destinations,
        {
          id: newDestinationId,
          path: picked,
          label: `Destination ${normalized.destinations.length + 1}`,
          replicate_to: [],
        },
      ];
      const templatesBySource = new Map<string, WatchedPath>();
      for (const watched of normalized.watched) {
        const kind = watched.kind ?? 'Directory';
        const key = sourceKey(watched.path, kind);
        if (templatesBySource.has(key)) continue;
        templatesBySource.set(key, watched);
      }
      const additions: WatchedPath[] = [];
      for (const watched of templatesBySource.values()) {
        additions.push({
          ...watched,
          destination_id: newDestinationId,
        });
      }
      await persist(
        {
          ...normalized,
          destinations: nextDestinations,
          watched: [...normalized.watched, ...additions],
        },
        `Added destination ${picked}. Existing protected paths now target it too.`,
      );
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      onEvent(`[useMinimalActions] Failed to add destination: ${reason}`, 'error');
    }
  }, [cfg, onEvent, persist, pickers]);

  const removeDestination = useCallback(
    async (destinationId: string) => {
      if (!cfg) return;
      try {
        const { cfg: normalized } = ensurePrimaryDestination(cfg);
        if ((normalized.destinations || []).length <= 1) {
          onEvent('At least one destination is required.', 'info');
          return;
        }
        const exists = normalized.destinations.some(
          (destination) => destination.id === destinationId,
        );
        if (!exists) {
          onEvent('Destination no longer exists. Reload and try again.', 'error');
          return;
        }

        const nextDestinations = normalized.destinations
          .filter((destination) => destination.id !== destinationId)
          .map((destination) => ({
            ...destination,
            replicate_to: (destination.replicate_to || []).filter(
              (destId) => destId !== destinationId,
            ),
          }));

        const nextPrimary = nextDestinations[0];
        if (!nextPrimary) {
          onEvent('At least one destination is required.', 'error');
          return;
        }
        const currentPrimaryId = normalized.destinations[0]?.id || 'default';
        const remainingWatched = normalized.watched.filter((watched) => {
          const currentDestinationId = resolveWatchedDestinationId(watched, currentPrimaryId);
          return currentDestinationId !== destinationId;
        });
        const sourceKeysCovered = new Set(
          remainingWatched.map((watched) => sourceKey(watched.path, watched.kind ?? 'Directory')),
        );
        const backfilledWatched = normalized.watched
          .filter((watched) => {
            const currentDestinationId = resolveWatchedDestinationId(watched, currentPrimaryId);
            if (currentDestinationId !== destinationId) return false;
            const key = sourceKey(watched.path, watched.kind ?? 'Directory');
            return !sourceKeysCovered.has(key);
          })
          .map((watched) => ({
            ...watched,
            destination_id: nextPrimary.id,
          }));
        const nextWatched = [...remainingWatched, ...backfilledWatched];

        await persist(
          {
            ...normalized,
            backup_root: nextPrimary.path,
            destinations: nextDestinations,
            watched: nextWatched,
          },
          'Destination removed. Protected paths now use the remaining destinations.',
        );
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        onEvent(`[useMinimalActions] Failed to remove destination: ${reason}`, 'error');
      }
    },
    [cfg, onEvent, persist],
  );

  const addPath = useCallback(
    async (kind: 'File' | 'Directory') => {
      if (!cfg || !primaryId) return;
      try {
        await pickers.pickPathForDest(primaryId, kind, async (path, pickedKind, _destId) => {
          try {
            const { cfg: normalized } = ensurePrimaryDestination(cfg);
            const fallbackDestinationId = normalized.destinations[0]?.id || primaryId;
            const destinationIds = normalized.destinations.map((destination) => destination.id);
            const existingDestinationIds = new Set(
              normalized.watched
                .filter(
                  (entry) => entry.path === path && (entry.kind ?? 'Directory') === pickedKind,
                )
                .map((entry) => resolveWatchedDestinationId(entry, fallbackDestinationId)),
            );
            const destinationIdsToAdd = destinationIds.filter(
              (destinationId) => !existingDestinationIds.has(destinationId),
            );
            if (destinationIdsToAdd.length === 0) {
              onEvent('That path is already protected for all destinations.', 'info');
              return;
            }
            const nextWatched = destinationIdsToAdd.map((destinationId) =>
              normalizeWatched(
                { path, kind: pickedKind, enabled: true, destination_id: destinationId },
                fallbackDestinationId,
                normalized.max_backups_per_file,
              ),
            );
            const next: Config = {
              ...normalized,
              watched: [...normalized.watched, ...nextWatched],
            };
            await persist(
              next,
              `Added ${path} to ${destinationCountLabel(destinationIdsToAdd.length)}.`,
            );
          } catch (innerError) {
            const reason = innerError instanceof Error ? innerError.message : String(innerError);
            onEvent(`[useMinimalActions] Failed to add path: ${reason}`, 'error');
          }
        });
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        onEvent(`[useMinimalActions] Path picker failed: ${reason}`, 'error');
      }
    },
    [cfg, onEvent, persist, pickers, primaryId],
  );

  const addFolder = useCallback(async () => {
    await addPath('Directory');
  }, [addPath]);

  const addFile = useCallback(async () => {
    await addPath('File');
  }, [addPath]);

  const changePath = useCallback(
    async (path: string, kind: 'File' | 'Directory') => {
      if (!cfg || !primaryId) return;
      try {
        await pickers.pickPathForDest(primaryId, kind, async (pickedPath, pickedKind) => {
          if (pickedPath === path) return;
          const duplicate = cfg.watched.some(
            (watched) =>
              watched.path === pickedPath && (watched.kind ?? 'Directory') === pickedKind,
          );
          if (duplicate) {
            onEvent('That path is already protected.', 'info');
            return;
          }
          const watched = cfg.watched.map((entry) =>
            entry.path === path && (entry.kind ?? 'Directory') === kind
              ? { ...entry, path: pickedPath, kind: pickedKind }
              : entry,
          );
          await persist({ ...cfg, watched }, `Protected path changed to ${pickedPath}.`);
        });
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        onEvent(`[useMinimalActions] Failed to change protected path: ${reason}`, 'error');
      }
    },
    [cfg, onEvent, persist, pickers, primaryId],
  );

  const removePath = useCallback(
    async (path: string, kind: 'File' | 'Directory', _sourceDestinationId: string) => {
      if (!cfg || !primaryId) return;
      const next: Config = {
        ...cfg,
        watched: cfg.watched.filter(
          (watched) => watched.path !== path || (watched.kind ?? 'Directory') !== kind,
        ),
      };
      await persist(next, `Removed ${path} from protection.`);
    },
    [cfg, persist, primaryId],
  );

  const updateKeep = useCallback(
    async (
      path: string,
      kind: 'File' | 'Directory',
      _sourceDestinationId: string,
      keep: number,
    ) => {
      if (!cfg || !primaryId) return;
      const nextKeep = Math.max(0, Math.min(1000, keep));
      const nextWatched = cfg.watched.map((w) => {
        if (w.path !== path || (w.kind ?? 'Directory') !== kind) {
          return w;
        }
        return {
          ...normalizeWatched(w, primaryId, cfg.max_backups_per_file),
          max_backups_per_file: nextKeep,
        };
      });
      await persist({ ...cfg, watched: nextWatched }, `Backups to keep updated for ${path}.`);
    },
    [cfg, persist, primaryId],
  );

  const updateDestination = useCallback(
    async (
      path: string,
      kind: 'File' | 'Directory',
      sourceDestinationId: string,
      destinationId: string,
    ) => {
      if (!cfg || !primaryId) return;
      const hasDestination = (cfg.destinations || []).some(
        (destination) => destination.id === destinationId,
      );
      if (!hasDestination) {
        onEvent('Destination no longer exists. Reload and try again.', 'error');
        return;
      }
      if (sourceDestinationId === destinationId) return;

      const duplicateAtTarget = cfg.watched.some((w) => {
        const currentDestinationId = resolveWatchedDestinationId(w, primaryId);
        return (
          w.path === path &&
          (w.kind ?? 'Directory') === kind &&
          currentDestinationId === destinationId
        );
      });
      if (duplicateAtTarget) {
        const nextWatched = cfg.watched.filter((w) => {
          const currentDestinationId = resolveWatchedDestinationId(w, primaryId);
          return !(
            w.path === path &&
            (w.kind ?? 'Directory') === kind &&
            currentDestinationId === sourceDestinationId
          );
        });
        await persist(
          { ...cfg, watched: nextWatched },
          `Moved ${path} to the selected destination.`,
        );
        onEvent(
          'Path already exists at that destination, so the source destination entry was removed.',
          'info',
        );
        return;
      }

      const nextWatched = cfg.watched.map((w) => {
        const currentDestinationId = resolveWatchedDestinationId(w, primaryId);
        if (
          w.path !== path ||
          (w.kind ?? 'Directory') !== kind ||
          currentDestinationId !== sourceDestinationId
        ) {
          return w;
        }
        return {
          ...normalizeWatched(w, primaryId, cfg.max_backups_per_file),
          destination_id: destinationId,
        };
      });
      await persist({ ...cfg, watched: nextWatched }, `Moved ${path} to the selected destination.`);
    },
    [cfg, onEvent, persist, primaryId],
  );

  return {
    chooseDestination,
    addDestination,
    removeDestination,
    addFolder,
    addFile,
    changePath,
    removePath,
    updateKeep,
    updateDestination,
  };
}
