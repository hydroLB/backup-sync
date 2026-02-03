import { correlationId } from '../correlation';

/**
 * Purpose: Validate correlation id format and uniqueness for service calls.
 *
 * Inputs: None.
 * Outputs: Throws on assertion failures.
 * Ties to: `correlationId` from the services layer.
 * Side effects: None.
 * Why: Guard the correlation id format used for tracing actions.
 */
function assertCorrelationId(): void {
  try {
    const a = correlationId('run');
    const b = correlationId('run');
    expect(a).not.toEqual(b);
    expect(a.startsWith('run-')).toBe(true);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[correlation.test.ts::assertCorrelationId] ${reason}`);
  }
}

describe('correlationId', () => {
  it('produces unique-ish ids', assertCorrelationId);
});
