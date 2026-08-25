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
          title: 'Missing path returns null',
          rule: { var: 'missing' },
          data: {},
          result: null,
          note: 'Never throws; use the default argument or ?? for a fallback',
        },
        {
          title: 'Empty path returns the whole context',
          rule: { var: '' },
          data: { a: 1 },
          result: { a: 1 },
        },
        {
          title: 'Current element in iterator',
          rule: { map: [['a', 'b'], { var: '' }] },
          result: ['a', 'b'],
          note: 'Inside map, filter, all, some and none, "" is the current element',
        },
      ],
      notes: [
        'Uses dot notation: "a.b.c" accesses data.a.b.c',
        'Empty string "" returns entire current context',
        'Second argument is default if path not found',
        'Missing paths return null and never throw',
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
        'Retrieves a value using an array of path components. Supports scope jumps for accessing parent contexts in nested iterators. Use [[N], "field", ...] to jump up N context levels (sign is ignored). Also provides access to iteration metadata (index and key) through the same scope form.',
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
          rule: { map: [['a', 'b'], { val: [] }] },
          result: ['a', 'b'],
          note: 'An empty path is the current element inside an iterator',
        },
        {
          title: 'Parent scope access',
          rule: {
            map: [
              { var: 'values' },
              { '*': [{ var: '' }, { val: [[1], 'multiplier'] }] },
            ],
          },
          data: { values: [1, 2, 3], multiplier: 10 },
          result: [10, 20, 30],
          note: '[[1]] jumps out of the map frame to the data holding multiplier',
        },
        {
          title: 'Grandparent scope',
          rule: {
            map: [
              { var: 'rows' },
              { map: [{ var: 'cols' }, { val: [[2], 'config', 'limit'] }] },
            ],
          },
          data: { rows: [{ cols: [1, 2] }, { cols: [3] }], config: { limit: 5 } },
          result: [[5, 5], [5]],
          note: 'Two nested iterators: [[2]] reaches the root data',
        },
        {
          title: 'Get iteration index',
          rule: { map: [['a', 'b', 'c'], { val: [[1], 'index'] }] },
          result: [0, 1, 2],
          note: 'Metadata needs the scope form; {"val": "index"} is a plain key lookup',
        },
        {
          title: 'Get object key',
          rule: {
            map: [{ var: 'o' }, { cat: [{ val: [[1], 'key'] }, '=', { var: '' }] }],
          },
          data: { o: { a: 1, b: 2 } },
          result: ['a=1', 'b=2'],
          note: 'Iterating an object visits its values; [[1], "key"] is the current key',
        },
        {
          title: 'Missing path returns null',
          rule: { val: ['missing'] },
          data: {},
          result: null,
          note: 'val has no default argument; use var\'s second argument or ??',
        },
      ],
      notes: [
        'Path is array of components: ["a", "b", "c"]',
        'Scope jump: [[N], ...] goes up N context levels',
        'Sign is ignored: [1] and [-1] are equivalent',
        'If level exceeds depth, returns root data',
        'Iteration metadata: {"val": [[1], "index"]} and {"val": [[1], "key"]} (the string form {"val": "index"} looks up a key named "index")',
        'No default argument: use var\'s second argument or ?? for fallbacks',
      ],
      seeAlso: ['var', 'exists'],
    },
    ui: {
      icon: 'database',
      shortLabel: 'val',
      nodeType: 'variable',
      scopeJump: true,
      metadata: true,
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
          description: 'Single key (string) or nested path (array of segments)',
          required: true,
        },
      ],
    },
    help: {
      summary: 'Check if a path exists in the data (returns boolean)',
      details:
        'Returns true if the specified path exists in the data, false otherwise. Checks for key presence, not whether the value is null or empty. A string argument is one literal key (dots are not special); use an array of segments for nested access.',
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
          title: 'Nested path (array)',
          rule: { exists: ['user', 'profile', 'email'] },
          data: { user: { profile: { email: 'a@b.com' } } },
          result: true,
        },
        {
          title: 'Nested path missing',
          rule: { exists: ['user', 'profile', 'phone'] },
          data: { user: { profile: { email: 'a@b.com' } } },
          result: false,
        },
        {
          title: 'Dots are not paths',
          rule: { exists: 'user.profile.email' },
          data: { user: { profile: { email: 'a@b.com' } } },
          result: false,
          note: 'A string is one literal key named "user.profile.email"; use the array form',
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
        'A string argument is a single literal key (dots are not special)',
        'Use an array path for nesting: ["a", "b", "c"]',
        'Returns false if any intermediate path segment is missing',
      ],
      seeAlso: ['var', 'val', 'missing'],
    },
    ui: {
      icon: 'search',
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
              defaultValue: 'array',
              options: [
                { value: 'dot', label: 'Single Key', description: 'One literal key, e.g. "email"' },
                { value: 'array', label: 'Array Path', description: 'Nested segments, e.g. ["user", "profile", "email"]' },
              ],
            },
            {
              id: 'dotPath',
              label: 'Key',
              inputType: 'text',
              placeholder: 'email',
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
