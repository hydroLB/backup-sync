import { useState } from "react";
import { Config } from "../types";
import { saveConfig } from "../../../services";

type SetStatus = (s: string) => void;
type SetValidation = (s: string | null) => void;

/**
 * Purpose: Provide settings persistence helpers with status tracking.
 *
 * Inputs: Status and validation setters.
 * Outputs: Persistence function and saving state.
 * Ties to: Settings state hook and save controls.
 * Side effects: Registers React state hooks and invokes config save IPC calls.
 * Why: Centralize persistence and saving state management.
 */
export function useSettingsPersistence(setStatus: SetStatus, setValidation: SetValidation) {
  try {
    const [saving, setSaving] = useState(false);

    /**
     * Purpose: Persist the config after validation.
     *
     * Inputs: Next config, validation message, and deduped ignore list.
     * Outputs: Updates saving and status state.
     * Ties to: Config save service and validation logic.
     * Side effects: Updates React state and invokes config save IPC calls.
     * Why: Ensure validated configs are persisted consistently.
     */
    const persist = (nextCfg: Config, validationMsg: string | null, dedupedIgnores: string[]) => {
      try {
        if (validationMsg) {
          setValidation(validationMsg);
          return;
        }
        setValidation(null);
        setSaving(true);
        saveConfig({ ...nextCfg, ignore_patterns: dedupedIgnores })
          .then(() => setStatus("Saved"))
          .catch((e) => setStatus(`[useSettingsPersistence::persist] ${String(e)}`))
          .finally(() => setSaving(false));
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        setStatus(`[useSettingsPersistence::persist] Failed to save config: ${reason}`);
        setSaving(false);
      }
    };

    return { persist, saving };
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    setStatus(`[useSettingsPersistence] Failed to initialize persistence: ${reason}`);
    return {
      persist: () => {
        setStatus(`[useSettingsPersistence] Persistence unavailable: ${reason}`);
      },
      saving: false,
    };
  }
}
