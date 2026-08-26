import { Config } from '../domain/config';
import { safeInvokeWithTimeout, wrapError } from './ipc';

export type DestinationRelocationResult = {
  config: Config;
  files_moved: number;
  bytes_moved: number;
  old_location_removed: boolean;
  warning: string | null;
};

const STORAGE_MOVE_TIMEOUT_MS = 30 * 60 * 1000;

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
