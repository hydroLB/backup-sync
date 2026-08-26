import { useEffect, useRef, type MutableRefObject } from 'react';
import { render, screen, act } from '@testing-library/react';
import { useMinimalFeedback } from '../useMinimalFeedback';
import { SavePulseScope } from '../useMinimalFeedback';

type EventKind = 'ok' | 'error' | 'info';

type Controller = {
  emit: (msg: string, kind?: EventKind, scope?: SavePulseScope) => void;
};

/** Keep badge and toast logic testable without relying on full screen renders. */
function Harness({
  onEvent,
  controllerRef,
}: {
  onEvent: (msg: string, kind?: EventKind) => void;
  controllerRef: MutableRefObject<Controller | null>;
}) {
  const { emitEvent, inline, showSavedBadge, savedBadgeNonce } = useMinimalFeedback({ onEvent });
  const emitRef = useRef(emitEvent);
  emitRef.current = emitEvent;

  useEffect(() => {
    controllerRef.current = {
      emit: (msg, kind = 'info', scope = 'none') => {
        emitRef.current(msg, kind, scope);
      },
    };
  }, [controllerRef]);

  return (
    <>
      {showSavedBadge ? <div key={savedBadgeNonce}>Saved</div> : null}
      {inline ? <div>{inline.msg}</div> : null}
    </>
  );
}

describe('useMinimalFeedback', () => {
  it('does not show Saved for background ok events', async () => {
    const controllerRef = { current: null as Controller | null };
    render(<Harness onEvent={() => undefined} controllerRef={controllerRef} />);

    await act(async () => {
      controllerRef.current?.emit('All destinations are reachable again.', 'ok', 'none');
      await new Promise((resolve) => setTimeout(resolve, 5));
    });

    expect(screen.queryByText('Saved')).not.toBeInTheDocument();
  });

  it('shows Saved for pause ok events', async () => {
    const controllerRef = { current: null as Controller | null };
    render(<Harness onEvent={() => undefined} controllerRef={controllerRef} />);

    await act(async () => {
      controllerRef.current?.emit('Paused.', 'ok', 'none');
      await new Promise((resolve) => setTimeout(resolve, 5));
    });

    expect(screen.getByText('Saved')).toBeInTheDocument();
  });

  it('restarts Saved when another success arrives', async () => {
    const controllerRef = { current: null as Controller | null };
    render(<Harness onEvent={() => undefined} controllerRef={controllerRef} />);

    await act(async () => {
      controllerRef.current?.emit('Paused.', 'ok', 'none');
      await new Promise((resolve) => setTimeout(resolve, 5));
    });

    const first = screen.getByText('Saved');

    await act(async () => {
      controllerRef.current?.emit('Saved.', 'ok', 'folders');
      await new Promise((resolve) => setTimeout(resolve, 5));
    });

    const second = screen.getByText('Saved');
    expect(second).not.toBe(first);
  });

  it('keeps scoped success out of document flow while showing Saved', async () => {
    const controllerRef = { current: null as Controller | null };
    render(<Harness onEvent={() => undefined} controllerRef={controllerRef} />);

    await act(async () => {
      controllerRef.current?.emit('Primary destination set to /tmp/backups.', 'ok', 'destination');
      await new Promise((resolve) => setTimeout(resolve, 5));
    });

    expect(screen.getByText('Saved')).toBeInTheDocument();
    expect(screen.queryByText('Primary destination set to /tmp/backups.')).not.toBeInTheDocument();
  });
});
