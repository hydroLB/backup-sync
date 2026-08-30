# Web Edition Contract

The [hosted edition](https://backup-sync-web-edition.young-hen-7947.chatgpt.site) renders the same React application as the Tauri desktop build. There is no separate demo screen: the build-time runtime adapter changes how the existing controls are fulfilled, not which interface is rendered.

## What is real

- Imported files are read as bytes through the browser file picker.
- SHA-256 hashes are computed with the Web Crypto API.
- Immutable manifests and content-addressed blobs are persisted in IndexedDB.
- Repeated content is deduplicated across versions.
- Verify re-hashes referenced blobs; restore validates hashes and relative paths before replacing browser-workspace data.
- A primary vault and local mirror exercise replication and repair behavior.
- A foreground 30-minute cycle runs while the page remains open and Running is enabled.

Browser data stays in that browser profile. The edition has no upload or remote storage path.

## Deliberate browser boundary

The hosted edition cannot run a daemon after the page closes, inspect arbitrary filesystem paths, create OS snapshots, integrate with the system tray, or provide desktop crash-durability guarantees. Those capabilities belong to the Rust engine and full desktop application. The website is therefore a real browser-local implementation of the product workflow, not a claim that browser storage is equivalent to a native backup destination.

The website-only **Download now** action points to `VITE_DESKTOP_DOWNLOAD_URL` (or the repository's latest-release page by default). It must refer only to a signed full native application. The website is never presented as an installer or substitute download.

## Development

From `crates/gui/frontend`:

```bash
npm run dev:web
npm run build:web
```

Set `VITE_DESKTOP_DOWNLOAD_URL` to the signed release asset URL for a production publish. The native frontend remains `npm run dev` / `npm run build`; build-time aliases ensure Tauri bridge modules are excluded from the hosted bundle.
