import React from 'react';
import { InlineAlert } from '../ui/InlineAlert';
import { Button } from '../ui/Button';
import { StateBlock } from '../ui/StateBlock';

type Props = {
  onTest: () => Promise<void> | void;
  result: {
    destination_writable: boolean;
    destination_message: string;
    watched_ok: string[];
    watched_missing: string[];
    watched_unwritable: string[];
  } | null;
  error?: string | null;
};

/**
 * Purpose: Render access test controls and results.
 *
 * Inputs: Test handler, result payload, and error message.
 * Outputs: An access test panel with status details.
 * Ties to: Settings access test workflow.
 * Side effects: Registers UI event handlers for access tests.
 * Why: Surface path and destination access issues early.
 */
const AccessTest: React.FC<Props> = ({ onTest, result, error }) => {
  const [running, setRunning] = React.useState(false);

  /**
   * Purpose: Execute the access test and expose loading state to the user.
   *
   * Inputs: None.
   * Outputs: Triggers the external test action.
   * Ties to: Access test button interaction.
   * Side effects: Updates local loading state around the async operation.
   * Why: Prevent duplicate runs and provide clear progress feedback.
   */
  const handleTest = async () => {
    try {
      setRunning(true);
      await onTest();
    } finally {
      setRunning(false);
    }
  };

  try {
    return (
      <div className="card card-subtle">
        <div className="section-title">
          <h4 className="heading-compact">Test access</h4>
          <Button
            tone="secondary"
            onClick={() => void handleTest()}
            loading={running}
            loadingLabel="Testing..."
          >
            Run test
          </Button>
        </div>
        {result ? (
          <div className="stack-sm">
            <div className="muted">Destination: {result.destination_message}</div>
            {result.watched_missing.length > 0 && (
              <InlineAlert kind="error">Missing: {result.watched_missing.join(', ')}</InlineAlert>
            )}
            {result.watched_unwritable.length > 0 && (
              <InlineAlert kind="error">
                Unwritable: {result.watched_unwritable.join(', ')}
              </InlineAlert>
            )}
            {result.watched_ok.length > 0 && (
              <div className="muted">OK: {result.watched_ok.length} paths</div>
            )}
          </div>
        ) : (
          <StateBlock
            tone="empty"
            title="No test results yet"
            message="Run a quick check to confirm watched paths and destination access."
          />
        )}
        {error && <InlineAlert kind="error">{error}</InlineAlert>}
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[AccessTest] Failed to render access test panel: ${reason}`);
  }
};

export default AccessTest;
