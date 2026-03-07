/**
 * Summary: Derive a semantic status-text class from free-form status copy.
 *
 * Inputs: Optional status message string.
 *
 * Outputs: Class name for one of ready, working, success, error, or default status text.
 *
 * Side effects: None.
 *
 * Error handling: Returns default class when message is missing or unclassified.
 *
 * Ties to other methods: Used by settings and status surfaces that render free-form messages.
 *
 * Why this exists: Keep Ready/Working/Success/Error color mapping consistent across widgets.
 */
export function statusTextClass(message?: string | null): string {
  if (!message) {
    return 'status-text';
  }

  const normalized = message.toLowerCase();

  if (
    /(failed|error|invalid|unavailable|unable|paused|denied|forbidden|timeout|warning)/.test(
      normalized,
    )
  ) {
    return 'status-text status-text-error';
  }

  if (
    /(saving|loading|running|refreshing|working|verifying|checking|processing)/.test(normalized)
  ) {
    return 'status-text status-text-working';
  }

  if (/(saved|success|done|added|completed|healthy|restored)/.test(normalized)) {
    return 'status-text status-text-success';
  }

  if (/(ready|idle|configured|connected|online|synced|none)/.test(normalized)) {
    return 'status-text status-text-ready';
  }

  return 'status-text';
}
