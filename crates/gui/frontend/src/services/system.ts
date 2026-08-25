import { correlationId } from './correlation';
import { safeInvoke, safeInvokeWithTimeout, wrapError } from './ipc';
import { AccessProbe, DestinationCheck, HardeningReport, ServiceStatusDto } from './types';
import { UI_TUNING } from '../config/uiTuning';

/** Reduce coordinated retries across clients. */
function applyJitter(delayMs: number, jitterPct: number): number {
  try {
    if (delayMs <= 0 || jitterPct <= 0) return delayMs;
    const pct = Math.min(Math.max(jitterPct, 0), 1);
    const delta = delayMs * pct;
    const rand = Math.random() * (delta * 2) - delta;
    return Math.max(0, Math.round(delayMs + rand));
  } catch (error) {
    throw wrapError('[applyJitter] Failed to apply jitter', error);
  }
}

/** Reduces transient failures during service actions. */
async function withBackoff<T>(label: string, fn: () => Promise<T>): Promise<T> {
  let lastErr: unknown;
  for (const delayMs of UI_TUNING.system.retryDelaysMs) {
    if (delayMs > 0) {
      const jittered = applyJitter(delayMs, UI_TUNING.system.retryJitterPct);
      await new Promise((resolve) => setTimeout(resolve, jittered));
    }
    try {
      return await fn();
    } catch (err) {
      lastErr = err;
    }
  }
  throw wrapError(`[withBackoff] Exhausted retries for ${label}`, lastErr);
}

/** Enables daemon autostart from the UI. */
export async function installService(): Promise<string> {
  try {
    return await withBackoff('installService', () => safeInvoke('install_service_cmd'));
  } catch (error) {
    throw wrapError('[installService] Failed to install service', error);
  }
}

/** Allows users to recover a stuck daemon. */
export async function restartDaemon(): Promise<string> {
  try {
    return await withBackoff('restartDaemon', () => safeInvoke('restart_daemon_cmd'));
  } catch (error) {
    throw wrapError('[restartDaemon] Failed to restart daemon', error);
  }
}

/** Reports update availability without leaving the app. */
export async function checkUpdates(): Promise<string> {
  try {
    return await safeInvoke('check_updates_cmd');
  } catch (error) {
    throw wrapError('[checkUpdates] Failed to check for updates', error);
  }
}

/** Surfaces validation results to the UI. */
export async function checkDestination(path: string): Promise<DestinationCheck> {
  try {
    return await safeInvoke<DestinationCheck>('check_destination_cmd', { path });
  } catch (error) {
    throw wrapError('[checkDestination] Failed to validate destination', error);
  }
}

/** Surfaces permission issues in the UI. */
export async function testAccess(): Promise<AccessProbe> {
  try {
    return await safeInvoke<AccessProbe>('test_access_cmd');
  } catch (error) {
    throw wrapError('[testAccess] Failed to test access permissions', error);
  }
}

/** Avoid enabling background writes before filesystem access is known-good. */
export async function hardeningCheck(opts?: {
  check_snapshots?: boolean;
  require_snapshots?: boolean;
}): Promise<HardeningReport> {
  try {
    // Hardening can legitimately take longer than the global IPC timeout on slow disks.
    // Keep this bounded so the UI never appears "stuck" indefinitely.
    return await safeInvokeWithTimeout<HardeningReport>(
      'hardening_check_cmd',
      {
        request: {
          check_snapshots: !!opts?.check_snapshots,
          require_snapshots: !!opts?.require_snapshots,
        },
      },
      15_000,
    );
  } catch (error) {
    throw wrapError('[hardeningCheck] Failed to run hardening checks', error);
  }
}

/** Allows users to export diagnostics quickly. */
export async function doctorReport(): Promise<string> {
  try {
    return await safeInvoke('doctor_report_cmd', {
      correlationId: correlationId('doctor'),
    });
  } catch (error) {
    throw wrapError('[doctorReport] Failed to generate doctor report', error);
  }
}

/** Displays service installation and reachability. */
export async function checkService(): Promise<ServiceStatusDto> {
  try {
    return await safeInvoke<ServiceStatusDto>('check_service_cmd');
  } catch (error) {
    throw wrapError('[checkService] Failed to check service status', error);
  }
}

/** Allows users to share a full support bundle. */
export async function exportDiagnosticBundle(): Promise<string> {
  try {
    return await safeInvoke('export_diagnostic_bundle_cmd', {
      correlationId: correlationId('diag'),
    });
  } catch (error) {
    throw wrapError('[exportDiagnosticBundle] Failed to export diagnostic bundle', error);
  }
}
