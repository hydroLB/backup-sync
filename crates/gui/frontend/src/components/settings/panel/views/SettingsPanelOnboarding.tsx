import { formatBytes } from '../../../../utils/format';
import OnboardingOverlay from '../../OnboardingOverlay';
import { Config } from '../../types';
import { DestinationCheck, HardeningReport } from '../../../../services/types';
import { isHardeningSatisfied } from '../../../../utils/hardening';

type Props = {
  step: 1 | 2 | 3 | 4;
  setStep: (updater: (prev: number) => number) => void;
  cfg: Config;
  destStatus: DestinationCheck | null;
  canFinish: boolean;
  hardeningBusy: boolean;
  hardeningReport: HardeningReport | null;
  statusMessage: string;
  startOnLoginMsg: string;
  onChangeRetention: (v: number) => void;
  onAddFolder: () => void;
  onQuickAddDesktop: () => void;
  onQuickAddDocuments: () => void;
  onQuickAddDownloads: () => void;
  onPickDestination: () => void;
  onUseDesktopDest: () => void;
  onUseDocumentsDest: () => void;
  onUseDownloadsDest: () => void;
  onStartOnLogin: () => void;
  onTestBackup: () => void;
  onRunHardening: (checkSnapshots: boolean) => void;
  onFinish: () => void;
};

/**
 * Summary: Render the onboarding overlay with consistent derived messaging.
 *
 * Inputs: Current onboarding step, config/destination status, and action handlers.
 * Outputs: An `OnboardingOverlay` element tree.
 * Side effects: Calls provided handlers on user interaction.
 * Error handling: Delegated to the provided handlers.
 * Ties to other methods: Used by `SettingsPanel` as the onboarding branch.
 * Why this exists: Keep onboarding prop wiring isolated and reduce noise in `SettingsPanel`.
 */
export function SettingsPanelOnboarding({
  step,
  setStep,
  cfg,
  destStatus,
  canFinish,
  hardeningBusy,
  hardeningReport,
  statusMessage,
  startOnLoginMsg,
  onChangeRetention,
  onAddFolder,
  onQuickAddDesktop,
  onQuickAddDocuments,
  onQuickAddDownloads,
  onPickDestination,
  onUseDesktopDest,
  onUseDocumentsDest,
  onUseDownloadsDest,
  onStartOnLogin,
  onTestBackup,
  onRunHardening,
  onFinish,
}: Props) {
  const summary = `Watched: ${cfg.watched.length}, Destination: ${destStatus?.message ?? 'Not set'}`;
  const freeMessage = destStatus
    ? `Free space: ${destStatus.free_bytes != null ? formatBytes(destStatus.free_bytes) : 'Checking…'}`
    : 'Free space: Checking…';
  const hardeningSatisfied = isHardeningSatisfied(cfg);

  return (
    <OnboardingOverlay
      step={step}
      hasWatched={cfg.watched.length > 0}
      destStatus={destStatus}
      canFinish={canFinish}
      hardeningSatisfied={hardeningSatisfied}
      hardeningBusy={hardeningBusy}
      hardeningReport={hardeningReport}
      retention={cfg.max_backups_per_file}
      watchedPaths={(cfg.watched || []).map((w) => w.path)}
      summary={summary}
      freeMessage={freeMessage}
      safeMode={cfg.safe_mode}
      statusMessage={statusMessage}
      startOnLoginMsg={startOnLoginMsg}
      onRunHardening={onRunHardening}
      onChangeRetention={onChangeRetention}
      onAddFolder={onAddFolder}
      onQuickAddDesktop={onQuickAddDesktop}
      onQuickAddDocuments={onQuickAddDocuments}
      onQuickAddDownloads={onQuickAddDownloads}
      onPickDestination={onPickDestination}
      onUseDesktopDest={onUseDesktopDest}
      onUseDocumentsDest={onUseDocumentsDest}
      onUseDownloadsDest={onUseDownloadsDest}
      onStartOnLogin={onStartOnLogin}
      onTestBackup={onTestBackup}
      onNext={() => setStep((s) => Math.min(4, s + 1))}
      onBack={() => setStep((s) => Math.max(1, s - 1))}
      onFinish={onFinish}
    />
  );
}
