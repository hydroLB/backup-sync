import { render, screen } from '@testing-library/react';
import { MinimalHeader } from '../MinimalHeader';

describe('MinimalHeader', () => {
  it('renders the full-app download only when the website supplies a release URL', () => {
    const { rerender } = render(
      <MinimalHeader
        liveSafeMode={false}
        busy={false}
        runningBusy={false}
        onRunningChange={vi.fn()}
      />,
    );
    expect(
      screen.queryByRole('link', { name: 'Download the full Backup Sync desktop app' }),
    ).not.toBeInTheDocument();

    rerender(
      <MinimalHeader
        liveSafeMode={false}
        busy={false}
        runningBusy={false}
        onRunningChange={vi.fn()}
        downloadUrl="https://example.test/Backup-Sync.dmg"
      />,
    );
    expect(
      screen.getByRole('link', { name: 'Download the full Backup Sync desktop app' }),
    ).toHaveAttribute('href', 'https://example.test/Backup-Sync.dmg');
  });
});
