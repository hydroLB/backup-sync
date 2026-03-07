import { useEffect, useMemo, useState } from 'react';
import { loadConfig, saveConfig } from '../../../services/config';
import { Config, Destination, WatchedPath } from '../../../domain/config';
import {
  enforceFixedAutomaticInterval,
  ensurePrimaryDestination,
  normalizeWatched,
} from '../helpers/config';

type EventKind = 'ok' | 'error' | 'info';

type Events = {
  onEvent: (msg: string, kind?: EventKind) => void;
};

type MinimalConfigState = {
  cfg: Config | null;
  primary: Destination | null;
  destinations: Destination[];
  watchedDirs: WatchedPath[];
  watchedItems: WatchedPath[];
  loading: boolean;
  busy: boolean;
  setBusy: (busy: boolean) => void;
  setCfg: (cfg: Config | null) => void;
  persist: (next: Config, successMessage?: string) => Promise<void>;
};

/**
 * Summary: Load, normalize, and persist config for the minimal UI.
 *
 * Inputs: `onEvent` handler for user-visible status and error messages.
 *
 * Outputs: Config state plus derived primary destination and watched directory list.
 *
 * Side effects: Reads and writes config via IPC-backed services.
 *
 * Error handling: Emits actionable error messages via `onEvent`.
 *
 * Ties to other methods: Used by `MinimalMain` and action hooks to centralize config handling.
 *
 * Why this exists: Keep the minimal screen orchestration small and ensure config normalization is consistent.
 */
export function useMinimalConfig({ onEvent }: Events): MinimalConfigState {
  const [cfg, setCfg] = useState<Config | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    loadConfig()
      .then(async (loaded) => {
        if (cancelled) return;
        const { cfg: withPrimary } = ensurePrimaryDestination(loaded);
        const next = enforceFixedAutomaticInterval(withPrimary);
        setCfg(next);
        if (next.interval_seconds === loaded.interval_seconds) return;
        try {
          await saveConfig(next);
        } catch (error) {
          const reason = error instanceof Error ? error.message : String(error);
          onEvent(`[useMinimalConfig] Failed to enforce automatic schedule: ${reason}`, 'error');
        }
      })
      .catch((error) => {
        const reason = error instanceof Error ? error.message : String(error);
        onEvent(`[useMinimalConfig] Failed to load config: ${reason}`, 'error');
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [onEvent]);

  useEffect(() => {
    if (!cfg) return;
    const { cfg: normalizedPrimary } = ensurePrimaryDestination(cfg);
    const normalized = enforceFixedAutomaticInterval(normalizedPrimary);
    if (normalized !== cfg) setCfg(normalized);
  }, [cfg]);

  const primary = useMemo(() => {
    if (!cfg) return null;
    return ensurePrimaryDestination(cfg).dest;
  }, [cfg]);

  const watchedDirs = useMemo(() => {
    if (!cfg || !primary) return [];
    return cfg.watched
      .filter((w) => (w.kind ?? 'Directory') === 'Directory')
      .map((w) => normalizeWatched(w, primary.id, cfg.max_backups_per_file));
  }, [cfg, primary]);

  const destinations = useMemo(() => {
    if (!cfg) return [];
    return cfg.destinations ?? [];
  }, [cfg]);

  const watchedItems = useMemo(() => {
    if (!cfg || !primary) return [];
    return cfg.watched.map((w) => normalizeWatched(w, primary.id, cfg.max_backups_per_file));
  }, [cfg, primary]);

  /**
   * Summary: Persist a full config object and update local state.
   *
   * Inputs: `next` config to write.
   *
   * Outputs: None.
   *
   * Side effects: Writes config to disk via IPC and updates local React state.
   *
   * Error handling: Emits a user-facing save error via `onEvent`.
   *
   * Ties to other methods: Called by schedule, destination, folder, and running handlers.
   *
   * Why this exists: Keep save behavior uniform and ensure busy state is applied consistently.
   */
  const persist = async (next: Config, successMessage = 'Saved.') => {
    try {
      setBusy(true);
      const normalized = enforceFixedAutomaticInterval(next);
      await saveConfig(normalized);
      setCfg(normalized);
      onEvent(successMessage, 'ok');
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      onEvent(`Save failed: ${reason}`, 'error');
    } finally {
      setBusy(false);
    }
  };

  return {
    cfg,
    primary,
    destinations,
    watchedDirs,
    watchedItems,
    loading,
    busy,
    setBusy,
    setCfg,
    persist,
  };
}
