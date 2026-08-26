import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { listVersions, restoreVersion } from '../../../services/restore';
import { RestoreModal } from '../RestoreModal';

vi.mock('../../../services/restore', () => ({
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
    vi.mocked(restoreVersion).mockResolvedValue({
      files_written: 4,
      files_removed: 1,
      dirs_created: 2,
      newer_versions_removed: 1,
    });
  });

  it('uses the protected item → version → confirmation flow and deletes newer history', async () => {
    const onClose = vi.fn();
    const onEvent = vi.fn();
    render(<RestoreModal open onClose={onClose} onEvent={onEvent} />);

    fireEvent.click(
      await screen.findByRole('button', { name: /\/projects\/portfolio.*2 versions/i }),
    );
    fireEvent.click(screen.getByRole('button', { name: /1 version newer/i }));

    expect(screen.getByRole('heading', { name: 'Confirm recovery' })).toBeInTheDocument();
    expect(screen.getByText('This changes the live folder.')).toBeInTheDocument();
    expect(screen.getByLabelText(/Keep the 1 newer saved version/i)).not.toBeChecked();

    fireEvent.click(screen.getByRole('button', { name: 'Recall this version' }));

    expect(await screen.findByRole('heading', { name: 'Recovery complete' })).toBeInTheDocument();
    expect(screen.getByText('1 newer version deleted')).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
    expect(restoreVersion).toHaveBeenCalledWith({
      source_path: '/projects/portfolio',
      version_id: 'older',
      mode: 'in_place',
      target_dir: null,
      keep_newer_versions: false,
    });

    fireEvent.click(screen.getByRole('button', { name: 'Done' }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('lets the user keep newer recovery points', async () => {
    render(<RestoreModal open onClose={vi.fn()} onEvent={vi.fn()} />);

    fireEvent.click(
      await screen.findByRole('button', { name: /\/projects\/portfolio.*2 versions/i }),
    );
    fireEvent.click(screen.getByRole('button', { name: /1 version newer/i }));
    fireEvent.click(screen.getByLabelText(/Keep the 1 newer saved version/i));
    fireEvent.click(screen.getByRole('button', { name: 'Recall this version' }));

    await waitFor(() => expect(restoreVersion).toHaveBeenCalledOnce());
    expect(restoreVersion).toHaveBeenCalledWith(
      expect.objectContaining({ keep_newer_versions: true }),
    );
  });
});
