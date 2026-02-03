/**
 * Purpose: Centralize UI tuning knobs and defaults used across the frontend.
 *
 * Inputs: None.
 * Outputs: Exported constants for UI behaviors.
 * Ties to: Status polling, toast timing, and storage keys in settings.
 * Side effects: None.
 * Why: Make UI behavior adjustments simple and consistent.
 */
export const UI_TUNING = {
  planTooLargeCode: 'PLAN_TOO_LARGE',
  statusRefreshMs: 10_000,
  toastDismissMs: 4_000,
  lowSpaceThresholdBytes: 2 * 1024 * 1024 * 1024,
  resumeOnSpaceStorageKey: 'resume_on_space',
  onboardingDoneStorageKey: 'onboarding_done',
  runNow: {
    maxLogLines: 30,
    logPreviewMaxHeightPx: 200,
    logPreviewPaddingPx: 10,
    logPreviewRadiusPx: 10,
  },
  actionLogLimit: 30,
  activityFeedLimit: 5,
  offlineNoticeMessage: 'Service not reachable yet. Start the daemon or finish setup to connect.',
  advancedJsonIndent: 2,
  verifyEmptyStatus: 'Not run yet',
  configLimits: {
    maxIgnorePatterns: 200,
  },
  system: {
    retryDelaysMs: [0, 200, 400, 800],
    retryJitterPct: 0.2,
    ipcTimeoutMs: 5_000,
  },
  performancePresets: {
    quiet: { max_parallel_copies: 1, max_bytes_per_second: 5 * 1024 * 1024 },
    balanced: { max_parallel_copies: 2, max_bytes_per_second: null as number | null },
    fast: { max_parallel_copies: 4, max_bytes_per_second: null as number | null },
  },
  commonIgnorePatterns: [
    '**/node_modules/**',
    '**/target/**',
    '**/build/**',
    '**/*.log',
    '**/tmp/**',
  ],
};
