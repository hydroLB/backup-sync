const BYTE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];
const BYTE_BASE = 1024;
const BYTE_SMALL_UNIT_DECIMALS = 2;
const BYTE_LARGE_UNIT_DECIMALS = 1;

/** Keeps size values consistent and readable across the UI. */
export function formatBytes(n?: number | null): string {
  try {
    if (n === undefined || n === null) {
      return 'n/a';
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

/** Keeps timing output compact and readable in tight UI areas. */
export function formatDuration(secs?: number | null): string {
  try {
    if (!secs || secs < 0) {
      return 'n/a';
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

/** Presents timestamps in a human friendly format for operators. */
export function formatDateTime(ts?: number | null): string {
  try {
    if (!ts) {
      return 'Never';
    }
    return new Date(ts * 1000).toLocaleString();
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[formatDateTime] Failed to format timestamp: ${reason}`);
  }
}

/** Helps operators judge freshness without reading exact timestamps. */
export function formatSince(ts?: number | null): string {
  try {
    if (!ts) {
      return 'never';
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
