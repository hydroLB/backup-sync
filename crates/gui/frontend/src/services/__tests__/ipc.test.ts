import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  IpcError,
  loadTauriInvoke,
  safeInvoke,
  safeInvokeWithTimeout,
  tauriAvailable,
  wrapError,
} from '../ipc';

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

function setBridge(available: boolean): void {
  Object.defineProperty(window, '__TAURI_INTERNALS__', {
    configurable: true,
    value: available ? { invoke: vi.fn() } : undefined,
  });
  Object.defineProperty(window, '__TAURI_IPC__', {
    configurable: true,
    value: undefined,
  });
}

describe('IPC service boundary', () => {
  beforeEach(() => {
    vi.useRealTimers();
    invokeMock.mockReset();
    setBridge(true);
  });

  it('detects unavailable native IPC and rejects guarded calls', async () => {
    setBridge(false);

    expect(tauriAvailable()).toBe(false);
    await expect(safeInvoke('status_cmd')).rejects.toMatchObject({
      name: 'IpcError',
      code: 'TAURI_UNAVAILABLE',
    });
    await expect(safeInvokeWithTimeout('status_cmd', undefined, 5)).rejects.toMatchObject({
      code: 'TAURI_UNAVAILABLE',
    });
  });

  it('recognizes the legacy Tauri bridge during migration', () => {
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      value: undefined,
    });
    Object.defineProperty(window, '__TAURI_IPC__', {
      configurable: true,
      value: vi.fn(),
    });
    expect(tauriAvailable()).toBe(true);
  });

  it('normalizes errors, backend payloads, primitives, and circular objects', () => {
    const coded = wrapError('save', new IpcError('denied', 'PERMISSION_DENIED'));
    expect(coded).toMatchObject({ message: 'save: denied', code: 'PERMISSION_DENIED' });
    expect(coded.details).toBeInstanceOf(IpcError);

    expect(wrapError('load', new Error('offline')).message).toBe('load: offline');
    expect(wrapError('load', { message: 'bad config', code: 'CONFIG_INVALID' })).toMatchObject({
      message: 'load: bad config',
      code: 'CONFIG_INVALID',
    });
    expect(wrapError('load', { error: 'backend failed' }).message).toBe('load: backend failed');
    expect(wrapError('load', { detail: 42 }).message).toBe('load: {"detail":42}');
    expect(wrapError('load', 17).message).toBe('load: 17');

    const circular: Record<string, unknown> = {};
    circular.self = circular;
    expect(wrapError('load', circular).message).toContain('JSON stringify failed');
  });

  it('invokes native commands and preserves backend codes on failure', async () => {
    invokeMock.mockResolvedValueOnce({ healthy: true });

    await expect(safeInvoke('health_cmd', { verbose: true })).resolves.toEqual({ healthy: true });
    expect(invokeMock).toHaveBeenCalledWith('health_cmd', { verbose: true });
    expect(tauriAvailable()).toBe(true);
    await expect(loadTauriInvoke()).resolves.toHaveProperty('invoke');

    invokeMock.mockRejectedValueOnce({ message: 'destination offline', code: 'IO_UNAVAILABLE' });
    await expect(safeInvoke('run_cmd')).rejects.toMatchObject({
      code: 'IO_UNAVAILABLE',
      message: 'IPC run_cmd failed: destination offline',
    });
  });

  it('times out a stalled command and accepts finite timeout fallbacks', async () => {
    vi.useFakeTimers();
    invokeMock.mockReturnValueOnce(new Promise(() => undefined));

    const timedOut = safeInvokeWithTimeout('slow_cmd', undefined, 25);
    const timeoutAssertion = expect(timedOut).rejects.toMatchObject({
      code: 'IPC_TIMEOUT',
      message: expect.stringContaining('slow_cmd failed'),
    });
    await vi.advanceTimersByTimeAsync(25);
    await timeoutAssertion;

    invokeMock.mockResolvedValueOnce('ok');
    await expect(safeInvokeWithTimeout('fast_cmd', {}, Number.NaN)).resolves.toBe('ok');

    invokeMock.mockImplementationOnce(
      () => new Promise((resolve) => setTimeout(() => resolve('restored'), 6_000)),
    );
    const unbounded = safeInvokeWithTimeout('restore_cmd', {}, 0);
    await vi.advanceTimersByTimeAsync(6_000);
    await expect(unbounded).resolves.toBe('restored');
  });
});
