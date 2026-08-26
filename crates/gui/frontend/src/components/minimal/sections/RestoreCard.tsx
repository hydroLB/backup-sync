import { Button } from '../../ui/Button';

type Props = {
  busy: boolean;
  disabled: boolean;
  onOpenRestore: () => void;
};

/** Recovery is one clear action; all choices happen in the focused dialog. */
export function RestoreCard({ busy, disabled, onOpenRestore }: Props) {
  return (
    <Button
      className="recovery-button"
      onClick={onOpenRestore}
      disabled={busy || disabled}
      aria-label="Recover a previous version"
      title={disabled ? 'A saved version is needed before recovery is available.' : undefined}
    >
      <span className="recovery-button__copy">
        <span className="recovery-button__step">03&nbsp;&nbsp;Recovery</span>
        <span className="recovery-button__title">Recover a previous version</span>
        <span className="recovery-button__hint">Browse saved versions and restore safely</span>
      </span>
      <span className="recovery-button__arrow" aria-hidden="true">
        →
      </span>
    </Button>
  );
}
