import { Config, Destination, WatchedPath } from '../../../domain/config';

/** Minimal mode assumes a single destination for scheduling and watch defaults. */
export function ensurePrimaryDestination(cfg: Config): { cfg: Config; dest: Destination } {
  if (cfg.destinations.length > 0) {
    return { cfg, dest: cfg.destinations[0]! };
  }
  const dest: Destination = { id: 'default', path: cfg.backup_root, label: 'Primary' };
  return { cfg: { ...cfg, destinations: [dest] }, dest };
}

/** Keep minimal UI stable across older config shapes and partial values. */
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
