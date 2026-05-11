/**
 * Comparison Operators
 *
 * Operators for comparing values.
 * - Equality: ==, ===, !=, !==
 * - Relational: >, >=, <, <= (chainable)
 */

import type { Operator } from '../operators.types';

export const comparisonOperators: Record<string, Operator> = {
  '==': {
    name: '==',
    label: 'Equals',
    category: 'comparison',
    description: 'Loose equality comparison (with type coercion)',
    arity: {
      type: 'chainable',
      min: 2,
      args: [
        { name: 'value', label: 'Value', type: 'any', required: true, repeatable: true },
      ],
    },
    help: {
      summary: 'Check if two values are loosely equal (with type coercion)',
      details:
        'Compares two values for equality with type coercion. For example, 1 == "1" is true. Use === for strict equality without type coercion.',
      returnType: 'boolean',
      examples: [
        {
          title: 'Equal numbers',
          rule: { '==': [1, 1] },
          result: true,
        },
        {
          title: 'Type coercion',
          rule: { '==': [1, '1'] },
          result: true,
          note: 'Number and string compared with coercion',
        },
        {
          title: 'With variable',
          rule: { '==': [{ var: 'status' }, 'active'] },
          data: { status: 'active' },
          result: true,
        },
        {
          title: 'Null comparison',
          rule: { '==': [null, null] },
          result: true,
        },
      ],
      notes: [
        'Type coercion is applied (1 == "1" is true)',
        'Use === for strict equality without coercion',
        'null == null is true',
      ],
      seeAlso: ['===', '!=', '!=='],
    },
    ui: {
      icon: 'equal',
      shortLabel: '==',
      nodeType: 'vertical',
      addArgumentLabel: 'Add Comparison',
    },
  },

  '===': {
    name: '===',
    label: 'Strict Equals',
    category: 'comparison',
    description: 'Strict equality comparison (no type coercion)',
    arity: {
      type: 'chainable',
      min: 2,
      args: [
        { name: 'value', label: 'Value', type: 'any', required: true, repeatable: true },
      ],
    },
    help: {
      summary: 'Check if two values are strictly equal (same type and value)',
      details:
        'Compares two values for equality without type coercion. Both type and value must match. For example, 1 === "1" is false.',
      returnType: 'boolean',
      examples: [
        {
          title: 'Equal numbers',
          rule: { '===': [1, 1] },
          result: true,
        },
        {
          title: 'Different types',
          rule: { '===': [1, '1'] },
          result: false,
          note: 'Number and string are different types',
        },
        {
          title: 'Equal strings',
          rule: { '===': ['hello', 'hello'] },
          result: true,
        },
      ],
      notes: [
        'No type coercion (1 === "1" is false)',
        'Both type and value must match',
        'Preferred for type-safe comparisons',
      ],
      seeAlso: ['==', '!==', '!='],
    },
    ui: {
      icon: 'equal',
      shortLabel: '===',
      nodeType: 'vertical',
      addArgumentLabel: 'Add Comparison',
    },
  },

  '!=': {
    name: '!=',
    label: 'Not Equals',
    category: 'comparison',
    description: 'Loose inequality comparison (with type coercion)',
    arity: {
      type: 'chainable',
      min: 2,
      args: [
        { name: 'value', label: 'Value', type: 'any', required: true, repeatable: true },
      ],
    },
    help: {
      summary: 'Check if two values are not loosely equal',
      details: 'Returns true if values are not equal after type coercion. Opposite of ==.',
      returnType: 'boolean',
      examples: [
        {
          title: 'Different values',
          rule: { '!=': [1, 2] },
          result: true,
        },
        {
          title: 'Same value different type',
          rule: { '!=': [1, '1'] },
          result: false,
          note: 'Type coercion makes them equal',
        },
        {
          title: 'With variable',
          rule: { '!=': [{ var: 'status' }, 'inactive'] },
          data: { status: 'active' },
          result: true,
        },
      ],
      notes: ['Opposite of ==', 'Type coercion is applied'],
      seeAlso: ['!==', '==', '==='],
    },
    ui: {
      icon: 'equal-not',
      shortLabel: '!=',
      nodeType: 'vertical',
      addArgumentLabel: 'Add Comparison',
    },
  },

  '!==': {
    name: '!==',
    label: 'Strict Not Equals',
    category: 'comparison',
    description: 'Strict inequality comparison (no type coercion)',
    arity: {
      type: 'chainable',
      min: 2,
      args: [
        { name: 'value', label: 'Value', type: 'any', required: true, repeatable: true },
      ],
    },
    help: {
      summary: 'Check if two values are not strictly equal',
      details:
        'Returns true if values differ in type or value. No type coercion. Opposite of ===.',
      returnType: 'boolean',
      examples: [
        {
          title: 'Different types',
          rule: { '!==': [1, '1'] },
          result: true,
          note: 'Different types are not strictly equal',
        },
        {
          title: 'Same value and type',
          rule: { '!==': [1, 1] },
          result: false,
        },
      ],
      notes: ['Opposite of ===', 'No type coercion'],
      seeAlso: ['!=', '===', '=='],
    },
    ui: {
      icon: 'equal-not',
      shortLabel: '!==',
      nodeType: 'vertical',
      addArgumentLabel: 'Add Comparison',
    },
  },

  '>': {
    name: '>',
    label: 'Greater Than',
    category: 'comparison',
    description: 'Check if values are in descending order (chainable)',
    arity: {
      type: 'chainable',
      min: 2,
      args: [
        { name: 'value', label: 'Value', type: 'any', required: true, repeatable: true },
      ],
    },
    help: {
      summary: 'Check if values are in strictly descending order',
      details:
        'With 2 arguments, checks if left > right. With 3+ arguments, checks if all values are in strictly descending order (chained comparison). For example, {">": [10, 5, 1]} checks if 10 > 5 > 1.',
      returnType: 'boolean',
      examples: [
        {
          title: 'Simple comparison',
          rule: { '>': [5, 3] },
          result: true,
        },
        {
          title: 'With variable',
          rule: { '>': [{ var: 'age' }, 18] },
          data: { age: 21 },
          result: true,
        },
        {
          title: 'Chained comparison',
          rule: { '>': [10, { var: 'x' }, 1] },
          data: { x: 5 },
          result: true,
          note: 'Checks: 10 > 5 > 1',
        },
        {
          title: 'Chained - false',
          rule: { '>': [10, { var: 'x' }, 1] },
          data: { x: 15 },
          result: false,
          note: '10 is not > 15',
        },
      ],
      notes: [
        'Supports 2+ arguments for chained comparisons',
        'All comparisons must be true for result to be true',
        'Useful for range checks: {">": [max, x, min]}',
      ],
      seeAlso: ['>=', '<', '<='],
    },
    ui: {
      icon: 'chevron-right',
      shortLabel: '>',
      nodeType: 'vertical',
      addArgumentLabel: 'Add Comparison',
    },
    panel: {
      chainable: true,
      sections: [
        {
          id: 'args',
          fields: [
            {
              id: 'values',
              label: 'Values',
              inputType: 'expression',
              repeatable: true,
              required: true,
              helpText: 'Values to compare in descending order (a > b > c)',
            },
          ],
        },
      ],
    },
  },

  '>=': {
    name: '>=',
    label: 'Greater Or Equal',
    category: 'comparison',
    description: 'Check if values are in non-ascending order (chainable)',
    arity: {
      type: 'chainable',
      min: 2,
      args: [
        { name: 'value', label: 'Value', type: 'any', required: true, repeatable: true },
      ],
    },
    help: {
      summary: 'Check if values are in non-ascending order (greater than or equal)',
      details:
        'With 2 arguments, checks if left >= right. With 3+ arguments, checks chained comparison.',
      returnType: 'boolean',
      examples: [
        {
          title: 'Greater than',
          rule: { '>=': [5, 3] },
          result: true,
        },
        {
          title: 'Equal',
          rule: { '>=': [5, 5] },
          result: true,
        },
        {
          title: 'Chained',
          rule: { '>=': [10, { var: 'x' }, 0] },
          data: { x: 5 },
          result: true,
        },
      ],
      notes: ['Supports 2+ arguments for chained comparisons', 'Equal values return true'],
      seeAlso: ['>', '<', '<='],
    },
    ui: {
      icon: 'chevron-right',
      shortLabel: '>=',
      nodeType: 'vertical',
      addArgumentLabel: 'Add Comparison',
    },
    panel: {
      chainable: true,
      sections: [
        {
          id: 'args',
          fields: [
            {
              id: 'values',
              label: 'Values',
              inputType: 'expression',
              repeatable: true,
              required: true,
              helpText: 'Values to compare in non-ascending order (a >= b >= c)',
            },
          ],
        },
      ],
    },
  },

  '<': {
    name: '<',
    label: 'Less Than',
    category: 'comparison',
    description: 'Check if values are in ascending order (chainable)',
    arity: {
      type: 'chainable',
      min: 2,
      args: [
        { name: 'value', label: 'Value', type: 'any', required: true, repeatable: true },
      ],
    },
    help: {
      summary: 'Check if values are in strictly ascending order',
      details:
        'With 2 arguments, checks if left < right. With 3+ arguments, checks if all values are in strictly ascending order (chained comparison). For example, {"<": [1, 5, 10]} checks if 1 < 5 < 10.',
      returnType: 'boolean',
      examples: [
        {
          title: 'Simple comparison',
          rule: { '<': [3, 5] },
          result: true,
        },
        {
          title: 'With variable',
          rule: { '<': [{ var: 'age' }, 18] },
          data: { age: 16 },
          result: true,
        },
        {
          title: 'Chained (between)',
          rule: { '<': [0, { var: 'score' }, 100] },
          data: { score: 75 },
          result: true,
          note: 'Checks: 0 < 75 < 100',
        },
        {
          title: 'Chained - false',
          rule: { '<': [0, { var: 'score' }, 100] },
          data: { score: 150 },
          result: false,
          note: '150 is not < 100',
        },
      ],
      notes: [
        'Supports 2+ arguments for chained comparisons',
        'All comparisons must be true for result to be true',
        'Useful for range checks: {"<": [min, x, max]}',
      ],
      seeAlso: ['<=', '>', '>='],
    },
    ui: {
      icon: 'chevron-left',
      shortLabel: '<',
      nodeType: 'vertical',
      addArgumentLabel: 'Add Comparison',
    },
    panel: {
      chainable: true,
      sections: [
        {
          id: 'args',
          fields: [
            {
              id: 'values',
              label: 'Values',
              inputType: 'expression',
              repeatable: true,
              required: true,
              helpText: 'Values to compare in ascending order (a < b < c)',
            },
          ],
        },
      ],
    },
  },

  '<=': {
    name: '<=',
    label: 'Less Or Equal',
    category: 'comparison',
    description: 'Check if values are in non-descending order (chainable)',
    arity: {
      type: 'chainable',
      min: 2,
      args: [
        { name: 'value', label: 'Value', type: 'any', required: true, repeatable: true },
      ],
    },
    help: {
      summary: 'Check if values are in non-descending order (less than or equal)',
      details:
        'With 2 arguments, checks if left <= right. With 3+ arguments, checks chained comparison.',
      returnType: 'boolean',
      examples: [
        {
          title: 'Less than',
          rule: { '<=': [3, 5] },
          result: true,
        },
        {
          title: 'Equal',
          rule: { '<=': [5, 5] },
          result: true,
        },
        {
          title: 'Chained',
          rule: { '<=': [0, { var: 'x' }, 100] },
          data: { x: 50 },
          result: true,
        },
      ],
      notes: ['Supports 2+ arguments for chained comparisons', 'Equal values return true'],
      seeAlso: ['<', '>', '>='],
    },
    ui: {
      icon: 'chevron-left',
      shortLabel: '<=',
      nodeType: 'vertical',
      addArgumentLabel: 'Add Comparison',
    },
    panel: {
      chainable: true,
      sections: [
        {
          id: 'args',
          fields: [
            {
              id: 'values',
              label: 'Values',
              inputType: 'expression',
              repeatable: true,
              required: true,
              helpText: 'Values to compare in non-descending order (a <= b <= c)',
            },
          ],
        },
      ],
    },
  },
};
