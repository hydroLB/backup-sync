import React from 'react';
import { WatchedPath } from '../types';

type Props = {
  items: WatchedPath[];
  onToggleEnabled: (path: string) => void;
  onRemove: (path: string) => void;
};

/**
 * Purpose: Render the list of watched items for a destination.
 *
 * Inputs: Watched items and action handlers.
 * Outputs: A list of destination items or an empty state.
 * Ties to: Destination card display.
 * Side effects: Registers UI event handlers for item actions.
 * Why: Show which paths map to each destination.
 */
const DestItems: React.FC<Props> = ({ items, onToggleEnabled, onRemove }) => {
  try {
    if (!items.length) return <div className="muted">Nothing protected yet.</div>;
    return (
      <div className="dest-items">
        {items.map((w) => (
          <div key={w.path} className="dest-item">
            <div>
              <strong>{w.path.split(/[/\\]/).pop()}</strong>
              <div className="muted small">{w.path}</div>
            </div>
            <div className="inline-actions">
              <button className="btn secondary" onClick={() => onToggleEnabled(w.path)}>
                {w.enabled ? 'Pause' : 'Enable'}
              </button>
              <button className="btn secondary" onClick={() => onRemove(w.path)}>
                Remove
              </button>
            </div>
          </div>
        ))}
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[DestItems] Failed to render destination items: ${reason}`);
  }
};

export default DestItems;
