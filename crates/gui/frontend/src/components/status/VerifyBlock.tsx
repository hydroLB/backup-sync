import React from 'react';
import { UI_TUNING } from '../../config/uiTuning';
import { formatDateTime } from '../../utils/format';
import { Button } from '../ui/Button';

type Props = {
  last_verify_ts: number | null;
  last_verify_status: string | null;
  last_verify_issues: number | null;
  onVerify: () => void;
  verifying: boolean;
  verifyMsg: string;
};

const VERIFY_STATUS_EMPTY = UI_TUNING.verifyEmptyStatus;

/**
 * Purpose: Render the backup verification summary and trigger button.
 *
 * Inputs: Last verify data, handler callback, and current UI state.
 * Outputs: A React element with verification metrics and action button.
 * Ties to: Status polling and the verification action in the parent.
 * Side effects: Registers UI event handlers for verification actions.
 * Why: Keeps verification results and actions in a single place.
 */
const VerifyBlock: React.FC<Props> = ({
  last_verify_ts,
  last_verify_status,
  last_verify_issues,
  onVerify,
  verifying,
  verifyMsg,
}) => {
  /**
   * Purpose: Wrap the verify handler with error context.
   *
   * Inputs: None.
   * Outputs: Invokes `onVerify` to start verification.
   * Ties to: `onVerify` supplied by the status card.
   * Side effects: Invokes the provided verification callback.
   * Why: Ensures handler errors are surfaced with component context.
   */
  const handleVerify = () => {
    try {
      onVerify();
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      throw new Error(`[VerifyBlock.handleVerify] Failed to trigger verification: ${reason}`);
    }
  };

  try {
    return (
      <div className="metric-row">
        <div>
          <div className="muted">Last verify</div>
          <div>{formatDateTime(last_verify_ts)}</div>
          <div className="muted">{last_verify_status ?? VERIFY_STATUS_EMPTY}</div>
        </div>
        <div>
          <div className="muted">Issues found</div>
          <div>{last_verify_issues ?? 0}</div>
        </div>
        <div>
          <Button
            onClick={handleVerify}
            disabled={verifying}
            loading={verifying}
            loadingLabel="Verifying..."
          >
            Verify backups
          </Button>
          <div className="muted">{verifyMsg}</div>
        </div>
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[VerifyBlock] Failed to render verify block: ${reason}`);
  }
};

export default VerifyBlock;
