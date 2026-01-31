/**
 * Purpose: Define a binary search tree node keyed by string.
 *
 * Inputs: `key`, `value`, and optional left/right children.
 * Outputs: A tree node shape used for indexed lookups.
 * Ties to: `bstInsert`, `bstFind`, `bstAnyPrefix`, and `buildPathIndex`.
 * Side effects: None.
 * Why: Enables fast prefix and exact lookup of path lists.
 */
export type BstNode<T> = {
  key: string;
  value: T;
  left?: BstNode<T>;
  right?: BstNode<T>;
};

/**
 * Purpose: Insert or update a key/value pair in a string keyed BST.
 *
 * Inputs: `root` as the existing tree, `key` and `value` as the entry.
 * Outputs: The updated tree root containing the entry.
 * Ties to: `buildPathIndex` and call sites that update the path index.
 * Side effects: Mutates tree nodes while inserting or updating entries.
 * Why: Keeps path indices updated as new items are added or replaced.
 */
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

/**
 * Purpose: Find a value by key in a string keyed BST.
 *
 * Inputs: `root` as the tree to search, `key` as the target.
 * Outputs: The matching value or `undefined` when absent.
 * Ties to: Exact path lookup helpers in settings validation and UI checks.
 * Side effects: None.
 * Why: Provides efficient exact lookup for path keyed data.
 */
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

/**
 * Purpose: Find a value when any stored key is a prefix of the input path.
 *
 * Inputs: `root` as the tree to search, `path` as the candidate path.
 * Outputs: The first matching value or `undefined` when none match.
 * Ties to: Prefix checks in watch list overlap detection.
 * Side effects: None.
 * Why: Detects when a path is covered by a watched prefix.
 */
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

/**
 * Purpose: Build a BST index from items containing a `path` field.
 *
 * Inputs: `items` as the list of path-bearing entries.
 * Outputs: The root node of the path index or `undefined` for empty input.
 * Ties to: Settings validation and overlap checks for watch lists.
 * Side effects: Allocates nodes and mutates tree links during construction.
 * Why: Enables fast prefix queries against path lists.
 */
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
