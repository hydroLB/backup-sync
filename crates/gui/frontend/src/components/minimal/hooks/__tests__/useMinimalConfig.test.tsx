import { act, render, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Config } from '../../../../domain/config';
import { useMinimalConfig } from '../useMinimalConfig';
import { loadConfig, saveConfig } from '../../../../services/config';
import type { ConfigSaveResult } from '../../../../services/config';

vi.mock('../../../../services/config', () => ({
  loadConfig: vi.fn(),
  saveConfig: vi.fn(),
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
    destinations: [],
  };
}

type Controller = {
  cfg: Config | null;
  loadError: string | null;
  reload: () => void;
  persist: (next: Config, successMessage?: string) => Promise<void>;
};

function Harness({
  onEvent,
  controller,
}: {
  onEvent: (msg: string, kind?: 'ok' | 'error' | 'info') => void;
  controller?: { current: Controller | null };
}) {
  const state = useMinimalConfig({ onEvent });

  if (controller) {
    controller.current = {
      cfg: state.cfg,
      loadError: state.loadError,
      reload: state.reload,
      persist: state.persist,
    };
  }

  return null;
}

const SAVE_OK: ConfigSaveResult = {
  daemon_restarted: true,
  daemon_restart_warning: null,
};

describe('useMinimalConfig', () => {
  beforeEach(() => {
    vi.mocked(loadConfig).mockReset();
    vi.mocked(saveConfig).mockReset();
  });

  it('surfaces daemon restart warnings after enforcing the fixed schedule', async () => {
    vi.mocked(loadConfig).mockResolvedValue(buildConfig());
    vi.mocked(saveConfig).mockResolvedValue({
      daemon_restarted: false,
      daemon_restart_warning: 'Configuration saved, but restart is still required.',
    });
    const onEvent = vi.fn();

    render(<Harness onEvent={onEvent} />);

    await waitFor(() => {
      expect(saveConfig).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(onEvent).toHaveBeenCalledWith(
        'Configuration saved, but restart is still required.',
        'error',
      );
    });
  });

  it('exposes a load error and recovers when retried', async () => {
    const controller = { current: null as Controller | null };
    vi.mocked(loadConfig)
      .mockRejectedValueOnce(new Error('config unavailable'))
      .mockResolvedValueOnce({ ...buildConfig(), interval_seconds: 1800 });
    vi.mocked(saveConfig).mockResolvedValue(SAVE_OK);
    const onEvent = vi.fn();

    render(<Harness onEvent={onEvent} controller={controller} />);

    await waitFor(() => {
      expect(controller.current?.loadError).toBe(
        'Failed to load configuration: config unavailable',
      );
    });

    act(() => {
      controller.current?.reload();
    });

    await waitFor(() => {
      expect(controller.current?.cfg?.interval_seconds).toBe(1800);
      expect(controller.current?.loadError).toBeNull();
    });
    expect(loadConfig).toHaveBeenCalledTimes(2);
  });

  it('does not save on load when the fixed schedule is already present', async () => {
    vi.mocked(loadConfig).mockResolvedValue({
      ...buildConfig(),
      interval_seconds: 1800,
    });
    vi.mocked(saveConfig).mockResolvedValue(SAVE_OK);

    render(<Harness onEvent={vi.fn()} />);

    await waitFor(() => {
      expect(loadConfig).toHaveBeenCalled();
    });
    expect(saveConfig).not.toHaveBeenCalled();
  });

  it('emits only the success message when manual persist has no daemon warning', async () => {
    const controller = { current: null as Controller | null };
    const cfg = {
      ...buildConfig(),
      interval_seconds: 1800,
    };
    vi.mocked(loadConfig).mockResolvedValue(cfg);
    vi.mocked(saveConfig).mockResolvedValue(SAVE_OK);
    const onEvent = vi.fn();

    render(<Harness onEvent={onEvent} controller={controller} />);

    await waitFor(() => {
      expect(loadConfig).toHaveBeenCalled();
    });

    await act(async () => {
      await controller.current?.persist(cfg, 'Persisted.');
    });

    expect(onEvent).toHaveBeenCalledWith('Persisted.', 'ok');
    expect(onEvent).not.toHaveBeenCalledWith(expect.stringContaining('daemon'), 'error');
  });

  it('emits success and daemon warning when manual persist saves but restart fails', async () => {
    const controller = { current: null as Controller | null };
    const cfg = {
      ...buildConfig(),
      interval_seconds: 1800,
    };
    vi.mocked(loadConfig).mockResolvedValue(cfg);
    vi.mocked(saveConfig).mockResolvedValue({
      daemon_restarted: false,
      daemon_restart_warning: 'Restart required.',
    });
    const onEvent = vi.fn();

    render(<Harness onEvent={onEvent} controller={controller} />);

    await waitFor(() => {
      expect(loadConfig).toHaveBeenCalled();
    });

    await act(async () => {
      await controller.current?.persist(cfg, 'Persisted.');
    });

    expect(onEvent).toHaveBeenCalledWith('Persisted.', 'ok');
    expect(onEvent).toHaveBeenCalledWith('Restart required.', 'error');
  });

  it('emits save failed when manual persist rejects', async () => {
    const controller = { current: null as Controller | null };
    const cfg = {
      ...buildConfig(),
      interval_seconds: 1800,
    };
    vi.mocked(loadConfig).mockResolvedValue(cfg);
    vi.mocked(saveConfig).mockRejectedValue(new Error('disk full'));
    const onEvent = vi.fn();

    render(<Harness onEvent={onEvent} controller={controller} />);

    await waitFor(() => {
      expect(loadConfig).toHaveBeenCalled();
    });

    await act(async () => {
      await controller.current?.persist(cfg, 'Persisted.');
    });

    expect(onEvent).toHaveBeenCalledWith('Save failed: disk full', 'error');
  });
});
