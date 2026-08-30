import { useCallback, type MouseEvent } from 'react';
import { AppErrorBoundary } from './components/AppErrorBoundary';
import { MinimalMain } from './components/minimal/MinimalMain';
import { useSystemColorMode } from './theme/colorMode';
import { useNativePrewarm } from './app-shell/hooks/useNativePrewarm';
import { IS_WEB_RUNTIME } from './runtime/mode';

/** Centralizes cross-panel state so activity logging and safe mode stay consistent. */
function AppShell() {
  useSystemColorMode();
  useNativePrewarm();
  const recordEvent = useCallback((_msg: string, _kind?: 'ok' | 'error' | 'info') => {
    // Minimal mode owns visible feedback. The app shell only keeps a stable callback boundary.
  }, []);
  const startNativeWindowDrag = useCallback((event: MouseEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    void import('@tauri-apps/api/window').then(({ getCurrentWindow }) =>
      getCurrentWindow().startDragging(),
    );
  }, []);

  return (
    <>
      {!IS_WEB_RUNTIME && (
        <div
          className="native-titlebar-drag-region"
          data-tauri-drag-region
          aria-hidden="true"
          onMouseDown={startNativeWindowDrag}
        />
      )}
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
