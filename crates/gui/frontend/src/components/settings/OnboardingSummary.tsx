import React from "react";
import { formatBytes } from "../../utils/format";

type Props = {
  watchedCount: number;
  destinationMessage?: string;
  freeBytes?: number | null;
  safeMode?: boolean;
  onSimulate?: () => void;
};

/**
 * Purpose: Render an onboarding summary snapshot.
 *
 * Inputs: Watched count, destination info, free space, and simulate handler.
 * Outputs: A summary card element.
 * Ties to: Onboarding and settings summary sections.
 * Side effects: Registers UI event handlers for simulation actions.
 * Why: Provide a quick snapshot of setup progress.
 */
const OnboardingSummary: React.FC<Props> = ({ watchedCount, destinationMessage, freeBytes, safeMode, onSimulate }) => {
  try {
    const freeLabel = freeBytes == null ? "Checking free space…" : formatBytes(freeBytes);
    return (
      <div className="muted" style={{ marginTop: 6, border: "1px solid var(--border)", borderRadius: 10, padding: 10, background: "rgba(255,255,255,0.04)" }}>
        <div>Watched: {watchedCount > 0 ? `${watchedCount} item${watchedCount === 1 ? "" : "s"}` : "None selected"}</div>
        <div>Destination: {destinationMessage ?? "Not set"}</div>
        <div>Free space: {freeLabel}</div>
        <div>Safe mode: {safeMode ? "On (scan/verify only)" : "Off (normal backups)"}</div>
        {onSimulate && <button className="btn secondary" style={{ marginTop: 8 }} onClick={onSimulate}>Simulate backup</button>}
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[OnboardingSummary] Failed to render onboarding summary: ${reason}`);
  }
};

export default OnboardingSummary;
