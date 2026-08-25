import { safeInvoke, wrapError } from './ipc';
import { StatusDto } from './types';

/** Keeps status panels up to date. */
export async function getStatus(): Promise<StatusDto> {
  try {
    return await safeInvoke<StatusDto>('get_status');
  } catch (error) {
    throw wrapError('[getStatus] Failed to fetch status', error);
  }
}
