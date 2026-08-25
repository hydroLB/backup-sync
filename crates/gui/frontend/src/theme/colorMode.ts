import { useCallback, useEffect, useMemo, useState } from 'react';
import { UI_TUNING } from '../config/uiTuning';
import { applyThemeTokens, ColorModePreference, ResolvedColorMode } from './tokens';

const COLOR_MODE_STORAGE_KEY = UI_TUNING.colorModeStorageKey;

/** Keep user preference parsing strict and predictable. */
export function isColorModePreference(value: unknown): value is ColorModePreference {
  return value === 'light' || value === 'dark' || value === 'auto';
}

/** Maintain one deterministic mode-resolution rule. */
export function resolveColorMode(
  preference: ColorModePreference,
  prefersDark: boolean,
): ResolvedColorMode {
  if (preference === 'light') return 'light';
  if (preference === 'dark') return 'dark';
  return prefersDark ? 'dark' : 'light';
}

/** Provide resilient preference hydration across browser and desktop contexts. */
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

/** Keep mode preference stable across app restarts. */
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

/** Support `auto` mode based on platform appearance. */
export function systemPrefersDark(): boolean {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
    return false;
  }
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
}

/** Keep `auto` mode reactive to OS appearance updates. */
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

/** Keep mode state, persistence, and DOM token application in one hook. */
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

  /** Keep setter behavior consistent across all callers. */
  const setPreference = useCallback((next: ColorModePreference) => {
    setPreferenceState(next);
    writeColorModePreference(next);
  }, []);

  return { preference, resolvedMode, setPreference };
}
