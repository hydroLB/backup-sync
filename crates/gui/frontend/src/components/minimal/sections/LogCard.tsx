import { Button } from '../../ui/Button';
import { ModalShell } from '../../ui/ModalShell';
import { StateBlock } from '../../ui/StateBlock';

type Props = {
  open: boolean;
  busy: boolean;
  loading: boolean;
  tail: string;
  onRefresh: () => void;
  onClose: () => void;
};

/**
 * Summary: Render the log tail in a modal popup for minimal mode.
 *
 * Inputs: Open state, busy/loading state, current tail text, and close/refresh handlers.
 * Outputs: Modal dialog element tree.
 * Side effects: Calls `onRefresh` for manual refresh and `onClose` for modal dismissal.
 * Error handling: Delegated to parent via handler.
 * Ties to other methods: Used by `MinimalMain` as the log popup surface.
 * Why this exists: Keep logs accessible without stretching the main dashboard layout.
 */
export function LogCard({ open, busy, loading, tail, onRefresh, onClose }: Props) {
  return (
    <ModalShell
      open={open}
      title="Log"
      onClose={onClose}
      closeLabel="×"
      closeAriaLabel="Close log"
      closeButtonClassName="modal-close-x"
      footer={
        <Button
          tone="secondary"
          onClick={onRefresh}
          disabled={busy}
          loading={loading}
          loadingLabel="Refreshing..."
        >
          Refresh
        </Button>
      }
    >
      {loading ? (
        <StateBlock
          tone="loading"
          title="Refreshing logs"
          message="Reading latest entries from the daemon."
          className="mt-3"
        />
      ) : tail ? (
        <pre className="log-pre">{tail}</pre>
      ) : (
        <StateBlock
          tone="empty"
          title="No log entries yet"
          message="Run a backup or refresh again to see activity."
          className="mt-3"
        />
      )}
    </ModalShell>
  );
}
