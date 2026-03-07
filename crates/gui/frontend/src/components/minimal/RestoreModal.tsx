import { Button } from '../ui/Button';
import { ModalShell } from '../ui/ModalShell';
import { useRestoreCatalog } from './restore/hooks/useRestoreCatalog';
import { useRestoreExecution } from './restore/hooks/useRestoreExecution';
import { useRestoreSearch } from './restore/hooks/useRestoreSearch';
import { RestoreModalBody } from './restore/views/RestoreModalBody';

type Props = {
  open: boolean;
  onClose: () => void;
  onEvent: (msg: string, kind?: 'ok' | 'error' | 'info') => void;
};

/**
 * Summary: Provide a minimal restore flow for a watched folder version.
 *
 * Inputs: open state, close handler, and a callback for user-visible events.
 *
 * Outputs: A modal dialog element when open.
 *
 * Side effects: Invokes IPC calls to list versions and perform restore operations.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: Minimal main screen restore action.
 *
 * Why this exists: Implements the product spec restore UX without exposing advanced settings.
 */
export function RestoreModal({ open: isOpen, onClose, onEvent }: Props) {
  const catalog = useRestoreCatalog({ isOpen, onEvent });
  const execution = useRestoreExecution({ isOpen, onClose, onEvent });
  const search = useRestoreSearch({
    isOpen,
    scope: execution.scope,
    sourcePath: catalog.sourcePath,
    versionId: catalog.versionId,
    onEvent,
  });
  const canRestore =
    !execution.busy &&
    !catalog.loading &&
    !catalog.loadError &&
    catalog.hasFolders &&
    !!catalog.sourcePath &&
    !!catalog.versionId &&
    (execution.mode !== 'to_directory' || !!execution.targetDir) &&
    (execution.scope === 'folder' || search.selectedCount > 0);

  return (
    <ModalShell
      open={isOpen}
      title="Restore Backup Version"
      onClose={onClose}
      busy={execution.busy}
      description="Choose a folder, pick a saved version, decide whether to restore the whole folder or specific files, and choose where it should go."
      footer={
        <>
          <Button tone="secondary" onClick={onClose} disabled={execution.busy}>
            Cancel
          </Button>
          <Button
            onClick={() => {
              void execution.runRestore({
                sourcePath: catalog.sourcePath,
                versionId: catalog.versionId,
                selected: search.selected,
              });
            }}
            disabled={!canRestore}
            loading={execution.busy}
            loadingLabel="Restoring..."
          >
            Restore
          </Button>
        </>
      }
    >
      <RestoreModalBody
        busy={execution.busy}
        folders={catalog.folders}
        loading={catalog.loading}
        loadError={catalog.loadError}
        hasFolders={catalog.hasFolders}
        singleVersionOnly={catalog.singleVersionOnly}
        sourcePath={catalog.sourcePath}
        setSourcePath={catalog.setSourcePath}
        versionId={catalog.versionId}
        setVersionId={catalog.setVersionId}
        selectedVersions={catalog.selectedVersions}
        scope={execution.scope}
        setScope={execution.setScope}
        mode={execution.mode}
        setMode={execution.setMode}
        targetDir={execution.targetDir}
        pickTargetDir={() => {
          void execution.pickTargetDir();
        }}
        query={search.query}
        setQuery={search.setQuery}
        fileResults={search.fileResults}
        fileTotal={search.fileTotal}
        selectedCount={search.selectedCount}
        fileLoading={search.fileLoading}
        setSelectedValue={search.setSelectedValue}
        runSearch={() => {
          void search.runSearch();
        }}
        selected={search.selected}
        onRetry={catalog.reload}
      />
    </ModalShell>
  );
}
