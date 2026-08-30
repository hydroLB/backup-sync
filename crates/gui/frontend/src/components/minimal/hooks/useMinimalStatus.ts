import { useEffect, useMemo, useRef, useState } from 'react';
import { getStatus } from '../../../services/status';
import { UI_TUNING } from '../../../config/uiTuning';
import { StatusDto } from '../../../services/types';

type EventKind = 'ok' | 'error' | 'info';

type Params = {
  onEvent: (msg: string, kind?: EventKind) => void;
};

type MinimalStatusState = {
  status: StatusDto | null;
  liveSafeMode: boolean | null;
  setLiveSafeMode: (safeMode: boolean | null) => void;
  destinationWarning: string | null;
  replicationWarning: string | null;
  safetyWarning: StatusDto['last_safety_warning'] | null;
};

/** Keep status polling and transition-notification logic out of screen rendering code. */
export function useMinimalStatus({ onEvent }: Params): MinimalStatusState {
  const [status, setStatus] = useState<StatusDto | null>(null);
  const [liveSafeMode, setLiveSafeMode] = useState<boolean | null>(null);
  const prevRef = useRef<{ destinationPaused: boolean; replicationFailed: boolean } | null>(null);
  const renderKeyRef = useRef<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    const refresh = () => {
      if (typeof document !== 'undefined' && document.hidden) return;
      getStatus()
        .then((nextStatus) => {
          if (cancelled) return;
          const renderKey = JSON.stringify({
            lastRunTs: nextStatus.last_run_ts,
            lastError: nextStatus.last_error,
            safeMode: nextStatus.safe_mode ?? null,
            destinationPaused: nextStatus.destination_paused ?? false,
            destinationReason: nextStatus.destination_pause_reason ?? null,
            replicationFailed: nextStatus.replication_last_pairs_failed ?? 0,
            replicationError: nextStatus.replication_last_error ?? null,
            safetyWarning: nextStatus.last_safety_warning ?? null,
          });
          if (renderKeyRef.current !== renderKey) {
            renderKeyRef.current = renderKey;
            setStatus(nextStatus);
          }
          setLiveSafeMode(nextStatus.safe_mode ?? null);

          const previous = prevRef.current;
          const nextDestinationPaused = !!nextStatus.destination_paused;
          const nextReplicationFailed = (nextStatus.replication_last_pairs_failed ?? 0) > 0;
          if (previous) {
            if (!previous.destinationPaused && nextDestinationPaused) {
              onEvent(
                nextStatus.destination_pause_reason ?? 'Destination unavailable. Writes paused.',
                'error',
              );
            }
            if (previous.destinationPaused && !nextDestinationPaused) {
              onEvent('Destination reconnected. Resuming backups.', 'ok');
            }
            if (!previous.replicationFailed && nextReplicationFailed) {
              onEvent(
                nextStatus.replication_last_error ??
                  'Replication degraded. Mirror destination may be offline; backups continue to primary.',
                'error',
              );
            }
            if (previous.replicationFailed && !nextReplicationFailed) {
              onEvent('Replication healthy again.', 'ok');
            }
          }
          prevRef.current = {
            destinationPaused: nextDestinationPaused,
            replicationFailed: nextReplicationFailed,
          };
        })
        .catch((error) => {
          if (cancelled) return;
          if (renderKeyRef.current !== null) {
            renderKeyRef.current = null;
            setStatus(null);
          }
          setLiveSafeMode(null);
          prevRef.current = null;
          const reason = error instanceof Error ? error.message : String(error);
          console.warn(`[useMinimalStatus::refresh] Status polling failed: ${reason}`);
        });
    };

    refresh();
    const id = setInterval(refresh, UI_TUNING.statusRefreshMs);
    const onVisibilityChange = () => {
      if (!document.hidden) refresh();
    };
    document.addEventListener('visibilitychange', onVisibilityChange);
    return () => {
      cancelled = true;
      clearInterval(id);
      document.removeEventListener('visibilitychange', onVisibilityChange);
    };
  }, [onEvent]);

  const destinationWarning = useMemo(() => {
    if (!status?.destination_paused) return null;
    return status.destination_pause_reason ?? 'Destination unavailable. Writes paused.';
  }, [status]);

  const replicationWarning = useMemo(() => {
    if ((status?.replication_last_pairs_failed ?? 0) <= 0) return null;
    return `Replication degraded: ${status?.replication_last_error ?? 'One or more mirror targets failed.'}`;
  }, [status]);

  const safetyWarning = useMemo(() => status?.last_safety_warning ?? null, [status]);

  return {
    status,
    liveSafeMode,
    setLiveSafeMode,
    destinationWarning,
    replicationWarning,
    safetyWarning,
  };
}
