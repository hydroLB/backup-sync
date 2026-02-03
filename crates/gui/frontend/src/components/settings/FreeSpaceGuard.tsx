import React from 'react';
import { formatBytes } from '../../utils/format';

type Props = {
  minFree: number | null | undefined;
  resumeOnSpace: boolean;
  onToggleResume: (v: boolean) => void;
  message?: string;
};

/**
 * Purpose: Render the free space guard details and toggle.
 *
 * Inputs: Minimum free space, resume flag, toggle handler, and message.
 * Outputs: A guard status block or null when disabled.
 * Ties to: Settings performance section and free space controls.
 * Side effects: Registers UI event handlers for resume toggles.
 * Why: Provide transparency and control over free space pausing.
 */
const FreeSpaceGuard: React.FC<Props> = ({ minFree, resumeOnSpace, onToggleResume, message }) => {
  try {
    if (!minFree || minFree <= 0) return null;
    return (
      <>
        <div className="muted" style={{ marginTop: 4 }}>
          Free-space guard is active.{' '}
          {resumeOnSpace
            ? 'Will resume when space recovers above the threshold.'
            : 'You can pause until space recovers.'}
        </div>
        <label style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
          <input
            type="checkbox"
            checked={resumeOnSpace}
            onChange={(e) => onToggleResume(e.target.checked)}
          />
          <span title="Automatically resume when free space is above the minimum.">
            Resume automatically when free space is above the minimum
          </span>
        </label>
        <div className="muted" style={{ marginTop: 2 }}>
          Threshold: {formatBytes(minFree)} ({minFree} bytes)
        </div>
        {message && <div className="muted">{message}</div>}
      </>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[FreeSpaceGuard] Failed to render free space guard: ${reason}`);
  }
};

export default FreeSpaceGuard;
