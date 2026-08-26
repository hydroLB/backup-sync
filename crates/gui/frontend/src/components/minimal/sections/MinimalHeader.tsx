type Props = {
  liveSafeMode: boolean | null;
  busy: boolean;
  runningBusy: boolean;
  onRunningChange: (running: boolean) => void;
};

/** Keep the only global control obvious: Backup Sync is either on or off. */
export function MinimalHeader({ liveSafeMode, busy, runningBusy, onRunningChange }: Props) {
  const isOn = liveSafeMode === false;
  const status = liveSafeMode === null ? 'Unavailable' : isOn ? 'On' : 'Off';

  return (
    <header className="compact-header">
      <div className="compact-brand">
        <span className="compact-brand__mark" aria-hidden="true">
          <span />
        </span>
        <div>
          <h1>Backup Sync</h1>
          <p>Automatic, versioned protection</p>
        </div>
      </div>
      <div className="compact-power">
        <div className="compact-power__copy">
          <span className={`compact-power__dot ${isOn ? 'is-on' : ''}`} aria-hidden="true" />
          <span>Backup Sync</span>
          <strong>{status}</strong>
        </div>
        <label className="toggle minimal-running-toggle">
          <span className="sr-only">Turn Backup Sync on or off</span>
          <input
            type="checkbox"
            checked={isOn}
            disabled={busy || runningBusy || liveSafeMode === null}
            aria-label="Running"
            onChange={(event) => onRunningChange(event.target.checked)}
          />
          <span className="toggle-track" aria-hidden="true">
            <span className="toggle-thumb" />
          </span>
        </label>
      </div>
    </header>
  );
}
