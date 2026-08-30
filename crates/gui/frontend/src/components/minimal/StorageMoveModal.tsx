import { Button } from '../ui/Button';
import { ModalShell } from '../ui/ModalShell';

export type PendingStorageMove = {
  destinationId: string;
  label: string;
  oldPath: string;
  newPath: string;
  mode: 'move' | 'promote';
  existingLabel?: string;
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
      title={move?.mode === 'promote' ? 'Make this Main storage?' : 'Move backup storage?'}
      description={
        move?.mode === 'promote'
          ? 'Backup Sync will pause protection and verify both copies before changing their roles.'
          : 'Backup Sync will pause protection while it moves this storage location.'
      }
      onClose={onCancel}
      busy={busy}
      closeLabel="Cancel"
      footer={
        <Button
          type="button"
          loading={busy}
          loadingLabel={
            move?.mode === 'promote' ? 'Verifying both copies…' : 'Moving and verifying…'
          }
          onClick={() => void onConfirm()}
        >
          {move?.mode === 'promote' ? 'Verify and switch' : 'Move backups safely'}
        </Button>
      }
    >
      {move && (
        <div className="storage-move-flow">
          {move.mode === 'promote' ? (
            <>
              <div className="storage-move-path is-new">
                <span>New Main storage · {move.existingLabel}</span>
                <strong title={move.newPath}>{move.newPath}</strong>
              </div>
              <span className="storage-move-arrow" aria-hidden="true">
                ⇄
              </span>
              <div className="storage-move-path">
                <span>Becomes a secondary backup</span>
                <strong title={move.oldPath}>{move.oldPath}</strong>
              </div>
              <p className="storage-move-warning" role="alert">
                No backup files will be moved or deleted. Backup Sync will fully verify the selected
                copy, switch the two storage roles, and roll back if the background service cannot
                activate the change.
              </p>
            </>
          ) : (
            <>
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
                Every backup file is copied and verified first. Only after the new location is
                safely active will the backup files in the old location be deleted.
              </p>
            </>
          )}
        </div>
      )}
    </ModalShell>
  );
}
