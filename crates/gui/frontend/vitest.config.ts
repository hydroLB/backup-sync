import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    environment: 'jsdom',
    pool: 'threads',
    fileParallelism: false,
    maxWorkers: 1,
    setupFiles: ['./vitest.setup.ts'],
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'lcov'],
      thresholds: {
        lines: 27,
        branches: 53,
        functions: 25,
        statements: 27,
      },
      exclude: ['src/main.tsx', 'src/App.tsx', 'src/styles.css', '**/*.d.ts'],
    },
  },
});
