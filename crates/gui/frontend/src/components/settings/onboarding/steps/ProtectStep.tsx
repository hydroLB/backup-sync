type Props = {
  watchedPaths: string[];
  onAddFolder: () => void;
  onQuickAddDesktop: () => void;
  onQuickAddDocuments: () => void;
  onQuickAddDownloads: () => void;
};

/**
 * Summary: Render onboarding step 1 (choose what to protect).
 *
 * Inputs: Current watched paths and add/quick-add handlers.
 * Outputs: A step body element tree.
 * Side effects: Calls provided handlers on user interaction.
 * Error handling: Delegated to the provided handlers.
 * Ties to other methods: Used by `OnboardingOverlay` body rendering.
 * Why this exists: Keep step-specific UI isolated for clarity and easier iteration.
 */
export function ProtectStep({
  watchedPaths,
  onAddFolder,
  onQuickAddDesktop,
  onQuickAddDocuments,
  onQuickAddDownloads,
}: Props) {
  return (
    <>
      <p className="muted">
        Add at least one folder. We’ll keep you here until something is selected.
      </p>
      <div className="inline-actions inline-actions-wrap">
        <button className="btn" onClick={onAddFolder}>
          Add path…
        </button>
      </div>
      <div className="inline-actions inline-actions-wrap">
        <button className="btn secondary" onClick={onQuickAddDesktop}>
          Quick add Desktop
        </button>
        <button className="btn secondary" onClick={onQuickAddDocuments}>
          Quick add Documents
        </button>
        <button className="btn secondary" onClick={onQuickAddDownloads}>
          Quick add Downloads
        </button>
      </div>
      {watchedPaths.length > 0 && (
        <div className="watched-list">
          <div className="muted">Currently protected:</div>
          <ul className="watched-ul">
            {watchedPaths.slice(0, 5).map((p) => (
              <li key={p}>{p}</li>
            ))}
          </ul>
          {watchedPaths.length > 5 && <div className="muted">+{watchedPaths.length - 5} more</div>}
        </div>
      )}
    </>
  );
}
