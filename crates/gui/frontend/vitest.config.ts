import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["./vitest.setup.ts"],
    include: ["src/utils/__tests__/**/*.test.ts", "src/**/*.test.tsx"],
    coverage: {
      provider: "v8",
      reporter: ["text", "lcov"],
      lines: 70,
      branches: 60,
      functions: 70,
      statements: 70,
      exclude: [
        "src/main.tsx",
        "src/App.tsx",
        "src/styles.css",
        "**/*.d.ts",
      ],
    },
  },
});
