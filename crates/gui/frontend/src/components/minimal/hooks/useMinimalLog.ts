import { useCallback, useEffect, useState } from 'react';
import { getLogTail } from '../../../services/logs';

type EventKind = 'ok' | 'error' | 'info';

type Params = {
  onEvent: (msg: string, kind?: EventKind) => void;
  setBusy: (busy: boolean) => void;
};

type MinimalLogState = {
  showLog: boolean;
  logTail: string;
  logLoading: boolean;
  setShowLog: (show: boolean | ((prev: boolean) => boolean)) => void;
  refreshLog: () => Promise<void>;
};

/**
 * Summary: Manage the minimal log tail state and refresh behavior.
 *
 * Inputs: Event handler for errors and a shared busy setter for UI disabling.
 * Outputs: `showLog` state, current `logTail`, a show setter, and refresh handler.
 * Side effects: Reads the log tail via IPC-backed services.
 * Error handling: Emits user-visible errors via `onEvent`.
 * Ties to other methods: Used by `MinimalMain` and `LogCard` to display and refresh logs.
 * Why this exists: Keep log concerns isolated so the main screen focuses on orchestration.
 */
export function useMinimalLog({ onEvent, setBusy }: Params): MinimalLogState {
  const [showLog, setShowLog] = useState(false);
  const [logTail, setLogTail] = useState<string>('');
  const [logLoading, setLogLoading] = useState(false);

  /**
   * Summary: Refresh the log tail from the backend.
   *
   * Inputs: None.
   * Outputs: None.
   * Side effects: Performs IPC calls and updates local state.
   * Error handling: Emits a user-visible error when the log cannot be read.
   * Ties to other methods: Used by the `showLog` effect and the LogCard refresh button.
   * Why this exists: Centralize log tail retrieval and keep busy state consistent.
   */
  const refreshLog = useCallback(async () => {
    try {
      setBusy(true);
      setLogLoading(true);
      const tail = await getLogTail();
      setLogTail(tail);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      onEvent(`Failed to load log tail: ${reason}`, 'error');
    } finally {
      setLogLoading(false);
      setBusy(false);
    }
  }, [onEvent, setBusy]);

  useEffect(() => {
    if (!showLog) return;
    void refreshLog();
  }, [refreshLog, showLog]);

  return { showLog, logTail, logLoading, setShowLog, refreshLog };
}
