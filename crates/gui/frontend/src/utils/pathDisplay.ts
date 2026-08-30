/** Keep the useful tail visible without changing the real filesystem path. */
export function compactFilesystemPath(path: string, tailSegments = 2): string {
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(path)) return path;

  const segments = path.split(/[\\/]+/).filter(Boolean);
  const safeTailSegments = Math.max(1, Math.round(tailSegments));
  if (segments.length <= safeTailSegments + 1) return path;

  return `…/${segments.slice(-safeTailSegments).join('/')}`;
}
