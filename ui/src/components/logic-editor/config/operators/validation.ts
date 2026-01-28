/**
 * Validation Operators
 *
 * Data validation operations.
 * - missing: Check for missing variables
 * - missing_some: Check if minimum required fields are present
 */

import type { Operator } from '../operators.types';

export const validationOperators: Record<string, Operator> = {
  missing: {
    name: 'missing',
    label: 'Missing',
    category: 'validation',
    description: 'Check for missing variables',
    arity: {
      type: 'nary',
      min: 1,
      args: [
        {
          name: 'path',
          label: 'Path',
          type: 'string',
          required: true,
          repeatable: true,
          description: 'Variable path to check',
        },
      ],
    },
    help: {
      summary: 'Returns an array of variable paths that are missing from the data',
      details:
        'Checks if the specified variable paths exist in the data. Returns an array containing paths that are missing (not present or null). Useful for form validation and required field checks.',
      returnType: 'array',
      examples: [
        {
          title: 'All present',
          rule: { missing: ['name', 'email'] },
          data: { name: 'Alice', email: 'alice@example.com' },
          result: [],
        },
        {
          title: 'One missing',
          rule: { missing: ['name', 'phone'] },
          data: { name: 'Alice' },
          result: ['phone'],
        },
        {
          title: 'Multiple missing',
          rule: { missing: ['a', 'b', 'c'] },
          data: { a: 1 },
          result: ['b', 'c'],
        },
        {
          title: 'Nested paths',
          rule: { missing: ['user.name', 'user.email'] },
          data: { user: { name: 'Alice' } },
          result: ['user.email'],
        },
        {
          title: 'Check array of paths',
          rule: { missing: [['name', 'email', 'phone']] },
          data: { name: 'Alice', email: 'alice@example.com' },
          result: ['phone'],
          note: 'Array argument is flattened',
        },
      ],
      notes: [
        'Returns empty array if all paths exist',
        'Paths use dot notation for nesting',
        'Can accept arrays of paths as arguments',
        'null values are considered missing',
      ],
      seeAlso: ['missing_some', 'exists', 'var'],
    },
    ui: {
      icon: 'alert-circle',
      shortLabel: 'miss',
      nodeType: 'operator',
    },
  },

  missing_some: {
    name: 'missing_some',
    label: 'Missing Some',
    category: 'validation',
    description: 'Check if minimum required fields are present',
    arity: {
      type: 'binary',
      min: 2,
      max: 2,
      args: [
        {
          name: 'minRequired',
          label: 'Minimum Required',
          type: 'number',
          required: true,
          description: 'Minimum number of fields that must be present',
        },
        {
          name: 'paths',
          label: 'Paths',
          type: 'array',
          required: true,
          description: 'Array of variable paths to check',
        },
      ],
    },
    help: {
      summary:
        'Returns empty array if minimum fields are present, otherwise returns missing fields',
      details:
        'Checks if at least N fields from a list are present in the data. Returns empty array if the requirement is met, otherwise returns the list of missing field paths. Useful for "at least one of these required" validation.',
      returnType: 'array',
      examples: [
        {
          title: 'Requirement met',
          rule: { missing_some: [1, ['email', 'phone', 'fax']] },
          data: { email: 'alice@example.com' },
          result: [],
          note: 'At least 1 contact method present',
        },
        {
          title: 'Requirement not met',
          rule: { missing_some: [2, ['email', 'phone', 'fax']] },
          data: { email: 'alice@example.com' },
          result: ['phone', 'fax'],
          note: 'Need 2, only have 1',
        },
        {
          title: 'All present',
          rule: { missing_some: [2, ['name', 'email', 'phone']] },
          data: { name: 'Alice', email: 'alice@example.com', phone: '555-1234' },
          result: [],
        },
        {
          title: 'None present',
          rule: { missing_some: [1, ['email', 'phone']] },
          data: { name: 'Alice' },
          result: ['email', 'phone'],
        },
      ],
      notes: [
        'Returns empty array [] when requirement is satisfied',
        'Returns missing paths when requirement is NOT satisfied',
        'Useful for "provide at least N of these fields" validation',
        'Early exits once minimum requirement is met',
      ],
      seeAlso: ['missing', 'exists'],
    },
    ui: {
      icon: 'list-checks',
      shortLabel: 'some',
      nodeType: 'operator',
    },
  },
};
