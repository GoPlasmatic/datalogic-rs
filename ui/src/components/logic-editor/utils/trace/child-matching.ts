import type { JsonLogicValue } from '../../types';
import type { ExpressionNode } from '../../types/trace';
import type { ChildMatch } from './types';

/**
 * Operator aliases the engine collapses at compile time. The trace tree only
 * ever shows the canonical name, so both sides are mapped before comparing.
 */
const OPERATOR_ALIASES: Record<string, string> = {
  '?:': 'if',
  match: 'switch',
};

const isPathSegment = (v: unknown): v is string | number =>
  typeof v === 'string' || typeof v === 'number';

const isScopeArray = (v: unknown): v is number[] =>
  Array.isArray(v) && v.every((n) => typeof n === 'number');

/**
 * Canonical form of a scoped path lookup: `[[level], ...segments]`. Level 0
 * folds into a plain `var` with a dotted path; anything else stays a `val`
 * with the absolute level (the engine normalizes `[-1]` to `[1]`).
 */
function canonicalScopedPath(scope: number[], segments: unknown[]): unknown | null {
  if (!segments.every(isPathSegment)) return null;
  const level = Math.abs(scope[0] ?? 0);
  if (level === 0) return { var: segments.map(String).join('.') };
  return { val: [[level], ...segments] };
}

/**
 * Canonicalize a static `var` / `val` read the way the compiler serializes
 * it: `{"var": "a.b"}`, `{"var": ["a.b", default]}` or `{"val": [[n], ...]}`.
 * Returns null for dynamic paths (an expression as the path), which are left
 * to the generic normalizer.
 */
function canonicalVarVal(operator: 'var' | 'val', raw: unknown): unknown | null {
  if (raw === null || raw === undefined) return { var: '' };
  if (isPathSegment(raw)) return { var: String(raw) };
  if (!Array.isArray(raw)) return null;
  if (raw.length === 0) return { var: '' };

  const [first, ...rest] = raw;
  if (isScopeArray(first)) return canonicalScopedPath(first, rest);

  if (operator === 'var') {
    if (first === null) return rest.length === 0 ? { var: '' } : { var: ['', normalizeExpression(rest[0])] };
    if (!isPathSegment(first)) return null;
    if (rest.length === 0) return { var: String(first) };
    return { var: [String(first), normalizeExpression(rest[0])] };
  }

  // val: every element is a path segment, joined into one dotted path
  if (raw.every(isPathSegment)) return { var: raw.map(String).join('.') };
  return null;
}

/**
 * Normalize a JSONLogic expression toward the compiler's canonical shape so
 * the original rule and the engine's trace expression compare equal:
 * - `{"op": [x]}` unwraps to `{"op": x}` (single-arg operators)
 * - `?:` becomes `if`, `match` becomes `switch`
 * - static `var` / `val` paths collapse to the compiled dotted / scoped form
 *   (including numeric paths, which the engine emits as strings)
 * - `{"throw": {"type": t}}` becomes `{"throw": t}`
 * Applied to both sides before comparison, so the direction only matters for
 * the shapes the engine actually rewrites.
 */
export function normalizeExpression(expr: unknown): unknown {
  if (expr === null || typeof expr !== 'object') return expr;
  if (Array.isArray(expr)) return expr.map(normalizeExpression);

  const obj = expr as Record<string, unknown>;
  const keys = Object.keys(obj);
  if (keys.length !== 1) {
    // Multi-key object (template structure): normalize values recursively
    const result: Record<string, unknown> = {};
    for (const k of keys) result[k] = normalizeExpression(obj[k]);
    return result;
  }

  // Single-key object = JSONLogic operator
  const rawKey = keys[0];
  const key = OPERATOR_ALIASES[rawKey] ?? rawKey;
  let value = obj[rawKey];

  if (key === 'var' || key === 'val') {
    const canonical = canonicalVarVal(key, value);
    if (canonical !== null) return canonical;
    // Computed path: the engine serializes a dynamic var as val; use one key for both
    return { var: normalizeExpression(value) };
  }

  if (key === 'throw' && value !== null && typeof value === 'object' && !Array.isArray(value)) {
    const thrown = value as Record<string, unknown>;
    const thrownKeys = Object.keys(thrown);
    if (thrownKeys.length === 1 && thrownKeys[0] === 'type' && typeof thrown.type === 'string') {
      return { throw: thrown.type };
    }
  }

  // Unwrap single-element arrays: {"op": [x]} -> {"op": x}
  if (Array.isArray(value) && value.length === 1) {
    value = value[0];
  }

  return { [key]: normalizeExpression(value) };
}

/**
 * Deep equality comparison that ignores object key ordering.
 * Rust's serde_json uses BTreeMap (alphabetical keys) while JS preserves insertion order.
 */
function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (a === null || b === null || typeof a !== typeof b) return a === b;
  if (Array.isArray(a)) {
    if (!Array.isArray(b) || a.length !== b.length) return false;
    return a.every((v, i) => deepEqual(v, b[i]));
  }
  if (typeof a === 'object') {
    const aObj = a as Record<string, unknown>;
    const bObj = b as Record<string, unknown>;
    const aKeys = Object.keys(aObj);
    const bKeys = Object.keys(bObj);
    if (aKeys.length !== bKeys.length) return false;
    return aKeys.every(k => k in bObj && deepEqual(aObj[k], bObj[k]));
  }
  return false;
}

/**
 * Returned by [`parseTraceExpression`] when the expression string is not
 * JSON. A distinct sentinel rather than null, which is a value the string
 * `"null"` legitimately parses to.
 */
export const UNPARSEABLE = Symbol('unparseable-trace-expression');

/**
 * Parse a trace child's expression string. The engine does not escape quotes
 * inside keys or paths, so a rare malformed string yields [`UNPARSEABLE`]
 * rather than throwing.
 */
export function parseTraceExpression(child: ExpressionNode): unknown | typeof UNPARSEABLE {
  try {
    return JSON.parse(child.expression);
  } catch {
    return UNPARSEABLE;
  }
}

/**
 * Coarse shape of an expression, used for the loose (second-pass) match:
 * `op:<canonical operator>` for operator objects (`var` and `val` share one
 * bucket since the engine converts between them), `array` for arrays with
 * nested content, `object` for template structures, null for primitives.
 */
function expressionKind(expr: unknown | typeof UNPARSEABLE): string | null {
  if (expr === UNPARSEABLE || expr === null || typeof expr !== 'object') return null;
  if (Array.isArray(expr)) return 'array';
  const normalized = normalizeExpression(expr) as Record<string, unknown>;
  const keys = Object.keys(normalized);
  if (keys.length !== 1) return 'object';
  const op = keys[0] === 'val' ? 'var' : keys[0];
  return `op:${op}`;
}

/**
 * Whether an operand could own a node in the engine's expression tree. The
 * tree keeps operators, arrays that contain operators, and (in templating
 * mode) nested template structures; plain literals never appear.
 */
export function mayHaveTraceNode(value: unknown, templating: boolean): boolean {
  if (value === null || typeof value !== 'object') return false;
  if (Array.isArray(value)) {
    // Any nested container may own a node; the recursive check adds nothing
    // here since it is only ever true for values this test already accepts.
    return value.some((item) => item !== null && typeof item === 'object');
  }
  const keys = Object.keys(value);
  if (keys.length === 1) return true;
  return templating && keys.length > 0;
}

/**
 * Find the matching child node for an operand by comparing expressions.
 * Uses deep equality (key-order insensitive) on the canonicalized forms so the
 * original rule matches the engine's rewritten expression string.
 */
export function findMatchingChild(
  operand: JsonLogicValue,
  children: ExpressionNode[],
  usedIndices: Set<number>
): ChildMatch | null {
  const normalizedOperand = normalizeExpression(operand);
  for (let i = 0; i < children.length; i++) {
    if (usedIndices.has(i)) continue;
    const childExpr = parseTraceExpression(children[i]);
    if (childExpr === UNPARSEABLE) {
      // Unparseable expression string: fall back to string comparison
      if (children[i].expression === JSON.stringify(operand)) {
        return { child: children[i], index: i };
      }
      continue;
    }
    if (deepEqual(operand, childExpr) || deepEqual(normalizedOperand, normalizeExpression(childExpr))) {
      return { child: children[i], index: i };
    }
  }
  return null;
}

/**
 * Get the next unused child (for positional matching when exact matching fails)
 */
export function getNextUnusedChild(
  children: ExpressionNode[],
  usedIndices: Set<number>
): ChildMatch | null {
  for (let i = 0; i < children.length; i++) {
    if (!usedIndices.has(i)) {
      return { child: children[i], index: i };
    }
  }
  return null;
}

/**
 * Resolve every operand of an operator to its trace child in three passes:
 * 1. exact (canonicalized) match,
 * 2. loose match on expression kind (same operator / array / structure) for
 *    operands the engine partially rewrote, e.g. constant-folded arguments,
 * 3. positional, only when the leftover operands and children pair up 1:1.
 * Returns one entry per operand (null when the operand has no trace node,
 * e.g. a literal or a fully constant-folded sub-expression).
 */
export function matchOperandsToChildren(
  operands: JsonLogicValue[],
  children: ExpressionNode[],
  templating: boolean
): (ChildMatch | null)[] {
  const used = new Set<number>();
  const matches: (ChildMatch | null)[] = operands.map(() => null);

  // Pass 1: exact match
  operands.forEach((operand, i) => {
    const match = findMatchingChild(operand, children, used);
    if (match) {
      used.add(match.index);
      matches[i] = match;
    }
  });

  // Pass 2: same expression kind, in order
  const pending = () =>
    operands
      .map((_, i) => i)
      .filter((i) => matches[i] === null && mayHaveTraceNode(operands[i], templating));

  for (const i of pending()) {
    const kind = expressionKind(operands[i]);
    if (!kind) continue;
    for (let c = 0; c < children.length; c++) {
      if (used.has(c)) continue;
      if (expressionKind(parseTraceExpression(children[c])) === kind) {
        used.add(c);
        matches[i] = { child: children[c], index: c };
        break;
      }
    }
  }

  // Pass 3: positional, only when unambiguous
  const leftover = pending();
  const unusedCount = children.length - used.size;
  if (leftover.length > 0 && leftover.length === unusedCount) {
    for (const i of leftover) {
      const next = getNextUnusedChild(children, used);
      if (!next) break;
      used.add(next.index);
      matches[i] = next;
    }
  }

  return matches;
}

/**
 * Trace children that no operand claimed. Their steps still need a home, so
 * callers fold them into the parent visual node.
 */
export function unmatchedChildren(
  children: ExpressionNode[],
  matches: (ChildMatch | null)[]
): ExpressionNode[] {
  const used = new Set(matches.filter((m): m is ChildMatch => m !== null).map((m) => m.index));
  return children.filter((_, i) => !used.has(i));
}
