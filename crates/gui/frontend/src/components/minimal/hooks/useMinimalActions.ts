import { useCallback } from 'react';
import { Config, WatchedPath } from '../../settings/types';
import { ensurePrimaryDestination, normalizeWatched } from '../helpers/config';

type EventKind = 'ok' | 'error' | 'info';

type Pickers = {
  pickDestinationPath: () => Promise<string | null>;
  pickPathForDest: (
    destId: string,
    kind: 'File' | 'Directory',
    addWatched: (path: string, kind: 'File' | 'Directory', destId?: string) => void,
  ) => Promise<void>;
};

type Params = {
  cfg: Config | null;
  primaryId: string | null;
  persist: (next: Config) => Promise<void>;
  pickers: Pickers;
  onEvent: (msg: string, kind?: EventKind) => void;
};

type MinimalActions = {
  chooseDestination: () => Promise<void>;
  addDestination: () => Promise<void>;
  removeDestination: (destinationId: string) => Promise<void>;
  addFolder: () => Promise<void>;
  addFile: () => Promise<void>;
  removePath: (path: string, kind: 'File' | 'Directory', sourceDestinationId: string) => Promise<void>;
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

/**
 * Summary: Resolve watched destination id with a safe fallback.
 *
 * Inputs: Watched entry and fallback destination id.
 * Outputs: Destination id that should be used for matching.
 * Side effects: None.
 * Error handling: None.
 * Ties to other methods: Used by add, remove, and update handlers for exact row targeting.
 * Why this exists: Older configs can omit destination ids and still need deterministic behavior.
 */
function resolveWatchedDestinationId(
  watched: WatchedPath,
  fallbackDestinationId: string,
): string {
  return watched.destination_id ?? fallbackDestinationId;
}

/**
 * Summary: Build a stable source key for watched entries ignoring destination assignment.
 *
 * Inputs: Source path and watched kind.
 * Outputs: Source key string.
 * Side effects: None.
 * Error handling: None.
 * Ties to other methods: Used when cloning watched items across destinations.
 * Why this exists: A source path should be uniquely tracked per kind when expanding to all destinations.
 */
function sourceKey(path: string, kind: 'File' | 'Directory'): string {
  return `${kind}:${path}`;
}

/**
 * Summary: Build a unique destination id for newly added destinations.
 *
 * Inputs: Config containing the existing destination list.
 * Outputs: Collision-safe destination id.
 * Side effects: None.
 * Error handling: Falls back to a timestamp suffix if deterministic ids are exhausted.
 * Ties to other methods: Used by `addDestination`.
 * Why this exists: Keep destination creation deterministic and avoid id conflicts.
 */
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

/**
 * Summary: Build minimal-mode action handlers that mutate config.
 *
 * Inputs: Current config, primary destination id, persist function, picker service, and event handler.
 * Outputs: A set of stable async handlers for destination selection and watched folder management.
 * Side effects: Opens native pickers and persists config updates via IPC services.
 * Error handling: Emits user-visible errors via `onEvent`.
 * Ties to other methods: Used by `MinimalMain` to wire section components without inline handler bodies.
 * Why this exists: Centralize config mutation logic so UI components remain presentational.
 */
export function useMinimalActions({ cfg, primaryId, persist, pickers, onEvent }: Params): MinimalActions {
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
      await persist(next);
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
      await persist({
        ...normalized,
        destinations: nextDestinations,
        watched: [...normalized.watched, ...additions],
      });
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
        const exists = normalized.destinations.some((destination) => destination.id === destinationId);
        if (!exists) {
          onEvent('Destination no longer exists. Reload and try again.', 'error');
          return;
        }

        const nextDestinations = normalized.destinations
          .filter((destination) => destination.id !== destinationId)
          .map((destination) => ({
            ...destination,
            replicate_to: (destination.replicate_to || []).filter((destId) => destId !== destinationId),
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

        await persist({
          ...normalized,
          backup_root: nextPrimary.path,
          destinations: nextDestinations,
          watched: nextWatched,
        });
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        onEvent(`[useMinimalActions] Failed to remove destination: ${reason}`, 'error');
      }
    },
    [cfg, onEvent, persist],
  );

  const addPath = useCallback(async (kind: 'File' | 'Directory') => {
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
          const next: Config = { ...normalized, watched: [...normalized.watched, ...nextWatched] };
          await persist(next);
        } catch (innerError) {
          const reason = innerError instanceof Error ? innerError.message : String(innerError);
          onEvent(`[useMinimalActions] Failed to add path: ${reason}`, 'error');
        }
      });
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      onEvent(`[useMinimalActions] Path picker failed: ${reason}`, 'error');
    }
  }, [cfg, onEvent, persist, pickers, primaryId]);

  const addFolder = useCallback(async () => {
    await addPath('Directory');
  }, [addPath]);

  const addFile = useCallback(async () => {
    await addPath('File');
  }, [addPath]);

  const removePath = useCallback(
    async (path: string, kind: 'File' | 'Directory', sourceDestinationId: string) => {
      if (!cfg || !primaryId) return;
      const next: Config = {
        ...cfg,
        watched: cfg.watched.filter((w) => {
          const currentDestinationId = resolveWatchedDestinationId(w, primaryId);
          return !(
            w.path === path &&
            (w.kind ?? 'Directory') === kind &&
            currentDestinationId === sourceDestinationId
          );
        }),
      };
      await persist(next);
    },
    [cfg, persist, primaryId],
  );

  const updateKeep = useCallback(
    async (path: string, kind: 'File' | 'Directory', sourceDestinationId: string, keep: number) => {
      if (!cfg || !primaryId) return;
      const nextKeep = Math.max(0, Math.min(1000, keep));
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
          max_backups_per_file: nextKeep,
        };
      });
      await persist({ ...cfg, watched: nextWatched });
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
        await persist({ ...cfg, watched: nextWatched });
        onEvent('Path already exists at that destination, so the source destination entry was removed.', 'info');
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
      await persist({ ...cfg, watched: nextWatched });
    },
    [cfg, onEvent, persist, primaryId],
  );

  return {
    chooseDestination,
    addDestination,
    removeDestination,
    addFolder,
    addFile,
    removePath,
    updateKeep,
    updateDestination,
  };
}
