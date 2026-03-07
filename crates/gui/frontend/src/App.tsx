import { useCallback } from 'react';
import { MinimalMain } from './components/minimal/MinimalMain';
import { ColorModeControl } from './components/ui/ColorModeControl';
import { useColorMode } from './theme/colorMode';
import { useNativePrewarm } from './app-shell/hooks/useNativePrewarm';

/**
 * Summary: Render the top-level application shell and coordinate shared UI state.
 *
 * Inputs: None.
 *
 * Outputs: A React element tree that wires the minimal desktop shell.
 *
 * Side effects: Registers React state hooks for color mode and native prewarm.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: `MinimalMain`, `ColorModeControl`, and `useNativePrewarm`.
 *
 * Why this exists: Centralizes cross-panel state so activity logging and safe mode stay consistent.
 */
export default function App() {
  const {
    preference: colorModePreference,
    resolvedMode,
    setPreference: setColorModePreference,
  } = useColorMode();
  useNativePrewarm();
  const recordEvent = useCallback((_msg: string, _kind?: 'ok' | 'error' | 'info') => {
    // Minimal mode owns visible feedback. The app shell only keeps a stable callback boundary.
  }, []);

  const colorModeDock = (
    <div
      className="mode-switch-shell"
      role="region"
      aria-label={`Color mode control (${resolvedMode})`}
    >
      <ColorModeControl preference={colorModePreference} onChange={setColorModePreference} />
    </div>
  );

  try {
    return (
      <>
        {colorModeDock}
        <MinimalMain onEvent={recordEvent} />
      </>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[App] Failed to render application shell: ${reason}`);
  }
}
