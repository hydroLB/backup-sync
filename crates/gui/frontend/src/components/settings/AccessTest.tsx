import React from "react";

type Props = {
  onTest: () => void;
  result: {
    destination_writable: boolean;
    destination_message: string;
    watched_ok: string[];
    watched_missing: string[];
    watched_unwritable: string[];
  } | null;
  error?: string | null;
};

/**
 * Purpose: Render access test controls and results.
 *
 * Inputs: Test handler, result payload, and error message.
 * Outputs: An access test panel with status details.
 * Ties to: Settings access test workflow.
 * Side effects: Registers UI event handlers for access tests.
 * Why: Surface path and destination access issues early.
 */
const AccessTest: React.FC<Props> = ({ onTest, result, error }) => {
  try {
    return (
      <div className="card" style={{ background: "rgba(255,255,255,0.04)" }}>
        <div className="section-title">
          <h4 style={{ margin: 0 }}>Test access</h4>
          <button className="btn secondary" onClick={onTest}>Run test</button>
        </div>
        {result ? (
          <div style={{ display: "grid", gap: 6 }}>
            <div className="muted">Destination: {result.destination_message}</div>
            {result.watched_missing.length > 0 && (
              <div style={{ color: "#ff7b7b" }}>Missing: {result.watched_missing.join(", ")}</div>
            )}
            {result.watched_unwritable.length > 0 && (
              <div style={{ color: "#ff7b7b" }}>Unwritable: {result.watched_unwritable.join(", ")}</div>
            )}
            {result.watched_ok.length > 0 && (
              <div className="muted">OK: {result.watched_ok.length} paths</div>
            )}
          </div>
        ) : (
          <p className="muted">Quickly check that watched paths exist and destination is writable.</p>
        )}
        {error && <div style={{ color: "#ff7b7b" }}>{error}</div>}
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[AccessTest] Failed to render access test panel: ${reason}`);
  }
};

export default AccessTest;
