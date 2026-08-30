import { act, cleanup, render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MinimalMain } from '../MinimalMain';
import { Config } from '../../../domain/config';
import { loadConfig, saveConfig, saveConfigRemovingSource } from '../../../services/config';
import { safeInvoke, safeInvokeWithTimeout } from '../../../services/ipc';
import { removeKeptExtraVersion } from '../../../services/safety';
import { openDialog } from '../../../services/dialog';
import { openDestinationFolder, relocateDestination } from '../../../services/storage';
import { StatusDto } from '../../../services/types';

const { tauriAvailableMock } = vi.hoisted(() => ({
  tauriAvailableMock: vi.fn(() => true),
}));

vi.mock('../../../services/config', () => ({
  loadConfig: vi.fn(),
  saveConfig: vi.fn(),
  saveConfigRemovingSource: vi.fn(),
}));

vi.mock('../../../services/ipc', () => ({
  safeInvoke: vi.fn(),
  safeInvokeWithTimeout: vi.fn(),
  tauriAvailable: tauriAvailableMock,
  wrapError: (context: string, error: unknown) => new Error(`${context}: ${String(error)}`),
}));

vi.mock('../../../services/safety', () => ({
  removeKeptExtraVersion: vi.fn(),
}));

vi.mock('../../../services/dialog', () => ({
  openDialog: vi.fn(),
}));

vi.mock('../../../services/storage', () => ({
  openDestinationFolder: vi.fn(),
  relocateDestination: vi.fn(),
}));

/** Keep background hardening deterministic in minimal UI tests. */
function makePassingHardeningReport() {
  return {
    ok: true,
    message: 'ok',
    watched_ok: ['/tmp/project'],
    watched_issues: [],
    destinations: [
      {
        id: 'default',
        path: '/tmp/backups',
        ok: true,
        free_bytes: 1024 * 1024 * 1024,
        required_free_bytes: 1024 * 1024,
        message: 'ok',
      },
    ],
    snapshots: {
      checked: false,
      supported: true,
      message: 'not checked',
    },
  };
}

/** Prevent status polling noise from obscuring real test failures. */
function makeStatus(
  safeMode = false,
  lastSafetyWarning: {
    ts: number;
    message: string;
    watched_path?: string | null;
    kept_version_id?: string | null;
  } | null = null,
): StatusDto {
  return {
    last_run_ts: 1709500000,
    last_files_backed_up: 3,
    last_error: null,
    last_dirty_count: 0,
    last_safety_warning: lastSafetyWarning,
    uptime_secs: 600,
    version: 'test',
    free_bytes: 1024 * 1024 * 1024,
    min_free_space_bytes: null,
    last_verify_ts: 1709500000,
    last_verify_status: 'ok',
    last_verify_issues: 0,
    recent_activity: [],
    safe_mode: safeMode,
    destination_paused: false,
    destination_pause_reason: null,
    destination_unavailable_ids: [],
    replication_last_run_ts: null,
    replication_last_status: 'idle',
    replication_last_error: null,
    replication_last_bytes_copied: 0,
    replication_last_blobs_copied: 0,
    replication_last_manifests_copied: 0,
    replication_last_manifests_deleted: 0,
    replication_last_pairs_ok: 0,
    replication_last_pairs_failed: 0,
    replication_last_targets_failed: [],
    destinations: [],
  };
}

/** Keep tests explicit without repeating config boilerplate. */
function makeConfig(overrides: Partial<Config> = {}): Config {
  return {
    backup_root: '/tmp/backups',
    interval_seconds: 1800,
    max_backups_per_file: 5,
    skip_hidden: true,
    ignore_patterns: [],
    max_parallel_copies: 4,
    max_bytes_per_second: null,
    min_free_space_bytes: null,
    hashing: { buffer_bytes: 65536, timeout_seconds: 30 },
    execution: {
      copy_buffer_bytes: 65536,
      copy_timeout_seconds: 300,
      free_space_safety_buffer_bytes: 1024 * 1024,
      recent_activity_cap: 50,
      retry_delays_ms: [100, 200, 400],
      retry_jitter_pct: 0.2,
    },
    planning: {
      hash_check_interval: 5,
      max_plan_items: 10_000,
      scan_timeout_seconds: 300,
      scan_capacity_multiplier: 16,
    },
    runtime: {
      prune_interval_cycles: 10,
      verify_interval_seconds: 86400,
      scrub_full_interval_seconds: 7 * 86400,
      scrub_sample_blobs: 200,
      scrub_sample_versions_per_source: 2,
      watcher_debounce_seconds: 2,
      ipc_timeout_seconds: 5,
      service_command_timeout_seconds: 15,
      service_command_retry_delay_ms: 300,
      service_command_poll_interval_ms: 50,
      source_snapshots_enabled: false,
      source_snapshot_timeout_seconds: 20,
      tray_tooltip_refresh_seconds: 10,
      log_tail_lines: 200,
      simulation_sample_limit: 10,
    },
    safe_mode: false,
    watched: [
      {
        path: '/tmp/project',
        kind: 'Directory',
        enabled: true,
        destination_id: 'default',
        max_backups_per_file: 5,
      },
    ],
    destinations: [{ id: 'default', path: '/tmp/backups', label: 'Primary' }],
    ...overrides,
  };
}

/** Ensure each test gets a clean instance and predictable mocks. */
async function renderMinimal(
  overrides: Partial<Config> = {},
  status = makeStatus(overrides.safe_mode ?? false),
): Promise<{ onEvent: ReturnType<typeof vi.fn> }> {
  try {
    localStorage.clear();
    tauriAvailableMock.mockReturnValue(true);
    const cfg = makeConfig(overrides);
    (loadConfig as unknown as ReturnType<typeof vi.fn>).mockResolvedValueOnce(cfg);
    (saveConfig as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
      daemon_restarted: true,
      daemon_restart_warning: null,
    });
    (saveConfigRemovingSource as unknown as ReturnType<typeof vi.fn>).mockResolvedValue({
      daemon_restarted: true,
      daemon_restart_scheduled: false,
      daemon_restart_warning: null,
    });
    const invokeImplementation = async (command: string, payload?: { desired?: boolean }) => {
      if (command === 'hardening_check_cmd') {
        return makePassingHardeningReport();
      }
      if (command === 'get_status') {
        return status;
      }
      if (command === 'check_destination_cmd') {
        return {
          writable: true,
          free_bytes: 1024 * 1024 * 1024,
          message: 'Ready',
        };
      }
      if (command === 'test_access_cmd') {
        return {
          destination_writable: true,
          destination_message: 'Ready',
          watched_ok: cfg.watched.map((entry) => entry.path),
          watched_missing: [],
          watched_unwritable: [],
        };
      }
      if (command === 'toggle_safe_mode_cmd') {
        return {
          safe_mode: payload?.desired ?? false,
          applied_live: true,
          warning: null,
        };
      }
      if (command === 'list_versions_cmd') {
        return [
          {
            source_path: '/tmp/project',
            versions: [{ id: 'v1', created_at_unix: 1_709_500_000 }],
          },
        ];
      }
      if (command === 'list_version_files_cmd') {
        return { total_files: 3, files: [] };
      }
      return null;
    };
    (safeInvoke as unknown as ReturnType<typeof vi.fn>).mockImplementation(invokeImplementation);
    (safeInvokeWithTimeout as unknown as ReturnType<typeof vi.fn>).mockImplementation(
      invokeImplementation,
    );
    const onEvent = vi.fn();
    render(<MinimalMain onEvent={onEvent} />);
    await screen.findByRole('heading', { name: 'Backup locations' });
    return { onEvent };
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::renderMinimal] ${reason}`);
  }
}

/** Ensure the UI matches the simplified spec. */
async function assertOnlyRestoreActionVisible(): Promise<void> {
  try {
    await renderMinimal();
    expect(screen.getByLabelText('Running')).toBeInTheDocument();
    expect(screen.getByText('Automatic, versioned protection')).toBeInTheDocument();
    expect(screen.getByLabelText('1 protected item total')).toHaveTextContent('1 total');
    expect(screen.getByLabelText('1 backup location total')).toHaveTextContent('1 total');
    expect(screen.getByLabelText('Backup interval minutes')).toHaveValue(30);
    expect(
      screen.getByText(
        'One complete backup. New restore points save only changes—not full copies.',
      ),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Recover a previous version' })).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Safety checks required' }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText('Actions')).not.toBeInTheDocument();
    expect(screen.queryByText('Back up now')).not.toBeInTheDocument();
    expect(screen.queryByText('How Backup Sync works')).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/Theme control/i)).not.toBeInTheDocument();
    expect(
      screen.queryByRole('link', { name: 'Download the full Backup Sync desktop app' }),
    ).not.toBeInTheDocument();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::assertOnlyRestoreActionVisible] ${reason}`);
  }
}

/** Protected content stays first even before storage has been chosen. */
async function assertProtectionFirstGuidance(): Promise<void> {
  try {
    await renderMinimal({
      backup_root: '',
      destinations: [{ id: 'default', path: '', label: 'Primary' }],
      watched: [],
    });
    expect(screen.getByText('Nothing is protected yet')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Choose storage' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add protected path' })).toBeEnabled();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::assertProtectionFirstGuidance] ${reason}`);
  }
}

/** Keep the next required step obvious after destination selection. */
async function assertAddPathGuidance(): Promise<void> {
  try {
    await renderMinimal({ watched: [] });
    expect(screen.getByText('Nothing is protected yet')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add protected path' })).toBeEnabled();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::assertAddPathGuidance] ${reason}`);
  }
}

/** Ensure the running toggle is wired to daemon safe mode. */
async function assertRunningToggleCallsIpc(): Promise<void> {
  try {
    await renderMinimal({ safe_mode: false });
    const runningToggle = screen.getByLabelText('Running');
    fireEvent.click(runningToggle);
    await waitFor(() => {
      expect(safeInvoke).toHaveBeenCalledWith('toggle_safe_mode_cmd', { desired: true });
    });
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::assertRunningToggleCallsIpc] ${reason}`);
  }
}

/** Prevent regressions where pause actions do not surface a success confirmation. */
async function assertPauseShowsSavedBadge(): Promise<void> {
  try {
    await renderMinimal({ safe_mode: false });
    const runningToggle = screen.getByLabelText('Running');
    fireEvent.click(runningToggle);
    await waitFor(() => {
      expect(screen.getByText('Saved')).toBeInTheDocument();
    });
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::assertPauseShowsSavedBadge] ${reason}`);
  }
}

/** Prevent regressions where added destinations exist in config but are not shown in UI. */
async function assertAdditionalDestinationsVisible(): Promise<void> {
  try {
    await renderMinimal({
      destinations: [
        { id: 'default', path: '/tmp/backups', label: 'Primary' },
        { id: 'dest-2', path: '/tmp/backup-2', label: 'Destination 2' },
      ],
    });
    expect(screen.getByText('Main storage')).toBeInTheDocument();
    expect(screen.getByText('Secondary backup location 1')).toBeInTheDocument();
    expect(screen.getByLabelText('2 backup locations total')).toHaveTextContent('2 total');
    expect(screen.getByText('Complete redundant backup copy')).toBeInTheDocument();
    expect(screen.getByText('/tmp/backup-2')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Change main storage' })).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Change secondary backup location 1' }),
    ).toBeInTheDocument();
    const removeCopy = screen.getByRole('button', {
      name: 'Remove secondary backup location /tmp/backup-2',
    });
    expect(removeCopy).toBeInTheDocument();
    fireEvent.click(removeCopy);
    expect(screen.getByText('Remove this storage location?')).toBeInTheDocument();
    expect(screen.queryByText('/tmp/backup-2')).not.toBeInTheDocument();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::assertAdditionalDestinationsVisible] ${reason}`);
  }
}

/** Give the section with overflowing content the unused height from its quieter neighbor. */
async function assertContentAwareSectionBalance(): Promise<void> {
  try {
    await renderMinimal({
      destinations: Array.from({ length: 6 }, (_, index) => ({
        id: index === 0 ? 'default' : `dest-${index + 1}`,
        path: `/tmp/backup-${index + 1}`,
        label: index === 0 ? 'Primary' : `Destination ${index + 1}`,
      })),
    });
    expect(document.querySelector('.minimal-grid')).toHaveAttribute(
      'data-section-balance',
      'destinations',
    );

    cleanup();
    await renderMinimal({
      watched: Array.from({ length: 3 }, (_, index) => ({
        path: `/tmp/project-${index + 1}`,
        kind: 'Directory' as const,
        enabled: true,
        destination_id: 'default',
        max_backups_per_file: 5,
      })),
    });
    expect(document.querySelector('.minimal-grid')).toHaveAttribute(
      'data-section-balance',
      'folders',
    );
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::assertContentAwareSectionBalance] ${reason}`);
  }
}

/** Folder and storage rows should advertise that their paths can be changed in place. */
async function assertPathRowsAreClickable(): Promise<void> {
  try {
    await renderMinimal();
    expect(
      screen.getByRole('button', { name: 'Change protected directory /tmp/project' }),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Change main storage' })).toBeInTheDocument();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::assertPathRowsAreClickable] ${reason}`);
  }
}

describe('MinimalMain', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    tauriAvailableMock.mockReturnValue(true);
    vi.mocked(openDialog).mockResolvedValue(null);
    vi.mocked(openDestinationFolder).mockResolvedValue(true);
  });

  it('shows running toggle, automatic cadence, and restore button', assertOnlyRestoreActionVisible);
  it('keeps protection first when no destination is selected', assertProtectionFirstGuidance);
  it('shows add-path guidance when no protected paths exist', assertAddPathGuidance);
  it('wires running toggle to IPC', assertRunningToggleCallsIpc);
  it('shows Saved badge when paused', assertPauseShowsSavedBadge);
  it('makes configured paths directly clickable', assertPathRowsAreClickable);
  it('shows additional destinations with remove controls', assertAdditionalDestinationsVisible);
  it('allocates unused card height to the section that needs it', assertContentAwareSectionBalance);

  it('opens a storage card directly without showing a picker', async () => {
    await renderMinimal();

    fireEvent.click(screen.getByRole('button', { name: 'Open main storage folder' }));

    await waitFor(() => {
      expect(openDestinationFolder).toHaveBeenCalledWith('default');
    });
    expect(openDialog).not.toHaveBeenCalled();
  });

  it('shows how many files and recovery versions are actually stored', async () => {
    await renderMinimal();

    expect(await screen.findByText('3 files · 1 saved version')).toBeInTheDocument();
  });

  it('uses the remaining storage card surface to change its location', async () => {
    await renderMinimal();

    fireEvent.click(screen.getByRole('button', { name: 'Change main storage' }));

    await waitFor(() => {
      expect(openDialog).toHaveBeenCalledWith(
        expect.objectContaining({ defaultPath: '/tmp/backups' }),
      );
    });
    expect(openDestinationFolder).not.toHaveBeenCalled();
  });

  it('warns that removing protection permanently deletes saved versions', async () => {
    await renderMinimal();
    fireEvent.click(screen.getByRole('button', { name: 'Remove protected path /tmp/project' }));

    expect(screen.getByText('Remove this folder from Backup Sync?')).toBeInTheDocument();
    expect(
      screen.getByText(/All of its saved versions will be deleted\. This cannot be undone\./),
    ).toBeInTheDocument();
    expect(screen.getByText(/Your original files will not be deleted\./)).toBeInTheDocument();
  });

  it('confirms removal through the command that deletes native saved history', async () => {
    await renderMinimal();
    fireEvent.click(screen.getByRole('button', { name: 'Remove protected path /tmp/project' }));
    fireEvent.click(screen.getByRole('button', { name: 'Remove' }));

    await waitFor(() => {
      expect(saveConfigRemovingSource).toHaveBeenCalledWith(
        expect.objectContaining({ watched: [] }),
        '/tmp/project',
        'Directory',
      );
    });
    expect(saveConfig).not.toHaveBeenCalled();
  });

  it('saves retention within its row without disabling the rest of the screen', async () => {
    await renderMinimal();
    let finishSave!: (result: { daemon_restarted: boolean; daemon_restart_warning: null }) => void;
    vi.mocked(saveConfig).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishSave = resolve;
        }),
    );

    fireEvent.click(
      screen.getByRole('button', { name: 'Keep more restore points for /tmp/project' }),
    );

    await waitFor(() => {
      expect(saveConfig).toHaveBeenCalled();
      expect(
        screen.getByRole('spinbutton', { name: 'Restore points to keep for /tmp/project' }),
      ).toHaveValue(6);
    });
    expect(screen.getByLabelText('Backup interval minutes')).toHaveValue(30);
    expect(screen.getByRole('button', { name: 'Add protected path' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Add backup location' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Change main storage' })).toBeEnabled();
    expect(
      screen.getByRole('button', { name: 'Keep more restore points for /tmp/project' }),
    ).toBeDisabled();

    await act(async () => {
      finishSave({ daemon_restarted: true, daemon_restart_warning: null });
    });
    await waitFor(() => {
      expect(
        screen.getByRole('button', { name: 'Keep more restore points for /tmp/project' }),
      ).toBeEnabled();
    });
  });

  it('keeps storage controls available while the source picker is open', async () => {
    let finishPicker: ((value: null) => void) | undefined;
    vi.mocked(openDialog).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishPicker = resolve;
        }),
    );
    await renderMinimal();

    fireEvent.click(screen.getByRole('button', { name: 'Add protected path' }));

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Add protected path' })).toBeDisabled();
    });
    expect(screen.getByRole('button', { name: 'Add backup location' })).toBeEnabled();

    finishPicker?.(null);
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Add protected path' })).toBeEnabled();
    });
  });

  it('confirms and safely migrates main storage before changing its path', async () => {
    const nextConfig = makeConfig({
      backup_root: '/tmp/new-backups',
      destinations: [{ id: 'default', path: '/tmp/new-backups', label: 'Primary' }],
    });
    vi.mocked(openDialog).mockResolvedValueOnce('/tmp/new-backups');
    vi.mocked(relocateDestination).mockResolvedValueOnce({
      config: nextConfig,
      operation: 'moved_to_new_location',
      files_moved: 12,
      bytes_moved: 4096,
      old_location_removed: true,
      warning: null,
    });
    await renderMinimal();

    fireEvent.click(screen.getByRole('button', { name: 'Change main storage' }));

    await waitFor(() => {
      expect(openDialog).toHaveBeenCalledWith(
        expect.objectContaining({
          directory: true,
          defaultPath: '/tmp/backups',
        }),
      );
    });

    expect(
      await screen.findByRole('heading', { name: 'Move backup storage?' }),
    ).toBeInTheDocument();
    expect(screen.getAllByText('/tmp/backups')).toHaveLength(2);
    expect(screen.getByText('/tmp/new-backups')).toBeInTheDocument();
    expect(relocateDestination).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Move backups safely' }));

    await waitFor(() => {
      expect(relocateDestination).toHaveBeenCalledWith('default', '/tmp/new-backups');
    });
    expect(await screen.findByText('/tmp/new-backups')).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Move backup storage?' })).not.toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('Updated');
  });

  it('requires an additional verified role-swap confirmation for an existing secondary', async () => {
    const destinations = [
      { id: 'default', path: '/tmp/backups', label: 'Primary' },
      { id: 'mirror', path: '/tmp/mirror', label: 'Secondary' },
    ];
    const nextConfig = makeConfig({
      backup_root: '/tmp/mirror',
      destinations: [
        { ...destinations[0]!, path: '/tmp/mirror' },
        { ...destinations[1]!, path: '/tmp/backups' },
      ],
    });
    vi.mocked(openDialog).mockResolvedValueOnce('/tmp/mirror');
    vi.mocked(relocateDestination).mockResolvedValueOnce({
      config: nextConfig,
      operation: 'promoted_existing_secondary',
      files_moved: 0,
      bytes_moved: 0,
      old_location_removed: false,
      warning: 'Both copies verified.',
    });
    await renderMinimal({ destinations });

    fireEvent.click(screen.getByRole('button', { name: 'Change main storage' }));

    expect(
      await screen.findByRole('heading', { name: 'Make this Main storage?' }),
    ).toBeInTheDocument();
    expect(screen.getByText(/No backup files will be moved or deleted/)).toBeInTheDocument();
    expect(
      screen.getByText(/roll back if the background service cannot activate/),
    ).toBeInTheDocument();
    expect(relocateDestination).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Verify and switch' }));

    await waitFor(() => {
      expect(relocateDestination).toHaveBeenCalledWith('default', '/tmp/mirror');
    });
    expect(await screen.findByText('/tmp/mirror')).toBeInTheDocument();
  });

  it('edits the backup check cadence in minutes', async () => {
    await renderMinimal();
    const interval = screen.getByLabelText('Backup interval minutes');

    fireEvent.change(interval, { target: { value: '45' } });
    fireEvent.blur(interval);

    await waitFor(() => {
      expect(saveConfig).toHaveBeenCalledWith(
        expect.objectContaining({ interval_seconds: 45 * 60 }),
      );
    });
    expect(interval).toHaveValue(45);
    expect(screen.getByRole('status')).toHaveTextContent('Updated');
  });

  it('shows a retryable configuration error and recovers', async () => {
    const cfg = makeConfig();
    vi.mocked(loadConfig)
      .mockRejectedValueOnce(new Error('permission denied'))
      .mockResolvedValueOnce(cfg);
    vi.mocked(saveConfig).mockResolvedValue({
      daemon_restarted: true,
      daemon_restart_warning: null,
    });
    vi.mocked(safeInvoke).mockImplementation(async (command: string) => {
      if (command === 'hardening_check_cmd') return makePassingHardeningReport();
      if (command === 'get_status') return makeStatus();
      if (command === 'check_destination_cmd') {
        return { writable: true, free_bytes: 1024, message: 'Ready' };
      }
      if (command === 'test_access_cmd') {
        return {
          destination_writable: true,
          destination_message: 'Ready',
          watched_ok: ['/tmp/project'],
          watched_missing: [],
          watched_unwritable: [],
        };
      }
      return null;
    });

    render(<MinimalMain onEvent={vi.fn()} />);

    expect(await screen.findByRole('alert')).toHaveTextContent('permission denied');
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(await screen.findByRole('heading', { name: 'Backup locations' })).toBeInTheDocument();
    expect(loadConfig).toHaveBeenCalledTimes(2);
  });

  it('surfaces remove-extra-version failures to the user', async () => {
    vi.mocked(removeKeptExtraVersion).mockRejectedValueOnce(new Error('version is locked'));
    const warning = {
      ts: 42,
      message: 'An extra version was kept for safety.',
      watched_path: '/tmp/project',
      kept_version_id: 'v1',
    };
    const { onEvent } = await renderMinimal({}, makeStatus(false, warning));

    const safetyMessage = await screen.findByText('An extra version was kept for safety.');
    const safetyBanner = safetyMessage.closest('.feedback-banner');
    expect(safetyMessage.closest('.feedback-overlay')).not.toBeNull();
    expect(safetyBanner).toHaveAttribute('data-tauri-drag-region');
    expect(screen.getByRole('button', { name: 'Remove extra version' })).not.toHaveAttribute(
      'data-tauri-drag-region',
    );

    fireEvent.click(await screen.findByRole('button', { name: 'Remove extra version' }));

    await waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        'Could not remove extra version: version is locked',
        'error',
      );
    });
  });

  it('hides a stale safety warning after its protected folder is removed', async () => {
    const warning = {
      ts: 42,
      message: 'Protected path is unusually smaller than before.',
      watched_path: '/tmp/removed-project',
      kept_version_id: 'v1',
    };

    await renderMinimal({}, makeStatus(false, warning));

    expect(screen.queryByText(warning.message)).not.toBeInTheDocument();
  });

  it('overlays persistent storage warnings without inserting them into a card', async () => {
    const status = {
      ...makeStatus(),
      destination_paused: true,
      destination_pause_reason: 'Main storage is unavailable.',
    };
    await renderMinimal({}, status);

    const warning = await screen.findByText('Main storage is unavailable.');
    expect(warning.closest('.feedback-overlay')).not.toBeNull();
    expect(warning.closest('.folders-card')).toBeNull();
    expect(warning.closest('.destination-card')).toBeNull();
    expect(screen.getAllByText('Main storage is unavailable.')).toHaveLength(1);
  });
});
