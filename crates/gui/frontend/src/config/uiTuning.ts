/** Make UI behavior adjustments simple and consistent. */
export const UI_TUNING = {
  planTooLargeCode: 'PLAN_TOO_LARGE',
  statusRefreshMs: 10_000,
  liveHealthRefreshMs: 20_000,
  toastDismissMs: 4_000,
  lowSpaceThresholdBytes: 2 * 1024 * 1024 * 1024,
  resumeOnSpaceStorageKey: 'resume_on_space',
  onboardingDoneStorageKey: 'onboarding_done',
  hardeningDoneStorageKey: 'hardening_done',
  colorModeStorageKey: 'color_mode_preference',
  runNow: {
    maxLogLines: 30,
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
    configSaveTimeoutMs: 30_000,
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
