import { beforeEach, describe, expect, it } from 'vitest';
import { invokeWebCommand, WebWorkspaceFile, WebWorkspaceSummary } from '../engine';
import { FolderVersionsDto, VerifyResult } from '../../../services/types';

describe('browser backup engine', () => {
  beforeEach(async () => {
    await invokeWebCommand('web_reset_cmd');
  });

  it('derives versions and deduplication metrics from real stored bytes', async () => {
    const summary = await invokeWebCommand<WebWorkspaceSummary>('web_workspace_summary_cmd');

    expect(summary.files).toHaveLength(3);
    expect(summary.versions).toBe(2);
    expect(summary.uniqueBlobs).toBeGreaterThan(0);
    expect(summary.logicalBytes).toBeGreaterThan(summary.storedBytes);
    expect(summary.integrity).toBe('healthy');
  });

  it('backs up edits, verifies hashes, and repairs a damaged mirror', async () => {
    await invokeWebCommand('web_write_file_cmd', {
      path: 'notes/recruiter.md',
      text: 'This content is hashed and versioned by the browser engine.\n',
    });
    const preview = await invokeWebCommand<{ items: number }>('run_simulate_cmd');
    expect(preview.items).toBe(1);

    await invokeWebCommand('run_now_cmd');
    const afterBackup = await invokeWebCommand<WebWorkspaceSummary>('web_workspace_summary_cmd');
    expect(afterBackup.versions).toBe(3);

    const healthy = await invokeWebCommand<VerifyResult>('verify_cmd');
    expect(healthy.bad).toBe(0);

    await invokeWebCommand('web_corrupt_replica_cmd');
    const damaged = await invokeWebCommand<VerifyResult>('verify_cmd');
    expect(damaged.bad).toBeGreaterThan(0);

    await invokeWebCommand('web_repair_replica_cmd');
    const repaired = await invokeWebCommand<VerifyResult>('verify_cmd');
    expect(repaired.bad).toBe(0);
  });

  it('restores an earlier manifest after validating every blob', async () => {
    const catalog = await invokeWebCommand<FolderVersionsDto[]>('list_versions_cmd');
    const folder = catalog[0];
    expect(folder?.versions).toHaveLength(2);
    const oldest = folder?.versions.at(-1);
    expect(folder && oldest).toBeTruthy();

    await invokeWebCommand('restore_version_cmd', {
      args: {
        source_path: folder!.source_path,
        version_id: oldest!.id,
        mode: 'in_place',
      },
    });

    const restored = await invokeWebCommand<WebWorkspaceFile>('web_read_file_cmd', {
      path: 'docs/architecture.md',
    });
    expect(restored.text).toContain('content-addressed vault -> restore');
    expect(restored.text).not.toContain('verified restore');
  });

  it('rejects traversal paths at the browser storage boundary', async () => {
    await expect(
      invokeWebCommand('web_write_file_cmd', { path: '../escape.txt', text: 'blocked' }),
    ).rejects.toThrow('Unsafe restore path');
  });
});
