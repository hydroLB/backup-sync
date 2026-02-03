import React from 'react';

type Props = {
  retention: number;
  onChange: (v: number) => void;
};

/**
 * Purpose: Render the destination retention control menu.
 *
 * Inputs: Retention value and change handler.
 * Outputs: A retention slider element.
 * Ties to: Destination card menus.
 * Side effects: Registers UI event handlers for retention updates.
 * Why: Allow per-destination retention tuning.
 */
const DestMenu: React.FC<Props> = ({ retention, onChange }) => {
  try {
    return (
      <div className="dest-menu">
        <div className="muted">Old versions to keep</div>
        <input
          type="range"
          min={1}
          max={10}
          value={retention}
          onChange={(e) => onChange(Number(e.target.value))}
        />
        <div className="muted small">Keeping {retention} versions by default.</div>
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[DestMenu] Failed to render destination menu: ${reason}`);
  }
};

export default DestMenu;
