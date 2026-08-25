import { useState } from 'react';
import { Config } from '../../../domain/config';
import { setSafeMode } from '../helpers/safeMode';

type EventKind = 'ok' | 'error' | 'info';

type Params = {
  cfg: Config | null;
  setCfg: (cfg: Config | null) => void;
  onEvent: (msg: string, kind?: EventKind) => void;
  onLiveSafeModeConfirmed: (safeMode: boolean | null) => void;
};

type MinimalRunningState = {
  runningBusy: boolean;
  setRunning: (running: boolean) => Promise<void>;
};

/** Keep the running toggle logic isolated and reusable without duplicating IPC calls. */
export function useMinimalRunning({
  cfg,
  setCfg,
  onEvent,
  onLiveSafeModeConfirmed,
}: Params): MinimalRunningState {
  const [runningBusy, setRunningBusy] = useState(false);

  /** Avoid UI flicker by keeping the safe mode flip local while IPC is in flight. */
  const applyRunningChange = async (running: boolean) => {
    if (!cfg) return;
    const desiredSafeMode = !running;
    const previousSafeMode = cfg.safe_mode;
    setCfg({ ...cfg, safe_mode: desiredSafeMode });
    try {
      const result = await setSafeMode(desiredSafeMode);
      setCfg({ ...cfg, safe_mode: result.safe_mode });
      if (result.warning) {
        onLiveSafeModeConfirmed(null);
        onEvent(result.warning, 'info');
      } else if (result.safe_mode !== previousSafeMode && result.applied_live) {
        onLiveSafeModeConfirmed(result.safe_mode);
        onEvent(result.safe_mode ? 'Paused.' : 'Running.', 'ok');
      }
    } catch (error) {
      setCfg({ ...cfg, safe_mode: previousSafeMode });
      throw error;
    }
  };

  /** Provide a simple handler surface to the UI components. */
  const setRunning = async (running: boolean) => {
    if (!cfg) return;
    try {
      setRunningBusy(true);
      await applyRunningChange(running);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      onEvent(`Failed to change running state: ${reason}`, 'error');
    } finally {
      setRunningBusy(false);
    }
  };

  return { runningBusy, setRunning };
}
