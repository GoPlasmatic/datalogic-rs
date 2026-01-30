import type { JsonLogicValue, LiteralNodeData } from '../types';
import { isPlainObject, looksLikeDate } from './type-helpers';

// Icon name type for all available icons
export type IconName =
  | 'scale'
  | 'diamond'
  | 'calculator'
  | 'repeat'
  | 'type'
  | 'box'
  | 'git-merge'
  | 'git-branch'
  | 'text'
  | 'hash'
  | 'toggle-left'
  | 'toggle-right'
  | 'check'
  | 'x'
  | 'ban'
  | 'list'
  | 'calendar'
  | 'cog'
  | 'database'
  | 'boxes'
  | 'circle-help'
  | 'circle-x'
  | 'git-commit-horizontal'
  | 'search'
  | 'divide'
  | 'quote'
  | 'braces'
  | 'binary'
  | 'layers'
  | 'clock'
  | 'alert-circle'
  | 'arrow-up';

// Iterator argument icons
export const ITERATOR_ARG_ICONS: Record<string, IconName[]> = {
  map: ['database', 'cog'],
  reduce: ['database', 'cog', 'boxes'],
  filter: ['database', 'cog'],
  some: ['database', 'cog'],
  none: ['database', 'cog'],
  all: ['database', 'cog'],
};

// Type icons for arg display
export const TYPE_ICONS: Record<string, IconName> = {
  string: 'text',
  number: 'hash',
  boolean: 'toggle-left',
  boolean_true: 'check',
  boolean_false: 'x',
  null: 'ban',
  array: 'list',
  date: 'calendar',
  variable: 'box',
  expression: 'cog',
};

// Literal type icons (for LiteralNode display)
export const LITERAL_TYPE_ICONS: Record<LiteralNodeData['valueType'], IconName> = {
  string: 'quote',
  number: 'hash',
  boolean: 'toggle-right',
  null: 'ban',
  array: 'list',
};

// Control flow icons
export const CONTROL_ICONS = {
  ifCondition: 'git-merge' as IconName,
  elseClause: 'git-commit-horizontal' as IconName,
  ifThenElse: 'git-merge' as IconName,
  orOperator: 'diamond' as IconName,
};

// Variable operator icons
export const VARIABLE_ICONS = {
  var: 'box' as IconName,
  val: 'database' as IconName,
  exists: 'search' as IconName,
};

// Get type icon for an operand
export function getOperandTypeIcon(operand: JsonLogicValue): IconName {
  if (operand === null) return TYPE_ICONS.null;
  if (typeof operand === 'boolean') return TYPE_ICONS.boolean;
  if (typeof operand === 'number') return TYPE_ICONS.number;
  if (typeof operand === 'string') {
    if (looksLikeDate(operand)) return TYPE_ICONS.date;
    return TYPE_ICONS.string;
  }
  if (Array.isArray(operand)) return TYPE_ICONS.array;

  if (isPlainObject(operand)) {
    const keys = Object.keys(operand);
    if (keys.length === 1) {
      const op = keys[0];
      if (op === 'var' || op === 'val' || op === 'exists') {
        return TYPE_ICONS.variable;
      }
    }
    return TYPE_ICONS.expression;
  }

  return TYPE_ICONS.expression;
}

// Re-export the Icon component from its own file
export { Icon } from './Icon';
