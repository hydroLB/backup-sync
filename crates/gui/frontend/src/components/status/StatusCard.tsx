import React, { useEffect, useRef } from 'react';
import { ActionLogEntry } from './ActionLogFlyout';
import { StatusDto } from '../../services/types';
import { useStatusPolling } from './statusCard/hooks/useStatusPolling';
import { useStatusActions } from './statusCard/hooks/useStatusActions';
import { StatusCardOnline } from './statusCard/views/StatusCardOnline';
import { StatusCardOffline } from './statusCard/views/StatusCardOffline';

type Props = {
  onEvent?: (msg: string, kind?: ActionLogEntry['kind']) => void;
  onSafeMode?: (value: boolean) => void;
};

/**
 * Summary: Render the status card and orchestrate online/offline action flows.
 *
 * Inputs: Optional event callback and safe-mode callback.
 * Outputs: Online or offline status card element tree.
 * Side effects: Registers status polling, toast updates, and event notifications.
 * Error handling: Delegates actionable errors to status hooks and view components.
 * Ties to other methods: Composes `useStatusPolling`, `useStatusActions`, and status-card view modules.
 * Why this exists: Keep the top-level status surface thin while behavior lives in dedicated hooks.
 */
const StatusCard: React.FC<Props> = ({ onEvent, onSafeMode }) => {
  const { status, error, logTail, serviceStatus, setServiceStatus, refresh } = useStatusPolling(
    onSafeMode ? { onSafeMode } : {},
  );
  const {
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
  } = useStatusActions(
    onEvent ? { onEvent, refresh, setServiceStatus } : { refresh, setServiceStatus },
  );
  const prevStatusRef = useRef<StatusDto | null>(null);

  useEffect(() => {
    if (!status) return;
    const prev = prevStatusRef.current;
    const nextDestinationPaused = !!status.destination_paused;
    const nextReplicationFailed = (status.replication_last_pairs_failed ?? 0) > 0;

    if (prev) {
      const prevDestinationPaused = !!prev.destination_paused;
      const prevReplicationFailed = (prev.replication_last_pairs_failed ?? 0) > 0;
      if (!prevDestinationPaused && nextDestinationPaused) {
        notify(
          status.destination_pause_reason ?? 'Destination unavailable. Writes paused.',
          'error',
        );
      }
      if (prevDestinationPaused && !nextDestinationPaused) {
        notify('Destination reconnected. Resuming backups.', 'ok');
      }
      if (!prevReplicationFailed && nextReplicationFailed) {
        notify(
          status.replication_last_error ??
            'Replication degraded. Mirror destination may be offline; backups continue to primary.',
          'error',
        );
      }
      if (prevReplicationFailed && !nextReplicationFailed) {
        notify('Replication healthy again.', 'ok');
      }
    }

    prevStatusRef.current = status;
  }, [notify, status]);

  if (!status) {
    return (
      <StatusCardOffline
        error={error}
        message={error || 'Not connected yet. Start the daemon or complete setup.'}
        onInstallService={offlineActions.onInstallService}
        onExportLogs={offlineActions.onExportLogs}
      />
    );
  }

  return (
    <StatusCardOnline
      status={status}
      error={error}
      serviceStatus={serviceStatus}
      logTail={logTail}
      verifying={verifying}
      actionBusy={actionBusy}
      verifyMsg={verifyMsg}
      onVerify={onVerify}
      resumeOnSpace={resumeOnSpace}
      onToggleResume={toggleResumeOnSpace}
      toast={toast}
      planModal={planModal}
      onClosePlanModal={() => setPlanModal(null)}
      actionMsg={actionMsg}
      runAction={runAction}
    />
  );
};

export default StatusCard;
