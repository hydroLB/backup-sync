import React from 'react';
import { formatBytes, formatDuration } from '../../utils/format';
import { UI_TUNING } from '../../config/uiTuning';

type Props = {
  version: string | null;
  uptime_secs: number | null;
  free_bytes: number | null;
  low_space_threshold?: number;
  safe_mode?: boolean;
};

const DEFAULT_LOW_SPACE_THRESHOLD_BYTES = UI_TUNING.lowSpaceThresholdBytes;

/**
 * Purpose: Render a health summary row showing version, uptime, and free space.
 *
 * Inputs: `version`, `uptime_secs`, `free_bytes`, `safe_mode`, and threshold overrides.
 * Outputs: A React fragment with health metrics and warning badges.
 * Ties to: Status cards and storage threshold logic in the UI.
 * Side effects: None.
 * Why: Provides at-a-glance operational health cues.
 */
const HealthRow: React.FC<Props> = ({
  version,
  uptime_secs,
  free_bytes,
  safe_mode,
  low_space_threshold = DEFAULT_LOW_SPACE_THRESHOLD_BYTES,
}) => {
  try {
    return (
      <>
        <div className="metric-row">
          <div>
            <div className="muted">Version</div>
            <div>{version || 'Unknown'}</div>
          </div>
          <div>
            <div className="muted">Uptime</div>
            <div>{formatDuration(uptime_secs)}</div>
          </div>
          <div>
            <div className="muted">Free space</div>
            <div>{free_bytes != null ? formatBytes(free_bytes) : 'n/a'}</div>
          </div>
        </div>
        {safe_mode && (
          <div className="pill" style={{ borderColor: '#ffd27b', color: '#ffd27b', marginTop: 8 }}>
            Safe mode: scan/verify only (no writes)
          </div>
        )}
        {free_bytes != null && free_bytes < low_space_threshold && (
          <div className="pill" style={{ borderColor: '#ff7b7b', color: '#ffb0b0', marginTop: 8 }}>
            Low disk space at backup destination
          </div>
        )}
      </>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[HealthRow] Failed to render health row: ${reason}`);
  }
};

export default HealthRow;
