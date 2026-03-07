import "@testing-library/jest-dom";

/**
 * Summary: Install a deterministic in-memory `localStorage` shim when the test runtime lacks one.
 *
 * Inputs: None.
 *
 * Outputs: None.
 *
 * Side effects: Defines `window.localStorage` and `globalThis.localStorage` for Vitest when needed.
 *
 * Error handling: None.
 *
 * Ties to other methods: Supports color-mode, hardening, and onboarding tests that rely on browser storage.
 *
 * Why this exists: Some local Node test environments start without a complete `localStorage` implementation.
 */
function ensureLocalStorage(): void {
  if (
    typeof window.localStorage?.getItem === "function" &&
    typeof window.localStorage?.setItem === "function" &&
    typeof window.localStorage?.removeItem === "function"
  ) {
    return;
  }

  const store = new Map<string, string>();
  const storage: Storage = {
    get length() {
      return store.size;
    },
    clear() {
      store.clear();
    },
    getItem(key: string) {
      return store.has(key) ? store.get(key)! : null;
    },
    key(index: number) {
      return Array.from(store.keys())[index] ?? null;
    },
    removeItem(key: string) {
      store.delete(key);
    },
    setItem(key: string, value: string) {
      store.set(key, String(value));
    },
  };

  Object.defineProperty(window, "localStorage", {
    configurable: true,
    value: storage,
  });
  Object.defineProperty(globalThis, "localStorage", {
    configurable: true,
    value: storage,
  });
}

ensureLocalStorage();
