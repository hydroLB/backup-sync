import { correlationId } from './correlation';
import { safeInvoke, wrapError } from './ipc';

/** When shrink is expected, the user should be able to delete the extra kept version. */
export async function removeKeptExtraVersion(watchedPath: string): Promise<void> {
  try {
    await safeInvoke('remove_kept_extra_version_cmd', {
      watchedPath,
      correlationId: correlationId('safety-remove'),
    });
  } catch (error) {
    throw wrapError('[removeKeptExtraVersion] Failed to remove extra kept version', error);
  }
}
