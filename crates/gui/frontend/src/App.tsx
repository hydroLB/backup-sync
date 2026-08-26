import { useCallback } from 'react';
import { AppErrorBoundary } from './components/AppErrorBoundary';
import { MinimalMain } from './components/minimal/MinimalMain';
import { useSystemColorMode } from './theme/colorMode';
import { useNativePrewarm } from './app-shell/hooks/useNativePrewarm';

/** Centralizes cross-panel state so activity logging and safe mode stay consistent. */
function AppShell() {
  useSystemColorMode();
  useNativePrewarm();
  const recordEvent = useCallback((_msg: string, _kind?: 'ok' | 'error' | 'info') => {
    // Minimal mode owns visible feedback. The app shell only keeps a stable callback boundary.
  }, []);

  return <MinimalMain onEvent={recordEvent} />;
}

export default function App() {
  return (
    <AppErrorBoundary>
      <AppShell />
    </AppErrorBoundary>
  );
}
