import { WatchedPath } from '../types';
import { PathBreadcrumb } from './PathBreadcrumb';
import { buildScopedExcludePattern, displayName, quickPatternLabel, quickPatterns } from './utils';

type Props = {
  item: WatchedPath;
  isOpen: boolean;
  excludeDraft: string;
  ignorePatterns: string[];
  onToggleOpen: (path: string | null) => void;
  onToggleEnabled: (path: string) => void;
  onRemove: (path: string) => void;
  onExcludeDraftChange: (path: string, value: string) => void;
  onAddIgnore: (pattern: string) => void;
  onClearExcludeDraft: (path: string) => void;
};

/**
 * Summary: Render a single watch list item card with enable/remove and ignore helpers.
 *
 * Inputs: Watched item, open state, draft values, ignore list, and handlers.
 * Outputs: A watch-card element tree.
 * Side effects: Calls provided handlers on user interaction.
 * Error handling: Emits console warnings for unexpected pattern build failures.
 * Ties to other methods: Used by `WatchList` to render each watched entry.
 * Why this exists: Keep list rendering readable by isolating row behavior into a dedicated component.
 */
export function WatchListItem({
  item,
  isOpen,
  excludeDraft,
  ignorePatterns,
  onToggleOpen,
  onToggleEnabled,
  onRemove,
  onExcludeDraftChange,
  onAddIgnore,
  onClearExcludeDraft,
}: Props) {
  const base = item.path.toString();
  const existingForPath = (ignorePatterns || []).filter((p) => p.startsWith(base));

  const addExclude = () => {
    try {
      const pattern = buildScopedExcludePattern(base, excludeDraft);
      if (!pattern) return;
      if (!ignorePatterns.includes(pattern)) onAddIgnore(pattern);
      onClearExcludeDraft(base);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      console.warn(`[WatchListItem::addExclude] Failed to add exclude for ${base}: ${reason}`);
    }
  };

  return (
    <div className="watch-card">
      <div className="watch-card__header" onClick={() => onToggleOpen(isOpen ? null : base)}>
        <div className={`status-dot ${item.enabled ? '' : 'paused'}`} />
        <div className="watch-card__title">
          <strong>{displayName(base)}</strong>
          <PathBreadcrumb path={base} />
        </div>
        <span className="badge">{item.kind === 'Directory' ? 'Folder' : 'File'}</span>
        <span className="badge">{item.enabled ? 'Watching' : 'Paused'}</span>
        <button
          className="icon-btn"
          title="Remove from protection"
          onClick={(e) => {
            e.stopPropagation();
            onRemove(base);
          }}
        >
          ×
        </button>
      </div>

      {isOpen && (
        <div className="watch-card__body">
          <div className="muted">
            Status:{' '}
            {item.enabled
              ? 'Backups will run on schedule'
              : 'Not backing up until you toggle it on'}
          </div>
          <div className="inline-actions">
            <button
              className="btn secondary"
              onClick={(e) => {
                e.stopPropagation();
                onToggleEnabled(base);
              }}
            >
              {item.enabled ? 'Pause' : 'Enable'}
            </button>
            <button
              className="btn secondary"
              onClick={(e) => {
                e.stopPropagation();
                onRemove(base);
              }}
            >
              Remove
            </button>
          </div>
          <div>
            <div className="muted">
              Exclude something inside this {item.kind === 'Directory' ? 'folder' : 'file parent'}:
            </div>
            <div className="inline-actions mt-2">
              <input
                type="text"
                placeholder="e.g. *.log or Cache"
                value={excludeDraft}
                onChange={(e) => onExcludeDraftChange(base, e.target.value)}
              />
              <button className="btn secondary" onClick={addExclude}>
                Add exclusion
              </button>
            </div>
            <div className="inline-actions mt-1">
              {quickPatterns(base).map((p) => (
                <button
                  key={p}
                  className="btn secondary"
                  onClick={() => {
                    if (!ignorePatterns.includes(p)) onAddIgnore(p);
                  }}
                >
                  {quickPatternLabel(p)}
                </button>
              ))}
            </div>
            {existingForPath.length > 0 && (
              <div className="muted mt-1">Existing excludes: {existingForPath.join(', ')}</div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
