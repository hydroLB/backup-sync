import { useCallback, useEffect, useMemo, useState } from 'react';
import { verifyBackups, exportLogs, installService } from '../../../../services';
import { checkService } from '../../../../services/system';
import { tauriAvailable } from '../../../../services/ipc';
import { UI_TUNING } from '../../../../config/uiTuning';
import { ServiceStatusDto, VerifyResult } from '../../../../services/types';
import { ActionLogEntry } from '../../ActionLogFlyout';
import { errorMessage } from '../utils/errors';
import { readResumeOnSpace, writeResumeOnSpace } from '../utils/resumeOnSpace';

type Toast = { msg: string; kind: 'ok' | 'error' } | null;

type Params = {
  onEvent?: (msg: string, kind?: ActionLogEntry['kind']) => void;
  refresh: () => void;
  setServiceStatus: (s: ServiceStatusDto | null) => void;
};

type State = {
  verifying: boolean;
  actionBusy: boolean;
  verifyMsg: string;
  actionMsg: string;
  toast: Toast;
  notify: (msg: string, kind: 'ok' | 'error') => void;
  planModal: string | null;
  setPlanModal: (msg: string | null) => void;
  resumeOnSpace: boolean;
  toggleResumeOnSpace: (next: boolean) => void;
  onVerify: () => Promise<void>;
  runAction: (
    fn: () => Promise<unknown>,
    label: string,
    opts?: { confirm?: string },
  ) => Promise<void>;
  offlineActions: {
    onInstallService: () => Promise<void>;
    onExportLogs: () => Promise<void>;
  };
};

/**
 * Summary: Provide status-card action handlers (verify, service actions, exports) with consistent feedback.
 *
 * Inputs: Optional event logger, a `refresh` function, and a service status setter.
 * Outputs: Action state (messages/toast/modals) plus stable action handlers.
 * Side effects: Invokes IPC services, updates localStorage, and triggers refreshes.
 * Error handling: Emits actionable errors via toast and optional external `onEvent`.
 * Ties to other methods: Used by `StatusCard` and online/offline views for actions.
 * Why this exists: Keep the rendering component focused on layout while behavior lives in one place.
 */
export function useStatusActions({ onEvent, refresh, setServiceStatus }: Params): State {
  const [verifyMsg, setVerifyMsg] = useState<string>('');
  const [verifying, setVerifying] = useState<boolean>(false);
  const [actionBusy, setActionBusy] = useState<boolean>(false);
  const [actionMsg, setActionMsg] = useState<string>('');
  const [toast, setToast] = useState<Toast>(null);
  const [planModal, setPlanModal] = useState<string | null>(null);
  const [resumeOnSpace, setResumeOnSpace] = useState<boolean>(readResumeOnSpace);

  /**
   * Summary: Emit a toast notification and forward it to the optional external event sink.
   *
   * Inputs: Message text and toast kind.
   * Outputs: None.
   * Side effects: Updates toast state and may call `onEvent`.
   * Error handling: None.
   * Ties to other methods: Used by transition notifications and generic action handlers.
   * Why this exists: Provide one feedback path for both inline actions and status transitions.
   */
  const notify = useCallback(
    (msg: string, kind: 'ok' | 'error') => {
      setToast({ msg, kind });
      onEvent?.(msg, kind);
    },
    [onEvent],
  );

  useEffect(() => {
    if (!toast) return;
    const id = setTimeout(() => setToast(null), UI_TUNING.toastDismissMs);
    return () => clearTimeout(id);
  }, [toast]);

  /**
   * Summary: Toggle resume behavior for low-space guard.
   *
   * Inputs: `next` desired resume state.
   * Outputs: None.
   * Side effects: Updates React state and writes to localStorage.
   * Error handling: Emits a toast and external event on failure.
   * Ties to other methods: Used by the low-space guard UI.
   * Why this exists: Preserve operator preference across sessions.
   */
  const toggleResumeOnSpace = useCallback(
    (next: boolean) => {
      try {
        setResumeOnSpace(next);
        writeResumeOnSpace(next);
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        const msg = `[useStatusActions::toggleResumeOnSpace] Failed to toggle resume on space: ${reason}`;
        notify(msg, 'error');
      }
    },
    [notify],
  );

  /**
   * Summary: Open the plan size modal and surface the warning toast.
   *
   * Inputs: Plan overflow message.
   * Outputs: None.
   * Side effects: Updates modal and toast state.
   * Error handling: Emits a toast and external event on failure.
   * Ties to other methods: Used by plan-too-large action errors.
   * Why this exists: Guide users to reduce scope when plan size is too large.
   */
  const handlePlanTooLarge = useCallback(
    (msg: string) => {
      try {
        setPlanModal(msg);
        setToast({ msg, kind: 'error' });
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        const fullMsg = `[useStatusActions::handlePlanTooLarge] Failed to open plan modal: ${reason}`;
        setToast({ msg: fullMsg, kind: 'error' });
        onEvent?.(fullMsg, 'error');
      }
    },
    [onEvent],
  );

  /**
   * Summary: Surface success feedback after an action completes.
   *
   * Inputs: Action label and result payload.
   * Outputs: None.
   * Side effects: Updates action message, toast, and external event log.
   * Error handling: Emits a toast and external event on failure.
   * Ties to other methods: Used by `runAction` success path.
   * Why this exists: Keep operators informed about completed actions.
   */
  const handleActionSuccess = useCallback(
    (label: string, res: unknown) => {
      try {
        const msg = typeof res === 'string' ? res : `${label} done`;
        setActionMsg(msg || `${label} done`);
        notify(msg || `${label} done`, 'ok');
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        const fullMsg = `[useStatusActions::handleActionSuccess] Failed to surface success: ${reason}`;
        notify(fullMsg, 'error');
      }
    },
    [notify],
  );

  /**
   * Summary: Surface error feedback after an action fails.
   *
   * Inputs: Action label and error payload.
   * Outputs: None.
   * Side effects: Updates action message, toast, external event log, and may open plan modal.
   * Error handling: Emits a toast and external event on failure.
   * Ties to other methods: Used by `runAction` error path.
   * Why this exists: Provide precise failure context and remediation guidance.
   */
  const handleActionError = useCallback(
    (label: string, e: unknown) => {
      try {
        const errObj = e as { message?: string; code?: string };
        const msg = errObj?.message || String(e);
        const code = errObj?.code;
        const extra =
          code === UI_TUNING.planTooLargeCode
            ? ' Plan too large; narrow watched scope or add ignores.'
            : '';
        const logMsg = `[useStatusActions::handleActionError] ${label} failed: ${msg}${extra}`;

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
        notify(userMsg, 'error');
        onEvent?.(logMsg, 'error');
        if (code === UI_TUNING.planTooLargeCode) {
          handlePlanTooLarge(
            'Too many files to back up in one go. Add ignore patterns (node_modules, build, cache) or narrow watched folders, then try again.',
          );
        }
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        const fullMsg = `[useStatusActions::handleActionError] Failed to surface error: ${reason}`;
        notify(fullMsg, 'error');
      }
    },
    [handlePlanTooLarge, notify, onEvent],
  );

  /**
   * Summary: Trigger a verification run and report results.
   *
   * Inputs: None.
   * Outputs: None.
   * Side effects: Invokes verification IPC and updates state.
   * Error handling: Emits a toast and external event log on failure.
   * Ties to other methods: Used by the `VerifyBlock`.
   * Why this exists: Provide an on-demand integrity check.
   */
  const onVerify = useCallback(async () => {
    if (!tauriAvailable()) {
      setToast({
        msg: '[useStatusActions::onVerify] IPC unavailable. Launch the app build to verify.',
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
      const msg = `[useStatusActions::onVerify] Verify failed: ${errorMessage(e)}`;
      setVerifyMsg(msg);
      setToast({ msg, kind: 'error' });
      onEvent?.(msg, 'error');
    } finally {
      setVerifying(false);
    }
  }, [onEvent, refresh]);

  /**
   * Summary: Run a status-card action with confirm, feedback, and post-refresh.
   *
   * Inputs: Action function, label, and optional confirm prompt.
   * Outputs: None.
   * Side effects: Invokes IPC services, updates toast/messages, and triggers refresh.
   * Error handling: Routes errors through `handleActionError` for actionable messaging.
   * Ties to other methods: Used by the online action button strip and offline notice buttons.
   * Why this exists: Keep action UX consistent across the status card.
   */
  const runAction = useCallback(
    async (fn: () => Promise<unknown>, label: string, opts?: { confirm?: string }) => {
      if (!tauriAvailable()) {
        setToast({
          msg: '[useStatusActions::runAction] IPC unavailable. Launch the app build to run actions.',
          kind: 'error',
        });
        return;
      }
      if (opts?.confirm && !window.confirm(opts.confirm)) {
        return;
      }
      try {
        setActionBusy(true);
        setActionMsg(`Working on ${label}...`);
        const res = await fn();
        handleActionSuccess(label, res);
        if (label.toLowerCase().includes('login') || label.toLowerCase().includes('daemon')) {
          try {
            const s = await checkService();
            setServiceStatus(s);
          } catch (e) {
            console.warn(
              `[useStatusActions::runAction] Failed to refresh service status: ${errorMessage(e)}`,
            );
          }
        }
        refresh();
      } catch (e) {
        handleActionError(label, e);
      } finally {
        setActionBusy(false);
      }
    },
    [handleActionError, handleActionSuccess, refresh, setServiceStatus],
  );

  const offlineActions = useMemo(() => {
    return {
      onInstallService: () => runAction(installService, 'Start on login'),
      onExportLogs: () =>
        runAction(exportLogs, 'Export logs', { confirm: 'Export logs to your Desktop?' }),
    };
  }, [runAction]);

  return {
    verifying,
    actionBusy,
    verifyMsg,
    actionMsg,
    toast,
    notify,
    planModal,
    setPlanModal,
    resumeOnSpace,
    toggleResumeOnSpace,
    onVerify,
    runAction,
    offlineActions,
  };
}
