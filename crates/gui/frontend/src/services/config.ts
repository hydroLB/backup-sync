import { correlationId } from './correlation';
import { Config } from '../components/settings/types';
import { safeInvoke, wrapError } from './ipc';

const FIXED_AUTOMATIC_INTERVAL_SECONDS = 30 * 60;

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
    return await safeInvoke<Config>('load_config_cmd');
  } catch (error) {
    throw wrapError('[loadConfig] Failed to load configuration', error);
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
    return await safeInvoke('save_config_cmd', {
      cfg: { ...cfg, interval_seconds: FIXED_AUTOMATIC_INTERVAL_SECONDS },
      correlationId: correlationId('save'),
    });
  } catch (error) {
    throw wrapError('[saveConfig] Failed to save configuration', error);
  }
}
