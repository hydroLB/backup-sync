import { useEffect, useMemo, useState } from 'react';
import { FolderVersionsDto, RestoreModeDto, VersionFileInfoDto } from '../../services/types';
import {
  listVersionFiles,
  listVersions,
  restoreFiles,
  restoreVersion,
} from '../../services/restore';
import { openDialog } from '../../services/dialog';
import { Button } from '../ui/Button';
import { FormField } from '../ui/FormField';
import { ModalShell } from '../ui/ModalShell';
import { StateBlock } from '../ui/StateBlock';

type Props = {
  open: boolean;
  onClose: () => void;
  onEvent: (msg: string, kind?: 'ok' | 'error' | 'info') => void;
};

/**
 * Purpose: Provide a minimal restore flow for a watched folder version.
 *
 * Inputs: open state, close handler, and a callback for user-visible events.
 * Outputs: A modal dialog element when open.
 * Ties to: Minimal main screen restore action.
 * Side effects: Invokes IPC calls to list versions and perform restore operations.
 * Why: Implements the product spec restore UX without exposing advanced settings.
 */
export function RestoreModal({ open: isOpen, onClose, onEvent }: Props) {
  const [folders, setFolders] = useState<FolderVersionsDto[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [reloadToken, setReloadToken] = useState(0);
  const [sourcePath, setSourcePath] = useState<string>('');
  const [versionId, setVersionId] = useState<string>('');
  const [mode, setMode] = useState<RestoreModeDto>('to_directory');
  const [scope, setScope] = useState<'folder' | 'files'>('folder');
  const [targetDir, setTargetDir] = useState<string>('');
  const [busy, setBusy] = useState(false);
  const [query, setQuery] = useState<string>('');
  const [fileResults, setFileResults] = useState<VersionFileInfoDto[] | null>(null);
  const [fileTotal, setFileTotal] = useState<number | null>(null);
  const [selected, setSelected] = useState<Record<string, boolean>>({});
  const [fileLoading, setFileLoading] = useState(false);

  useEffect(() => {
    if (!isOpen) return;
    let cancelled = false;
    setLoading(true);
    setLoadError(null);
    setFileResults(null);
    setFileTotal(null);
    setSelected({});
    setQuery('');
    setTargetDir('');
    setScope('folder');
    listVersions()
      .then((v) => {
        if (cancelled) return;
        setFolders(v);
        const firstFolder = v[0]?.source_path ?? '';
        setSourcePath(firstFolder);
        const firstVersion = v[0]?.versions?.[0]?.id ?? '';
        setVersionId(firstVersion);
      })
      .catch((e) => {
        if (cancelled) return;
        const reason = e instanceof Error ? e.message : String(e);
        setLoadError(reason);
        onEvent(`[Restore] Failed to list versions: ${reason}`, 'error');
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [isOpen, onEvent, reloadToken]);

  const selectedVersions = useMemo(() => {
    const folder = folders?.find((f) => f.source_path === sourcePath);
    return folder?.versions ?? [];
  }, [folders, sourcePath]);

  useEffect(() => {
    if (!isOpen) return;
    if (selectedVersions.length === 0) {
      setVersionId('');
      return;
    }
    if (!selectedVersions.some((v) => v.id === versionId)) {
      setVersionId(selectedVersions[0]!.id);
    }
  }, [isOpen, selectedVersions, versionId]);

  const pickTargetDir = async () => {
    try {
      const selection = await openDialog({
        directory: true,
        multiple: false,
        title: 'Choose a restore destination folder',
      });
      if (typeof selection === 'string') {
        setTargetDir(selection);
      }
    } catch (e) {
      const reason = e instanceof Error ? e.message : String(e);
      onEvent(`[Restore] Failed to open folder picker: ${reason}`, 'error');
    }
  };

  const runSearch = async () => {
    try {
      if (!sourcePath || !versionId) return;
      setFileLoading(true);
      const res = await listVersionFiles({
        source_path: sourcePath,
        version_id: versionId,
        query: query.trim() || null,
        limit: 200,
      });
      setFileResults(res.files);
      setFileTotal(res.total_files);
    } catch (e) {
      const reason = e instanceof Error ? e.message : String(e);
      onEvent(`[Restore] Failed to search files: ${reason}`, 'error');
    } finally {
      setFileLoading(false);
    }
  };

  useEffect(() => {
    if (!isOpen) return;
    if (scope !== 'files') return;
    // Refresh list on folder or version change.
    void runSearch();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, scope, sourcePath, versionId]);

  const runRestore = async () => {
    try {
      if (!sourcePath) {
        onEvent('Choose a folder to restore.', 'error');
        return;
      }
      if (!versionId) {
        onEvent('Choose a version to restore.', 'error');
        return;
      }
      if (mode === 'to_directory' && !targetDir) {
        onEvent('Choose a restore destination folder.', 'error');
        return;
      }
      setBusy(true);
      const isInPlace = mode === 'in_place';
      if (scope === 'folder') {
        if (
          isInPlace &&
          !confirm(
            'Restore in place will overwrite files and remove any files not present in the selected version. Continue?',
          )
        ) {
          return;
        }
        const result = await restoreVersion({
          source_path: sourcePath,
          version_id: versionId,
          mode,
          target_dir: mode === 'to_directory' ? targetDir : null,
        });
        onEvent(
          `[Restore] Complete. files_written=${result.files_written} files_removed=${result.files_removed} dirs_created=${result.dirs_created}`,
          'ok',
        );
      } else {
        const relPaths = Object.keys(selected).filter((k) => selected[k]);
        if (relPaths.length === 0) {
          onEvent('Select at least one file to restore.', 'error');
          return;
        }
        if (
          isInPlace &&
          !confirm('File restore in place will overwrite selected files. Continue?')
        ) {
          return;
        }
        const result = await restoreFiles({
          source_path: sourcePath,
          version_id: versionId,
          rel_paths: relPaths,
          mode,
          target_dir: mode === 'to_directory' ? targetDir : null,
        });
        onEvent(
          `[Restore] Complete. files_written=${result.files_written} dirs_created=${result.dirs_created}`,
          'ok',
        );
      }
      onClose();
    } catch (e) {
      const reason = e instanceof Error ? e.message : String(e);
      onEvent(`[Restore] Failed: ${reason}`, 'error');
    } finally {
      setBusy(false);
    }
  };

  const selectedCount = Object.values(selected).filter(Boolean).length;
  const hasFolders = (folders ?? []).length > 0;
  const singleVersionOnly = selectedVersions.length === 1;
  const canRestore =
    !busy &&
    !loading &&
    !loadError &&
    hasFolders &&
    !!sourcePath &&
    !!versionId &&
    (mode !== 'to_directory' || !!targetDir) &&
    (scope === 'folder' || selectedCount > 0);

  return (
    <ModalShell
      open={isOpen}
      title="Restore Backup Version"
      onClose={onClose}
      busy={busy}
      description="Choose a folder, pick a saved version, decide whether to restore the whole folder or specific files, and choose where it should go."
      footer={
        <>
          <Button tone="secondary" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button
            onClick={runRestore}
            disabled={!canRestore}
            loading={busy}
            loadingLabel="Restoring..."
          >
            Restore
          </Button>
        </>
      }
    >
      <div className="stack">
        {loading && (
          <StateBlock
            tone="loading"
            title="Loading versions"
            message="Fetching restore points for your watched folders."
          />
        )}
        {!loading && loadError && (
          <StateBlock
            tone="error"
            title="Could not load restore versions"
            message={loadError}
            action={
              <Button tone="secondary" size="sm" onClick={() => setReloadToken((prev) => prev + 1)}>
                Retry
              </Button>
            }
          />
        )}
        {!loading && !loadError && !hasFolders && (
          <StateBlock
            tone="empty"
            title="No restore points available"
            message="Run at least one backup cycle, then open restore again."
          />
        )}

        {!loading && !loadError && hasFolders && (
          <>
            <FormField label="Folder">
              <select
                value={sourcePath}
                onChange={(e) => setSourcePath(e.target.value)}
                disabled={busy || loading}
              >
                {(folders ?? []).map((folder) => (
                  <option key={folder.source_path} value={folder.source_path}>
                    {folder.source_path} ({folder.versions.length} version
                    {folder.versions.length === 1 ? '' : 's'})
                  </option>
                ))}
              </select>
            </FormField>

            <FormField label="Version">
              <select
                value={versionId}
                onChange={(e) => setVersionId(e.target.value)}
                disabled={busy || loading}
              >
                {selectedVersions.map((version) => (
                  <option key={version.id} value={version.id}>
                    {version.id} ({new Date(version.created_at_unix * 1000).toLocaleString()})
                  </option>
                ))}
              </select>
            </FormField>

            {singleVersionOnly && (
              <StateBlock
                tone="info"
                title="One saved version available"
                message="No additional changes were captured for this folder yet, so only one version can be restored."
              />
            )}

            <FormField label="What do you want to restore?">
              <div className="choice-grid" role="group" aria-label="Restore scope">
                <button
                  type="button"
                  className={`choice-btn ${scope === 'folder' ? 'is-active' : ''}`}
                  onClick={() => setScope('folder')}
                  disabled={busy || loading}
                >
                  <span className="choice-title">Entire folder</span>
                  <span className="choice-hint">Restore the full folder state from this version.</span>
                </button>
                <button
                  type="button"
                  className={`choice-btn ${scope === 'files' ? 'is-active' : ''}`}
                  onClick={() => setScope('files')}
                  disabled={busy || loading}
                >
                  <span className="choice-title">Individual files</span>
                  <span className="choice-hint">Pick specific files from this saved version.</span>
                </button>
              </div>
            </FormField>

            <FormField label="Where should it be restored?">
              <div className="choice-grid" role="group" aria-label="Restore destination mode">
                <button
                  type="button"
                  className={`choice-btn ${mode === 'to_directory' ? 'is-active' : ''}`}
                  onClick={() => setMode('to_directory')}
                  disabled={busy || loading}
                >
                  <span className="choice-title">Choose location</span>
                  <span className="choice-hint">Restore to a different folder to compare safely.</span>
                </button>
                <button
                  type="button"
                  className={`choice-btn ${mode === 'in_place' ? 'is-active' : ''}`}
                  onClick={() => setMode('in_place')}
                  disabled={busy || loading}
                >
                  <span className="choice-title">In place</span>
                  <span className="choice-hint">Overwrite files at the original source location.</span>
                </button>
              </div>
            </FormField>

            {mode === 'to_directory' && (
              <div className="stack">
                <div className="pill pill-row" title={targetDir || ''}>
                  <span className="pill-main truncate">
                    {targetDir || 'Choose a restore destination folder'}
                  </span>
                  <Button
                    tone="secondary"
                    size="sm"
                    className="pill-action"
                    onClick={pickTargetDir}
                    disabled={busy || loading}
                  >
                    Choose…
                  </Button>
                </div>
              </div>
            )}

            {scope === 'files' && (
              <div className="stack">
                <FormField
                  label="Search"
                  hint="Type part of a file path, then press Enter or Search."
                >
                  <div className="row-inline">
                    <input
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                      placeholder="notes/todo"
                      disabled={busy || loading || fileLoading}
                      onKeyDown={(event) => {
                        if (event.key !== 'Enter') return;
                        event.preventDefault();
                        void runSearch();
                      }}
                    />
                    <Button
                      tone="secondary"
                      onClick={runSearch}
                      disabled={busy || loading}
                      loading={fileLoading}
                      loadingLabel="Searching..."
                    >
                      Search
                    </Button>
                  </div>
                </FormField>

                <div className="pill pill-row">
                  <span>
                    Results: {(fileResults ?? []).length}
                    {fileTotal != null ? ` (total_files=${fileTotal})` : ''}
                  </span>
                  <span>Selected: {selectedCount}</span>
                </div>

                <div className="scroll-pane" role="region" aria-label="Restore file search results">
                  {fileLoading && (
                    <StateBlock
                      tone="loading"
                      title="Searching files"
                      message="Scanning the selected restore version."
                    />
                  )}
                  {!fileLoading && (fileResults ?? []).length === 0 && (
                    <StateBlock
                      tone="empty"
                      title="No matching files"
                      message="Adjust the query or choose a different version."
                    />
                  )}
                  {!fileLoading &&
                    (fileResults ?? []).map((file) => (
                      <label key={file.rel_path} className="scroll-pane-row">
                        <input
                          type="checkbox"
                          checked={!!selected[file.rel_path]}
                          onChange={(event) =>
                            setSelected((prev) => ({
                              ...prev,
                              [file.rel_path]: event.target.checked,
                            }))
                          }
                          disabled={busy || loading}
                        />
                        <span className="truncate pill-main" title={file.rel_path}>
                          {file.rel_path}
                        </span>
                        <span className="scroll-pane-meta">{file.len} B</span>
                      </label>
                    ))}
                </div>
              </div>
            )}
          </>
        )}
      </div>
    </ModalShell>
  );
}
