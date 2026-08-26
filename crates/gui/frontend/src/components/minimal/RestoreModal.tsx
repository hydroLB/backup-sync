import { useEffect, useMemo, useState } from 'react';
import { createSafetyBackup, restoreVersion } from '../../services/restore';
import { Button } from '../ui/Button';
import { ModalShell } from '../ui/ModalShell';
import { StateBlock } from '../ui/StateBlock';
import { useRestoreCatalog } from './restore/hooks/useRestoreCatalog';

type Props = {
  open: boolean;
  onClose: () => void;
  onEvent: (msg: string, kind?: 'ok' | 'error' | 'info') => void;
};

type RecoveryStep = 'source' | 'version' | 'confirm';

function versionDate(unix: number): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(new Date(unix * 1000));
}

/** A deliberately linear recovery flow: protected item, version, confirmation. */
export function RestoreModal({ open, onClose, onEvent }: Props) {
  const catalog = useRestoreCatalog({ isOpen: open, onEvent });
  const [step, setStep] = useState<RecoveryStep>('source');
  const [preserveCurrent, setPreserveCurrent] = useState(true);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!open) return;
    setStep('source');
    setPreserveCurrent(true);
  }, [open]);

  const selectedVersion = useMemo(
    () => catalog.selectedVersions.find((version) => version.id === catalog.versionId) ?? null,
    [catalog.selectedVersions, catalog.versionId],
  );
  const newerVersionCount = useMemo(() => {
    const index = catalog.selectedVersions.findIndex((version) => version.id === catalog.versionId);
    return index < 0 ? 0 : index;
  }, [catalog.selectedVersions, catalog.versionId]);

  const close = () => {
    if (busy) return;
    onClose();
  };

  const recall = async () => {
    if (!catalog.sourcePath || !catalog.versionId) return;
    try {
      setBusy(true);
      if (preserveCurrent) {
        await createSafetyBackup();
      }
      const result = await restoreVersion({
        source_path: catalog.sourcePath,
        version_id: catalog.versionId,
        mode: 'in_place',
        target_dir: null,
      });
      onEvent(
        `Restored ${result.files_written} files from ${versionDate(selectedVersion?.created_at_unix ?? 0)}.`,
        'ok',
      );
      onClose();
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      onEvent(`Recovery failed: ${reason}`, 'error');
    } finally {
      setBusy(false);
    }
  };

  const title =
    step === 'source'
      ? 'Choose what to recover'
      : step === 'version'
        ? 'Choose a version'
        : 'Confirm recovery';

  return (
    <ModalShell
      open={open}
      title={title}
      onClose={close}
      busy={busy}
      closeLabel="×"
      closeAriaLabel="Close recovery"
      closeButtonClassName="modal-close-x"
      footer={
        step === 'confirm' ? (
          <>
            <Button tone="secondary" onClick={() => setStep('version')} disabled={busy}>
              Back
            </Button>
            <Button
              tone="danger"
              onClick={() => void recall()}
              loading={busy}
              loadingLabel="Recovering…"
            >
              Recall this version
            </Button>
          </>
        ) : undefined
      }
    >
      {catalog.loading && (
        <StateBlock
          tone="loading"
          title="Loading saved versions"
          message="Reading recovery history."
        />
      )}
      {!catalog.loading && catalog.loadError && (
        <StateBlock
          tone="error"
          title="Could not load recovery history"
          message={catalog.loadError}
          action={
            <Button tone="secondary" size="sm" onClick={catalog.reload}>
              Retry
            </Button>
          }
        />
      )}
      {!catalog.loading && !catalog.loadError && !catalog.hasFolders && (
        <StateBlock
          tone="empty"
          title="No saved versions yet"
          message="Backup Sync will create one after it protects your first changes."
        />
      )}

      {!catalog.loading && !catalog.loadError && catalog.hasFolders && step === 'source' && (
        <div className="recovery-choice-list" aria-label="Protected items with saved versions">
          {(catalog.folders ?? []).map((folder) => (
            <button
              type="button"
              className="recovery-source-row"
              key={folder.source_path}
              onClick={() => {
                catalog.setSourcePath(folder.source_path);
                setStep('version');
              }}
            >
              <span className="folder-kind">Folder</span>
              <span className="folder-path truncate" title={folder.source_path}>
                {folder.source_path}
              </span>
              <span className="recovery-row-meta">
                {folder.versions.length} version{folder.versions.length === 1 ? '' : 's'}
              </span>
              <span aria-hidden="true">→</span>
            </button>
          ))}
        </div>
      )}

      {!catalog.loading && !catalog.loadError && catalog.hasFolders && step === 'version' && (
        <div className="recovery-step">
          <button className="recovery-back" type="button" onClick={() => setStep('source')}>
            ← {catalog.sourcePath}
          </button>
          <div className="version-list" aria-label="Saved versions">
            {catalog.selectedVersions.map((version, index) => (
              <button
                type="button"
                className="version-row"
                key={version.id}
                onClick={() => {
                  catalog.setVersionId(version.id);
                  setStep('confirm');
                }}
              >
                <span className="version-row__dot" aria-hidden="true" />
                <span>
                  <strong>{versionDate(version.created_at_unix)}</strong>
                  <small>
                    {index === 0
                      ? 'Latest saved version'
                      : `${index} version${index === 1 ? '' : 's'} newer`}
                  </small>
                </span>
                <span aria-hidden="true">→</span>
              </button>
            ))}
          </div>
        </div>
      )}

      {!catalog.loading && !catalog.loadError && catalog.hasFolders && step === 'confirm' && (
        <div className="recovery-confirmation">
          <div className="recovery-selection">
            <span className="folder-kind">Folder</span>
            <strong className="truncate" title={catalog.sourcePath}>
              {catalog.sourcePath}
            </strong>
            <span>{selectedVersion ? versionDate(selectedVersion.created_at_unix) : ''}</span>
          </div>
          <div className="destructive-note" role="alert">
            <strong>This changes the live folder.</strong>
            <p>
              Files and changes newer than this restore point will be replaced or removed.
              {newerVersionCount > 0
                ? ` ${newerVersionCount} newer saved version${newerVersionCount === 1 ? '' : 's'} will remain in recovery history.`
                : ' Your saved recovery history will remain available.'}
            </p>
          </div>
          <label className="preserve-choice">
            <input
              type="checkbox"
              checked={preserveCurrent}
              onChange={(event) => setPreserveCurrent(event.target.checked)}
              disabled={busy}
            />
            <span>
              <strong>Save the current state first</strong>
              <small>Creates one safety version before recovery.</small>
            </span>
          </label>
        </div>
      )}
    </ModalShell>
  );
}
