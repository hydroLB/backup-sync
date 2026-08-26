import { useCallback, useEffect, useMemo, useState } from 'react';
import { ConfigSaveResult, loadConfig, saveConfig } from '../../../services/config';
import { Config, Destination, WatchedPath } from '../../../domain/config';
import { ensurePrimaryDestination, normalizeWatched } from '../helpers/config';

type EventKind = 'ok' | 'error' | 'info';

type Events = {
  onEvent: (msg: string, kind?: EventKind) => void;
};

export type PersistOptions = {
  globalBusy?: boolean;
};

type MinimalConfigState = {
  cfg: Config | null;
  primary: Destination | null;
  destinations: Destination[];
  watchedDirs: WatchedPath[];
  watchedItems: WatchedPath[];
  loading: boolean;
  loadError: string | null;
  busy: boolean;
  setBusy: (busy: boolean) => void;
  setCfg: (cfg: Config | null) => void;
  reload: () => void;
  persist: (next: Config, successMessage?: string, options?: PersistOptions) => Promise<void>;
};

/** Keep the minimal screen orchestration small and ensure config normalization is consistent. */
export function useMinimalConfig({ onEvent }: Events): MinimalConfigState {
  const [cfg, setCfg] = useState<Config | null>(null);
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [loadNonce, setLoadNonce] = useState(0);
  const [busy, setBusy] = useState(false);

  const reportSaveOutcome = useCallback(
    (result: ConfigSaveResult) => {
      if (result.daemon_restart_warning) {
        onEvent(result.daemon_restart_warning, 'error');
      }
    },
    [onEvent],
  );

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setLoadError(null);
    loadConfig()
      .then(async (loaded) => {
        if (cancelled) return;
        const { cfg: withPrimary } = ensurePrimaryDestination(loaded);
        setCfg(withPrimary);
      })
      .catch((error) => {
        const reason = error instanceof Error ? error.message : String(error);
        if (!cancelled) setLoadError(`Failed to load configuration: ${reason}`);
        onEvent(`[useMinimalConfig] Failed to load config: ${reason}`, 'error');
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [loadNonce, onEvent, reportSaveOutcome]);

  const reload = useCallback(() => {
    setLoadNonce((previous) => previous + 1);
  }, []);

  useEffect(() => {
    if (!cfg) return;
    const { cfg: normalizedPrimary } = ensurePrimaryDestination(cfg);
    if (normalizedPrimary !== cfg) setCfg(normalizedPrimary);
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

  /** Save small inline controls without making unrelated sections look unavailable. */
  const persist = async (next: Config, successMessage = 'Saved.', options: PersistOptions = {}) => {
    const globalBusy = options.globalBusy ?? true;
    try {
      if (globalBusy) setBusy(true);
      const { cfg: normalized } = ensurePrimaryDestination(next);
      const result = await saveConfig(normalized);
      setCfg(normalized);
      onEvent(successMessage, 'ok');
      reportSaveOutcome(result);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      onEvent(`Save failed: ${reason}`, 'error');
    } finally {
      if (globalBusy) setBusy(false);
    }
  };

  return {
    cfg,
    primary,
    destinations,
    watchedDirs,
    watchedItems,
    loading,
    loadError,
    busy,
    setBusy,
    setCfg,
    reload,
    persist,
  };
}
