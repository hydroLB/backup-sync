import { useState } from 'react';
import { Button } from '../../ui/Button';
import { StateBlock } from '../../ui/StateBlock';
import { compactFilesystemPath } from '../../../utils/pathDisplay';

type Destination = {
  id: string;
  label?: string | null;
  path: string;
};

type Props = {
  destinations: Destination[];
  busy: boolean;
  updated: boolean;
  onChoose: () => Promise<void>;
  onOpenDestination: (destinationId: string) => Promise<void>;
  onChangeDestination: (destinationId: string) => Promise<void>;
  onAddDestination: () => Promise<void>;
  onRemoveDestination: (destinationId: string) => Promise<void>;
};

/** Keep storage separate from protected content: every location receives every source. */
export function DestinationCard({
  destinations,
  busy,
  updated,
  onChoose,
  onOpenDestination,
  onChangeDestination,
  onAddDestination,
  onRemoveDestination,
}: Props) {
  const [confirmRemoveId, setConfirmRemoveId] = useState<string | null>(null);
  const configured = destinations.filter((destination) => destination.path.trim().length > 0);

  return (
    <section className="card destination-card" aria-labelledby="destination-card-title">
      {updated && (
        <span className="section-update-indicator" role="status">
          <span aria-hidden="true">✓</span> Updated
        </span>
      )}
      <div className="section-title">
        <div className="section-identity">
          <span className="section-step-badge" aria-hidden="true">
            02
          </span>
          <div>
            <span className="section-eyebrow">Storage</span>
            <div className="section-heading-row">
              <h2 className="section-heading" id="destination-card-title">
                Backup locations
              </h2>
              <span
                className="section-count"
                aria-label={`${configured.length} backup ${configured.length === 1 ? 'location' : 'locations'} total`}
              >
                {configured.length} total
              </span>
            </div>
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
                    className="destination-item__change-surface"
                    onClick={() => void onChangeDestination(destination.id)}
                    disabled={busy}
                    aria-label={
                      index === 0
                        ? 'Change main storage'
                        : `Change secondary backup location ${index}`
                    }
                  />
                  <div className="destination-item__main">
                    <button
                      type="button"
                      className="destination-item__open"
                      onClick={() => void onOpenDestination(destination.id)}
                      disabled={busy}
                      aria-label={
                        index === 0
                          ? 'Open main storage folder'
                          : `Open secondary backup location ${index}`
                      }
                    >
                      <span className="destination-item__label">
                        {index === 0 ? 'Main storage' : `Secondary backup location ${index}`}
                      </span>
                      <span className="destination-item__path-line">
                        <span className="destination-item__path truncate" title={destination.path}>
                          {compactFilesystemPath(destination.path)}
                        </span>
                        <svg
                          className="destination-item__open-icon"
                          viewBox="0 0 20 20"
                          fill="none"
                          aria-hidden="true"
                        >
                          <path d="M3.25 6.25h5l1.5 1.75h7v7.25a1.5 1.5 0 0 1-1.5 1.5H4.75a1.5 1.5 0 0 1-1.5-1.5v-9Z" />
                          <path d="M12 3.25h4.75V8M16.5 3.5l-5.25 5.25" />
                        </svg>
                      </span>
                    </button>
                    <span className="destination-item__hint">
                      {index === 0 ? 'Primary backup copy' : 'Complete redundant backup copy'}
                    </span>
                  </div>
                  <svg
                    className="destination-item__edit-icon"
                    viewBox="0 0 20 20"
                    fill="none"
                    aria-hidden="true"
                  >
                    <path d="m4 14.75-.5 2.25 2.25-.5L15.9 6.35a1.55 1.55 0 0 0 0-2.2l-.05-.05a1.55 1.55 0 0 0-2.2 0L4 14.75Z" />
                    <path d="m12.5 5.25 2.25 2.25" />
                  </svg>
                  <div className="destination-item__actions">
                    {index > 0 && (
                      <button
                        type="button"
                        className="icon-remove"
                        onClick={() => setConfirmRemoveId(destination.id)}
                        disabled={busy}
                        aria-label={`Remove secondary backup location ${destination.path}`}
                        title="Remove secondary backup location"
                      >
                        <span className="icon-remove__glyph" aria-hidden="true">
                          ×
                        </span>
                      </button>
                    )}
                  </div>
                </>
              )}
            </div>
          ))}
        </div>
      )}
    </section>
  );
}
