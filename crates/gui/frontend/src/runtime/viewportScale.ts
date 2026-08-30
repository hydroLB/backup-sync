export const GOLDEN_VIEWPORT_WIDTH = 1087;
export const GOLDEN_VIEWPORT_HEIGHT = 924;
export const GOLDEN_BASE_ZOOM = 1.25;
export const MIN_GOLDEN_SCALE = 1 / GOLDEN_BASE_ZOOM;
export const MAX_GOLDEN_SCALE = 1.35;
export const COMPACT_VIEWPORT_WIDTH = 720;

/** Preserve the showcase proportions while allowing exceptionally large windows to plateau. */
export function goldenViewportScale(width: number, height: number): number {
  if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
    return 1;
  }
  return Math.max(
    MIN_GOLDEN_SCALE,
    Math.min(width / GOLDEN_VIEWPORT_WIDTH, height / GOLDEN_VIEWPORT_HEIGHT, MAX_GOLDEN_SCALE),
  );
}

/** Keep every desktop visual dimension on one shared scale instead of resizing controls separately. */
export function applyGoldenViewportScale(root: HTMLElement, width: number, height: number): void {
  if (width <= COMPACT_VIEWPORT_WIDTH) {
    root.dataset.viewportMode = 'compact';
    root.style.removeProperty('--golden-ui-zoom');
    root.style.removeProperty('--golden-canvas-width');
    root.style.removeProperty('--golden-canvas-height');
    return;
  }

  const scale = goldenViewportScale(width, height);
  const zoom = GOLDEN_BASE_ZOOM * scale;
  root.dataset.viewportMode = 'scaled';
  root.style.setProperty('--golden-ui-zoom', String(zoom));
  root.style.setProperty('--golden-canvas-width', `${width / zoom}px`);
  root.style.setProperty('--golden-canvas-height', `${height / zoom}px`);
}

/** Recompute the single canvas scale whenever the native or browser window changes size. */
export function startGoldenViewportScaling(root: HTMLElement): () => void {
  const apply = () => applyGoldenViewportScale(root, window.innerWidth, window.innerHeight);
  apply();
  window.addEventListener('resize', apply, { passive: true });
  window.visualViewport?.addEventListener('resize', apply, { passive: true });

  return () => {
    window.removeEventListener('resize', apply);
    window.visualViewport?.removeEventListener('resize', apply);
  };
}
