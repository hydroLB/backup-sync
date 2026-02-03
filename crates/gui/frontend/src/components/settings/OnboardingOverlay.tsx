import React from 'react';
import { formatBytes } from '../../utils/format';

type Step = 1 | 2 | 3 | 4;

type DestinationStatus = { writable: boolean; free_bytes: number | null; message: string } | null;

type Props = {
  step: Step;
  hasWatched: boolean;
  destStatus: DestinationStatus;
  canFinish: boolean;
  retention: number;
  watchedPaths: string[];
  summary: string;
  freeMessage?: string;
  safeMode?: boolean;
  statusMessage?: string;
  startOnLoginMsg?: string;
  onChangeRetention: (value: number) => void;
  onAddFolder: () => void;
  onAddFile: () => void;
  onQuickAddDesktop: () => void;
  onQuickAddDocuments: () => void;
  onQuickAddDownloads: () => void;
  onPickDestination: () => void;
  onUseDesktopDest: () => void;
  onUseDocumentsDest: () => void;
  onUseDownloadsDest: () => void;
  onStartOnLogin: () => void;
  onTestBackup: () => void;
  onNext: () => void;
  onBack: () => void;
  onFinish: () => void;
};

/**
 * Purpose: Render the onboarding overlay with step guidance.
 *
 * Inputs: Step data, handlers, and status values.
 * Outputs: An onboarding overlay element.
 * Ties to: Settings onboarding flow.
 * Side effects: Registers UI event handlers for onboarding actions.
 * Why: Guide first time users through essential setup.
 */
const OnboardingOverlay: React.FC<Props> = ({
  step,
  hasWatched,
  destStatus,
  canFinish,
  summary,
  freeMessage,
  safeMode,
  statusMessage,
  startOnLoginMsg,
  onAddFolder,
  onAddFile,
  onQuickAddDesktop,
  onQuickAddDocuments,
  onQuickAddDownloads,
  onPickDestination,
  onUseDesktopDest,
  onUseDocumentsDest,
  onUseDownloadsDest,
  onStartOnLogin,
  onTestBackup,
  onNext,
  onBack,
  onFinish,
  retention,
  onChangeRetention,
  watchedPaths,
}) => {
  const stepMeta = [
    { id: 1 as Step, label: 'What to protect', done: hasWatched },
    { id: 2 as Step, label: 'Where to store', done: destStatus?.writable ?? false },
    { id: 3 as Step, label: 'Versions to keep', done: false },
    { id: 4 as Step, label: 'Run in background', done: false },
  ];

  /**
   * Purpose: Determine if the Next button should be disabled.
   *
   * Inputs: None.
   * Outputs: Boolean flag for button state.
   * Ties to: Onboarding navigation controls.
   * Side effects: None.
   * Why: Prevent progression until required steps are complete.
   */
  const nextDisabled = () => {
    try {
      if (step === 1) return !hasWatched;
      if (step === 2) return !(destStatus?.writable ?? false);
      if (step === 3) return !(hasWatched && (destStatus?.writable ?? false));
      return false;
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      console.warn(`[OnboardingOverlay::nextDisabled] Failed to evaluate next state: ${reason}`);
      return true;
    }
  };

  /**
   * Purpose: Render the body for the active onboarding step.
   *
   * Inputs: None.
   * Outputs: A step-specific content block.
   * Ties to: Step navigation and action handlers.
   * Side effects: None.
   * Why: Keep step content encapsulated and readable.
   */
  const renderBody = () => {
    try {
      if (step === 1) {
        return (
          <>
            <p className="muted">
              Add at least one folder or file. We’ll keep you here until something is selected.
            </p>
            <div className="inline-actions" style={{ flexWrap: 'wrap' }}>
              <button className="btn" onClick={onAddFolder}>
                Add folder
              </button>
              <button className="btn secondary" onClick={onAddFile}>
                Add file
              </button>
            </div>
            <div className="inline-actions" style={{ flexWrap: 'wrap' }}>
              <button className="btn secondary" onClick={onQuickAddDesktop}>
                Quick add Desktop
              </button>
              <button className="btn secondary" onClick={onQuickAddDocuments}>
                Quick add Documents
              </button>
              <button className="btn secondary" onClick={onQuickAddDownloads}>
                Quick add Downloads
              </button>
            </div>
            {watchedPaths.length > 0 && (
              <div className="watched-list">
                <div className="muted">Currently protected:</div>
                <ul className="watched-ul">
                  {watchedPaths.slice(0, 5).map((p) => (
                    <li key={p}>{p}</li>
                  ))}
                </ul>
                {watchedPaths.length > 5 && (
                  <div className="muted">+{watchedPaths.length - 5} more</div>
                )}
              </div>
            )}
          </>
        );
      }
      if (step === 2) {
        return (
          <>
            <p className="muted">
              Pick a backup destination. We’ll create the folder and block finishing until it’s
              writable.
            </p>
            <div className="inline-actions" style={{ flexWrap: 'wrap' }}>
              <button className="btn" onClick={onPickDestination}>
                Choose destination
              </button>
              <button className="btn secondary" onClick={onUseDesktopDest}>
                Use Desktop
              </button>
              <button className="btn secondary" onClick={onUseDocumentsDest}>
                Use Documents
              </button>
              <button className="btn secondary" onClick={onUseDownloadsDest}>
                Use Downloads
              </button>
            </div>
            <div className="muted">
              {destStatus ? (
                <>
                  {destStatus.message}
                  {destStatus.free_bytes != null &&
                    ` • Free: ${formatBytes(destStatus.free_bytes)}`}
                </>
              ) : (
                'Waiting for destination check…'
              )}
            </div>
          </>
        );
      }
      if (step === 3) {
        return (
          <>
            <p className="muted">How many old versions do you want to keep per file?</p>
            <div className="slider-wrap" style={{ display: 'grid', gap: 8 }}>
              <div className="slider-track">
                <input
                  type="range"
                  min={1}
                  max={10}
                  step={1}
                  value={retention}
                  onChange={(e) => onChangeRetention(Number(e.target.value))}
                />
                <div className="slider-dots">
                  {Array.from({ length: 10 }).map((_, idx) => {
                    const val = idx + 1;
                    const active = val <= retention;
                    return (
                      <div key={val} className="slider-dot-wrap">
                        <div className={`slider-dot ${active ? 'active' : ''}`} />
                        <div className="slider-num">{val}</div>
                      </div>
                    );
                  })}
                </div>
              </div>
              <div className="muted">
                Keeping <strong>{retention}</strong> old revisions.
              </div>
            </div>
            <div className="muted">You can adjust this anytime in settings.</div>
          </>
        );
      }
      return (
        <>
          <p className="muted">
            Enable background mode so backups run even when the window is closed.
          </p>
          <div className="inline-actions" style={{ flexWrap: 'wrap' }}>
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
            <div style={{ color: 'var(--danger)' }}>
              Add a watched item and destination before finishing.
            </div>
          )}
        </>
      );
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      return (
        <div className="muted">[OnboardingOverlay::renderBody] Failed to render: {reason}</div>
      );
    }
  };

  return (
    <div className="onboarding-overlay">
      <div className="onboarding-card">
        <div className="section-title">
          <div>
            <div className="pill step-pill">First-run guide</div>
            <h3 style={{ margin: '6px 0 2px' }}>Let’s finish setup</h3>
            <p className="muted" style={{ margin: 0 }}>
              We’ll block the app until the essentials are valid.
            </p>
          </div>
          <div className="pill step-pill">{step} / 4</div>
        </div>
        <div className="onboarding-steps">
          {stepMeta.map((s) => (
            <div
              key={s.id}
              className={`onboarding-step ${step === s.id ? 'active' : ''} ${s.done ? 'done' : ''}`}
            >
              <span>{s.label}</span>
              <span className="badge">{s.done ? 'Done' : s.id === step ? 'Now' : 'Next'}</span>
            </div>
          ))}
        </div>
        <div className="onboarding-body">{renderBody()}</div>
        <div className="onboarding-actions">
          <div className="muted">{statusMessage}</div>
          <div className="onboarding-nav">
            <button className="btn secondary" onClick={onBack} disabled={step === 1}>
              Back
            </button>
            {step < 4 ? (
              <button className="btn" onClick={onNext} disabled={nextDisabled()}>
                Next
              </button>
            ) : (
              <button className="btn" onClick={onFinish} disabled={!canFinish}>
                Finish
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};

export default OnboardingOverlay;
