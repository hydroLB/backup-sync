import { correlationId } from './correlation';
import {
  FolderVersionsDto,
  ListVersionFilesArgs,
  ListVersionFilesResultDto,
  RestoreArgs,
  RestoreFilesArgs,
  RestoreResultDto,
} from './types';
import { safeInvoke, safeInvokeWithTimeout, wrapError } from './ipc';

/** Preserve the current live state before an in-place recall when the user asks for it. */
export async function createSafetyBackup(): Promise<void> {
  try {
    await safeInvokeWithTimeout<void>(
      'run_now_cmd',
      { correlationId: correlationId('restore_safety') },
      0,
    );
  } catch (error) {
    throw wrapError('[createSafetyBackup] Failed to preserve the current state', error);
  }
}

/** Enables selecting a restore version from the GUI. */
export async function listVersions(): Promise<FolderVersionsDto[]> {
  try {
    return await safeInvoke<FolderVersionsDto[]>('list_versions_cmd', {
      correlationId: correlationId('restore_list'),
    });
  } catch (error) {
    throw wrapError('[listVersions] Failed to list restore versions', error);
  }
}

/** Provides the end user restore behavior for versioned backups. */
export async function restoreVersion(args: RestoreArgs): Promise<RestoreResultDto> {
  try {
    return await safeInvokeWithTimeout<RestoreResultDto>(
      'restore_version_cmd',
      { args, correlationId: correlationId('restore') },
      0,
    );
  } catch (error) {
    throw wrapError('[restoreVersion] Failed to restore selected version', error);
  }
}

/** Enables searching within a version manifest without loading everything client-side. */
export async function listVersionFiles(
  args: ListVersionFilesArgs,
): Promise<ListVersionFilesResultDto> {
  try {
    return await safeInvoke<ListVersionFilesResultDto>('list_version_files_cmd', {
      args,
      correlationId: correlationId('restore_files_list'),
    });
  } catch (error) {
    throw wrapError('[listVersionFiles] Failed to list files for version', error);
  }
}

/** Provide a safer restore option when only a few files are needed. */
export async function restoreFiles(args: RestoreFilesArgs): Promise<RestoreResultDto> {
  try {
    return await safeInvokeWithTimeout<RestoreResultDto>(
      'restore_files_cmd',
      { args, correlationId: correlationId('restore_files') },
      0,
    );
  } catch (error) {
    throw wrapError('[restoreFiles] Failed to restore selected files', error);
  }
}
