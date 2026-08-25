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

/** Keep section layout consistent and reduce inline markup in `MinimalMain`. */
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
  const destinationCount = destinations.filter(
    (destination) => destination.path.trim().length > 0,
  ).length;

  return (
    <section className="card destination-card" aria-labelledby="destination-card-title">
      <div className="section-identity">
        <span className="section-step-badge" aria-hidden="true">
          01
        </span>
        <div>
          <span className="section-eyebrow">Storage</span>
          <h2 className="section-heading" id="destination-card-title">
            Where should copies live?
          </h2>
        </div>
      </div>
      <p className="section-subtitle section-subtitle--roomy">
        {destinationCount > 0
          ? `${destinationCount} location${destinationCount === 1 ? ' is' : 's are'} ready. Every added location receives its own recoverable copy.`
          : 'Choose the first place where Backup Sync should preserve version history.'}
      </p>
      {primaryDestination?.path ? (
        <div className="pill pill-row destination-primary-row mt-3" title={primaryDestination.path}>
          <span className="pill-main destination-primary-copy">
            <span className="destination-prefix">Primary destination</span>
            <span className="truncate destination-primary-path">{primaryDestination.path}</span>
          </span>
          <Button
            tone="secondary"
            size="sm"
            className="pill-action destination-primary-change"
            onClick={onChoose}
            disabled={busy}
            aria-label="Change primary destination"
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
              aria-label="Choose primary destination"
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
          aria-label="Add another destination"
        >
          Add another destination
        </Button>
      </div>
      {replicationWarning && <InlineAlert kind="error">{replicationWarning}</InlineAlert>}
    </section>
  );
}
