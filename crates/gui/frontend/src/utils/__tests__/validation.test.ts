import { validateIgnorePatterns } from '../validation';

/**
 * Summary: Verify empty ignore patterns are accepted.
 *
 * Inputs: None.
 *
 * Outputs: Asserts null validation response.
 *
 * Side effects: None.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: `validateIgnorePatterns` empty handling.
 *
 * Why this exists: Allow operators to skip ignore patterns.
 */
function assertEmptyPatternsAllowed(): void {
  try {
    expect(validateIgnorePatterns([])).toBeNull();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[validation.test.ts::assertEmptyPatternsAllowed] ${reason}`);
  }
}

/**
 * Summary: Verify excessive ignore patterns are rejected.
 *
 * Inputs: None.
 *
 * Outputs: Asserts validation error text.
 *
 * Side effects: None.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: `validateIgnorePatterns` limit enforcement.
 *
 * Why this exists: Keep ignore pattern counts within safe limits.
 */
function assertRejectsTooManyPatterns(): void {
  try {
    const arr = new Array(201).fill('**/*.tmp');
    expect(validateIgnorePatterns(arr)).toContain('Too many');
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[validation.test.ts::assertRejectsTooManyPatterns] ${reason}`);
  }
}

/**
 * Summary: Verify invalid patterns are rejected.
 *
 * Inputs: None.
 *
 * Outputs: Asserts validation error text.
 *
 * Side effects: None.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: `validateIgnorePatterns` syntax validation.
 *
 * Why this exists: Block malformed ignore patterns early.
 */
function assertRejectsInvalidPatterns(): void {
  try {
    expect(validateIgnorePatterns(['[unclosed'])).toContain('Invalid ignore pattern');
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[validation.test.ts::assertRejectsInvalidPatterns] ${reason}`);
  }
}

/**
 * Summary: Verify valid glob patterns pass validation.
 *
 * Inputs: None.
 *
 * Outputs: Asserts null validation response.
 *
 * Side effects: None.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: `validateIgnorePatterns` happy path.
 *
 * Why this exists: Keep common glob patterns working as expected.
 */
function assertAcceptsValidGlobs(): void {
  try {
    expect(validateIgnorePatterns(['**/*.log', '**/node_modules/**'])).toBeNull();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[validation.test.ts::assertAcceptsValidGlobs] ${reason}`);
  }
}

describe('validateIgnorePatterns', () => {
  it('allows empty', assertEmptyPatternsAllowed);
  it('rejects too many', assertRejectsTooManyPatterns);
  it('rejects invalid regex-ish patterns', assertRejectsInvalidPatterns);
  it('passes valid globs', assertAcceptsValidGlobs);
});
