import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { MinimalMain } from '../MinimalMain';
import { Config } from '../../settings/types';
import { loadConfig, saveConfig } from '../../../services/config';
import { safeInvoke } from '../../../services/ipc';

vi.mock('../../../services/config', () => ({
  loadConfig: vi.fn(),
  saveConfig: vi.fn(),
}));

vi.mock('../../../services/ipc', () => ({
  safeInvoke: vi.fn(),
  tauriAvailable: () => false,
  wrapError: (context: string, error: unknown) => new Error(`${context}: ${String(error)}`),
}));

/**
 * Summary: Build a successful hardening report payload for background checks.
 *
 * Inputs: None.
 * Outputs: A `HardeningReport` compatible object for IPC mocks.
 * Side effects: None.
 * Error handling: None.
 * Ties to other methods: Used by `renderMinimal` safeInvoke default implementation.
 * Why this exists: Keep background hardening deterministic in minimal UI tests.
 */
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

/**
 * Purpose: Build a complete config object for minimal UI tests.
 *
 * Inputs: Optional partial overrides.
 * Outputs: A valid `Config` object.
 * Side effects: None.
 * Error handling: None.
 * Ties to other methods: Used by all MinimalMain tests.
 * Why this exists: Keep tests explicit without repeating config boilerplate.
 */
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

/**
 * Purpose: Render the MinimalMain component with a mocked config load.
 *
 * Inputs: Optional config overrides.
 * Outputs: A mocked `onEvent` callback.
 * Side effects: Mocks service calls and mounts React component tree.
 * Error handling: Throws with test context on failures.
 * Ties to other methods: Used by all MinimalMain tests.
 * Why this exists: Ensure each test gets a clean instance and predictable mocks.
 */
async function renderMinimal(
  overrides: Partial<Config> = {},
): Promise<{ onEvent: ReturnType<typeof vi.fn> }> {
  try {
    const cfg = makeConfig(overrides);
    (loadConfig as unknown as ReturnType<typeof vi.fn>).mockResolvedValueOnce(cfg);
    (saveConfig as unknown as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
    (safeInvoke as unknown as ReturnType<typeof vi.fn>).mockImplementation(
      async (command: string, payload?: { desired?: boolean }) => {
        if (command === 'hardening_check_cmd') {
          return makePassingHardeningReport();
        }
        if (command === 'toggle_safe_mode_cmd') {
          return payload?.desired ?? false;
        }
        return null;
      },
    );
    const onEvent = vi.fn();
    render(<MinimalMain onEvent={onEvent} />);
    await screen.findByRole('heading', { name: 'Destination' });
    return { onEvent };
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::renderMinimal] ${reason}`);
  }
}

/**
 * Purpose: Verify the minimal UI exposes running toggle, automatic cadence summary, and only restore action.
 *
 * Inputs: None.
 * Outputs: Asserts presence/absence of key controls.
 * Side effects: Renders the component and queries DOM.
 * Why: Ensure the UI matches the simplified spec.
 */
async function assertOnlyRestoreActionVisible(): Promise<void> {
  try {
    await renderMinimal();
    expect(screen.getByLabelText('Running')).toBeInTheDocument();
    expect(screen.getByText('Automatic backups')).toBeInTheDocument();
    expect(screen.getByText(/Every 30 min/i)).toBeInTheDocument();
    expect(screen.queryByLabelText('Backup interval minutes')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Restore Backup Version' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Safety checks required' })).not.toBeInTheDocument();
    expect(screen.queryByText('Actions')).not.toBeInTheDocument();
    expect(screen.queryByText('Back up now')).not.toBeInTheDocument();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::assertOnlyRestoreActionVisible] ${reason}`);
  }
}

/**
 * Purpose: Verify toggling running calls the safe mode IPC command.
 *
 * Inputs: None.
 * Outputs: Asserts `safeInvoke` is called with `toggle_safe_mode_cmd`.
 * Side effects: Fires checkbox click and awaits async handler.
 * Why: Ensure the running toggle is wired to daemon safe mode.
 */
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

/**
 * Purpose: Verify pausing shows Saved badge confirmation.
 *
 * Inputs: None.
 * Outputs: Asserts the Saved badge is visible after pausing.
 * Side effects: Clicks running toggle and awaits UI updates.
 * Why: Prevent regressions where pause actions do not surface a success confirmation.
 */
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

/**
 * Purpose: Verify additional destinations are visible and removable from the minimal destination card.
 *
 * Inputs: None.
 * Outputs: Asserts extra destination text and remove control are rendered.
 * Side effects: Renders the component and queries DOM.
 * Why: Prevent regressions where added destinations exist in config but are not shown in UI.
 */
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
  it('shows running toggle, automatic cadence, and restore button', assertOnlyRestoreActionVisible);
  it('wires running toggle to IPC', assertRunningToggleCallsIpc);
  it('shows Saved badge when paused', assertPauseShowsSavedBadge);
  it('shows additional destinations with remove controls', assertAdditionalDestinationsVisible);
});
