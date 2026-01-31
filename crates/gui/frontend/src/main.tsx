import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

/**
 * Purpose: Mount the React application into the DOM root element.
 *
 * Inputs: None.
 * Outputs: Renders `<App />` into the `#root` container.
 * Ties to: `App` for the UI shell and `styles.css` for global styling.
 * Side effects: Mounts React into the DOM and triggers initial render.
 * Why: Keeps startup wiring in one function for safer initialization and error context.
 */
function mountApp(): void {
  try {
    const root = document.getElementById("root");
    if (!root) {
      throw new Error("Root element #root was not found");
    }
    ReactDOM.createRoot(root as HTMLElement).render(
      <React.StrictMode>
        <App />
      </React.StrictMode>
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[mountApp] Failed to mount React application: ${reason}`);
  }
}

mountApp();
