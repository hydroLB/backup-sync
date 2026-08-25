import { correlationId } from '../correlation';

/** Guard the correlation id format used for tracing actions. */
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
