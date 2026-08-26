import { act, render } from '@testing-library/react';
import { Config } from '../../../../domain/config';
import { useMinimalActions } from '../useMinimalActions';

type Params = Parameters<typeof useMinimalActions>[0];
type Actions = ReturnType<typeof useMinimalActions>;

function buildConfig(): Config {
  return {
    backup_root: '/backup',
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
      free_space_safety_buffer_bytes: 10485760,
      recent_activity_cap: 50,
      retry_delays_ms: [100, 200],
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
      source_snapshots_enabled: false,
      source_snapshot_timeout_seconds: 20,
      tray_tooltip_refresh_seconds: 10,
      log_tail_lines: 200,
      simulation_sample_limit: 10,
    },
    safe_mode: false,
    watched: [],
    destinations: [{ id: 'primary', path: '/backup', label: 'Primary' }],
  };
}

function renderActions(overrides: Partial<Params> = {}) {
  const persist = vi.fn<Params['persist']>(async () => undefined);
  const onEvent = vi.fn<Params['onEvent']>();
  const pickDestinationPath = vi.fn<Params['pickers']['pickDestinationPath']>(async () => null);
  const pickPathForDest = vi.fn<Params['pickers']['pickPathForDest']>(async () => undefined);
  const params: Params = {
    cfg: buildConfig(),
    primaryId: 'primary',
    persist,
    onEvent,
    pickers: { pickDestinationPath, pickPathForDest },
    ...overrides,
  };
  let actions!: Actions;

  function Harness() {
    actions = useMinimalActions(params);
    return null;
  }

  render(<Harness />);
  return { actions, persist, onEvent, pickDestinationPath, pickPathForDest };
}

describe('useMinimalActions', () => {
  it('keeps every action inert until configuration is loaded', async () => {
    const { actions, persist } = renderActions({ cfg: null, primaryId: null });

    await act(async () => {
      await actions.chooseDestination();
      await actions.addDestination();
      await actions.removeDestination('missing');
      await actions.addFolder();
      await actions.addFile();
      await actions.changePath('/tmp/a', 'Directory');
      await actions.removePath('/tmp/a', 'Directory', 'primary');
      await actions.updateKeep('/tmp/a', 'Directory', 'primary', 4);
      await actions.updateDestination('/tmp/a', 'Directory', 'primary', 'other');
    });

    expect(persist).not.toHaveBeenCalled();
  });

  it('changes every destination copy of a protected path together', async () => {
    const cfg = buildConfig();
    cfg.destinations.push({ id: 'mirror', path: '/mirror' });
    cfg.watched = [
      { path: '/source', kind: 'Directory', enabled: true, destination_id: 'primary' },
      { path: '/source', kind: 'Directory', enabled: true, destination_id: 'mirror' },
      { path: '/other', kind: 'File', enabled: true, destination_id: 'primary' },
    ];
    const pickPathForDest = vi.fn<Params['pickers']['pickPathForDest']>(
      async (_destinationId, kind, updateWatched) => {
        await updateWatched('/new-source', kind, 'primary');
      },
    );
    const { actions, persist } = renderActions({
      cfg,
      pickers: { pickDestinationPath: vi.fn(async () => null), pickPathForDest },
    });

    await act(async () => actions.changePath('/source', 'Directory'));

    const saved = persist.mock.calls[0]?.[0] as Config;
    expect(saved.watched.filter((entry) => entry.path === '/new-source')).toHaveLength(2);
    expect(saved.watched).toContainEqual(
      expect.objectContaining({ path: '/other', destination_id: 'primary' }),
    );
  });

  it('chooses a primary destination and reports picker failures', async () => {
    const cfg = { ...buildConfig(), backup_root: '', destinations: [] };
    const pickDestinationPath = vi
      .fn<Params['pickers']['pickDestinationPath']>()
      .mockResolvedValueOnce(null)
      .mockResolvedValueOnce('/new-backup')
      .mockRejectedValueOnce('dialog unavailable');
    const { actions, persist, onEvent } = renderActions({
      cfg,
      primaryId: null,
      pickers: { pickDestinationPath, pickPathForDest: vi.fn(async () => undefined) },
    });

    await act(async () => {
      await actions.chooseDestination();
      await actions.chooseDestination();
      await actions.chooseDestination();
    });

    expect(persist).toHaveBeenCalledWith(
      expect.objectContaining({
        backup_root: '/new-backup',
        destinations: [expect.objectContaining({ id: 'default', path: '/new-backup' })],
      }),
      'Primary destination set to /new-backup.',
    );
    expect(onEvent).toHaveBeenCalledWith(
      '[useMinimalActions] Failed to choose destination: dialog unavailable',
      'error',
    );
  });

  it('adds unique destinations and clones each protected source once', async () => {
    const cfg = buildConfig();
    cfg.destinations = [
      { id: 'primary', path: '/backup', label: 'Primary' },
      { id: 'dest', path: '/existing', label: 'Destination 2' },
    ];
    cfg.watched = [
      { path: '/source', kind: 'Directory', enabled: true, destination_id: 'primary' },
      { path: '/source', kind: 'Directory', enabled: true, destination_id: 'dest' },
    ];
    const pickDestinationPath = vi
      .fn<Params['pickers']['pickDestinationPath']>()
      .mockResolvedValueOnce('/existing')
      .mockResolvedValueOnce('/mirror');
    const { actions, persist, onEvent } = renderActions({
      cfg,
      pickers: { pickDestinationPath, pickPathForDest: vi.fn(async () => undefined) },
    });

    await act(async () => {
      await actions.addDestination();
      await actions.addDestination();
    });

    expect(onEvent).toHaveBeenCalledWith('That destination is already configured.', 'info');
    const saved = persist.mock.calls[0]?.[0] as Config;
    expect(saved.destinations.at(-1)).toMatchObject({ id: 'dest-2', path: '/mirror' });
    expect(saved.watched.filter((entry) => entry.destination_id === 'dest-2')).toHaveLength(1);
  });

  it('guards destination removal and backfills uncovered sources', async () => {
    const single = renderActions();
    await act(async () => single.actions.removeDestination('primary'));
    expect(single.onEvent).toHaveBeenCalledWith('At least one destination is required.', 'info');

    const cfg = buildConfig();
    cfg.destinations = [
      { id: 'primary', path: '/backup', replicate_to: ['mirror'] },
      { id: 'mirror', path: '/mirror' },
    ];
    cfg.watched = [
      { path: '/both', kind: 'Directory', enabled: true, destination_id: 'primary' },
      { path: '/both', kind: 'Directory', enabled: true, destination_id: 'mirror' },
      { path: '/mirror-only', kind: 'File', enabled: true, destination_id: 'mirror' },
    ];
    const guarded = renderActions({ cfg });
    await act(async () => {
      await guarded.actions.removeDestination('missing');
      await guarded.actions.removeDestination('mirror');
    });

    expect(guarded.onEvent).toHaveBeenCalledWith(
      'Destination no longer exists. Reload and try again.',
      'error',
    );
    const saved = guarded.persist.mock.calls[0]?.[0] as Config;
    expect(saved.destinations).toEqual([
      expect.objectContaining({ id: 'primary', replicate_to: [] }),
    ]);
    expect(saved.watched).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ path: '/both', destination_id: 'primary' }),
        expect.objectContaining({ path: '/mirror-only', destination_id: 'primary' }),
      ]),
    );
    expect(saved.watched).toHaveLength(2);
  });

  it('adds a picked path only to destinations that do not already protect it', async () => {
    const cfg = buildConfig();
    cfg.destinations.push({ id: 'mirror', path: '/mirror' });
    cfg.watched = [
      { path: '/source', kind: 'Directory', enabled: true, destination_id: 'primary' },
    ];
    const pickPathForDest = vi.fn<Params['pickers']['pickPathForDest']>(
      async (_destinationId, kind, addWatched) => {
        await addWatched('/source', kind, 'primary');
      },
    );
    const { actions, persist } = renderActions({
      cfg,
      pickers: { pickDestinationPath: vi.fn(async () => null), pickPathForDest },
    });

    await act(async () => actions.addFolder());

    const saved = persist.mock.calls[0]?.[0] as Config;
    expect(saved.watched).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ path: '/source', destination_id: 'primary' }),
        expect.objectContaining({ path: '/source', destination_id: 'mirror' }),
      ]),
    );
    expect(persist).toHaveBeenCalledWith(expect.anything(), 'Added /source to 1 destination.');
  });

  it('handles duplicate paths, picker rejection, and inner persistence failures', async () => {
    const cfg = buildConfig();
    cfg.watched = [{ path: '/source', kind: 'File', enabled: true, destination_id: 'primary' }];
    const duplicatePicker = vi.fn<Params['pickers']['pickPathForDest']>(
      async (_destinationId, kind, addWatched) => {
        await addWatched('/source', kind, 'primary');
      },
    );
    const duplicate = renderActions({
      cfg,
      pickers: { pickDestinationPath: vi.fn(async () => null), pickPathForDest: duplicatePicker },
    });
    await act(async () => duplicate.actions.addFile());
    expect(duplicate.onEvent).toHaveBeenCalledWith(
      'That path is already protected for all destinations.',
      'info',
    );

    duplicate.persist.mockRejectedValueOnce(new Error('disk full'));
    cfg.watched = [];
    await act(async () => duplicate.actions.addFile());
    expect(duplicate.onEvent).toHaveBeenCalledWith(
      '[useMinimalActions] Failed to add path: disk full',
      'error',
    );

    const pickerFailure = renderActions({
      pickers: {
        pickDestinationPath: vi.fn(async () => null),
        pickPathForDest: vi.fn(async () => {
          throw new Error('cancel transport failed');
        }),
      },
    });
    await act(async () => pickerFailure.actions.addFolder());
    expect(pickerFailure.onEvent).toHaveBeenCalledWith(
      '[useMinimalActions] Path picker failed: cancel transport failed',
      'error',
    );
  });

  it('removes paths, clamps retention, and moves destination assignments safely', async () => {
    const cfg = buildConfig();
    cfg.destinations.push({ id: 'mirror', path: '/mirror' });
    cfg.watched = [
      { path: '/source', kind: 'Directory', enabled: true, destination_id: 'primary' },
      { path: '/source', kind: 'Directory', enabled: true, destination_id: 'mirror' },
      { path: '/other', kind: 'File', enabled: true, destination_id: 'primary' },
    ];
    const { actions, persist, onEvent } = renderActions({ cfg });

    await act(async () => {
      await actions.removePath('/other', 'File', 'primary');
      await actions.updateKeep('/source', 'Directory', 'primary', 5000);
      await actions.updateDestination('/source', 'Directory', 'primary', 'missing');
      await actions.updateDestination('/source', 'Directory', 'primary', 'primary');
      await actions.updateDestination('/source', 'Directory', 'primary', 'mirror');
    });

    const removed = persist.mock.calls[0]?.[0] as Config;
    expect(removed.watched.some((entry) => entry.path === '/other')).toBe(false);
    const retention = persist.mock.calls[1]?.[0] as Config;
    expect(
      retention.watched.find(
        (entry) => entry.path === '/source' && entry.destination_id === 'primary',
      )?.max_backups_per_file,
    ).toBe(1000);
    expect(onEvent).toHaveBeenCalledWith(
      'Destination no longer exists. Reload and try again.',
      'error',
    );
    const deduplicated = persist.mock.calls[2]?.[0] as Config;
    expect(deduplicated.watched.filter((entry) => entry.path === '/source')).toHaveLength(1);
    expect(onEvent).toHaveBeenCalledWith(
      'Path already exists at that destination, so the source destination entry was removed.',
      'info',
    );
  });

  it('moves a unique destination assignment without changing unrelated entries', async () => {
    const cfg = buildConfig();
    cfg.destinations.push({ id: 'mirror', path: '/mirror' });
    cfg.watched = [
      { path: '/source', kind: 'Directory', enabled: true, destination_id: 'primary' },
      { path: '/other', kind: 'File', enabled: true, destination_id: 'primary' },
    ];
    const { actions, persist } = renderActions({ cfg });

    await act(async () => {
      await actions.updateDestination('/source', 'Directory', 'primary', 'mirror');
    });

    const saved = persist.mock.calls[0]?.[0] as Config;
    expect(saved.watched).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ path: '/source', destination_id: 'mirror' }),
        expect.objectContaining({ path: '/other', destination_id: 'primary' }),
      ]),
    );
  });
});
