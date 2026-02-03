import React, { useState } from 'react';
import { WatchedPath } from './types';

type Props = {
  items: WatchedPath[];
  openPath: string | null;
  onToggleOpen: (path: string | null) => void;
  onToggleEnabled: (path: string) => void;
  onRemove: (path: string) => void;
  ignore_patterns: string[];
  onAddIgnore: (pattern: string) => void;
};

/**
 * Purpose: Create a short display name for a path.
 *
 * Inputs: Full filesystem path.
 * Outputs: Short label for UI display.
 * Ties to: Watch list item labels.
 * Side effects: None.
 * Why: Keep the watch list compact and readable.
 */
function displayName(path: string) {
  try {
    const parts = path.split(/[/\\]/).filter(Boolean);
    return parts[parts.length - 1] || path;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    return `[displayName] Failed to parse path: ${reason}`;
  }
}

/**
 * Purpose: Render a breadcrumb view for a path.
 *
 * Inputs: Path string.
 * Outputs: Breadcrumb element.
 * Ties to: Watch list item display.
 * Side effects: None.
 * Why: Show path context without full path overflow.
 */
function PathBreadcrumb({ path }: { path: string }) {
  try {
    const parts = path.split(/[/\\]/).filter(Boolean);
    return (
      <div className="path-breadcrumb">
        {parts.map((p, idx) => (
          <React.Fragment key={`${p}-${idx}`}>
            <span className="crumb">{p}</span>
            {idx < parts.length - 1 && <span className="crumb-sep">/</span>}
          </React.Fragment>
        ))}
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    return <div className="muted">[PathBreadcrumb] Failed to render path: {reason}</div>;
  }
}

/**
 * Purpose: Render the watch list with controls and ignore helpers.
 *
 * Inputs: Watched items, open path, handlers, and ignore patterns.
 * Outputs: A watch list element with actions.
 * Ties to: Settings watch list management.
 * Side effects: Registers React state hooks and event handlers for watch actions.
 * Why: Provide direct control over watched paths and ignores.
 */
const WatchList: React.FC<Props> = ({
  items,
  openPath,
  onToggleOpen,
  onToggleEnabled,
  onRemove,
  ignore_patterns,
  onAddIgnore,
}) => {
  const [excludes, setExcludes] = useState<Record<string, string>>({});

  /**
   * Purpose: Add an ignore pattern scoped to a watch path.
   *
   * Inputs: Base path for the ignore pattern.
   * Outputs: Updates ignore patterns and local state.
   * Ties to: Ignore entry input for a watched path.
   * Side effects: Updates component state and triggers ignore updates upstream.
   * Why: Allow targeted ignores without manual pattern typing.
   */
  const addExclude = (base: string) => {
    try {
      const rel = excludes[base] || '';
      if (!rel.trim()) return;
      const pattern = base.endsWith('/') ? `${base}**/${rel.trim()}` : `${base}/**/${rel.trim()}`;
      if (!ignore_patterns.includes(pattern)) {
        onAddIgnore(pattern);
      }
      setExcludes({ ...excludes, [base]: '' });
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      console.warn(`[WatchList::addExclude] Failed to add exclude for ${base}: ${reason}`);
    }
  };

  /**
   * Purpose: Provide quick ignore patterns for common noise.
   *
   * Inputs: Base path for the watch entry.
   * Outputs: Array of ignore pattern strings.
   * Ties to: Quick ignore buttons.
   * Side effects: None.
   * Why: Reduce manual typing for common ignores.
   */
  const quickPatterns = (base: string) => {
    try {
      return [
        `${base}/**/*.log`,
        `${base}/**/node_modules/**`,
        `${base}/**/build/**`,
        `${base}/**/target/**`,
        `${base}/**/Cache/**`,
      ];
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      console.warn(`[WatchList::quickPatterns] Failed to build patterns for ${base}: ${reason}`);
      return [];
    }
  };

  try {
    return (
      <div className="watch-list">
        {items.map((w) => {
          const isOpen = openPath === w.path;
          const existingForPath = (ignore_patterns || []).filter((p) =>
            p.startsWith(w.path.toString()),
          );
          return (
            <div key={w.path} className="watch-card">
              <div
                className="watch-card__header"
                onClick={() => onToggleOpen(isOpen ? null : w.path)}
              >
                <div className={`status-dot ${w.enabled ? '' : 'paused'}`} />
                <div className="watch-card__title">
                  <strong>{displayName(w.path)}</strong>
                  <PathBreadcrumb path={w.path} />
                </div>
                <span className="badge">{w.kind === 'Directory' ? 'Folder' : 'File'}</span>
                <span className="badge">{w.enabled ? 'Watching' : 'Paused'}</span>
                <button
                  className="icon-btn"
                  title="Remove from protection"
                  onClick={(e) => {
                    e.stopPropagation();
                    onRemove(w.path);
                  }}
                >
                  ×
                </button>
              </div>
              {isOpen && (
                <div className="watch-card__body">
                  <div className="muted">
                    Status:{' '}
                    {w.enabled
                      ? 'Backups will run on schedule'
                      : 'Not backing up until you toggle it on'}
                  </div>
                  <div className="inline-actions">
                    <button
                      className="btn secondary"
                      onClick={(e) => {
                        e.stopPropagation();
                        onToggleEnabled(w.path);
                      }}
                    >
                      {w.enabled ? 'Pause' : 'Enable'}
                    </button>
                    <button
                      className="btn secondary"
                      onClick={(e) => {
                        e.stopPropagation();
                        onRemove(w.path);
                      }}
                    >
                      Remove
                    </button>
                  </div>
                  <div>
                    <div className="muted">
                      Exclude something inside this{' '}
                      {w.kind === 'Directory' ? 'folder' : 'file parent'}:
                    </div>
                    <div className="inline-actions" style={{ marginTop: 6 }}>
                      <input
                        type="text"
                        placeholder="e.g. *.log or Cache"
                        value={excludes[w.path.toString()] || ''}
                        onChange={(e) =>
                          setExcludes({ ...excludes, [w.path.toString()]: e.target.value })
                        }
                      />
                      <button
                        className="btn secondary"
                        onClick={() => addExclude(w.path.toString())}
                      >
                        Add exclusion
                      </button>
                    </div>
                    <div className="inline-actions" style={{ marginTop: 4 }}>
                      {quickPatterns(w.path.toString()).map((p) => {
                        let label = 'Ignore pattern';
                        if (p.includes('node_modules')) label = 'Ignore node_modules';
                        else if (p.includes('target')) label = 'Ignore target';
                        else if (p.includes('build')) label = 'Ignore build';
                        else if (p.includes('Cache')) label = 'Ignore Cache';
                        else if (p.endsWith('*.log')) label = 'Ignore *.log';
                        return (
                          <button
                            key={p}
                            className="btn secondary"
                            onClick={() => {
                              if (!ignore_patterns.includes(p)) onAddIgnore(p);
                            }}
                          >
                            {label}
                          </button>
                        );
                      })}
                    </div>
                    {existingForPath.length > 0 && (
                      <div className="muted" style={{ marginTop: 4 }}>
                        Existing excludes: {existingForPath.join(', ')}
                      </div>
                    )}
                  </div>
                </div>
              )}
            </div>
          );
        })}
      </div>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[WatchList] Failed to render watch list: ${reason}`);
  }
};

export default WatchList;
