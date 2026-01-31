import React from "react";

type Props = {
  interval_seconds: number;
  max_backups_per_file: number;
  onChange: (data: Partial<{ interval_seconds: number; max_backups_per_file: number }>) => void;
};

/**
 * Purpose: Render cadence controls for backup interval and retention.
 *
 * Inputs: Interval seconds, max backups per file, and change handler.
 * Outputs: A form section for cadence settings.
 * Ties to: Settings panel cadence section.
 * Side effects: Registers UI event handlers for cadence updates.
 * Why: Allow operators to tune backup frequency and retention defaults.
 */
const BackupCadence: React.FC<Props> = ({ interval_seconds, max_backups_per_file, onChange }) => {
  try {
    return (
      <>
        <h3>Backup cadence</h3>
        <label>
          Backup interval (seconds)
          <input type="number" value={interval_seconds} onChange={(e) => onChange({ interval_seconds: Number(e.target.value) })} />
        </label>
        <label>
          Max backups per file
          <input type="number" value={max_backups_per_file} onChange={(e) => onChange({ max_backups_per_file: Number(e.target.value) })} />
        </label>
      </>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[BackupCadence] Failed to render cadence controls: ${reason}`);
  }
};

export default BackupCadence;
