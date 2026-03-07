import { useCallback, useEffect, useMemo, useState } from 'react';
import { VersionFileInfoDto } from '../../../../services/types';
import { listVersionFiles } from '../../../../services/restore';

type EventKind = 'ok' | 'error' | 'info';

type Params = {
  isOpen: boolean;
  scope: 'folder' | 'files';
  sourcePath: string;
  versionId: string;
  onEvent: (msg: string, kind?: EventKind) => void;
};

type Result = {
  query: string;
  setQuery: (value: string) => void;
  fileResults: VersionFileInfoDto[] | null;
  fileTotal: number | null;
  selected: Record<string, boolean>;
  fileLoading: boolean;
  selectedCount: number;
  setSelectedValue: (relPath: string, checked: boolean) => void;
  runSearch: () => Promise<void>;
};

/**
 * Summary: Manage restore file search state and file-level selections.
 *
 * Inputs: Modal open state, scope, selected restore source/version, and event callback.
 *
 * Outputs: Search state, selection state, and a search trigger.
 *
 * Side effects: Calls the restore-file listing IPC endpoint and updates React state.
 *
 * Error handling: Emits contextual search failures through `onEvent`.
 *
 * Ties to other methods: Used by `RestoreModal` and `RestoreModalBody`.
 *
 * Why this exists: Keep file-search behavior isolated from the modal shell and restore execution.
 */
export function useRestoreSearch({
  isOpen,
  scope,
  sourcePath,
  versionId,
  onEvent,
}: Params): Result {
  const [query, setQuery] = useState<string>('');
  const [fileResults, setFileResults] = useState<VersionFileInfoDto[] | null>(null);
  const [fileTotal, setFileTotal] = useState<number | null>(null);
  const [selected, setSelected] = useState<Record<string, boolean>>({});
  const [fileLoading, setFileLoading] = useState(false);

  useEffect(() => {
    if (!isOpen) {
      return;
    }
    setQuery('');
    setFileResults(null);
    setFileTotal(null);
    setSelected({});
  }, [isOpen]);

  /**
   * Summary: Search the selected restore version for matching files.
   *
   * Inputs: None.
   *
   * Outputs: Updates the file search results and total count.
   *
   * Side effects: Calls the restore-file listing IPC endpoint and updates React state.
   *
   * Error handling: Emits contextual failures through `onEvent`.
   *
   * Ties to other methods: Triggered by the search button, Enter key, and scope-selection effect.
   *
   * Why this exists: File-level restore needs an explicit queryable index of version contents.
   */
  const runSearch = useCallback(async () => {
    try {
      if (!sourcePath || !versionId) {
        return;
      }
      setFileLoading(true);
      const response = await listVersionFiles({
        source_path: sourcePath,
        version_id: versionId,
        query: query.trim() || null,
        limit: 200,
      });
      setFileResults(response.files);
      setFileTotal(response.total_files);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      onEvent(`[Restore] Failed to search files: ${reason}`, 'error');
    } finally {
      setFileLoading(false);
    }
  }, [onEvent, query, sourcePath, versionId]);

  useEffect(() => {
    if (!isOpen || scope !== 'files') {
      return;
    }
    void runSearch();
  }, [isOpen, runSearch, scope, sourcePath, versionId]);

  /**
   * Summary: Toggle whether an individual restore result is selected.
   *
   * Inputs: Relative path and next checked state.
   *
   * Outputs: Updates the file-selection map.
   *
   * Side effects: Updates React state.
   *
   * Error handling: None.
   *
   * Ties to other methods: Used by `RestoreModalBody` checkbox rows and `useRestoreExecution`.
   *
   * Why this exists: File-level restore needs stable, path-keyed selection state.
   */
  const setSelectedValue = useCallback((relPath: string, checked: boolean) => {
    setSelected((previous) => ({
      ...previous,
      [relPath]: checked,
    }));
  }, []);

  const selectedCount = useMemo(() => Object.values(selected).filter(Boolean).length, [selected]);

  return {
    query,
    setQuery,
    fileResults,
    fileTotal,
    selected,
    fileLoading,
    selectedCount,
    setSelectedValue,
    runSearch,
  };
}
