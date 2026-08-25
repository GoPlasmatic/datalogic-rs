/**
 * Structure path helpers
 *
 * A structure (template) node keeps its full JSON value in `data.expression`
 * and records where each embedded expression lives as a path of segments
 * (object keys or array indices as strings). These helpers read, write and
 * remove values at such paths, and regenerate the placeholder-formatted JSON
 * the StructureNode renders.
 */

import type { JsonLogicValue, StructureElement } from '../../types';

// Placeholder marker used in formatted JSON for expressions
export const EXPR_PLACEHOLDER = '{{EXPR}}';
// The placeholder as it appears in JSON.stringify output (with quotes)
export const EXPR_PLACEHOLDER_QUOTED = `"${EXPR_PLACEHOLDER}"`;

export function cloneJson<T>(value: T): T {
  return value === undefined ? value : (JSON.parse(JSON.stringify(value)) as T);
}

function isContainer(value: unknown): value is Record<string, unknown> | unknown[] {
  return typeof value === 'object' && value !== null;
}

/** Read the value at `path` (undefined when any segment is missing). */
export function getAtPath(root: unknown, path: string[]): unknown {
  let current: unknown = root;
  for (const segment of path) {
    if (!isContainer(current)) return undefined;
    current = Array.isArray(current) ? current[Number(segment)] : current[segment];
  }
  return current;
}

/**
 * Set the value at `path`, creating intermediate containers as needed
 * (arrays for numeric segments, objects otherwise). Mutates `root` and
 * returns it; a root replaced wholesale (empty path) is returned instead.
 */
export function setAtPath(root: JsonLogicValue, path: string[], value: JsonLogicValue): JsonLogicValue {
  if (path.length === 0) return value;
  let current: Record<string, unknown> | unknown[] = isContainer(root)
    ? root
    : /^\d+$/.test(path[0]) ? [] : {};
  const result = current as JsonLogicValue;

  for (let i = 0; i < path.length; i++) {
    const segment = path[i];
    const isLast = i === path.length - 1;
    if (Array.isArray(current)) {
      const index = Number(segment);
      if (isLast) {
        current[index] = value;
      } else {
        if (!isContainer(current[index])) {
          current[index] = /^\d+$/.test(path[i + 1]) ? [] : {};
        }
        current = current[index] as Record<string, unknown> | unknown[];
      }
    } else {
      if (isLast) {
        current[segment] = value;
      } else {
        if (!isContainer(current[segment])) {
          current[segment] = /^\d+$/.test(path[i + 1]) ? [] : {};
        }
        current = current[segment] as Record<string, unknown> | unknown[];
      }
    }
  }
  return result;
}

/** Remove the value at `path` (array entries are spliced out). Mutates `root`. */
export function removeAtPath(root: JsonLogicValue, path: string[]): void {
  if (path.length === 0) return;
  const parent = getAtPath(root, path.slice(0, -1));
  const last = path[path.length - 1];
  if (Array.isArray(parent)) {
    const index = Number(last);
    if (index >= 0 && index < parent.length) parent.splice(index, 1);
  } else if (isContainer(parent)) {
    delete (parent as Record<string, unknown>)[last];
  }
}

/**
 * After removing the entry at `removedPath` from an array, later siblings in
 * that array (and everything nested under them) shift down by one index.
 * Returns the adjusted path for `path`.
 */
export function shiftPathAfterRemoval(path: string[], removedPath: string[], root: JsonLogicValue): string[] {
  const depth = removedPath.length - 1;
  if (depth < 0 || path.length <= depth) return path;
  for (let i = 0; i < depth; i++) {
    if (path[i] !== removedPath[i]) return path;
  }
  const container = getAtPath(root, removedPath.slice(0, depth));
  if (!Array.isArray(container)) return path;
  const removedIndex = Number(removedPath[depth]);
  const siblingIndex = Number(path[depth]);
  if (!Number.isInteger(siblingIndex) || siblingIndex <= removedIndex) return path;
  const shifted = [...path];
  shifted[depth] = String(siblingIndex - 1);
  return shifted;
}

/** Character span of one expression placeholder in the formatted JSON. */
export interface ExprSpan {
  startOffset: number;
  endOffset: number;
}

/**
 * A marker factory whose tokens cannot occur anywhere inside `source`.
 * Marking each expression slot with a distinct token (rather than one
 * shared `{{EXPR}}`) is what makes the offset search exact: a literal
 * `"{{EXPR}}"` in the user's own data can no longer be mistaken for a
 * placeholder, and slots no longer have to be listed in document order.
 */
export function exprMarkerFactory(source: JsonLogicValue): (slot: number) => string {
  const serialized = JSON.stringify(source) ?? '';
  let body = 'EXPR';
  while (serialized.includes(`{{${body}`)) body += '_';
  return (slot) => `{{${body}:${slot}}}`;
}

/**
 * Swap each unique marker in `marked` back to the canonical `"{{EXPR}}"`,
 * returning the final JSON plus the span each slot ended up occupying.
 * `spans[i]` is null when `markers[i]` is not present (an element whose
 * path no longer resolves).
 */
export function resolveExprMarkers(
  marked: string,
  markers: string[]
): { formattedJson: string; spans: (ExprSpan | null)[] } {
  const hits = markers
    .map((marker, slot) => {
      const quoted = JSON.stringify(marker);
      return { slot, quoted, pos: marked.indexOf(quoted) };
    })
    .filter((hit) => hit.pos !== -1)
    .sort((a, b) => a.pos - b.pos);

  const spans: (ExprSpan | null)[] = markers.map(() => null);
  let formattedJson = '';
  let cursor = 0;
  for (const hit of hits) {
    formattedJson += marked.slice(cursor, hit.pos);
    const startOffset = formattedJson.length;
    formattedJson += EXPR_PLACEHOLDER_QUOTED;
    spans[hit.slot] = { startOffset, endOffset: formattedJson.length };
    cursor = hit.pos + hit.quoted.length;
  }
  formattedJson += marked.slice(cursor);

  return { formattedJson, spans };
}

/**
 * Produce the pretty-printed JSON with a placeholder at every expression
 * element's path, and return the elements with their character offsets set.
 */
export function formatStructureWithPlaceholders(
  expression: JsonLogicValue,
  elements: StructureElement[]
): { formattedJson: string; elements: StructureElement[] } {
  const marker = exprMarkerFactory(expression);
  const markers: string[] = [];
  const slotOf = new Map<number, number>();

  let withPlaceholders = cloneJson(expression);
  elements.forEach((element, i) => {
    if (element.type !== 'expression') return;
    const slot = markers.length;
    slotOf.set(i, slot);
    markers.push(marker(slot));
    withPlaceholders = setAtPath(withPlaceholders, element.path, markers[slot]);
  });

  const { formattedJson, spans } = resolveExprMarkers(
    JSON.stringify(withPlaceholders, null, 2),
    markers
  );

  const updated = elements.map((element, i) => {
    const slot = slotOf.get(i);
    const span = slot === undefined ? null : spans[slot];
    return span ? { ...element, ...span } : element;
  });

  return { formattedJson, elements: updated };
}
