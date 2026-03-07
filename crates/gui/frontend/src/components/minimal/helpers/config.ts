import { Config, Destination, WatchedPath } from '../../../domain/config';
import { DEFAULT_AUTOMATIC_INTERVAL_MINUTES } from './interval';

/**
 * Summary: Ensure a primary destination exists for minimal mode rendering.
 *
 * Inputs: A `Config` value possibly missing destination entries.
 *
 * Outputs: A normalized config and the primary destination entry.
 *
 * Side effects: None.
 *
 * Error handling: None; creates a default destination when missing.
 *
 * Ties to other methods: Used by `MinimalMain` load normalization and destination persistence.
 *
 * Why this exists: Minimal mode assumes a single destination for scheduling and watch defaults.
 */
export function ensurePrimaryDestination(cfg: Config): { cfg: Config; dest: Destination } {
  if (cfg.destinations.length > 0) {
    return { cfg, dest: cfg.destinations[0]! };
  }
  const dest: Destination = { id: 'default', path: cfg.backup_root, label: 'Primary' };
  return { cfg: { ...cfg, destinations: [dest] }, dest };
}

/**
 * Summary: Normalize a watched entry for minimal mode assumptions.
 *
 * Inputs: Watched entry, destination id, and a keep-default value.
 *
 * Outputs: A fully populated watched entry with safe defaults.
 *
 * Side effects: None.
 *
 * Error handling: None; fills missing fields with defaults.
 *
 * Ties to other methods: Used by folder list rendering and add-folder creation.
 *
 * Why this exists: Keep minimal UI stable across older config shapes and partial values.
 */
export function normalizeWatched(w: WatchedPath, destId: string, keepDefault: number): WatchedPath {
  const kind = w.kind ?? 'Directory';
  return {
    ...w,
    kind,
    enabled: w.enabled ?? true,
    destination_id: w.destination_id ?? destId,
    max_backups_per_file: w.max_backups_per_file ?? keepDefault,
  };
}

/**
 * Summary: Enforce the fixed minimal-mode automatic backup interval.
 *
 * Inputs: A `Config` value to normalize.
 *
 * Outputs: Config with `interval_seconds` pinned to the minimal-mode cadence.
 *
 * Side effects: None.
 *
 * Error handling: None; applies a deterministic default when the config is missing or invalid.
 *
 * Ties to other methods: Used by `useMinimalConfig` load and persist flows.
 *
 * Why this exists: The minimal UI intentionally removes schedule tuning, so the cadence must be enforced centrally.
 */
export function enforceFixedAutomaticInterval(cfg: Config): Config {
  const nextSeconds = DEFAULT_AUTOMATIC_INTERVAL_MINUTES * 60;
  if (cfg.interval_seconds === nextSeconds) return cfg;
  return { ...cfg, interval_seconds: nextSeconds };
}
