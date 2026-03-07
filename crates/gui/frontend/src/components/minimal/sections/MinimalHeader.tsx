import { Config } from '../../../domain/config';
import { Button } from '../../ui/Button';
import { DEFAULT_AUTOMATIC_INTERVAL_MINUTES } from '../helpers/interval';

type Props = {
  cfg: Config;
  busy: boolean;
  runningBusy: boolean;
  showLog: boolean;
  onRunningChange: (running: boolean) => void;
  onToggleLog: () => void;
};

/**
 * Summary: Render the minimal header, including running toggle and fixed automatic schedule summary.
 *
 * Inputs: Current config, busy flags, and header action handlers.
 *
 * Outputs: Header React element tree.
 *
 * Side effects: Calls provided handlers on user interaction.
 *
 * Error handling: Delegated to parent via handlers.
 *
 * Ties to other methods: Used by `MinimalMain` for header layout consistency.
 *
 * Why this exists: Keep the primary experience Mac-like by removing schedule tuning from the main UI.
 */
export function MinimalHeader({
  cfg,
  busy,
  runningBusy,
  showLog,
  onRunningChange,
  onToggleLog,
}: Props) {
  const cadenceLabel = `Every ${DEFAULT_AUTOMATIC_INTERVAL_MINUTES} min`;

  return (
    <div className="hero hero-row">
      <div className="hero-left">
        <h1>Local Backup Manager</h1>
        <div className="hero-controls">
          <div className="control-strip" aria-label="Backup controls">
            <label className="toggle toggle-stack minimal-running-toggle">
              <input
                type="checkbox"
                checked={!cfg.safe_mode}
                disabled={busy || runningBusy}
                aria-label="Running"
                onChange={(e) => onRunningChange(e.target.checked)}
              />
              <span className="toggle-track" aria-hidden="true">
                <span className="toggle-thumb" />
              </span>
              <span className="toggle-status">{cfg.safe_mode ? 'Paused' : 'Running'}</span>
            </label>
            <div
              className="pill schedule-auto-pill"
              title={`Automatic backups, ${cadenceLabel.toLowerCase()}`}
              aria-label="Automatic backups schedule"
            >
              <span className="schedule-auto-title">Automatic backups</span>
              <span className="schedule-auto-meta">{cadenceLabel}</span>
            </div>
          </div>
        </div>
      </div>
      <div className="hero-actions">
        <div className="action-strip action-strip--compact" aria-label="Header actions">
          <Button
            tone="secondary"
            size="sm"
            onClick={onToggleLog}
            disabled={busy}
            title={showLog ? 'Hide recent activity log' : 'Open recent activity log'}
            aria-label={showLog ? 'Hide activity log' : 'Open activity log'}
          >
            {showLog ? 'Hide activity' : 'Activity'}
          </Button>
        </div>
      </div>
    </div>
  );
}
