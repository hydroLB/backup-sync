import { WatchedPath } from '../../../domain/config';
import { Button } from '../../ui/Button';
import { InlineAlert } from '../../ui/InlineAlert';
import { StateBlock } from '../../ui/StateBlock';

type Item = {
  path: string;
  kind: 'File' | 'Directory';
  destination_id: string;
  max_backups_per_file: number | null;
};

type DestinationOption = {
  id: string;
  label?: string | null;
  path: string;
};

type Props = {
  busy: boolean;
  items: Item[];
  destinations: DestinationOption[];
  destinationReady: boolean;
  defaultKeep: number;
  watchedWarning: string | null;
  onAddFolder: () => void;
  onRemovePath: (path: string, kind: 'File' | 'Directory', sourceDestinationId: string) => void;
  onUpdateKeep: (
    path: string,
    kind: 'File' | 'Directory',
    sourceDestinationId: string,
    keep: number,
  ) => void;
  onUpdateDestination: (
    path: string,
    kind: 'File' | 'Directory',
    sourceDestinationId: string,
    destinationId: string,
  ) => void;
};

/** Encapsulate folder rendering and keep `MinimalMain` focused on orchestration. */
export function FoldersCard({
  busy,
  items,
  destinations,
  destinationReady,
  defaultKeep,
  watchedWarning,
  onAddFolder,
  onRemovePath,
  onUpdateKeep,
  onUpdateDestination,
}: Props) {
  const hasItems = items.length > 0;

  return (
    <section className="card folders-card" aria-labelledby="folders-card-title">
      <div className="section-title paths-head">
        <div className="section-identity">
          <span className="section-step-badge" aria-hidden="true">
            02
          </span>
          <div>
            <span className="section-eyebrow">Protection</span>
            <h2 className="section-heading" id="folders-card-title">
              What should stay protected?
            </h2>
          </div>
        </div>
        <div className="btn-ring">
          <Button
            className="add-path-btn"
            onClick={onAddFolder}
            disabled={busy || !destinationReady}
            aria-label="Add protected path"
          >
            Add path…
          </Button>
        </div>
      </div>
      <p className="section-subtitle section-subtitle--roomy">
        {destinationReady
          ? 'Add the folders and files that matter. Backup Sync stores only new or changed content.'
          : 'Choose a storage location first so protected content has somewhere safe to go.'}
      </p>
      {watchedWarning && (
        <InlineAlert kind="error" className="mt-3">
          {watchedWarning}
        </InlineAlert>
      )}

      {!hasItems && (
        <StateBlock
          tone="empty"
          title={
            destinationReady ? 'No paths protected yet' : 'Choose a destination before adding paths'
          }
          message={
            destinationReady
              ? 'Add the first folder or file, then Backup Sync will route it to every destination.'
              : 'The first destination defines where new protected paths will store backup history.'
          }
          className="mt-3 paths-empty-state"
          action={
            <Button
              tone="secondary"
              size="sm"
              onClick={onAddFolder}
              disabled={busy || !destinationReady}
              aria-label="Add protected path"
            >
              Add path…
            </Button>
          }
        />
      )}

      {hasItems && (
        <div className="stack folder-list">
          {items.map((w) => (
            <FolderRow
              key={`${w.kind}:${w.path}:${w.destination_id}`}
              busy={busy}
              path={w.path}
              kind={w.kind}
              destinationId={w.destination_id}
              destinations={destinations}
              keep={w.max_backups_per_file ?? defaultKeep}
              onRemove={() => onRemovePath(w.path, w.kind, w.destination_id)}
              onUpdateKeep={(keep) => onUpdateKeep(w.path, w.kind, w.destination_id, keep)}
              onUpdateDestination={(destinationId) =>
                onUpdateDestination(w.path, w.kind, w.destination_id, destinationId)
              }
            />
          ))}
        </div>
      )}
    </section>
  );
}

type RowProps = {
  busy: boolean;
  path: string;
  kind: 'File' | 'Directory';
  destinationId: string;
  destinations: DestinationOption[];
  keep: number;
  onRemove: () => void;
  onUpdateKeep: (keep: number) => void;
  onUpdateDestination: (destinationId: string) => void;
};

/** Keep folder list rendering small and reuse consistent markup. */
function FolderRow({
  busy,
  path,
  destinationId,
  destinations,
  keep,
  onRemove,
  onUpdateKeep,
  onUpdateDestination,
}: RowProps) {
  return (
    <div className="folder-row">
      <div className="pill" title={path}>
        <span className="pill-main truncate">{path}</span>
      </div>
      <div className="folder-row-controls">
        <div className="stack-sm">
          <span className="muted text-xs">Destination</span>
          <select
            value={destinationId}
            aria-label={`Destination for ${path}`}
            onChange={(event) => onUpdateDestination(event.target.value)}
            disabled={busy}
          >
            {destinations.map((destination) => (
              <option key={destination.id} value={destination.id}>
                {(destination.label && destination.label.trim().length > 0
                  ? destination.label.trim()
                  : destination.path) || destination.id}
              </option>
            ))}
          </select>
        </div>
        <div className="stack-sm">
          <span className="muted text-xs">Backups to keep</span>
          <div className="stepper" aria-label="Backups to keep">
            <button
              className="btn secondary btn-sm stepper-btn"
              type="button"
              onClick={() => onUpdateKeep(Math.max(0, keep - 1))}
              disabled={busy}
              aria-label="Decrease backups to keep"
            >
              -
            </button>
            <input
              className="stepper-input"
              type="number"
              min={0}
              max={1000}
              value={keep}
              aria-label={`Backups to keep for ${path}`}
              onChange={(e) => {
                const n = Number(e.target.value);
                if (!Number.isFinite(n)) return;
                onUpdateKeep(Math.max(0, Math.min(1000, Math.round(n))));
              }}
              disabled={busy}
            />
            <button
              className="btn secondary btn-sm stepper-btn"
              type="button"
              onClick={() => onUpdateKeep(Math.min(1000, keep + 1))}
              disabled={busy}
              aria-label="Increase backups to keep"
            >
              +
            </button>
          </div>
        </div>
        <Button
          tone="secondary"
          size="sm"
          onClick={onRemove}
          disabled={busy}
          aria-label={`Remove protected path ${path}`}
        >
          Remove
        </Button>
      </div>
    </div>
  );
}

/** Keep adapter logic centralized and testable. */
export function watchedToItem(w: WatchedPath): Item {
  return {
    path: w.path,
    kind: w.kind ?? 'Directory',
    destination_id: w.destination_id ?? 'default',
    max_backups_per_file: w.max_backups_per_file ?? null,
  };
}
