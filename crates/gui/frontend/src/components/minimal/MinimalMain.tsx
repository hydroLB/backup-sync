import { useEffect, useMemo, useState } from 'react';
import { loadConfig, saveConfig } from '../../services/config';
import { getLogTail } from '../../services/logs';
import { safeInvoke, wrapError } from '../../services/ipc';
import { Config, Destination, WatchedPath } from '../settings/types';
import { usePickers } from '../settings/hooks/usePickers';
import { RestoreModal } from './RestoreModal';

type EventKind = 'ok' | 'error' | 'info';

/**
 * Purpose: Toggle safe mode using the backend command so the daemon is updated when reachable.
 *
 * Inputs: Desired safe mode value.
 * Outputs: The resulting safe mode value.
 * Ties to: Start/Stop actions on the minimal main screen.
 * Side effects: Writes config and may update the running daemon via IPC.
 * Why: Keep Start/Stop semantics simple while avoiding daemon restarts when possible.
 */
async function setSafeMode(desired: boolean): Promise<boolean> {
  try {
    return await safeInvoke<boolean>('toggle_safe_mode_cmd', { desired });
  } catch (error) {
    throw wrapError('[setSafeMode] Failed to toggle safe mode', error);
  }
}

/**
 * Purpose: Convert the configured interval seconds into a UI-safe minute value.
 *
 * Inputs: `seconds` as the config interval in seconds.
 * Outputs: A minute count suitable for display and editing.
 * Side effects: None.
 * Error handling: Returns a safe default when input is invalid.
 * Ties to other methods: Used by `commitIntervalMinutes` and the schedule input.
 * Why this exists: Keep UI behavior stable even when config contains non-minute values.
 */
function intervalSecondsToMinutes(seconds: number): number {
  if (!Number.isFinite(seconds) || seconds <= 0) return 30;
  return Math.max(1, Math.ceil(seconds / 60));
}

/**
 * Purpose: Clamp a user-provided interval minutes value to backend guardrails.
 *
 * Inputs: `minutes` as a user-provided minute value.
 * Outputs: A clamped integer minute value.
 * Side effects: None.
 * Error handling: Normalizes non-finite input to a safe default.
 * Ties to other methods: Used by `commitIntervalMinutes`.
 * Why this exists: Prevent saves that fail validation or create busy loops.
 */
function clampIntervalMinutes(minutes: number): number {
  const min = 1;
  const max = 60 * 24 * 30; // 30 days, matches backend `max_interval_seconds`.
  if (!Number.isFinite(minutes)) return 30;
  return Math.max(min, Math.min(max, Math.round(minutes)));
}

function ensurePrimaryDestination(cfg: Config): { cfg: Config; dest: Destination } {
  if (cfg.destinations.length > 0) {
    return { cfg, dest: cfg.destinations[0]! };
  }
  const dest: Destination = { id: 'default', path: cfg.backup_root, label: 'Primary' };
  return { cfg: { ...cfg, destinations: [dest] }, dest };
}

function normalizeWatched(w: WatchedPath, destId: string, keepDefault: number): WatchedPath {
  const kind = w.kind ?? 'Directory';
  return {
    ...w,
    kind,
    enabled: w.enabled ?? true,
    destination_id: w.destination_id ?? destId,
    max_backups_per_file: w.max_backups_per_file ?? keepDefault,
  };
}

/**
 * Purpose: Render the spec-minimal main screen for the backup app.
 *
 * Inputs: a callback that receives user-visible events.
 * Outputs: A React element tree.
 * Ties to: config load/save, run-now, restore, and log-tail IPC commands.
 * Side effects: Invokes IPC calls and updates configuration.
 * Why: Implements the simplified UX required by the product spec.
 */
export function MinimalMain({ onEvent }: { onEvent: (msg: string, kind?: EventKind) => void }) {
  const [cfg, setCfg] = useState<Config | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [showLog, setShowLog] = useState(false);
  const [logTail, setLogTail] = useState<string>('');
  const [restoreOpen, setRestoreOpen] = useState(false);
  const [intervalMinutesDraft, setIntervalMinutesDraft] = useState<string>('');
  const [intervalEditing, setIntervalEditing] = useState(false);

  const pickers = usePickers((msg) => onEvent(msg, 'info'));

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    loadConfig()
      .then((loaded) => {
        if (cancelled) return;
        const { cfg: next } = ensurePrimaryDestination(loaded);
        setCfg(next);
      })
      .catch((e) => {
        const reason = e instanceof Error ? e.message : String(e);
        onEvent(`[MinimalMain] Failed to load config: ${reason}`, 'error');
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
    if (intervalEditing) return;
    setIntervalMinutesDraft(String(intervalSecondsToMinutes(cfg.interval_seconds)));
  }, [cfg, intervalEditing]);

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

  const persist = async (next: Config) => {
    try {
      setBusy(true);
      await saveConfig(next);
      setCfg(next);
      onEvent('Saved.', 'ok');
    } catch (e) {
      const reason = e instanceof Error ? e.message : String(e);
      onEvent(`Save failed: ${reason}`, 'error');
    } finally {
      setBusy(false);
    }
  };

  /**
   * Purpose: Persist a new backup interval schedule in minutes.
   *
   * Inputs: Minute value as a string from the UI input.
   * Outputs: Updates config state and persists to disk via IPC.
   * Side effects: Writes config and updates local React state.
   * Error handling: Reverts the UI field and emits a user-facing error event.
   * Ties to other methods: Calls `persist` to save the derived `interval_seconds`.
   * Why this exists: Keep the schedule editable without exposing raw seconds to users.
   */
  const commitIntervalMinutes = async (draft: string) => {
    if (!cfg) return;
    const trimmed = draft.trim();
    if (trimmed.length === 0) {
      setIntervalMinutesDraft(String(intervalSecondsToMinutes(cfg.interval_seconds)));
      return;
    }
    const parsed = Number(trimmed);
    if (!Number.isFinite(parsed)) {
      onEvent('Interval must be a number of minutes.', 'error');
      setIntervalMinutesDraft(String(intervalSecondsToMinutes(cfg.interval_seconds)));
      return;
    }
    const clamped = clampIntervalMinutes(parsed);
    setIntervalMinutesDraft(String(clamped));
    const nextSeconds = clamped * 60;
    if (nextSeconds === cfg.interval_seconds) return;
    await persist({ ...cfg, interval_seconds: nextSeconds });
  };

  /**
   * Purpose: Toggle the daemon running state via safe mode semantics.
   *
   * Inputs: `running` desired running state.
   * Outputs: Updates local config state and emits a status event.
   * Side effects: Invokes IPC to update safe mode and updates React state.
   * Error handling: Emits a precise failure message and preserves current state.
   * Ties to other methods: Uses `setSafeMode` to update the running daemon when reachable.
   * Why this exists: Present a single running toggle instead of Start/Stop action buttons.
   */
  const setRunning = async (running: boolean) => {
    if (!cfg) return;
    try {
      setBusy(true);
      const desiredSafeMode = !running;
      const nextSafeMode = await setSafeMode(desiredSafeMode);
      const next: Config = { ...cfg, safe_mode: nextSafeMode };
      setCfg(next);
      onEvent(nextSafeMode ? 'Paused.' : 'Running.', 'ok');
    } catch (e) {
      const reason = e instanceof Error ? e.message : String(e);
      onEvent(`Failed to change running state: ${reason}`, 'error');
    } finally {
      setBusy(false);
    }
  };

  const chooseDestination = async () => {
    if (!cfg) return;
    const { cfg: normalized, dest } = ensurePrimaryDestination(cfg);
    if (normalized !== cfg) setCfg(normalized);
    await pickers.pickDestination(async (path) => {
      const next: Config = {
        ...normalized,
        backup_root: path,
        destinations: [{ ...dest, path }],
      };
      await persist(next);
    });
  };

  const addFolder = async () => {
    if (!cfg || !primary) return;
    await pickers.pickPathForDest(primary.id, 'Directory', async (path, kind, destId) => {
      try {
        const nextWatched: WatchedPath = normalizeWatched(
          { path, kind, enabled: true, destination_id: destId ?? primary.id },
          primary.id,
          cfg.max_backups_per_file,
        );
        const next: Config = { ...cfg, watched: [...cfg.watched, nextWatched] };
        await persist(next);
      } catch (e) {
        const reason = e instanceof Error ? e.message : String(e);
        onEvent(`[MinimalMain] Failed to add folder: ${reason}`, 'error');
      }
    });
  };

  const removeFolder = async (path: string) => {
    if (!cfg) return;
    const next: Config = { ...cfg, watched: cfg.watched.filter((w) => w.path !== path) };
    await persist(next);
  };

  const updateKeep = async (path: string, keep: number) => {
    if (!cfg || !primary) return;
    const nextKeep = Math.max(0, Math.min(1000, keep));
    const nextWatched = cfg.watched.map((w) => {
      if (w.path !== path) return w;
      return {
        ...normalizeWatched(w, primary.id, cfg.max_backups_per_file),
        max_backups_per_file: nextKeep,
      };
    });
    await persist({ ...cfg, watched: nextWatched });
  };

  const refreshLog = async () => {
    try {
      setBusy(true);
      const tail = await getLogTail();
      setLogTail(tail);
    } catch (e) {
      const reason = e instanceof Error ? e.message : String(e);
      onEvent(`Failed to load log tail: ${reason}`, 'error');
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    if (!showLog) return;
    refreshLog();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showLog]);

  if (loading || !cfg || !primary) {
    return (
      <div className="app">
        <div className="hero">
          <h1>Local Backup Manager</h1>
          <p>Loading…</p>
        </div>
      </div>
    );
  }

  return (
    <div className="app">
      <div
        className="hero"
        style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}
      >
        <div style={{ display: 'grid', gap: 4 }}>
          <h1>Local Backup Manager</h1>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 12, alignItems: 'center' }}>
            <label className="toggle">
              <input
                type="checkbox"
                checked={!cfg.safe_mode}
                disabled={busy}
                aria-label="Running"
                onChange={(e) => setRunning(e.target.checked)}
              />
              <span className="toggle-track" aria-hidden="true">
                <span className="toggle-thumb" />
              </span>
              <span style={{ color: 'var(--muted)', fontSize: 12 }}>
                {cfg.safe_mode ? 'Paused' : 'Running'}
              </span>
            </label>
            <div className="pill" style={{ width: 'fit-content' }} title="Backup schedule interval">
              Every
              <input
                type="number"
                min={1}
                max={60 * 24 * 30}
                value={intervalMinutesDraft}
                disabled={busy}
                aria-label="Backup interval minutes"
                onFocus={() => setIntervalEditing(true)}
                onBlur={() => {
                  setIntervalEditing(false);
                  commitIntervalMinutes(intervalMinutesDraft);
                }}
                onKeyDown={(e) => {
                  if (e.key !== 'Enter') return;
                  setIntervalEditing(false);
                  commitIntervalMinutes(intervalMinutesDraft);
                }}
                onChange={(e) => setIntervalMinutesDraft(e.target.value)}
                style={{
                  width: 84,
                  height: 28,
                  padding: '0 8px',
                  borderRadius: 10,
                  border: '1px solid var(--border)',
                  background: 'rgba(255,255,255,0.06)',
                  color: 'var(--text)',
                  fontSize: 13,
                  textAlign: 'center',
                }}
              />
              min
            </div>
          </div>
        </div>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <button
            className="btn"
            onClick={() => setRestoreOpen(true)}
            disabled={busy || watchedDirs.length === 0}
            title="Pick a folder and version to restore."
          >
            Restore version
          </button>
          <button className="btn secondary" onClick={() => setShowLog((v) => !v)} disabled={busy}>
            {showLog ? 'Hide log' : 'Show log'}
          </button>
        </div>
      </div>

      <div className="grid" style={{ gridTemplateColumns: '1fr', gap: 10 }}>
        <div className="card">
          <h3 style={{ marginTop: 0 }}>Destination</h3>
          <div className="pill" title={primary.path} style={{ justifyContent: 'space-between' }}>
            <span
              style={{
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
                maxWidth: 720,
              }}
            >
              {primary.path || 'Choose a destination'}
            </span>
            <button className="btn secondary" onClick={chooseDestination} disabled={busy}>
              Choose…
            </button>
          </div>
        </div>

        <div className="card">
          <div
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              gap: 10,
            }}
          >
            <h3 style={{ marginTop: 0, marginBottom: 0 }}>Folders</h3>
            <button className="btn secondary" onClick={addFolder} disabled={busy}>
              Add folder…
            </button>
          </div>
          {watchedDirs.length === 0 ? (
            <p style={{ marginTop: 10, color: 'var(--muted)', fontSize: 13 }}>
              Add one or more folders to back up.
            </p>
          ) : (
            <div style={{ display: 'grid', gap: 10, marginTop: 10 }}>
              {watchedDirs.map((w) => (
                <div
                  key={w.path}
                  style={{
                    display: 'grid',
                    gridTemplateColumns: '1fr 220px 96px',
                    gap: 8,
                    alignItems: 'center',
                  }}
                >
                  <div className="pill" title={w.path}>
                    <span
                      style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
                    >
                      {w.path}
                    </span>
                  </div>
                  <div style={{ display: 'grid', gap: 4 }}>
                    <span style={{ color: 'var(--muted)', fontSize: 11 }}>Backups to keep</span>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                      <button
                        className="btn secondary"
                        type="button"
                        onClick={() =>
                          updateKeep(
                            w.path,
                            (w.max_backups_per_file ?? cfg.max_backups_per_file) - 1,
                          )
                        }
                        disabled={busy}
                        style={{ minWidth: 44, height: 38, padding: 0 }}
                        aria-label="Decrease backups to keep"
                      >
                        -
                      </button>
                      <input
                        type="number"
                        min={0}
                        max={1000}
                        value={w.max_backups_per_file ?? cfg.max_backups_per_file}
                        onChange={(e) => {
                          const n = Number(e.target.value);
                          if (!Number.isFinite(n)) return;
                          updateKeep(w.path, n);
                        }}
                        disabled={busy}
                        style={{
                          width: 92,
                          height: 38,
                          padding: '0 10px',
                          borderRadius: 10,
                          border: '1px solid var(--border)',
                          background: 'rgba(255,255,255,0.06)',
                          color: 'var(--text)',
                          fontSize: 16,
                          textAlign: 'center',
                          appearance: 'textfield',
                          MozAppearance: 'textfield',
                        }}
                      />
                      <button
                        className="btn secondary"
                        type="button"
                        onClick={() =>
                          updateKeep(
                            w.path,
                            (w.max_backups_per_file ?? cfg.max_backups_per_file) + 1,
                          )
                        }
                        disabled={busy}
                        style={{ minWidth: 44, height: 38, padding: 0 }}
                        aria-label="Increase backups to keep"
                      >
                        +
                      </button>
                    </div>
                  </div>
                  <button
                    className="btn secondary"
                    onClick={() => removeFolder(w.path)}
                    disabled={busy}
                  >
                    Remove
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>

        {showLog && (
          <div className="card">
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                gap: 10,
              }}
            >
              <h3 style={{ marginTop: 0, marginBottom: 0 }}>Log</h3>
              <button className="btn secondary" onClick={refreshLog} disabled={busy}>
                Refresh
              </button>
            </div>
            <pre
              style={{
                marginTop: 10,
                marginBottom: 0,
                padding: 10,
                borderRadius: 8,
                border: '1px solid var(--border)',
                background: 'rgba(0,0,0,0.25)',
                maxHeight: 260,
                overflow: 'auto',
                fontSize: 12,
              }}
            >
              {logTail || '(no logs yet)'}
            </pre>
          </div>
        )}
      </div>

      <RestoreModal open={restoreOpen} onClose={() => setRestoreOpen(false)} onEvent={onEvent} />
    </div>
  );
}
