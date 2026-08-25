import { safeInvoke, wrapError } from '../../../services/ipc';

export type SafeModeUpdateResult = {
  safe_mode: boolean;
  applied_live: boolean;
  warning: string | null;
};

/** Persist the preference and report separately whether the live daemon acknowledged it. */
export async function setSafeMode(desired: boolean): Promise<SafeModeUpdateResult> {
  try {
    return await safeInvoke<SafeModeUpdateResult>('toggle_safe_mode_cmd', { desired });
  } catch (error) {
    throw wrapError('[setSafeMode] Failed to toggle safe mode', error);
  }
}
