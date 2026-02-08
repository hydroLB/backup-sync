import { correlationId } from './correlation';
import { safeInvoke, wrapError } from './ipc';

/**
 * Purpose: Remove the extra safety baseline version kept after a large deletion event.
 *
 * Inputs: Watched path string for the protected source.
 * Outputs: Resolves when the baseline is removed.
 * Ties to: Minimal safety warning banner action.
 * Side effects: Invokes backend commands that mutate the versioned store.
 * Why: When shrink is expected, the user should be able to delete the extra kept version.
 */
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

