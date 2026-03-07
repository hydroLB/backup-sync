import { UI_TUNING } from '../config/uiTuning';
import { Config } from '../domain/config';

/**
 * Summary: Build a stable fingerprint of configuration that affects hardening checks.
 *
 * Inputs: Current config object.
 *
 * Outputs: A stable string fingerprint that changes when relevant fields change.
 *
 * Side effects: None.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: Hardening completion persistence in localStorage.
 *
 * Why this exists: Hardening results should be invalidated when watched paths or destinations change.
 */
export function hardeningFingerprint(cfg: Config): string {
  /**
   * Summary: watched orchestrates this method's core behavior.
   *
   * Inputs: Method parameters and required receiver state.
   *
   * Outputs: Return value and observable result for callers.
   *
   * Side effects: None beyond this method's explicit operations.
   *
   * Error handling: Propagates contextual errors to the caller when operations fail.
   *
   * Ties to other methods: Invoked by and composes with adjacent module methods.
   *
   * Why this exists: Keeps this behavior isolated, testable, and reusable.
   */

  const watched = (cfg.watched || [])
    .filter((w) => w.enabled)
    .map((w) => `${w.destination_id ?? ''}|${w.kind}|${w.path}`)
    .sort()
    .join('\n');
  /**
   * Summary: destinations orchestrates this method's core behavior.
   *
   * Inputs: Method parameters and required receiver state.
   *
   * Outputs: Return value and observable result for callers.
   *
   * Side effects: None beyond this method's explicit operations.
   *
   * Error handling: Propagates contextual errors to the caller when operations fail.
   *
   * Ties to other methods: Invoked by and composes with adjacent module methods.
   *
   * Why this exists: Keeps this behavior isolated, testable, and reusable.
   */

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

/**
 * Summary: Read the persisted hardening fingerprint from localStorage.
 *
 * Inputs: None.
 *
 * Outputs: Stored fingerprint string or null.
 *
 * Side effects: Reads localStorage.
 *
 * Error handling: Returns null when storage access fails.
 *
 * Ties to other methods: Hardening gating in UI flows.
 *
 * Why this exists: Avoid repeatedly forcing hardening checks when the config is unchanged.
 */
export function readHardeningDone(): string | null {
  try {
    return localStorage.getItem(UI_TUNING.hardeningDoneStorageKey);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.warn(`[hardening::readHardeningDone] Failed to read localStorage: ${reason}`);
    return null;
  }
}

/**
 * Summary: Persist a hardening completion fingerprint to localStorage.
 *
 * Inputs: Fingerprint string.
 *
 * Outputs: None.
 *
 * Side effects: Writes localStorage.
 *
 * Error handling: Swallows storage errors to avoid breaking UI flows.
 *
 * Ties to other methods: Hardening gating in UI flows.
 *
 * Why this exists: Hardening should remain satisfied across restarts until config changes.
 */
export function writeHardeningDone(fingerprint: string): void {
  try {
    localStorage.setItem(UI_TUNING.hardeningDoneStorageKey, fingerprint);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.warn(`[hardening::writeHardeningDone] Failed to persist localStorage value: ${reason}`);
  }
}

/**
 * Summary: Clear the persisted hardening completion state.
 *
 * Inputs: None.
 *
 * Outputs: None.
 *
 * Side effects: Writes localStorage.
 *
 * Error handling: Swallows storage errors.
 *
 * Ties to other methods: Hardening invalidation when config changes significantly.
 *
 * Why this exists: Provide an explicit reset path for debugging and UI flows.
 */
export function clearHardeningDone(): void {
  try {
    localStorage.removeItem(UI_TUNING.hardeningDoneStorageKey);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.warn(`[hardening::clearHardeningDone] Failed to clear localStorage value: ${reason}`);
  }
}

/**
 * Summary: Determine whether hardening is satisfied for the current config.
 *
 * Inputs: Config object.
 *
 * Outputs: Boolean indicating hardening completion is still valid.
 *
 * Side effects: Reads localStorage.
 *
 * Error handling: Returns false on errors.
 *
 * Ties to other methods: Onboarding gating and minimal UI running toggle gating.
 *
 * Why this exists: Do not enable scheduling if config has changed since the last successful hardening run.
 */
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
