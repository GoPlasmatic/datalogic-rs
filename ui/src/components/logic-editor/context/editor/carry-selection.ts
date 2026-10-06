import type { LogicNode } from '../../types';

/**
 * Where each node sits in its tree: the chain of argument slots from the
 * root (`root/1/0`). Two conversions of similar rules give the node at the
 * same place the same key, whatever its id.
 */
function structuralKeys(nodes: LogicNode[]): Map<string, string> {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const keys = new Map<string, string>();
  const keyOf = (node: LogicNode, depth: number): string => {
    const cached = keys.get(node.id);
    if (cached !== undefined) return cached;
    const parent = node.data.parentId ? byId.get(node.data.parentId) : undefined;
    // The depth guard stops a malformed parent cycle from recursing forever.
    const key =
      parent && depth < nodes.length
        ? `${keyOf(parent, depth + 1)}/${node.data.argIndex ?? 0}`
        : 'root';
    keys.set(node.id, key);
    return key;
  };
  for (const node of nodes) keyOf(node, 0);
  return keys;
}

function sameExpression(a: LogicNode, b: LogicNode): boolean {
  return JSON.stringify(a.data.expression) === JSON.stringify(b.data.expression);
}

/**
 * Map node ids selected in `from` onto `to`, a fresh conversion of the
 * rule. An id carries over to the node at the same place in the tree, as
 * long as that node still holds the same expression; otherwise it is
 * dropped. Keeps a selection made on a node the editor just created (whose
 * id the conversion replaces) and refuses one that a shifted sibling would
 * otherwise inherit.
 */
export function carryNodeIds(ids: Iterable<string>, from: LogicNode[], to: LogicNode[]): Map<string, string> {
  const carried = new Map<string, string>();
  const wanted = [...ids];
  if (wanted.length === 0) return carried;

  const fromById = new Map(from.map((n) => [n.id, n]));
  const fromKeys = structuralKeys(from);
  const toKeys = structuralKeys(to);
  const toByKey = new Map<string, LogicNode>();
  for (const node of to) {
    const key = toKeys.get(node.id);
    if (key !== undefined && !toByKey.has(key)) toByKey.set(key, node);
  }

  for (const id of wanted) {
    const before = fromById.get(id);
    const key = before && fromKeys.get(id);
    const after = key !== undefined ? toByKey.get(key) : undefined;
    if (before && after && sameExpression(before, after)) carried.set(id, after.id);
  }
  return carried;
}
