import { useState } from 'react';
import { Config } from '../../../domain/config';
import { setSafeMode } from '../helpers/safeMode';

type EventKind = 'ok' | 'error' | 'info';

type Params = {
  cfg: Config | null;
  setCfg: (cfg: Config | null) => void;
  onEvent: (msg: string, kind?: EventKind) => void;
};

type MinimalRunningState = {
  runningBusy: boolean;
  setRunning: (running: boolean) => Promise<void>;
};

/**
 * Summary: Provide the minimal running toggle behavior using safe mode semantics.
 *
 * Inputs: Current config, config setter, and an event handler for user-visible messages.
 *
 * Outputs: Busy flag and a `setRunning` handler.
 *
 * Side effects: Optimistically updates local config and invokes IPC to update safe mode.
 *
 * Error handling: Emits an actionable error message and restores prior config state on failure.
 *
 * Ties to other methods: Used by `MinimalHeader` via `MinimalMain` for the Running switch.
 *
 * Why this exists: Keep the running toggle logic isolated and reusable without duplicating IPC calls.
 */
export function useMinimalRunning({ cfg, setCfg, onEvent }: Params): MinimalRunningState {
  const [runningBusy, setRunningBusy] = useState(false);

  /**
   * Summary: Apply the running toggle as a safe mode update, with optimistic UI.
   *
   * Inputs: `running` desired running state.
   *
   * Outputs: None.
   *
   * Side effects: Updates local state and writes safe mode via IPC.
   *
   * Error handling: Restores previous state and rethrows to allow caller error formatting.
   *
   * Ties to other methods: Calls `setSafeMode` to update the daemon when reachable.
   *
   * Why this exists: Avoid UI flicker by keeping the safe mode flip local while IPC is in flight.
   */
  const applyRunningChange = async (running: boolean) => {
    if (!cfg) return;
    const desiredSafeMode = !running;
    const previousSafeMode = cfg.safe_mode;
    setCfg({ ...cfg, safe_mode: desiredSafeMode });
    try {
      const nextSafeMode = await setSafeMode(desiredSafeMode);
      setCfg({ ...cfg, safe_mode: nextSafeMode });
      if (nextSafeMode !== previousSafeMode) {
        onEvent(nextSafeMode ? 'Paused.' : 'Running.', 'ok');
      }
    } catch (error) {
      setCfg({ ...cfg, safe_mode: previousSafeMode });
      throw error;
    }
  };

  /**
   * Summary: Public running toggle handler with busy state and error formatting.
   *
   * Inputs: `running` desired running state.
   *
   * Outputs: None.
   *
   * Side effects: Updates busy flag and may update config.
   *
   * Error handling: Emits a user-visible error and leaves config unchanged on failure.
   *
   * Ties to other methods: Wraps `applyRunningChange`.
   *
   * Why this exists: Provide a simple handler surface to the UI components.
   */
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
