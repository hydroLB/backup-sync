import { correlationId } from "./correlation";
import { SimulationResult, VerifyResult } from "./types";
import { safeInvoke } from "./ipc";

/**
 * Purpose: Trigger an immediate backup run.
 *
 * Inputs: None.
 * Outputs: Resolves when the run completes.
 * Ties to: The Run Now UI action and backend run command.
 * Side effects: Invokes IPC calls that trigger backup execution.
 * Why: Allows on demand backups from the UI.
 */
export async function runNow(): Promise<void> {
  try {
    return await safeInvoke("run_now_cmd", { correlationId: correlationId("run") });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`[runNow] Failed to trigger backup run: ${message}`);
  }
}

/**
 * Purpose: Run a backup simulation without writing files.
 *
 * Inputs: None.
 * Outputs: A `SimulationResult` payload.
 * Ties to: The Simulation UI action and backend simulation command.
 * Side effects: Invokes IPC calls that run simulation workloads.
 * Why: Allows previewing planned changes before running.
 */
export async function runSimulation(): Promise<SimulationResult> {
  try {
    return await safeInvoke("run_simulate_cmd", { correlationId: correlationId("sim") });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`[runSimulation] Failed to simulate backup: ${message}`);
  }
}

/**
 * Purpose: Verify the latest backups for integrity.
 *
 * Inputs: None.
 * Outputs: A `VerifyResult` payload.
 * Ties to: The Verify UI action and backend verification command.
 * Side effects: Invokes IPC calls that read backup data.
 * Why: Allows users to validate backup correctness.
 */
export async function verifyBackups(): Promise<VerifyResult> {
  try {
    return await safeInvoke("verify_cmd", { correlationId: correlationId("verify") });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`[verifyBackups] Failed to verify backups: ${message}`);
  }
}

/**
 * Purpose: Export a health report to the Desktop.
 *
 * Inputs: None.
 * Outputs: The path to the exported report.
 * Ties to: Health report export actions and backend diagnostics.
 * Side effects: Invokes IPC calls that write diagnostic reports.
 * Why: Allows sharing diagnostics from the UI.
 */
export async function exportHealthReport(): Promise<string> {
  try {
    return await safeInvoke("export_health_report_cmd", {
      correlationId: correlationId("health"),
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`[exportHealthReport] Failed to export health report: ${message}`);
  }
}
