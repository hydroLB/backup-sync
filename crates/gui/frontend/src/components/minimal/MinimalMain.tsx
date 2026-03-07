import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { hardeningFingerprint, writeHardeningDone } from '../../utils/hardening';
import { hardeningCheck } from '../../services/system';
import { RestoreModal } from './RestoreModal';
import { useMinimalActions } from './hooks/useMinimalActions';
import { useMinimalConfig } from './hooks/useMinimalConfig';
import { useMinimalRunning } from './hooks/useMinimalRunning';
import { useMinimalLog } from './hooks/useMinimalLog';
import { SavePulseScope, useMinimalFeedback } from './hooks/useMinimalFeedback';
import { useMinimalStatus } from './hooks/useMinimalStatus';
import { useMinimalLiveHealth } from './hooks/useMinimalLiveHealth';
import { useMinimalPickers } from './hooks/useMinimalPickers';
import { removeKeptExtraVersion } from '../../services/safety';
import { MinimalHeader } from './sections/MinimalHeader';
import { DestinationCard } from './sections/DestinationCard';
import { FoldersCard } from './sections/FoldersCard';
import { LogCard } from './sections/LogCard';
import { RestoreCard } from './sections/RestoreCard';
import { SetupNotice } from './sections/SetupNotice';
import { ToastMessage } from '../ui/ToastMessage';
import { InlineAlert } from '../ui/InlineAlert';
import { Button } from '../ui/Button';
import { StateBlock } from '../ui/StateBlock';

type EventKind = 'ok' | 'error' | 'info';

/**
 * Summary: Render the spec-minimal main screen for the backup app.
 *
 * Inputs: `onEvent` callback for user-visible event messages.
 *
 * Outputs: React element tree for the minimal main screen.
 *
 * Side effects: Invokes IPC-backed config, status, and log operations through focused hooks.
 *
 * Error handling: Emits actionable messages via toast and event callback.
 *
 * Ties to other methods: Composes minimal hooks, section components, and modal flows.
 *
 * Why this exists: Keep the primary UX compact while preserving operational controls.
 */
export function MinimalMain({ onEvent }: { onEvent: (msg: string, kind?: EventKind) => void }) {
  const [restoreOpen, setRestoreOpen] = useState(false);
  const [pickerBusy, setPickerBusy] = useState(false);
  const [hardeningBusy, setHardeningBusy] = useState(false);
  const [hardeningOk, setHardeningOk] = useState<boolean | null>(null);
  const [hardeningIssue, setHardeningIssue] = useState<string | null>(null);
  const [hardeningNonce, setHardeningNonce] = useState(0);
  const [dismissedSafetyWarningTs, setDismissedSafetyWarningTs] = useState<number | null>(null);
  const [safetyRemoveBusy, setSafetyRemoveBusy] = useState(false);
  const lastHardeningKeyRef = useRef<string | null>(null);
  const hardeningRunIdRef = useRef(0);

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
    onPickerBusyChange: (next) => setPickerBusy(next),
  });

  const {
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
  } = useMinimalConfig({ onEvent: emitEvent });
  const { runningBusy, setRunning: applyRunningState } = useMinimalRunning({
    cfg,
    setCfg,
    onEvent: emitEvent,
  });
  const {
    chooseDestination,
    addDestination,
    removeDestination,
    addFolder,
    removePath,
    updateKeep,
    updateDestination,
  } = useMinimalActions({
    cfg,
    primaryId: primary?.id ?? null,
    persist,
    pickers,
    onEvent: emitEvent,
  });
  const { showLog, logTail, logLoading, setShowLog, refreshLog } = useMinimalLog({
    onEvent: emitEvent,
    setBusy,
  });
  const { destinationWarning, replicationWarning, safetyWarning } = useMinimalStatus({
    onEvent: emitEvent,
  });
  const { destinationHealthWarning, watchedHealthWarning } = useMinimalLiveHealth({
    cfg,
    onEvent: emitEvent,
    suspend: pickerBusy,
  });

  const folderItems = useMemo(
    () =>
      watchedItems.map((watched) => ({
        path: watched.path,
        kind: watched.kind ?? 'Directory',
        destination_id: watched.destination_id ?? primary?.id ?? 'default',
        max_backups_per_file: watched.max_backups_per_file ?? null,
      })),
    [primary?.id, watchedItems],
  );
  const destinationReady = (primary?.path ?? '').trim().length > 0;
  const configuredDestinationCount = useMemo(
    () => destinations.filter((destination) => destination.path.trim().length > 0).length,
    [destinations],
  );
  const setupStep = useMemo(() => {
    if (!destinationReady) return 'destination' as const;
    if (folderItems.length === 0) return 'folders' as const;
    return 'ready' as const;
  }, [destinationReady, folderItems.length]);

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

  /**
   * Summary: Change running state while enforcing hardening preconditions.
   *
   * Inputs: Desired running state.
   *
   * Outputs: None.
   *
   * Side effects: May block enablement while background hardening runs and toggles backend safe mode.
   *
   * Error handling: Delegated to running hook and event callback.
   *
   * Ties to other methods: Used by `MinimalHeader` running toggle and background hardening checks.
   *
   * Why this exists: Keep precondition gating in one place while preserving optimistic UI behavior.
   */
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
        await runWithSaveScope('global', async () => {
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

  if (loading || !cfg || !primary) {
    return (
      <div className="app" aria-busy="true">
        <div className="hero">
          <h1>Local Backup Manager</h1>
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

  return (
    <div
      className={`app ${savePulseActive ? `pulse-${savePulseScope}` : ''} ${pausePulseActive ? 'is-paused' : ''}`}
      aria-busy={busy || runningBusy}
    >
      <MinimalHeader
        cfg={cfg}
        busy={busy}
        runningBusy={runningBusy}
        showLog={showLog}
        onRunningChange={(running) => {
          void setRunning(running);
        }}
        onToggleLog={() => setShowLog((prev) => !prev)}
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

      <SetupNotice
        step={setupStep}
        destinationCount={configuredDestinationCount}
        watchedCount={folderItems.length}
        busy={busy || pickerBusy}
        onChooseDestination={() => {
          void runWithSaveScope('destination', async () => {
            await chooseDestination();
          });
        }}
        onAddPath={() => {
          void runWithSaveScope('folders', async () => {
            await addFolder();
          });
        }}
      />

      <div className="grid minimal-grid">
        <DestinationCard
          destinations={destinations}
          busy={busy || pickerBusy}
          destinationWarning={destinationWarning ?? destinationHealthWarning}
          replicationWarning={replicationWarning}
          onChoose={() => {
            void runWithSaveScope('destination', async () => {
              await chooseDestination();
            });
          }}
          onAddDestination={() => {
            void runWithSaveScope('destination', async () => {
              await addDestination();
            });
          }}
          onRemoveDestination={(destinationId) => {
            void runWithSaveScope('destination', async () => {
              await removeDestination(destinationId);
            });
          }}
        />

        <FoldersCard
          busy={busy}
          items={folderItems}
          destinations={destinations}
          destinationReady={destinationReady}
          defaultKeep={cfg.max_backups_per_file}
          watchedWarning={watchedHealthWarning}
          onAddFolder={() => {
            void runWithSaveScope('folders', async () => {
              await addFolder();
            });
          }}
          onRemovePath={(path, kind, sourceDestinationId) => {
            void runWithSaveScope('folders', async () => {
              await removePath(path, kind, sourceDestinationId);
            });
          }}
          onUpdateKeep={(path, kind, sourceDestinationId, keep) => {
            void runWithSaveScope('folders', async () => {
              await updateKeep(path, kind, sourceDestinationId, keep);
            });
          }}
          onUpdateDestination={(path, kind, sourceDestinationId, destinationId) => {
            void runWithSaveScope('folders', async () => {
              await updateDestination(path, kind, sourceDestinationId, destinationId);
            });
          }}
        />

        <RestoreCard
          busy={busy}
          disabled={watchedDirs.length === 0}
          watchedCount={watchedDirs.length}
          onOpenRestore={() => setRestoreOpen(true)}
        />
      </div>

      <LogCard
        open={showLog}
        busy={busy}
        loading={logLoading}
        tail={logTail}
        onRefresh={() => {
          void refreshLog();
        }}
        onClose={() => setShowLog(false)}
      />
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
      <ToastMessage toast={toast} onDismiss={clearToast} />
      {showSavedBadge && (
        <div key={savedBadgeNonce} className="save-badge">
          Saved
        </div>
      )}
    </div>
  );
}
