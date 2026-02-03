import { correlationId } from './correlation';
import { FolderVersionsDto, RestoreArgs, RestoreResultDto } from './types';
import { safeInvoke, wrapError } from './ipc';

/**
 * Purpose: List available restore versions for each watched folder.
 *
 * Inputs: None.
 * Outputs: A list of watched folders and their known versions.
 * Ties to: The Restore UI flow.
 * Side effects: Invokes IPC calls to the backend.
 * Why: Enables selecting a restore version from the GUI.
 */
export async function listVersions(): Promise<FolderVersionsDto[]> {
  try {
    return await safeInvoke<FolderVersionsDto[]>('list_versions_cmd', {
      correlationId: correlationId('restore_list'),
    });
  } catch (error) {
    throw wrapError('[listVersions] Failed to list restore versions', error);
  }
}

/**
 * Purpose: Restore a selected version for a watched folder.
 *
 * Inputs: Restore args (source folder, version id, mode, optional target dir).
 * Outputs: A restore result summary.
 * Ties to: The Restore UI flow.
 * Side effects: Writes files to disk and may delete files during in-place restores.
 * Why: Provides the end user restore behavior for versioned backups.
 */
export async function restoreVersion(args: RestoreArgs): Promise<RestoreResultDto> {
  try {
    return await safeInvoke<RestoreResultDto>('restore_version_cmd', {
      args,
      correlationId: correlationId('restore'),
    });
  } catch (error) {
    throw wrapError('[restoreVersion] Failed to restore selected version', error);
  }
}
