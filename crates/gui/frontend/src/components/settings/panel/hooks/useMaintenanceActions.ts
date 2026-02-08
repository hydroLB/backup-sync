import { useCallback } from 'react';
import { doctorReport, installService } from '../../../../services';
import { AccessProbe, SimulationResult } from '../../../../services/types';
import { formatBytes } from '../../../../utils/format';
import { UI_TUNING } from '../../../../config/uiTuning';

type Params = {
  setStartOnLoginMsg: (msg: string) => void;
  setSimulateMsg: (msg: string) => void;
  setStatus: (msg: string) => void;
  setDoctorMsg: (msg: string) => void;
  setAccessResult: (res: AccessProbe | null) => void;
  setAccessError: (err: string | null) => void;
  setResumeOnSpace: (next: boolean) => void;
  popup: (msg: string) => void;
};

/**
 * Summary: Build maintenance and diagnostics actions for the settings panel.
 *
 * Inputs: Status setters, state setters, and popup handler.
 * Outputs: Action handlers for service install, simulation, doctor export, access test, and preference persistence.
 * Side effects: Invokes IPC services, writes localStorage, and updates component state.
 * Error handling: Emits contextual popup messages on failures.
 * Ties to other methods: Used by `useSettingsPanelActions` to compose the full action set.
 * Why this exists: Keep non-config-mutating operations grouped and easy to audit.
 */
export function useMaintenanceActions({
  setStartOnLoginMsg,
  setSimulateMsg,
  setStatus,
  setDoctorMsg,
  setAccessResult,
  setAccessError,
  setResumeOnSpace,
  popup,
}: Params) {
  const enableStartOnLogin = useCallback(async () => {
    try {
      setStartOnLoginMsg('Enabling start on login...');
      await installService();
      setStartOnLoginMsg('Enabled. The helper will start on login.');
    } catch (error) {
      const msg = `[useMaintenanceActions::enableStartOnLogin] Failed to enable start on login: ${String(error)}`;
      setStartOnLoginMsg(msg);
      popup(msg);
    }
  }, [popup, setStartOnLoginMsg]);

  const persistResumeOnSpace = useCallback(
    (next: boolean) => {
      try {
        setResumeOnSpace(next);
        localStorage.setItem(UI_TUNING.resumeOnSpaceStorageKey, next ? '1' : '0');
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        popup(
          `[useMaintenanceActions::persistResumeOnSpace] Failed to persist resume preference: ${reason}`,
        );
      }
    },
    [popup, setResumeOnSpace],
  );

  const runSimulate = useCallback(async () => {
    try {
      setSimulateMsg('Simulating...');
      const { runSimulation } = await import('../../../../services/backup');
      const res: SimulationResult = await runSimulation();
      const summary = `Simulation: ${res.items} items, ${formatBytes(res.bytes)}.`;
      const sample = res.sample && res.sample.length > 0 ? ` Sample: ${res.sample.join(', ')}` : '';
      const msg = `${summary}${sample}`;
      setSimulateMsg(msg);
      setStatus(msg);
    } catch (error) {
      const msg = `[useMaintenanceActions::runSimulate] Simulation failed: ${String(error)}`;
      setSimulateMsg(msg);
      setStatus(msg);
    }
  }, [setSimulateMsg, setStatus]);

  const exportDoctor = useCallback(async () => {
    try {
      if (!window.confirm('Run a doctor report and save it to your Desktop?')) {
        return;
      }
      setDoctorMsg('Running doctor...');
      const path = await doctorReport();
      setDoctorMsg(`Doctor report saved to ${path}`);
    } catch (error) {
      const msg = `[useMaintenanceActions::exportDoctor] Failed to export doctor report: ${String(error)}`;
      setDoctorMsg(msg);
      popup(msg);
    }
  }, [popup, setDoctorMsg]);

  const runAccessTest = useCallback(async () => {
    try {
      const { testAccess } = await import('../../../../services/system');
      const res: AccessProbe = await testAccess();
      setAccessResult(res);
      setAccessError(null);
    } catch (error) {
      setAccessError(`[useMaintenanceActions::runAccessTest] ${String(error)}`);
    }
  }, [setAccessError, setAccessResult]);

  return { enableStartOnLogin, persistResumeOnSpace, runSimulate, exportDoctor, runAccessTest };
}

