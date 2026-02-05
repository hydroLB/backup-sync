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
  wrapError: (context: string, error: unknown) => new Error(`${context}: ${String(error)}`),
}));

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
    const onEvent = vi.fn();
    render(<MinimalMain onEvent={onEvent} />);
    await screen.findByText('Destination');
    return { onEvent };
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::renderMinimal] ${reason}`);
  }
}

/**
 * Purpose: Verify the minimal UI exposes running toggle, interval editor, and only restore action.
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
    expect(screen.getByLabelText('Backup interval minutes')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Restore version' })).toBeInTheDocument();
    expect(screen.queryByText('Actions')).not.toBeInTheDocument();
    expect(screen.queryByText('Back up now')).not.toBeInTheDocument();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::assertOnlyRestoreActionVisible] ${reason}`);
  }
}

/**
 * Purpose: Verify editing the interval commits a config save with derived seconds.
 *
 * Inputs: None.
 * Outputs: Asserts `saveConfig` receives updated `interval_seconds`.
 * Side effects: Fires input events and awaits async save.
 * Why: Ensure the schedule editor actually persists changes.
 */
async function assertIntervalEditSaves(): Promise<void> {
  try {
    await renderMinimal();
    const intervalInput = screen.getByLabelText('Backup interval minutes');
    fireEvent.change(intervalInput, { target: { value: '45' } });
    fireEvent.blur(intervalInput);
    await waitFor(() => {
      expect(saveConfig).toHaveBeenCalled();
    });
    const calls = (saveConfig as unknown as ReturnType<typeof vi.fn>).mock.calls;
    const first = calls[0]?.[0];
    if (!first) {
      throw new Error('[MinimalMain.test.tsx::assertIntervalEditSaves] Missing saveConfig payload');
    }
    const saved = first as Config;
    expect(saved.interval_seconds).toBe(45 * 60);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[MinimalMain.test.tsx::assertIntervalEditSaves] ${reason}`);
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
    (safeInvoke as unknown as ReturnType<typeof vi.fn>).mockResolvedValueOnce(true);
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

describe('MinimalMain', () => {
  it('shows running toggle, interval editor, and restore button', assertOnlyRestoreActionVisible);
  it('persists interval edits', assertIntervalEditSaves);
  it('wires running toggle to IPC', assertRunningToggleCallsIpc);
});
