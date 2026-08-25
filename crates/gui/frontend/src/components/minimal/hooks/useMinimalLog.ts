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

/** Keep log concerns isolated so the main screen focuses on orchestration. */
export function useMinimalLog({ onEvent, setBusy }: Params): MinimalLogState {
  const [showLog, setShowLog] = useState(false);
  const [logTail, setLogTail] = useState<string>('');
  const [logLoading, setLogLoading] = useState(false);

  /** Centralize log tail retrieval and keep busy state consistent. */
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
