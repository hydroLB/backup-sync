import { describe, expect, it, beforeEach, vi } from 'vitest';
import type { Config } from '../../domain/config';
import { loadConfig, saveConfig } from '../config';
import { IpcError } from '../ipc';

const { safeInvokeMock, safeInvokeWithTimeoutMock, wrapErrorMock, correlationIdMock } = vi.hoisted(
  () => ({
    safeInvokeMock: vi.fn(),
    safeInvokeWithTimeoutMock: vi.fn(),
    wrapErrorMock: vi.fn((context: string, error: unknown) => {
      const message = error instanceof Error ? error.message : String(error);
      return new IpcError(`${context}: ${message}`);
    }),
    correlationIdMock: vi.fn(() => 'save-cid-1'),
  }),
);

vi.mock('../ipc', async () => {
  const actual = await vi.importActual<typeof import('../ipc')>('../ipc');
  return {
    ...actual,
    safeInvoke: safeInvokeMock,
    safeInvokeWithTimeout: safeInvokeWithTimeoutMock,
    wrapError: wrapErrorMock,
  };
});

vi.mock('../correlation', () => ({
  correlationId: correlationIdMock,
}));

function buildConfig(): Config {
  return {
    backup_root: '/tmp/backups',
    interval_seconds: 60,
    max_backups_per_file: 5,
    skip_hidden: true,
    ignore_patterns: [],
    max_parallel_copies: 4,
    max_bytes_per_second: null,
    min_free_space_bytes: null,
    hashing: {
      buffer_bytes: 65536,
      timeout_seconds: 30,
    },
    execution: {
      copy_buffer_bytes: 65536,
      copy_timeout_seconds: 300,
      free_space_safety_buffer_bytes: 10485760,
      recent_activity_cap: 50,
      retry_delays_ms: [100, 200, 400],
      retry_jitter_pct: 0.2,
    },
    planning: {
      hash_check_interval: 5,
      max_plan_items: 20000,
      scan_timeout_seconds: 300,
      scan_capacity_multiplier: 16,
    },
    runtime: {
      prune_interval_cycles: 10,
      verify_interval_seconds: 86400,
      scrub_full_interval_seconds: 604800,
      scrub_sample_blobs: 10,
      scrub_sample_versions_per_source: 5,
      watcher_debounce_seconds: 2,
      ipc_timeout_seconds: 5,
      service_command_timeout_seconds: 15,
      service_command_retry_delay_ms: 300,
      service_command_poll_interval_ms: 50,
      source_snapshots_enabled: true,
      source_snapshot_timeout_seconds: 30,
      tray_tooltip_refresh_seconds: 10,
      log_tail_lines: 200,
      simulation_sample_limit: 10,
      replication_enabled: true,
      replication_mirror_manifests: true,
      replication_max_manifest_deletes_per_cycle: 500,
    },
    safe_mode: false,
    watched: [],
    destinations: [
      {
        id: 'primary',
        path: '/tmp/backups',
        label: 'Primary',
      },
    ],
  };
}

describe('config service', () => {
  beforeEach(() => {
    safeInvokeMock.mockReset();
    safeInvokeWithTimeoutMock.mockReset();
    wrapErrorMock.mockClear();
    correlationIdMock.mockClear();
  });

  it('loads config via the backend command', async () => {
    const cfg = buildConfig();
    safeInvokeMock.mockResolvedValue(cfg);

    await expect(loadConfig()).resolves.toEqual(cfg);
    expect(safeInvokeMock).toHaveBeenCalledWith('load_config_cmd');
  });

  it('preserves interval_seconds and returns daemon restart warnings from save', async () => {
    const cfg = buildConfig();
    safeInvokeWithTimeoutMock.mockResolvedValue({
      daemon_restarted: false,
      daemon_restart_warning: 'Restart manually.',
    });

    await expect(saveConfig(cfg)).resolves.toEqual({
      daemon_restarted: false,
      daemon_restart_warning: 'Restart manually.',
    });
    expect(safeInvokeWithTimeoutMock).toHaveBeenCalledWith(
      'save_config_cmd',
      {
        cfg: expect.objectContaining({
          interval_seconds: 60,
        }),
        correlationId: 'save-cid-1',
      },
      30_000,
    );
    expect(correlationIdMock).toHaveBeenCalledWith('save');
  });

  it('wraps backend save failures with service context', async () => {
    safeInvokeWithTimeoutMock.mockRejectedValue(new Error('backend exploded'));

    await expect(saveConfig(buildConfig())).rejects.toThrow(
      '[saveConfig] Failed to save configuration: backend exploded',
    );
    expect(wrapErrorMock).toHaveBeenCalled();
  });
});
