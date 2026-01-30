/**
 * Variable Operators
 *
 * Operators for accessing data from the context.
 * - var: Dot notation path access
 * - val: Array path with scope jump support
 * - exists: Check if path exists
 */

import type { Operator } from '../operators.types';

export const variableOperators: Record<string, Operator> = {
  var: {
    name: 'var',
    label: 'Variable',
    category: 'variable',
    description: 'Access data using dot notation path',
    arity: {
      type: 'range',
      min: 1,
      max: 2,
      args: [
        {
          name: 'path',
          label: 'Path',
          type: 'string',
          description: 'Dot notation path (e.g., "user.profile.name")',
          required: true,
        },
        {
          name: 'default',
          label: 'Default',
          type: 'any',
          description: 'Default value if path not found',
          required: false,
        },
      ],
    },
    help: {
      summary: 'Access data using dot notation path string',
      details:
        'Retrieves a value from the data context using dot notation. Supports nested paths like "user.profile.name". Use empty string "" to get the current element in iterators.',
      returnType: 'any',
      examples: [
        {
          title: 'Simple field access',
          rule: { var: 'name' },
          data: { name: 'Alice' },
          result: 'Alice',
        },
        {
          title: 'Nested path',
          rule: { var: 'user.profile.email' },
          data: { user: { profile: { email: 'alice@example.com' } } },
          result: 'alice@example.com',
        },
        {
          title: 'With default value',
          rule: { var: ['theme', 'light'] },
          data: {},
          result: 'light',
        },
        {
          title: 'Array index access',
          rule: { var: 'items.0.name' },
          data: { items: [{ name: 'First' }] },
          result: 'First',
        },
        {
          title: 'Current element in iterator',
          rule: { var: '' },
          note: 'Used inside map, filter, reduce',
          data: null,
          result: '(current element)',
        },
      ],
      notes: [
        'Uses dot notation: "a.b.c" accesses data.a.b.c',
        'Empty string "" returns entire current context',
        'Second argument is default if path not found',
        'For scope jumps in nested iterators, use val instead',
      ],
      seeAlso: ['val', 'exists'],
    },
    ui: {
      icon: 'box',
      shortLabel: 'var',
      nodeType: 'variable',
      inlineEditable: true,
      addArgumentLabel: 'Add Default',
    },
    panel: {
      sections: [
        {
          id: 'path',
          fields: [
            {
              id: 'path',
              label: 'Path',
              inputType: 'text',
              placeholder: 'user.profile.name',
              helpText: 'Dot notation path to the data',
              required: true,
            },
            {
              id: 'hasDefault',
              label: 'Has Default',
              inputType: 'boolean',
              defaultValue: false,
            },
            {
              id: 'default',
              label: 'Default Value',
              inputType: 'expression',
              helpText: 'Value to return if path not found',
              showWhen: [{ field: 'hasDefault', operator: 'equals', value: true }],
            },
          ],
        },
      ],
    },
  },

  val: {
    name: 'val',
    label: 'Value',
    category: 'variable',
    description: 'Access data using array path with scope jump support',
    arity: {
      type: 'special',
      min: 1,
      args: [
        {
          name: 'path',
          label: 'Path',
          type: 'path',
          description: 'Array of path components, optionally with scope level',
          required: true,
        },
      ],
    },
    help: {
      summary: 'Access data using array path components with scope jump and metadata support',
      details:
        'Retrieves a value using an array of path components. Supports scope jumps for accessing parent contexts in nested iterators. Use [[N], "field", ...] to jump up N context levels (sign is ignored). Also provides access to iteration metadata and datetime properties.',
      returnType: 'any',
      examples: [
        {
          title: 'Array path',
          rule: { val: ['user', 'profile', 'name'] },
          data: { user: { profile: { name: 'Alice' } } },
          result: 'Alice',
        },
        {
          title: 'Current element',
          rule: { val: [] },
          note: 'Used inside iterators',
          data: null,
          result: '(current element)',
        },
        {
          title: 'Parent scope access',
          rule: { val: [[1], 'multiplier'] },
          note: 'Jump up 1 level',
          data: null,
          result: '(parent multiplier)',
        },
        {
          title: 'Grandparent scope',
          rule: { val: [[2], 'config', 'limit'] },
          note: 'Jump up 2 levels',
          data: null,
          result: '(grandparent config.limit)',
        },
        {
          title: 'Get iteration index',
          rule: { val: 'index' },
          note: 'Inside map, filter, etc.',
          data: null,
          result: '0, 1, 2, ...',
        },
        {
          title: 'Get object key',
          rule: { val: 'key' },
          note: 'During object iteration',
          data: null,
          result: '(current key name)',
        },
        {
          title: 'DateTime property',
          rule: { val: [{ var: 'date' }, 'year'] },
          data: { date: '2024-01-15T10:30:00Z' },
          result: 2024,
        },
      ],
      notes: [
        'Path is array of components: ["a", "b", "c"]',
        'Scope jump: [[N], ...] goes up N context levels',
        'Sign is ignored: [1] and [-1] are equivalent',
        'If level exceeds depth, returns root data',
        'Special metadata: "index" and "key" for iteration',
        'DateTime props: year, month, day, hour, minute, second, timestamp, iso',
        'Duration props: days, hours, minutes, seconds, total_seconds',
      ],
      seeAlso: ['var', 'exists'],
    },
    ui: {
      icon: 'brackets',
      shortLabel: 'val',
      nodeType: 'variable',
      scopeJump: true,
      metadata: true,
      datetimeProps: true,
      addArgumentLabel: 'Add Path',
    },
    panel: {
      sections: [
        {
          id: 'accessType',
          fields: [
            {
              id: 'accessType',
              label: 'Access Type',
              inputType: 'select',
              required: true,
              defaultValue: 'path',
              options: [
                { value: 'path', label: 'Data Path', description: 'Access data using array path' },
                {
                  value: 'metadata',
                  label: 'Metadata',
                  description: 'Access iteration metadata (index/key)',
                },
              ],
            },
          ],
        },
        {
          id: 'pathConfig',
          title: 'Path Configuration',
          showWhen: [{ field: 'accessType', operator: 'equals', value: 'path' }],
          fields: [
            {
              id: 'scopeLevel',
              label: 'Scope Jump',
              inputType: 'number',
              min: 0,
              max: 10,
              defaultValue: 0,
              helpText: 'Number of context levels to jump up (0 = current scope)',
            },
            {
              id: 'path',
              label: 'Path Components',
              inputType: 'pathArray',
              repeatable: true,
              helpText: 'Path segments to traverse',
            },
          ],
        },
        {
          id: 'metadataConfig',
          title: 'Metadata Configuration',
          showWhen: [{ field: 'accessType', operator: 'equals', value: 'metadata' }],
          fields: [
            {
              id: 'metadataKey',
              label: 'Key',
              inputType: 'select',
              required: true,
              options: [
                { value: 'index', label: 'index', description: 'Current iteration index (0, 1, 2...)' },
                { value: 'key', label: 'key', description: 'Current object key during iteration' },
              ],
            },
          ],
        },
      ],
    },
  },

  exists: {
    name: 'exists',
    label: 'Exists',
    category: 'variable',
    description: 'Check if a path exists in the data',
    arity: {
      type: 'special',
      min: 1,
      args: [
        {
          name: 'path',
          label: 'Path',
          type: 'path',
          description: 'Path to check (dot notation or array)',
          required: true,
        },
      ],
    },
    help: {
      summary: 'Check if a path exists in the data (returns boolean)',
      details:
        'Returns true if the specified path exists in the data, false otherwise. Checks for key presence, not whether the value is null or empty. Supports both dot notation and array paths.',
      returnType: 'boolean',
      examples: [
        {
          title: 'Simple field check',
          rule: { exists: 'name' },
          data: { name: 'Alice' },
          result: true,
        },
        {
          title: 'Missing field',
          rule: { exists: 'email' },
          data: { name: 'Alice' },
          result: false,
        },
        {
          title: 'Nested path (dot notation)',
          rule: { exists: 'user.profile.email' },
          data: { user: { profile: { email: 'a@b.com' } } },
          result: true,
        },
        {
          title: 'Nested path (array)',
          rule: { exists: ['user', 'profile', 'phone'] },
          data: { user: { profile: { email: 'a@b.com' } } },
          result: false,
        },
        {
          title: 'Null value still exists',
          rule: { exists: 'value' },
          data: { value: null },
          result: true,
          note: 'Key exists even though value is null',
        },
      ],
      notes: [
        'Returns true if the key exists, regardless of value',
        'Null values count as existing',
        'Supports dot notation: "a.b.c"',
        'Supports array path: ["a", "b", "c"]',
        'Returns false if any intermediate path segment is missing',
      ],
      seeAlso: ['var', 'val', 'missing'],
    },
    ui: {
      icon: 'help-circle',
      shortLabel: '?',
      nodeType: 'variable',
    },
    panel: {
      sections: [
        {
          id: 'path',
          fields: [
            {
              id: 'pathType',
              label: 'Path Format',
              inputType: 'select',
              defaultValue: 'dot',
              options: [
                { value: 'dot', label: 'Dot Notation', description: 'e.g., user.profile.name' },
                { value: 'array', label: 'Array Path', description: 'e.g., ["user", "profile", "name"]' },
              ],
            },
            {
              id: 'dotPath',
              label: 'Path',
              inputType: 'text',
              placeholder: 'user.profile.name',
              required: true,
              showWhen: [{ field: 'pathType', operator: 'equals', value: 'dot' }],
            },
            {
              id: 'arrayPath',
              label: 'Path Components',
              inputType: 'pathArray',
              repeatable: true,
              required: true,
              showWhen: [{ field: 'pathType', operator: 'equals', value: 'array' }],
            },
          ],
        },
      ],
    },
  },
};
