import React, { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/api/dialog";
import { doctorReport, installService } from "../services";
import { Config, Destination } from "./settings/types";
import { formatBytes } from "../utils/format";
import EmptyState from "./settings/EmptyState";
import BackupCadence from "./settings/BackupCadence";
import DestinationBoard from "./settings/destinations/DestinationBoard";
import PerformanceControls from "./PerformanceControls";
import OnboardingOverlay from "./settings/OnboardingOverlay";
import AccessTest from "./settings/AccessTest";
import SafeModeToggle from "./settings/SafeModeToggle";
import FreeSpaceGuard from "./settings/FreeSpaceGuard";
import SaveBar from "./settings/SaveBar";
import OnboardingSummary from "./settings/OnboardingSummary";
import { useSettingsState } from "./settings/hooks/useSettingsState";
import { usePickers } from "./settings/hooks/usePickers";
import { useOnboardingState } from "./settings/useOnboarding";
import { useWatchActions } from "./settings/useWatchActions";
import { tauriAvailable } from "../services/ipc";
import { UI_TUNING } from "../config/uiTuning";
import { AccessProbe, SimulationResult } from "../services/types";
import AuthLockModal from "./settings/AuthLockModal";
import { authStatus, lockSession, unlockSession, AuthStatus } from "../services/auth";
import { IpcError } from "../services/ipc";

/**
 * Purpose: Render the settings panel and onboarding flows.
 *
 * Inputs: None.
 * Outputs: A settings panel element with configuration controls.
 * Ties to: Settings hooks, onboarding, and persistence helpers.
 * Side effects: Registers React hooks and invokes backend services for settings actions.
 * Why: Provide a unified configuration surface for operators.
 */
export const SettingsPanel: React.FC = () => {
  const defaultCfg: Config = {
    backup_root: "",
    interval_seconds: 60,
    max_backups_per_file: 3,
    skip_hidden: true,
    ignore_patterns: [],
    max_parallel_copies: 2,
    max_bytes_per_second: null,
    min_free_space_bytes: null,
    hashing: {
      buffer_bytes: 64 * 1024,
      timeout_seconds: 30,
    },
    execution: {
      copy_buffer_bytes: 64 * 1024,
      copy_timeout_seconds: 300,
      free_space_safety_buffer_bytes: 10 * 1024 * 1024,
      recent_activity_cap: 50,
      retry_delays_ms: [100, 200, 400, 800],
      retry_jitter_pct: 0.2,
    },
    planning: {
      hash_check_interval: 5,
      max_plan_items: 20_000,
      scan_timeout_seconds: 300,
      scan_capacity_multiplier: 16,
    },
    runtime: {
      prune_interval_cycles: 10,
      verify_interval_seconds: 24 * 3600,
      watcher_debounce_seconds: 2,
      ipc_timeout_seconds: 5,
      service_command_timeout_seconds: 15,
      service_command_retry_delay_ms: 300,
      service_command_poll_interval_ms: 50,
      auth_unlock_seconds: 15 * 60,
      tray_tooltip_refresh_seconds: 10,
      log_tail_lines: 200,
      simulation_sample_limit: 10,
    },
    safe_mode: false,
    watched: [],
    destinations: [{ id: "default", label: "Primary", path: "", max_backups_per_file: null }],
  };
  const state = useSettingsState(defaultCfg, (msg) => state.setStatus(msg));
  const {
    cfg,
    setCfg,
    status,
    setStatus,
    validation,
    destStatus,
    doctorMsg,
    setDoctorMsg,
    startOnLoginMsg,
    setStartOnLoginMsg,
    simulateMsg,
    setSimulateMsg,
    resumeOnSpace,
    setResumeOnSpace,
    persist: saveCfg,
  } = state;
  const [accessResult, setAccessResult] = useState<AccessProbe | null>(null);
  const [accessError, setAccessError] = useState<string | null>(null);
  const [authInfo, setAuthInfo] = useState<AuthStatus>({ unlocked: false, seconds_left: null });
  const [authError, setAuthError] = useState<string | null>(null);
  const [authVisible, setAuthVisible] = useState<boolean>(false);
  const [passcode, setPasscode] = useState<string>("");
  const pendingAuthAction = useRef<null | (() => void)>(null);
  const onboarding = useOnboardingState(cfg, destStatus?.writable);
  /**
   * Purpose: Refresh the auth status from the backend.
   *
   * Inputs: None.
   * Outputs: Updates auth state and error messages.
   * Ties to: Settings auth guard and unlock modal state.
   * Side effects: Invokes IPC to fetch auth status and updates state.
   * Why: Keep auth state accurate before privileged actions.
   */
  const refreshAuthStatus = async () => {
    try {
      const status = await authStatus();
      setAuthInfo(status);
      setAuthError(null);
    } catch (error) {
      const reason =
        error instanceof IpcError && error.code === "TAURI_UNAVAILABLE"
          ? "IPC unavailable. Launch the desktop app (./start) instead of a browser."
          : error instanceof Error
            ? error.message
            : String(error);
      setAuthInfo({ unlocked: false, seconds_left: null });
      setAuthError(reason);
    }
  };
  /**
   * Purpose: Require auth before running a privileged action.
   *
   * Inputs: Label and action function.
   * Outputs: Runs the action or opens the auth modal.
   * Ties to: Save, service, and diagnostics actions.
   * Side effects: Updates auth modal state and queues pending actions.
   * Why: Prevent unauthorized changes without blocking the UI flow.
   */
  const guardAuth = (label: string, action: () => void | Promise<void>) => {
    try {
      if (authInfo.unlocked) {
        void action();
        return;
      }
      pendingAuthAction.current = () => {
        void action();
      };
      setAuthError(`[SettingsPanel::guardAuth] ${label} requires unlock`);
      setAuthVisible(true);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      setAuthError(`[SettingsPanel::guardAuth] Failed to guard auth: ${reason}`);
    }
  };
  /**
   * Purpose: Unlock the session using the entered passcode.
   *
   * Inputs: Current passcode entry.
   * Outputs: Updates auth state and resumes any pending action.
   * Ties to: `unlockSession` and guarded actions.
   * Side effects: Invokes IPC, updates auth state, and clears pending actions.
   * Why: Provide a single unlock handler with consistent error handling.
   */
  const handleUnlock = async () => {
    try {
      if (!passcode.trim()) {
        setAuthError("[SettingsPanel::handleUnlock] Passcode is required.");
        return;
      }
      const msg = await unlockSession(passcode);
      setPasscode("");
      await refreshAuthStatus();
      setAuthVisible(false);
      setStatus(msg);
      const pending = pendingAuthAction.current;
      pendingAuthAction.current = null;
      if (pending) {
        pending();
      }
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      setAuthError(`[SettingsPanel::handleUnlock] ${reason}`);
    }
  };
  /**
   * Purpose: Lock the current session.
   *
   * Inputs: None.
   * Outputs: Updates auth state and status message.
   * Ties to: `lockSession` and auth status refresh.
   * Side effects: Invokes IPC and updates local auth state.
   * Why: Allow users to revoke privileged access explicitly.
   */
  const handleLock = async () => {
    try {
      await lockSession();
      await refreshAuthStatus();
      setStatus("Session locked.");
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      setAuthError(`[SettingsPanel::handleLock] ${reason}`);
    }
  };
  /**
   * Purpose: Surface a status message to the settings UI.
   *
   * Inputs: Status message string.
   * Outputs: Updates local status state.
   * Ties to: Settings helper functions and error paths.
   * Side effects: Updates React state for status messaging.
   * Why: Keep status updates consistent across handlers.
   */
  const popup = (msg: string) => {
    try {
      setStatus(msg);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      setStatus(`[SettingsPanel::popup] Failed to set status: ${reason}`);
    }
  };
  const pickerHelpers = usePickers(popup);
  const saving = false;

  /**
   * Purpose: Update the backup root and primary destination path.
   *
   * Inputs: Backup path and optional label.
   * Outputs: Persists updated config and status messaging.
   * Ties to: Destination pickers and onboarding steps.
   * Side effects: Updates config state and triggers persistence.
   * Why: Keep the primary destination aligned with backup root.
   */
  const setBackupRootPath = (path: string, label: string) => {
    try {
      const existing: Destination[] = cfg.destinations && cfg.destinations.length > 0 ? cfg.destinations : [];
      const primary: Destination = existing[0] ?? { id: "default", label: "Primary", path, max_backups_per_file: null };
      const nextDests: Destination[] = [{ ...primary, path }, ...existing.slice(1)];
      saveCfg({ ...cfg, backup_root: path, destinations: nextDests }, `Backup location set${label ? ` to ${label}` : ""}`);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      popup(`[SettingsPanel::setBackupRootPath] Failed to set backup root: ${reason}`);
    }
  };

  /**
   * Purpose: Resolve a special directory and apply it to config.
   *
   * Inputs: Directory getter, label, onSuccess handler, and status prefix.
   * Outputs: Updates status and invokes the success callback.
   * Ties to: Quick add and quick destination actions.
   * Side effects: Invokes resolver callbacks and updates React state.
   * Why: Avoid repeating resolution logic across quick actions.
   */
  const resolveSpecialPath = async (
    getter: () => Promise<string>,
    label: string,
    onSuccess: (path: string) => void,
    statusPrefix: string
  ) => {
    try {
      const path = await getter();
      if (!path) {
        popup(`Couldn't find ${label}. Pick a folder instead.`);
        return;
      }
      onSuccess(path);
      setStatus(`${statusPrefix} ${label}`);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      popup(`[SettingsPanel::resolveSpecialPath] Failed to resolve ${label}: ${reason}`);
    }
  };

  /**
   * Purpose: Run and export a doctor report to the Desktop.
   *
   * Inputs: None.
   * Outputs: Updates doctor status messages.
   * Ties to: Doctor report action in the settings panel.
   * Side effects: Invokes IPC to generate a report and updates local state.
   * Why: Make diagnostics export available from settings.
   */
  const exportDoctor = async () => {
    guardAuth("Export doctor report", async () => {
      try {
        if (!window.confirm("Run a doctor report and save it to your Desktop?")) {
          return;
        }
        setDoctorMsg("Running doctor...");
        const path = await doctorReport();
        setDoctorMsg(`Doctor report saved to ${path}`);
      } catch (e) {
        const msg = `[SettingsPanel::exportDoctor] Failed to export doctor report: ${String(e)}`;
        setDoctorMsg(msg);
        popup(msg);
      }
    });
  };

  /**
   * Purpose: Resolve a special OS directory through Tauri.
   *
   * Inputs: Directory kind.
   * Outputs: Promise resolving to the directory path.
   * Ties to: Quick add and destination selection flows.
   * Side effects: Performs IPC calls to resolve system paths.
   * Why: Centralize special directory resolution logic.
   */
  const specialDir = async (kind: "desktop" | "documents" | "downloads") => {
    try {
      if (!tauriAvailable()) {
        throw new Error("[SettingsPanel::specialDir] IPC unavailable. Launch the app build to pick paths.");
      }
      const { desktopDir, documentDir, downloadDir } = await import("@tauri-apps/api/path");
      if (kind === "desktop") return desktopDir();
      if (kind === "documents") return documentDir();
      return downloadDir();
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      throw new Error(`[SettingsPanel::specialDir] Failed to resolve ${kind} directory: ${reason}`);
    }
  };

  /**
   * Purpose: Open a path picker and add a watched entry.
   *
   * Inputs: Picker kind (file or directory).
   * Outputs: Updates watch list and status messages.
   * Ties to: Add watch actions in settings and onboarding.
   * Side effects: Opens a native picker and updates config state.
   * Why: Provide a safe path picker with error handling.
   */
  const pickPath = async (kind: "File" | "Directory") => {
    try {
      if (!tauriAvailable()) {
        popup("Picker unavailable (IPC). Launch the Tauri app build to select paths.");
        return;
      }
      const destId = (cfg.destinations && cfg.destinations[0]?.id) || "default";
      const selection = await open({
        directory: kind === "Directory",
        multiple: false,
        title: kind === "Directory" ? "Choose folder to protect" : "Choose file to protect",
      });
      if (typeof selection === "string") {
        addWatched(selection, kind, destId);
        setStatus(`Added ${selection}`);
      } else {
        popup("No selection made or picker was closed.");
      }
    } catch (e) {
      popup(`[SettingsPanel::pickPath] Path picker failed: ${e}`);
    }
  };

  /**
   * Purpose: Pick the backup destination root.
   *
   * Inputs: None.
   * Outputs: Updates backup root state and status.
   * Ties to: Destination picker helper.
   * Side effects: Opens a native picker and updates config state.
   * Why: Centralize backup root selection logic.
   */
  const pickBackupRoot = async () => {
    try {
      await pickerHelpers.pickDestination((path) => setBackupRootPath(path, ""));
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      popup(`[SettingsPanel::pickBackupRoot] Failed to pick backup root: ${reason}`);
    }
  };

  /**
   * Purpose: Add a new destination using the picker.
   *
   * Inputs: None.
   * Outputs: Updates destinations and status messaging.
   * Ties to: Destination picker helper.
   * Side effects: Opens a native picker and updates config state.
   * Why: Allow multiple backup destinations.
   */
  const addDestination = async () => {
    try {
      await pickerHelpers.pickDestination((selection) => {
        const id = `dest-${Date.now()}`;
        const nextDests: Destination[] = [...(cfg.destinations || []), { id, path: selection, max_backups_per_file: null }];
        saveCfg({ ...cfg, destinations: nextDests, backup_root: nextDests[0]?.path || selection }, "Destination added");
        setStatus(`Destination added: ${selection}`);
      });
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      popup(`[SettingsPanel::addDestination] Failed to add destination: ${reason}`);
    }
  };

  /**
   * Purpose: Pick a path and assign it to a destination.
   *
   * Inputs: Destination id and item kind.
   * Outputs: Updates watch list and status messaging.
   * Ties to: Destination board add path actions.
   * Side effects: Opens a native picker and updates config state.
   * Why: Attach watched items to specific destinations.
   */
  const addPathToDestination = async (destId: string, kind: "File" | "Directory") => {
    try {
      if (!tauriAvailable()) {
        popup("Picker unavailable (IPC). Launch the Tauri app build to select paths.");
        return;
      }
      const selection = await open({
        directory: kind === "Directory",
        multiple: false,
        title: kind === "Directory" ? "Choose folder to protect" : "Choose file to protect",
      });
      if (typeof selection === "string") {
        addWatched(selection, kind, destId);
        setStatus(`Added ${selection}`);
      } else {
        popup("No selection made or picker was closed.");
      }
    } catch (e) {
      popup(`[SettingsPanel::addPathToDestination] Path picker failed: ${e}`);
    }
  };

  /**
   * Purpose: Update retention setting for a destination.
   *
   * Inputs: Destination id and retention value.
   * Outputs: Persists updated configuration.
   * Ties to: Destination board retention controls.
   * Side effects: Updates config state and triggers persistence.
   * Why: Allow per-destination retention tuning.
   */
  const setDestinationRetention = (destId: string, v: number) => {
    try {
      const nextDests = (cfg.destinations || []).map((d) => (d.id === destId ? { ...d, max_backups_per_file: v } : d));
      saveCfg({ ...cfg, destinations: nextDests }, "Destination retention updated");
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      popup(`[SettingsPanel::setDestinationRetention] Failed to update retention: ${reason}`);
    }
  };

  /**
   * Purpose: Quick add a special folder to watched paths.
   *
   * Inputs: Special folder getter and label.
   * Outputs: Updates watch list and status messaging.
   * Ties to: Quick add buttons in settings and onboarding.
   * Side effects: Resolves system paths and updates config state.
   * Why: Provide one click add flows for common directories.
   */
  const quickAdd = async (getter: () => Promise<string>, label: string) => {
    try {
      if (!tauriAvailable()) {
        popup("IPC unavailable. Launch the app build to use quick add.");
        return;
      }
      const destId = (cfg.destinations && cfg.destinations[0]?.id) || "default";
      await resolveSpecialPath(getter, label, (path) => addWatched(path, "Directory", destId), "Added");
    } catch (e) {
      popup(`[SettingsPanel::quickAdd] Quick add failed: ${e}`);
    }
  };

  /**
   * Purpose: Quick set the backup destination to a special folder.
   *
   * Inputs: Special folder getter and label.
   * Outputs: Updates backup root and status messaging.
   * Ties to: Quick destination buttons in onboarding.
   * Side effects: Resolves system paths and updates config state.
   * Why: Enable fast setup of a primary destination.
   */
  const quickAddPathToBackup = async (getter: () => Promise<string>, label: string) => {
    try {
      if (!tauriAvailable()) {
        popup("IPC unavailable. Launch the app build to set destination.");
        return;
      }
      await resolveSpecialPath(getter, label, (path) => setBackupRootPath(path, label), "Backup location set to");
    } catch (e) {
      popup(`[SettingsPanel::quickAddPathToBackup] Quick destination set failed: ${e}`);
    }
  };

  /**
   * Purpose: Enable start on login for the background service.
   *
   * Inputs: None.
   * Outputs: Updates status messaging.
   * Ties to: Service installation flow.
   * Side effects: Invokes IPC to install the service and updates state.
   * Why: Make service enablement accessible in settings.
   */
  const enableStartOnLogin = async () => {
    guardAuth("Enable start on login", async () => {
      try {
        setStartOnLoginMsg("Enabling start on login...");
        await installService();
        setStartOnLoginMsg("Enabled. The helper will start on login.");
      } catch (e) {
        const msg = `[SettingsPanel::enableStartOnLogin] Failed to enable start on login: ${String(e)}`;
        setStartOnLoginMsg(msg);
        popup(msg);
      }
    });
  };

  const canFinishOnboarding = cfg.watched.length > 0 && !!(destStatus?.writable);

  /**
   * Purpose: Persist resume on space preference to storage.
   *
   * Inputs: Desired resume flag.
   * Outputs: Updates local storage and state.
   * Ties to: Free space guard settings.
   * Side effects: Writes to localStorage and updates state.
   * Why: Keep resume preference consistent across sessions.
   */
  const persistResumeOnSpace = (next: boolean) => {
    try {
      setResumeOnSpace(next);
      localStorage.setItem(UI_TUNING.resumeOnSpaceStorageKey, next ? "1" : "0");
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      popup(`[SettingsPanel::persistResumeOnSpace] Failed to persist resume preference: ${reason}`);
    }
  };

  /**
   * Purpose: Run a backup simulation and surface results.
   *
   * Inputs: None.
   * Outputs: Updates simulation and status messages.
   * Ties to: Simulation action in onboarding and settings.
   * Side effects: Invokes IPC and updates component state.
   * Why: Provide immediate feedback on planned backups.
   */
  const runSimulate = async () => {
    try {
      setSimulateMsg("Simulating...");
      const { runSimulation } = await import("../services/backup");
      const res: SimulationResult = await runSimulation();
      const summary = `Simulation: ${res.items} items, ${formatBytes(res.bytes)}.`;
      const sample = res.sample && res.sample.length > 0 ? ` Sample: ${res.sample.join(", ")}` : "";
      const msg = `${summary}${sample}`;
      setSimulateMsg(msg);
      setStatus(msg);
    } catch (e) {
      const msg = `[SettingsPanel::runSimulate] Simulation failed: ${String(e)}`;
      setSimulateMsg(msg);
      setStatus(msg);
    }
  };

  /**
   * Purpose: Finalize onboarding when prerequisites are met.
   *
   * Inputs: None.
   * Outputs: Marks onboarding as completed.
   * Ties to: Onboarding overlay flow.
   * Side effects: Updates onboarding state in memory.
   * Why: Ensure onboarding only completes when required steps are done.
   */
  const finishOnboarding = () => {
    try {
      if (!canFinishOnboarding) return;
      onboarding.setDone();
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      setStatus(`[SettingsPanel::finishOnboarding] Failed to finish onboarding: ${reason}`);
    }
  };

  /**
   * Purpose: Run an access test against the configured destination.
   *
   * Inputs: None.
   * Outputs: Updates access test state and errors.
   * Ties to: Access test panel in settings.
   * Side effects: Invokes IPC and updates component state.
   * Why: Verify destination access before running backups.
   */
  const runAccessTest = async () => {
    try {
      const { testAccess } = await import("../services/system");
      const res: AccessProbe = await testAccess();
      setAccessResult(res);
      setAccessError(null);
    } catch (e) {
      setAccessError(`[SettingsPanel::runAccessTest] ${String(e)}`);
    }
  };

  const { toggleEnabled, remove, addWatchedPath: addWatched } = useWatchActions(cfg, saveCfg, setStatus, () => {});

  useEffect(() => {
    refreshAuthStatus();
  }, []);

  return (
    <div className="card settings-panel">
      <div className="inline-actions" style={{ marginBottom: 8, alignItems: "center" }}>
        <div className="pill" style={{ borderColor: authInfo.unlocked ? "#7bffae" : "#ffb86b", color: authInfo.unlocked ? "#7bffae" : "#ffb86b" }}>
          {authInfo.unlocked ? "Session unlocked" : "Session locked"}
        </div>
        {authInfo.seconds_left != null && authInfo.unlocked && (
          <span className="muted">Time left: {Math.max(1, Math.round(authInfo.seconds_left / 60))}m</span>
        )}
        {authInfo.unlocked ? (
          <button className="btn secondary" onClick={handleLock}>Lock</button>
        ) : (
          <button className="btn secondary" onClick={() => setAuthVisible(true)}>Unlock</button>
        )}
      </div>
      {authError && <div className="muted" style={{ marginBottom: 8 }}>{authError}</div>}
      {onboarding.showOnboarding ? (
        <OnboardingOverlay
          step={onboarding.step as 1 | 2 | 3 | 4}
          hasWatched={cfg.watched.length > 0}
          destStatus={destStatus}
          canFinish={canFinishOnboarding}
          retention={cfg.max_backups_per_file}
          watchedPaths={(cfg.watched || []).map((w) => w.path)}
          summary={`Watched: ${cfg.watched.length}, Destination: ${destStatus?.message ?? "Not set"}`}
          freeMessage={destStatus ? `Free space: ${destStatus.free_bytes != null ? formatBytes(destStatus.free_bytes) : "Checking…"}` : "Free space: Checking…"}
          safeMode={cfg.safe_mode}
          statusMessage={validation ?? status}
          startOnLoginMsg={startOnLoginMsg}
          onChangeRetention={(v) => setCfg({ ...cfg, max_backups_per_file: v })}
          onAddFolder={() => pickPath("Directory")}
          onAddFile={() => pickPath("File")}
          onQuickAddDesktop={() => quickAdd(() => specialDir("desktop"), "Desktop")}
          onQuickAddDocuments={() => quickAdd(() => specialDir("documents"), "Documents")}
          onQuickAddDownloads={() => quickAdd(() => specialDir("downloads"), "Downloads")}
          onPickDestination={pickBackupRoot}
          onUseDesktopDest={() => quickAddPathToBackup(() => specialDir("desktop"), "Desktop")}
          onUseDocumentsDest={() => quickAddPathToBackup(() => specialDir("documents"), "Documents")}
          onUseDownloadsDest={() => quickAddPathToBackup(() => specialDir("downloads"), "Downloads")}
          onStartOnLogin={enableStartOnLogin}
          onTestBackup={runSimulate}
          onNext={() => onboarding.setStep((s) => Math.min(4, s + 1))}
          onBack={() => onboarding.setStep((s) => Math.max(1, s - 1))}
          onFinish={finishOnboarding}
        />
      ) : (
      <div className="settings-content">
        <DestinationBoard
          destinations={cfg.destinations || []}
          watched={cfg.watched || []}
          onAddPath={(destId, path, kind) => addWatched(path, kind, destId)}
          onPickPath={addPathToDestination}
          onToggleEnabled={toggleEnabled}
          onRemove={remove}
          onAddDestination={addDestination}
          onSetDestRetention={setDestinationRetention}
          onSetDestLabel={(destId, label) => {
            const nextDests = (cfg.destinations || []).map((d) => (d.id === destId ? { ...d, label } : d));
            setCfg({ ...cfg, destinations: nextDests });
          }}
        />
        {cfg.watched.length === 0 && (
          <EmptyState
            status={status}
            onAddFolder={() => pickPath("Directory")}
            onQuickAddDesktop={() => quickAdd(() => specialDir("desktop"), "Desktop")}
            onQuickAddDocuments={() => quickAdd(() => specialDir("documents"), "Documents")}
            onQuickAddDownloads={() => quickAdd(() => specialDir("downloads"), "Downloads")}
          />
        )}

        <div className="divider" />
        <BackupCadence
          interval_seconds={cfg.interval_seconds}
          max_backups_per_file={cfg.max_backups_per_file}
          onChange={(data) => setCfg({ ...cfg, ...data })}
        />
        <OnboardingSummary
          watchedCount={cfg.watched.length}
          destinationMessage={destStatus?.message ?? "Not set"}
          freeBytes={destStatus?.free_bytes ?? null}
          safeMode={cfg.safe_mode}
          onSimulate={runSimulate}
        />
        <div className="divider" />
        <h3>Performance & filters</h3>
        <SafeModeToggle value={!!cfg.safe_mode} onChange={(v) => setCfg({ ...cfg, safe_mode: v })} />
        <PerformanceControls
          skip_hidden={cfg.skip_hidden}
          ignore_patterns={cfg.ignore_patterns}
          max_parallel_copies={cfg.max_parallel_copies}
          max_bytes_per_second={cfg.max_bytes_per_second}
          min_free_space_bytes={cfg.min_free_space_bytes}
          onChange={(data) => setCfg({ ...cfg, ...data })}
          onStatus={setStatus}
        />
        <FreeSpaceGuard
          minFree={cfg.min_free_space_bytes}
          resumeOnSpace={resumeOnSpace}
          onToggleResume={persistResumeOnSpace}
        />
        <div className="divider" />
        <AccessTest
          onTest={runAccessTest}
          result={accessResult}
          error={accessError}
        />
        <div className="sticky-actions">
          <button className="btn" onClick={() => guardAuth("Save settings", () => saveCfg(cfg, "Settings saved"))} disabled={saving}>Save settings</button>
          <span className="muted">{validation ?? status}</span>
        </div>
        <div className="muted" style={{ marginTop: 4 }}>
          {validation ? `Why can't I save? ${validation}` : "All required fields look good."}
        </div>
        {simulateMsg && <div className="muted" style={{ marginTop: 4 }}>{simulateMsg}</div>}
        <div className="inline-actions" style={{ marginTop: 6 }}>
          <button className="btn secondary" onClick={exportDoctor}>Run doctor (export)</button>
          <span className="muted">{doctorMsg}</span>
        </div>
        <SaveBar
          disabled={saving}
          status={validation ?? status}
          onSave={() => guardAuth("Save settings", () => saveCfg(cfg, "Settings saved"))}
        />
      </div>
      )}
      <AuthLockModal
        visible={authVisible}
        passcode={passcode}
        unlockSeconds={cfg.runtime.auth_unlock_seconds}
        onChange={setPasscode}
        onUnlock={handleUnlock}
        onClose={() => {
          setAuthVisible(false);
          setPasscode("");
          pendingAuthAction.current = null;
        }}
      />
    </div>
  );
};
