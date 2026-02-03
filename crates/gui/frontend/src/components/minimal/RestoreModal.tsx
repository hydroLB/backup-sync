import { useEffect, useMemo, useState } from 'react';
import { open } from '@tauri-apps/api/dialog';
import { FolderVersionsDto, RestoreModeDto } from '../../services/types';
import { listVersions, restoreVersion } from '../../services/restore';

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
  const [sourcePath, setSourcePath] = useState<string>('');
  const [versionId, setVersionId] = useState<string>('');
  const [mode, setMode] = useState<RestoreModeDto>('to_directory');
  const [targetDir, setTargetDir] = useState<string>('');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!isOpen) return;
    let cancelled = false;
    setLoading(true);
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
        const reason = e instanceof Error ? e.message : String(e);
        onEvent(`[Restore] Failed to list versions: ${reason}`, 'error');
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [isOpen, onEvent]);

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
      const selection = await open({
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
      if (
        mode === 'in_place' &&
        !confirm(
          'Restore in place will overwrite files and remove any files not present in the selected version. Continue?',
        )
      ) {
        return;
      }
      setBusy(true);
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
      onClose();
    } catch (e) {
      const reason = e instanceof Error ? e.message : String(e);
      onEvent(`[Restore] Failed: ${reason}`, 'error');
    } finally {
      setBusy(false);
    }
  };

  if (!isOpen) return null;

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(0,0,0,0.55)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        padding: 12,
        zIndex: 50,
      }}
      onClick={onClose}
    >
      <div
        className="card"
        style={{ width: 640, maxWidth: '100%' }}
        onClick={(e) => e.stopPropagation()}
      >
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            gap: 10,
          }}
        >
          <h3 style={{ margin: 0 }}>Restore</h3>
          <button className="btn secondary" onClick={onClose} disabled={busy}>
            Close
          </button>
        </div>
        <p style={{ marginTop: 8, marginBottom: 10, color: 'var(--muted)', fontSize: 13 }}>
          Choose a folder and version to restore. Restore in place will also delete files that
          should not exist in that version.
        </p>

        {loading && <div className="pill">Loading versions…</div>}

        <div style={{ display: 'grid', gap: 10 }}>
          <label style={{ display: 'grid', gap: 6 }}>
            <span style={{ color: 'var(--muted)', fontSize: 12 }}>Folder</span>
            <select
              value={sourcePath}
              onChange={(e) => setSourcePath(e.target.value)}
              disabled={busy || loading}
            >
              {(folders ?? []).map((f) => (
                <option key={f.source_path} value={f.source_path}>
                  {f.source_path}
                </option>
              ))}
            </select>
          </label>

          <label style={{ display: 'grid', gap: 6 }}>
            <span style={{ color: 'var(--muted)', fontSize: 12 }}>Version</span>
            <select
              value={versionId}
              onChange={(e) => setVersionId(e.target.value)}
              disabled={busy || loading}
            >
              {selectedVersions.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.id} ({new Date(v.created_at_unix * 1000).toLocaleString()})
                </option>
              ))}
            </select>
          </label>

          <label style={{ display: 'grid', gap: 6 }}>
            <span style={{ color: 'var(--muted)', fontSize: 12 }}>Restore mode</span>
            <select
              value={mode}
              onChange={(e) => setMode(e.target.value as RestoreModeDto)}
              disabled={busy || loading}
            >
              <option value="to_directory">To a new folder…</option>
              <option value="in_place">In place (overwrite)</option>
            </select>
          </label>

          {mode === 'to_directory' && (
            <div style={{ display: 'grid', gap: 8 }}>
              <div
                className="pill"
                title={targetDir || ''}
                style={{ justifyContent: 'space-between' }}
              >
                <span
                  style={{
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    whiteSpace: 'nowrap',
                    maxWidth: 520,
                  }}
                >
                  {targetDir || 'Choose a restore destination folder'}
                </span>
                <button
                  className="btn secondary"
                  onClick={pickTargetDir}
                  disabled={busy || loading}
                >
                  Choose…
                </button>
              </div>
            </div>
          )}

          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8 }}>
            <button className="btn secondary" onClick={onClose} disabled={busy}>
              Cancel
            </button>
            <button
              className="btn"
              onClick={runRestore}
              disabled={busy || loading || !sourcePath || !versionId}
            >
              Restore
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
