import { useEffect } from 'react';
import { prewarmNativeApis } from '../../services/nativePrewarm';

const RETRY_DELAYS_MS = [0, 150, 350, 700, 1200, 2000, 3200] as const;

/**
 * Summary: Prewarm native APIs after first paint with bounded retries.
 *
 * Inputs: None.
 *
 * Outputs: None.
 *
 * Side effects: Schedules and clears browser timers that invoke native prewarm calls.
 *
 * Error handling: Delegated to `prewarmNativeApis`, which handles its own failures.
 *
 * Ties to other methods: Used by `App` during startup.
 *
 * Why this exists: Keep native picker startup latency low without cluttering the root component.
 */
export function useNativePrewarm(): void {
  useEffect(() => {
    const timeouts: Array<ReturnType<typeof setTimeout>> = [];

    /**
     * Summary: Schedule native prewarm retries against the known delay budget.
     *
     * Inputs: None.
     *
     * Outputs: None.
     *
     * Side effects: Registers timeout callbacks that call `prewarmNativeApis`.
     *
     * Error handling: Delegated to `prewarmNativeApis`.
     *
     * Ties to other methods: Invoked by `useNativePrewarm` after the initial timeout fires.
     *
     * Why this exists: Tauri can attach IPC shortly after first paint, so retries smooth over that gap.
     */
    const runPrewarmSchedule = () => {
      for (const delayMs of RETRY_DELAYS_MS) {
        timeouts.push(
          setTimeout(() => {
            void prewarmNativeApis();
          }, delayMs),
        );
      }
    };

    const initialTimeout = setTimeout(runPrewarmSchedule, 0);
    return () => {
      clearTimeout(initialTimeout);
      for (const timeoutId of timeouts) {
        clearTimeout(timeoutId);
      }
    };
  }, []);
}
