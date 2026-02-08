import React from 'react';
import { UI_TUNING } from '../../config/uiTuning';
import { InlineAlert } from '../ui/InlineAlert';
import { Button } from '../ui/Button';

type Props = {
  error: string | null;
  onInstallService?: () => Promise<void> | void;
  onExportLogs?: () => Promise<void> | void;
  message?: string;
};

const DEFAULT_OFFLINE_MESSAGE = UI_TUNING.offlineNoticeMessage;

/**
 * Purpose: Render an offline notice when the daemon is unreachable.
 *
 * Inputs: `error`, optional action handlers, and an optional message.
 * Outputs: A React element tree with offline status and actions.
 * Ties to: Status polling logic and service recovery actions.
 * Side effects: Registers UI event handlers for recovery actions.
 * Why: Provides clear recovery actions when the daemon is offline.
 */
const OfflineNotice: React.FC<Props> = ({ error, onInstallService, onExportLogs, message }) => {
  const [installing, setInstalling] = React.useState(false);
  const [exporting, setExporting] = React.useState(false);

  /**
   * Purpose: Safely invoke the install service action.
   *
   * Inputs: None.
   * Outputs: Invokes `onInstallService` when provided.
   * Ties to: Status card actions for service installation.
   * Side effects: Invokes the provided install callback.
   * Why: Provides error context for recovery actions.
   */
  const handleInstall = async () => {
    try {
      if (!onInstallService) return;
      setInstalling(true);
      await onInstallService();
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      throw new Error(`[OfflineNotice.handleInstall] Failed to trigger install action: ${reason}`);
    } finally {
      setInstalling(false);
    }
  };

  /**
   * Purpose: Safely invoke the export logs action.
   *
   * Inputs: None.
   * Outputs: Invokes `onExportLogs` when provided.
   * Ties to: Status card actions for log export.
   * Side effects: Invokes the provided export callback.
   * Why: Provides error context for recovery actions.
   */
  const handleExportLogs = async () => {
    try {
      if (!onExportLogs) return;
      setExporting(true);
      await onExportLogs();
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      throw new Error(`[OfflineNotice.handleExportLogs] Failed to trigger log export: ${reason}`);
    } finally {
      setExporting(false);
    }
  };

  try {
    return (
      <div className="card">
        <div className="section-title">
          <h3>Live status</h3>
          <span className="pill">Not connected</span>
        </div>
        <p className="muted">{message || DEFAULT_OFFLINE_MESSAGE}</p>
        <div className="inline-actions mt-2">
          {onInstallService && (
            <Button
              onClick={() => void handleInstall()}
              loading={installing}
              loadingLabel="Starting..."
            >
              Start on login
            </Button>
          )}
          {onExportLogs && (
            <Button
              tone="secondary"
              onClick={() => void handleExportLogs()}
              loading={exporting}
              loadingLabel="Exporting..."
            >
              Export logs
            </Button>
          )}
        </div>
        {error && <InlineAlert kind="error">Error: {error}</InlineAlert>}
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[OfflineNotice] Failed to render offline notice: ${reason}`);
  }
};

export default OfflineNotice;
