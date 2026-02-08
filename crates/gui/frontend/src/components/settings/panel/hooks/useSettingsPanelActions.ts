import { useMemo } from 'react';
import { DestinationCheck } from '../../../../services/types';
import { AccessProbe } from '../../../../services/types';
import { Config } from '../../types';
import { useDestinationActions } from './useDestinationActions';
import { useMaintenanceActions } from './useMaintenanceActions';
import { useOnboardingFinish } from './useOnboardingFinish';
import { usePanelPickers } from './usePanelPickers';
import { useSpecialDirectories } from './useSpecialDirectories';
import { useWatchedActions } from './useWatchedActions';

type PickerHelpers = {
  pickDestination: (onChosen: (path: string) => void) => Promise<void>;
};

type AddWatched = (path: string, kind: 'File' | 'Directory', destId: string) => void;

type OnboardingState = {
  setDone: () => void;
};

type Params = {
  cfg: Config;
  setCfg: (cfg: Config) => void;
  saveCfg: (cfg: Config, message?: string) => void;
  status: string;
  setStatus: (msg: string) => void;
  validation: string | null;
  destStatus: DestinationCheck | null;
  setDoctorMsg: (msg: string) => void;
  setStartOnLoginMsg: (msg: string) => void;
  setSimulateMsg: (msg: string) => void;
  setAccessResult: (res: AccessProbe | null) => void;
  setAccessError: (err: string | null) => void;
  setResumeOnSpace: (next: boolean) => void;
  pickerHelpers: PickerHelpers;
  addWatched: AddWatched;
  onboarding: OnboardingState;
  popup: (msg: string) => void;
};

type Actions = {
  canFinishOnboarding: boolean;
  pickPath: (kind: 'File' | 'Directory') => Promise<void>;
  pickBackupRoot: () => Promise<void>;
  addDestination: () => Promise<void>;
  addPathToDestination: (destId: string, kind: 'File' | 'Directory') => Promise<void>;
  setDestinationRetention: (destId: string, v: number) => void;
  setDestinationLabel: (destId: string, label: string) => void;
  quickAdd: (getter: () => Promise<string>, label: string) => Promise<void>;
  quickAddPathToBackup: (getter: () => Promise<string>, label: string) => Promise<void>;
  enableStartOnLogin: () => Promise<void>;
  persistResumeOnSpace: (next: boolean) => void;
  runSimulate: () => Promise<void>;
  exportDoctor: () => Promise<void>;
  finishOnboarding: () => void;
  runAccessTest: () => Promise<void>;
  specialDir: (kind: 'desktop' | 'documents' | 'downloads') => Promise<string>;
};

/**
 * Summary: Compose all settings-panel action handlers from smaller, focused hooks.
 *
 * Inputs: Panel state setters, persistence helpers, picker helpers, and popup handler.
 * Outputs: A stable set of callbacks used by settings + onboarding views.
 * Side effects: Opens native pickers, persists config, writes localStorage, and invokes IPC services.
 * Error handling: Emits actionable messages through `popup` and panel status setters.
 * Ties to other methods: Wired into `SettingsPanelOnboarding` and `SettingsPanelContent`.
 * Why this exists: Keep the top-level panel orchestration small while behavior stays centralized.
 */
export function useSettingsPanelActions(params: Params): Actions {
  const {
    cfg,
    setCfg,
    saveCfg,
    status,
    setStatus,
    validation,
    destStatus,
    setDoctorMsg,
    setStartOnLoginMsg,
    setSimulateMsg,
    setAccessResult,
    setAccessError,
    setResumeOnSpace,
    pickerHelpers,
    addWatched,
    onboarding,
    popup,
  } = params;

  const primaryDestinationId = useMemo(() => cfg.destinations?.[0]?.id || 'default', [cfg.destinations]);
  const canFinishOnboarding = cfg.watched.length > 0 && !!destStatus?.writable;

  const { specialDir } = useSpecialDirectories();
  const { pickSinglePath } = usePanelPickers({ popup });

  const destinationActions = useDestinationActions({
    cfg,
    setCfg,
    saveCfg,
    setStatus,
    popup,
    pickerHelpers,
  });

  const watchedActions = useWatchedActions({
    primaryDestinationId,
    addWatched,
    setStatus,
    popup,
    pickSinglePath,
    setBackupRootPath: destinationActions.setBackupRootPath,
  });

  const maintenanceActions = useMaintenanceActions({
    setStartOnLoginMsg,
    setSimulateMsg,
    setStatus,
    setDoctorMsg,
    setAccessResult,
    setAccessError,
    setResumeOnSpace,
    popup,
  });

  const onboardingFinish = useOnboardingFinish({
    canFinishOnboarding,
    setDone: onboarding.setDone,
    setStatus,
  });

  void status;
  void validation;

  return {
    canFinishOnboarding,
    pickPath: watchedActions.pickPath,
    pickBackupRoot: destinationActions.pickBackupRoot,
    addDestination: destinationActions.addDestination,
    addPathToDestination: watchedActions.addPathToDestination,
    setDestinationRetention: destinationActions.setDestinationRetention,
    setDestinationLabel: destinationActions.setDestinationLabel,
    quickAdd: watchedActions.quickAdd,
    quickAddPathToBackup: watchedActions.quickAddPathToBackup,
    enableStartOnLogin: maintenanceActions.enableStartOnLogin,
    persistResumeOnSpace: maintenanceActions.persistResumeOnSpace,
    runSimulate: maintenanceActions.runSimulate,
    exportDoctor: maintenanceActions.exportDoctor,
    finishOnboarding: onboardingFinish.finishOnboarding,
    runAccessTest: maintenanceActions.runAccessTest,
    specialDir,
  };
}
