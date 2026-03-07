import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

/**
 * Summary: Compute a stable 32-bit FNV-1a hash for an input string.
 *
 * Inputs: `input` as a string.
 *
 * Outputs: A 32-bit unsigned integer hash.
 *
 * Side effects: None.
 *
 * Error handling: None (pure computation).
 *
 * Ties to other methods: Used by `stableDevPort`.
 *
 * Why this exists: Derive a deterministic per-repo port without hardcoding.
 */
function fnv1a32(input: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < input.length; i++) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

/**
 * Summary: Choose a deterministic dev server port in the ephemeral range.
 *
 * Inputs: Reads `process.cwd()` to scope uniqueness to the working directory.
 *
 * Outputs: A port number in 49152–65535.
 *
 * Side effects: Reads process state (`cwd`).
 *
 * Error handling: None (falls back via arithmetic only).
 *
 * Ties to other methods: Used by `resolveDevPort`.
 *
 * Why this exists: Avoid collisions with other projects and well-known ports.
 */
function stableDevPort(): number {
  const base = 49152;
  const range = 65535 - base + 1;
  return base + (fnv1a32(process.cwd()) % range);
}

/**
 * Summary: Resolve the dev server port from env vars with validation.
 *
 * Inputs: `BACKUP_SYNC_DEV_PORT` or `VITE_PORT` when set.
 *
 * Outputs: A validated port (0–65535) or a deterministic fallback.
 *
 * Side effects: Reads environment variables.
 *
 * Error handling: Invalid values fall back to `stableDevPort`.
 *
 * Ties to other methods: Used by Vite `server.port`.
 *
 * Why this exists: Allow overrides while keeping default behavior collision-resistant.
 */
function resolveDevPort(): number {
  const raw = process.env.BACKUP_SYNC_DEV_PORT ?? process.env.VITE_PORT;
  if (!raw) return stableDevPort();
  const parsed = Number(raw);
  if (!Number.isInteger(parsed) || parsed < 0 || parsed > 65535) {
    return stableDevPort();
  }
  return parsed;
}

export default defineConfig({
  plugins: [react()],
  server: {
    host: "127.0.0.1",
    port: resolveDevPort(),
    strictPort: false,
  },
});
