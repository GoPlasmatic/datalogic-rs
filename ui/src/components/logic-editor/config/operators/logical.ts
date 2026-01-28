/**
 * Logical Operators
 *
 * Boolean logic operations.
 * - !: Logical NOT
 * - !!: Convert to boolean
 * - and: Logical AND (short-circuit)
 * - or: Logical OR (short-circuit)
 */

import type { Operator } from '../operators.types';

export const logicalOperators: Record<string, Operator> = {
  '!': {
    name: '!',
    label: 'Not',
    category: 'logical',
    description: 'Logical NOT - negates a boolean value',
    arity: {
      type: 'unary',
      min: 1,
      max: 1,
      args: [
        {
          name: 'value',
          label: 'Value',
          type: 'any',
          description: 'Value to negate',
          required: true,
        },
      ],
    },
    help: {
      summary: 'Negates a boolean value',
      details:
        'Returns true if the value is falsy (false, null, 0, empty string), false otherwise. Converts any value to its boolean opposite.',
      returnType: 'boolean',
      examples: [
        {
          title: 'Negate true',
          rule: { '!': [true] },
          result: false,
        },
        {
          title: 'Negate false',
          rule: { '!': [false] },
          result: true,
        },
        {
          title: 'Negate zero',
          rule: { '!': [0] },
          result: true,
          note: '0 is falsy',
        },
        {
          title: 'Negate empty string',
          rule: { '!': [''] },
          result: true,
          note: 'Empty string is falsy',
        },
        {
          title: 'Negate null',
          rule: { '!': [null] },
          result: true,
        },
        {
          title: 'With variable',
          rule: { '!': [{ var: 'isDisabled' }] },
          data: { isDisabled: false },
          result: true,
        },
      ],
      notes: [
        'Falsy values: false, null, 0, "" (empty string)',
        'All other values are considered truthy',
        'Arrays and objects are always truthy (even if empty)',
      ],
      seeAlso: ['!!', 'and', 'or'],
    },
    ui: {
      icon: 'circle-slash',
      shortLabel: '!',
      nodeType: 'operator',
    },
  },

  '!!': {
    name: '!!',
    label: 'To Boolean',
    category: 'logical',
    description: 'Convert any value to boolean',
    arity: {
      type: 'unary',
      min: 1,
      max: 1,
      args: [
        {
          name: 'value',
          label: 'Value',
          type: 'any',
          description: 'Value to convert to boolean',
          required: true,
        },
      ],
    },
    help: {
      summary: 'Convert any value to its boolean equivalent',
      details:
        'Returns false for falsy values (false, null, 0, empty string), true for everything else. Useful for explicit boolean conversion.',
      returnType: 'boolean',
      examples: [
        {
          title: 'Truthy number',
          rule: { '!!': [1] },
          result: true,
        },
        {
          title: 'Zero',
          rule: { '!!': [0] },
          result: false,
        },
        {
          title: 'Non-empty string',
          rule: { '!!': ['hello'] },
          result: true,
        },
        {
          title: 'Empty string',
          rule: { '!!': [''] },
          result: false,
        },
        {
          title: 'Null',
          rule: { '!!': [null] },
          result: false,
        },
        {
          title: 'Empty array',
          rule: { '!!': [[]] },
          result: true,
          note: 'Arrays are always truthy',
        },
      ],
      notes: [
        'Equivalent to double negation: !(!value)',
        'Falsy: false, null, 0, ""',
        'Empty arrays and objects are truthy',
      ],
      seeAlso: ['!', 'and', 'or'],
    },
    ui: {
      icon: 'toggle-right',
      shortLabel: '!!',
      nodeType: 'operator',
    },
  },

  and: {
    name: 'and',
    label: 'And',
    category: 'logical',
    description: 'Logical AND - all conditions must be true',
    arity: {
      type: 'variadic',
      min: 2,
      args: [
        {
          name: 'condition',
          label: 'Condition',
          type: 'any',
          description: 'Condition to evaluate',
          required: true,
          repeatable: true,
        },
      ],
    },
    help: {
      summary: 'Returns true only if all conditions are truthy (short-circuit evaluation)',
      details:
        'Evaluates conditions from left to right. Returns the first falsy value encountered, or the last value if all are truthy. Short-circuits: stops evaluating once a falsy value is found.',
      returnType: 'any',
      examples: [
        {
          title: 'All true',
          rule: { and: [true, true, true] },
          result: true,
        },
        {
          title: 'One false',
          rule: { and: [true, false, true] },
          result: false,
        },
        {
          title: 'With variables',
          rule: { and: [{ var: 'isActive' }, { var: 'isVerified' }] },
          data: { isActive: true, isVerified: true },
          result: true,
        },
        {
          title: 'Returns first falsy',
          rule: { and: [1, 0, 2] },
          result: 0,
          note: 'Returns the falsy value, not false',
        },
        {
          title: 'Returns last truthy',
          rule: { and: [1, 2, 3] },
          result: 3,
          note: 'All truthy, returns last value',
        },
        {
          title: 'Complex conditions',
          rule: {
            and: [
              { '>': [{ var: 'age' }, 18] },
              { '==': [{ var: 'status' }, 'active'] },
            ],
          },
          data: { age: 25, status: 'active' },
          result: true,
        },
      ],
      notes: [
        'Short-circuit evaluation: stops at first falsy value',
        'Returns the actual value, not just true/false',
        'Accepts 2 or more arguments',
        'Empty and is not allowed (minimum 2 args)',
      ],
      seeAlso: ['or', '!', '!!'],
    },
    ui: {
      icon: 'circle-dot',
      shortLabel: 'AND',
      nodeType: 'vertical',
    },
  },

  or: {
    name: 'or',
    label: 'Or',
    category: 'logical',
    description: 'Logical OR - at least one condition must be true',
    arity: {
      type: 'variadic',
      min: 2,
      args: [
        {
          name: 'condition',
          label: 'Condition',
          type: 'any',
          description: 'Condition to evaluate',
          required: true,
          repeatable: true,
        },
      ],
    },
    help: {
      summary: 'Returns true if any condition is truthy (short-circuit evaluation)',
      details:
        'Evaluates conditions from left to right. Returns the first truthy value encountered, or the last value if all are falsy. Short-circuits: stops evaluating once a truthy value is found.',
      returnType: 'any',
      examples: [
        {
          title: 'One true',
          rule: { or: [false, true, false] },
          result: true,
        },
        {
          title: 'All false',
          rule: { or: [false, false, false] },
          result: false,
        },
        {
          title: 'With variables',
          rule: { or: [{ var: 'isPremium' }, { var: 'isTrial' }] },
          data: { isPremium: false, isTrial: true },
          result: true,
        },
        {
          title: 'Returns first truthy',
          rule: { or: [0, '', 'hello'] },
          result: 'hello',
          note: 'Returns the truthy value',
        },
        {
          title: 'Returns last falsy',
          rule: { or: [0, '', null] },
          result: null,
          note: 'All falsy, returns last value',
        },
        {
          title: 'Default value pattern',
          rule: { or: [{ var: 'name' }, 'Anonymous'] },
          data: { name: '' },
          result: 'Anonymous',
          note: 'Common pattern for defaults',
        },
      ],
      notes: [
        'Short-circuit evaluation: stops at first truthy value',
        'Returns the actual value, not just true/false',
        'Useful for default values: {"or": [{"var": "x"}, default]}',
        'Accepts 2 or more arguments',
      ],
      seeAlso: ['and', '!', '!!', '??'],
    },
    ui: {
      icon: 'circle',
      shortLabel: 'OR',
      nodeType: 'vertical',
    },
  },
};
