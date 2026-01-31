import React, { useState } from "react";
import { Destination, WatchedPath } from "../types";
import DestMenu from "./DestMenu";
import DestItems from "./DestItems";

type Props = {
  dest: Destination;
  items: WatchedPath[];
  onAddPath: (destId: string, path: string, kind: "File" | "Directory") => void;
  onPickPath: (destId: string, kind: "File" | "Directory") => void;
  onToggleEnabled: (path: string) => void;
  onRemove: (path: string) => void;
  onSetRetention: (destId: string, v: number) => void;
  onSetLabel?: (destId: string, label: string) => void;
};

/**
 * Purpose: Resolve a usable path string from a dropped file.
 *
 * Inputs: Browser or Tauri file object.
 * Outputs: Path string for onAddPath handlers.
 * Ties to: Drag and drop handling in destination cards.
 * Side effects: None.
 * Why: Avoid unsafe casts while supporting native file paths.
 */
const resolveDropPath = (file: File): string => {
  const maybePath = (file as File & { path?: string }).path;
  return maybePath && maybePath.length > 0 ? maybePath : file.name;
};

/**
 * Purpose: Render a destination card with watch list and controls.
 *
 * Inputs: Destination data, watched items, and action handlers.
 * Outputs: A destination card element.
 * Ties to: Destination board and watch list management.
 * Side effects: Registers React state hooks and event handlers for destination actions.
 * Why: Provide destination-scoped controls and drop targets.
 */
const DestCard: React.FC<Props> = ({
  dest,
  items,
  onAddPath,
  onPickPath,
  onToggleEnabled,
  onRemove,
  onSetRetention,
  onSetLabel,
}) => {
  const [menuOpen, setMenuOpen] = useState(false);
  const [label, setLabel] = useState(dest.label || "");
  const [status, setStatus] = useState("");

  /**
   * Purpose: Handle drag and drop of files or folders onto the card.
   *
   * Inputs: Drag event payload.
   * Outputs: Updates watch list and status messaging.
   * Ties to: Destination drop zone.
   * Side effects: Prevents default drag behavior, updates state, and adds watch paths.
   * Why: Allow quick add by dropping paths on a destination.
   */
  const handleDrop = (e: React.DragEvent) => {
    try {
      e.preventDefault();
      const files = Array.from(e.dataTransfer?.files || []);
      if (!files.length) {
        setStatus("No files detected in drop.");
        return;
      }
      files.forEach((f) => {
        const path = resolveDropPath(f);
        const kind: "File" | "Directory" = f.type === "" && !f.name.includes(".") ? "Directory" : "File";
        if (path) onAddPath(dest.id, path, kind);
      });
      setStatus(`Added ${files.length} item(s)`);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      setStatus(`[DestCard::handleDrop] Failed to add dropped items: ${reason}`);
    }
  };

  try {
    return (
      <div className="dest-card" onDrop={handleDrop} onDragOver={(e) => e.preventDefault()}>
        <div className="dest-card__header">
          <div>
            <input
              className="dest-label-input"
              value={label}
              placeholder={dest.label || dest.id}
              onChange={(e) => {
                setLabel(e.target.value);
                onSetLabel?.(dest.id, e.target.value);
              }}
            />
            <div className="muted dest-path">{dest.path}</div>
          </div>
          <div className="dest-actions">
            <button className="icon-btn" onClick={() => setMenuOpen(!menuOpen)}>⋮</button>
          </div>
        </div>
        {menuOpen && <DestMenu retention={dest.max_backups_per_file ?? 3} onChange={(v) => onSetRetention(dest.id, v)} />}
        <div className="dest-drop">
          <div>Drag files/folders here</div>
          <div className="inline-actions" style={{ marginTop: 6 }}>
            <button className="btn secondary" onClick={() => onPickPath(dest.id, "Directory")}>Add folder</button>
            <button className="btn secondary" onClick={() => onPickPath(dest.id, "File")}>Add file</button>
          </div>
          {status && <div className="muted small">{status}</div>}
        </div>
        <DestItems items={items} onToggleEnabled={onToggleEnabled} onRemove={onRemove} />
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[DestCard] Failed to render destination card: ${reason}`);
  }
};

export default DestCard;
