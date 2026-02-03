import React from 'react';

type Props = {
  visible: boolean;
  passcode: string;
  unlockSeconds: number;
  onChange: (v: string) => void;
  onUnlock: () => void;
  onClose: () => void;
};

/**
 * Purpose: Render the settings unlock modal.
 *
 * Inputs: Visibility, passcode value, unlock duration, and action handlers.
 * Outputs: A modal element or null when hidden.
 * Ties to: Settings authentication flow.
 * Side effects: Registers UI event handlers for unlock actions.
 * Why: Protect settings behind an explicit unlock step.
 */
const AuthLockModal: React.FC<Props> = ({
  visible,
  passcode,
  unlockSeconds,
  onChange,
  onUnlock,
  onClose,
}) => {
  try {
    if (!visible) return null;
    const unlockMinutes = Math.max(1, Math.round(unlockSeconds / 60));
    return (
      <div className="modal">
        <div className="modal__content">
          <h4>Unlock to edit settings</h4>
          <p className="muted">
            Enter your passcode (set BACKUP_SYNC_PASSPHRASE). Session stays unlocked for about{' '}
            {unlockMinutes} minutes.
          </p>
          <input
            type="password"
            placeholder="Passcode"
            value={passcode}
            onChange={(e) => onChange(e.target.value)}
          />
          <div className="inline-actions" style={{ marginTop: 8 }}>
            <button className="btn" onClick={onUnlock}>
              Unlock
            </button>
            <button className="btn secondary" onClick={onClose}>
              Cancel
            </button>
          </div>
        </div>
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[AuthLockModal::render] Failed to render auth lock modal: ${reason}`);
  }
};

export default AuthLockModal;
