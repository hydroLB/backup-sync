import { useCallback, useEffect, useMemo, useState } from 'react';
import { FolderVersionsDto } from '../../../../services/types';
import { listVersions } from '../../../../services/restore';

type EventKind = 'ok' | 'error' | 'info';

type Params = {
  isOpen: boolean;
  onEvent: (msg: string, kind?: EventKind) => void;
};

type Result = {
  folders: FolderVersionsDto[] | null;
  loading: boolean;
  loadError: string | null;
  sourcePath: string;
  setSourcePath: (value: string) => void;
  versionId: string;
  setVersionId: (value: string) => void;
  selectedVersions: FolderVersionsDto['versions'];
  hasFolders: boolean;
  singleVersionOnly: boolean;
  reload: () => void;
};

/** Keep version-catalog loading and selection synchronization out of the modal shell. */
export function useRestoreCatalog({ isOpen, onEvent }: Params): Result {
  const [folders, setFolders] = useState<FolderVersionsDto[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const [sourcePath, setSourcePath] = useState<string>('');
  const [versionId, setVersionId] = useState<string>('');

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    let cancelled = false;
    setLoading(true);
    setLoadError(null);

    void listVersions()
      .then((versions) => {
        if (cancelled) {
          return;
        }
        setFolders(versions);
        setSourcePath(versions[0]?.source_path ?? '');
        setVersionId(versions[0]?.versions?.[0]?.id ?? '');
      })
      .catch((error) => {
        if (cancelled) {
          return;
        }
        const reason = error instanceof Error ? error.message : String(error);
        setLoadError(reason);
        onEvent(`[Restore] Failed to list versions: ${reason}`, 'error');
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [isOpen, onEvent, reloadToken]);

  const selectedVersions = useMemo(() => {
    const folder = folders?.find((entry) => entry.source_path === sourcePath);
    return folder?.versions ?? [];
  }, [folders, sourcePath]);

  useEffect(() => {
    if (!isOpen) {
      return;
    }
    if (selectedVersions.length === 0) {
      setVersionId('');
      return;
    }
    if (!selectedVersions.some((version) => version.id === versionId)) {
      setVersionId(selectedVersions[0]!.id);
    }
  }, [isOpen, selectedVersions, versionId]);

  /** Keep retry semantics explicit and easy to pass into the view. */
  const reload = useCallback(() => {
    setReloadToken((previous) => previous + 1);
  }, []);

  const hasFolders = (folders ?? []).length > 0;
  const singleVersionOnly = selectedVersions.length === 1;

  return {
    folders,
    loading,
    loadError,
    sourcePath,
    setSourcePath,
    versionId,
    setVersionId,
    selectedVersions,
    hasFolders,
    singleVersionOnly,
    reload,
  };
}
