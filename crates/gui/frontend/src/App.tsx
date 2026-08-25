import { useCallback } from 'react';
import { AppErrorBoundary } from './components/AppErrorBoundary';
import { MinimalMain } from './components/minimal/MinimalMain';
import { ColorModeControl } from './components/ui/ColorModeControl';
import { useColorMode } from './theme/colorMode';
import { useNativePrewarm } from './app-shell/hooks/useNativePrewarm';

/** Centralizes cross-panel state so activity logging and safe mode stay consistent. */
function AppShell() {
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

  return (
    <>
      {colorModeDock}
      <MinimalMain onEvent={recordEvent} />
    </>
  );
}

export default function App() {
  return (
    <AppErrorBoundary>
      <AppShell />
    </AppErrorBoundary>
  );
}
