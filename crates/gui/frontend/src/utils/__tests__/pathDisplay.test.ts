import { compactFilesystemPath } from '../pathDisplay';

describe('compactFilesystemPath', () => {
  it('keeps the useful tail of a long filesystem path', () => {
    expect(compactFilesystemPath('/workspace/Documents/Codex/2026-08-27/backup-sync')).toBe(
      '…/2026-08-27/backup-sync',
    );
    expect(compactFilesystemPath('/workspace/Documents/Codex/2026-08-27/backup-sync', 1)).toBe(
      '…/backup-sync',
    );
  });

  it('keeps short paths and browser vault identifiers intact', () => {
    expect(compactFilesystemPath('/Browser Workspace/Portfolio')).toBe(
      '/Browser Workspace/Portfolio',
    );
    expect(compactFilesystemPath('browser-vault://demo-backup-choice-1')).toBe(
      'browser-vault://demo-backup-choice-1',
    );
  });

  it('handles Windows separators while preserving the useful tail', () => {
    expect(compactFilesystemPath('C:\\Users\\liam\\Documents\\Work\\Project')).toBe(
      '…/Work/Project',
    );
  });
});
