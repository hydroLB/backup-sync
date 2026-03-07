import { FolderVersionsDto, RestoreModeDto, VersionFileInfoDto } from '../../../../services/types';
import { Button } from '../../../ui/Button';
import { FormField } from '../../../ui/FormField';
import { StateBlock } from '../../../ui/StateBlock';

type Props = {
  busy: boolean;
  folders: FolderVersionsDto[] | null;
  loading: boolean;
  loadError: string | null;
  hasFolders: boolean;
  singleVersionOnly: boolean;
  sourcePath: string;
  setSourcePath: (value: string) => void;
  versionId: string;
  setVersionId: (value: string) => void;
  selectedVersions: FolderVersionsDto['versions'];
  scope: 'folder' | 'files';
  setScope: (value: 'folder' | 'files') => void;
  mode: RestoreModeDto;
  setMode: (value: RestoreModeDto) => void;
  targetDir: string;
  pickTargetDir: () => void;
  query: string;
  setQuery: (value: string) => void;
  fileResults: VersionFileInfoDto[] | null;
  fileTotal: number | null;
  selectedCount: number;
  fileLoading: boolean;
  setSelectedValue: (relPath: string, checked: boolean) => void;
  runSearch: () => void;
  selected: Record<string, boolean>;
  onRetry: () => void;
};

/**
 * Summary: Render the restore modal body for catalog selection, search, and file picking.
 *
 * Inputs: Restore catalog/search state and user-action callbacks.
 *
 * Outputs: The restore body element tree.
 *
 * Side effects: Delegated to the provided callbacks.
 *
 * Error handling: Delegated to the restore hooks and modal shell.
 *
 * Ties to other methods: Used by `RestoreModal`.
 *
 * Why this exists: Keep the restore UI layout isolated from modal orchestration and IPC logic.
 */
export function RestoreModalBody({
  busy,
  folders,
  loading,
  loadError,
  hasFolders,
  singleVersionOnly,
  sourcePath,
  setSourcePath,
  versionId,
  setVersionId,
  selectedVersions,
  scope,
  setScope,
  mode,
  setMode,
  targetDir,
  pickTargetDir,
  query,
  setQuery,
  fileResults,
  fileTotal,
  selectedCount,
  fileLoading,
  setSelectedValue,
  runSearch,
  selected,
  onRetry,
}: Props) {
  return (
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
            <Button tone="secondary" size="sm" onClick={onRetry}>
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
              onChange={(event) => setSourcePath(event.target.value)}
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
              onChange={(event) => setVersionId(event.target.value)}
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
                <span className="choice-hint">
                  Restore the full folder state from this version.
                </span>
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
                <span className="choice-hint">
                  Restore to a different folder to compare safely.
                </span>
              </button>
              <button
                type="button"
                className={`choice-btn ${mode === 'in_place' ? 'is-active' : ''}`}
                onClick={() => setMode('in_place')}
                disabled={busy || loading}
              >
                <span className="choice-title">In place</span>
                <span className="choice-hint">
                  Overwrite files at the original source location.
                </span>
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
                    onChange={(event) => setQuery(event.target.value)}
                    placeholder="notes/todo"
                    disabled={busy || loading || fileLoading}
                    onKeyDown={(event) => {
                      if (event.key !== 'Enter') {
                        return;
                      }
                      event.preventDefault();
                      runSearch();
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
                        onChange={(event) => setSelectedValue(file.rel_path, event.target.checked)}
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
  );
}
