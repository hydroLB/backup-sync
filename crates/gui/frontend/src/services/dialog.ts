import { wrapError } from './ipc';
import { safeInvoke } from './ipc';
import { IS_WEB_RUNTIME } from '../runtime/mode';

type DialogModule = typeof import('@tauri-apps/plugin-dialog');
type OpenOptions = import('@tauri-apps/plugin-dialog').OpenDialogOptions;
type OpenResult = Awaited<ReturnType<DialogModule['open']>>;

let dialogModulePromise: Promise<DialogModule> | null = null;

/** Avoid repeated module resolution and reduce click-to-dialog latency. */
export async function loadDialogModule(): Promise<DialogModule> {
  try {
    if (!dialogModulePromise) {
      dialogModulePromise = import('@tauri-apps/plugin-dialog');
    }
    return await dialogModulePromise;
  } catch (error) {
    dialogModulePromise = null;
    throw wrapError('[loadDialogModule] Failed to load dialog module', error);
  }
}

/** Centralize dialog calls so we can prewarm/cache for performance. */
export async function openDialog(options: OpenOptions): Promise<OpenResult> {
  try {
    if (IS_WEB_RUNTIME) {
      const title = options.title?.toLocaleLowerCase() ?? '';
      if (title.includes('restore destination')) {
        return '/Browser Workspace/Restored' as OpenResult;
      }
      if (title.includes('destination')) {
        return null as OpenResult;
      }

      const selection = await new Promise<File[]>((resolve) => {
        const input = document.createElement('input');
        let settled = false;
        const finish = (files: File[]) => {
          if (settled) return;
          settled = true;
          window.removeEventListener('focus', handleFocus);
          resolve(files);
        };
        const handleFocus = () => {
          window.setTimeout(() => finish(Array.from(input.files ?? [])), 250);
        };
        input.type = 'file';
        input.multiple = Boolean(options.multiple) || Boolean(options.directory);
        if (options.directory) {
          input.setAttribute('webkitdirectory', '');
          input.setAttribute('directory', '');
        }
        input.addEventListener('change', () => finish(Array.from(input.files ?? [])), {
          once: true,
        });
        window.addEventListener('focus', handleFocus, { once: true });
        input.click();
      });
      if (selection.length === 0) return null as OpenResult;
      const files = await Promise.all(
        selection.map(async (file) => {
          const browserFile = file as File & { webkitRelativePath?: string };
          const candidate = browserFile.webkitRelativePath || file.name;
          const segments = candidate.split('/').filter(Boolean);
          const path = segments.length > 1 ? segments.slice(1).join('/') : segments[0] ?? file.name;
          return {
            path,
            bytes: await file.arrayBuffer(),
            modifiedAt: Math.floor(file.lastModified / 1000),
          };
        }),
      );
      await safeInvoke('web_import_files_cmd', { files });
      return '/Browser Workspace/Portfolio' as OpenResult;
    }
    const mod = await loadDialogModule();
    return await mod.open(options);
  } catch (error) {
    throw wrapError('[openDialog] Failed to open native picker dialog', error);
  }
}

/** Move module initialization cost off the critical click path. */
export async function prewarmDialog(): Promise<void> {
  try {
    if (IS_WEB_RUNTIME) return;
    await loadDialogModule();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    console.warn(`[prewarmDialog] Best-effort prewarm failed: ${reason}`);
  }
}
