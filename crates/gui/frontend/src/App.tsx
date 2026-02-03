import { useMemo, useState } from 'react';
import StatusCard from './components/status/StatusCard';
import { SettingsPanel } from './components/SettingsPanel';
import { RunNow } from './components/RunNow';
import ActionLogFlyout, { ActionLogEntry } from './components/status/ActionLogFlyout';
import { UI_TUNING } from './config/uiTuning';
import { MinimalMain } from './components/minimal/MinimalMain';

/**
 * Purpose: Render the top-level application shell and coordinate shared UI state.
 *
 * Inputs: None.
 * Outputs: A React element tree that wires the status, run, and settings panels.
 * Ties to: `StatusCard`, `RunNow`, `SettingsPanel`, and `ActionLogFlyout` via props.
 * Side effects: Registers React state hooks for action log and safe mode.
 * Why: Centralizes cross-panel state so activity logging and safe mode stay consistent.
 */
export default function App() {
  const [actionLog, setActionLog] = useState<ActionLogEntry[]>([]);
  const [safeMode, setSafeMode] = useState<boolean>(false);
  const legacy = useMemo(() => {
    try {
      return new URLSearchParams(window.location.search).has('legacy');
    } catch {
      return false;
    }
  }, []);
  /**
   * Purpose: Append a new action entry to the in-memory activity feed.
   *
   * Inputs: `msg` as the human readable message; `kind` as the status type.
   * Outputs: Updates `actionLog` state used by `ActionLogFlyout`.
   * Ties to: `ActionLogFlyout` and `StatusCard` event callbacks.
   * Side effects: Updates React state for the action log.
   * Why: Keeps operator feedback visible without touching persistent state.
   */
  const recordAction = (msg: string, kind: 'ok' | 'error' | 'info' = 'info') => {
    try {
      setActionLog((prev) => {
        const next = [...prev, { msg, kind, ts: Date.now() / 1000 }];
        return next.slice(-UI_TUNING.actionLogLimit);
      });
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      throw new Error(`[App.recordAction] Failed to append action log entry: ${reason}`);
    }
  };

  try {
    if (legacy) {
      return (
        <div className="app">
          <div className="hero">
            <h1>Local Backup Manager</h1>
            <ActionLogFlyout items={actionLog} />
          </div>
          <div className="grid">
            <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
              <StatusCard onEvent={recordAction} onSafeMode={setSafeMode} />
              <RunNow onEvent={recordAction} safeMode={safeMode} />
            </div>
            <div style={{ alignSelf: 'start' }}>
              <SettingsPanel />
            </div>
          </div>
        </div>
      );
    }
    return <MinimalMain onEvent={recordAction} />;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[App] Failed to render application shell: ${reason}`);
  }
}
