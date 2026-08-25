/** Web-build replacement that keeps the native IPC runtime out of the browser bundle. */
export async function invoke<T>(): Promise<T> {
  throw new Error('Native IPC is unavailable in the browser edition.');
}
