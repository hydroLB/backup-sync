import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { hardeningFingerprint, writeHardeningDone } from '../../utils/hardening';
import { hardeningCheck } from '../../services/system';
import { RestoreModal } from './RestoreModal';
import { useMinimalActions } from './hooks/useMinimalActions';
import { useMinimalConfig } from './hooks/useMinimalConfig';
import { useMinimalRunning } from './hooks/useMinimalRunning';
import { SavePulseScope, useMinimalFeedback } from './hooks/useMinimalFeedback';
import { useMinimalStatus } from './hooks/useMinimalStatus';
import { useMinimalLiveHealth } from './hooks/useMinimalLiveHealth';
import { type PickerBusyScope, useMinimalPickers } from './hooks/useMinimalPickers';
import { removeKeptExtraVersion } from '../../services/safety';
import { MinimalHeader } from './sections/MinimalHeader';
import { DestinationCard } from './sections/DestinationCard';
import { FoldersCard } from './sections/FoldersCard';
import { RestoreCard } from './sections/RestoreCard';
import { ToastMessage } from '../ui/ToastMessage';
import { InlineAlert } from '../ui/InlineAlert';
import { Button } from '../ui/Button';
import { StateBlock } from '../ui/StateBlock';
import { PendingStorageMove, StorageMoveModal } from './StorageMoveModal';
import { relocateDestination } from '../../services/storage';
import { IS_WEB_RUNTIME } from '../../runtime/mode';
import { StorageLocationPickerModal } from './StorageLocationPickerModal';

type EventKind = 'ok' | 'error' | 'info';

/** Keep the primary UX compact while preserving operational controls. */
export function MinimalMain({ onEvent }: { onEvent: (msg: string, kind?: EventKind) => void }) {
  const [restoreOpen, setRestoreOpen] = useState(false);
  const [pickerBusyScope, setPickerBusyScope] = useState<PickerBusyScope>(null);
  const [storagePickerOpen, setStoragePickerOpen] = useState(false);
  const [hardeningBusy, setHardeningBusy] = useState(false);
  const [hardeningOk, setHardeningOk] = useState<boolean | null>(null);
  const [hardeningIssue, setHardeningIssue] = useState<string | null>(null);
  const [hardeningNonce, setHardeningNonce] = useState(0);
  const [dismissedSafetyWarningTs, setDismissedSafetyWarningTs] = useState<number | null>(null);
  const [safetyRemoveBusy, setSafetyRemoveBusy] = useState(false);
  const [pendingStorageMove, setPendingStorageMove] = useState<PendingStorageMove | null>(null);
  const [storageMoveBusy, setStorageMoveBusy] = useState(false);
  const lastHardeningKeyRef = useRef<string | null>(null);
  const hardeningRunIdRef = useRef(0);
  const storagePickerResolverRef = useRef<((path: string | null) => void) | null>(null);

  const requestWebStoragePath = useCallback(
    () =>
      new Promise<string | null>((resolve) => {
        storagePickerResolverRef.current?.(null);
        storagePickerResolverRef.current = resolve;
        setStoragePickerOpen(true);
      }),
    [],
  );
  const finishWebStoragePicker = useCallback((path: string | null) => {
    setStoragePickerOpen(false);
    const resolve = storagePickerResolverRef.current;
    storagePickerResolverRef.current = null;
    resolve?.(path);
  }, []);
  useEffect(
    () => () => {
      storagePickerResolverRef.current?.(null);
      storagePickerResolverRef.current = null;
    },
    [],
  );

  const {
    toast,
    inline,
    savePulseActive,
    savePulseScope,
    pausePulseActive,
    showSavedBadge,
    savedBadgeNonce,
    emitEvent: baseEmitEvent,
    clearToast,
    clearInline,
  } = useMinimalFeedback({ onEvent });
  const pendingSaveScopeRef = useRef<SavePulseScope>('none');
  const emitEvent = useCallback(
    (msg: string, kind: EventKind = 'info') => {
      let scope: SavePulseScope = 'none';
      if (kind === 'ok') {
        if (/\brunning\b/i.test(msg)) {
          scope = 'global';
        } else if (/\brestore(d|)\b/i.test(msg)) {
          scope = 'restore';
        } else if (pendingSaveScopeRef.current !== 'none') {
          scope = pendingSaveScopeRef.current;
        }
      }
      baseEmitEvent(msg, kind, scope);
      if (kind !== 'ok' || scope !== 'none' || /\bpaused\b/i.test(msg)) {
        pendingSaveScopeRef.current = 'none';
      }
    },
    [baseEmitEvent],
  );
  const runWithSaveScope = useCallback(
    async (scope: SavePulseScope, action: () => Promise<void>) => {
      pendingSaveScopeRef.current = scope;
      try {
        await action();
      } finally {
        if (pendingSaveScopeRef.current === scope) {
          pendingSaveScopeRef.current = 'none';
        }
      }
    },
    [],
  );
  const pickers = useMinimalPickers({
    onEvent: (msg) => emitEvent(msg, 'info'),
    onPickerBusyChange: setPickerBusyScope,
    ...(IS_WEB_RUNTIME ? { pickWebStoragePath: requestWebStoragePath } : {}),
  });

  const {
    cfg,
    primary,
    destinations,
    watchedItems,
    loading,
    loadError,
    busy,
    setBusy,
    setCfg,
    reload,
    persist,
  } = useMinimalConfig({ onEvent: emitEvent });
  const {
    chooseDestination,
    addDestination,
    removeDestination,
    addFolder,
    changePath,
    removePath,
    updateKeep,
  } = useMinimalActions({
    cfg,
    primaryId: primary?.id ?? null,
    persist,
    pickers,
    onEvent: emitEvent,
  });
  const { liveSafeMode, setLiveSafeMode, destinationWarning, replicationWarning, safetyWarning } =
    useMinimalStatus({ onEvent: emitEvent });
  const { runningBusy, setRunning: applyRunningState } = useMinimalRunning({
    cfg,
    setCfg,
    onEvent: emitEvent,
    onLiveSafeModeConfirmed: setLiveSafeMode,
  });
  const { destinationHealthWarning, watchedHealthWarning } = useMinimalLiveHealth({
    cfg,
    onEvent: emitEvent,
    suspend: pickerBusyScope !== null,
  });

  const requestStorageMove = useCallback(
    async (destinationId: string) => {
      const destination = destinations.find((candidate) => candidate.id === destinationId);
      if (!destination) {
        emitEvent('That storage location no longer exists.', 'error');
        return;
      }
      const picked = await pickers.pickDestinationPath();
      if (!picked || picked === destination.path) return;
      if (
        destinations.some(
          (candidate) => candidate.id !== destinationId && candidate.path === picked,
        )
      ) {
        emitEvent('That storage location is already configured.', 'info');
        return;
      }
      const index = destinations.findIndex((candidate) => candidate.id === destinationId);
      setPendingStorageMove({
        destinationId,
        label: index === 0 ? 'Main storage' : 'Secondary backup location',
        oldPath: destination.path,
        newPath: picked,
      });
    },
    [destinations, emitEvent, pickers],
  );

  const confirmStorageMove = useCallback(async () => {
    if (!pendingStorageMove) return;
    try {
      setStorageMoveBusy(true);
      setBusy(true);
      const result = await relocateDestination(
        pendingStorageMove.destinationId,
        pendingStorageMove.newPath,
      );
      setCfg(result.config);
      setPendingStorageMove(null);
      if (result.warning) emitEvent(result.warning, 'info');
      emitEvent(`${pendingStorageMove.label} moved safely to ${pendingStorageMove.newPath}.`, 'ok');
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      emitEvent(
        `Storage move failed. The old backup location was kept unchanged: ${reason}`,
        'error',
      );
    } finally {
      setStorageMoveBusy(false);
      setBusy(false);
    }
  }, [emitEvent, pendingStorageMove, setBusy, setCfg]);

  const folderItems = useMemo(() => {
    const unique = new Map<
      string,
      {
        path: string;
        kind: 'File' | 'Directory';
        destination_id: string;
        max_backups_per_file: number | null;
      }
    >();
    for (const watched of watchedItems) {
      const kind = watched.kind ?? 'Directory';
      const key = `${kind}:${watched.path}`;
      if (unique.has(key)) continue;
      unique.set(key, {
        path: watched.path,
        kind,
        destination_id: watched.destination_id ?? primary?.id ?? 'default',
        max_backups_per_file: watched.max_backups_per_file ?? null,
      });
    }
    return [...unique.values()];
  }, [primary?.id, watchedItems]);
  useEffect(() => {
    if (!cfg) return;
    const fingerprint = hardeningFingerprint(cfg);
    const key = `${fingerprint}#${hardeningNonce}`;
    if (lastHardeningKeyRef.current === key) return;
    lastHardeningKeyRef.current = key;

    const runId = hardeningRunIdRef.current + 1;
    hardeningRunIdRef.current = runId;
    setHardeningBusy(true);
    setHardeningIssue(null);

    void (async () => {
      try {
        // Full startup check: include snapshot capability probing so failures surface immediately.
        const report = await hardeningCheck({ check_snapshots: true });
        if (hardeningRunIdRef.current !== runId) return;
        setHardeningOk(report.ok);
        if (report.ok) {
          writeHardeningDone(fingerprint);
          setHardeningIssue(null);
          return;
        }

        const message = `Safety checks failed: ${report.message}`;
        setHardeningIssue(message);
        emitEvent(message, 'error');
        if (!cfg.safe_mode) {
          await applyRunningState(false);
        }
      } catch (error) {
        if (hardeningRunIdRef.current !== runId) return;
        const reason = error instanceof Error ? error.message : String(error);
        const message = `Safety checks failed: ${reason}`;
        setHardeningOk(false);
        setHardeningIssue(message);
        emitEvent(message, 'error');
        if (!cfg.safe_mode) {
          await applyRunningState(false);
        }
      } finally {
        if (hardeningRunIdRef.current === runId) {
          setHardeningBusy(false);
        }
      }
    })();
  }, [applyRunningState, cfg, emitEvent, hardeningNonce]);

  /** Keep precondition gating in one place while preserving optimistic UI behavior. */
  const setRunning = useCallback(
    async (running: boolean) => {
      if (!cfg) return;
      if (running && hardeningOk === false) {
        if (hardeningBusy) {
          emitEvent('Safety checks are running in the background. Try again in a moment.', 'info');
          return;
        }
        setHardeningOk(null);
        setHardeningIssue(null);
        setHardeningNonce((prev) => prev + 1);
        emitEvent(
          hardeningIssue ?? 'Safety checks must pass before enabling Running. Re-checking now.',
          hardeningIssue ? 'error' : 'info',
        );
        return;
      }
      if (running && hardeningOk === null && hardeningBusy) {
        emitEvent('Safety checks are running in the background. Try again in a moment.', 'info');
        return;
      }
      if (running) {
        await runWithSaveScope('header', async () => {
          await applyRunningState(running);
        });
        return;
      }
      pendingSaveScopeRef.current = 'none';
      await applyRunningState(running);
    },
    [
      applyRunningState,
      cfg,
      emitEvent,
      hardeningBusy,
      hardeningIssue,
      hardeningOk,
      runWithSaveScope,
    ],
  );

  if (loading) {
    return (
      <div className="app" aria-busy="true">
        <div className="hero">
          <h1>Backup Sync</h1>
          <p>Preparing your backup workspace…</p>
        </div>
        <StateBlock
          tone="loading"
          title="Loading configuration"
          message="Reading folders, destinations, and runtime status."
        />
      </div>
    );
  }

  if (loadError || !cfg || !primary) {
    return (
      <div className="app">
        <div className="hero">
          <h1>Backup Sync</h1>
          <p>Your backup workspace could not be prepared.</p>
        </div>
        <StateBlock
          tone="error"
          title="Could not load configuration"
          message={loadError ?? 'The configuration was unavailable.'}
          action={
            <Button type="button" tone="secondary" size="sm" onClick={reload}>
              Retry
            </Button>
          }
        />
      </div>
    );
  }

  return (
    <div
      className={`app ${savePulseActive ? `pulse-${savePulseScope}` : ''} ${pausePulseActive ? 'is-paused' : ''}`}
      aria-busy={busy || runningBusy}
    >
      <MinimalHeader
        liveSafeMode={liveSafeMode}
        busy={busy}
        runningBusy={runningBusy}
        onRunningChange={(running) => {
          void setRunning(running);
        }}
      />

      {inline && (
        <InlineAlert kind={inline.kind} className="feedback-banner">
          <span>{inline.msg}</span>
          <Button
            type="button"
            tone="secondary"
            size="sm"
            className="feedback-dismiss"
            onClick={clearInline}
          >
            Dismiss
          </Button>
        </InlineAlert>
      )}

      {safetyWarning &&
        (dismissedSafetyWarningTs === null || safetyWarning.ts > dismissedSafetyWarningTs) && (
          <InlineAlert kind="warn" className="feedback-banner">
            <span>{safetyWarning.message}</span>
            <div className="feedback-actions">
              <Button
                type="button"
                size="sm"
                className="feedback-dismiss"
                loading={safetyRemoveBusy}
                loadingLabel="Removing…"
                onClick={() => {
                  const watchedPath = safetyWarning.watched_path;
                  if (!watchedPath) {
                    emitEvent('Safety warning missing path context.', 'error');
                    return;
                  }
                  void runWithSaveScope('folders', async () => {
                    try {
                      setSafetyRemoveBusy(true);
                      await removeKeptExtraVersion(watchedPath);
                      setDismissedSafetyWarningTs(safetyWarning.ts);
                      emitEvent('Saved.', 'ok');
                    } catch (error) {
                      const reason = error instanceof Error ? error.message : String(error);
                      emitEvent(`Could not remove extra version: ${reason}`, 'error');
                    } finally {
                      setSafetyRemoveBusy(false);
                    }
                  });
                }}
              >
                Remove extra version
              </Button>
              <Button
                type="button"
                tone="secondary"
                size="sm"
                className="feedback-dismiss"
                onClick={() => setDismissedSafetyWarningTs(safetyWarning.ts)}
              >
                Dismiss
              </Button>
            </div>
          </InlineAlert>
        )}

      <div className="grid minimal-grid">
        <FoldersCard
          busy={busy || pickerBusyScope === 'source'}
          items={folderItems}
          defaultKeep={cfg.max_backups_per_file}
          intervalSeconds={cfg.interval_seconds}
          watchedWarning={watchedHealthWarning}
          onAddFolder={() =>
            runWithSaveScope('none', async () => {
              await addFolder();
            })
          }
          onChangePath={(path, kind) =>
            runWithSaveScope('none', async () => {
              await changePath(path, kind);
            })
          }
          onRemovePath={(path, kind, sourceDestinationId) =>
            runWithSaveScope('none', async () => {
              await removePath(path, kind, sourceDestinationId);
            })
          }
          onUpdateKeep={(path, kind, sourceDestinationId, keep) =>
            runWithSaveScope('none', async () => {
              await updateKeep(path, kind, sourceDestinationId, keep);
            })
          }
        />

        <DestinationCard
          destinations={destinations}
          busy={busy || pickerBusyScope === 'destination'}
          destinationWarning={destinationWarning ?? destinationHealthWarning}
          replicationWarning={replicationWarning}
          onChoose={() =>
            runWithSaveScope('none', async () => {
              await chooseDestination();
            })
          }
          onChangeDestination={(destinationId) =>
            runWithSaveScope('none', async () => {
              await requestStorageMove(destinationId);
            })
          }
          onAddDestination={() =>
            runWithSaveScope('none', async () => {
              await addDestination();
            })
          }
          onRemoveDestination={(destinationId) =>
            runWithSaveScope('none', async () => {
              await removeDestination(destinationId);
            })
          }
        />

        <RestoreCard
          busy={busy}
          disabled={folderItems.length === 0}
          onOpenRestore={() => setRestoreOpen(true)}
        />
      </div>

      <RestoreModal
        open={restoreOpen}
        onClose={() => setRestoreOpen(false)}
        onEvent={(msg, kind) => {
          if (kind === 'ok' && /\brestore(d|)\b/i.test(msg)) {
            pendingSaveScopeRef.current = 'restore';
          }
          emitEvent(msg, kind);
        }}
      />
      <StorageLocationPickerModal
        open={storagePickerOpen}
        usedPaths={destinations.map((destination) => destination.path)}
        onCancel={() => finishWebStoragePicker(null)}
        onChoose={(path) => finishWebStoragePicker(path)}
      />
      <StorageMoveModal
        move={pendingStorageMove}
        busy={storageMoveBusy}
        onCancel={() => setPendingStorageMove(null)}
        onConfirm={confirmStorageMove}
      />
      <ToastMessage toast={toast} onDismiss={clearToast} />
      {showSavedBadge && (
        <div key={savedBadgeNonce} className="save-badge">
          Saved
        </div>
      )}
    </div>
  );
}
