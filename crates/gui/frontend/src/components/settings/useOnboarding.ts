import { useEffect, useState } from 'react';
import { Config } from './types';
import { UI_TUNING } from '../../config/uiTuning';

/**
 * Purpose: Manage onboarding state based on config readiness.
 *
 * Inputs: Current config and destination writable flag.
 * Outputs: Onboarding state and control helpers.
 * Ties to: Onboarding overlay flow in settings.
 * Side effects: Reads from localStorage and registers React effects.
 * Why: Guide first time setup through required steps.
 */
export function useOnboardingState(cfg: Config | null, destWritable: boolean | undefined) {
  const [step, setStep] = useState<number>(1);
  const [done, setDone] = useState<boolean>(false);

  useEffect(() => {
    try {
      const flag = localStorage.getItem(UI_TUNING.onboardingDoneStorageKey);
      if (flag === '1') setDone(true);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      console.warn(`[useOnboardingState::init] Failed to read onboarding flag: ${reason}`);
    }
  }, []);

  useEffect(() => {
    if (!cfg) return;
    if (cfg.watched.length === 0) {
      setStep(1);
      setDone(false);
      return;
    }
    if (!destWritable) {
      setStep(2);
      setDone(false);
      return;
    }
    if (!done) {
      setStep((s) => Math.max(s, 3));
    }
  }, [cfg?.watched.length, destWritable, done, cfg]);

  /**
   * Purpose: Finalize onboarding and persist completion state.
   *
   * Inputs: None.
   * Outputs: Updates onboarding step and done state.
   * Ties to: Onboarding overlay completion flow.
   * Side effects: Writes onboarding completion to localStorage.
   * Why: Keep onboarding completion persistent across sessions.
   */
  const finish = () => {
    setDone(true);
    try {
      localStorage.setItem(UI_TUNING.onboardingDoneStorageKey, '1');
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      console.warn(`[useOnboardingState::finish] Failed to persist onboarding flag: ${reason}`);
    }
    setStep(4);
  };

  const show = !done || (cfg && (cfg.watched.length === 0 || !destWritable));

  return { step, setStep, done, setDone: finish, showOnboarding: show };
}
