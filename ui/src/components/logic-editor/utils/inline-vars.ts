/**
 * Inline-var collapsing — a static var/val read renders as an inline data pill
 * instead of its own wired child node, so simple lookups don't clutter the graph.
 * Shared by the static converter (converters/operator-converter) and the trace
 * builder (trace/node-creators/vertical-cell) so both views agree.
 */
import type { JsonLogicValue } from '../types';
import { isPlainObject } from './type-helpers';
import { formatVarValPath } from './formatting';

// How many static var reads may collapse into inline pills within one operator.
// Busier expressions (3+ vars) keep wiring, so the data flow stays legible.
export const MAX_INLINE_VARS = 2;

const isStatic = (v: JsonLogicValue): boolean => typeof v === 'string' || typeof v === 'number';

/**
 * The display label for a static var/val read that can collapse into an inline
 * pill, or null if it can't. Covered: a bare string/number path (including `""`,
 * the whole context), and a `val` scope-jump like `{val:[[-1],"x"]}` → "↑x".
 * A `var` with a default value stays wired so the default isn't hidden.
 */
export function staticVarLabel(operand: JsonLogicValue): string | null {
  if (!isPlainObject(operand)) return null;
  const keys = Object.keys(operand);
  if (keys.length !== 1) return null;
  const op = keys[0];
  if (op !== 'var' && op !== 'val') return null;
  const raw = (operand as Record<string, JsonLogicValue>)[op];

  if (op === 'var') {
    if (isStatic(raw)) return formatVarValPath('var', raw);
    // Single-element array is just a path (no default); 2+ has a default → wire it.
    if (Array.isArray(raw) && raw.length <= 1) return formatVarValPath('var', raw);
    return null;
  }

  // val: bare path, or scope-jump form [[n], ...static parts].
  if (isStatic(raw)) return formatVarValPath('val', raw);
  if (Array.isArray(raw)) {
    const [scope, ...parts] = raw;
    const scopeOk =
      scope === undefined ||
      (Array.isArray(scope) ? scope.every((x) => typeof x === 'number') : isStatic(scope));
    if (scopeOk && parts.every(isStatic)) return formatVarValPath('val', raw);
  }
  return null;
}

/**
 * The set of operand indices that should collapse into inline var pills. Empty
 * when there are none or more than MAX_INLINE_VARS (all vars stay wired then).
 */
export function inlineVarIndices(operandArray: JsonLogicValue[]): Set<number> {
  const idxs: number[] = [];
  operandArray.forEach((operand, i) => {
    if (staticVarLabel(operand) !== null) idxs.push(i);
  });
  return idxs.length > 0 && idxs.length <= MAX_INLINE_VARS ? new Set(idxs) : new Set<number>();
}
