import { safeInvoke, wrapError } from './ipc';
import { StatusDto } from './types';

/**
 * Purpose: Fetch the current daemon status from the backend.
 *
 * Inputs: None.
 * Outputs: A `StatusDto` payload.
 * Ties to: Status refresh actions across the UI.
 * Side effects: Invokes IPC calls to the backend.
 * Why: Keeps status panels up to date.
 */
export async function getStatus(): Promise<StatusDto> {
  try {
    return await safeInvoke<StatusDto>('get_status');
  } catch (error) {
    throw wrapError('[getStatus] Failed to fetch status', error);
  }
}
