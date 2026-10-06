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
 *
 * Pointers are into the rule as written, so they resolve only against that
 * rule. Without it (`rule` undefined) no node has a source and matching falls
 * back to the expression text; the pointers still order nodes by containment
 * (see `enclosingNode`).
 */
export class TraceSources {
  private readonly sources = new Map<number, unknown>();
  readonly pointers = new Map<number, string>();

  constructor(rule: unknown, pointers: Record<string, string> | undefined) {
    for (const [id, pointer] of Object.entries(pointers ?? {})) {
      const nodeId = Number(id);
      this.pointers.set(nodeId, pointer);
      if (rule === undefined) continue;
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
 *
 * A child with no source (the trace carries no pointers, or the rule as
 * written is not known) pairs, in order, with an operand whose JSON equals the
 * child's expression, which holds when the operands are the engine's own
 * serialization. An operand written in another form (`?:` for `if`, or with
 * such an alias anywhere inside) then pairs by position, but only when the
 * operands and sourceless children left over are as many: the engine lists
 * one child per non-literal operand, in order. Otherwise it stays unmatched
 * and its steps fold into the parent node.
 */
export function matchOperandsToChildren(
  operands: readonly unknown[],
  children: readonly ExpressionNode[],
  sources: TraceSources,
): (ChildMatch | null)[] {
  const used = new Set<number>();
  const matches = operands.map((operand): ChildMatch | null => {
    if (operand === null || typeof operand !== 'object') return null;
    for (let i = 0; i < children.length; i++) {
      if (!used.has(i) && sources.sourceOf(children[i].id) === operand) {
        used.add(i);
        return { child: children[i], index: i };
      }
    }
    return null;
  });
  // Later passes, so a sourceless child never takes an operand that a
  // sourced sibling names
  const sourceless = (i: number) => !used.has(i) && sources.sourceOf(children[i].id) === undefined;
  const open = () => operands.flatMap((operand, k) =>
    matches[k] || operand === null || typeof operand !== 'object' ? [] : [k]);
  for (const k of open()) {
    const text = JSON.stringify(operands[k]);
    const i = children.findIndex((child, j) => sourceless(j) && expressionText(child) === text);
    if (i !== -1) {
      used.add(i);
      matches[k] = { child: children[i], index: i };
    }
  }
  const left = open();
  const spare = children.flatMap((_, i) => (sourceless(i) ? [i] : []));
  if (left.length === spare.length) {
    left.forEach((k, n) => {
      used.add(spare[n]);
      matches[k] = { child: children[spare[n]], index: spare[n] };
    });
  }
  return matches;
}

/**
 * A tree node's expression in `JSON.stringify` form. The engine spaces its
 * JSON differently, so the text is compared only after a round trip.
 */
function expressionText(node: ExpressionNode): string | undefined {
  try {
    return JSON.stringify(JSON.parse(node.expression));
  } catch {
    return undefined;
  }
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
 * The tree node a step belongs to when no visual node claims its id (an
 * operator whose arguments the tree folds into one leaf, such as `missing`
 * with computed paths): the listed node whose pointer is the longest proper
 * prefix of the step's, ending at a token boundary. Only strict ancestors
 * count, so the step's own id (or one compiled from the same value) is never
 * the answer. `undefined` when nothing contains it.
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
    if (candidate === id) continue;
    const p = sources.pointers.get(candidate);
    if (p === undefined || p === pointer) continue;
    const contains = p === '' || pointer.startsWith(`${p}/`);
    if (contains && p.length > bestLength) {
      best = candidate;
      bestLength = p.length;
    }
  }
  return best;
}
