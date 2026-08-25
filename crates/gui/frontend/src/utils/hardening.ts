import { UI_TUNING } from '../config/uiTuning';
import { Config } from '../domain/config';
import { IS_WEB_RUNTIME } from '../runtime/mode';

/** Hardening results should be invalidated when watched paths or destinations change. */
export function hardeningFingerprint(cfg: Config): string {
  const watched = (cfg.watched || [])
    .filter((w) => w.enabled)
    .map((w) => `${w.destination_id ?? ''}|${w.kind}|${w.path}`)
    .sort()
    .join('\n');
  const destinations = (cfg.destinations || [])
    .map((d) => `${d.id}|${d.path}`)
    .sort()
    .join('\n');
  const minFree = cfg.min_free_space_bytes ?? '';
  const safety = cfg.execution?.free_space_safety_buffer_bytes ?? '';
  const snapshotTimeout = cfg.runtime?.source_snapshot_timeout_seconds ?? '';
  return [
    `minFree=${minFree}`,
    `safety=${safety}`,
    `snapT=${snapshotTimeout}`,
    destinations,
    watched,
  ]
    .filter((s) => s.length > 0)
    .join('|');
}

/** Avoid repeatedly forcing hardening checks when the config is unchanged. */
export function readHardeningDone(): string | null {
  try {
    if (IS_WEB_RUNTIME) return null;
    return localStorage.getItem(UI_TUNING.hardeningDoneStorageKey);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.warn(`[hardening::readHardeningDone] Failed to read localStorage: ${reason}`);
    return null;
  }
}

/** Hardening should remain satisfied across restarts until config changes. */
export function writeHardeningDone(fingerprint: string): void {
  try {
    if (IS_WEB_RUNTIME) return;
    localStorage.setItem(UI_TUNING.hardeningDoneStorageKey, fingerprint);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.warn(`[hardening::writeHardeningDone] Failed to persist localStorage value: ${reason}`);
  }
}

/** Provide an explicit reset path for debugging and UI flows. */
export function clearHardeningDone(): void {
  try {
    if (IS_WEB_RUNTIME) return;
    localStorage.removeItem(UI_TUNING.hardeningDoneStorageKey);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.warn(`[hardening::clearHardeningDone] Failed to clear localStorage value: ${reason}`);
  }
}

/** Do not enable scheduling if config has changed since the last successful hardening run. */
export function isHardeningSatisfied(cfg: Config): boolean {
  try {
    const stored = readHardeningDone();
    if (!stored) return false;
    return stored === hardeningFingerprint(cfg);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.warn(`[hardening::isHardeningSatisfied] Failed to evaluate hardening state: ${reason}`);
    return false;
  }
}
