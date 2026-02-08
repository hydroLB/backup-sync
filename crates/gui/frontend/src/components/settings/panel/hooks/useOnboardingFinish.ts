import { useCallback } from 'react';

type Params = {
  canFinishOnboarding: boolean;
  setDone: () => void;
  setStatus: (msg: string) => void;
};

/**
 * Summary: Provide the onboarding finish handler with prerequisite gating.
 *
 * Inputs: `canFinishOnboarding` flag, onboarding `setDone`, and status setter.
 * Outputs: A `finishOnboarding` function.
 * Side effects: Marks onboarding done when prerequisites are met.
 * Error handling: Writes a contextual failure message into status on error.
 * Ties to other methods: Used by `useSettingsPanelActions` and onboarding views.
 * Why this exists: Keep finish semantics consistent and safe across UI entry points.
 */
export function useOnboardingFinish({ canFinishOnboarding, setDone, setStatus }: Params) {
  const finishOnboarding = useCallback(() => {
    try {
      if (!canFinishOnboarding) return;
      setDone();
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      setStatus(`[useOnboardingFinish::finishOnboarding] Failed to finish onboarding: ${reason}`);
    }
  }, [canFinishOnboarding, setDone, setStatus]);

  return { finishOnboarding };
}

