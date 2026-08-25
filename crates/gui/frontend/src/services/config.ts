import { correlationId } from './correlation';
import { Config } from '../domain/config';
import { UI_TUNING } from '../config/uiTuning';
import { safeInvoke, safeInvokeWithTimeout, wrapError } from './ipc';

const FIXED_AUTOMATIC_INTERVAL_SECONDS = 30 * 60;

export type ConfigSaveResult = {
  daemon_restarted: boolean;
  daemon_restart_warning: string | null;
};

/** Displays and edits configuration in the UI. */
export async function loadConfig(): Promise<Config> {
  try {
    return await safeInvoke<Config>('load_config_cmd');
  } catch (error) {
    throw wrapError('[loadConfig] Failed to load configuration', error);
  }
}

/** Persists configuration changes from the UI. */
export async function saveConfig(cfg: Config): Promise<ConfigSaveResult> {
  try {
    const nextConfig = { ...cfg, interval_seconds: FIXED_AUTOMATIC_INTERVAL_SECONDS };
    return await safeInvokeWithTimeout<ConfigSaveResult>(
      'save_config_cmd',
      { cfg: nextConfig, correlationId: correlationId('save') },
      UI_TUNING.system.configSaveTimeoutMs,
    );
  } catch (error) {
    throw wrapError('[saveConfig] Failed to save configuration', error);
  }
}
