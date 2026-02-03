import React, { useEffect, useState } from 'react';
import {
  getStatus,
  verifyBackups,
  installService,
  exportLogs,
  checkUpdates,
  exportHealthReport,
  restartDaemon,
  doctorReport,
} from '../../services';
import { formatBytes, formatDateTime } from '../../utils/format';
import OfflineNotice from './OfflineNotice';
import HealthRow from './HealthRow';
import VerifyBlock from './VerifyBlock';
import ActivityFeed from './ActivityFeed';
import { ActionLogEntry } from './ActionLogFlyout';
import { checkService } from '../../services/system';
import AdvancedPane from './AdvancedPane';
import ServiceBanner from './ServiceBanner';
import PlanModal from './PlanModal';
import LowSpaceGuard from './LowSpaceGuard';
import { StatusDto, ServiceStatusDto, VerifyResult } from '../../services/types';
import { getLogTail } from '../../services/logs';
import { IpcError, tauriAvailable } from '../../services/ipc';
import { UI_TUNING } from '../../config/uiTuning';

type Props = {
  onEvent?: (msg: string, kind?: ActionLogEntry['kind']) => void;
  onSafeMode?: (v: boolean) => void;
};

/**
 * Purpose: Normalize unknown errors into a safe message string.
 *
 * Inputs: Unknown error value.
 * Outputs: A string message suitable for UI and logs.
 * Ties to: Status refresh and action error handling.
 * Side effects: None.
 * Why: Avoid unsafe casts while preserving error context.
 */
const errorMessage = (error: unknown): string => {
  if (error instanceof Error) return error.message;
  return String(error);
};

/**
 * Purpose: Convert known IPC error codes into a user-facing message.
 *
 * Inputs: IPC error or unknown error payload.
 * Outputs: A user friendly message string.
 * Ties to: Status refresh and offline banners.
 * Side effects: None.
 * Why: Keep UI messages actionable without leaking internal stack context.
 */
const displayError = (error: unknown): string => {
  if (error instanceof IpcError) {
    if (error.code === 'DAEMON_OFFLINE') {
      return 'Not connected yet. Finish setup, or enable Start on login to keep the daemon running.';
    }
    if (error.code === 'IPC_TIMEOUT') {
      return 'Daemon did not respond in time. If this persists, restart the daemon or check logs.';
    }
    if (error.code === 'TAURI_UNAVAILABLE') {
      return 'IPC unavailable. Launch the desktop app (./start) instead of a browser.';
    }
    return error.message;
  }
  return errorMessage(error);
};

/**
 * Purpose: Read the resume on space preference from storage.
 *
 * Inputs: None.
 * Outputs: Boolean value indicating resume behavior.
 * Ties to: Low space guard toggle state and settings sync.
 * Side effects: Reads from localStorage.
 * Why: Keep the UI in sync with persisted resume behavior.
 */
const readResumeOnSpace = (): boolean => {
  try {
    return localStorage.getItem(UI_TUNING.resumeOnSpaceStorageKey) === '1';
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.warn(`[StatusCard::readResumeOnSpace] Failed to read local storage: ${reason}`);
    return false;
  }
};

/**
 * Purpose: Persist the resume on space preference to storage.
 *
 * Inputs: `next` as the desired resume state.
 * Outputs: None.
 * Ties to: Low space guard toggle changes.
 * Side effects: Writes to localStorage.
 * Why: Preserve operator preference across app sessions.
 */
const writeResumeOnSpace = (next: boolean): void => {
  try {
    localStorage.setItem(UI_TUNING.resumeOnSpaceStorageKey, next ? '1' : '0');
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.warn(`[StatusCard::writeResumeOnSpace] Failed to write local storage: ${reason}`);
  }
};

/**
 * Purpose: Render the live status card and action controls.
 *
 * Inputs: Optional event and safe mode callbacks.
 * Outputs: A status card element with live data and actions.
 * Ties to: Status polling, action handlers, and settings storage.
 * Side effects: Registers React hooks, schedules polling, and invokes IPC calls.
 * Why: Provide a unified control surface for daemon status and actions.
 */
const StatusCard: React.FC<Props> = ({ onEvent, onSafeMode }) => {
  const [status, setStatus] = useState<StatusDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [verifyMsg, setVerifyMsg] = useState<string>('');
  const [verifying, setVerifying] = useState<boolean>(false);
  const [actionMsg, setActionMsg] = useState<string>('');
  const [toast, setToast] = useState<{ msg: string; kind: 'ok' | 'error' } | null>(null);
  const [serviceStatus, setServiceStatus] = useState<ServiceStatusDto | null>(null);
  const [planModal, setPlanModal] = useState<string | null>(null);
  const [logTail, setLogTail] = useState<string>('');
  const [resumeOnSpace, setResumeOnSpace] = useState<boolean>(readResumeOnSpace);
  const refreshInFlight = React.useRef(false);

  /**
   * Purpose: Refresh status and log tail from the daemon.
   *
   * Inputs: None.
   * Outputs: Updates local component state.
   * Ties to: Status API calls and safe mode updates.
   * Side effects: Invokes IPC calls and updates React state.
   * Why: Keep status panels current without redundant polling.
   */
  const refresh = React.useCallback(() => {
    if (refreshInFlight.current) return;
    refreshInFlight.current = true;
    if (!tauriAvailable()) {
      setError(
        '[StatusCard::refresh] Tauri IPC unavailable. Please launch via the app (./launch.sh) instead of a browser.',
      );
      setStatus(null);
      setActionMsg('');
      refreshInFlight.current = false;
      return;
    }
    getStatus()
      .then((s) => {
        setStatus(s);
        setError(null);
        setActionMsg('');
        onSafeMode?.(!!s.safe_mode);
      })
      .catch((e) => {
        console.error(e);
        setError(displayError(e));
        setStatus(null);
        setActionMsg('');
      })
      .finally(() => {
        refreshInFlight.current = false;
      });
  }, [onSafeMode]);

  useEffect(() => {
    if (!tauriAvailable()) {
      refresh();
      return;
    }
    refresh();
    getLogTail()
      .then((tail) => setLogTail(String(tail)))
      .catch((e) => {
        console.warn(`[StatusCard::loadLogTail] Failed to read log tail: ${errorMessage(e)}`);
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
          `[StatusCard::checkService] Failed to read service status: ${errorMessage(e)}`,
        );
      });
  }, []);

  /**
   * Purpose: Toggle resume behavior for low space guard.
   *
   * Inputs: `next` as the desired resume state.
   * Outputs: Updates state and storage preference.
   * Ties to: Low space guard UI and persistent settings.
   * Side effects: Updates React state and writes to localStorage.
   * Why: Allow operators to pause or resume when space recovers.
   */
  const toggleResumeOnSpace = (next: boolean) => {
    try {
      setResumeOnSpace(next);
      writeResumeOnSpace(next);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      const msg = `[StatusCard::toggleResumeOnSpace] Failed to toggle resume on space: ${reason}`;
      setToast({ msg, kind: 'error' });
      onEvent?.(msg, 'error');
    }
  };

  /**
   * Purpose: Open the plan size modal and surface the warning toast.
   *
   * Inputs: `msg` as the plan overflow message.
   * Outputs: Updates modal and toast state.
   * Ties to: Plan size validation from the backend.
   * Side effects: Updates React state for modal and toast.
   * Why: Guide users to reduce scope when plan size is too large.
   */
  const handlePlanTooLarge = (msg: string) => {
    try {
      setPlanModal(msg);
      setToast({ msg, kind: 'error' });
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      const fullMsg = `[StatusCard::handlePlanTooLarge] Failed to open plan modal: ${reason}`;
      setToast({ msg: fullMsg, kind: 'error' });
      onEvent?.(fullMsg, 'error');
    }
  };

  /**
   * Purpose: Surface success feedback after an action completes.
   *
   * Inputs: `label` and action result payload.
   * Outputs: Updates action message, toast, and external event log.
   * Ties to: Action button handlers and status refresh.
   * Side effects: Updates React state and emits external events.
   * Why: Keep operators informed about completed actions.
   */
  const handleActionSuccess = (label: string, res: unknown) => {
    try {
      const msg = typeof res === 'string' ? res : `${label} done`;
      setActionMsg(msg || `${label} done`);
      setToast({ msg: msg || `${label} done`, kind: 'ok' });
      onEvent?.(msg || `${label} done`, 'ok');
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      const fullMsg = `[StatusCard::handleActionSuccess] Failed to surface success: ${reason}`;
      setToast({ msg: fullMsg, kind: 'error' });
      onEvent?.(fullMsg, 'error');
    }
  };

  /**
   * Purpose: Surface error feedback after an action fails.
   *
   * Inputs: `label` and error payload.
   * Outputs: Updates action message, toast, and external event log.
   * Ties to: Action button handlers and plan size detection.
   * Side effects: Updates React state, emits external events, and may open the plan modal.
   * Why: Provide precise failure context and remediation guidance.
   */
  const handleActionError = (label: string, e: unknown) => {
    try {
      const errObj = e as { message?: string; code?: string };
      const msg = errObj?.message || String(e);
      const code = errObj?.code;
      const extra =
        code === UI_TUNING.planTooLargeCode
          ? ' Plan too large; narrow watched scope or add ignores.'
          : '';
      const logMsg = `[StatusCard::handleActionError] ${label} failed: ${msg}${extra}`;

      let userMsg = `${label} failed.`;
      if (code === UI_TUNING.planTooLargeCode) {
        userMsg = 'Too many files to back up at once. Add ignores or narrow watched folders.';
      } else if (msg.includes('no destinations configured')) {
        userMsg = 'Finish setup first: choose where to store backups, then try again.';
      } else if (msg.includes('no watched paths configured')) {
        userMsg = 'Finish setup first: add a folder or file to protect, then try again.';
      } else if (code === 'TAURI_UNAVAILABLE') {
        userMsg = 'This action requires the desktop app. Launch via ./start.';
      } else if (code === 'DAEMON_OFFLINE') {
        userMsg = 'Not connected yet. Finish setup, then enable run in background.';
      } else if (typeof msg === 'string' && msg.trim().length > 0) {
        userMsg = msg;
      }

      setActionMsg(userMsg);
      setToast({ msg: userMsg, kind: 'error' });
      onEvent?.(logMsg, 'error');
      if (code === UI_TUNING.planTooLargeCode) {
        handlePlanTooLarge(
          'Too many files to back up in one go. Add ignore patterns (node_modules, build, cache) or narrow watched folders, then try again.',
        );
      }
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      const fullMsg = `[StatusCard::handleActionError] Failed to surface error: ${reason}`;
      setToast({ msg: fullMsg, kind: 'error' });
      onEvent?.(fullMsg, 'error');
    }
  };

  /**
   * Purpose: Trigger a verification run and report results.
   *
   * Inputs: None.
   * Outputs: Updates verification status and toast feedback.
   * Ties to: Verify action in the status panel.
   * Side effects: Invokes IPC verification, updates React state, and emits events.
   * Why: Provide an on demand integrity check.
   */
  const onVerify = async () => {
    if (!tauriAvailable()) {
      setToast({
        msg: '[StatusCard::onVerify] IPC unavailable. Launch the app build to verify.',
        kind: 'error',
      });
      return;
    }
    try {
      setVerifying(true);
      const res: VerifyResult = await verifyBackups();
      setVerifyMsg(`Verified: ${res.ok} ok, ${res.bad} issues`);
      setToast({
        msg: `Verification finished (${res.ok} ok, ${res.bad} issues)`,
        kind: res.bad > 0 ? 'error' : 'ok',
      });
      onEvent?.(`Verify finished (${res.ok} ok, ${res.bad} issues)`, res.bad > 0 ? 'error' : 'ok');
      refresh();
    } catch (e) {
      const msg = `[StatusCard::onVerify] Verify failed: ${String(e)}`;
      setVerifyMsg(msg);
      setToast({ msg, kind: 'error' });
      onEvent?.(msg, 'error');
    } finally {
      setVerifying(false);
    }
  };

  useEffect(() => {
    if (!toast) return;
    const id = setTimeout(() => setToast(null), UI_TUNING.toastDismissMs);
    return () => clearTimeout(id);
  }, [toast]);

  /**
   * Purpose: Run an action with optional confirmation and feedback handling.
   *
   * Inputs: Action function, label, and optional confirmation settings.
   * Outputs: Updates action status, toast messaging, and status refresh.
   * Ties to: All action buttons in the status panel.
   * Side effects: Invokes IPC actions, updates React state, and shows confirm dialogs.
   * Why: Keep action execution paths consistent and safe.
   */
  const runAction = async <T,>(
    fn: () => Promise<T>,
    label: string,
    opts?: { confirm?: string },
  ) => {
    if (!tauriAvailable()) {
      setToast({
        msg: '[StatusCard::runAction] IPC unavailable. Launch the app build to run actions.',
        kind: 'error',
      });
      return;
    }
    if (opts?.confirm && !window.confirm(opts.confirm)) {
      return;
    }
    try {
      setActionMsg(`Working on ${label}...`);
      const res = await fn();
      handleActionSuccess(label, res);
      if (label.toLowerCase().includes('login') || label.toLowerCase().includes('daemon')) {
        try {
          const s = await checkService();
          setServiceStatus(s);
        } catch (e) {
          console.warn(
            `[StatusCard::runAction] Failed to refresh service status: ${errorMessage(e)}`,
          );
        }
      }
      refresh();
    } catch (e) {
      handleActionError(label, e);
    }
  };

  if (!status) {
    return (
      <OfflineNotice
        error={error}
        message={error || 'Not connected yet. Start the daemon or complete setup.'}
        onInstallService={() => runAction(installService, 'Start on login')}
        onExportLogs={() =>
          runAction(exportLogs, 'Export logs', { confirm: 'Export logs to your Desktop?' })
        }
      />
    );
  }

  return (
    <div className="card">
      {serviceStatus && (
        <ServiceBanner
          message={serviceStatus.message}
          reachable={serviceStatus.reachable}
          fixCommand={serviceStatus.fix_command ?? ''}
          uptimeSecs={serviceStatus.uptime_secs ?? null}
          lastIpcTs={serviceStatus.last_ipc_ts ?? null}
          onFix={() => runAction(installService, 'Start on login')}
        />
      )}
      <div className="section-title">
        <h3>Live status</h3>
        <span className="pill">{error ? 'Issue' : 'Online'}</span>
      </div>
      {status.safe_mode && (
        <div className="service-banner warn">
          <div>Safe mode enabled: scan/verify only. Writes are paused.</div>
        </div>
      )}
      <HealthRow
        version={status.version}
        uptime_secs={status.uptime_secs}
        free_bytes={status.free_bytes}
        safe_mode={status.safe_mode ?? false}
      />
      <LowSpaceGuard
        freeBytes={status.free_bytes}
        threshold={UI_TUNING.lowSpaceThresholdBytes}
        resumeOnSpace={resumeOnSpace}
        onToggleResume={toggleResumeOnSpace}
      />
      <div className="divider" />
      <div className="muted">Destinations</div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6, marginTop: 6 }}>
        {(status.destinations || []).length === 0 && <div className="muted">None configured</div>}
        {(status.destinations || []).map((d) => (
          <div
            key={d.id}
            style={{
              display: 'flex',
              flexDirection: 'column',
              padding: '8px 10px',
              borderRadius: 8,
              background: 'rgba(255,255,255,0.03)',
              border: '1px solid rgba(255,255,255,0.05)',
            }}
          >
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
              <div style={{ fontWeight: 600 }}>{d.label || d.id}</div>
              <div className="pill">{formatBytes(d.free_bytes)}</div>
            </div>
            <div className="muted" style={{ wordBreak: 'break-all' }}>
              {d.path}
            </div>
          </div>
        ))}
      </div>
      {status.min_free_space_bytes != null &&
        status.free_bytes != null &&
        status.free_bytes < status.min_free_space_bytes && (
          <div
            className="pill"
            style={{ borderColor: '#ff7b7b', color: '#ffb0b0', marginTop: 6 }}
            title="Free-space guard pauses backups until space recovers."
          >
            Paused: below free space guard ({status.free_bytes} / {status.min_free_space_bytes}{' '}
            bytes)
          </div>
        )}
      <div className="metric-row">
        <div>
          <div className="muted">Last run</div>
          <div>{formatDateTime(status.last_run_ts)}</div>
        </div>
        <div>
          <div className="muted">Backed up</div>
          <div>{status.last_files_backed_up}</div>
        </div>
        <div>
          <div className="muted">Dirty queue</div>
          <div>{status.last_dirty_count ?? 0}</div>
        </div>
        <div>
          <div className="muted">Verify issues</div>
          <div>{status.last_verify_issues ?? 0}</div>
        </div>
      </div>
      <div className="divider" />
      <VerifyBlock
        last_verify_ts={status.last_verify_ts}
        last_verify_status={status.last_verify_status}
        last_verify_issues={status.last_verify_issues}
        onVerify={onVerify}
        verifying={verifying}
        verifyMsg={verifyMsg}
      />
      <div className="divider" />
      <div className="muted">Last error</div>
      <div>{status.last_error ?? 'None'}</div>
      <div className="divider" />
      <div className="inline-actions" style={{ marginBottom: 8 }}>
        <button
          className="btn secondary"
          onClick={() =>
            runAction(installService, 'Start on login', {
              confirm: 'Enable the background helper to start on login?',
            })
          }
        >
          Start on login
        </button>
        <button
          className="btn secondary"
          onClick={() =>
            runAction(exportLogs, 'Export logs', { confirm: 'Export logs to your Desktop?' })
          }
        >
          Export logs
        </button>
        <button className="btn secondary" onClick={() => runAction(checkUpdates, 'Check updates')}>
          Check updates
        </button>
        <button
          className="btn secondary"
          onClick={() =>
            runAction(exportHealthReport, 'Export health report', {
              confirm: 'Export a health report to your Desktop?',
            })
          }
        >
          Export health
        </button>
        <button
          className="btn secondary"
          onClick={() =>
            runAction(doctorReport, 'Doctor report', {
              confirm: 'Run and save a doctor report to your Desktop?',
            })
          }
        >
          Doctor report
        </button>
        <button
          className="btn secondary"
          onClick={() =>
            runAction(
              () => import('../../services/system').then((m) => m.exportDiagnosticBundle()),
              'Diagnostic bundle',
              { confirm: 'Create a diagnostic bundle on your Desktop?' },
            )
          }
        >
          Diagnostic bundle
        </button>
        <button
          className="btn secondary"
          onClick={() =>
            runAction(restartDaemon, 'Restart daemon', {
              confirm: 'Restart the background daemon now?',
            })
          }
        >
          Restart daemon
        </button>
        <span className="muted">{actionMsg}</span>
      </div>
      <div className="divider" />
      <ActivityFeed items={status.recent_activity || []} />
      <AdvancedPane status={status} logTail={logTail} />
      {error && <div style={{ color: '#ff7b7b', marginTop: 8 }}>Status error: {error}</div>}
      {toast && (
        <div className={`toast ${toast.kind === 'error' ? 'toast-error' : 'toast-ok'}`}>
          {toast.msg}
        </div>
      )}
      <PlanModal message={planModal} onClose={() => setPlanModal(null)} />
    </div>
  );
};

export default StatusCard;
