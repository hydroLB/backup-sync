import { validateIgnorePatterns } from '../validation';

/**
 * Purpose: Verify empty ignore patterns are accepted.
 *
 * Inputs: None.
 * Outputs: Asserts null validation response.
 * Ties to: `validateIgnorePatterns` empty handling.
 * Side effects: None.
 * Why: Allow operators to skip ignore patterns.
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
 * Purpose: Verify excessive ignore patterns are rejected.
 *
 * Inputs: None.
 * Outputs: Asserts validation error text.
 * Ties to: `validateIgnorePatterns` limit enforcement.
 * Side effects: None.
 * Why: Keep ignore pattern counts within safe limits.
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
 * Purpose: Verify invalid patterns are rejected.
 *
 * Inputs: None.
 * Outputs: Asserts validation error text.
 * Ties to: `validateIgnorePatterns` syntax validation.
 * Side effects: None.
 * Why: Block malformed ignore patterns early.
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
 * Purpose: Verify valid glob patterns pass validation.
 *
 * Inputs: None.
 * Outputs: Asserts null validation response.
 * Ties to: `validateIgnorePatterns` happy path.
 * Side effects: None.
 * Why: Keep common glob patterns working as expected.
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
