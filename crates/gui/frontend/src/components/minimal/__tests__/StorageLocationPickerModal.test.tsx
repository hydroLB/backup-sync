import { fireEvent, render, screen } from '@testing-library/react';
import { StorageLocationPickerModal } from '../StorageLocationPickerModal';

describe('StorageLocationPickerModal', () => {
  it('requires an explicit storage choice', () => {
    const onChoose = vi.fn();
    render(
      <StorageLocationPickerModal open usedPaths={[]} onCancel={vi.fn()} onChoose={onChoose} />,
    );

    expect(screen.getByRole('dialog', { name: 'Choose backup location' })).toBeInTheDocument();
    expect(onChoose).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: /External drive/i }));
    expect(onChoose).toHaveBeenCalledWith('browser-vault://external-drive');
  });

  it('disables locations that are already configured', () => {
    render(
      <StorageLocationPickerModal
        open
        usedPaths={['browser-vault://network-storage']}
        onCancel={vi.fn()}
        onChoose={vi.fn()}
      />,
    );

    expect(screen.getByRole('button', { name: /Network storage/i })).toBeDisabled();
    expect(screen.getByText('Already in use')).toBeInTheDocument();
  });
});
