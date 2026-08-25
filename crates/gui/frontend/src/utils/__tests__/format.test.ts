import { formatBytes, formatDuration, formatDateTime, formatSince } from '../format';

/** Keep missing values readable in the UI. */
function assertFormatBytesHandlesNull(): void {
  try {
    expect(formatBytes(null)).toBe('n/a');
    expect(formatBytes(undefined)).toBe('n/a');
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatBytesHandlesNull] ${reason}`);
  }
}

/** Ensure small sizes display correctly. */
function assertFormatBytesSmallNumbers(): void {
  try {
    expect(formatBytes(500)).toBe('500.00 B');
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatBytesSmallNumbers] ${reason}`);
  }
}

/** Keep consistent unit labels across UI. */
function assertFormatBytesUnits(): void {
  try {
    expect(formatBytes(1024)).toBe('1.00 KB');
    expect(formatBytes(1024 * 1024)).toBe('1.00 MB');
    expect(formatBytes(5 * 1024 * 1024 * 1024)).toBe('5.0 GB');
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatBytesUnits] ${reason}`);
  }
}

/** Prevent invalid durations from rendering as real values. */
function assertFormatDurationHandlesNull(): void {
  try {
    expect(formatDuration(null)).toBe('n/a');
    expect(formatDuration(-1)).toBe('n/a');
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatDurationHandlesNull] ${reason}`);
  }
}

/** Keep uptime displays consistent. */
function assertFormatDurationMinutesHours(): void {
  try {
    expect(formatDuration(60)).toBe('1m');
    expect(formatDuration(3600 + 120)).toBe('1h 2m');
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatDurationMinutesHours] ${reason}`);
  }
}

/** Keep missing timestamps obvious in the UI. */
function assertFormatDateTimeMissing(): void {
  try {
    expect(formatDateTime(null)).toBe('Never');
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatDateTimeMissing] ${reason}`);
  }
}

/** Ensure audit timestamps are readable. */
function assertFormatDateTimeTimestamp(): void {
  try {
    const ts = 1_600_000_000; // epoch seconds
    expect(formatDateTime(ts)).toContain('2020');
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatDateTimeTimestamp] ${reason}`);
  }
}

/** Keep stale timestamps obvious in the UI. */
function assertFormatSinceMissing(): void {
  try {
    expect(formatSince(null)).toBe('never');
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatSinceMissing] ${reason}`);
  }
}

/** Ensure relative timestamps are consistent. */
function assertFormatSinceMinutesHours(): void {
  try {
    const now = Date.now() / 1000;
    expect(formatSince(now - 60)).toBe('1m ago');
    expect(formatSince(now - 7200)).toBe('2h 0m ago');
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatSinceMinutesHours] ${reason}`);
  }
}

describe('formatBytes', () => {
  it('formats null/undefined', assertFormatBytesHandlesNull);
  it('formats small numbers', assertFormatBytesSmallNumbers);
  it('formats KB/MB/GB', assertFormatBytesUnits);
});

describe('formatDuration', () => {
  it('handles null/negative', assertFormatDurationHandlesNull);
  it('formats minutes/hours', assertFormatDurationMinutesHours);
});

describe('formatDateTime', () => {
  it('returns Never for missing', assertFormatDateTimeMissing);
  it('formats timestamps', assertFormatDateTimeTimestamp);
});

describe('formatSince', () => {
  it('handles missing', assertFormatSinceMissing);
  it('formats minutes and hours', assertFormatSinceMinutesHours);
});
