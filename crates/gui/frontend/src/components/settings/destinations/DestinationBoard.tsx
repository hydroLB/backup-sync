import React from "react";
import { Destination, WatchedPath } from "../types";
import DestCard from "./DestCard";

type Props = {
  destinations: Destination[];
  watched: WatchedPath[];
  onAddPath: (destId: string, path: string, kind: "File" | "Directory") => void;
  onPickPath: (destId: string, kind: "File" | "Directory") => void;
  onToggleEnabled: (path: string) => void;
  onRemove: (path: string) => void;
  onAddDestination: () => void;
  onSetDestRetention: (destId: string, v: number) => void;
  onSetDestLabel: (destId: string, label: string) => void;
};

/**
 * Purpose: Render the destination board with all configured destinations.
 *
 * Inputs: Destinations, watched items, and action handlers.
 * Outputs: A destination board element.
 * Ties to: Settings destination management.
 * Side effects: Registers UI event handlers for destination actions.
 * Why: Provide a unified view of destinations and their watched paths.
 */
const DestinationBoard: React.FC<Props> = ({
  destinations,
  watched,
  onAddPath,
  onPickPath,
  onToggleEnabled,
  onRemove,
  onAddDestination,
  onSetDestRetention,
  onSetDestLabel,
}) => {
  try {
    return (
      <div className="destination-board">
        <div className="board-header">
          <h3>Backup locations</h3>
          <button className="btn secondary" onClick={onAddDestination}>Add destination</button>
        </div>
        <div className="dest-grid">
          {destinations.map((d) => {
            const items = watched.filter((w) => (w.destination_id || "default") === d.id);
            return (
              <DestCard
                key={d.id}
                dest={d}
                items={items}
                onAddPath={onAddPath}
                onPickPath={onPickPath}
                onToggleEnabled={onToggleEnabled}
                onRemove={onRemove}
                onSetRetention={onSetDestRetention}
                onSetLabel={onSetDestLabel}
              />
            );
          })}
        </div>
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[DestinationBoard] Failed to render destinations: ${reason}`);
  }
};

export default DestinationBoard;
