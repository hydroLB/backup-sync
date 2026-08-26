import { Button } from '../ui/Button';
import { ModalShell } from '../ui/ModalShell';

export type PendingStorageMove = {
  destinationId: string;
  label: string;
  oldPath: string;
  newPath: string;
};

type Props = {
  move: PendingStorageMove | null;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => Promise<void>;
};

/** Make destructive storage relocation explicit before any files or settings change. */
export function StorageMoveModal({ move, busy, onCancel, onConfirm }: Props) {
  return (
    <ModalShell
      open={move !== null}
      title="Move backup storage?"
      description="Backup Sync will pause protection while it moves this storage location."
      onClose={onCancel}
      busy={busy}
      closeLabel="Cancel"
      footer={
        <Button
          type="button"
          loading={busy}
          loadingLabel="Moving and verifying…"
          onClick={() => void onConfirm()}
        >
          Move backups safely
        </Button>
      }
    >
      {move && (
        <div className="storage-move-flow">
          <div className="storage-move-path">
            <span>Current {move.label.toLowerCase()}</span>
            <strong title={move.oldPath}>{move.oldPath}</strong>
          </div>
          <span className="storage-move-arrow" aria-hidden="true">
            ↓
          </span>
          <div className="storage-move-path is-new">
            <span>New location</span>
            <strong title={move.newPath}>{move.newPath}</strong>
          </div>
          <p className="storage-move-warning">
            Every backup file is copied and verified first. Only after the new location is safely
            active will the backup files in the old location be deleted.
          </p>
        </div>
      )}
    </ModalShell>
  );
}
