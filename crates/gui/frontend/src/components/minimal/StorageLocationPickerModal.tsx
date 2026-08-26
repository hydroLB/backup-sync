import { ModalShell } from '../ui/ModalShell';

type Props = {
  open: boolean;
  usedPaths: string[];
  onCancel: () => void;
  onChoose: (path: string) => void;
};

function nextDemoChoiceNumber(usedPaths: string[]): number {
  return (
    usedPaths.reduce((highest, path) => {
      const match = path.match(/^browser-vault:\/\/demo-backup-choice-(\d+)$/);
      return match ? Math.max(highest, Number(match[1])) : highest;
    }, 0) + 1
  );
}

/** Browser storage is selected explicitly before the shared move confirmation is shown. */
export function StorageLocationPickerModal({ open, usedPaths, onCancel, onChoose }: Props) {
  const choiceNumber = nextDemoChoiceNumber(usedPaths);
  const choiceName = `demo-backup-choice-${choiceNumber}`;
  const choicePath = `browser-vault://${choiceName}`;

  return (
    <ModalShell
      open={open}
      title="Choose backup location"
      description="Select a controlled storage destination for the browser showcase."
      onClose={onCancel}
      closeLabel="Cancel"
    >
      <div className="storage-choice-list">
        <button type="button" className="storage-choice-row" onClick={() => onChoose(choicePath)}>
          <span className="storage-choice-row__copy">
            <strong>{choiceName}</strong>
            <small>Demo-only location for trying the backup flow</small>
          </span>
          <span className="storage-choice-row__status">Choose</span>
        </button>
      </div>
    </ModalShell>
  );
}
