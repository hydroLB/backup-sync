import { statusTextClass } from '../../theme/statusText';

/**
 * Summary: Verify semantic status-class mapping for common status strings.
 *
 * Inputs: None.
 *
 * Outputs: Assertions for each Ready/Working/Success/Error bucket.
 *
 * Side effects: None.
 *
 * Error handling: None.
 *
 * Ties to other methods: Covers shared status mapping used across UI surfaces.
 *
 * Why this exists: Prevent regressions in status color semantics.
 */
describe('statusTextClass', () => {
  it('maps ready-like text', () => {
    expect(statusTextClass('Ready')).toBe('status-text status-text-ready');
  });

  it('maps working-like text', () => {
    expect(statusTextClass('Saving settings...')).toBe('status-text status-text-working');
  });

  it('maps success-like text', () => {
    expect(statusTextClass('Saved.')).toBe('status-text status-text-success');
  });

  it('maps error-like text', () => {
    expect(statusTextClass('Failed to save config')).toBe('status-text status-text-error');
  });

  it('uses default styling for unknown text', () => {
    expect(statusTextClass('Custom message')).toBe('status-text');
  });
});
