import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MinimalMain } from '../MinimalMain';
import { Config } from '../../../domain/config';
import { loadConfig, saveConfig } from '../../../services/config';
import { safeInvoke } from '../../../services/ipc';
import { removeKeptExtraVersion } from '../../../services/safety';

const { tauriAvailableMock } = vi.hoisted(() => ({
  tauriAvailableMock: vi.fn(() => true),
}));

vi.mock('../../../services/config', () => ({
  loadConfig: vi.fn(),
  saveConfig: vi.fn(),
}));

vi.mock('../../../services/ipc', () => ({
  safeInvoke: vi.fn(),
  tauriAvailable: tauriAvailableMock,
  wrapError: (context: string, error: unknown) => new Error(`${context}: ${String(error)}`),
}));

vi.mock('../../../services/safety', () => ({
  removeKeptExtraVersion: vi.fn(),
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
) {
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
    (safeInvoke as unknown as ReturnType<typeof vi.fn>).mockImplementation(
      async (command: string, payload?: { desired?: boolean }) => {
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
        return null;
      },
    );
    const onEvent = vi.fn();
    render(<MinimalMain onEvent={onEvent} />);
    await screen.findByRole('heading', { name: 'Where should copies live?' });
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
    expect(screen.getAllByText('Automatic protection').length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Every 30 min/i).length).toBeGreaterThan(0);
    expect(screen.queryByLabelText('Backup interval minutes')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Browse saved versions' })).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: 'Safety checks required' }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText('Actions')).not.toBeInTheDocument();
    expect(screen.queryByText('Back up now')).not.toBeInTheDocument();
    expect(screen.getByText('Your protection workflow is connected.')).toBeInTheDocument();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::assertOnlyRestoreActionVisible] ${reason}`);
  }
}

/** Prevent regressions where first-run setup order becomes ambiguous. */
async function assertDestinationFirstGuidance(): Promise<void> {
  try {
    await renderMinimal({
      backup_root: '',
      destinations: [{ id: 'default', path: '', label: 'Primary' }],
      watched: [],
    });
    expect(screen.getByText('Start with a safe place for your copies.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Choose destination' })).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'Add protected path' })[0]).toBeDisabled();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::assertDestinationFirstGuidance] ${reason}`);
  }
}

/** Keep the next required step obvious after destination selection. */
async function assertAddPathGuidance(): Promise<void> {
  try {
    await renderMinimal({ watched: [] });
    expect(screen.getByText('Storage is ready. Choose what matters.')).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: 'Add protected path' }).length).toBeGreaterThan(0);
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
    expect(screen.getByText(/Destination 2: \/tmp\/backup-2/)).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Remove destination Destination 2' }),
    ).toBeInTheDocument();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::assertAdditionalDestinationsVisible] ${reason}`);
  }
}

describe('MinimalMain', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    tauriAvailableMock.mockReturnValue(true);
  });

  it('shows running toggle, automatic cadence, and restore button', assertOnlyRestoreActionVisible);
  it(
    'shows destination-first setup guidance when no destination is selected',
    assertDestinationFirstGuidance,
  );
  it('shows add-path guidance when no protected paths exist', assertAddPathGuidance);
  it('wires running toggle to IPC', assertRunningToggleCallsIpc);
  it('shows Saved badge when paused', assertPauseShowsSavedBadge);
  it('shows additional destinations with remove controls', assertAdditionalDestinationsVisible);

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
    expect(
      await screen.findByRole('heading', { name: 'Where should copies live?' }),
    ).toBeInTheDocument();
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

    fireEvent.click(await screen.findByRole('button', { name: 'Remove extra version' }));

    await waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        'Could not remove extra version: version is locked',
        'error',
      );
    });
  });
});
