/** Allows log aggregation across UI and backend actions. */
export function correlationId(prefix: string): string {
  try {
    const rand = randomHex(2);
    return `${prefix}-${Date.now().toString(36)}-${rand}`;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[correlationId] Failed to generate correlation id: ${reason}`);
  }
}

/** Avoids predictable correlation ids in logs. */
function randomHex(bytes: number): string {
  try {
    if (!globalThis.crypto || typeof globalThis.crypto.getRandomValues !== 'function') {
      throw new Error('crypto.getRandomValues unavailable');
    }
    const data = new Uint8Array(bytes);
    globalThis.crypto.getRandomValues(data);
    return Array.from(data)
      .map((b) => b.toString(16).padStart(2, '0'))
      .join('');
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[randomHex] Failed to generate hex string: ${reason}`);
  }
}
