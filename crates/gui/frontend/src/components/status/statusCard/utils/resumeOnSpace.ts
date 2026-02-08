import { UI_TUNING } from '../../../../config/uiTuning';

/**
 * Summary: Read the resume-on-space preference from storage.
 *
 * Inputs: None.
 * Outputs: Boolean value indicating resume behavior.
 * Side effects: Reads from localStorage.
 * Error handling: Returns `false` when storage is unavailable.
 * Ties to other methods: Used by status card low-space guard state initialization.
 * Why this exists: Keep the UI in sync with persisted resume behavior.
 */
export function readResumeOnSpace(): boolean {
  try {
    return localStorage.getItem(UI_TUNING.resumeOnSpaceStorageKey) === '1';
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.warn(
      `[statusCard::readResumeOnSpace] Failed to read local storage: ${reason}`,
    );
    return false;
  }
}

/**
 * Summary: Persist the resume-on-space preference to storage.
 *
 * Inputs: `next` as the desired resume state.
 * Outputs: None.
 * Side effects: Writes to localStorage.
 * Error handling: Logs a warning when storage is unavailable.
 * Ties to other methods: Used by status card low-space guard toggle handler.
 * Why this exists: Preserve operator preference across app sessions.
 */
export function writeResumeOnSpace(next: boolean): void {
  try {
    localStorage.setItem(UI_TUNING.resumeOnSpaceStorageKey, next ? '1' : '0');
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.warn(
      `[statusCard::writeResumeOnSpace] Failed to write local storage: ${reason}`,
    );
  }
}

