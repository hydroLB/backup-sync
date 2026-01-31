import React from "react";

type Props = {
  backup_root: string;
  onChange: (value: string) => void;
  onBrowse: () => void;
  onUseDownloads: () => void;
  onUseDocuments: () => void;
  onUseDesktop: () => void;
};

/**
 * Purpose: Render the backup destination picker controls.
 *
 * Inputs: Current destination value and action handlers.
 * Outputs: A destination picker form field.
 * Ties to: Settings destination selection.
 * Side effects: Registers UI event handlers for destination changes.
 * Why: Make destination selection quick and accessible.
 */
const DestinationPicker: React.FC<Props> = ({ backup_root, onChange, onBrowse, onUseDownloads, onUseDocuments, onUseDesktop }) => {
  try {
    return (
      <label>
        Backup destination
        <div className="inline-actions" style={{ flexWrap: "wrap" }}>
          <input
            type="text"
            value={backup_root}
            onChange={(e) => onChange(e.target.value)}
            style={{ flex: 1, minWidth: 220 }}
          />
          <button className="btn secondary" onClick={onBrowse}>Browse…</button>
          <button className="btn secondary" onClick={onUseDownloads}>Use Downloads</button>
          <button className="btn secondary" onClick={onUseDocuments}>Use Documents</button>
          <button className="btn secondary" onClick={onUseDesktop}>Use Desktop</button>
        </div>
      </label>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[DestinationPicker] Failed to render destination picker: ${reason}`);
  }
};

export default DestinationPicker;
