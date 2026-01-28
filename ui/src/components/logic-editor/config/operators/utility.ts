/**
 * Utility Operators
 *
 * General-purpose utility operations.
 * - type: Get the type of a value
 * - preserve: Pass values through unchanged (templating mode)
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

  preserve: {
    name: 'preserve',
    label: 'Preserve',
    category: 'utility',
    description: 'Pass values through unchanged (templating mode)',
    arity: {
      type: 'special',
      min: 0,
      args: [
        {
          name: 'value',
          label: 'Value',
          type: 'any',
          required: false,
          repeatable: true,
          description: 'Value(s) to preserve',
        },
      ],
    },
    help: {
      summary: 'Evaluate and return values unchanged',
      details:
        'Used in structure preservation (templating) mode to pass values through without interpretation as operators. With one argument, returns that value. With multiple arguments, returns an array.',
      returnType: 'any',
      examples: [
        {
          title: 'Preserve single value',
          rule: { preserve: 42 },
          result: 42,
        },
        {
          title: 'Preserve expression result',
          rule: { preserve: { '+': [1, 2] } },
          result: 3,
        },
        {
          title: 'Preserve multiple values',
          rule: { preserve: [1, 2, 3] },
          result: [1, 2, 3],
        },
        {
          title: 'No arguments',
          rule: { preserve: [] },
          result: [],
        },
        {
          title: 'In template context',
          rule: {
            name: { var: 'user.name' },
            status: { preserve: 'active' },
          },
          data: { user: { name: 'Alice' } },
          result: { name: 'Alice', status: 'active' },
          note: 'With preserve_structure enabled',
        },
      ],
      notes: [
        'Used with preserve_structure mode for JSON templating',
        'No arguments: returns empty array',
        'One argument: returns that argument evaluated',
        'Multiple arguments: returns array of evaluated arguments',
      ],
      seeAlso: ['var', 'val'],
    },
    ui: {
      icon: 'lock',
      shortLabel: 'keep',
      nodeType: 'operator',
    },
  },
};
