import {
  applyGoldenViewportScale,
  GOLDEN_BASE_ZOOM,
  goldenViewportScale,
  MAX_GOLDEN_SCALE,
  MIN_GOLDEN_SCALE,
} from '../viewportScale';

describe('golden viewport scaling', () => {
  it('treats the showcase viewport as scale 1', () => {
    expect(goldenViewportScale(1087, 924)).toBe(1);
  });

  it('uses the limiting dimension and caps excessive enlargement', () => {
    expect(goldenViewportScale(2174, 924)).toBe(1);
    expect(goldenViewportScale(1087, 462)).toBe(MIN_GOLDEN_SCALE);
    expect(goldenViewportScale(4000, 4000)).toBe(MAX_GOLDEN_SCALE);
  });

  it('applies one shared zoom and preserves the compact phone layout', () => {
    const root = document.createElement('div');
    applyGoldenViewportScale(root, 1087, 924);
    expect(root.dataset.viewportMode).toBe('scaled');
    expect(root.style.getPropertyValue('--golden-ui-zoom')).toBe(String(GOLDEN_BASE_ZOOM));
    expect(root.style.getPropertyValue('--golden-canvas-width')).toBe('869.6px');
    expect(root.style.getPropertyValue('--golden-canvas-height')).toBe('739.2px');

    applyGoldenViewportScale(root, 600, 1200);
    expect(root.dataset.viewportMode).toBe('compact');
    expect(root.style.getPropertyValue('--golden-ui-zoom')).toBe('');
    expect(root.style.getPropertyValue('--golden-canvas-width')).toBe('');
    expect(root.style.getPropertyValue('--golden-canvas-height')).toBe('');

    applyGoldenViewportScale(root, 721, 1200);
    expect(root.dataset.viewportMode).toBe('scaled');
  });
});
