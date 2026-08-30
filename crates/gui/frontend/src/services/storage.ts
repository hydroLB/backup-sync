import { Config } from '../domain/config';
import { IS_WEB_RUNTIME } from '../runtime/mode';
import { safeInvoke } from './ipc';
import { safeInvokeWithTimeout, wrapError } from './ipc';

export type DestinationRelocationResult = {
  config: Config;
  operation: 'moved_to_new_location' | 'promoted_existing_secondary';
  files_moved: number;
  bytes_moved: number;
  old_location_removed: boolean;
  warning: string | null;
};

const STORAGE_MOVE_TIMEOUT_MS = 30 * 60 * 1000;

/** Open a configured native storage location without exposing an arbitrary-path command. */
export async function openDestinationFolder(destinationId: string): Promise<boolean> {
  if (IS_WEB_RUNTIME) return false;
  try {
    await safeInvoke<void>('open_destination_cmd', { destinationId });
    return true;
  } catch (error) {
    throw wrapError('[openDestinationFolder] Failed to open backup storage', error);
  }
}

/** Move an existing backup store only through the backend's pause, verify, switch, and cleanup flow. */
export async function relocateDestination(
  destinationId: string,
  newPath: string,
): Promise<DestinationRelocationResult> {
  try {
    return await safeInvokeWithTimeout<DestinationRelocationResult>(
      'relocate_destination_cmd',
      { destinationId, newPath },
      STORAGE_MOVE_TIMEOUT_MS,
    );
  } catch (error) {
    throw wrapError('[relocateDestination] Failed to move backup storage', error);
  }
}
