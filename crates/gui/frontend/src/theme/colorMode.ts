import { useCallback, useEffect, useMemo, useState } from 'react';
import { UI_TUNING } from '../config/uiTuning';
import { applyThemeTokens, ColorModePreference, ResolvedColorMode } from './tokens';

const COLOR_MODE_STORAGE_KEY = UI_TUNING.colorModeStorageKey;

/**
 * Summary: Validate that a value is a supported color-mode preference.
 *
 * Inputs: Unknown value read from storage or query sources.
 *
 * Outputs: True when the value is one of `light`, `dark`, or `auto`.
 *
 * Side effects: None.
 *
 * Error handling: None.
 *
 * Ties to other methods: Used by storage readers and setter guards.
 *
 * Why this exists: Keep user preference parsing strict and predictable.
 */
export function isColorModePreference(value: unknown): value is ColorModePreference {
  return value === 'light' || value === 'dark' || value === 'auto';
}

/**
 * Summary: Resolve the concrete mode from user preference and OS preference.
 *
 * Inputs: `preference` and `prefersDark` system flag.
 *
 * Outputs: `light` or `dark` resolved mode.
 *
 * Side effects: None.
 *
 * Error handling: None.
 *
 * Ties to other methods: Used by `useColorMode` and tests.
 *
 * Why this exists: Maintain one deterministic mode-resolution rule.
 */
export function resolveColorMode(
  preference: ColorModePreference,
  prefersDark: boolean,
): ResolvedColorMode {
  if (preference === 'light') return 'light';
  if (preference === 'dark') return 'dark';
  return prefersDark ? 'dark' : 'light';
}

/**
 * Summary: Read the persisted color-mode preference from local storage.
 *
 * Inputs: None.
 *
 * Outputs: Stored preference or `auto` when unavailable/invalid.
 *
 * Side effects: Reads browser local storage.
 *
 * Error handling: Returns `auto` when storage cannot be read.
 *
 * Ties to other methods: Used for `useColorMode` initialization.
 *
 * Why this exists: Provide resilient preference hydration across browser and desktop contexts.
 */
export function readColorModePreference(): ColorModePreference {
  try {
    if (typeof window === 'undefined') {
      return 'auto';
    }
    const storedValue = window.localStorage.getItem(COLOR_MODE_STORAGE_KEY);
    return isColorModePreference(storedValue) ? storedValue : 'auto';
  } catch {
    return 'auto';
  }
}

/**
 * Summary: Persist color-mode preference in local storage.
 *
 * Inputs: `preference` value to persist.
 *
 * Outputs: None.
 *
 * Side effects: Writes browser local storage when available.
 *
 * Error handling: Swallows storage errors because preference persistence is non-critical.
 *
 * Ties to other methods: Used by `useColorMode` setter.
 *
 * Why this exists: Keep mode preference stable across app restarts.
 */
export function writeColorModePreference(preference: ColorModePreference): void {
  try {
    if (typeof window === 'undefined') {
      return;
    }
    window.localStorage.setItem(COLOR_MODE_STORAGE_KEY, preference);
  } catch {
    // Best effort persistence only.
  }
}

/**
 * Summary: Read OS dark-mode preference using matchMedia.
 *
 * Inputs: None.
 *
 * Outputs: True when the OS currently prefers dark mode.
 *
 * Side effects: Reads browser media state.
 *
 * Error handling: Returns false when browser APIs are unavailable.
 *
 * Ties to other methods: Used by `useColorMode` and tests.
 *
 * Why this exists: Support `auto` mode based on platform appearance.
 */
export function systemPrefersDark(): boolean {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
    return false;
  }
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

/**
 * Summary: Subscribe to OS dark-mode preference changes.
 *
 * Inputs: Callback invoked with the latest dark preference state.
 *
 * Outputs: Cleanup function that removes listeners.
 *
 * Side effects: Registers/removes media query listeners.
 *
 * Error handling: Returns a no-op cleanup when matchMedia is unavailable.
 *
 * Ties to other methods: Used by `useColorMode` effect lifecycle.
 *
 * Why this exists: Keep `auto` mode reactive to OS appearance updates.
 */
export function observeSystemColorScheme(onChange: (prefersDark: boolean) => void): () => void {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
    return () => {};
  }

  const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
  const handleChange = (event: MediaQueryListEvent) => {
    onChange(event.matches);
  };

  if (typeof mediaQuery.addEventListener === 'function') {
    mediaQuery.addEventListener('change', handleChange);
    return () => mediaQuery.removeEventListener('change', handleChange);
  }

  mediaQuery.addListener(handleChange);
  return () => mediaQuery.removeListener(handleChange);
}

/**
 * Summary: Provide color-mode preference state and apply resolved theme tokens.
 *
 * Inputs: None.
 *
 * Outputs: Current preference, resolved mode, and setter.
 *
 * Side effects: Persists user preference and writes theme tokens to the document root.
 *
 * Error handling: Uses safe fallbacks when storage or media APIs are unavailable.
 *
 * Ties to other methods: Consumed by top-level app shell and theme controls.
 *
 * Why this exists: Keep mode state, persistence, and DOM token application in one hook.
 */
export function useColorMode(): {
  preference: ColorModePreference;
  resolvedMode: ResolvedColorMode;
  setPreference: (next: ColorModePreference) => void;
} {
  const [preference, setPreferenceState] = useState<ColorModePreference>(() =>
    readColorModePreference(),
  );
  const [prefersDark, setPrefersDark] = useState<boolean>(() => systemPrefersDark());

  useEffect(() => {
    return observeSystemColorScheme((nextPrefersDark) => {
      setPrefersDark(nextPrefersDark);
    });
  }, []);

  const resolvedMode = useMemo<ResolvedColorMode>(() => {
    return resolveColorMode(preference, prefersDark);
  }, [preference, prefersDark]);

  useEffect(() => {
    applyThemeTokens(resolvedMode);
    if (typeof document !== 'undefined') {
      document.documentElement.dataset.colorModePreference = preference;
    }
  }, [preference, resolvedMode]);

  /**
   * Summary: Update preference state and persist it for future sessions.
   *
   * Inputs: `next` preference chosen by the operator.
   *
   * Outputs: None.
   *
   * Side effects: Updates React state and local storage.
   *
   * Error handling: Persistence failures are handled in `writeColorModePreference`.
   *
   * Ties to other methods: Called by segmented color mode controls.
   *
   * Why this exists: Keep setter behavior consistent across all callers.
   */
  const setPreference = useCallback((next: ColorModePreference) => {
    setPreferenceState(next);
    writeColorModePreference(next);
  }, []);

  return { preference, resolvedMode, setPreference };
}
