export type ColorModePreference = 'light' | 'dark' | 'auto';
export type ResolvedColorMode = 'light' | 'dark';

const DARK_THEME_TOKENS = {
  'color-scheme': 'dark',
  'app-bg-start': '#050910',
  'app-bg-mid': '#050a16',
  'app-bg-end': '#050a18',
  'app-bg-overlay-1': 'rgba(20, 52, 110, 0.8)',
  'app-bg-overlay-2': 'rgba(18, 40, 86, 0.8)',
  'app-bg-overlay-3': 'rgba(10, 18, 42, 0.55)',
  bg: '#050a16',
  'surface-0': 'rgba(255, 255, 255, 0.03)',
  'surface-1': 'rgba(255, 255, 255, 0.05)',
  'surface-2': 'rgba(255, 255, 255, 0.08)',
  'surface-3': 'rgba(255, 255, 255, 0.12)',
  panel: 'rgba(255, 255, 255, 0.05)',
  'panel-strong': 'rgba(255, 255, 255, 0.09)',
  glass: 'rgba(14, 28, 60, 0.62)',
  'glass-strong': 'rgba(10, 22, 48, 0.78)',
  'glass-border': 'rgba(255, 255, 255, 0.14)',
  accent: '#0a84ff',
  'accent-strong': '#4da3ff',
  'accent-soft': 'rgba(10, 132, 255, 0.14)',
  'toggle-on': '#34c759',
  text: '#f5f8ff',
  muted: '#b5c5e2',
  danger: '#ff7b7b',
  warning: '#ffbf66',
  'warning-soft': 'rgba(255, 191, 102, 0.14)',
  success: '#8ff7c8',
  border: 'rgba(255, 255, 255, 0.12)',
  'focus-ring': 'rgba(90, 173, 255, 0.66)',
  'text-section-header': '#f5f8ff',
  'text-form-label': '#c6d4ed',
  'text-hint': '#a7b8d8',
  'text-empty-state': '#a9b9d8',
  'text-status': '#eaf2ff',
  'text-status-ready': '#d1f8e7',
  'text-status-working': '#c7e6ff',
  'text-status-success': '#d8ffe8',
  'text-status-error': '#ffb0b0',
  'surface-frame-outer': 'linear-gradient(160deg, rgba(14, 28, 60, 0.62), rgba(10, 22, 48, 0.78))',
  'surface-frame-inset': 'rgba(255, 255, 255, 0.04)',
  'border-frame-outer': 'rgba(255, 255, 255, 0.14)',
  'border-frame-inset': 'rgba(255, 255, 255, 0.1)',
  'control-bg': 'rgba(255, 255, 255, 0.06)',
  'control-bg-hover': 'rgba(255, 255, 255, 0.1)',
  'control-bg-active': 'rgba(77, 163, 255, 0.12)',
  'control-bg-pressed': 'rgba(255, 255, 255, 0.12)',
  'control-bg-disabled': 'rgba(255, 255, 255, 0.03)',
  'control-border': 'rgba(255, 255, 255, 0.14)',
  'control-border-hover': 'rgba(255, 255, 255, 0.24)',
  'control-border-active': 'rgba(77, 163, 255, 0.52)',
  'control-border-pressed': 'rgba(255, 255, 255, 0.3)',
  'control-border-disabled': 'rgba(255, 255, 255, 0.1)',
  'control-text': '#f5f8ff',
  'control-text-disabled': '#8697b8',
  'btn-primary-bg': '#0a84ff',
  'btn-primary-bg-hover': '#1f8fff',
  'btn-primary-bg-pressed': '#0676e6',
  'btn-primary-border': 'rgba(255, 255, 255, 0.12)',
  'btn-primary-shadow': '0 10px 24px rgba(10, 132, 255, 0.25)',
  'btn-secondary-bg': 'rgba(255, 255, 255, 0.08)',
  'btn-secondary-bg-hover': 'rgba(255, 255, 255, 0.12)',
  'btn-secondary-bg-pressed': 'rgba(255, 255, 255, 0.1)',
  'btn-secondary-border': 'rgba(255, 255, 255, 0.2)',
  'btn-secondary-shadow': 'none',
  'btn-secondary-text': '#f5f8ff',
  'btn-danger-bg': 'rgba(201, 69, 69, 0.95)',
  'btn-danger-bg-hover': 'rgba(221, 84, 84, 0.95)',
  'btn-danger-bg-pressed': 'rgba(181, 52, 52, 0.95)',
  'btn-danger-border': 'rgba(255, 161, 161, 0.28)',
  'btn-danger-shadow': '0 10px 24px rgba(172, 60, 60, 0.35)',
  'segmented-bg': 'rgba(255, 255, 255, 0.04)',
  'segmented-border': 'rgba(255, 255, 255, 0.16)',
  'segmented-btn-bg': 'transparent',
  'segmented-btn-hover-bg': 'rgba(255, 255, 255, 0.08)',
  'segmented-btn-selected-bg': 'rgba(77, 163, 255, 0.16)',
  'segmented-btn-selected-border': 'rgba(77, 163, 255, 0.42)',
  'segmented-btn-text': '#f5f8ff',
  'status-ready-bg': 'rgba(77, 163, 255, 0.12)',
  'status-ready-border': 'rgba(77, 163, 255, 0.45)',
  'status-ready-text': '#c8e8ff',
  'status-working-bg': 'rgba(255, 191, 102, 0.12)',
  'status-working-border': 'rgba(255, 191, 102, 0.48)',
  'status-working-text': '#ffe3ba',
  'status-success-bg': 'rgba(143, 247, 200, 0.12)',
  'status-success-border': 'rgba(143, 247, 200, 0.52)',
  'status-success-text': '#d8ffe8',
  'status-error-bg': 'rgba(255, 123, 123, 0.12)',
  'status-error-border': 'rgba(255, 123, 123, 0.58)',
  'status-error-text': '#ffb0b0',
  'input-placeholder': 'rgba(159, 176, 211, 0.7)',
  'overlay-backdrop': 'rgba(0, 0, 0, 0.55)',
  'log-bg': 'rgba(0, 0, 0, 0.22)',
  'scrollbar-track': 'rgba(255, 255, 255, 0.06)',
  'scrollbar-thumb': 'rgba(255, 255, 255, 0.2)',
  'scrollbar-thumb-hover': 'rgba(255, 255, 255, 0.3)',
  'control-strip-bg-start': 'rgba(255, 255, 255, 0.08)',
  'control-strip-bg-end': 'rgba(255, 255, 255, 0.04)',
  'control-strip-border': 'rgba(255, 255, 255, 0.16)',
  'run-log-preview-max-height': '200px',
  'run-log-preview-padding': '10px',
  'run-log-preview-radius': '10px',
  'table-heading-font-size': '13px',
  'table-row-height': '34px',
} as const;

export type ThemeTokenName = keyof typeof DARK_THEME_TOKENS;

const LIGHT_THEME_TOKENS: Record<ThemeTokenName, string> = {
  'color-scheme': 'light',
  'app-bg-start': '#f4f8ff',
  'app-bg-mid': '#edf3fd',
  'app-bg-end': '#e9f0fb',
  'app-bg-overlay-1': 'rgba(145, 184, 244, 0.5)',
  'app-bg-overlay-2': 'rgba(164, 201, 249, 0.42)',
  'app-bg-overlay-3': 'rgba(205, 225, 252, 0.55)',
  bg: '#edf3fd',
  'surface-0': 'rgba(12, 35, 79, 0.03)',
  'surface-1': 'rgba(12, 35, 79, 0.05)',
  'surface-2': 'rgba(12, 35, 79, 0.09)',
  'surface-3': 'rgba(12, 35, 79, 0.14)',
  panel: 'rgba(255, 255, 255, 0.74)',
  'panel-strong': 'rgba(255, 255, 255, 0.9)',
  glass: 'rgba(255, 255, 255, 0.76)',
  'glass-strong': 'rgba(255, 255, 255, 0.9)',
  'glass-border': 'rgba(22, 53, 111, 0.18)',
  accent: '#0a69d1',
  'accent-strong': '#055bb8',
  'accent-soft': 'rgba(10, 105, 209, 0.14)',
  'toggle-on': '#1f9d49',
  text: '#0c1f44',
  muted: '#46608f',
  danger: '#c64545',
  warning: '#ad6a12',
  'warning-soft': 'rgba(173, 106, 18, 0.14)',
  success: '#147b49',
  border: 'rgba(22, 53, 111, 0.2)',
  'focus-ring': 'rgba(10, 105, 209, 0.44)',
  'text-section-header': '#0c1f44',
  'text-form-label': '#2c4777',
  'text-hint': '#4a6390',
  'text-empty-state': '#4f6998',
  'text-status': '#0f274f',
  'text-status-ready': '#0e4d8f',
  'text-status-working': '#855309',
  'text-status-success': '#14643d',
  'text-status-error': '#a93a3a',
  'surface-frame-outer':
    'linear-gradient(160deg, rgba(255, 255, 255, 0.86), rgba(247, 251, 255, 0.98))',
  'surface-frame-inset': 'rgba(255, 255, 255, 0.7)',
  'border-frame-outer': 'rgba(22, 53, 111, 0.2)',
  'border-frame-inset': 'rgba(22, 53, 111, 0.18)',
  'control-bg': 'rgba(255, 255, 255, 0.82)',
  'control-bg-hover': 'rgba(255, 255, 255, 0.94)',
  'control-bg-active': 'rgba(10, 105, 209, 0.12)',
  'control-bg-pressed': 'rgba(222, 234, 252, 0.88)',
  'control-bg-disabled': 'rgba(239, 244, 251, 0.92)',
  'control-border': 'rgba(22, 53, 111, 0.26)',
  'control-border-hover': 'rgba(10, 105, 209, 0.45)',
  'control-border-active': 'rgba(10, 105, 209, 0.6)',
  'control-border-pressed': 'rgba(22, 53, 111, 0.42)',
  'control-border-disabled': 'rgba(121, 143, 181, 0.4)',
  'control-text': '#0c1f44',
  'control-text-disabled': '#7588ac',
  'btn-primary-bg': '#0a69d1',
  'btn-primary-bg-hover': '#055bb8',
  'btn-primary-bg-pressed': '#024fa2',
  'btn-primary-border': 'rgba(5, 70, 146, 0.34)',
  'btn-primary-shadow': '0 10px 22px rgba(10, 105, 209, 0.18)',
  'btn-secondary-bg': 'rgba(255, 255, 255, 0.84)',
  'btn-secondary-bg-hover': 'rgba(255, 255, 255, 0.96)',
  'btn-secondary-bg-pressed': 'rgba(232, 240, 252, 0.92)',
  'btn-secondary-border': 'rgba(22, 53, 111, 0.24)',
  'btn-secondary-shadow': 'none',
  'btn-secondary-text': '#0c1f44',
  'btn-danger-bg': '#d45a5a',
  'btn-danger-bg-hover': '#c64d4d',
  'btn-danger-bg-pressed': '#ad3f3f',
  'btn-danger-border': 'rgba(158, 38, 38, 0.35)',
  'btn-danger-shadow': '0 10px 24px rgba(170, 55, 55, 0.2)',
  'segmented-bg': 'rgba(255, 255, 255, 0.86)',
  'segmented-border': 'rgba(22, 53, 111, 0.24)',
  'segmented-btn-bg': 'transparent',
  'segmented-btn-hover-bg': 'rgba(10, 105, 209, 0.08)',
  'segmented-btn-selected-bg': 'rgba(10, 105, 209, 0.16)',
  'segmented-btn-selected-border': 'rgba(10, 105, 209, 0.4)',
  'segmented-btn-text': '#0c1f44',
  'status-ready-bg': 'rgba(10, 105, 209, 0.12)',
  'status-ready-border': 'rgba(10, 105, 209, 0.45)',
  'status-ready-text': '#0e4d8f',
  'status-working-bg': 'rgba(173, 106, 18, 0.12)',
  'status-working-border': 'rgba(173, 106, 18, 0.45)',
  'status-working-text': '#855309',
  'status-success-bg': 'rgba(20, 123, 73, 0.12)',
  'status-success-border': 'rgba(20, 123, 73, 0.44)',
  'status-success-text': '#14643d',
  'status-error-bg': 'rgba(198, 69, 69, 0.12)',
  'status-error-border': 'rgba(198, 69, 69, 0.48)',
  'status-error-text': '#a93a3a',
  'input-placeholder': 'rgba(74, 99, 144, 0.75)',
  'overlay-backdrop': 'rgba(21, 39, 73, 0.4)',
  'log-bg': 'rgba(245, 249, 255, 0.86)',
  'scrollbar-track': 'rgba(22, 53, 111, 0.08)',
  'scrollbar-thumb': 'rgba(22, 53, 111, 0.26)',
  'scrollbar-thumb-hover': 'rgba(22, 53, 111, 0.36)',
  'control-strip-bg-start': 'rgba(255, 255, 255, 0.84)',
  'control-strip-bg-end': 'rgba(255, 255, 255, 0.65)',
  'control-strip-border': 'rgba(22, 53, 111, 0.2)',
  'run-log-preview-max-height': '200px',
  'run-log-preview-padding': '10px',
  'run-log-preview-radius': '10px',
  'table-heading-font-size': '13px',
  'table-row-height': '34px',
};

const THEME_TOKENS: Record<ResolvedColorMode, Record<ThemeTokenName, string>> = {
  dark: DARK_THEME_TOKENS,
  light: LIGHT_THEME_TOKENS,
};

/**
 * Summary: Build a CSS variable expression for a registered theme token.
 *
 * Inputs: `token` as the token key.
 *
 * Outputs: CSS variable expression (for example `var(--accent)`).
 *
 * Side effects: None.
 *
 * Error handling: None.
 *
 * Ties to other methods: Used by view helpers that need token-based inline styles.
 *
 * Why this exists: Keep token access centralized and typo-safe.
 */
export function themeColorVar(token: ThemeTokenName): string {
  return `var(--${token})`;
}

/**
 * Summary: Resolve a token to its concrete color value for a mode.
 *
 * Inputs: `mode` as the resolved mode and `token` as the token key.
 *
 * Outputs: Token value for the selected mode.
 *
 * Side effects: None.
 *
 * Error handling: None.
 *
 * Ties to other methods: Used by tests and theme-aware utility functions.
 *
 * Why this exists: Avoid direct object access in callers and preserve one lookup path.
 */
export function resolveThemeToken(mode: ResolvedColorMode, token: ThemeTokenName): string {
  return THEME_TOKENS[mode][token];
}

/**
 * Summary: Return the full token map for a resolved color mode.
 *
 * Inputs: `mode` as the resolved color mode.
 *
 * Outputs: Immutable token map for the requested mode.
 *
 * Side effects: None.
 *
 * Error handling: None.
 *
 * Ties to other methods: Used by `applyThemeTokens` and theme tests.
 *
 * Why this exists: Keep mode specific token selection in one place.
 */
export function themeTokensForMode(mode: ResolvedColorMode): Record<ThemeTokenName, string> {
  return THEME_TOKENS[mode];
}

/**
 * Summary: Apply all resolved theme tokens to the document root.
 *
 * Inputs: `mode` as the active color mode and optional `root` element override.
 *
 * Outputs: None.
 *
 * Side effects: Writes CSS custom properties and `data-color-mode` on the root element.
 *
 * Error handling: No-op when DOM globals are unavailable.
 *
 * Ties to other methods: Called by color-mode lifecycle hooks during mode changes.
 *
 * Why this exists: Ensure every visual surface resolves through one token source.
 */
export function applyThemeTokens(mode: ResolvedColorMode, root?: HTMLElement): void {
  const targetRoot =
    root ?? (typeof document !== 'undefined' ? document.documentElement : undefined);
  if (!targetRoot) {
    return;
  }

  const tokens = themeTokensForMode(mode);
  for (const token of Object.keys(tokens) as ThemeTokenName[]) {
    targetRoot.style.setProperty(`--${token}`, tokens[token]);
  }
  targetRoot.dataset.colorMode = mode;
}
