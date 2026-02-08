import React, { useState } from 'react';
import { runNow, runSimulation } from '../services/backup';
import { getLogTail } from '../services/logs';
import { formatBytes } from '../utils/format';
import { ActionLogEntry } from './status/ActionLogFlyout';
import { SimulationResult } from '../services/types';
import { UI_TUNING } from '../config/uiTuning';
import { Button } from './ui/Button';

type Props = { onEvent?: (msg: string, kind?: ActionLogEntry['kind']) => void; safeMode?: boolean };
type PlanError = { message?: string; code?: string };

const PLAN_TOO_LARGE_CODE = UI_TUNING.planTooLargeCode;
const { maxLogLines, logPreviewMaxHeightPx, logPreviewPaddingPx, logPreviewRadiusPx } =
  UI_TUNING.runNow;

/**
 * Purpose: Format a plan error with context and notify the UI where applicable.
 *
 * Inputs: `err` as the thrown value, `label` for the action label, callbacks for UI updates.
 * Outputs: A formatted error message string for UI display.
 * Ties to: Manual run and simulation flows that surface planning errors.
 * Side effects: Emits UI events and triggers plan overflow callbacks.
 * Why: Keeps error messaging consistent and actionable for operators.
 */
const formatPlanError = (
  err: unknown,
  label: string,
  onEvent?: (msg: string, kind?: ActionLogEntry['kind']) => void,
  onPlanTooLarge?: () => void,
) => {
  try {
    const errObj = err as PlanError;
    const msg = errObj?.message || String(err);
    const extra =
      errObj?.code === PLAN_TOO_LARGE_CODE
        ? ' Plan is too large; narrow watched scope or add ignores.'
        : '';
    const full = `${msg}${extra}`;
    onEvent?.(`${label} failed: ${full}`, 'error');
    if (errObj?.code === PLAN_TOO_LARGE_CODE) {
      onPlanTooLarge?.();
    }
    return full;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    return `[formatPlanError] Failed to format plan error for ${label}: ${reason}`;
  }
};

/**
 * Purpose: Render the manual run and simulation actions for backups.
 *
 * Inputs: `onEvent` callback and `safeMode` flag from parent UI state.
 * Outputs: A React element tree with manual action buttons and feedback.
 * Ties to: Backup service calls and the activity log flyout.
 * Side effects: Registers React state hooks and triggers backend actions from handlers.
 * Why: Allows operators to trigger backups on demand with feedback.
 */
export const RunNow: React.FC<Props> = ({ onEvent, safeMode }) => {
  const [message, setMessage] = useState<string>('');
  const [log, setLog] = useState<string>('');
  const [loading, setLoading] = useState<boolean>(false);
  const [simulating, setSimulating] = useState<boolean>(false);

  /**
   * Purpose: Trigger a manual backup run and update UI feedback.
   *
   * Inputs: None.
   * Outputs: Updates UI state with the result message and log tail.
   * Ties to: `runNow`, `getLogTail`, and activity notifications.
   * Side effects: Updates state, invokes IPC, fetches log tail, and emits activity events.
   * Why: Lets operators trigger immediate backups safely.
   */
  const handleRun = async () => {
    try {
      setLoading(true);
      await runNow();
      setMessage('Manual backup triggered.');
      onEvent?.('Manual backup triggered', 'ok');
      const tail = await getLogTail();
      const lines = String(tail).split('\n');
      setLog(lines.slice(-maxLogLines).join('\n'));
    } catch (e) {
      try {
        const full = formatPlanError(e, 'Manual run', onEvent, () => {
          setLog(
            'Too many files to back up. Add ignores (node_modules, build, target, logs) or narrow watched scope, then try again.',
          );
        });
        setMessage(full);
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        setMessage(`[handleRun] Failed to handle manual run error: ${reason}`);
      }
    } finally {
      setLoading(false);
    }
  };

  /**
   * Purpose: Trigger a backup simulation run and update UI feedback.
   *
   * Inputs: None.
   * Outputs: Updates UI state with simulation summary.
   * Ties to: `runSimulation` and activity notifications.
   * Side effects: Updates state, invokes IPC, and emits activity events.
   * Why: Provides a safe preview of planned backup operations.
   */
  const handleSimulate = async () => {
    try {
      setSimulating(true);
      const res: SimulationResult = await runSimulation();
      const summary = `Simulation: ${res.items} items, ${formatBytes(res.bytes)}.`;
      const sample = res.sample && res.sample.length > 0 ? ` Sample: ${res.sample.join(', ')}` : '';
      const msg = `${summary}${sample}`;
      setMessage(msg);
      onEvent?.(msg, res.items === 0 ? 'info' : 'ok');
    } catch (e) {
      try {
        const full = formatPlanError(e, 'Simulation', onEvent);
        setMessage(full);
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        setMessage(`[handleSimulate] Failed to handle simulation error: ${reason}`);
      }
    } finally {
      setSimulating(false);
    }
  };

  try {
    return (
      <div className="card">
        <div className="section-title">
          <h3>Manual run</h3>
          <span className="pill">Instant</span>
        </div>
        <p className="muted mt-0">
          Trigger a backup cycle right now without waiting for the scheduler.
        </p>
        <div className="inline-actions">
          <Button
            onClick={handleRun}
            disabled={safeMode}
            loading={loading}
            loadingLabel="Running..."
          >
            {safeMode ? 'Safe mode enabled' : loading ? 'Running...' : 'Run backup now'}
          </Button>
          <Button
            tone="secondary"
            onClick={handleSimulate}
            loading={simulating}
            loadingLabel="Simulating..."
          >
            Simulate backup
          </Button>
          {message && <span className="muted">{message}</span>}
        </div>
        {log && (
          <pre
            className="log-preview"
            style={{
              maxHeight: logPreviewMaxHeightPx,
              padding: logPreviewPaddingPx,
              borderRadius: logPreviewRadiusPx,
            }}
          >
            {log}
          </pre>
        )}
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[RunNow] Failed to render manual run panel: ${reason}`);
  }
};
