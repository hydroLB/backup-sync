const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"];
const BYTE_BASE = 1024;
const BYTE_SMALL_UNIT_DECIMALS = 2;
const BYTE_LARGE_UNIT_DECIMALS = 1;

/**
 * Purpose: Convert a byte count into a human readable storage string.
 *
 * Inputs: `n` as a byte count or null/undefined.
 * Outputs: A formatted size string for UI display.
 * Ties to: Status cards and storage indicators that surface capacity data.
 * Side effects: None.
 * Why: Keeps size values consistent and readable across the UI.
 */
export function formatBytes(n?: number | null): string {
  try {
    if (n === undefined || n === null) {
      return "n/a";
    }
    let val = n;
    let i = 0;
    while (val >= BYTE_BASE && i < BYTE_UNITS.length - 1) {
      val /= BYTE_BASE;
      i++;
    }
    if (i === 0) {
      return `${val.toFixed(BYTE_SMALL_UNIT_DECIMALS)} ${BYTE_UNITS[i]}`;
    }
    if (i <= 2) {
      return `${val.toFixed(BYTE_SMALL_UNIT_DECIMALS)} ${BYTE_UNITS[i]}`;
    }
    return `${val.toFixed(BYTE_LARGE_UNIT_DECIMALS)} ${BYTE_UNITS[i]}`;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[formatBytes] Failed to format byte value: ${reason}`);
  }
}

const SECONDS_PER_HOUR = 3600;
const SECONDS_PER_MINUTE = 60;
const MINUTES_PER_HOUR = 60;

/**
 * Purpose: Convert a duration in seconds into a compact display string.
 *
 * Inputs: `secs` as a number of seconds or null/undefined.
 * Outputs: A short duration string such as "2h 5m".
 * Ties to: Status panels that display last run duration and uptime.
 * Side effects: None.
 * Why: Keeps timing output compact and readable in tight UI areas.
 */
export function formatDuration(secs?: number | null): string {
  try {
    if (!secs || secs < 0) {
      return "n/a";
    }
    const h = Math.floor(secs / SECONDS_PER_HOUR);
    const m = Math.floor((secs % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE);
    if (h > 0) {
      return `${h}h ${m}m`;
    }
    return `${m}m`;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[formatDuration] Failed to format duration: ${reason}`);
  }
}

/**
 * Purpose: Convert a Unix timestamp into a localized datetime string.
 *
 * Inputs: `ts` as seconds since epoch or null/undefined.
 * Outputs: A localized datetime string such as "2/14/2025, 3:22:10 PM".
 * Ties to: Status summaries and log entries with point-in-time labels.
 * Side effects: None.
 * Why: Presents timestamps in a human friendly format for operators.
 */
export function formatDateTime(ts?: number | null): string {
  try {
    if (!ts) {
      return "Never";
    }
    return new Date(ts * 1000).toLocaleString();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[formatDateTime] Failed to format timestamp: ${reason}`);
  }
}

/**
 * Purpose: Convert a Unix timestamp into a relative time string.
 *
 * Inputs: `ts` as seconds since epoch or null/undefined.
 * Outputs: A relative time string like "5m ago" or "2h 1m ago".
 * Ties to: Recency indicators in status cards and activity feeds.
 * Side effects: None.
 * Why: Helps operators judge freshness without reading exact timestamps.
 */
export function formatSince(ts?: number | null): string {
  try {
    if (!ts) {
      return "never";
    }
    const now = Date.now() / 1000;
    const delta = Math.max(0, now - ts);
    const mins = Math.floor(delta / SECONDS_PER_MINUTE);
    const hours = Math.floor(mins / MINUTES_PER_HOUR);
    if (hours > 0) {
      return `${hours}h ${mins % MINUTES_PER_HOUR}m ago`;
    }
    return `${mins}m ago`;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[formatSince] Failed to format relative time: ${reason}`);
  }
}
