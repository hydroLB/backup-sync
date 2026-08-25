import { ComponentProps } from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { RestoreModalBody } from '../RestoreModalBody';

type Props = ComponentProps<typeof RestoreModalBody>;

const folders: NonNullable<Props['folders']> = [
  {
    source_path: '/source-one',
    versions: [{ id: 'v1', created_at_unix: 1_700_000_000 }],
  },
  {
    source_path: '/source-two',
    versions: [
      { id: 'v2', created_at_unix: 1_700_000_100 },
      { id: 'v3', created_at_unix: 1_700_000_200 },
    ],
  },
];

function props(overrides: Partial<Props> = {}): Props {
  return {
    busy: false,
    folders,
    loading: false,
    loadError: null,
    hasFolders: true,
    singleVersionOnly: false,
    sourcePath: '/source-one',
    setSourcePath: vi.fn(),
    versionId: 'v1',
    setVersionId: vi.fn(),
    selectedVersions: folders[0]!.versions,
    scope: 'folder',
    setScope: vi.fn(),
    mode: 'to_directory',
    setMode: vi.fn(),
    targetDir: '',
    pickTargetDir: vi.fn(),
    query: '',
    setQuery: vi.fn(),
    fileResults: null,
    fileTotal: null,
    selectedCount: 0,
    fileLoading: false,
    setSelectedValue: vi.fn(),
    runSearch: vi.fn(),
    selected: {},
    onRetry: vi.fn(),
    ...overrides,
  };
}

describe('RestoreModalBody', () => {
  it('renders loading, failure with retry, and empty catalog states', () => {
    const initial = props({ loading: true, hasFolders: false, folders: null });
    const view = render(<RestoreModalBody {...initial} />);
    expect(screen.getByText('Loading versions')).toBeInTheDocument();

    const retry = vi.fn();
    view.rerender(
      <RestoreModalBody
        {...props({
          loading: false,
          loadError: 'daemon offline',
          hasFolders: false,
          folders: null,
          onRetry: retry,
        })}
      />,
    );
    expect(screen.getByText('daemon offline')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(retry).toHaveBeenCalledOnce();

    view.rerender(
      <RestoreModalBody
        {...props({ loading: false, loadError: null, hasFolders: false, folders: [] })}
      />,
    );
    expect(screen.getByText('No restore points available')).toBeInTheDocument();
  });

  it('renders catalog choices and dispatches folder, version, scope, mode, and target actions', () => {
    const setSourcePath = vi.fn();
    const setVersionId = vi.fn();
    const setScope = vi.fn();
    const setMode = vi.fn();
    const pickTargetDir = vi.fn();
    render(
      <RestoreModalBody
        {...props({
          singleVersionOnly: true,
          setSourcePath,
          setVersionId,
          setScope,
          setMode,
          pickTargetDir,
        })}
      />,
    );

    expect(screen.getByText('One saved version available')).toBeInTheDocument();
    expect(screen.getByRole('option', { name: '/source-one (1 version)' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: '/source-two (2 versions)' })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Folder'), { target: { value: '/source-two' } });
    fireEvent.change(screen.getByLabelText('Version'), { target: { value: 'v1' } });
    fireEvent.click(screen.getByText('Individual files').closest('button')!);
    fireEvent.click(screen.getByText('In place').closest('button')!);
    fireEvent.click(screen.getByRole('button', { name: 'Choose…' }));

    expect(setSourcePath).toHaveBeenCalledWith('/source-two');
    expect(setVersionId).toHaveBeenCalledWith('v1');
    expect(setScope).toHaveBeenCalledWith('files');
    expect(setMode).toHaveBeenCalledWith('in_place');
    expect(pickTargetDir).toHaveBeenCalledOnce();
    expect(screen.getByText('Choose a restore destination folder')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Entire folder/ })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(screen.getByRole('button', { name: /Individual files/ })).toHaveAttribute(
      'aria-pressed',
      'false',
    );
    expect(screen.getByRole('button', { name: /Choose location/ })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(screen.getByRole('button', { name: /In place/ })).toHaveAttribute(
      'aria-pressed',
      'false',
    );
  });

  it('renders file search loading, empty, and selectable result states', () => {
    const runSearch = vi.fn();
    const setQuery = vi.fn();
    const setSelectedValue = vi.fn();
    const view = render(
      <RestoreModalBody
        {...props({ scope: 'files', mode: 'in_place', fileLoading: true, runSearch })}
      />,
    );
    expect(screen.getByText('Searching files')).toBeInTheDocument();

    view.rerender(
      <RestoreModalBody
        {...props({
          scope: 'files',
          mode: 'in_place',
          fileLoading: false,
          fileResults: [],
          runSearch,
        })}
      />,
    );
    expect(screen.getByText('No matching files')).toBeInTheDocument();
    expect(screen.getByText('Results: 0')).toBeInTheDocument();

    const fileResults = [
      { rel_path: 'notes/todo.txt', len: 12, mtime_unix: 1, mtime_nanos: 0, sha256: 'a' },
      { rel_path: 'photos/image.jpg', len: 24, mtime_unix: 2, mtime_nanos: 0, sha256: 'b' },
    ];
    view.rerender(
      <RestoreModalBody
        {...props({
          scope: 'files',
          mode: 'in_place',
          query: 'notes',
          fileResults,
          fileTotal: 8,
          selectedCount: 1,
          selected: { 'notes/todo.txt': true },
          setQuery,
          setSelectedValue,
          runSearch,
        })}
      />,
    );

    expect(screen.getByText('Results: 2 (total_files=8)')).toBeInTheDocument();
    expect(screen.getByText('Selected: 1')).toBeInTheDocument();
    const search = screen.getByLabelText('Search');
    fireEvent.change(search, { target: { value: 'photos' } });
    fireEvent.keyDown(search, { key: 'ArrowDown' });
    fireEvent.keyDown(search, { key: 'Enter' });
    fireEvent.click(screen.getByRole('button', { name: 'Search' }));
    fireEvent.click(screen.getByRole('checkbox', { name: /photos\/image.jpg/ }));

    expect(setQuery).toHaveBeenCalledWith('photos');
    expect(runSearch).toHaveBeenCalledTimes(2);
    expect(setSelectedValue).toHaveBeenCalledWith('photos/image.jpg', true);
    expect(screen.getByRole('checkbox', { name: /notes\/todo.txt/ })).toBeChecked();
  });

  it('disables catalog controls while a restore is busy', () => {
    render(<RestoreModalBody {...props({ busy: true, targetDir: '/restore-here' })} />);

    expect(screen.getByLabelText('Folder')).toBeDisabled();
    expect(screen.getByLabelText('Version')).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Choose…' })).toBeDisabled();
    expect(screen.getByText('/restore-here').closest('.pill')).toHaveAttribute(
      'title',
      '/restore-here',
    );
  });
});
