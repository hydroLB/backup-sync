# Backup Sync browser edition

Try versioned backup and recovery on a browser-local sample workspace. Add a sample file, run a backup, change its contents, and restore a recorded version. The recovery receipt preserves the completed operation's counts, including newer versions intentionally kept.

This React/TypeScript interface has separate browser and Tauri runtime adapters. The browser demonstration uses local sample storage; the Rust desktop engine handles real filesystem snapshots and background protection. See [the main README](../../../README.md) for the desktop architecture and setup. Browser data is specific to this browser profile and is not protection for the visitor's real folders.

## Run and build

Use Node 22.22 or newer in the Node 22 series. From this directory:

```bash
npm ci
npm run dev:web
npm test
npm run typecheck
npm run build:web
```

The explicit web commands select the browser adapter. `npm run dev` and `npm run build` are also used by the desktop workflow. No account or API key is required for the web edition.
