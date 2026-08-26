import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './styles.css';
import { resolveColorMode, systemPrefersDark } from './theme/colorMode';
import { applyThemeTokens } from './theme/tokens';

/** Keeps startup wiring in one function for safer initialization and error context. */
function mountApp(): void {
  try {
    const initialMode = resolveColorMode('auto', systemPrefersDark());
    applyThemeTokens(initialMode);

    const root = document.getElementById('root');
    if (!root) {
      throw new Error('Root element #root was not found');
    }
    ReactDOM.createRoot(root as HTMLElement).render(
      <React.StrictMode>
        <App />
      </React.StrictMode>,
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[mountApp] Failed to mount React application: ${reason}`);
  }
}

mountApp();
