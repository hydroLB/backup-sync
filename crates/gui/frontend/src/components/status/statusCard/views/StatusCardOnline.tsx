import { formatBytes, formatDateTime } from '../../../../utils/format';
import { UI_TUNING } from '../../../../config/uiTuning';
import { StatusDto, ServiceStatusDto } from '../../../../services/types';
import {
  checkUpdates,
  doctorReport,
  exportHealthReport,
  exportLogs,
  installService,
  restartDaemon,
} from '../../../../services';
import { exportDiagnosticBundle } from '../../../../services/system';
import ServiceBanner from '../../ServiceBanner';
import HealthRow from '../../HealthRow';
import LowSpaceGuard from '../../LowSpaceGuard';
import VerifyBlock from '../../VerifyBlock';
import ActivityFeed from '../../ActivityFeed';
import AdvancedPane from '../../AdvancedPane';
import PlanModal from '../../PlanModal';
import { InlineAlert } from '../../../ui/InlineAlert';
import { ToastMessage } from '../../../ui/ToastMessage';
import { Button } from '../../../ui/Button';

type Toast = { msg: string; kind: 'ok' | 'error' } | null;

type Props = {
  status: StatusDto;
  error: string | null;
  serviceStatus: ServiceStatusDto | null;
  logTail: string;
  verifying: boolean;
  actionBusy: boolean;
  verifyMsg: string;
  onVerify: () => Promise<void>;
  resumeOnSpace: boolean;
  onToggleResume: (next: boolean) => void;
  toast: Toast;
  planModal: string | null;
  onClosePlanModal: () => void;
  actionMsg: string;
  runAction: (
    fn: () => Promise<unknown>,
    label: string,
    opts?: { confirm?: string },
  ) => Promise<void>;
};

/**
 * Summary: Render the online status view (daemon reachable) with metrics and actions.
 *
 * Inputs: Status payload, service/log state, action state, and action handlers.
 * Outputs: Status card element tree.
 * Side effects: Calls provided handlers on user interaction.
 * Error handling: Delegated to handlers; displays current status error string when present.
 * Ties to other methods: Used by `StatusCard` when status is available.
 * Why this exists: Keep the main status layout modular and easier to iterate on.
 */
export function StatusCardOnline({
  status,
  error,
  serviceStatus,
  logTail,
  verifying,
  actionBusy,
  verifyMsg,
  onVerify,
  resumeOnSpace,
  onToggleResume,
  toast,
  planModal,
  onClosePlanModal,
  actionMsg,
  runAction,
}: Props) {
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

      {!!status.destination_paused && (
        <div className="service-banner warn">
          <div>
            {status.destination_pause_reason ??
              'Destination unavailable. Writes are paused and will auto-resume when the destination returns.'}
          </div>
        </div>
      )}

      {(status.replication_last_pairs_failed ?? 0) > 0 && (
        <div className="service-banner warn">
          <div>
            {status.replication_last_error ??
              'Replication degraded. Mirror destination may be offline; backups continue to primary.'}
          </div>
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
        onToggleResume={onToggleResume}
      />

      <div className="divider" />
      <div className="muted">Destinations</div>
      <div className="dest-list">
        {(status.destinations || []).length === 0 && <div className="muted">None configured</div>}
        {(status.destinations || []).map((d) => (
          <div key={d.id} className="dest-item">
            <div className="dest-head">
              <div className="dest-label">{d.label || d.id}</div>
              <div className="pill">
                {d.reachable === false ? 'Offline' : formatBytes(d.free_bytes)}
              </div>
            </div>
            <div className="dest-path">{d.path}</div>
            {d.reachable === false && (
              <div className="muted">{d.message ?? 'Destination unavailable.'}</div>
            )}
          </div>
        ))}
      </div>

      {status.min_free_space_bytes != null &&
        status.free_bytes != null &&
        status.free_bytes < status.min_free_space_bytes && (
          <div
            className="pill pill-danger mt-2"
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
      <div className="inline-actions">
        <Button
          tone="secondary"
          onClick={() =>
            runAction(installService, 'Start on login', {
              confirm: 'Enable the background helper to start on login?',
            })
          }
          disabled={actionBusy}
        >
          Start on login
        </Button>
        <Button
          tone="secondary"
          onClick={() =>
            runAction(exportLogs, 'Export logs', {
              confirm: 'Export logs to your Desktop?',
            })
          }
          disabled={actionBusy}
        >
          Export logs
        </Button>
        <Button
          tone="secondary"
          onClick={() => runAction(checkUpdates, 'Check updates')}
          disabled={actionBusy}
        >
          Check updates
        </Button>
        <Button
          tone="secondary"
          onClick={() =>
            runAction(exportHealthReport, 'Export health report', {
              confirm: 'Export a health report to your Desktop?',
            })
          }
          disabled={actionBusy}
        >
          Export health
        </Button>
        <Button
          tone="secondary"
          onClick={() =>
            runAction(doctorReport, 'Doctor report', {
              confirm: 'Run and save a doctor report to your Desktop?',
            })
          }
          disabled={actionBusy}
        >
          Doctor report
        </Button>
        <Button
          tone="secondary"
          onClick={() =>
            runAction(exportDiagnosticBundle, 'Diagnostic bundle', {
              confirm: 'Create a diagnostic bundle on your Desktop?',
            })
          }
          disabled={actionBusy}
        >
          Diagnostic bundle
        </Button>
        <Button
          tone="secondary"
          onClick={() =>
            runAction(restartDaemon, 'Restart daemon', {
              confirm: 'Restart the background daemon now?',
            })
          }
          disabled={actionBusy}
        >
          Restart daemon
        </Button>
        <span className="muted">{actionMsg}</span>
      </div>

      <div className="divider" />
      <ActivityFeed items={status.recent_activity || []} />
      <AdvancedPane status={status} logTail={logTail} />

      {error && <InlineAlert kind="error">Status error: {error}</InlineAlert>}
      <ToastMessage toast={toast} />
      <PlanModal message={planModal} onClose={onClosePlanModal} />
    </div>
  );
}
