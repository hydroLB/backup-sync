import { formatBytes } from '../../../../utils/format';
import { DestinationStatus } from '../types';

type Props = {
  destStatus: DestinationStatus;
  onPickDestination: () => void;
  onUseDesktopDest: () => void;
  onUseDocumentsDest: () => void;
  onUseDownloadsDest: () => void;
};

/**
 * Summary: Render onboarding step 2 (choose where to store backups).
 *
 * Inputs: Destination status and destination selection handlers.
 * Outputs: A step body element tree.
 * Side effects: Calls provided handlers on user interaction.
 * Error handling: Delegated to the provided handlers.
 * Ties to other methods: Used by `OnboardingOverlay` body rendering.
 * Why this exists: Keep destination selection UI isolated and consistent.
 */
export function DestinationStep({
  destStatus,
  onPickDestination,
  onUseDesktopDest,
  onUseDocumentsDest,
  onUseDownloadsDest,
}: Props) {
  return (
    <>
      <p className="muted">
        Pick a backup destination. We’ll create the folder and block finishing until it’s writable.
      </p>
      <div className="inline-actions inline-actions-wrap">
        <button className="btn" onClick={onPickDestination}>
          Choose destination
        </button>
        <button className="btn secondary" onClick={onUseDesktopDest}>
          Use Desktop
        </button>
        <button className="btn secondary" onClick={onUseDocumentsDest}>
          Use Documents
        </button>
        <button className="btn secondary" onClick={onUseDownloadsDest}>
          Use Downloads
        </button>
      </div>
      <div className="muted">
        {destStatus ? (
          <>
            {destStatus.message}
            {destStatus.free_bytes != null && ` • Free: ${formatBytes(destStatus.free_bytes)}`}
          </>
        ) : (
          'Waiting for destination check…'
        )}
      </div>
    </>
  );
}
