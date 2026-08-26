import { useState } from 'react';
import { InlineAlert } from '../../ui/InlineAlert';
import { Button } from '../../ui/Button';
import { StateBlock } from '../../ui/StateBlock';

type Destination = {
  id: string;
  label?: string | null;
  path: string;
};

type Props = {
  destinations: Destination[];
  busy: boolean;
  destinationWarning: string | null;
  replicationWarning: string | null;
  onChoose: () => Promise<void>;
  onChangeDestination: (destinationId: string) => Promise<void>;
  onAddDestination: () => Promise<void>;
  onRemoveDestination: (destinationId: string) => Promise<void>;
};

/** Keep storage separate from protected content: every location receives every source. */
export function DestinationCard({
  destinations,
  busy,
  destinationWarning,
  replicationWarning,
  onChoose,
  onChangeDestination,
  onAddDestination,
  onRemoveDestination,
}: Props) {
  const [confirmRemoveId, setConfirmRemoveId] = useState<string | null>(null);
  const configured = destinations.filter((destination) => destination.path.trim().length > 0);

  return (
    <section className="card destination-card" aria-labelledby="destination-card-title">
      <div className="section-title">
        <div className="section-identity">
          <span className="section-step-badge" aria-hidden="true">
            02
          </span>
          <div>
            <span className="section-eyebrow">Storage</span>
            <h2 className="section-heading" id="destination-card-title">
              Backup locations
            </h2>
            <p className="section-summary">Every protected item is copied to each location</p>
          </div>
        </div>
        <Button
          size="sm"
          onClick={() => void onAddDestination()}
          disabled={busy}
          aria-label="Add backup location"
        >
          <span aria-hidden="true">+</span> Add location
        </Button>
      </div>

      {configured.length === 0 ? (
        <StateBlock
          tone="empty"
          title="No storage selected"
          message="Choose one destination. A second location adds an independent copy."
          className="compact-empty-state"
          action={
            <Button size="sm" onClick={() => void onChoose()} disabled={busy}>
              Choose storage
            </Button>
          }
        />
      ) : (
        <div className="destination-list compact-destination-list">
          {configured.map((destination, index) => (
            <div
              className={`destination-item ${confirmRemoveId === destination.id ? 'is-confirming' : ''}`}
              key={destination.id}
            >
              {confirmRemoveId === destination.id ? (
                <div className="inline-confirm destination-confirm" role="alert">
                  <span>Remove this storage location?</span>
                  <div>
                    <Button
                      tone="secondary"
                      size="sm"
                      onClick={() => setConfirmRemoveId(null)}
                      disabled={busy}
                    >
                      Cancel
                    </Button>
                    <Button
                      tone="danger"
                      size="sm"
                      onClick={() => void onRemoveDestination(destination.id)}
                      disabled={busy}
                    >
                      Remove
                    </Button>
                  </div>
                </div>
              ) : (
                <>
                  <button
                    type="button"
                    className="destination-item__main"
                    onClick={() => void onChangeDestination(destination.id)}
                    disabled={busy}
                    aria-label={
                      index === 0 ? 'Change main storage' : 'Change secondary backup location'
                    }
                  >
                    <span className="destination-item__label">
                      {index === 0
                        ? 'Main storage'
                        : index === 1
                          ? 'Secondary backup location'
                          : `Secondary backup location ${index}`}
                    </span>
                    <span className="destination-item__path truncate" title={destination.path}>
                      {destination.path}
                    </span>
                    <span className="destination-item__hint">
                      {index === 0 ? 'Primary backup copy' : 'Complete redundant backup copy'}
                    </span>
                  </button>
                  {index > 0 && (
                    <button
                      type="button"
                      className="icon-remove"
                      onClick={() => setConfirmRemoveId(destination.id)}
                      disabled={busy}
                      aria-label={`Remove secondary backup location ${destination.path}`}
                      title="Remove secondary backup location"
                    >
                      ×
                    </button>
                  )}
                </>
              )}
            </div>
          ))}
        </div>
      )}

      {destinationWarning && <InlineAlert kind="error">{destinationWarning}</InlineAlert>}
      {replicationWarning && <InlineAlert kind="error">{replicationWarning}</InlineAlert>}
    </section>
  );
}
