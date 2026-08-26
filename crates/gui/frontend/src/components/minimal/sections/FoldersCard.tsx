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
  updated: boolean;
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
  onUpdateInterval: (minutes: number) => Promise<void>;
};

/** Put the user's protected content first and show each source only once. */
export function FoldersCard({
  busy,
  items,
  defaultKeep,
  intervalSeconds,
  updated,
  watchedWarning,
  onAddFolder,
  onChangePath,
  onRemovePath,
  onUpdateKeep,
  onUpdateInterval,
}: Props) {
  return (
    <section className="card folders-card" aria-labelledby="folders-card-title">
      {updated && (
        <span className="section-update-indicator" role="status">
          <span aria-hidden="true">✓</span> Updated
        </span>
      )}
      <div className="section-title paths-head">
        <div className="section-identity">
          <span className="section-step-badge" aria-hidden="true">
            01
          </span>
          <div>
            <span className="section-eyebrow">Protection</span>
            <div className="section-heading-row">
              <h2 className="section-heading" id="folders-card-title">
                Protected items
              </h2>
              <span
                className="section-count"
                aria-label={`${items.length} protected ${items.length === 1 ? 'item' : 'items'} total`}
              >
                {items.length} total
              </span>
            </div>
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
              onUpdateInterval={onUpdateInterval}
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
  onUpdateInterval: (minutes: number) => Promise<void>;
};

function FolderRow({
  busy,
  path,
  kind,
  keep,
  intervalSeconds,
  onChange,
  onRemove,
  onUpdateKeep,
  onUpdateInterval,
}: RowProps) {
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [keepBusy, setKeepBusy] = useState(false);
  const [intervalBusy, setIntervalBusy] = useState(false);
  const [displayKeep, setDisplayKeep] = useState(keep);
  const intervalMinutes = Math.max(1, Math.round(intervalSeconds / 60));
  const [intervalDraft, setIntervalDraft] = useState(String(intervalMinutes));

  useEffect(() => {
    setDisplayKeep(keep);
  }, [keep]);

  useEffect(() => {
    setIntervalDraft(String(intervalMinutes));
  }, [intervalMinutes]);

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

  const saveInterval = async () => {
    const parsed = Number(intervalDraft);
    if (!Number.isFinite(parsed)) {
      setIntervalDraft(String(intervalMinutes));
      return;
    }
    const nextMinutes = Math.max(1, Math.min(43_200, Math.round(parsed)));
    setIntervalDraft(String(nextMinutes));
    if (intervalBusy || nextMinutes === intervalMinutes) return;
    setIntervalBusy(true);
    try {
      await onUpdateInterval(nextMinutes);
    } finally {
      setIntervalBusy(false);
    }
  };

  return (
    <div className="folder-row">
      {confirmRemove ? (
        <div className="inline-confirm" role="alert">
          <div className="inline-confirm__copy">
            <strong>
              Remove this {kind === 'Directory' ? 'folder' : 'file'} from Backup Sync?
            </strong>
            <span>
              All of its saved versions will be deleted. This cannot be undone. Your original files
              will not be deleted.
            </span>
          </div>
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
            <span className="folder-row__keep-label">Backup settings</span>
            <div className="folder-row__keep-detail">
              <div className="folder-row__retention">
                <span>Keep</span>
                <div
                  className="stepper"
                  role="group"
                  aria-label={`Version retention controls for ${path}`}
                >
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
                <span>previous version{displayKeep === 1 ? '' : 's'}</span>
              </div>
              <span className="folder-row__settings-divider" aria-hidden="true" />
              <label className="folder-row__cadence">
                <span>Check every</span>
                <input
                  className="interval-minute-input"
                  type="number"
                  min={1}
                  max={43_200}
                  inputMode="numeric"
                  value={intervalDraft}
                  aria-label="Backup interval minutes"
                  onFocus={(event) => event.currentTarget.select()}
                  onChange={(event) => setIntervalDraft(event.target.value)}
                  onBlur={() => void saveInterval()}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter') event.currentTarget.blur();
                    if (event.key === 'Escape') {
                      setIntervalDraft(String(intervalMinutes));
                      event.currentTarget.blur();
                    }
                  }}
                  disabled={busy || intervalBusy}
                />
                <span>min</span>
              </label>
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
            <span className="icon-remove__glyph" aria-hidden="true">
              ×
            </span>
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
