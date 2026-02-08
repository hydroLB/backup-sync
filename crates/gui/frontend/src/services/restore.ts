import { correlationId } from './correlation';
import {
  FolderVersionsDto,
  ListVersionFilesArgs,
  ListVersionFilesResultDto,
  RestoreArgs,
  RestoreFilesArgs,
  RestoreResultDto,
} from './types';
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

/**
 * Purpose: List files within a specific version for file-level restore flows.
 *
 * Inputs: Source folder, version id, optional query, and optional limit.
 * Outputs: A list of matching file entries and a total file count.
 * Ties to: File-level Restore UI flow.
 * Side effects: Invokes IPC calls to the backend.
 * Why: Enables searching within a version manifest without loading everything client-side.
 */
export async function listVersionFiles(args: ListVersionFilesArgs): Promise<ListVersionFilesResultDto> {
  try {
    return await safeInvoke<ListVersionFilesResultDto>('list_version_files_cmd', {
      args,
      correlationId: correlationId('restore_files_list'),
    });
  } catch (error) {
    throw wrapError('[listVersionFiles] Failed to list files for version', error);
  }
}

/**
 * Purpose: Restore individual files from a selected version.
 *
 * Inputs: Source folder, version id, rel paths, mode, and optional target dir.
 * Outputs: A restore result summary.
 * Ties to: File-level Restore UI flow.
 * Side effects: Writes selected files to disk; does not delete extraneous files.
 * Why: Provide a safer restore option when only a few files are needed.
 */
export async function restoreFiles(args: RestoreFilesArgs): Promise<RestoreResultDto> {
  try {
    return await safeInvoke<RestoreResultDto>('restore_files_cmd', {
      args,
      correlationId: correlationId('restore_files'),
    });
  } catch (error) {
    throw wrapError('[restoreFiles] Failed to restore selected files', error);
  }
}
