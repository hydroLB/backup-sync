/** Keep Ready/Working/Success/Error color mapping consistent across widgets. */
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
