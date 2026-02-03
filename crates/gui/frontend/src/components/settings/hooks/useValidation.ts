import { Config, Destination } from "../types";
import { buildPathIndex, bstAnyPrefix } from "../../../utils/bst";

/**
 * Purpose: Provide validation helpers for settings state.
 *
 * Inputs: None.
 * Outputs: Validation functions for config checks.
 * Ties to: Settings persistence and onboarding gating.
 * Side effects: None.
 * Why: Ensure config data remains valid before save.
 */
export function useValidation() {
  /**
   * Purpose: Validate a candidate config and ignore list.
   *
   * Inputs: Candidate config and ignore pattern list.
   * Outputs: Validation error message or null.
   * Ties to: Settings save flow and UI validation banners.
   * Side effects: None.
   * Why: Prevent invalid config states from being persisted.
   */
  const validate = (next: Config, ignores: string[]) => {
    try {
      const destinations: Destination[] =
        next.destinations && next.destinations.length > 0
          ? next.destinations
          : [{ id: "default", path: next.backup_root, label: "Primary", max_backups_per_file: null }];
      if (!destinations.length) return "Add at least one backup destination.";
      const primary = destinations[0]?.path ?? "";
      if (!next.watched || next.watched.length === 0) return "Add at least one folder or file to back up.";
      if (next.watched.length > 500) return "Too many watched entries; trim to 500 or fewer.";
      if (!primary || primary.trim().length === 0) return "Pick a backup destination.";

      const index = buildPathIndex(next.watched || []);
      for (const w of next.watched) {
        if (!w.destination_id || !destinations.find((d) => d.id === w.destination_id))
          return "Each item must target a valid destination.";
        if (primary.startsWith(w.path)) return "Backup destination cannot be inside a watched path.";
        if (w.path.startsWith(primary)) return "Watched path is inside backup destination; choose a different destination.";
        if (!w.path || w.path.trim().length === 0) return "Watched paths cannot be empty.";
        const parentParts = w.path.split(/[\\/]/).filter(Boolean);
        if (parentParts.length === 0) return "Watched path cannot be the filesystem root.";
        const conflict = bstAnyPrefix(index, w.path);
        if (conflict && conflict.path !== w.path) return "A parent/child path is already protected. Remove duplicates.";
      }
      if (next.interval_seconds < 5) return "Backup interval must be at least 5 seconds.";
      if (next.max_backups_per_file < 1) return "Max backups per file must be at least 1.";
      if (next.max_parallel_copies < 1) return "Max parallel copies must be at least 1.";
      if (ignores.length > 200) return "Too many ignore patterns; trim to 200 or fewer.";
      for (const pat of ignores) {
        if (!pat.trim()) continue;
        try {
          new RegExp(pat.replace(/\*\*/g, ".*").replace(/\*/g, "[^/]*"));
        } catch (_) {
          return `Invalid ignore pattern: ${pat}`;
        }
      }
      return null;
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      return `[useValidation::validate] Validation failed: ${reason}`;
    }
  };

  return { validate };
}
