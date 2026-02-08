import { useCallback, useEffect, useRef, useState } from 'react';
import { getStatus } from '../../../../services';
import { checkService } from '../../../../services/system';
import { getLogTail } from '../../../../services/logs';
import { tauriAvailable } from '../../../../services/ipc';
import { UI_TUNING } from '../../../../config/uiTuning';
import { ServiceStatusDto, StatusDto } from '../../../../services/types';
import { displayError, errorMessage } from '../utils/errors';

type Params = {
  onSafeMode?: (v: boolean) => void;
};

type State = {
  status: StatusDto | null;
  error: string | null;
  logTail: string;
  serviceStatus: ServiceStatusDto | null;
  setServiceStatus: (s: ServiceStatusDto | null) => void;
  refresh: () => void;
};

/**
 * Summary: Poll daemon status, service state, and recent log tail for the status card.
 *
 * Inputs: Optional `onSafeMode` callback to mirror daemon safe-mode status upward.
 * Outputs: Status, error string, log tail, service status, and a refresh function.
 * Side effects: Schedules polling timers and performs IPC-backed service calls.
 * Error handling: Converts IPC errors into stable user-facing messages.
 * Ties to other methods: Used by `StatusCard` to render online/offline states and actions.
 * Why this exists: Centralize status polling so UI code stays focused on rendering.
 */
export function useStatusPolling({ onSafeMode }: Params): State {
  const [status, setStatus] = useState<StatusDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [logTail, setLogTail] = useState<string>('');
  const [serviceStatus, setServiceStatus] = useState<ServiceStatusDto | null>(null);
  const refreshInFlight = useRef(false);

  /**
   * Summary: Refresh status from the daemon with in-flight suppression.
   *
   * Inputs: None.
   * Outputs: Updates status and error state.
   * Side effects: Invokes IPC via `getStatus` and emits safe-mode callbacks.
   * Error handling: Populates `error` and clears status when IPC fails.
   * Ties to other methods: Used by polling effects and action handlers after mutations.
   * Why this exists: Prevent redundant polling and keep the UI responsive.
   */
  const refresh = useCallback(() => {
    if (refreshInFlight.current) return;
    refreshInFlight.current = true;
    if (!tauriAvailable()) {
      setError(
        '[useStatusPolling::refresh] Tauri IPC unavailable. Please launch via the app (./launch.sh) instead of a browser.',
      );
      setStatus(null);
      refreshInFlight.current = false;
      return;
    }
    getStatus()
      .then((s) => {
        setStatus(s);
        setError(null);
        onSafeMode?.(!!s.safe_mode);
      })
      .catch((e) => {
        console.error(e);
        setError(displayError(e));
        setStatus(null);
      })
      .finally(() => {
        refreshInFlight.current = false;
      });
  }, [onSafeMode]);

  useEffect(() => {
    refresh();
    if (!tauriAvailable()) return;
    getLogTail()
      .then((tail) => setLogTail(String(tail)))
      .catch((e) => {
        console.warn(`[useStatusPolling::loadLogTail] Failed to read log tail: ${errorMessage(e)}`);
      });
    const id = setInterval(() => {
      if (document.hidden) return;
      refresh();
    }, UI_TUNING.statusRefreshMs);
    return () => clearInterval(id);
  }, [refresh]);

  useEffect(() => {
    if (!tauriAvailable()) return;
    checkService()
      .then((s) => setServiceStatus(s))
      .catch((e) => {
        console.warn(
          `[useStatusPolling::checkService] Failed to read service status: ${errorMessage(e)}`,
        );
      });
  }, []);

  return { status, error, logTail, serviceStatus, setServiceStatus, refresh };
}

