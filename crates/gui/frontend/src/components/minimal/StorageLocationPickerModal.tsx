import { ModalShell } from '../ui/ModalShell';

type Props = {
  open: boolean;
  usedPaths: string[];
  onCancel: () => void;
  onChoose: (path: string) => void;
};

const STORAGE_OPTIONS = [
  {
    label: 'Local backup vault',
    detail: 'Browser-local storage on this device',
    path: 'browser-vault://local-backup',
  },
  {
    label: 'External drive',
    detail: 'A separate removable-drive backup',
    path: 'browser-vault://external-drive',
  },
  {
    label: 'Network storage',
    detail: 'A complete copy on network-attached storage',
    path: 'browser-vault://network-storage',
  },
] as const;

/** Browser storage is selected explicitly before the shared move confirmation is shown. */
export function StorageLocationPickerModal({ open, usedPaths, onCancel, onChoose }: Props) {
  const used = new Set(usedPaths);

  return (
    <ModalShell
      open={open}
      title="Choose backup location"
      description="Select a controlled storage destination for the browser showcase."
      onClose={onCancel}
      closeLabel="Cancel"
    >
      <div className="storage-choice-list">
        {STORAGE_OPTIONS.map((option) => {
          const alreadyUsed = used.has(option.path);
          return (
            <button
              key={option.path}
              type="button"
              className="storage-choice-row"
              disabled={alreadyUsed}
              onClick={() => onChoose(option.path)}
            >
              <span className="storage-choice-row__copy">
                <strong>{option.label}</strong>
                <small>{option.detail}</small>
              </span>
              <span className="storage-choice-row__status">
                {alreadyUsed ? 'Already in use' : 'Choose'}
              </span>
            </button>
          );
        })}
      </div>
    </ModalShell>
  );
}
