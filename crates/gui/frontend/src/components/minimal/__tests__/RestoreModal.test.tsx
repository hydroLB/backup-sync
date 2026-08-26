import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { createSafetyBackup, listVersions, restoreVersion } from '../../../services/restore';
import { RestoreModal } from '../RestoreModal';

vi.mock('../../../services/restore', () => ({
  createSafetyBackup: vi.fn(),
  listVersions: vi.fn(),
  restoreVersion: vi.fn(),
}));

describe('RestoreModal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(listVersions).mockResolvedValue([
      {
        source_path: '/projects/portfolio',
        versions: [
          { id: 'newer', created_at_unix: 1_700_000_100 },
          { id: 'older', created_at_unix: 1_700_000_000 },
        ],
      },
    ]);
    vi.mocked(createSafetyBackup).mockResolvedValue();
    vi.mocked(restoreVersion).mockResolvedValue({
      files_written: 4,
      files_removed: 1,
      dirs_created: 2,
    });
  });

  it('uses the protected item → version → confirmation flow and preserves current state first', async () => {
    const onClose = vi.fn();
    const onEvent = vi.fn();
    render(<RestoreModal open onClose={onClose} onEvent={onEvent} />);

    fireEvent.click(
      await screen.findByRole('button', { name: /\/projects\/portfolio.*2 versions/i }),
    );
    fireEvent.click(screen.getByRole('button', { name: /1 version newer/i }));

    expect(screen.getByRole('heading', { name: 'Confirm recovery' })).toBeInTheDocument();
    expect(screen.getByText('This changes the live folder.')).toBeInTheDocument();
    expect(screen.getByLabelText(/Save the current state first/i)).toBeChecked();

    fireEvent.click(screen.getByRole('button', { name: 'Recall this version' }));

    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
    expect(createSafetyBackup).toHaveBeenCalledOnce();
    expect(restoreVersion).toHaveBeenCalledWith({
      source_path: '/projects/portfolio',
      version_id: 'older',
      mode: 'in_place',
      target_dir: null,
    });
    expect(vi.mocked(createSafetyBackup).mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(restoreVersion).mock.invocationCallOrder[0]!,
    );
  });

  it('lets the user skip the optional safety version', async () => {
    render(<RestoreModal open onClose={vi.fn()} onEvent={vi.fn()} />);

    fireEvent.click(
      await screen.findByRole('button', { name: /\/projects\/portfolio.*2 versions/i }),
    );
    fireEvent.click(screen.getByRole('button', { name: /Latest saved version/i }));
    fireEvent.click(screen.getByLabelText(/Save the current state first/i));
    fireEvent.click(screen.getByRole('button', { name: 'Recall this version' }));

    await waitFor(() => expect(restoreVersion).toHaveBeenCalledOnce());
    expect(createSafetyBackup).not.toHaveBeenCalled();
  });
});
