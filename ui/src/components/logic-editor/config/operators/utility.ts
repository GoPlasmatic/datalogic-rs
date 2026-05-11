/**
 * Utility Operators
 *
 * General-purpose utility operations.
 * - type: Get the type of a value
 *
 * Note: the v4 `preserve` operator was removed in v5 — literal scalars and
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
          rule: { type: [1, 2, 3] },
          result: 'array',
        },
        {
          title: 'Object type',
          rule: { type: { key: 'value' } },
          result: 'object',
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
        'Duration: detected by time units (d, h, m, s) with digits',
        'Empty arrays and objects are still "array" and "object"',
      ],
      seeAlso: ['!!'],
    },
    ui: {
      icon: 'info',
      shortLabel: 'type',
      nodeType: 'operator',
    },
  },

};
