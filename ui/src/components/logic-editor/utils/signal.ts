/**
 * Signal mapping — the Signal Board colour axis.
 *
 * A node is coloured by the *type of value it produces* (its signal), not by
 * its operator category. This maps an operator's declared `returnType`
 * (from the operator config) — or a literal's value type — to a signal key,
 * which resolves to a `--sig-*` CSS variable defined in theme.css.
 *
 * Consumers override the palette by redefining `--sig-*` on `.logic-editor`.
 */

import { getOperator } from '../config/operators';
import type { OperatorCategory } from '../config/operators.types';
import type { LiteralNodeData } from '../types';

export type SignalKey =
  | 'bool-true'
  | 'bool-false'
  | 'bool-rest'
  | 'number'
  | 'string'
  | 'collection'
  | 'data'
  | 'temporal'
  | 'null';

// operator returnType -> signal
const RETURN_TYPE_SIGNAL: Record<string, SignalKey> = {
  boolean: 'bool-rest', // resting boolean; debug state colours true/false
  number: 'number',
  string: 'string',
  array: 'collection',
  object: 'collection',
  null: 'null',
  datetime: 'temporal',
  duration: 'temporal',
  'number | string': 'number',
};

/** Signal for an operator node, from its returnType (falling back to category). */
export function signalForOperator(
  operator: string,
  category?: OperatorCategory,
): SignalKey {
  // var / val / exists read the data context — always the teal data tap.
  if (category === 'variable') return 'data';

  const rt = getOperator(operator)?.help?.returnType;
  if (rt && RETURN_TYPE_SIGNAL[rt]) return RETURN_TYPE_SIGNAL[rt];

  // Generic returnTypes ('any' | 'same' | 'never') fall back to the category.
  if (category === 'datetime') return 'temporal';
  if (category === 'error') return 'bool-false';
  return 'bool-rest';
}

/** Signal for a literal, from its value type. */
export function signalForLiteral(valueType: LiteralNodeData['valueType']): SignalKey {
  switch (valueType) {
    case 'number': return 'number';
    case 'string': return 'string';
    case 'boolean': return 'bool-true';
    case 'array': return 'collection';
    case 'null': return 'null';
    default: return 'string';
  }
}

/** The `--sig-*` CSS variable reference for a signal key. */
export function signalVar(key: SignalKey): string {
  return `var(--sig-${key})`;
}
