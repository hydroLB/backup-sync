import React from 'react';
import { Button } from '../ui/Button';
import { ModalShell } from '../ui/ModalShell';

type Props = {
  message: string | null;
  onClose: () => void;
};

/**
 * Purpose: Render a modal explaining when the backup plan is too large.
 *
 * Inputs: `message` as the warning detail and `onClose` handler.
 * Outputs: A modal element or `null` when no message is present.
 * Ties to: Plan sizing errors from the backup planner.
 * Side effects: Registers UI event handlers for modal dismissal.
 * Why: Gives operators clear remediation steps when plans exceed limits.
 */
const PlanModal: React.FC<Props> = ({ message, onClose }) => {
  /**
   * Purpose: Safely close the modal when the user acknowledges the warning.
   *
   * Inputs: None.
   * Outputs: Invokes `onClose` to clear the warning state.
   * Ties to: Status card state that controls the modal visibility.
   * Side effects: Invokes the provided close handler.
   * Why: Ensures close actions are traced with error context.
   */
  const handleClose = () => {
    try {
      onClose();
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      throw new Error(`[PlanModal.handleClose] Failed to close plan modal: ${reason}`);
    }
  };

  try {
    if (!message) {
      return null;
    }
    return (
      <ModalShell
        open={!!message}
        title="Too many files to back up"
        onClose={handleClose}
        description={message}
        footer={
          <Button onClick={handleClose} autoFocus>
            Got it
          </Button>
        }
      >
        <ul className="muted">
          <li>Add ignores: node_modules, build, target, *.log, Cache</li>
          <li>Limit watched folders to what you need</li>
          <li>Run &quot;Simulate backup&quot; to see the scope before retrying</li>
        </ul>
      </ModalShell>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[PlanModal] Failed to render plan modal: ${reason}`);
  }
};

export default PlanModal;
