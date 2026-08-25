import { fireEvent, render, screen } from '@testing-library/react';
import { AppErrorBoundary } from '../AppErrorBoundary';

function suppressExpectedRenderError(event: ErrorEvent): void {
  event.preventDefault();
}

describe('AppErrorBoundary', () => {
  beforeEach(() => {
    window.addEventListener('error', suppressExpectedRenderError);
  });

  afterEach(() => {
    window.removeEventListener('error', suppressExpectedRenderError);
    vi.restoreAllMocks();
  });

  it('contains a render error and remounts the child tree when retried', () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    let shouldThrow = true;
    let mounts = 0;

    function UnstableScreen() {
      mounts += 1;
      if (shouldThrow) {
        throw new Error('/private/source/path: sensitive diagnostic');
      }
      return <p>Backup screen restored</p>;
    }

    render(
      <AppErrorBoundary>
        <UnstableScreen />
      </AppErrorBoundary>,
    );

    expect(screen.getByRole('alert')).toHaveAccessibleName('Backup Sync needs a moment');
    expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument();
    expect(screen.queryByText(/private\/source\/path/i)).not.toBeInTheDocument();

    const attemptsBeforeRetry = mounts;
    shouldThrow = false;
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));

    expect(screen.getByText('Backup screen restored')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(mounts).toBeGreaterThan(attemptsBeforeRetry);
  });

  it('offers reload only when a safe reload callback is provided', () => {
    const onReload = vi.fn();

    function BrokenScreen(): never {
      throw new Error('broken');
    }

    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    render(
      <AppErrorBoundary onReload={onReload}>
        <BrokenScreen />
      </AppErrorBoundary>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Reload app' }));
    expect(onReload).toHaveBeenCalledOnce();
  });
});
