/**
 * Summary: Split a filesystem path into normalized display parts.
 *
 * Inputs: Path string.
 * Outputs: Array of non-empty path components.
 * Side effects: None.
 * Error handling: Returns a single-element array with the raw path on failure.
 * Ties to other methods: Used by `displayName` and `PathBreadcrumb`.
 * Why this exists: Keep path parsing consistent across watch list UI components.
 */
export function splitPathParts(path: string): string[] {
  try {
    return path.split(/[/\\]/).filter(Boolean);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    return [`[splitPathParts] Failed to parse path: ${reason}`];
  }
}

/**
 * Summary: Create a short display name for a filesystem path.
 *
 * Inputs: Full filesystem path.
 * Outputs: A short label for UI display.
 * Side effects: None.
 * Error handling: Returns the original path on failure.
 * Ties to other methods: Used by `WatchListItem` title rendering.
 * Why this exists: Keep the watch list compact and readable.
 */
export function displayName(path: string): string {
  const parts = splitPathParts(path);
  return parts[parts.length - 1] || path;
}

/**
 * Summary: Build a scoped ignore pattern that excludes a relative glob beneath a base path.
 *
 * Inputs: Base path and a relative pattern (glob fragment).
 * Outputs: A full ignore pattern string.
 * Side effects: None.
 * Error handling: Returns an empty string when inputs are blank.
 * Ties to other methods: Used by the watch list "Add exclusion" input flow.
 * Why this exists: Centralize ignore pattern formatting to avoid subtle duplication bugs.
 */
export function buildScopedExcludePattern(base: string, rel: string): string {
  const trimmedBase = (base || '').trim();
  const trimmedRel = (rel || '').trim();
  if (!trimmedBase || !trimmedRel) return '';
  return trimmedBase.endsWith('/')
    ? `${trimmedBase}**/${trimmedRel}`
    : `${trimmedBase}/**/${trimmedRel}`;
}

/**
 * Summary: Provide quick ignore patterns for common noise under a base path.
 *
 * Inputs: Base path for the watch entry.
 * Outputs: Array of ignore pattern strings.
 * Side effects: None.
 * Error handling: Returns an empty array on failure.
 * Ties to other methods: Used by watch list quick ignore buttons.
 * Why this exists: Reduce manual typing for common excludes.
 */
export function quickPatterns(base: string): string[] {
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
    console.warn(`[watchList::quickPatterns] Failed to build patterns for ${base}: ${reason}`);
    return [];
  }
}

/**
 * Summary: Derive a human-friendly button label for a known quick ignore pattern.
 *
 * Inputs: Ignore pattern string.
 * Outputs: Short label string.
 * Side effects: None.
 * Error handling: Never throws; returns a generic label when no match exists.
 * Ties to other methods: Used by quick ignore button rendering.
 * Why this exists: Keep button labeling logic consistent and centralized.
 */
export function quickPatternLabel(pattern: string): string {
  if (pattern.includes('node_modules')) return 'Ignore node_modules';
  if (pattern.includes('target')) return 'Ignore target';
  if (pattern.includes('build')) return 'Ignore build';
  if (pattern.includes('Cache')) return 'Ignore Cache';
  if (pattern.endsWith('*.log')) return 'Ignore *.log';
  return 'Ignore pattern';
}

