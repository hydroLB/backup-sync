import { useCallback, useEffect, useState } from 'react';
import { RestoreModeDto } from '../../../../services/types';
import { openDialog } from '../../../../services/dialog';
import { restoreFiles, restoreVersion } from '../../../../services/restore';

type EventKind = 'ok' | 'error' | 'info';

type RunRestoreParams = {
  sourcePath: string;
  versionId: string;
  selected: Record<string, boolean>;
};

type Params = {
  isOpen: boolean;
  onClose: () => void;
  onEvent: (msg: string, kind?: EventKind) => void;
};

type Result = {
  busy: boolean;
  mode: RestoreModeDto;
  setMode: (value: RestoreModeDto) => void;
  scope: 'folder' | 'files';
  setScope: (value: 'folder' | 'files') => void;
  targetDir: string;
  pickTargetDir: () => Promise<void>;
  runRestore: (params: RunRestoreParams) => Promise<void>;
};

/**
 * Summary: Manage restore destination settings and execute restore operations.
 *
 * Inputs: Modal close callback and event callback.
 *
 * Outputs: Restore mode/scope state, target-folder picker, and restore runner.
 *
 * Side effects: Opens the native folder picker and invokes restore IPC endpoints.
 *
 * Error handling: Emits contextual restore failures through `onEvent`.
 *
 * Ties to other methods: Used by `RestoreModal` and `RestoreModalBody`.
 *
 * Why this exists: Keep restore execution rules and prompts out of the presentation layer.
 */
export function useRestoreExecution({ isOpen, onClose, onEvent }: Params): Result {
  const [mode, setMode] = useState<RestoreModeDto>('to_directory');
  const [scope, setScope] = useState<'folder' | 'files'>('folder');
  const [targetDir, setTargetDir] = useState<string>('');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!isOpen) {
      return;
    }
    setMode('to_directory');
    setScope('folder');
    setTargetDir('');
  }, [isOpen]);

  /**
   * Summary: Open the native folder picker for choosing a restore destination.
   *
   * Inputs: None.
   *
   * Outputs: Updates the selected restore target directory.
   *
   * Side effects: Opens a native directory picker.
   *
   * Error handling: Emits contextual picker failures through `onEvent`.
   *
   * Ties to other methods: Used by `RestoreModalBody` when restore mode is `to_directory`.
   *
   * Why this exists: Restore-to-directory mode needs a safe explicit target path.
   */
  const pickTargetDir = useCallback(async () => {
    try {
      const selection = await openDialog({
        directory: true,
        multiple: false,
        title: 'Choose a restore destination folder',
      });
      if (typeof selection === 'string') {
        setTargetDir(selection);
      }
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      onEvent(`[Restore] Failed to open folder picker: ${reason}`, 'error');
    }
  }, [onEvent]);

  /**
   * Summary: Execute a restore using the current scope, mode, and target selection.
   *
   * Inputs: Selected source path, version id, and file-selection map.
   *
   * Outputs: Runs the restore request and closes the modal on success.
   *
   * Side effects: Invokes restore IPC calls, shows confirmation dialogs, and updates busy state.
   *
   * Error handling: Emits actionable validation and execution failures through `onEvent`.
   *
   * Ties to other methods: Triggered by the modal primary action button.
   *
   * Why this exists: The restore flow needs one place to enforce confirmation and input rules.
   */
  const runRestore = useCallback(
    async ({ sourcePath, versionId, selected }: RunRestoreParams) => {
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
          const relPaths = Object.keys(selected).filter((key) => selected[key]);
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
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        onEvent(`[Restore] Failed: ${reason}`, 'error');
      } finally {
        setBusy(false);
      }
    },
    [mode, onClose, onEvent, scope, targetDir],
  );

  return {
    busy,
    mode,
    setMode,
    scope,
    setScope,
    targetDir,
    pickTargetDir,
    runRestore,
  };
}
