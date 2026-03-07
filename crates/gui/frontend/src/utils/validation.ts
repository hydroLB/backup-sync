import { UI_TUNING } from '../config/uiTuning';

const MAX_IGNORE_PATTERNS = UI_TUNING.configLimits.maxIgnorePatterns;

/**
 * Summary: Validate ignore patterns for syntax and size limits before saving settings.
 *
 * Inputs: `patterns` as user supplied glob-like strings.
 *
 * Outputs: `null` when valid, or a message describing the first validation failure.
 *
 * Side effects: None.
 *
 * Error handling: Propagates contextual errors to the caller when operations fail.
 *
 * Ties to other methods: Settings forms that persist ignore rules and backend ignore filtering.
 *
 * Why this exists: Protects the backup planner from invalid patterns and excessive configuration.
 */
export function validateIgnorePatterns(patterns: string[]): string | null {
  try {
    if (patterns.length > MAX_IGNORE_PATTERNS) {
      return `[validateIgnorePatterns] Too many ignore patterns; trim to ${MAX_IGNORE_PATTERNS} or fewer.`;
    }
    for (const pat of patterns) {
      if (!pat.trim()) {
        continue;
      }
      try {
        new RegExp(pat.replace(/\*\*/g, '.*').replace(/\*/g, '[^/]*'));
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        return `[validateIgnorePatterns] Invalid ignore pattern: ${pat} (${reason})`;
      }
    }
    return null;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    return `[validateIgnorePatterns] Unexpected validation failure: ${reason}`;
  }
}
