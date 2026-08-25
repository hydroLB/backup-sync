import { applyThemeTokens, resolveThemeToken, themeColorVar } from '../../theme/tokens';
import {
  isColorModePreference,
  readColorModePreference,
  resolveColorMode,
  systemPrefersDark,
  writeColorModePreference,
} from '../../theme/colorMode';
import { UI_TUNING } from '../../config/uiTuning';

/** Keep tests deterministic and isolated. */
function resetThemeState(): void {
  window.localStorage.removeItem(UI_TUNING.colorModeStorageKey);
  document.documentElement.removeAttribute('data-color-mode');
  document.documentElement.removeAttribute('data-color-mode-preference');
  document.documentElement.style.removeProperty('--accent');
  document.documentElement.style.removeProperty('--color-scheme');
}

describe('color mode utilities', () => {
  beforeEach(() => {
    resetThemeState();
  });

  it('accepts only supported preference literals', () => {
    expect(isColorModePreference('light')).toBe(true);
    expect(isColorModePreference('dark')).toBe(true);
    expect(isColorModePreference('auto')).toBe(true);
    expect(isColorModePreference('nope')).toBe(false);
  });

  it('resolves auto mode using system preference', () => {
    expect(resolveColorMode('auto', true)).toBe('dark');
    expect(resolveColorMode('auto', false)).toBe('light');
    expect(resolveColorMode('dark', false)).toBe('dark');
    expect(resolveColorMode('light', true)).toBe('light');
  });

  it('persists and reloads mode preference', () => {
    writeColorModePreference('dark');
    expect(readColorModePreference()).toBe('dark');

    window.localStorage.setItem(UI_TUNING.colorModeStorageKey, 'invalid');
    expect(readColorModePreference()).toBe('auto');
  });

  it('applies theme tokens to the root element', () => {
    applyThemeTokens('light');
    expect(document.documentElement.dataset.colorMode).toBe('light');
    expect(document.documentElement.style.getPropertyValue('--color-scheme')).toBe('light');
    expect(document.documentElement.style.getPropertyValue('--accent')).toBe(
      resolveThemeToken('light', 'accent'),
    );
  });

  it('exposes token variables and default system preference checks', () => {
    expect(themeColorVar('accent')).toBe('var(--accent)');
    expect(typeof systemPrefersDark()).toBe('boolean');
  });
});
