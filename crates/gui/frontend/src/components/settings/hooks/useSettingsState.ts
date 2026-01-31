import { useEffect, useState } from "react";
import { Config } from "../types";
import { loadConfig, checkDestination } from "../../../services";
import { DestinationCheck } from "../../../services/types";
import { useValidation } from "./useValidation";
import { useSettingsPersistence } from "./useSettingsPersistence";
import { UI_TUNING } from "../../../config/uiTuning";

/**
 * Purpose: Read resume-on-space preference from storage.
 *
 * Inputs: None.
 * Outputs: Boolean flag for resume behavior.
 * Ties to: Settings state initialization.
 * Side effects: Reads from localStorage.
 * Why: Keep the UI aligned with persisted operator preference.
 */
const readResumeOnSpace = (): boolean => {
  try {
    return localStorage.getItem(UI_TUNING.resumeOnSpaceStorageKey) === "1";
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.warn(`[useSettingsState::readResumeOnSpace] Failed to read local storage: ${reason}`);
    return false;
  }
};

/**
 * Purpose: Manage settings state, validation, and persistence wiring.
 *
 * Inputs: Default config and a popup handler for errors.
 * Outputs: Settings state, status setters, and persistence helpers.
 * Ties to: Settings panel and onboarding flows.
 * Side effects: Registers React hooks, loads config, and reads local storage.
 * Why: Centralize settings state management in one hook.
 */
export function useSettingsState(defaultCfg: Config, popup: (msg: string) => void) {
  const [cfg, setCfg] = useState<Config>(defaultCfg);
  const [initializing, setInitializing] = useState<boolean>(true);
  const [status, setStatus] = useState<string>("");
  const [validation, setValidation] = useState<string | null>(null);
  const [destStatus, setDestStatus] = useState<DestinationCheck | null>(null);
  const [doctorMsg, setDoctorMsg] = useState<string>("");
  const [startOnLoginMsg, setStartOnLoginMsg] = useState<string>("");
  const [simulateMsg, setSimulateMsg] = useState<string>("");
  const [resumeOnSpace, setResumeOnSpace] = useState<boolean>(readResumeOnSpace);

  const validationHelper = useValidation();
  const persistence = useSettingsPersistence(setStatus, setValidation);

  /**
   * Purpose: Ensure a valid destination list exists on the config.
   *
   * Inputs: Partial configuration payload.
   * Outputs: A config with at least one destination and destination ids.
   * Ties to: Settings persistence and onboarding defaults.
   * Side effects: None.
   * Why: Avoid invalid configs with missing destination wiring.
   */
  const ensureDestinations = (config: Config): Config => {
    try {
      const withDests =
        config.destinations && config.destinations.length > 0
          ? config
          : {
              ...config,
              destinations: [{ id: "default", label: "Primary", path: config.backup_root || "", max_backups_per_file: null }],
            };
      const primaryId = withDests.destinations?.[0]?.id || "default";
      return {
        ...withDests,
        watched: (withDests.watched || []).map((w) => ({ ...w, destination_id: w.destination_id || primaryId })),
      };
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      throw new Error(`[useSettingsState::ensureDestinations] Failed to normalize destinations: ${reason}`);
    }
  };

  /**
   * Purpose: Load the initial config into state.
   *
   * Inputs: None.
   * Outputs: Updates local state and initialization flag.
   * Ties to: Config loading and destination defaults.
   * Side effects: Loads config via IPC and updates React state.
   * Why: Provide a reliable starting point for the settings UI.
   */
  const loadInitialConfig = async () => {
    try {
      const config = ensureDestinations(await loadConfig());
      setCfg(config);
    } catch (e) {
      setStatus(`[useSettingsState::loadInitialConfig] Failed to load config: ${e}`);
    } finally {
      setInitializing(false);
    }
  };

  useEffect(() => {
    loadInitialConfig();
  }, []);

  useEffect(() => {
    /**
     * Purpose: Check destination health and update state.
     *
     * Inputs: None.
     * Outputs: Updates destination status state.
     * Ties to: Destination health indicator in settings.
     * Side effects: Invokes backend destination checks and updates React state.
     * Why: Surface writable and free space status to operators.
     */
    const check = async () => {
      if (!cfg) return;
      const primary = cfg.destinations && cfg.destinations[0] ? cfg.destinations[0].path : cfg.backup_root;
      if (primary) {
        try {
          const res = await checkDestination(primary);
          setDestStatus({ writable: !!res.writable, free_bytes: res.free_bytes ?? null, message: res.message });
        } catch (e) {
          setDestStatus({ writable: false, free_bytes: null, message: `[useSettingsState::checkDestination] ${String(e)}` });
        }
      }
    };
    check();
  }, [cfg.backup_root, cfg.destinations]);

  /**
   * Purpose: Persist a settings update with validation and normalization.
   *
   * Inputs: `next` config and optional status message.
   * Outputs: Updates local state and triggers persistence.
   * Ties to: Validation helper and persistence hook.
   * Side effects: Updates React state and persists config via IPC.
   * Why: Keep settings normalized before saving.
   */
  const persist = (next: Config, message = "Saved") => {
    try {
      const dedupedIgnores = Array.from(new Set(next.ignore_patterns || []));
      const validationMsg = validationHelper.validate(next, dedupedIgnores);
      const destinations =
        next.destinations && next.destinations.length > 0
          ? next.destinations
          : [{ id: "default", label: "Primary", path: next.backup_root, max_backups_per_file: null }];
      const nextCfg: Config = {
        ...next,
        destinations,
        backup_root: destinations[0]?.path || next.backup_root,
        ignore_patterns: dedupedIgnores,
        watched: (next.watched || []).map((w) => ({
          ...w,
          destination_id: w.destination_id || destinations[0]?.id || "default",
        })),
      };
      setCfg(nextCfg);
      persistence.persist(nextCfg, validationMsg, dedupedIgnores);
      if (message) {
        setStatus(message);
      }
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      popup(`[useSettingsState::persist] Failed to persist settings: ${reason}`);
    }
  };

  return {
    cfg,
    setCfg,
    initializing,
    status,
    setStatus,
    validation,
    setValidation,
    destStatus,
    setDestStatus,
    doctorMsg,
    setDoctorMsg,
    startOnLoginMsg,
    setStartOnLoginMsg,
    simulateMsg,
    setSimulateMsg,
    resumeOnSpace,
    setResumeOnSpace,
    persist,
  };
}
