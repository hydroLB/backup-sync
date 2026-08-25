import { act, render, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { listVersionFiles } from '../../../../../services/restore';
import { useRestoreSearch } from '../useRestoreSearch';

vi.mock('../../../../../services/restore', () => ({
  listVersionFiles: vi.fn(),
}));

type SearchState = ReturnType<typeof useRestoreSearch>;

function Harness({
  sourcePath,
  versionId,
  controller,
}: {
  sourcePath: string;
  versionId: string;
  controller: { current: SearchState | null };
}) {
  controller.current = useRestoreSearch({
    isOpen: true,
    scope: 'files',
    sourcePath,
    versionId,
    onEvent: vi.fn(),
  });
  return null;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((fulfill) => {
    resolve = fulfill;
  });
  return { promise, resolve };
}

const firstFile = {
  rel_path: 'first.txt',
  len: 1,
  mtime_unix: 1,
  mtime_nanos: 0,
  sha256: 'first',
};
const secondFile = {
  rel_path: 'second.txt',
  len: 2,
  mtime_unix: 2,
  mtime_nanos: 0,
  sha256: 'second',
};

describe('useRestoreSearch', () => {
  beforeEach(() => {
    vi.mocked(listVersionFiles).mockReset();
  });

  it('waits for an explicit search instead of querying while typing', async () => {
    const controller = { current: null as SearchState | null };
    render(<Harness sourcePath="/source" versionId="v1" controller={controller} />);

    act(() => {
      controller.current?.setQuery('notes');
    });

    await waitFor(() => {
      expect(controller.current?.query).toBe('notes');
    });
    expect(listVersionFiles).not.toHaveBeenCalled();
  });

  it('clears results and selection when the source or version changes', async () => {
    const controller = { current: null as SearchState | null };
    vi.mocked(listVersionFiles).mockResolvedValue({ files: [firstFile], total_files: 1 });
    const view = render(
      <Harness sourcePath="/source-one" versionId="v1" controller={controller} />,
    );

    await act(async () => {
      await controller.current?.runSearch();
    });
    act(() => {
      controller.current?.setSelectedValue('first.txt', true);
    });
    expect(controller.current?.selectedCount).toBe(1);

    view.rerender(<Harness sourcePath="/source-two" versionId="v2" controller={controller} />);

    await waitFor(() => {
      expect(controller.current?.fileResults).toBeNull();
      expect(controller.current?.selectedCount).toBe(0);
    });
  });

  it('ignores a stale response after a newer search completes', async () => {
    const controller = { current: null as SearchState | null };
    const oldResponse = deferred<{ files: (typeof firstFile)[]; total_files: number }>();
    const newResponse = deferred<{ files: (typeof secondFile)[]; total_files: number }>();
    vi.mocked(listVersionFiles)
      .mockReturnValueOnce(oldResponse.promise)
      .mockReturnValueOnce(newResponse.promise);
    render(<Harness sourcePath="/source" versionId="v1" controller={controller} />);

    let oldSearch!: Promise<void>;
    act(() => {
      controller.current?.setQuery('first');
    });
    await waitFor(() => expect(controller.current?.query).toBe('first'));
    act(() => {
      oldSearch = controller.current!.runSearch();
    });

    act(() => {
      controller.current?.setQuery('second');
    });
    await waitFor(() => expect(controller.current?.query).toBe('second'));
    let newSearch!: Promise<void>;
    act(() => {
      newSearch = controller.current!.runSearch();
    });

    await act(async () => {
      newResponse.resolve({ files: [secondFile], total_files: 1 });
      await newSearch;
    });
    await act(async () => {
      oldResponse.resolve({ files: [firstFile], total_files: 1 });
      await oldSearch;
    });

    expect(controller.current?.fileResults).toEqual([secondFile]);
  });
});
