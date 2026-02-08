import EmptyState from '../../EmptyState';
import BackupCadence from '../../BackupCadence';
import DestinationBoard from '../../destinations/DestinationBoard';
import OnboardingSummary from '../../OnboardingSummary';
import SafeModeToggle from '../../SafeModeToggle';
import FreeSpaceGuard from '../../FreeSpaceGuard';
import AccessTest from '../../AccessTest';
import SaveBar from '../../SaveBar';
import PerformanceControls from '../../../PerformanceControls';
import { Config, Destination, WatchedPath } from '../../types';
import { DestinationCheck, AccessProbe } from '../../../../services/types';
import { Button } from '../../../ui/Button';

type Props = {
  cfg: Config;
  setCfg: (cfg: Config) => void;
  status: string;
  setStatus: (msg: string) => void;
  validation: string | null;
  doctorMsg: string;
  simulateMsg: string;
  destStatus: DestinationCheck | null;
  saving: boolean;
  resumeOnSpace: boolean;
  accessResult: AccessProbe | null;
  accessError: string | null;
  toggleEnabled: (path: string) => void;
  remove: (path: string) => void;
  addWatched: (path: string, kind: 'File' | 'Directory', destId: string) => void;
  onPickPath: (destId: string, kind: 'File' | 'Directory') => void;
  onAddDestination: () => void;
  onSetDestRetention: (destId: string, v: number) => void;
  onSetDestLabel: (destId: string, label: string) => void;
  onPickFolder: () => void;
  onQuickAddDesktop: () => void;
  onQuickAddDocuments: () => void;
  onQuickAddDownloads: () => void;
  onToggleResume: (next: boolean) => void;
  onSimulate: () => void;
  onRunAccessTest: () => void;
  onExportDoctor: () => void;
  onSave: () => void;
};

/**
 * Summary: Render the main settings content view (non-onboarding).
 *
 * Inputs: Current config, status/validation state, and action handlers.
 * Outputs: The settings content element tree.
 * Side effects: Calls handlers that may persist config or invoke IPC operations.
 * Error handling: Delegated to handlers and underlying hooks.
 * Ties to other methods: Used by `SettingsPanel` when onboarding is not active.
 * Why this exists: Keep `SettingsPanel` small by moving the content layout into a dedicated view component.
 */
export function SettingsPanelContent({
  cfg,
  setCfg,
  status,
  setStatus,
  validation,
  doctorMsg,
  simulateMsg,
  destStatus,
  saving,
  resumeOnSpace,
  accessResult,
  accessError,
  toggleEnabled,
  remove,
  addWatched,
  onPickPath,
  onAddDestination,
  onSetDestRetention,
  onSetDestLabel,
  onPickFolder,
  onQuickAddDesktop,
  onQuickAddDocuments,
  onQuickAddDownloads,
  onToggleResume,
  onSimulate,
  onRunAccessTest,
  onExportDoctor,
  onSave,
}: Props) {
  return (
    <div className="settings-content">
      <DestinationBoard
        destinations={cfg.destinations || ([] as Destination[])}
        watched={cfg.watched || ([] as WatchedPath[])}
        onAddPath={(destId, path, kind) => addWatched(path, kind, destId)}
        onPickPath={onPickPath}
        onToggleEnabled={toggleEnabled}
        onRemove={remove}
        onAddDestination={onAddDestination}
        onSetDestRetention={onSetDestRetention}
        onSetDestLabel={onSetDestLabel}
      />

      {cfg.watched.length === 0 && (
        <EmptyState
          status={status}
          onAddFolder={onPickFolder}
          onQuickAddDesktop={onQuickAddDesktop}
          onQuickAddDocuments={onQuickAddDocuments}
          onQuickAddDownloads={onQuickAddDownloads}
        />
      )}

      <div className="divider" />
      <BackupCadence
        max_backups_per_file={cfg.max_backups_per_file}
        onChange={(data) => setCfg({ ...cfg, ...data })}
      />
      <OnboardingSummary
        watchedCount={cfg.watched.length}
        destinationMessage={destStatus?.message ?? 'Not set'}
        freeBytes={destStatus?.free_bytes ?? null}
        safeMode={cfg.safe_mode}
        onSimulate={onSimulate}
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
        onToggleResume={onToggleResume}
      />

      <div className="divider" />
      <AccessTest onTest={onRunAccessTest} result={accessResult} error={accessError} />

      <div className="sticky-actions">
        <Button onClick={onSave} disabled={saving}>
          Save settings
        </Button>
        <span className="muted">{validation ?? status}</span>
      </div>

      <div className="muted mt-1">
        {validation ? `Why can't I save? ${validation}` : 'All required fields look good.'}
      </div>
      {simulateMsg && <div className="muted mt-1">{simulateMsg}</div>}

      <div className="inline-actions mt-2">
        <Button tone="secondary" onClick={onExportDoctor}>
          Run doctor (export)
        </Button>
        <span className="muted">{doctorMsg}</span>
      </div>

      <SaveBar disabled={saving} status={validation ?? status} onSave={onSave} />
    </div>
  );
}
