import { correlationId } from './correlation';
import { Config } from '../domain/config';
import { safeInvoke, wrapError } from './ipc';

const FIXED_AUTOMATIC_INTERVAL_SECONDS = 30 * 60;

/**
 * Summary: Load the current configuration from the backend.
 *
 * Inputs: None.
 *
 * Outputs: The current `Config` object.
 *
 * Side effects: Invokes IPC calls to the backend.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: Settings screens and initial app bootstrap.
 *
 * Why this exists: Displays and edits configuration in the UI.
 */
export async function loadConfig(): Promise<Config> {
  try {
    return await safeInvoke<Config>('load_config_cmd');
  } catch (error) {
    throw wrapError('[loadConfig] Failed to load configuration', error);
  }
}

/**
 * Summary: Save a configuration update to the backend.
 *
 * Inputs: `cfg` as the updated configuration object.
 *
 * Outputs: Resolves when the save completes.
 *
 * Side effects: Invokes IPC calls that persist configuration.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: Settings persistence workflows and config validation.
 *
 * Why this exists: Persists configuration changes from the UI.
 */
export async function saveConfig(cfg: Config): Promise<void> {
  try {
    const nextConfig = { ...cfg, interval_seconds: FIXED_AUTOMATIC_INTERVAL_SECONDS };
    return await safeInvoke('save_config_cmd', {
      cfg: nextConfig,
      correlationId: correlationId('save'),
    });
  } catch (error) {
    throw wrapError('[saveConfig] Failed to save configuration', error);
  }
}
