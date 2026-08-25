/**
 * Utility Operators
 *
 * General-purpose utility operations.
 * - type: Get the type of a value
 *
 * Note: the v4 `preserve` operator was removed in v5: literal scalars and
 * arrays pass through inline already, and templated objects are handled by
 * templating mode (the toolbar toggle), not by an operator.
 */

import type { Operator } from '../operators.types';

export const utilityOperators: Record<string, Operator> = {
  type: {
    name: 'type',
    label: 'Type',
    category: 'utility',
    description: 'Get the type of a value',
    arity: {
      type: 'unary',
      min: 1,
      max: 1,
      args: [
        {
          name: 'value',
          label: 'Value',
          type: 'any',
          required: true,
          description: 'Value to check type of',
        },
      ],
    },
    help: {
      summary: 'Returns a string indicating the type of a value',
      details:
        'Inspects a value and returns its type as a string. Useful for conditional logic based on data types. Includes special detection for datetime and duration strings.',
      returnType: 'string',
      examples: [
        {
          title: 'Number type',
          rule: { type: 42 },
          result: 'number',
        },
        {
          title: 'String type',
          rule: { type: 'hello' },
          result: 'string',
        },
        {
          title: 'Boolean type',
          rule: { type: true },
          result: 'boolean',
        },
        {
          title: 'Null type',
          rule: { type: null },
          result: 'null',
        },
        {
          title: 'Array type',
          rule: { type: [[1, 2, 3]] },
          result: 'array',
          note: 'Wrap a literal array: the outer array is the argument list',
        },
        {
          title: 'Object type',
          rule: { type: { var: 'obj' } },
          data: { obj: { key: 'value' } },
          result: 'object',
        },
        {
          title: 'Object literal (templating mode)',
          rule: { type: { key: 'value' } },
          templating: true,
          result: 'object',
          note: 'Multi-key object literals only parse in templating mode',
        },
        {
          title: 'Datetime detection',
          rule: { type: '2024-01-15T10:30:00Z' },
          result: 'datetime',
          note: 'ISO 8601 strings detected as datetime',
        },
        {
          title: 'Duration detection',
          rule: { type: '2h30m' },
          result: 'duration',
          note: 'Duration strings detected automatically',
        },
        {
          title: 'With variable',
          rule: { type: { var: 'value' } },
          data: { value: [1, 2, 3] },
          result: 'array',
        },
      ],
      notes: [
        'Returns: "null", "boolean", "number", "string", "array", "object", "datetime", "duration"',
        'Datetime: detected by ISO 8601 format (contains T, :, and Z or +)',
        'Duration heuristic: no spaces, at least one digit and one of d/h/m/s (can misclassify e.g. "item5")',
        'A literal array must be wrapped ({"type": [[1, 2, 3]]}) because the outer array is the argument list',
        'Multi-key object literals need templating mode; in the default mode read the object from data',
        'Empty arrays and objects are still "array" and "object"',
      ],
      seeAlso: ['!!'],
    },
    ui: {
      icon: 'tag',
      shortLabel: 'type',
      nodeType: 'operator',
    },
  },

};
