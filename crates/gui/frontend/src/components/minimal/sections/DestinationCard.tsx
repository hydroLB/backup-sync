import { InlineAlert } from '../../ui/InlineAlert';
import { Button } from '../../ui/Button';
import { StateBlock } from '../../ui/StateBlock';

type Props = {
  destinations: Array<{
    id: string;
    label?: string | null;
    path: string;
  }>;
  busy: boolean;
  destinationWarning: string | null;
  replicationWarning: string | null;
  onChoose: () => void;
  onAddDestination: () => void;
  onRemoveDestination: (destinationId: string) => void;
};

/**
 * Summary: Render the destination selection card in minimal mode.
 *
 * Inputs: Current destination path, busy flag, and choose handler.
 * Outputs: Card React element tree.
 * Side effects: Calls `onChoose` when the user clicks the picker button.
 * Error handling: Delegated to parent via handler.
 * Ties to other methods: Used by `MinimalMain` layout for destination setup.
 * Why this exists: Keep section layout consistent and reduce inline markup in `MinimalMain`.
 */
export function DestinationCard({
  destinations,
  busy,
  destinationWarning,
  replicationWarning,
  onChoose,
  onAddDestination,
  onRemoveDestination,
}: Props) {
  const primaryDestination = destinations[0] || null;
  const additionalDestinations = destinations.slice(1);

  return (
    <div className="card destination-card">
      <h2 className="section-heading">Destination</h2>
      {primaryDestination?.path ? (
        <div className="pill pill-row destination-primary-row mt-3" title={primaryDestination.path}>
          <span className="pill-main truncate">
            <span className="destination-prefix">Default:</span> {primaryDestination.path}
          </span>
          <Button
            tone="secondary"
            size="sm"
            className="pill-action destination-primary-change"
            onClick={onChoose}
            disabled={busy}
          >
            Change…
          </Button>
        </div>
      ) : (
        <StateBlock
          tone="empty"
          title="No primary destination selected"
          message="Choose a backup location before running backups."
          className="mt-3"
          action={
            <Button
              tone="secondary"
              size="sm"
              className="pill-action"
              onClick={onChoose}
              disabled={busy}
            >
              Choose…
            </Button>
          }
        />
      )}
      {additionalDestinations.length > 0 && (
        <div className="destination-list mt-3">
          {additionalDestinations.map((destination) => (
            <div
              key={destination.id}
              className="pill pill-row destination-row"
              title={destination.path}
            >
              <span className="pill-main truncate">
                {(destination.label && destination.label.trim().length > 0
                  ? destination.label
                  : 'Destination') + ': '}
                {destination.path}
              </span>
              <Button
                tone="secondary"
                size="sm"
                className="pill-action destination-remove-btn"
                onClick={() => onRemoveDestination(destination.id)}
                disabled={busy}
                aria-label={`Remove destination ${destination.label ?? destination.path}`}
                title="Remove destination"
              >
                ×
              </Button>
            </div>
          ))}
        </div>
      )}
      {destinationWarning && <InlineAlert kind="error">{destinationWarning}</InlineAlert>}
      <div className="btn-ring btn-ring--block mt-3">
        <Button
          tone="secondary"
          size="sm"
          className="destination-action-btn"
          block
          onClick={onAddDestination}
          disabled={busy}
        >
          Add another destination
        </Button>
      </div>
      {replicationWarning && <InlineAlert kind="error">{replicationWarning}</InlineAlert>}
    </div>
  );
}
