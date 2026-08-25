import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
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

/** Keep file-search behavior isolated from the modal shell and restore execution. */
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
  const requestIdRef = useRef(0);

  useEffect(() => {
    requestIdRef.current += 1;
    setFileLoading(false);
    setFileResults(null);
    setFileTotal(null);
    setSelected({});
    if (!isOpen) setQuery('');
  }, [isOpen, scope, sourcePath, versionId]);

  /** File-level restore needs an explicit queryable index of version contents. */
  const runSearch = useCallback(async () => {
    const requestId = requestIdRef.current + 1;
    requestIdRef.current = requestId;
    try {
      if (!sourcePath || !versionId) {
        setFileResults(null);
        setFileTotal(null);
        setSelected({});
        return;
      }
      setFileLoading(true);
      const response = await listVersionFiles({
        source_path: sourcePath,
        version_id: versionId,
        query: query.trim() || null,
        limit: 200,
      });
      if (requestIdRef.current !== requestId) return;
      setFileResults(response.files);
      setFileTotal(response.total_files);
    } catch (error) {
      if (requestIdRef.current !== requestId) return;
      const reason = error instanceof Error ? error.message : String(error);
      onEvent(`[Restore] Failed to search files: ${reason}`, 'error');
    } finally {
      if (requestIdRef.current === requestId) setFileLoading(false);
    }
  }, [onEvent, query, sourcePath, versionId]);

  /** File-level restore needs stable, path-keyed selection state. */
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
