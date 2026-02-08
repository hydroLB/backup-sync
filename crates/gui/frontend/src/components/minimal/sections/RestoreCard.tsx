import { Button } from '../../ui/Button';
import { StateBlock } from '../../ui/StateBlock';

type Props = {
  busy: boolean;
  disabled: boolean;
  watchedCount: number;
  onOpenRestore: () => void;
};

/**
 * Summary: Render a dedicated restore entry card with clear restore capabilities.
 *
 * Inputs: Busy and disabled flags, watched folder count, and modal open handler.
 * Outputs: A restore card element tree.
 * Side effects: Calls `onOpenRestore` when the user starts restore.
 * Error handling: None.
 * Ties to other methods: Used by `MinimalMain` and opens `RestoreModal`.
 * Why this exists: Keep restore as a first-class workflow instead of hiding it in header actions.
 */
export function RestoreCard({ busy, disabled, watchedCount, onOpenRestore }: Props) {
  return (
    <div className="card restore-card">
      <div className="section-title restore-card-head">
        <h2 className="section-heading">Restore Backup Version</h2>
        <span className="pill restore-count-pill">
          {watchedCount} watched folder{watchedCount === 1 ? '' : 's'}
        </span>
      </div>

      {disabled ? (
        <StateBlock
          tone="empty"
          title="Add a folder before restoring"
          message="Restore options appear after at least one folder has backup history."
        />
      ) : (
        <>
          <div className="restore-capabilities">
            <span className="restore-capability">Entire folder restore</span>
            <span className="restore-capability">Individual file restore</span>
            <span className="restore-capability">Saved version selection</span>
            <span className="restore-capability">Choose restore location</span>
          </div>
          <div className="restore-actions">
            <Button
              onClick={onOpenRestore}
              disabled={busy}
              title="Open restore flow and choose what to restore."
            >
              Restore Backup Version
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
