import { safeInvoke, wrapError } from './ipc';
import { StatusDto } from './types';

/**
 * Summary: Fetch the current daemon status from the backend.
 *
 * Inputs: None.
 *
 * Outputs: A `StatusDto` payload.
 *
 * Side effects: Invokes IPC calls to the backend.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: Status refresh actions across the UI.
 *
 * Why this exists: Keeps status panels up to date.
 */
export async function getStatus(): Promise<StatusDto> {
  try {
    return await safeInvoke<StatusDto>('get_status');
  } catch (error) {
    throw wrapError('[getStatus] Failed to fetch status', error);
  }
}
