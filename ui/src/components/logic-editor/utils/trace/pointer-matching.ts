import type { ExpressionNode } from '../../types/trace';
import type { ChildMatch } from './types';

/**
 * Resolve an RFC 6901 JSON Pointer against `root`. `undefined` when a token
 * names nothing.
 */
export function resolvePointer(root: unknown, pointer: string): unknown {
  if (pointer === '') return root;
  if (!pointer.startsWith('/')) return undefined;
  let current: unknown = root;
  for (const raw of pointer.slice(1).split('/')) {
    const token = raw.replace(/~1/g, '/').replace(/~0/g, '~');
    if (Array.isArray(current)) {
      if (!/^(0|[1-9]\d*)$/.test(token)) return undefined;
      current = current[Number(token)];
    } else if (current !== null && typeof current === 'object' && Object.hasOwn(current, token)) {
      current = (current as Record<string, unknown>)[token];
    } else {
      return undefined;
    }
  }
  return current;
}

/**
 * Where each trace node came from in the rule. The engine records, for every
 * node id, the JSON Pointer of the rule value it compiled (`pointers` in the
 * trace envelope); this resolves those pointers once against the rule the
 * editor shows, so a node's source is the very object the editor holds.
 */
export class TraceSources {
  private readonly sources = new Map<number, unknown>();
  readonly pointers = new Map<number, string>();

  constructor(rule: unknown, pointers: Record<string, string> | undefined) {
    for (const [id, pointer] of Object.entries(pointers ?? {})) {
      const nodeId = Number(id);
      this.pointers.set(nodeId, pointer);
      const source = resolvePointer(rule, pointer);
      if (source !== undefined) this.sources.set(nodeId, source);
    }
  }

  /** The rule value node `id` was compiled from, if known. */
  sourceOf(id: number): unknown {
    return this.sources.get(id);
  }
}

/**
 * Pair each operand with the trace child compiled from it. An operand is a
 * value inside the rule the editor shows, and a child's source is resolved
 * from the same rule, so the pairing is by identity: no canonical forms,
 * aliases or positions involved. Literal operands have no trace node and stay
 * unmatched. Returns one entry per operand.
 */
export function matchOperandsToChildren(
  operands: readonly unknown[],
  children: readonly ExpressionNode[],
  sources: TraceSources,
): (ChildMatch | null)[] {
  const used = new Set<number>();
  return operands.map((operand) => {
    if (operand === null || typeof operand !== 'object') return null;
    for (let i = 0; i < children.length; i++) {
      if (!used.has(i) && sources.sourceOf(children[i].id) === operand) {
        used.add(i);
        return { child: children[i], index: i };
      }
    }
    return null;
  });
}

/**
 * Trace children no operand claimed. Their steps still need a home, so
 * callers fold them into the parent visual node.
 */
export function unmatchedChildren(
  children: readonly ExpressionNode[],
  matches: readonly (ChildMatch | null)[],
): ExpressionNode[] {
  const used = new Set(matches.filter((m): m is ChildMatch => m !== null).map((m) => m.index));
  return children.filter((_, i) => !used.has(i));
}

/**
 * The tree node a step belongs to when the expression tree does not list its
 * id (an operator whose arguments the tree folds into one leaf, such as
 * `missing` with computed paths): the listed node whose pointer is the
 * longest prefix of the step's. `undefined` when nothing contains it.
 */
export function enclosingNode(
  id: number,
  listed: readonly number[],
  sources: TraceSources,
): number | undefined {
  const pointer = sources.pointers.get(id);
  if (pointer === undefined) return undefined;
  let best: number | undefined;
  let bestLength = -1;
  for (const candidate of listed) {
    const p = sources.pointers.get(candidate);
    if (p === undefined) continue;
    const contains = p === pointer || p === '' || pointer.startsWith(`${p}/`);
    if (contains && p.length > bestLength) {
      best = candidate;
      bestLength = p.length;
    }
  }
  return best;
}
