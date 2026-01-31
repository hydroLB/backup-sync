import { Config } from "./types";
import { useState } from "react";
import { buildPathIndex, bstAnyPrefix } from "../../utils/bst";

type PersistFn = (next: Config, message?: string) => void;

/**
 * Purpose: Manage watch list mutations for settings workflows.
 *
 * Inputs: Current config, persistence handler, and UI callbacks.
 * Outputs: Watch list action helpers and active state.
 * Ties to: Settings panel watch list controls.
 * Side effects: Updates React state, persists config, and emits status messages.
 * Why: Centralize watch list mutations with validation.
 */
export function useWatchActions(cfg: Config, persist: PersistFn, setStatus: (s: string) => void, setOpenPath: (p: string | null) => void) {
  const [active, setActive] = useState<string | null>(null);

  /**
   * Purpose: Add a watched path with overlap validation.
   *
   * Inputs: Path, kind, destination id, and retention override.
   * Outputs: Updates config and UI status.
   * Ties to: Watch list add actions.
   * Side effects: Updates React state, persists config, and emits status messages.
   * Why: Prevent duplicate or overlapping watch entries.
   */
  const addWatchedPath = (
    path: string,
    kind: "File" | "Directory",
    destination_id?: string,
    max_backups_per_file?: number | null
  ) => {
    try {
      const existingIndex = buildPathIndex(cfg.watched || []);
      if (bstAnyPrefix(existingIndex, path) || (cfg.watched || []).some((w) => w.path === path)) {
        setStatus("Already watching that path or a parent/child path.");
        return;
      }
      const next = [
        ...(cfg.watched || []),
        { path, kind, enabled: true, destination_id: destination_id || "default", max_backups_per_file },
      ];
      persist({ ...cfg, watched: next }, "Added");
      if (!active) setActive(path);
      setOpenPath(path);
    } catch (e) {
      setStatus(`[useWatchActions::addWatchedPath] Failed to add watched path: ${e}`);
    }
  };

  /**
   * Purpose: Toggle a watched path enabled state.
   *
   * Inputs: Path to toggle.
   * Outputs: Updates config and UI status.
   * Ties to: Watch list toggles.
   * Side effects: Persists config changes and emits status messages.
   * Why: Allow quick enable or disable of watch entries.
   */
  const toggleEnabled = (path: string) => {
    try {
      const next = (cfg.watched || []).map((w) =>
        w.path === path ? { ...w, enabled: !w.enabled } : w
      );
      persist({ ...cfg, watched: next }, "Updated");
    } catch (e) {
      setStatus(`[useWatchActions::toggleEnabled] Failed to toggle watched path: ${e}`);
    }
  };

  /**
   * Purpose: Remove a watched path from the config.
   *
   * Inputs: Path to remove.
   * Outputs: Updates config and UI status.
   * Ties to: Watch list delete actions.
   * Side effects: Persists config changes and emits status messages.
   * Why: Allow operators to prune the watch list safely.
   */
  const remove = (path: string) => {
    try {
      const next = (cfg.watched || []).filter((w) => w.path !== path);
      if (next.length === 0) {
        setStatus("Add at least one folder or file to back up.");
      }
      persist({ ...cfg, watched: next }, "Removed");
    } catch (e) {
      setStatus(`[useWatchActions::remove] Failed to remove watched path: ${e}`);
    }
  };

  return { addWatchedPath, toggleEnabled, remove, activePath: active, setActivePath: setActive };
}
