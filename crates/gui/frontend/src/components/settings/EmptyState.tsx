import React from 'react';
import { Button } from '../ui/Button';

type Props = {
  status: string;
  onAddFolder: () => void;
  onQuickAddDesktop: () => void;
  onQuickAddDocuments: () => void;
  onQuickAddDownloads: () => void;
};

/**
 * Purpose: Render the empty state when no watch paths exist.
 *
 * Inputs: Status message and quick action handlers.
 * Outputs: An empty state panel with call to action buttons.
 * Ties to: Settings panel when watched list is empty.
 * Side effects: Registers UI event handlers for quick add actions.
 * Why: Guide users through first time setup quickly.
 */
const EmptyState: React.FC<Props> = ({
  status,
  onAddFolder,
  onQuickAddDesktop,
  onQuickAddDocuments,
  onQuickAddDownloads,
}) => {
  try {
    return (
      <div className="empty-hero">
        <h3>Protect your first folder</h3>
        <p>Tap once, we handle the rest. You can always add more later.</p>
        <Button className="big-cta" onClick={onAddFolder}>
          Protect my files
        </Button>
        <div className="divider" />
        <div className="quick-grid">
          <Button tone="secondary" onClick={onQuickAddDesktop}>
            Add Desktop
          </Button>
          <Button tone="secondary" onClick={onQuickAddDocuments}>
            Add Documents
          </Button>
          <Button tone="secondary" onClick={onQuickAddDownloads}>
            Add Downloads
          </Button>
        </div>
        {status && <div className="mt-2">{status}</div>}
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[EmptyState] Failed to render empty state: ${reason}`);
  }
};

export default EmptyState;
