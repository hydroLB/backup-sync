/** Enables fast prefix and exact lookup of path lists. */
export type BstNode<T> = {
  key: string;
  value: T;
  left?: BstNode<T>;
  right?: BstNode<T>;
};

/** Keeps path indices updated as new items are added or replaced. */
export function bstInsert<T>(root: BstNode<T> | undefined, key: string, value: T): BstNode<T> {
  try {
    if (!root) {
      return { key, value };
    }
    if (key < root.key) {
      root.left = bstInsert(root.left, key, value);
    } else if (key > root.key) {
      root.right = bstInsert(root.right, key, value);
    } else {
      root.value = value;
    }
    return root;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[bstInsert] Failed to insert BST node: ${reason}`);
  }
}

/** Provides efficient exact lookup for path keyed data. */
export function bstFind<T>(root: BstNode<T> | undefined, key: string): T | undefined {
  try {
    if (!root) {
      return undefined;
    }
    if (key === root.key) {
      return root.value;
    }
    if (key < root.key) {
      return bstFind(root.left, key);
    }
    return bstFind(root.right, key);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[bstFind] Failed to locate BST key: ${reason}`);
  }
}

/** Detects when a path is covered by a watched prefix. */
export function bstAnyPrefix<T>(root: BstNode<T> | undefined, path: string): T | undefined {
  try {
    if (!root) {
      return undefined;
    }
    if (path.startsWith(root.key)) {
      return root.value;
    }
    if (path < root.key) {
      return bstAnyPrefix(root.left, path);
    }
    return bstAnyPrefix(root.right, path);
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[bstAnyPrefix] Failed to resolve prefix match: ${reason}`);
  }
}

/** Enables fast prefix queries against path lists. */
export function buildPathIndex<T extends { path: string }>(items: T[]): BstNode<T> | undefined {
  try {
    let root: BstNode<T> | undefined;
    for (const item of items) {
      root = bstInsert(root, item.path, item);
    }
    return root;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`[buildPathIndex] Failed to build path index: ${reason}`);
  }
}
