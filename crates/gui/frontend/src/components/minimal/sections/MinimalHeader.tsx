type Props = {
  liveSafeMode: boolean | null;
  busy: boolean;
  runningBusy: boolean;
  onRunningChange: (running: boolean) => void;
  desktopUrl?: string;
};

/** Keep the only global control obvious: Backup Sync is either on or off. */
export function MinimalHeader({
  liveSafeMode,
  busy,
  runningBusy,
  onRunningChange,
  desktopUrl,
}: Props) {
  const isOn = liveSafeMode === false;
  const status = liveSafeMode === null ? 'Unavailable' : isOn ? 'On' : 'Off';

  return (
    <header className={`compact-header${desktopUrl ? ' compact-header--browser' : ''}`}>
      <div className="compact-brand">
        <span className="compact-brand__mark" aria-hidden="true">
          <span />
        </span>
        <div>
          <h1>Backup Sync</h1>
          <p>
            {desktopUrl ? 'Browser edition · sample workspace' : 'Automatic, versioned protection'}
          </p>
          {desktopUrl && (
            <p>
              Browser files and vaults stay here. Use the desktop app for background folder backups.
            </p>
          )}
        </div>
      </div>
      <div className="compact-header__actions">
        {desktopUrl && (
          <a
            className="website-download-button"
            href={desktopUrl}
            target="_blank"
            rel="noreferrer"
            aria-label="View Backup Sync desktop setup instructions"
          >
            <span aria-hidden="true">↗</span>
            Desktop setup
          </a>
        )}
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
      </div>
    </header>
  );
}
