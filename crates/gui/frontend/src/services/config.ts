import { correlationId } from './correlation';
import { Config } from '../domain/config';
import { UI_TUNING } from '../config/uiTuning';
import { safeInvoke, safeInvokeWithTimeout, wrapError } from './ipc';

export type ConfigSaveResult = {
  daemon_restarted: boolean;
  daemon_restart_scheduled?: boolean;
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
    return await safeInvokeWithTimeout<ConfigSaveResult>(
      'save_config_cmd',
      { cfg, correlationId: correlationId('save') },
      UI_TUNING.system.configSaveTimeoutMs,
    );
  } catch (error) {
    throw wrapError('[saveConfig] Failed to save configuration', error);
  }
}

/** Persist removal only after the native store deletes that source's saved history. */
export async function saveConfigRemovingSource(
  cfg: Config,
  sourcePath: string,
  kind: 'File' | 'Directory',
): Promise<ConfigSaveResult> {
  try {
    return await safeInvokeWithTimeout<ConfigSaveResult>(
      'remove_protected_path_cmd',
      { cfg, sourcePath, kind, correlationId: correlationId('remove-protected-path') },
      UI_TUNING.system.configSaveTimeoutMs,
    );
  } catch (error) {
    throw wrapError('[saveConfigRemovingSource] Failed to remove protected path', error);
  }
}
