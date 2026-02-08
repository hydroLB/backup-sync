import React, { useMemo } from 'react';
import { UI_TUNING } from '../../config/uiTuning';
import { formatBytes, formatDateTime } from '../../utils/format';
import { StatusDto } from '../../services/types';

type Props = {
  status: StatusDto | null;
  logTail?: string;
};

const JSON_INDENT = UI_TUNING.advancedJsonIndent;

/**
 * Purpose: Render advanced debug details for status and log output.
 *
 * Inputs: `status` and optional `logTail` for diagnostics.
 * Outputs: A React element tree with debug metadata or `null`.
 * Ties to: Status polling and log tail fetches from the backend.
 * Side effects: Registers React memoization hooks for JSON formatting.
 * Why: Provides deep diagnostics for troubleshooting and support.
 */
const AdvancedPane: React.FC<Props> = ({ status, logTail }) => {
  const json = useMemo(() => {
    try {
      return JSON.stringify(status, null, JSON_INDENT);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      return `[AdvancedPane] Failed to serialize status: ${reason}`;
    }
  }, [status]);

  try {
    if (!status) {
      return null;
    }
    return (
      <div className="advanced-pane">
        <div className="section-title advanced-head">
          <h4 className="heading-compact">Advanced</h4>
          <span className="pill">Debug</span>
        </div>
        <div className="muted advanced-note">
          IPC healthy. Free: {formatBytes(status.free_bytes)} • Last run:{' '}
          {formatDateTime(status.last_run_ts)}
        </div>
        <pre className="advanced-pane__code">{json}</pre>
        {logTail && (
          <>
            <div className="muted advanced-log-label">Log tail (latest entries)</div>
            <pre className="advanced-pane__code">{logTail}</pre>
          </>
        )}
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[AdvancedPane] Failed to render advanced pane: ${reason}`);
  }
};

export default AdvancedPane;
