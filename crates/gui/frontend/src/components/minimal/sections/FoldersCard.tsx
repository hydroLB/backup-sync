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

/**
 * Summary: Render the watched folders section for minimal mode.
 *
 * Inputs: Folder list, busy flag, defaults, and action handlers.
 *
 * Outputs: Card React element tree with folder rows.
 *
 * Side effects: Calls provided handlers for add/remove/update actions.
 *
 * Error handling: Delegated to parent via handlers.
 *
 * Ties to other methods: Used by `MinimalMain` watch management flow.
 *
 * Why this exists: Encapsulate folder rendering and keep `MinimalMain` focused on orchestration.
 */
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
    <div className="card folders-card">
      <div className="section-title paths-head">
        <div>
          <h2 className="section-heading">Protected Paths</h2>
          <p className="section-subtitle">
            {destinationReady
              ? 'Each protected path is assigned to every configured destination by default.'
              : 'Choose a destination first so new protected paths have somewhere to write.'}
          </p>
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
    </div>
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

/**
 * Summary: Render a single watched folder row with retention stepper.
 *
 * Inputs: Path, current keep value, busy state, and handlers.
 *
 * Outputs: A row element suitable for the folder list.
 *
 * Side effects: Calls the provided handlers for update/remove operations.
 *
 * Error handling: Ignores invalid numeric input to avoid throwing during typing.
 *
 * Ties to other methods: Used by `FoldersCard` and ultimately persisted by `MinimalMain`.
 *
 * Why this exists: Keep folder list rendering small and reuse consistent markup.
 */
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

/**
 * Summary: Convert a watched entry to the minimal list item shape.
 *
 * Inputs: A watched entry.
 *
 * Outputs: Item containing the path and keep override.
 *
 * Side effects: None.
 *
 * Error handling: None.
 *
 * Ties to other methods: Used by callers that want to reuse `FoldersCard` with config data.
 *
 * Why this exists: Keep adapter logic centralized and testable.
 */
export function watchedToItem(w: WatchedPath): Item {
  return {
    path: w.path,
    kind: w.kind ?? 'Directory',
    destination_id: w.destination_id ?? 'default',
    max_backups_per_file: w.max_backups_per_file ?? null,
  };
}
