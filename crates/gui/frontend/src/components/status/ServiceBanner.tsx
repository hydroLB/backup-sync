import React from 'react';
import { Button } from '../ui/Button';

type Props = {
  message: string;
  reachable: boolean;
  fixCommand?: string;
  onFix: () => void;
  uptimeSecs?: number | null;
  lastIpcTs?: number | null;
};

const SECONDS_PER_HOUR = 3600;

/**
 * Purpose: Render a service status banner with remediation actions.
 *
 * Inputs: Service status fields, optional fix command, and fix handler.
 * Outputs: A React element tree with status and action controls.
 * Ties to: Service status polling and repair flows.
 * Side effects: Registers UI event handlers for remediation actions.
 * Why: Surfaces service health and recovery steps in one place.
 */
const ServiceBanner: React.FC<Props> = ({
  message,
  reachable,
  fixCommand,
  onFix,
  uptimeSecs,
  lastIpcTs,
}) => {
  /**
   * Purpose: Safely invoke the fix action.
   *
   * Inputs: None.
   * Outputs: Invokes `onFix` to trigger remediation.
   * Ties to: Status card actions for service recovery.
   * Side effects: Invokes the provided fix callback.
   * Why: Adds error context around recovery actions.
   */
  const handleFix = () => {
    try {
      onFix();
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      throw new Error(`[ServiceBanner.handleFix] Failed to trigger fix action: ${reason}`);
    }
  };

  try {
    return (
      <div className={`service-banner ${reachable ? 'ok' : 'warn'}`}>
        <div>{message}</div>
        <div className="muted service-meta">
          {uptimeSecs != null && uptimeSecs > 0 && (
            <span>Uptime: {Math.floor(uptimeSecs / SECONDS_PER_HOUR)}h</span>
          )}
          {lastIpcTs ? (
            <span>Last contact: {new Date(lastIpcTs * 1000).toLocaleTimeString()}</span>
          ) : null}
        </div>
        {!reachable && (
          <div className="inline-actions">
            <Button tone="secondary" onClick={handleFix}>
              Fix
            </Button>
            {fixCommand && <span className="muted">Try: {fixCommand}</span>}
          </div>
        )}
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[ServiceBanner] Failed to render service banner: ${reason}`);
  }
};

export default ServiceBanner;
