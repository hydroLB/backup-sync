/** Web-build replacement that keeps native dialog bindings out of the browser bundle. */
export type OpenDialogOptions = {
  title?: string;
  multiple?: boolean;
  directory?: boolean;
  [key: string]: unknown;
};

export async function open(): Promise<string | string[] | null> {
  throw new Error('Native dialogs are unavailable in the browser edition.');
}
