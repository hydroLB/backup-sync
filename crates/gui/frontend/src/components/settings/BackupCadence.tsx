import React from 'react';

type Props = {
  max_backups_per_file: number;
  onChange: (data: Partial<{ interval_seconds: number; max_backups_per_file: number }>) => void;
};

/**
 * Purpose: Render cadence controls for the fixed automatic schedule and retention.
 *
 * Inputs: Max backups per file and change handler.
 * Outputs: A form section for cadence settings.
 * Ties to: Settings panel cadence section.
 * Side effects: Registers UI event handlers for cadence updates.
 * Why: Keep scheduling consistent while still letting operators tune retention defaults.
 */
const BackupCadence: React.FC<Props> = ({ max_backups_per_file, onChange }) => {
  try {
    return (
        <>
          <h3>Backup cadence</h3>
        <div className="muted">Automatic backups run every 30 minutes.</div>
        <label>
          Max backups per file
          <input
            type="number"
            value={max_backups_per_file}
            onChange={(e) => onChange({ max_backups_per_file: Number(e.target.value) })}
          />
        </label>
      </>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[BackupCadence] Failed to render cadence controls: ${reason}`);
  }
};

export default BackupCadence;
