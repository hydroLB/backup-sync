import { Button } from '../../ui/Button';
import { StateBlock } from '../../ui/StateBlock';

type Props = {
  busy: boolean;
  disabled: boolean;
  watchedCount: number;
  onOpenRestore: () => void;
};

/** Keep restore as a first-class workflow instead of hiding it in header actions. */
export function RestoreCard({ busy, disabled, watchedCount, onOpenRestore }: Props) {
  return (
    <section className="card restore-card" aria-labelledby="restore-card-title">
      <div className="section-title restore-card-head">
        <div className="section-identity">
          <span className="section-step-badge" aria-hidden="true">
            03
          </span>
          <div>
            <span className="section-eyebrow">Recovery</span>
            <h2 className="section-heading" id="restore-card-title">
              Go back to any saved version
            </h2>
          </div>
        </div>
        <span className="pill restore-count-pill">
          {watchedCount} watched folder{watchedCount === 1 ? '' : 's'}
        </span>
      </div>
      <p className="section-subtitle section-subtitle--roomy">
        Browse history by date, recover one file, or rebuild an entire folder somewhere safe.
      </p>
      <div className="restore-capabilities">
        <span className="restore-capability">Folder restore</span>
        <span className="restore-capability">Single-file restore</span>
        <span className="restore-capability">Saved versions</span>
        <span className="restore-capability">Alternate destination</span>
      </div>

      {disabled ? (
        <StateBlock
          tone="empty"
          title="Restore unlocks after setup finishes"
          message="Add a protected path first. Restore becomes available after Backup Sync has at least one saved version to read from."
          className="mt-3 restore-empty-state"
        />
      ) : (
        <>
          <div className="restore-actions">
            <Button
              onClick={onOpenRestore}
              disabled={busy}
              title="Open restore flow and choose what to restore."
            >
              Browse saved versions
            </Button>
          </div>
        </>
      )}
    </section>
  );
}
