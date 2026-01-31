import React from "react";

type Props = {
  value: boolean;
  onChange: (v: boolean) => void;
};

/**
 * Purpose: Render the safe mode toggle control.
 *
 * Inputs: Current safe mode value and change handler.
 * Outputs: A toggle control element.
 * Ties to: Settings performance section.
 * Side effects: Registers UI event handlers for safe mode toggles.
 * Why: Allow disabling writes while keeping scan and verify.
 */
const SafeModeToggle: React.FC<Props> = ({ value, onChange }) => {
  try {
    return (
      <label style={{ flexDirection: "row", alignItems: "center", gap: 8 }}>
        <input
          type="checkbox"
          checked={!!value}
          onChange={(e) => onChange(e.target.checked)}
        />
        <span title="Backups won't write; only scan and verify. Good for debugging.">Safe mode (scan/verify only; no writes)</span>
      </label>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[SafeModeToggle] Failed to render safe mode toggle: ${reason}`);
  }
};

export default SafeModeToggle;
