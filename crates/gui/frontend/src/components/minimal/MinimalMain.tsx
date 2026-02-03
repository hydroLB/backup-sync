import { useEffect, useMemo, useState } from 'react';
import { loadConfig, saveConfig } from '../../services/config';
import { runNow } from '../../services/backup';
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
    const nextWatched = cfg.watched.map((w) => {
      if (w.path !== path) return w;
      return {
        ...normalizeWatched(w, primary.id, cfg.max_backups_per_file),
        max_backups_per_file: keep,
      };
    });
    await persist({ ...cfg, watched: nextWatched });
  };

  const toggleStartStop = async () => {
    if (!cfg) return;
    try {
      setBusy(true);
      const desired = !cfg.safe_mode;
      const nextValue = await setSafeMode(desired);
      const next: Config = { ...cfg, safe_mode: nextValue };
      setCfg(next);
      onEvent(nextValue ? 'Paused (safe mode enabled).' : 'Running (safe mode disabled).', 'ok');
    } catch (e) {
      const reason = e instanceof Error ? e.message : String(e);
      onEvent(`Start/Stop failed: ${reason}`, 'error');
    } finally {
      setBusy(false);
    }
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

  const backupNow = async () => {
    try {
      setBusy(true);
      await runNow();
      onEvent('Backup complete.', 'ok');
    } catch (e) {
      const reason = e instanceof Error ? e.message : String(e);
      onEvent(`Backup failed: ${reason}`, 'error');
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

  const canBackup = watchedDirs.length > 0 && !!primary.path && !cfg.safe_mode;

  return (
    <div className="app">
      <div
        className="hero"
        style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}
      >
        <div style={{ display: 'grid', gap: 4 }}>
          <h1>Local Backup Manager</h1>
          <div className="pill" style={{ width: 'fit-content' }}>
            {cfg.safe_mode ? 'Paused' : 'Running'} • Every {Math.round(cfg.interval_seconds / 60)}{' '}
            min
          </div>
        </div>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
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
                    gridTemplateColumns: '1fr 140px 96px',
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
                  <label style={{ display: 'grid', gap: 4 }}>
                    <span style={{ color: 'var(--muted)', fontSize: 11 }}>Backups to keep</span>
                    <input
                      type="number"
                      min={1}
                      max={1000}
                      value={w.max_backups_per_file ?? cfg.max_backups_per_file}
                      onChange={(e) =>
                        updateKeep(w.path, Number(e.target.value || cfg.max_backups_per_file))
                      }
                      disabled={busy}
                    />
                  </label>
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

        <div className="card">
          <h3 style={{ marginTop: 0 }}>Actions</h3>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 8 }}>
            <button className="btn secondary" onClick={toggleStartStop} disabled={busy}>
              {cfg.safe_mode ? 'Start' : 'Stop'}
            </button>
            <button className="btn" onClick={backupNow} disabled={busy || !canBackup}>
              Back up now
            </button>
            <button
              className="btn secondary"
              onClick={() => setRestoreOpen(true)}
              disabled={busy || watchedDirs.length === 0}
            >
              Restore…
            </button>
          </div>
          <p style={{ marginTop: 10, marginBottom: 0, color: 'var(--muted)', fontSize: 12 }}>
            Start/Stop uses safe mode to pause writes. Restores can target a new folder or overwrite
            in place.
          </p>
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
