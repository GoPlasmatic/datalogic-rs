import type { JsonLogicValue } from '../../types';
import { isPlainObject } from '../type-helpers';

// Operator categories for expression formatting
const COMPARISON_OPERATORS = ['==', '===', '!=', '!==', '>', '>=', '<', '<='];
const ITERATOR_OPERATORS = ['map', 'reduce', 'filter', 'some', 'none', 'all'];
const UNARY_OPERATORS = ['!', '!!'];
const ARITHMETIC_BINARY_OPERATORS = ['+', '-', '*', '/', '%'];

// Generate full expression text for a JSONLogic value (for collapsed view)
export function generateExpressionText(value: JsonLogicValue, maxLength = 100): string {
  // Get the operator of a value (if it's an operator expression)
  function getOperator(val: JsonLogicValue): string | null {
    if (isPlainObject(val)) {
      const keys = Object.keys(val);
      if (keys.length === 1) return keys[0];
    }
    return null;
  }

  // Check if a sub-expression needs parentheses when used inside a parent operator
  function needsParens(subVal: JsonLogicValue, parentOp: string): boolean {
    const subOp = getOperator(subVal);
    if (!subOp) return false;

    // AND inside OR or OR inside AND needs parens
    if ((parentOp === 'and' && subOp === 'or') || (parentOp === 'or' && subOp === 'and')) {
      return true;
    }
    // Logical inside comparison needs parens
    if (COMPARISON_OPERATORS.includes(parentOp) && (subOp === 'and' || subOp === 'or')) {
      return true;
    }
    // Lower precedence arithmetic
    if ((parentOp === '*' || parentOp === '/') && (subOp === '+' || subOp === '-')) {
      return true;
    }
    return false;
  }

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  function toText(val: JsonLogicValue, _parentOp?: string): string {
    if (val === null) return 'null';
    if (typeof val === 'boolean') return String(val);
    if (typeof val === 'number') return String(val);
    if (typeof val === 'string') return `"${val}"`;

    if (Array.isArray(val)) {
      if (val.length === 0) return '[]';
      const items = val.map(v => toText(v)).join(', ');
      return `[${items}]`;
    }

    if (isPlainObject(val)) {
      const keys = Object.keys(val);
      if (keys.length !== 1) return JSON.stringify(val);

      const op = keys[0];
      const operands = val[op];

      // Variable access
      if (op === 'var') {
        const path = Array.isArray(operands) ? operands[0] : operands;
        return String(path ?? '');
      }
      if (op === 'val') {
        const path = Array.isArray(operands) ? operands[0] : operands;
        return `val(${path ?? ''})`;
      }
      if (op === 'exists') {
        return `exists(${operands})`;
      }

      const args: JsonLogicValue[] = Array.isArray(operands) ? operands : [operands];

      // Helper to wrap sub-expression in parens if needed
      const wrapIfNeeded = (subVal: JsonLogicValue): string => {
        const text = toText(subVal, op);
        return needsParens(subVal, op) ? `(${text})` : text;
      };

      // Comparison and arithmetic: infix notation
      if ([...COMPARISON_OPERATORS, ...ARITHMETIC_BINARY_OPERATORS].includes(op)) {
        if (args.length === 2) {
          return `${wrapIfNeeded(args[0])} ${op} ${wrapIfNeeded(args[1])}`;
        }
        // Chained comparison: a < b < c
        if (args.length > 2) {
          return args.map(wrapIfNeeded).join(` ${op} `);
        }
      }

      // Logical operators
      if (op === 'and') {
        return args.map(wrapIfNeeded).join(' AND ');
      }
      if (op === 'or') {
        return args.map(wrapIfNeeded).join(' OR ');
      }

      // Unary operators
      if (UNARY_OPERATORS.includes(op)) {
        const argText = toText(args[0], op);
        const subOp = getOperator(args[0]);
        // Wrap if the argument is a complex expression
        if (subOp && (subOp === 'and' || subOp === 'or' || COMPARISON_OPERATORS.includes(subOp))) {
          return `${op}(${argText})`;
        }
        return `${op}${argText}`;
      }

      // Iterator operators
      if (ITERATOR_OPERATORS.includes(op)) {
        const [arr] = args;
        return `${op}(${toText(arr)}, ...)`;
      }

      // If/else - handle full if-elseif-else chains
      if (op === 'if' || op === '?:') {
        const parts: string[] = [];
        let i = 0;
        while (i < args.length) {
          if (i + 1 < args.length) {
            // condition-then pair
            const prefix = i === 0 ? 'if' : 'else if';
            parts.push(`${prefix} ${toText(args[i])} then ${toText(args[i + 1])}`);
            i += 2;
          } else {
            // final else value
            parts.push(`else ${toText(args[i])}`);
            i++;
          }
        }
        return parts.join(' ');
      }

      // Switch/match - discriminant with case/result pairs
      if (op === 'switch' || op === 'match') {
        const parts: string[] = [`${op}(${toText(args[0])})`];
        if (args.length >= 2 && Array.isArray(args[1])) {
          const cases = args[1] as JsonLogicValue[];
          for (const c of cases) {
            if (Array.isArray(c) && c.length >= 2) {
              parts.push(`${toText(c[0])}: ${toText(c[1])}`);
            }
          }
        }
        if (args.length >= 3) {
          parts.push(`default: ${toText(args[2])}`);
        }
        return parts.join(', ');
      }

      // Default: function notation
      return `${op}(${args.map(a => toText(a)).join(', ')})`;
    }

    return String(val);
  }

  const text = toText(value);
  if (text.length > maxLength) {
    return text.slice(0, maxLength - 3) + '...';
  }
  return text;
}
