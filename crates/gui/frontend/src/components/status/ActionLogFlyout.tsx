import React, { useState } from "react";
import { formatDateTime } from "../../utils/format";

export type ActionLogEntry = { msg: string; kind: "ok" | "error" | "info"; ts: number };

type Props = {
  items: ActionLogEntry[];
};

/**
 * Purpose: Render a flyout panel with recent action log entries.
 *
 * Inputs: `items` as the list of action log entries.
 * Outputs: A React element tree showing recent activity and a toggle button.
 * Ties to: `formatDateTime` for timestamps and `ActionLogEntry` for typing.
 * Side effects: Registers React state hooks and event handlers for toggling.
 * Why: Gives operators a quick timeline of recent UI actions.
 */
const ActionLogFlyout: React.FC<Props> = ({ items }) => {
  const [open, setOpen] = useState<boolean>(false);
  /**
   * Purpose: Toggle the open state of the action log panel.
   *
   * Inputs: None.
   * Outputs: Updates `open` state used by the flyout.
   * Ties to: `open` state and the toggle button click handler.
   * Side effects: Updates React state for panel visibility.
   * Why: Allows operators to expand or collapse the activity feed.
   */
  const toggleOpen = () => {
    try {
      setOpen((prev) => !prev);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      throw new Error(`[ActionLogFlyout.toggleOpen] Failed to toggle activity panel: ${reason}`);
    }
  };

  try {
    const latest = items.slice(-1)[0];
    return (
      <div className="action-log">
        <button className="btn secondary" onClick={toggleOpen}>
          {open ? "Hide activity" : "What just happened?"}
        </button>
        {latest && !open && (
          <span className="muted" style={{ marginLeft: 8 }}>
            Last: {latest.msg}
          </span>
        )}
        {open && (
          <div className="action-log__panel">
            <div className="section-title" style={{ marginBottom: 6 }}>
              <h4 style={{ margin: 0 }}>Recent activity</h4>
              <span className="pill">{items.length} events</span>
            </div>
            {items.length === 0 && (
              <div className="muted">No actions yet. Run a backup or change settings to see updates.</div>
            )}
            {items.slice().reverse().map((e, idx) => (
              <div key={idx} className={`action-log__row action-log__row--${e.kind}`}>
                <div className="muted">{formatDateTime(e.ts)}</div>
                <div>{e.msg}</div>
              </div>
            ))}
          </div>
        )}
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[ActionLogFlyout] Failed to render action log flyout: ${reason}`);
  }
};

export default ActionLogFlyout;
