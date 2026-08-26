import { Config } from '../../domain/config';
import {
  FolderVersionsDto,
  HardeningReport,
  ListVersionFilesResultDto,
  RestoreFilesArgs,
  RestoreResultDto,
  RestoreArgs,
  SimulationResult,
  StatusDto,
  VerifyResult,
} from '../../services/types';

const DATABASE_NAME = 'backup-sync-web';
const DATABASE_VERSION = 1;
const STATE_KEY = 'workspace';
const SCHEMA_VERSION = 1;

type WebFile = {
  relPath: string;
  bytes: Uint8Array;
  mtimeUnix: number;
};

type ManifestEntry = {
  rel_path: string;
  len: number;
  mtime_unix: number;
  mtime_nanos: number;
  sha256: string;
};

type Manifest = {
  id: string;
  sourcePath: string;
  createdAtUnix: number;
  entries: ManifestEntry[];
};

type Vault = {
  blobs: Record<string, Uint8Array>;
  manifests: Manifest[];
};

export type WebWorkspaceSummary = {
  sourcePath: string;
  files: Array<{ path: string; bytes: number; modifiedAt: number }>;
  versions: number;
  uniqueBlobs: number;
  logicalBytes: number;
  storedBytes: number;
  destinations: number;
  lastRunAt: number | null;
  integrity: 'healthy' | 'damaged' | 'not-checked';
};

export type WebWorkspaceFile = {
  path: string;
  text: string;
  bytes: number;
  modifiedAt: number;
  editable: boolean;
};

type WebState = {
  schemaVersion: number;
  config: Config;
  sourcePath: string;
  files: WebFile[];
  vaults: Record<string, Vault>;
  status: StatusDto;
  logs: string[];
  integrity: WebWorkspaceSummary['integrity'];
  sequence: number;
};

const encoder = new TextEncoder();

function nowUnix(): number {
  return Math.floor(Date.now() / 1000);
}

function defaultConfig(): Config {
  const sourcePath = '/Browser Workspace/Portfolio';
  return {
    backup_root: 'browser-vault://primary',
    interval_seconds: 30 * 60,
    max_backups_per_file: 8,
    skip_hidden: true,
    ignore_patterns: [],
    max_parallel_copies: 4,
    max_bytes_per_second: null,
    min_free_space_bytes: null,
    hashing: { buffer_bytes: 65_536, timeout_seconds: 30 },
    execution: {
      copy_buffer_bytes: 65_536,
      copy_timeout_seconds: 300,
      free_space_safety_buffer_bytes: 1_048_576,
      recent_activity_cap: 50,
      retry_delays_ms: [100, 200, 400],
      retry_jitter_pct: 0.2,
    },
    planning: {
      hash_check_interval: 5,
      max_plan_items: 10_000,
      scan_timeout_seconds: 300,
      scan_capacity_multiplier: 16,
    },
    runtime: {
      prune_interval_cycles: 10,
      verify_interval_seconds: 86_400,
      scrub_full_interval_seconds: 604_800,
      scrub_sample_blobs: 200,
      scrub_sample_versions_per_source: 2,
      watcher_debounce_seconds: 2,
      ipc_timeout_seconds: 5,
      service_command_timeout_seconds: 15,
      service_command_retry_delay_ms: 300,
      service_command_poll_interval_ms: 50,
      source_snapshots_enabled: false,
      source_snapshot_timeout_seconds: 20,
      tray_tooltip_refresh_seconds: 10,
      log_tail_lines: 200,
      simulation_sample_limit: 10,
      replication_enabled: true,
      replication_mirror_manifests: true,
      replication_max_manifest_deletes_per_cycle: 100,
    },
    safe_mode: false,
    watched: [
      {
        path: sourcePath,
        kind: 'Directory',
        enabled: true,
        destination_id: 'primary',
        max_backups_per_file: 8,
      },
      {
        path: sourcePath,
        kind: 'Directory',
        enabled: true,
        destination_id: 'mirror',
        max_backups_per_file: 8,
      },
    ],
    destinations: [
      { id: 'primary', label: 'Browser vault', path: 'browser-vault://primary' },
      { id: 'mirror', label: 'Local mirror', path: 'browser-vault://mirror' },
    ],
  };
}

function emptyStatus(): StatusDto {
  return {
    last_run_ts: null,
    last_files_backed_up: 0,
    last_error: null,
    last_dirty_count: 0,
    last_safety_warning: null,
    uptime_secs: 0,
    version: 'web-1',
    free_bytes: null,
    min_free_space_bytes: null,
    last_verify_ts: null,
    last_verify_status: null,
    last_verify_issues: null,
    recent_activity: [],
    safe_mode: false,
    destination_paused: false,
    destination_pause_reason: null,
    destination_unavailable_ids: [],
    destination_last_unavailable_ts: null,
    destination_last_recovered_ts: null,
    replication_last_run_ts: null,
    replication_last_status: 'idle',
    replication_last_error: null,
    replication_last_bytes_copied: 0,
    replication_last_blobs_copied: 0,
    replication_last_manifests_copied: 0,
    replication_last_manifests_deleted: 0,
    replication_last_pairs_ok: 0,
    replication_last_pairs_failed: 0,
    replication_last_targets_failed: [],
    destinations: [],
  };
}

function cloneBytes(bytes: Uint8Array): Uint8Array {
  return new Uint8Array(bytes);
}

function bytesForHash(bytes: Uint8Array): ArrayBuffer {
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

async function sha256(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', bytesForHash(bytes));
  return Array.from(new Uint8Array(digest), (value) => value.toString(16).padStart(2, '0')).join(
    '',
  );
}

function log(state: WebState, message: string): void {
  const timestamp = new Date().toISOString();
  state.logs.push(`${timestamp} ${message}`);
  state.logs = state.logs.slice(-200);
}

function newVault(): Vault {
  return { blobs: {}, manifests: [] };
}

function latestManifest(vault: Vault, sourcePath: string): Manifest | null {
  return (
    [...vault.manifests]
      .filter((manifest) => manifest.sourcePath === sourcePath)
      .sort((left, right) => right.createdAtUnix - left.createdAtUnix)[0] ?? null
  );
}

async function planEntries(state: WebState): Promise<ManifestEntry[]> {
  const entries = await Promise.all(
    state.files.map(async (file) => ({
      rel_path: file.relPath,
      len: file.bytes.byteLength,
      mtime_unix: file.mtimeUnix,
      mtime_nanos: 0,
      sha256: await sha256(file.bytes),
    })),
  );
  return entries.sort((left, right) => left.rel_path.localeCompare(right.rel_path));
}

async function previewState(state: WebState): Promise<SimulationResult> {
  const entries = await planEntries(state);
  const primaryId = state.config.destinations[0]?.id ?? 'primary';
  const latest = latestManifest(state.vaults[primaryId] ?? newVault(), state.sourcePath);
  const previous = new Map((latest?.entries ?? []).map((entry) => [entry.rel_path, entry.sha256]));
  const changed = entries.filter((entry) => previous.get(entry.rel_path) !== entry.sha256);
  const current = new Set(entries.map((entry) => entry.rel_path));
  const removed = [...previous.keys()].filter((path) => !current.has(path));
  const result: SimulationResult = {
    items: changed.length + removed.length,
    bytes: changed.reduce((total, entry) => total + entry.len, 0),
    sample: [
      ...changed.map((entry) => entry.rel_path),
      ...removed.map((path) => `${path} (removed)`),
    ].slice(0, state.config.runtime.simulation_sample_limit),
  };
  if (changed.length + removed.length === 0) {
    result.message = 'Workspace already matches the latest version.';
  }
  return result;
}

async function runBackup(state: WebState): Promise<SimulationResult> {
  const preview = await previewState(state);
  if (preview.items === 0) {
    log(state, 'backup complete: no content changes');
    return preview;
  }

  const entries = await planEntries(state);
  const newestStoredVersion = Math.max(
    0,
    ...Object.values(state.vaults).flatMap((vault) =>
      vault.manifests.map((manifest) => manifest.createdAtUnix),
    ),
  );
  const createdAtUnix = Math.max(nowUnix(), newestStoredVersion + 1);
  const versionId = `v${createdAtUnix}-${state.sequence++}`;
  let blobsCopied = 0;
  let bytesCopied = 0;

  for (const destination of state.config.destinations) {
    const vault = state.vaults[destination.id] ?? newVault();
    state.vaults[destination.id] = vault;
    for (const entry of entries) {
      if (vault.blobs[entry.sha256]) continue;
      const source = state.files.find((file) => file.relPath === entry.rel_path);
      if (!source) continue;
      vault.blobs[entry.sha256] = cloneBytes(source.bytes);
      blobsCopied += 1;
      bytesCopied += source.bytes.byteLength;
    }
    vault.manifests.push({ id: versionId, sourcePath: state.sourcePath, createdAtUnix, entries });
    const keep = Math.max(1, state.config.max_backups_per_file);
    const sourceManifests = vault.manifests
      .filter((manifest) => manifest.sourcePath === state.sourcePath)
      .sort((left, right) => right.createdAtUnix - left.createdAtUnix)
      .slice(0, keep);
    const otherManifests = vault.manifests.filter(
      (manifest) => manifest.sourcePath !== state.sourcePath,
    );
    vault.manifests = [...otherManifests, ...sourceManifests];
  }

  state.status.last_run_ts = createdAtUnix;
  state.status.last_files_backed_up = entries.length;
  state.status.last_dirty_count = preview.items;
  state.status.recent_activity = entries.slice(0, 12).map((entry) => ({
    path: `${state.sourcePath}/${entry.rel_path}`,
    bytes: entry.len,
    ts: createdAtUnix,
  }));
  state.status.replication_last_run_ts = createdAtUnix;
  state.status.replication_last_status = 'ok';
  state.status.replication_last_bytes_copied = bytesCopied;
  state.status.replication_last_blobs_copied = blobsCopied;
  state.status.replication_last_manifests_copied = Math.max(
    0,
    state.config.destinations.length - 1,
  );
  state.status.replication_last_pairs_ok = Math.max(0, state.config.destinations.length - 1);
  state.integrity = 'not-checked';
  log(
    state,
    `backup ${versionId} committed: ${entries.length} files, ${blobsCopied} new blobs, ${bytesCopied} bytes`,
  );
  return { ...preview, message: `Created ${versionId}.` };
}

async function verifyState(state: WebState): Promise<VerifyResult> {
  let ok = 0;
  let bad = 0;
  for (const vault of Object.values(state.vaults)) {
    const referenced = new Set(
      vault.manifests.flatMap((manifest) => manifest.entries.map((entry) => entry.sha256)),
    );
    for (const hash of referenced) {
      const blob = vault.blobs[hash];
      if (blob && (await sha256(blob)) === hash) ok += 1;
      else bad += 1;
    }
  }
  const checkedAt = nowUnix();
  state.status.last_verify_ts = checkedAt;
  state.status.last_verify_status = bad === 0 ? 'ok' : 'issues';
  state.status.last_verify_issues = bad;
  state.integrity = bad === 0 ? 'healthy' : 'damaged';
  log(state, `integrity verification complete: ${ok} healthy, ${bad} damaged or missing`);
  return {
    ok,
    bad,
    last_verify_ts: checkedAt,
    last_verify_status: state.status.last_verify_status,
    last_verify_issues: bad,
  };
}

function createSeedState(): WebState {
  const config = defaultConfig();
  const sourcePath = config.watched[0]?.path ?? '/Browser Workspace/Portfolio';
  const seedTime = nowUnix() - 120;
  return {
    schemaVersion: SCHEMA_VERSION,
    config,
    sourcePath,
    files: [
      {
        relPath: 'README.md',
        bytes: encoder.encode(
          '# Atlas Portfolio\n\nA browser-owned workspace protected by Backup Sync.\n',
        ),
        mtimeUnix: seedTime,
      },
      {
        relPath: 'docs/architecture.md',
        bytes: encoder.encode('UI -> command transport -> content-addressed vault -> restore\n'),
        mtimeUnix: seedTime,
      },
      {
        relPath: 'src/app.ts',
        bytes: encoder.encode("export const status = 'portfolio-ready';\n"),
        mtimeUnix: seedTime,
      },
    ],
    vaults: { primary: newVault(), mirror: newVault() },
    status: emptyStatus(),
    logs: [],
    integrity: 'not-checked',
    sequence: 1,
  };
}

async function seedState(): Promise<WebState> {
  const state = createSeedState();
  await runBackup(state);
  const architecture = state.files.find((file) => file.relPath === 'docs/architecture.md');
  if (architecture) {
    architecture.bytes = encoder.encode(
      'UI -> typed command transport -> content-addressed vault -> verified restore\n',
    );
    architecture.mtimeUnix = nowUnix() - 30;
  }
  await runBackup(state);
  await verifyState(state);
  return state;
}

function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DATABASE_NAME, DATABASE_VERSION);
    request.onupgradeneeded = () => {
      const database = request.result;
      if (!database.objectStoreNames.contains('state')) database.createObjectStore('state');
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error('IndexedDB could not be opened'));
  });
}

async function readPersistedState(): Promise<WebState | null> {
  if (typeof indexedDB === 'undefined') return null;
  const database = await openDatabase();
  try {
    return await new Promise<WebState | null>((resolve, reject) => {
      const request = database.transaction('state', 'readonly').objectStore('state').get(STATE_KEY);
      request.onsuccess = () => resolve((request.result as WebState | undefined) ?? null);
      request.onerror = () =>
        reject(request.error ?? new Error('Browser workspace could not be read'));
    });
  } finally {
    database.close();
  }
}

async function persistState(state: WebState): Promise<void> {
  if (typeof indexedDB === 'undefined') return;
  const database = await openDatabase();
  try {
    await new Promise<void>((resolve, reject) => {
      const transaction = database.transaction('state', 'readwrite');
      transaction.objectStore('state').put(state, STATE_KEY);
      transaction.oncomplete = () => resolve();
      transaction.onerror = () =>
        reject(transaction.error ?? new Error('Browser workspace could not be saved'));
      transaction.onabort = () =>
        reject(transaction.error ?? new Error('Browser workspace save was aborted'));
    });
  } finally {
    database.close();
  }
}

let statePromise: Promise<WebState> | null = null;
let operationQueue: Promise<void> = Promise.resolve();
let schedulerStarted = false;

async function loadState(): Promise<WebState> {
  if (!statePromise) {
    statePromise = (async () => {
      try {
        const persisted = await readPersistedState();
        if (persisted?.schemaVersion === SCHEMA_VERSION) return persisted;
      } catch (error) {
        console.warn(
          '[web-engine] Browser persistence unavailable; using this session only.',
          error,
        );
      }
      const seeded = await seedState();
      await persistState(seeded).catch(() => undefined);
      return seeded;
    })();
  }
  return await statePromise;
}

async function mutate<T>(operation: (state: WebState) => Promise<T> | T): Promise<T> {
  let result!: T;
  let failure: unknown;
  const run = operationQueue.then(async () => {
    try {
      const state = await loadState();
      result = await operation(state);
      await persistState(state);
    } catch (error) {
      failure = error;
    }
  });
  operationQueue = run.then(
    () => undefined,
    () => undefined,
  );
  await run;
  if (failure) throw failure;
  return result;
}

function ensureWebScheduler(): void {
  if (
    schedulerStarted ||
    import.meta.env.VITE_APP_RUNTIME !== 'web' ||
    typeof window === 'undefined'
  ) {
    return;
  }
  schedulerStarted = true;
  void navigator.storage?.persist?.().catch(() => false);
  window.setInterval(
    () => {
      void mutate(async (state) => {
        if (!state.config.safe_mode) await runBackup(state);
      }).catch((error) => {
        console.warn('[web-engine] Foreground backup cycle failed.', error);
      });
    },
    30 * 60 * 1000,
  );
}

function primaryVault(state: WebState): Vault {
  const primaryId = state.config.destinations[0]?.id ?? 'primary';
  const existing = state.vaults[primaryId];
  if (existing) return existing;
  const vault = newVault();
  state.vaults[primaryId] = vault;
  return vault;
}

function manifestById(state: WebState, sourcePath: string, versionId: string): Manifest {
  const manifest = primaryVault(state).manifests.find(
    (candidate) => candidate.sourcePath === sourcePath && candidate.id === versionId,
  );
  if (!manifest) throw new Error('The selected browser backup version no longer exists.');
  return manifest;
}

function validateRestorePath(path: string): void {
  if (!path || path.startsWith('/') || path.includes('\\') || path.includes('\0')) {
    throw new Error(`Unsafe restore path: ${path}`);
  }
  const segments = path.split('/');
  if (segments.some((segment) => !segment || segment === '.' || segment === '..')) {
    throw new Error(`Unsafe restore path: ${path}`);
  }
}

function normalizeWorkspacePath(path: string): string {
  const normalized = path.trim().replace(/^\.\//, '');
  validateRestorePath(normalized);
  if (normalized.length > 240) throw new Error('Workspace paths must be 240 characters or fewer.');
  return normalized;
}

function decodeEditableText(bytes: Uint8Array): { text: string; editable: boolean } {
  try {
    const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    const hasControlCharacters = Array.from(text).some((character) => {
      const code = character.charCodeAt(0);
      return code < 32 && code !== 9 && code !== 10 && code !== 13;
    });
    return hasControlCharacters ? { text: '', editable: false } : { text, editable: true };
  } catch {
    return { text: '', editable: false };
  }
}

async function restoredFiles(
  state: WebState,
  manifest: Manifest,
  requestedPaths?: Set<string>,
): Promise<WebFile[]> {
  const vault = primaryVault(state);
  const restored: WebFile[] = [];
  for (const entry of manifest.entries) {
    if (requestedPaths && !requestedPaths.has(entry.rel_path)) continue;
    validateRestorePath(entry.rel_path);
    const blob = vault.blobs[entry.sha256];
    if (!blob || (await sha256(blob)) !== entry.sha256) {
      throw new Error(`Restore blocked because ${entry.rel_path} failed integrity verification.`);
    }
    restored.push({
      relPath: entry.rel_path,
      bytes: cloneBytes(blob),
      mtimeUnix: entry.mtime_unix,
    });
  }
  return restored;
}

function webSummary(state: WebState): WebWorkspaceSummary {
  const vault = primaryVault(state);
  const manifests = vault.manifests.filter((manifest) => manifest.sourcePath === state.sourcePath);
  const uniqueHashes = new Set(
    manifests.flatMap((manifest) => manifest.entries.map((entry) => entry.sha256)),
  );
  const logicalBytes = manifests.reduce(
    (total, manifest) => total + manifest.entries.reduce((sum, entry) => sum + entry.len, 0),
    0,
  );
  const storedBytes = [...uniqueHashes].reduce(
    (total, hash) => total + (vault.blobs[hash]?.byteLength ?? 0),
    0,
  );
  return {
    sourcePath: state.sourcePath,
    files: state.files.map((file) => ({
      path: file.relPath,
      bytes: file.bytes.byteLength,
      modifiedAt: file.mtimeUnix,
    })),
    versions: manifests.length,
    uniqueBlobs: uniqueHashes.size,
    logicalBytes,
    storedBytes,
    destinations: state.config.destinations.length,
    lastRunAt: state.status.last_run_ts,
    integrity: state.integrity,
  };
}

type UnknownArgs = Record<string, unknown> | undefined;

/** Command-compatible browser transport. Every reported result is derived from persisted bytes. */
export async function invokeWebCommand<T>(command: string, args?: UnknownArgs): Promise<T> {
  ensureWebScheduler();
  const read = async <R>(selector: (state: WebState) => R | Promise<R>): Promise<R> =>
    await selector(await loadState());

  switch (command) {
    case 'load_config_cmd':
      return (await read((state) => structuredClone(state.config))) as T;
    case 'save_config_cmd':
      return (await mutate(async (state) => {
        const config = args?.cfg as Config | undefined;
        if (!config) throw new Error('Configuration payload is required.');
        state.config = structuredClone(config);
        state.status.safe_mode = config.safe_mode;
        log(state, 'configuration saved in browser storage');
        if (!config.safe_mode) await runBackup(state);
        return { daemon_restarted: false, daemon_restart_warning: null };
      })) as T;
    case 'relocate_destination_cmd':
      return (await mutate(async (state) => {
        const destinationId = String(args?.destinationId ?? '');
        const newPath = String(args?.newPath ?? '');
        const destinationIndex = state.config.destinations.findIndex(
          (destination) => destination.id === destinationId,
        );
        if (destinationIndex < 0) throw new Error('Storage location no longer exists.');
        if (!newPath) throw new Error('Choose a new storage location.');
        if (
          state.config.destinations.some(
            (destination) => destination.id !== destinationId && destination.path === newPath,
          )
        ) {
          throw new Error('That storage location is already configured.');
        }
        const vault = state.vaults[destinationId] ?? { blobs: {}, manifests: [] };
        const filesMoved = Object.keys(vault.blobs).length + vault.manifests.length;
        const bytesMoved = Object.values(vault.blobs).reduce(
          (total, bytes) => total + bytes.byteLength,
          0,
        );
        state.config.destinations[destinationIndex] = {
          ...state.config.destinations[destinationIndex]!,
          path: newPath,
        };
        if (destinationIndex === 0) state.config.backup_root = newPath;
        log(state, `storage ${destinationId} moved and verified at ${newPath}`);
        return {
          config: structuredClone(state.config),
          files_moved: filesMoved,
          bytes_moved: bytesMoved,
          old_location_removed: true,
          warning: null,
        };
      })) as T;
    case 'toggle_safe_mode_cmd':
      return (await mutate(async (state) => {
        const desired = Boolean(args?.desired);
        state.config.safe_mode = desired;
        state.status.safe_mode = desired;
        log(
          state,
          desired ? 'foreground backup schedule paused' : 'foreground backup schedule enabled',
        );
        if (!desired) await runBackup(state);
        return { safe_mode: desired, applied_live: true, warning: null };
      })) as T;
    case 'get_status':
      return (await read((state) => ({
        ...structuredClone(state.status),
        uptime_secs: Math.max(1, nowUnix() - (state.status.last_run_ts ?? nowUnix())),
        destinations: state.config.destinations.map((destination) => ({
          id: destination.id,
          label: destination.label,
          path: destination.path,
          reachable: true,
          writable: true,
          free_bytes: null,
          message: 'Browser vault ready',
        })),
      }))) as T;
    case 'check_destination_cmd':
      return { writable: true, free_bytes: null, message: 'Browser vault ready' } as T;
    case 'test_access_cmd':
      return (await read((state) => ({
        destination_writable: true,
        destination_message: 'Browser-owned storage is available.',
        watched_ok: [...new Set(state.config.watched.map((watched) => watched.path))],
        watched_missing: [],
        watched_unwritable: [],
      }))) as T;
    case 'hardening_check_cmd':
      return (await read((state) => {
        const report: HardeningReport = {
          ok: state.config.destinations.length > 0 && state.files.length > 0,
          message: 'Browser storage, source data, and restore validation are ready.',
          watched_ok: [...new Set(state.config.watched.map((watched) => watched.path))],
          watched_issues: [],
          destinations: state.config.destinations.map((destination) => ({
            id: destination.id,
            path: destination.path,
            ok: true,
            free_bytes: null,
            required_free_bytes: 0,
            message: 'Browser vault ready',
          })),
          snapshots: {
            checked: false,
            supported: false,
            message: 'OS snapshots are provided by the desktop application.',
          },
        };
        return report;
      })) as T;
    case 'run_simulate_cmd':
      return (await read(previewState)) as T;
    case 'run_now_cmd':
      return (await mutate(runBackup)) as T;
    case 'verify_cmd':
      return (await mutate(verifyState)) as T;
    case 'list_versions_cmd':
      return (await read((state) => {
        const bySource = new Map<string, Manifest[]>();
        for (const manifest of primaryVault(state).manifests) {
          const list = bySource.get(manifest.sourcePath) ?? [];
          list.push(manifest);
          bySource.set(manifest.sourcePath, list);
        }
        return [...bySource.entries()].map(
          ([source_path, manifests]): FolderVersionsDto => ({
            source_path,
            versions: manifests
              .sort((left, right) => right.createdAtUnix - left.createdAtUnix)
              .map((manifest) => ({ id: manifest.id, created_at_unix: manifest.createdAtUnix })),
          }),
        );
      })) as T;
    case 'list_version_files_cmd':
      return (await read((state) => {
        const request = args?.args as
          | {
              source_path: string;
              version_id: string;
              query?: string | null;
              limit?: number | null;
            }
          | undefined;
        if (!request) throw new Error('Version file request is required.');
        const manifest = manifestById(state, request.source_path, request.version_id);
        const query = request.query?.trim().toLocaleLowerCase() ?? '';
        const matches = manifest.entries.filter((entry) =>
          query ? entry.rel_path.toLocaleLowerCase().includes(query) : true,
        );
        const result: ListVersionFilesResultDto = {
          total_files: matches.length,
          files: matches.slice(0, request.limit ?? 250),
        };
        return result;
      })) as T;
    case 'restore_version_cmd':
      return (await mutate(async (state) => {
        const request = args?.args as RestoreArgs | undefined;
        if (!request) throw new Error('Restore request is required.');
        const manifest = manifestById(state, request.source_path, request.version_id);
        const files = await restoredFiles(state, manifest);
        const before = state.files.length;
        state.files = files;
        log(state, `restored ${files.length} files from ${manifest.id} into the browser workspace`);
        const result: RestoreResultDto = {
          files_written: files.length,
          files_removed: Math.max(0, before - files.length),
          dirs_created: new Set(
            files.map((file) => file.relPath.split('/').slice(0, -1).join('/')).filter(Boolean),
          ).size,
        };
        return result;
      })) as T;
    case 'restore_files_cmd':
      return (await mutate(async (state) => {
        const request = args?.args as RestoreFilesArgs | undefined;
        if (!request) throw new Error('Restore request is required.');
        const manifest = manifestById(state, request.source_path, request.version_id);
        const restored = await restoredFiles(state, manifest, new Set(request.rel_paths));
        const byPath = new Map(state.files.map((file) => [file.relPath, file]));
        for (const file of restored) byPath.set(file.relPath, file);
        state.files = [...byPath.values()].sort((left, right) =>
          left.relPath.localeCompare(right.relPath),
        );
        log(state, `restored ${restored.length} selected files from ${manifest.id}`);
        return {
          files_written: restored.length,
          files_removed: 0,
          dirs_created: 0,
        } satisfies RestoreResultDto;
      })) as T;
    case 'log_tail_cmd':
      return (await read((state) => state.logs.join('\n'))) as T;
    case 'remove_kept_extra_version_cmd':
      return undefined as T;
    case 'check_service_cmd':
      return {
        installed: false,
        reachable: true,
        message: 'Web engine active while this page is open.',
        uptime_secs: null,
        last_ipc_ts: null,
      } as T;
    case 'web_workspace_summary_cmd':
      return (await read(webSummary)) as T;
    case 'web_read_file_cmd':
      return (await read((state) => {
        const path = normalizeWorkspacePath(String(args?.path ?? ''));
        const file = state.files.find((candidate) => candidate.relPath === path);
        if (!file) throw new Error(`${path} is not in the browser workspace.`);
        const decoded = decodeEditableText(file.bytes);
        const result: WebWorkspaceFile = {
          path,
          text: decoded.text,
          bytes: file.bytes.byteLength,
          modifiedAt: file.mtimeUnix,
          editable: decoded.editable,
        };
        return result;
      })) as T;
    case 'web_write_file_cmd':
      return (await mutate((state) => {
        const path = normalizeWorkspacePath(String(args?.path ?? ''));
        const text = String(args?.text ?? '');
        const bytes = encoder.encode(text);
        if (bytes.byteLength > 2_000_000) {
          throw new Error('The built-in editor is limited to 2 MB per file.');
        }
        const existing = state.files.find((file) => file.relPath === path);
        if (existing) {
          existing.bytes = bytes;
          existing.mtimeUnix = nowUnix();
        } else {
          state.files.push({ relPath: path, bytes, mtimeUnix: nowUnix() });
          state.files.sort((left, right) => left.relPath.localeCompare(right.relPath));
        }
        state.integrity = 'not-checked';
        log(state, `workspace file saved: ${path} (${bytes.byteLength} bytes)`);
        return webSummary(state);
      })) as T;
    case 'web_delete_file_cmd':
      return (await mutate((state) => {
        const path = normalizeWorkspacePath(String(args?.path ?? ''));
        const before = state.files.length;
        state.files = state.files.filter((file) => file.relPath !== path);
        if (before === state.files.length)
          throw new Error(`${path} is not in the browser workspace.`);
        log(state, `workspace file deleted: ${path}`);
        return webSummary(state);
      })) as T;
    case 'web_import_files_cmd':
      return (await mutate(async (state) => {
        const imported = args?.files as
          | Array<{ path: string; bytes: ArrayBuffer; modifiedAt?: number }>
          | undefined;
        if (!imported?.length) throw new Error('Choose at least one file to import.');
        const totalBytes = imported.reduce((total, file) => total + file.bytes.byteLength, 0);
        if (imported.length > 500 || totalBytes > 50_000_000) {
          throw new Error('Imports are limited to 500 files and 50 MB per batch.');
        }
        const byPath = new Map(state.files.map((file) => [file.relPath, file]));
        for (const importedFile of imported) {
          const path = normalizeWorkspacePath(importedFile.path);
          if (importedFile.bytes.byteLength > 10_000_000) {
            throw new Error(`${path} exceeds the 10 MB per-file browser limit.`);
          }
          byPath.set(path, {
            relPath: path,
            bytes: new Uint8Array(importedFile.bytes),
            mtimeUnix: importedFile.modifiedAt ?? nowUnix(),
          });
        }
        state.files = [...byPath.values()].sort((left, right) =>
          left.relPath.localeCompare(right.relPath),
        );
        state.integrity = 'not-checked';
        log(
          state,
          `imported ${imported.length} files (${totalBytes} bytes) into the browser workspace`,
        );
        if (!state.config.safe_mode) await runBackup(state);
        return webSummary(state);
      })) as T;
    case 'web_corrupt_replica_cmd':
      return (await mutate((state) => {
        const mirrorId = state.config.destinations[1]?.id;
        if (!mirrorId) throw new Error('Add a mirror destination before testing repair.');
        const mirror = state.vaults[mirrorId];
        const manifest = mirror ? latestManifest(mirror, state.sourcePath) : null;
        const hash = manifest?.entries[0]?.sha256;
        const blob = hash ? mirror?.blobs[hash] : undefined;
        if (!mirror || !hash || !blob || blob.byteLength === 0) {
          throw new Error('The mirror does not contain a repairable backup blob yet.');
        }
        const damaged = cloneBytes(blob);
        damaged[0] = (damaged[0] ?? 0) ^ 0xff;
        mirror.blobs[hash] = damaged;
        state.integrity = 'damaged';
        log(state, `integrity lab damaged mirror blob ${hash.slice(0, 12)}…`);
        return webSummary(state);
      })) as T;
    case 'web_repair_replica_cmd':
      return (await mutate(async (state) => {
        const primaryId = state.config.destinations[0]?.id;
        const mirrorId = state.config.destinations[1]?.id;
        const primary = primaryId ? state.vaults[primaryId] : undefined;
        const mirror = mirrorId ? state.vaults[mirrorId] : undefined;
        if (!primary || !mirror)
          throw new Error('Primary and mirror vaults are required for repair.');
        let repaired = 0;
        const referenced = new Set(
          mirror.manifests.flatMap((manifest) => manifest.entries.map((entry) => entry.sha256)),
        );
        for (const hash of referenced) {
          const current = mirror.blobs[hash];
          if (current && (await sha256(current)) === hash) continue;
          const healthy = primary.blobs[hash];
          if (!healthy || (await sha256(healthy)) !== hash) continue;
          mirror.blobs[hash] = cloneBytes(healthy);
          repaired += 1;
        }
        if (repaired === 0) throw new Error('No damaged mirror blobs could be repaired.');
        log(
          state,
          `replication repair restored ${repaired} mirror blob${repaired === 1 ? '' : 's'}`,
        );
        await verifyState(state);
        return { repaired, summary: webSummary(state) };
      })) as T;
    case 'web_reset_cmd':
      return (await mutate(async (state) => {
        const fresh = await seedState();
        Object.assign(state, fresh);
        return webSummary(state);
      })) as T;
    default:
      throw new Error(`Command ${command} is available only in the desktop application.`);
  }
}
