type Props = {
  canFinish: boolean;
  summary: string;
  freeMessage?: string;
  safeMode?: boolean;
  startOnLoginMsg?: string;
  onStartOnLogin: () => void;
  onFinish: () => void;
  onTestBackup: () => void;
};

/**
 * Summary: Render onboarding step 4 (enable background mode and finish setup).
 *
 * Inputs: Finish eligibility flags, summary text, status strings, and action handlers.
 * Outputs: A step body element tree.
 * Side effects: Calls provided handlers on user interaction.
 * Error handling: Delegated to the provided handlers.
 * Ties to other methods: Used by `OnboardingOverlay` body rendering.
 * Why this exists: Keep background/finish step UI isolated and consistent.
 */
export function BackgroundStep({
  canFinish,
  summary,
  freeMessage,
  safeMode,
  startOnLoginMsg,
  onStartOnLogin,
  onFinish,
  onTestBackup,
}: Props) {
  return (
    <>
      <p className="muted">Enable background mode so backups run even when the window is closed.</p>
      <div className="inline-actions inline-actions-wrap">
        <button className="btn secondary" onClick={onStartOnLogin}>
          Enable start on login
        </button>
        <button className="btn secondary" onClick={onFinish} disabled={!canFinish}>
          Finish setup
        </button>
        <button className="btn" onClick={onTestBackup} disabled={!canFinish}>
          Test backup now
        </button>
      </div>
      <div className="muted">
        {startOnLoginMsg || 'We’ll install a small helper and keep the tray icon running.'}
      </div>
      <div className="muted">Summary: {summary}</div>
      {freeMessage && <div className="muted">{freeMessage}</div>}
      <div className="muted">
        Safe mode: {safeMode ? 'On (scan/verify only)' : 'Off (normal backups)'}
      </div>
      {!canFinish && (
        <div className="text-danger">Add a watched item and destination before finishing.</div>
      )}
    </>
  );
}
