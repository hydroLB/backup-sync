import { correlationId } from "./correlation";
import { Config } from "../components/settings/types";
import { safeInvoke } from "./ipc";

/**
 * Purpose: Load the current configuration from the backend.
 *
 * Inputs: None.
 * Outputs: The current `Config` object.
 * Ties to: Settings screens and initial app bootstrap.
 * Side effects: Invokes IPC calls to the backend.
 * Why: Displays and edits configuration in the UI.
 */
export async function loadConfig(): Promise<Config> {
  try {
    return await safeInvoke<Config>("load_config_cmd");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`[loadConfig] Failed to load configuration: ${message}`);
  }
}

/**
 * Purpose: Save a configuration update to the backend.
 *
 * Inputs: `cfg` as the updated configuration object.
 * Outputs: Resolves when the save completes.
 * Ties to: Settings persistence workflows and config validation.
 * Side effects: Invokes IPC calls that persist configuration.
 * Why: Persists configuration changes from the UI.
 */
export async function saveConfig(cfg: Config): Promise<void> {
  try {
    return await safeInvoke("save_config_cmd", { cfg, correlationId: correlationId("save") });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`[saveConfig] Failed to save configuration: ${message}`);
  }
}
