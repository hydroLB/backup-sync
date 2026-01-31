import { correlationId } from "./correlation";
import { safeInvoke } from "./ipc";
import { AccessProbe, DestinationCheck, ServiceStatusDto } from "./types";
import { UI_TUNING } from "../config/uiTuning";

/**
 * Purpose: Apply jitter to a base delay value.
 *
 * Inputs: Base delay in milliseconds and jitter percentage.
 * Outputs: A jittered delay in milliseconds.
 * Ties to: Service retry backoff in this module.
 * Side effects: Uses `Math.random` for jitter.
 * Why: Reduce coordinated retries across clients.
 */
function applyJitter(delayMs: number, jitterPct: number): number {
  try {
    if (delayMs <= 0 || jitterPct <= 0) return delayMs;
    const pct = Math.min(Math.max(jitterPct, 0), 1);
    const delta = delayMs * pct;
    const rand = Math.random() * (delta * 2) - delta;
    return Math.max(0, Math.round(delayMs + rand));
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[applyJitter] Failed to apply jitter: ${reason}`);
  }
}

/**
 * Purpose: Run an async function with a fixed retry backoff.
 *
 * Inputs: `label` for error context, `fn` as the async operation.
 * Outputs: The resolved value or a thrown error with context.
 * Ties to: Service actions that may race IPC startup.
 * Side effects: Delays execution via timers between retries.
 * Why: Reduces transient failures during service actions.
 */
async function withBackoff<T>(label: string, fn: () => Promise<T>): Promise<T> {
  try {
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
    const message = lastErr instanceof Error ? lastErr.message : String(lastErr);
    throw new Error(`[withBackoff] Exhausted retries for ${label}: ${message}`);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[withBackoff] Backoff execution failed for ${label}: ${reason}`);
  }
}

/**
 * Purpose: Install the background service via the backend.
 *
 * Inputs: None.
 * Outputs: A backend status message.
 * Ties to: Start on login UI actions.
 * Side effects: Invokes IPC calls that install or update service state.
 * Why: Enables daemon autostart from the UI.
 */
export async function installService(): Promise<string> {
  try {
    return await withBackoff("installService", () => safeInvoke("install_service_cmd"));
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[installService] Failed to install service: ${reason}`);
  }
}

/**
 * Purpose: Restart the daemon via the backend.
 *
 * Inputs: None.
 * Outputs: A backend status message.
 * Ties to: Restart UI actions.
 * Side effects: Invokes IPC calls that restart the daemon service.
 * Why: Allows users to recover a stuck daemon.
 */
export async function restartDaemon(): Promise<string> {
  try {
    return await withBackoff("restartDaemon", () => safeInvoke("restart_daemon_cmd"));
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[restartDaemon] Failed to restart daemon: ${reason}`);
  }
}

/**
 * Purpose: Check for updates using the backend update feed.
 *
 * Inputs: None.
 * Outputs: A status message string.
 * Ties to: Update UI actions and release notifications.
 * Side effects: Invokes IPC calls that access update metadata.
 * Why: Reports update availability without leaving the app.
 */
export async function checkUpdates(): Promise<string> {
  try {
    return await safeInvoke("check_updates_cmd");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`[checkUpdates] Failed to check for updates: ${message}`);
  }
}

/**
 * Purpose: Validate a destination path via the backend.
 *
 * Inputs: `path` as the destination path string.
 * Outputs: A `DestinationCheck` payload.
 * Ties to: Destination picker validation.
 * Side effects: Invokes IPC calls that probe filesystem access.
 * Why: Surfaces validation results to the UI.
 */
export async function checkDestination(path: string): Promise<DestinationCheck> {
  try {
    return await safeInvoke<DestinationCheck>("check_destination_cmd", { path });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`[checkDestination] Failed to validate destination: ${message}`);
  }
}

/**
 * Purpose: Test access permissions for watched paths and destinations.
 *
 * Inputs: None.
 * Outputs: An access probe payload.
 * Ties to: Diagnostics and settings screens.
 * Side effects: Invokes IPC calls that probe filesystem permissions.
 * Why: Surfaces permission issues in the UI.
 */
export async function testAccess(): Promise<AccessProbe> {
  try {
    return await safeInvoke<AccessProbe>("test_access_cmd");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`[testAccess] Failed to test access permissions: ${message}`);
  }
}

/**
 * Purpose: Generate a doctor report via the backend.
 *
 * Inputs: None.
 * Outputs: The report path string.
 * Ties to: Diagnostics actions.
 * Side effects: Invokes IPC calls that write diagnostic reports.
 * Why: Allows users to export diagnostics quickly.
 */
export async function doctorReport(): Promise<string> {
  try {
    return await safeInvoke("doctor_report_cmd", {
      correlationId: correlationId("doctor"),
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`[doctorReport] Failed to generate doctor report: ${message}`);
  }
}

/**
 * Purpose: Check the background service status via the backend.
 *
 * Inputs: None.
 * Outputs: A `ServiceStatusDto` payload.
 * Ties to: Service status UI widgets.
 * Side effects: Invokes IPC calls to query service status.
 * Why: Displays service installation and reachability.
 */
export async function checkService(): Promise<ServiceStatusDto> {
  try {
    return await safeInvoke<ServiceStatusDto>("check_service_cmd");
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`[checkService] Failed to check service status: ${message}`);
  }
}

/**
 * Purpose: Export a diagnostic bundle via the backend.
 *
 * Inputs: None.
 * Outputs: The bundle path string.
 * Ties to: Diagnostics export actions.
 * Side effects: Invokes IPC calls that write diagnostic bundles.
 * Why: Allows users to share a full support bundle.
 */
export async function exportDiagnosticBundle(): Promise<string> {
  try {
    return await safeInvoke("export_diagnostic_bundle_cmd", {
      correlationId: correlationId("diag"),
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`[exportDiagnosticBundle] Failed to export diagnostic bundle: ${message}`);
  }
}
