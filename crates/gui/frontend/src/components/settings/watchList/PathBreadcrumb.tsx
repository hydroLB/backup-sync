import React from 'react';
import { splitPathParts } from './utils';

type Props = {
  path: string;
};

/**
 * Summary: Render a breadcrumb view for a path.
 *
 * Inputs: Path string.
 * Outputs: Breadcrumb element.
 * Side effects: None.
 * Error handling: Renders a muted error message when parsing fails.
 * Ties to other methods: Used by `WatchListItem` to show context without full-path overflow.
 * Why this exists: Provide consistent path context in a compact UI.
 */
export function PathBreadcrumb({ path }: Props) {
  try {
    const parts = splitPathParts(path);
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

