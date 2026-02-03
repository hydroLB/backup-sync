import { correlationId } from './correlation';
import { safeInvoke, wrapError } from './ipc';

export type AuthStatus = { unlocked: boolean; seconds_left: number | null };

type AuthStatusResponse = [boolean, number | null];

/**
 * Purpose: Unlock the session using the provided passcode.
 *
 * Inputs: `passcode` as a user supplied unlock code.
 * Outputs: A backend status message string.
 * Ties to: GUI unlock flows and backend auth commands.
 * Side effects: Invokes IPC calls that update auth state.
 * Why: Enables authenticated access to privileged actions.
 */
export async function unlockSession(passcode: string): Promise<string> {
  try {
    return await safeInvoke<string>('unlock_session_cmd', {
      passcode,
      correlationId: correlationId('auth'),
    });
  } catch (error) {
    throw wrapError('[unlockSession] Failed to unlock session', error);
  }
}

/**
 * Purpose: Lock the session immediately.
 *
 * Inputs: None.
 * Outputs: Resolves when the lock completes.
 * Ties to: GUI lock flows and backend auth commands.
 * Side effects: Invokes IPC calls that update auth state.
 * Why: Allows users to revoke privileged access on demand.
 */
export async function lockSession(): Promise<void> {
  try {
    return await safeInvoke<void>('lock_session_cmd');
  } catch (error) {
    throw wrapError('[lockSession] Failed to lock session', error);
  }
}

/**
 * Purpose: Fetch the current auth status from the backend.
 *
 * Inputs: None.
 * Outputs: An `AuthStatus` object describing unlock state and remaining time.
 * Ties to: GUI auth status displays and access controls.
 * Side effects: Invokes IPC calls to query auth state.
 * Why: Shows whether privileged actions are allowed.
 */
export async function authStatus(): Promise<AuthStatus> {
  try {
    const [unlocked, seconds_left] = await safeInvoke<AuthStatusResponse>('auth_status_cmd');
    return { unlocked, seconds_left };
  } catch (error) {
    throw wrapError('[authStatus] Failed to fetch auth status', error);
  }
}
