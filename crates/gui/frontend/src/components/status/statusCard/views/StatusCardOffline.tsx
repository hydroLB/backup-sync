import OfflineNotice from '../../OfflineNotice';

type Props = {
  error: string | null;
  message: string;
  onInstallService: () => Promise<void>;
  onExportLogs: () => Promise<void>;
};

/**
 * Summary: Render the offline status view (daemon unavailable).
 *
 * Inputs: Error/message strings and offline action handlers.
 * Outputs: An `OfflineNotice` element tree.
 * Side effects: Calls provided handlers on user interaction.
 * Error handling: Delegated to handlers.
 * Ties to other methods: Used by `StatusCard` when no status is available.
 * Why this exists: Keep offline rendering separate from the online status UI.
 */
export function StatusCardOffline({ error, message, onInstallService, onExportLogs }: Props) {
  return (
    <OfflineNotice
      error={error}
      message={message}
      onInstallService={onInstallService}
      onExportLogs={onExportLogs}
    />
  );
}

