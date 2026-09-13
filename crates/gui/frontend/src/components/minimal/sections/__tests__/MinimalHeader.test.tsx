import { render, screen } from '@testing-library/react';
import { MinimalHeader } from '../MinimalHeader';

describe('MinimalHeader', () => {
  it('renders desktop setup only when the website supplies a project URL', () => {
    const { rerender } = render(
      <MinimalHeader
        liveSafeMode={false}
        busy={false}
        runningBusy={false}
        onRunningChange={vi.fn()}
      />,
    );
    expect(
      screen.queryByRole('link', { name: 'View Backup Sync desktop setup instructions' }),
    ).not.toBeInTheDocument();

    rerender(
      <MinimalHeader
        liveSafeMode={false}
        busy={false}
        runningBusy={false}
        onRunningChange={vi.fn()}
        desktopUrl="https://example.test/backup-sync#quick-start"
      />,
    );
    expect(
      screen.getByRole('link', { name: 'View Backup Sync desktop setup instructions' }),
    ).toHaveAttribute('href', 'https://example.test/backup-sync#quick-start');
  });
});
