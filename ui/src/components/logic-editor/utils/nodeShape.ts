/**
 * Node shape vocabulary — "shape = role" from the Signal Board design.
 * Each operator maps to a silhouette so its role is readable at a glance,
 * independent of the signal colour (which encodes its output type).
 */
import { getOperator } from '../config/operators';
import type { OperatorCategory } from '../config/operators.types';
import type { CellData, OperatorNodeData } from '../types';

export type NodeShape =
  | 'tap'        // var / val / exists — a plug into the data
  | 'gate'       // comparison / and / or / not — boolean gate with an LED
  | 'arith'      // + - * / % max min ... — a compute node
  | 'branch'     // if / switch / match — a decision
  | 'iterator'   // map / filter / reduce — produces a collection
  | 'quantifier' // all / some / none — iterates, returns a boolean
  | 'value';     // cat / substr / merge / datetime / ... — a typed producer

const QUANTIFIERS = new Set(['all', 'some', 'none']);

export function shapeForOperator(operator: string, category?: OperatorCategory): NodeShape {
  if (category === 'variable') return 'tap';
  const nt = getOperator(operator)?.ui?.nodeType;
  if (nt === 'decision') return 'branch';
  if (nt === 'iterator' || category === 'array') {
    if (QUANTIFIERS.has(operator)) return 'quantifier';
    if (nt === 'iterator') return 'iterator';
  }
  if (category === 'comparison' || category === 'logical') return 'gate';
  if (category === 'arithmetic') return 'arith';
  return 'value';
}

/** Whether a shape produces a boolean and should carry a truth LED. */
export function shapeHasLed(shape: NodeShape): boolean {
  return shape === 'gate' || shape === 'quantifier';
}

// Display glyphs — prefer real math symbols over verbose labels.
const GLYPHS: Record<string, string> = {
  '>=': '≥', '<=': '≤', '>': '>', '<': '<',
  '==': '=', '===': '≡', '!=': '≠', '!==': '≢',
  and: '∧', or: '∨', '!': '¬', '!!': '!!', in: '∈',
  '+': '+', '-': '−', '*': '×', '/': '÷', '%': '%',
  '??': '??', map: '⟳', filter: '⟳', reduce: '⟳',
  all: '∀', some: '∃', none: '∄',
};

/** The compact glyph to show for an operator (math symbol or shortLabel). */
export function glyphForOperator(operator: string): string {
  if (GLYPHS[operator]) return GLYPHS[operator];
  return getOperator(operator)?.ui?.shortLabel ?? operator;
}

export type PillType = 'tap' | 'num' | 'str' | 'bool' | 'coll' | 'temp' | 'nul' | 'expr';

const NUMBER_RE = /^-?\d/;
const QUOTED_RE = /^["'`]/;
const COLLECTION_RE = /^[[{]/;
const IDENT_RE = /^[A-Za-z_$][\w.$-]*$/; // bare identifier / dotted path

/** Infer the pill (signal) type of an operand from its displayed text. */
export function pillTypeForText(text: string | undefined): PillType {
  if (text == null) return 'expr';
  const t = text.trim();
  if (t === '') return 'str';
  if (t === 'null' || t === 'undefined') return 'nul';
  if (t === 'true' || t === 'false') return 'bool';
  if (NUMBER_RE.test(t)) return 'num';
  if (QUOTED_RE.test(t)) return 'str';
  if (COLLECTION_RE.test(t)) return 'coll';
  if (t.startsWith('↑')) return 'tap'; // val scope-jump read (e.g. "↑threshold")
  if (IDENT_RE.test(t)) return 'tap'; // reads from context -> a data tap
  return 'expr';
}

/** Pill type for a wired child's output, from its ArgSummary `valueType`. */
export function pillForValueType(valueType: string | undefined): PillType {
  switch (valueType) {
    case 'number': return 'num';
    case 'boolean': return 'bool';
    case 'null': return 'nul';
    case 'array': return 'coll';
    case 'date': return 'temp';
    case 'string': return 'str';
    default: return 'expr';
  }
}

// ---------------------------------------------------------------------------
// Render-kind classification — the single source of truth for how an operator
// node renders (tap plug / infix chip / stacked card). Both the renderer
// (UnifiedOperatorNode) and the layout sizer (utils/layout.ts) consume this, so
// a node's reserved footprint always matches what is actually drawn.
// ---------------------------------------------------------------------------

/** A cell that wires out to its own child node + edge (vs. an inline value). */
export function isWiredCell(cell: CellData): boolean {
  return cell.type === 'branch' || !!cell.branchId;
}

/** Whether any cell hosts a computed sub-expression (its own handle). */
export function hasChildCell(cells: CellData[]): boolean {
  return cells.some(
    (c) => c.type === 'branch' || c.branchId || c.conditionBranchId || c.thenBranchId,
  );
}

export type RenderKind = 'tap' | 'infix-gate' | 'infix-arith' | 'card' | 'gate-shape' | 'decision';

/** How an operator node renders — see the branches in UnifiedOperatorNode. */
export function operatorRenderKind(
  data: Pick<OperatorNodeData, 'operator' | 'category' | 'cells'>,
): RenderKind {
  // var / val / exists with a plain path render as a compact data tap; a
  // computed path needs the full card (the plug can't host a child handle).
  if (data.category === 'variable' && !hasChildCell(data.cells)) return 'tap';

  // if / else-if — each condition is its own decision diamond (when/then/else).
  if (data.operator === 'if') return 'decision';

  // Boolean AND / OR / NOT with EVERY operand wired to a child renders as a real
  // logic-gate silhouette (design-system gate shapes) — input ports on the left,
  // no repeated logic. With any inline operand it stays a card / infix chip.
  if (data.category === 'logical' && data.cells.length > 0 && data.cells.every(isWiredCell)) {
    return 'gate-shape';
  }

  // Compare / arithmetic / unary-logical (!, !!) render as an infix chip — but
  // only when EVERY operand is inline. As soon as one operand extends to a child
  // node, we fall through to the stacked card so the connectors line up vertically.
  const infix: RenderKind | null =
    data.category === 'arithmetic'
      ? 'infix-arith'
      : data.category === 'comparison'
        ? 'infix-gate'
        : data.category === 'logical' && getOperator(data.operator)?.arity.type === 'unary'
          ? 'infix-gate'
          : null;
  if (infix && data.cells.length > 0 && !data.cells.some(isWiredCell)) {
    return infix;
  }
  return 'card';
}

/** Display text of an operand cell (summary for wired, value/label for inline). */
export function cellDisplayText(cell: CellData): string {
  return isWiredCell(cell)
    ? cell.summary?.label ?? cell.label ?? '…'
    : cell.label ?? String(cell.value ?? '…');
}
