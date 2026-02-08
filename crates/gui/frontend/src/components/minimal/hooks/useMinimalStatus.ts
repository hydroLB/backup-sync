import { useEffect, useMemo, useRef, useState } from 'react';
import { getStatus } from '../../../services/status';
import { tauriAvailable } from '../../../services/ipc';
import { UI_TUNING } from '../../../config/uiTuning';
import { StatusDto } from '../../../services/types';

type EventKind = 'ok' | 'error' | 'info';

type Params = {
  onEvent: (msg: string, kind?: EventKind) => void;
};

type MinimalStatusState = {
  status: StatusDto | null;
  destinationWarning: string | null;
  replicationWarning: string | null;
  safetyWarning: StatusDto['last_safety_warning'] | null;
};

/**
 * Summary: Poll daemon status and emit transition feedback for minimal mode.
 *
 * Inputs: Event callback for transition notifications.
 * Outputs: Latest status plus derived destination and replication warning strings.
 * Side effects: Starts/stops polling interval timers and reads daemon status via IPC.
 * Error handling: Ignores polling failures so minimal mode remains usable offline.
 * Ties to other methods: Used by `MinimalMain` to render warning banners and toasts.
 * Why this exists: Keep status polling and transition-notification logic out of screen rendering code.
 */
export function useMinimalStatus({ onEvent }: Params): MinimalStatusState {
  const [status, setStatus] = useState<StatusDto | null>(null);
  const prevRef = useRef<{ destinationPaused: boolean; replicationFailed: boolean } | null>(null);

  useEffect(() => {
    if (!tauriAvailable()) return;
    let cancelled = false;

    const refresh = () => {
      getStatus()
        .then((nextStatus) => {
          if (cancelled) return;
          setStatus(nextStatus);

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
        .catch(() => {
          // Minimal mode stays usable even when daemon status is unavailable.
        });
    };

    refresh();
    const id = setInterval(refresh, UI_TUNING.statusRefreshMs);
    return () => {
      cancelled = true;
      clearInterval(id);
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

  return { status, destinationWarning, replicationWarning, safetyWarning };
}
