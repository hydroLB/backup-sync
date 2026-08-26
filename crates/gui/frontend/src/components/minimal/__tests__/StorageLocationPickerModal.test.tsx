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

    fireEvent.click(screen.getByRole('button', { name: /demo-backup-choice-1/i }));
    expect(onChoose).toHaveBeenCalledWith('browser-vault://demo-backup-choice-1');
  });

  it('always offers the next numbered demo location', () => {
    render(
      <StorageLocationPickerModal
        open
        usedPaths={[
          'browser-vault://demo-backup-choice-1',
          'browser-vault://demo-backup-choice-2',
          'browser-vault://demo-backup-choice-7',
        ]}
        onCancel={vi.fn()}
        onChoose={vi.fn()}
      />,
    );

    expect(screen.getByRole('button', { name: /demo-backup-choice-8/i })).toBeEnabled();
    expect(screen.queryByText('Already in use')).not.toBeInTheDocument();
  });
});
