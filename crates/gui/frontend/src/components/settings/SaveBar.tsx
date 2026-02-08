import React from 'react';
import { Button } from '../ui/Button';

type Props = {
  disabled?: boolean;
  status: string;
  onSave: () => void;
};

/**
 * Purpose: Render the sticky save bar with status.
 *
 * Inputs: Disabled flag, status text, and save handler.
 * Outputs: A save bar element.
 * Ties to: Settings panel save actions.
 * Side effects: Registers UI event handlers for save actions.
 * Why: Keep save controls visible while editing settings.
 */
const SaveBar: React.FC<Props> = ({ disabled, status, onSave }) => {
  try {
    return (
      <div className="sticky-actions">
        <Button onClick={onSave} disabled={disabled}>
          Save settings
        </Button>
        <span className="muted">{status}</span>
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[SaveBar] Failed to render save bar: ${reason}`);
  }
};

export default SaveBar;
