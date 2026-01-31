import { formatBytes, formatDuration, formatDateTime, formatSince } from "../format";

/**
 * Purpose: Verify null and undefined bytes format to a fallback.
 *
 * Inputs: None.
 * Outputs: Asserts fallback strings.
 * Ties to: `formatBytes` null handling.
 * Side effects: None.
 * Why: Keep missing values readable in the UI.
 */
function assertFormatBytesHandlesNull(): void {
  try {
    expect(formatBytes(null)).toBe("n/a");
    expect(formatBytes(undefined)).toBe("n/a");
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatBytesHandlesNull] ${reason}`);
  }
}

/**
 * Purpose: Verify byte formatting for small numbers.
 *
 * Inputs: None.
 * Outputs: Asserts formatted byte label.
 * Ties to: `formatBytes` small number formatting.
 * Side effects: None.
 * Why: Ensure small sizes display correctly.
 */
function assertFormatBytesSmallNumbers(): void {
  try {
    expect(formatBytes(500)).toBe("500.00 B");
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatBytesSmallNumbers] ${reason}`);
  }
}

/**
 * Purpose: Verify byte formatting for KB, MB, and GB.
 *
 * Inputs: None.
 * Outputs: Asserts formatted unit labels.
 * Ties to: `formatBytes` unit scaling.
 * Side effects: None.
 * Why: Keep consistent unit labels across UI.
 */
function assertFormatBytesUnits(): void {
  try {
    expect(formatBytes(1024)).toBe("1.00 KB");
    expect(formatBytes(1024 * 1024)).toBe("1.00 MB");
    expect(formatBytes(5 * 1024 * 1024 * 1024)).toBe("5.0 GB");
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatBytesUnits] ${reason}`);
  }
}

/**
 * Purpose: Verify duration formatting handles null and negative values.
 *
 * Inputs: None.
 * Outputs: Asserts fallback strings.
 * Ties to: `formatDuration` null handling.
 * Side effects: None.
 * Why: Prevent invalid durations from rendering as real values.
 */
function assertFormatDurationHandlesNull(): void {
  try {
    expect(formatDuration(null)).toBe("n/a");
    expect(formatDuration(-1)).toBe("n/a");
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatDurationHandlesNull] ${reason}`);
  }
}

/**
 * Purpose: Verify duration formatting for minutes and hours.
 *
 * Inputs: None.
 * Outputs: Asserts formatted duration strings.
 * Ties to: `formatDuration` output formatting.
 * Side effects: None.
 * Why: Keep uptime displays consistent.
 */
function assertFormatDurationMinutesHours(): void {
  try {
    expect(formatDuration(60)).toBe("1m");
    expect(formatDuration(3600 + 120)).toBe("1h 2m");
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatDurationMinutesHours] ${reason}`);
  }
}

/**
 * Purpose: Verify missing timestamps render as "Never".
 *
 * Inputs: None.
 * Outputs: Asserts the "Never" label.
 * Ties to: `formatDateTime` null handling.
 * Side effects: None.
 * Why: Keep missing timestamps obvious in the UI.
 */
function assertFormatDateTimeMissing(): void {
  try {
    expect(formatDateTime(null)).toBe("Never");
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatDateTimeMissing] ${reason}`);
  }
}

/**
 * Purpose: Verify timestamps render human-readable dates.
 *
 * Inputs: None.
 * Outputs: Asserts formatted date contains the expected year.
 * Ties to: `formatDateTime` date formatting.
 * Side effects: None.
 * Why: Ensure audit timestamps are readable.
 */
function assertFormatDateTimeTimestamp(): void {
  try {
    const ts = 1_600_000_000; // epoch seconds
    expect(formatDateTime(ts)).toContain("2020");
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatDateTimeTimestamp] ${reason}`);
  }
}

/**
 * Purpose: Verify missing timestamps for "since" render as "never".
 *
 * Inputs: None.
 * Outputs: Asserts the "never" label.
 * Ties to: `formatSince` null handling.
 * Side effects: None.
 * Why: Keep stale timestamps obvious in the UI.
 */
function assertFormatSinceMissing(): void {
  try {
    expect(formatSince(null)).toBe("never");
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatSinceMissing] ${reason}`);
  }
}

/**
 * Purpose: Verify "since" formatting for minutes and hours.
 *
 * Inputs: None.
 * Outputs: Asserts formatted relative time strings.
 * Ties to: `formatSince` time delta formatting.
 * Side effects: None.
 * Why: Ensure relative timestamps are consistent.
 */
function assertFormatSinceMinutesHours(): void {
  try {
    const now = Date.now() / 1000;
    expect(formatSince(now - 60)).toBe("1m ago");
    expect(formatSince(now - 7200)).toBe("2h 0m ago");
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[format.test.ts::assertFormatSinceMinutesHours] ${reason}`);
  }
}

describe("formatBytes", () => {
  it("formats null/undefined", assertFormatBytesHandlesNull);
  it("formats small numbers", assertFormatBytesSmallNumbers);
  it("formats KB/MB/GB", assertFormatBytesUnits);
});

describe("formatDuration", () => {
  it("handles null/negative", assertFormatDurationHandlesNull);
  it("formats minutes/hours", assertFormatDurationMinutesHours);
});

describe("formatDateTime", () => {
  it("returns Never for missing", assertFormatDateTimeMissing);
  it("formats timestamps", assertFormatDateTimeTimestamp);
});

describe("formatSince", () => {
  it("handles missing", assertFormatSinceMissing);
  it("formats minutes and hours", assertFormatSinceMinutesHours);
});
