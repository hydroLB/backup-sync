import { correlationId } from './correlation';
import { safeInvoke, wrapError } from './ipc';

/**
 * Summary: Remove the extra safety baseline version kept after a large deletion event.
 *
 * Inputs: Watched path string for the protected source.
 *
 * Outputs: Resolves when the baseline is removed.
 *
 * Side effects: Invokes backend commands that mutate the versioned store.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: Minimal safety warning banner action.
 *
 * Why this exists: When shrink is expected, the user should be able to delete the extra kept version.
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
