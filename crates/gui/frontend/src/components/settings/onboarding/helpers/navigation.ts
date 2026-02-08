import { DestinationStatus, Step } from '../types';

/**
 * Summary: Determine whether the onboarding Next button should be disabled.
 *
 * Inputs: Current step, whether at least one watched path exists, and destination status.
 * Outputs: `true` when navigation should be blocked.
 * Side effects: None.
 * Error handling: Returns `true` on unexpected errors to avoid skipping required steps.
 * Ties to other methods: Used by `OnboardingOverlay` navigation controls.
 * Why this exists: Keep navigation rules centralized and reusable across onboarding UI variants.
 */
export function isNextDisabled(step: Step, hasWatched: boolean, destStatus: DestinationStatus): boolean {
  try {
    const destOk = destStatus?.writable ?? false;
    if (step === 1) return !hasWatched;
    if (step === 2) return !destOk;
    if (step === 3) return !(hasWatched && destOk);
    return false;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.warn(`[onboarding::isNextDisabled] Failed to evaluate next state: ${reason}`);
    return true;
  }
}

