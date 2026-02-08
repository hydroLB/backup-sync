import { UI_TUNING } from '../config/uiTuning';
import { Config } from '../components/settings/types';

/**
 * Purpose: Build a stable fingerprint of configuration that affects hardening checks.
 *
 * Inputs: Current config object.
 * Outputs: A stable string fingerprint that changes when relevant fields change.
 * Ties to: Hardening completion persistence in localStorage.
 * Side effects: None.
 * Why: Hardening results should be invalidated when watched paths or destinations change.
 */
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
  return [`minFree=${minFree}`, `safety=${safety}`, `snapT=${snapshotTimeout}`, destinations, watched]
    .filter((s) => s.length > 0)
    .join('|');
}

/**
 * Purpose: Read the persisted hardening fingerprint from localStorage.
 *
 * Inputs: None.
 * Outputs: Stored fingerprint string or null.
 * Side effects: Reads localStorage.
 * Error handling: Returns null when storage access fails.
 * Ties to: Hardening gating in UI flows.
 * Why: Avoid repeatedly forcing hardening checks when the config is unchanged.
 */
export function readHardeningDone(): string | null {
  try {
    return localStorage.getItem(UI_TUNING.hardeningDoneStorageKey);
  } catch {
    return null;
  }
}

/**
 * Purpose: Persist a hardening completion fingerprint to localStorage.
 *
 * Inputs: Fingerprint string.
 * Outputs: None.
 * Side effects: Writes localStorage.
 * Error handling: Swallows storage errors to avoid breaking UI flows.
 * Ties to: Hardening gating in UI flows.
 * Why: Hardening should remain satisfied across restarts until config changes.
 */
export function writeHardeningDone(fingerprint: string): void {
  try {
    localStorage.setItem(UI_TUNING.hardeningDoneStorageKey, fingerprint);
  } catch {
    // Ignore: hardening still works in-memory, persistence is best-effort.
  }
}

/**
 * Purpose: Clear the persisted hardening completion state.
 *
 * Inputs: None.
 * Outputs: None.
 * Side effects: Writes localStorage.
 * Error handling: Swallows storage errors.
 * Ties to: Hardening invalidation when config changes significantly.
 * Why: Provide an explicit reset path for debugging and UI flows.
 */
export function clearHardeningDone(): void {
  try {
    localStorage.removeItem(UI_TUNING.hardeningDoneStorageKey);
  } catch {
    // Ignore.
  }
}

/**
 * Purpose: Determine whether hardening is satisfied for the current config.
 *
 * Inputs: Config object.
 * Outputs: Boolean indicating hardening completion is still valid.
 * Side effects: Reads localStorage.
 * Error handling: Returns false on errors.
 * Ties to: Onboarding gating and minimal UI running toggle gating.
 * Why: Do not enable scheduling if config has changed since the last successful hardening run.
 */
export function isHardeningSatisfied(cfg: Config): boolean {
  try {
    const stored = readHardeningDone();
    if (!stored) return false;
    return stored === hardeningFingerprint(cfg);
  } catch {
    return false;
  }
}

