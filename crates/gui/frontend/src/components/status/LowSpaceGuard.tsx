import React from "react";

type Props = {
  freeBytes: number | null;
  threshold: number;
  resumeOnSpace: boolean;
  onToggleResume: (v: boolean) => void;
};

/**
 * Purpose: Render a low disk space banner with resume toggle.
 *
 * Inputs: `freeBytes`, `threshold`, `resumeOnSpace`, and a toggle handler.
 * Outputs: A warning banner or `null` when space is sufficient.
 * Ties to: Status card and free-space guard settings.
 * Side effects: Registers UI event handlers for resume toggles.
 * Why: Highlights capacity risks and lets operators control resume behavior.
 */
const LowSpaceGuard: React.FC<Props> = ({ freeBytes, threshold, resumeOnSpace, onToggleResume }) => {
  /**
   * Purpose: Toggle resume behavior when low space is detected.
   *
   * Inputs: None.
   * Outputs: Calls `onToggleResume` with the inverted resume state.
   * Ties to: Status card state management for resume behavior.
   * Side effects: Invokes the provided toggle handler.
   * Why: Allows operators to choose auto-resume vs manual recovery.
   */
  const handleToggle = () => {
    try {
      onToggleResume(!resumeOnSpace);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      throw new Error(`[LowSpaceGuard.handleToggle] Failed to toggle resume behavior: ${reason}`);
    }
  };

  try {
    if (freeBytes == null || freeBytes >= threshold) {
      return null;
    }
    return (
      <div className="pill" style={{ borderColor: "#ff7b7b", color: "#ffb0b0", marginTop: 6 }}>
        <span title="Free-space guard will pause backups until space recovers.">
          Low disk space at backup destination
        </span>
        <button className="btn secondary" style={{ marginLeft: 8 }} onClick={handleToggle}>
          {resumeOnSpace ? "Pause until fixed" : "Resume when ok"}
        </button>
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[LowSpaceGuard] Failed to render low space guard: ${reason}`);
  }
};

export default LowSpaceGuard;
