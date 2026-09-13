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
import { openDestinationFolder, relocateDestination } from '../../services/storage';
import { IS_WEB_RUNTIME } from '../../runtime/mode';
import { StorageLocationPickerModal } from './StorageLocationPickerModal';
import { listVersionFiles, listVersions } from '../../services/restore';

type EventKind = 'ok' | 'error' | 'info';

const DESKTOP_PROJECT_URL =
  import.meta.env.VITE_DESKTOP_PROJECT_URL?.trim() ||
  'https://github.com/hydroLB/backup-sync#quick-start';

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
  const [backupInventory, setBackupInventory] = useState<
    Record<string, { fileCount: number; versionCount: number }>
  >({});
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
    updateIntervalMinutes,
  } = useMinimalActions({
    cfg,
    primaryId: primary?.id ?? null,
    persist,
    pickers,
    onEvent: emitEvent,
  });
  const {
    status,
    liveSafeMode,
    setLiveSafeMode,
    destinationWarning,
    replicationWarning,
    safetyWarning,
  } = useMinimalStatus({ onEvent: emitEvent });
  const { runningBusy, setRunning: applyRunningState } = useMinimalRunning({
    cfg,
    setCfg,
    onEvent: emitEvent,
    onLiveSafeModeConfirmed: setLiveSafeMode,
  });
  const { destinationHealthWarning, watchedHealthWarning } = useMinimalLiveHealth({
    cfg,
    onEvent: emitEvent,
    suspend: pickerBusyScope !== null || hardeningBusy,
  });

  const requestStorageMove = useCallback(
    async (destinationId: string) => {
      const destination = destinations.find((candidate) => candidate.id === destinationId);
      if (!destination) {
        emitEvent('That storage location no longer exists.', 'error');
        return;
      }
      const picked = await pickers.pickDestinationPath(destination.path);
      if (!picked || picked === destination.path) return;
      const index = destinations.findIndex((candidate) => candidate.id === destinationId);
      const existingIndex = destinations.findIndex(
        (candidate) => candidate.id !== destinationId && candidate.path === picked,
      );
      if (existingIndex >= 0 && index !== 0) {
        emitEvent('Only Main storage can switch with an existing secondary backup.', 'info');
        return;
      }
      setPendingStorageMove({
        destinationId,
        label: index === 0 ? 'Main storage' : 'Secondary backup location',
        oldPath: destination.path,
        newPath: picked,
        mode: existingIndex >= 0 ? 'promote' : 'move',
        ...(existingIndex >= 0
          ? { existingLabel: `Secondary backup location ${existingIndex}` }
          : {}),
      });
    },
    [destinations, emitEvent, pickers],
  );

  const openStorageFolder = useCallback(
    async (destinationId: string) => {
      try {
        const opened = await openDestinationFolder(destinationId);
        if (!opened) {
          emitEvent('The downloadable app opens this storage folder in Finder.', 'info');
        }
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        emitEvent(`Could not open this storage folder: ${reason}`, 'error');
      }
    },
    [emitEvent],
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
      emitEvent(
        pendingStorageMove.mode === 'promote'
          ? `Main storage switched safely to ${pendingStorageMove.newPath}. The previous Main storage is still protected as a secondary backup.`
          : `${pendingStorageMove.label} moved safely to ${pendingStorageMove.newPath}.`,
        'ok',
      );
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      emitEvent(
        `Storage change failed. The previous storage configuration was kept unchanged: ${reason}`,
        'error',
      );
    } finally {
      setStorageMoveBusy(false);
      setBusy(false);
    }
  }, [emitEvent, pendingStorageMove, setBusy, setCfg]);

  const backupInventoryConfigKey =
    cfg === null
      ? null
      : JSON.stringify(
          cfg.watched
            .map((watched) => [watched.path, watched.kind ?? 'Directory'])
            .sort(([left], [right]) => left!.localeCompare(right!)),
        );

  useEffect(() => {
    if (backupInventoryConfigKey === null) {
      setBackupInventory({});
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const catalog = await listVersions();
        const uniqueFolders = new Map<string, (typeof catalog)[number]>();
        for (const folder of catalog) {
          const current = uniqueFolders.get(folder.source_path);
          if (!current || folder.versions.length > current.versions.length) {
            uniqueFolders.set(folder.source_path, folder);
          }
        }
        const entries = await Promise.all(
          [...uniqueFolders.values()].map(async (folder) => {
            const latest = [...folder.versions].sort(
              (left, right) => right.created_at_unix - left.created_at_unix,
            )[0];
            if (!latest) {
              return [folder.source_path, { fileCount: 0, versionCount: 0 }] as const;
            }
            const files = await listVersionFiles({
              source_path: folder.source_path,
              version_id: latest.id,
              limit: 1,
            });
            return [
              folder.source_path,
              { fileCount: files.total_files, versionCount: folder.versions.length },
            ] as const;
          }),
        );
        if (!cancelled) setBackupInventory(Object.fromEntries(entries));
      } catch (error) {
        if (cancelled) return;
        const reason = error instanceof Error ? error.message : String(error);
        console.warn(`[MinimalMain::backupInventory] Could not read backup inventory: ${reason}`);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [backupInventoryConfigKey, status?.last_run_ts]);

  const folderItems = useMemo(() => {
    const unique = new Map<
      string,
      {
        path: string;
        kind: 'File' | 'Directory';
        destination_id: string;
        max_backups_per_file: number | null;
        saved_file_count?: number;
        saved_version_count?: number;
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
        ...(backupInventory[watched.path]
          ? {
              saved_file_count: backupInventory[watched.path]!.fileCount,
              saved_version_count: backupInventory[watched.path]!.versionCount,
            }
          : {}),
      });
    }
    return [...unique.values()];
  }, [backupInventory, primary?.id, watchedItems]);
  const sectionBalance = useMemo(() => {
    const configuredDestinationCount = destinations.filter(
      (destination) => destination.path.trim().length > 0,
    ).length;
    const foldersNeedScroll = folderItems.length > 2;
    const destinationsNeedScroll = Math.ceil(configuredDestinationCount / 2) > 2;

    if (foldersNeedScroll === destinationsNeedScroll) return 'balanced';
    return foldersNeedScroll ? 'folders' : 'destinations';
  }, [destinations, folderItems.length]);
  const operationalWarnings = useMemo(
    () =>
      Array.from(
        new Set(
          [
            hardeningIssue,
            watchedHealthWarning,
            destinationWarning ?? destinationHealthWarning,
            replicationWarning,
          ].filter((message): message is string => !!message),
        ),
      ),
    [
      destinationHealthWarning,
      destinationWarning,
      hardeningIssue,
      replicationWarning,
      watchedHealthWarning,
    ],
  );
  const warningStillApplies =
    !safetyWarning?.watched_path ||
    cfg?.watched.some((watched) => watched.path === safetyWarning.watched_path);
  const visibleSafetyWarning =
    safetyWarning &&
    warningStillApplies &&
    (dismissedSafetyWarningTs === null || safetyWarning.ts > dismissedSafetyWarningTs)
      ? safetyWarning
      : null;
  const visibleInline = inline && !operationalWarnings.includes(inline.msg) ? inline : null;
  const showFeedbackOverlay =
    visibleInline !== null || operationalWarnings.length > 0 || visibleSafetyWarning !== null;
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
        // Snapshot probing can create and mount a real filesystem snapshot. Only pay that cost
        // when snapshot-backed scans are enabled; ordinary startup stays lightweight.
        const snapshotsEnabled = cfg.runtime.source_snapshots_enabled === true;
        const report = await hardeningCheck({
          check_snapshots: snapshotsEnabled,
          require_snapshots: snapshotsEnabled,
        });
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
        {...(IS_WEB_RUNTIME ? { desktopUrl: DESKTOP_PROJECT_URL } : {})}
      />

      {showFeedbackOverlay && (
        <div
          className="feedback-overlay"
          aria-label="Backup Sync notifications"
          data-tauri-drag-region
        >
          {visibleInline && (
            <InlineAlert kind={visibleInline.kind} className="feedback-banner" dragRegion>
              <span data-tauri-drag-region>{visibleInline.msg}</span>
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

          {operationalWarnings.map((message) => (
            <InlineAlert
              key={message}
              kind="error"
              className="feedback-banner feedback-banner--persistent"
              dragRegion
            >
              <span data-tauri-drag-region>{message}</span>
            </InlineAlert>
          ))}

          {visibleSafetyWarning && (
            <InlineAlert kind="warn" className="feedback-banner" dragRegion>
              <span data-tauri-drag-region>{visibleSafetyWarning.message}</span>
              <div className="feedback-actions" data-tauri-drag-region>
                <Button
                  type="button"
                  size="sm"
                  className="feedback-dismiss"
                  loading={safetyRemoveBusy}
                  loadingLabel="Removing…"
                  onClick={() => {
                    const watchedPath = visibleSafetyWarning.watched_path;
                    if (!watchedPath) {
                      emitEvent('Safety warning missing path context.', 'error');
                      return;
                    }
                    void runWithSaveScope('folders', async () => {
                      try {
                        setSafetyRemoveBusy(true);
                        await removeKeptExtraVersion(watchedPath);
                        setDismissedSafetyWarningTs(visibleSafetyWarning.ts);
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
                  onClick={() => setDismissedSafetyWarningTs(visibleSafetyWarning.ts)}
                >
                  Dismiss
                </Button>
              </div>
            </InlineAlert>
          )}
        </div>
      )}

      <div className="grid minimal-grid" data-section-balance={sectionBalance}>
        <FoldersCard
          busy={busy || pickerBusyScope === 'source'}
          items={folderItems}
          defaultKeep={cfg.max_backups_per_file}
          intervalSeconds={cfg.interval_seconds}
          updated={savePulseActive && savePulseScope === 'folders'}
          onAddFolder={() =>
            runWithSaveScope('folders', async () => {
              await addFolder();
            })
          }
          onChangePath={(path, kind) =>
            runWithSaveScope('folders', async () => {
              await changePath(path, kind);
            })
          }
          onRemovePath={(path, kind, sourceDestinationId) =>
            runWithSaveScope('folders', async () => {
              await removePath(path, kind, sourceDestinationId);
            })
          }
          onUpdateKeep={(path, kind, sourceDestinationId, keep) =>
            runWithSaveScope('folders', async () => {
              await updateKeep(path, kind, sourceDestinationId, keep);
            })
          }
          onUpdateInterval={(minutes) =>
            runWithSaveScope('folders', async () => {
              await updateIntervalMinutes(minutes);
            })
          }
        />

        <DestinationCard
          destinations={destinations}
          busy={busy || pickerBusyScope === 'destination'}
          updated={savePulseActive && savePulseScope === 'destination'}
          onChoose={() =>
            runWithSaveScope('destination', async () => {
              await chooseDestination();
            })
          }
          onOpenDestination={openStorageFolder}
          onChangeDestination={(destinationId) =>
            runWithSaveScope('none', async () => {
              await requestStorageMove(destinationId);
            })
          }
          onAddDestination={() =>
            runWithSaveScope('destination', async () => {
              await addDestination();
            })
          }
          onRemoveDestination={(destinationId) =>
            runWithSaveScope('destination', async () => {
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
        onConfirm={() => runWithSaveScope('destination', confirmStorageMove)}
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
