import { Button } from '../../ui/Button';
import { DEFAULT_AUTOMATIC_INTERVAL_MINUTES } from '../helpers/interval';

type Props = {
  liveSafeMode: boolean | null;
  busy: boolean;
  runningBusy: boolean;
  showLog: boolean;
  destinationCount: number;
  watchedCount: number;
  onRunningChange: (running: boolean) => void;
  onToggleLog: () => void;
};

/** Keep the primary experience Mac-like by removing schedule tuning from the main UI. */
export function MinimalHeader({
  liveSafeMode,
  busy,
  runningBusy,
  showLog,
  destinationCount,
  watchedCount,
  onRunningChange,
  onToggleLog,
}: Props) {
  const cadenceLabel = `Every ${DEFAULT_AUTOMATIC_INTERVAL_MINUTES} min`;
  const statusTitle =
    liveSafeMode === null
      ? 'Service is offline'
      : liveSafeMode
        ? 'Protection is paused'
        : 'Protection is on';
  const statusState = liveSafeMode === null ? 'offline' : liveSafeMode ? 'paused' : 'running';

  return (
    <header className="hero-dashboard">
      <div className="hero-dashboard__copy">
        <div className="product-kicker">
          <span className="product-mark" aria-hidden="true">
            <span />
            <span />
            <span />
          </span>
          <span>Backup Sync</span>
          <span className="product-kicker__tag">Local-first</span>
        </div>
        <h1>
          Your files, protected.
          <span>Every version, recoverable.</span>
        </h1>
        <p className="hero-dashboard__lede">
          Choose what matters, keep independent copies, and roll back to a clean version whenever
          you need it.
        </p>
        <div className="hero-metrics" aria-label="Protection overview">
          <div className="hero-metric">
            <strong>{destinationCount}</strong>
            <span>storage location{destinationCount === 1 ? '' : 's'}</span>
          </div>
          <div className="hero-metric">
            <strong>{watchedCount}</strong>
            <span>protected path{watchedCount === 1 ? '' : 's'}</span>
          </div>
          <div className="hero-metric hero-metric--accent">
            <strong>{watchedCount > 0 && destinationCount > 0 ? 'Ready' : 'Set up'}</strong>
            <span>
              {watchedCount > 0 && destinationCount > 0 ? 'restore available' : 'next step below'}
            </span>
          </div>
        </div>
      </div>

      <div className={`protection-panel protection-panel--${statusState}`}>
        <div className="protection-panel__status">
          <span className="protection-orb" aria-hidden="true">
            <span />
          </span>
          <div>
            <span className="protection-panel__eyebrow">Protection status</span>
            <strong>{statusTitle}</strong>
          </div>
        </div>
        <p>
          {liveSafeMode === false
            ? `Backup Sync checks protected files ${cadenceLabel.toLowerCase()} and saves only what changed.`
            : liveSafeMode
              ? 'Automatic backups are stopped. Your existing versions stay safe and restorable.'
              : 'The background service is unavailable. Existing versions remain untouched.'}
        </p>
        <div className="protection-panel__controls" aria-label="Backup controls">
          <div className="protection-toggle-copy">
            <strong>Automatic protection</strong>
            <span>{cadenceLabel}</span>
          </div>
          <label className="toggle minimal-running-toggle">
            <span className="sr-only">Automatic protection</span>
            <input
              type="checkbox"
              checked={liveSafeMode === false}
              disabled={busy || runningBusy}
              aria-label="Running"
              onChange={(e) => onRunningChange(e.target.checked)}
            />
            <span className="toggle-track" aria-hidden="true">
              <span className="toggle-thumb" />
            </span>
          </label>
        </div>
        <Button
          tone="secondary"
          size="sm"
          block
          className="protection-panel__activity"
          onClick={onToggleLog}
          disabled={busy}
          title={showLog ? 'Hide recent activity log' : 'Open recent activity log'}
          aria-label={showLog ? 'Hide activity log' : 'Open activity log'}
        >
          {showLog ? 'Close activity' : 'View recent activity'}
        </Button>
      </div>
    </header>
  );
}
