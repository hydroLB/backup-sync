import { useEffect, useState } from 'react';
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

type Props = {
  busy: boolean;
  items: Item[];
  defaultKeep: number;
  intervalSeconds: number;
  watchedWarning: string | null;
  onAddFolder: () => Promise<void>;
  onChangePath: (path: string, kind: 'File' | 'Directory') => Promise<void>;
  onRemovePath: (
    path: string,
    kind: 'File' | 'Directory',
    sourceDestinationId: string,
  ) => Promise<void>;
  onUpdateKeep: (
    path: string,
    kind: 'File' | 'Directory',
    sourceDestinationId: string,
    keep: number,
  ) => Promise<void>;
};

/** Put the user's protected content first and show each source only once. */
export function FoldersCard({
  busy,
  items,
  defaultKeep,
  intervalSeconds,
  watchedWarning,
  onAddFolder,
  onChangePath,
  onRemovePath,
  onUpdateKeep,
}: Props) {
  return (
    <section className="card folders-card" aria-labelledby="folders-card-title">
      <div className="section-title paths-head">
        <div className="section-identity">
          <span className="section-step-badge" aria-hidden="true">
            01
          </span>
          <div>
            <span className="section-eyebrow">Protection</span>
            <h2 className="section-heading" id="folders-card-title">
              Protected items
            </h2>
            <p className="section-summary">Folders and files included in every backup</p>
          </div>
        </div>
        <Button
          size="sm"
          className="add-path-btn"
          onClick={() => void onAddFolder()}
          disabled={busy}
          aria-label="Add protected path"
        >
          <span aria-hidden="true">+</span> Add item
        </Button>
      </div>

      {watchedWarning && (
        <InlineAlert kind="error" className="mt-3">
          {watchedWarning}
        </InlineAlert>
      )}

      {items.length === 0 ? (
        <StateBlock
          tone="empty"
          title="Nothing is protected yet"
          message="Add a folder or file now. Choose where its copies live in step 2."
          className="compact-empty-state"
          action={
            <Button size="sm" onClick={() => void onAddFolder()} disabled={busy}>
              Add your first folder
            </Button>
          }
        />
      ) : (
        <div className="folder-list">
          {items.map((item) => (
            <FolderRow
              key={`${item.kind}:${item.path}`}
              busy={busy}
              path={item.path}
              kind={item.kind}
              keep={item.max_backups_per_file ?? defaultKeep}
              intervalSeconds={intervalSeconds}
              onChange={() => onChangePath(item.path, item.kind)}
              onRemove={() => onRemovePath(item.path, item.kind, item.destination_id)}
              onUpdateKeep={(keep) => onUpdateKeep(item.path, item.kind, item.destination_id, keep)}
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
  keep: number;
  intervalSeconds: number;
  onChange: () => Promise<void>;
  onRemove: () => Promise<void>;
  onUpdateKeep: (keep: number) => Promise<void>;
};

function formatCheckCadence(seconds: number, abbreviated = false): string {
  if (seconds % 3600 === 0) {
    const hours = seconds / 3600;
    return `${hours} ${abbreviated ? 'hr' : `hour${hours === 1 ? '' : 's'}`}`;
  }
  if (seconds % 60 === 0) {
    const minutes = seconds / 60;
    return `${minutes} ${abbreviated ? 'min' : `minute${minutes === 1 ? '' : 's'}`}`;
  }
  return `${seconds} ${abbreviated ? 'sec' : `second${seconds === 1 ? '' : 's'}`}`;
}

function FolderRow({
  busy,
  path,
  kind,
  keep,
  intervalSeconds,
  onChange,
  onRemove,
  onUpdateKeep,
}: RowProps) {
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [keepBusy, setKeepBusy] = useState(false);
  const [displayKeep, setDisplayKeep] = useState(keep);

  useEffect(() => {
    setDisplayKeep(keep);
  }, [keep]);

  const saveKeep = async (nextKeep: number) => {
    if (keepBusy || nextKeep === displayKeep) return;
    setDisplayKeep(nextKeep);
    setKeepBusy(true);
    try {
      await onUpdateKeep(nextKeep);
    } finally {
      setKeepBusy(false);
    }
  };

  return (
    <div className="folder-row">
      {confirmRemove ? (
        <div className="inline-confirm" role="alert">
          <span>Stop protecting this {kind.toLowerCase()}?</span>
          <div>
            <Button
              tone="secondary"
              size="sm"
              onClick={() => setConfirmRemove(false)}
              disabled={busy}
            >
              Cancel
            </Button>
            <Button tone="danger" size="sm" onClick={() => void onRemove()} disabled={busy}>
              Remove
            </Button>
          </div>
        </div>
      ) : (
        <div className="folder-row__source">
          <button
            className="folder-row__main"
            type="button"
            onClick={() => void onChange()}
            disabled={busy}
            aria-label={`Change protected ${kind.toLowerCase()} ${path}`}
          >
            <span className="folder-kind">{kind === 'Directory' ? 'Folder' : 'File'}</span>
            <span className="folder-path truncate" title={path}>
              {path}
            </span>
          </button>
          <div className="folder-row__keep">
            <div className="folder-row__keep-copy" aria-live="polite">
              <span className="folder-row__keep-label">Versions kept</span>
              <span className="folder-row__keep-detail">
                Keeps {displayKeep} previous version{displayKeep === 1 ? '' : 's'} for recovery
                <span aria-hidden="true"> · </span>
                <span>Checks every {formatCheckCadence(intervalSeconds, true)}</span>
              </span>
            </div>
            <div className="stepper" aria-label={`Versions to keep for ${path}`}>
              <button
                className="btn secondary btn-sm stepper-btn"
                type="button"
                onClick={() => void saveKeep(Math.max(1, displayKeep - 1))}
                disabled={busy || keepBusy || displayKeep <= 1}
                aria-label={`Keep fewer versions for ${path}`}
              >
                −
              </button>
              <input
                className="stepper-input"
                type="number"
                min={1}
                max={1000}
                value={displayKeep}
                aria-label={`Versions to keep for ${path}`}
                onChange={(event) => {
                  const value = Number(event.target.value);
                  if (!Number.isFinite(value)) return;
                  void saveKeep(Math.max(1, Math.min(1000, Math.round(value))));
                }}
                disabled={busy || keepBusy}
              />
              <button
                className="btn secondary btn-sm stepper-btn"
                type="button"
                onClick={() => void saveKeep(Math.min(1000, displayKeep + 1))}
                disabled={busy || keepBusy}
                aria-label={`Keep more versions for ${path}`}
              >
                +
              </button>
            </div>
          </div>
          <button
            className="icon-remove"
            type="button"
            onClick={() => setConfirmRemove(true)}
            disabled={busy}
            aria-label={`Remove protected path ${path}`}
            title="Remove from protection"
          >
            ×
          </button>
        </div>
      )}
    </div>
  );
}

/** Keep adapter logic centralized and testable. */
export function watchedToItem(watched: WatchedPath): Item {
  return {
    path: watched.path,
    kind: watched.kind ?? 'Directory',
    destination_id: watched.destination_id ?? 'default',
    max_backups_per_file: watched.max_backups_per_file ?? null,
  };
}
