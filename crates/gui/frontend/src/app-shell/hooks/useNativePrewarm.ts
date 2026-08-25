import { useEffect } from 'react';
import { prewarmNativeApis } from '../../services/nativePrewarm';

const RETRY_DELAYS_MS = [0, 150, 350, 700, 1200, 2000, 3200] as const;

/** Keep native picker startup latency low without cluttering the root component. */
export function useNativePrewarm(): void {
  useEffect(() => {
    const timeouts: Array<ReturnType<typeof setTimeout>> = [];

    /** Tauri can attach IPC shortly after first paint, so retries smooth over that gap. */
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
