/**
 * Array Iteration Operators
 *
 * Iteration operations: map, filter, reduce, all, some, none
 */

import type { Operator } from '../operators.types';

export const arrayIterationOperators: Record<string, Operator> = {
  map: {
    name: 'map',
    label: 'Map',
    category: 'array',
    description: 'Transform each element of an array',
    arity: {
      type: 'binary',
      min: 2,
      max: 2,
      args: [
        { name: 'array', label: 'Array', type: 'array', required: true },
        {
          name: 'expression',
          label: 'Expression',
          type: 'expression',
          required: true,
          description: 'Applied to each element',
        },
      ],
    },
    help: {
      summary: 'Apply an expression to each element of an array',
      details:
        'Iterates over an array and applies the given expression to each element. Use {"var": ""} to access the current element. Use {"val": "index"} for the current index. Use {"val": [[1], "field"]} to access parent scope.',
      returnType: 'array',
      examples: [
        {
          title: 'Double each number',
          rule: { map: [[1, 2, 3], { '*': [{ var: '' }, 2] }] },
          result: [2, 4, 6],
        },
        {
          title: 'Extract field',
          rule: { map: [{ var: 'users' }, { var: 'name' }] },
          data: { users: [{ name: 'Alice' }, { name: 'Bob' }] },
          result: ['Alice', 'Bob'],
        },
        {
          title: 'With index',
          rule: {
            map: [
              { var: 'items' },
              { cat: ['Item ', { val: 'index' }, ': ', { var: '' }] },
            ],
          },
          data: { items: ['a', 'b', 'c'] },
          result: ['Item 0: a', 'Item 1: b', 'Item 2: c'],
        },
        {
          title: 'Access parent scope',
          rule: {
            map: [
              { var: 'values' },
              { '*': [{ var: '' }, { val: [[1], 'multiplier'] }] },
            ],
          },
          data: { values: [1, 2, 3], multiplier: 10 },
          result: [10, 20, 30],
        },
      ],
      notes: [
        '{"var": ""} = current element',
        '{"val": "index"} = current index (0, 1, 2...)',
        '{"val": [[1], "field"]} = parent scope field',
        'Returns a new array; original unchanged',
      ],
      seeAlso: ['filter', 'reduce', 'all', 'some', 'none'],
    },
    ui: {
      icon: 'repeat',
      shortLabel: 'map',
      nodeType: 'iterator',
      iteratorContext: true,
    },
    panel: {
      sections: [
        {
          id: 'args',
          fields: [
            {
              id: 'array',
              label: 'Array',
              inputType: 'expression',
              required: true,
              helpText: 'The array to iterate over',
            },
            {
              id: 'expression',
              label: 'Expression',
              inputType: 'expression',
              required: true,
              helpText: 'Expression applied to each element',
            },
          ],
        },
      ],
      contextVariables: [
        {
          name: '',
          label: 'Current Element',
          accessor: 'var',
          example: '{"var": ""}',
          description: 'The current array element being processed',
        },
        {
          name: 'index',
          label: 'Index',
          accessor: 'val',
          example: '{"val": "index"}',
          description: 'Zero-based index of the current element (0, 1, 2...)',
        },
      ],
    },
  },

  filter: {
    name: 'filter',
    label: 'Filter',
    category: 'array',
    description: 'Keep elements that match a condition',
    arity: {
      type: 'binary',
      min: 2,
      max: 2,
      args: [
        { name: 'array', label: 'Array', type: 'array', required: true },
        {
          name: 'condition',
          label: 'Condition',
          type: 'expression',
          required: true,
          description: 'Must return truthy to keep element',
        },
      ],
    },
    help: {
      summary: 'Filter array elements based on a condition',
      details:
        'Returns a new array containing only elements for which the condition returns a truthy value.',
      returnType: 'array',
      examples: [
        {
          title: 'Filter numbers',
          rule: { filter: [[1, 2, 3, 4, 5], { '>': [{ var: '' }, 2] }] },
          result: [3, 4, 5],
        },
        {
          title: 'Filter objects',
          rule: {
            filter: [
              { var: 'users' },
              { '==': [{ var: 'status' }, 'active'] },
            ],
          },
          data: {
            users: [
              { name: 'Alice', status: 'active' },
              { name: 'Bob', status: 'inactive' },
              { name: 'Carol', status: 'active' },
            ],
          },
          result: [
            { name: 'Alice', status: 'active' },
            { name: 'Carol', status: 'active' },
          ],
        },
        {
          title: 'Filter with index',
          rule: {
            filter: [
              ['a', 'b', 'c', 'd'],
              { '==': [{ '%': [{ val: 'index' }, 2] }, 0] },
            ],
          },
          result: ['a', 'c'],
          note: 'Keep even-indexed elements',
        },
      ],
      notes: [
        'Condition must return truthy to keep element',
        '{"var": ""} = current element',
        '{"val": "index"} = current index',
        'Returns empty array if nothing matches',
      ],
      seeAlso: ['map', 'all', 'some', 'none'],
    },
    ui: {
      icon: 'filter',
      shortLabel: 'filter',
      nodeType: 'iterator',
      iteratorContext: true,
    },
    panel: {
      sections: [
        {
          id: 'args',
          fields: [
            {
              id: 'array',
              label: 'Array',
              inputType: 'expression',
              required: true,
              helpText: 'The array to filter',
            },
            {
              id: 'condition',
              label: 'Condition',
              inputType: 'expression',
              required: true,
              helpText: 'Condition that must be truthy to keep element',
            },
          ],
        },
      ],
      contextVariables: [
        {
          name: '',
          label: 'Current Element',
          accessor: 'var',
          example: '{"var": ""}',
          description: 'The current array element being tested',
        },
        {
          name: 'index',
          label: 'Index',
          accessor: 'val',
          example: '{"val": "index"}',
          description: 'Zero-based index of the current element (0, 1, 2...)',
        },
      ],
    },
  },

  reduce: {
    name: 'reduce',
    label: 'Reduce',
    category: 'array',
    description: 'Reduce array to a single value',
    arity: {
      type: 'ternary',
      min: 3,
      max: 3,
      args: [
        { name: 'array', label: 'Array', type: 'array', required: true },
        {
          name: 'expression',
          label: 'Expression',
          type: 'expression',
          required: true,
          description: 'Combines accumulator and current',
        },
        {
          name: 'initial',
          label: 'Initial',
          type: 'any',
          required: true,
          description: 'Starting value for accumulator',
        },
      ],
    },
    help: {
      summary: 'Reduce an array to a single value using an accumulator',
      details:
        'Iterates over the array, applying the expression to each element. The expression has access to "current" (current element) and "accumulator" (running result). The result of each iteration becomes the new accumulator.',
      returnType: 'any',
      examples: [
        {
          title: 'Sum numbers',
          rule: {
            reduce: [
              [1, 2, 3, 4, 5],
              { '+': [{ var: 'accumulator' }, { var: 'current' }] },
              0,
            ],
          },
          result: 15,
        },
        {
          title: 'Product',
          rule: {
            reduce: [
              [1, 2, 3, 4],
              { '*': [{ var: 'accumulator' }, { var: 'current' }] },
              1,
            ],
          },
          result: 24,
        },
        {
          title: 'Find maximum',
          rule: {
            reduce: [
              [3, 1, 4, 1, 5],
              {
                if: [
                  { '>': [{ var: 'current' }, { var: 'accumulator' }] },
                  { var: 'current' },
                  { var: 'accumulator' },
                ],
              },
              0,
            ],
          },
          result: 5,
        },
        {
          title: 'Build object',
          rule: {
            reduce: [
              { var: 'pairs' },
              {
                merge: [
                  { var: 'accumulator' },
                  {
                    cat: [
                      '{"',
                      { var: 'current.key' },
                      '":"',
                      { var: 'current.value' },
                      '"}',
                    ],
                  },
                ],
              },
              {},
            ],
          },
          data: {
            pairs: [
              { key: 'a', value: '1' },
              { key: 'b', value: '2' },
            ],
          },
          result: { a: '1', b: '2' },
        },
      ],
      notes: [
        '{"var": "current"} = current element',
        '{"var": "accumulator"} = running result',
        '{"val": "index"} = current index',
        'Initial value is required',
      ],
      seeAlso: ['map', 'filter'],
    },
    ui: {
      icon: 'fold-vertical',
      shortLabel: 'reduce',
      nodeType: 'iterator',
      iteratorContext: true,
    },
    panel: {
      sections: [
        {
          id: 'args',
          fields: [
            {
              id: 'array',
              label: 'Array',
              inputType: 'expression',
              required: true,
              helpText: 'The array to reduce',
            },
            {
              id: 'expression',
              label: 'Expression',
              inputType: 'expression',
              required: true,
              helpText: 'Expression that combines accumulator and current element',
            },
            {
              id: 'initial',
              label: 'Initial Value',
              inputType: 'expression',
              required: true,
              helpText: 'Starting value for the accumulator',
            },
          ],
        },
      ],
      contextVariables: [
        {
          name: 'current',
          label: 'Current Element',
          accessor: 'var',
          example: '{"var": "current"}',
          description: 'The current array element being processed',
        },
        {
          name: 'accumulator',
          label: 'Accumulator',
          accessor: 'var',
          example: '{"var": "accumulator"}',
          description: 'The running result value',
        },
        {
          name: 'index',
          label: 'Index',
          accessor: 'val',
          example: '{"val": "index"}',
          description: 'Zero-based index of the current element (0, 1, 2...)',
        },
      ],
    },
  },

  all: {
    name: 'all',
    label: 'All',
    category: 'array',
    description: 'Check if all elements match condition',
    arity: {
      type: 'binary',
      min: 2,
      max: 2,
      args: [
        { name: 'array', label: 'Array', type: 'array', required: true },
        { name: 'condition', label: 'Condition', type: 'expression', required: true },
      ],
    },
    help: {
      summary: 'Check if all elements satisfy a condition',
      details:
        'Returns true if the condition returns truthy for every element. Returns true for empty arrays.',
      returnType: 'boolean',
      examples: [
        {
          title: 'All positive',
          rule: { all: [[1, 2, 3], { '>': [{ var: '' }, 0] }] },
          result: true,
        },
        {
          title: 'Not all positive',
          rule: { all: [[1, -2, 3], { '>': [{ var: '' }, 0] }] },
          result: false,
        },
        {
          title: 'All active users',
          rule: {
            all: [{ var: 'users' }, { '==': [{ var: 'status' }, 'active'] }],
          },
          data: {
            users: [
              { status: 'active' },
              { status: 'active' },
            ],
          },
          result: true,
        },
        {
          title: 'Empty array',
          rule: { all: [[], { '>': [{ var: '' }, 0] }] },
          result: true,
          note: 'Vacuous truth: all of nothing is true',
        },
      ],
      notes: [
        'Short-circuits: stops on first false',
        'Empty array returns true',
        '{"var": ""} = current element',
      ],
      seeAlso: ['some', 'none', 'filter'],
    },
    ui: {
      icon: 'check-check',
      shortLabel: 'all',
      nodeType: 'iterator',
      iteratorContext: true,
    },
    panel: {
      sections: [
        {
          id: 'args',
          fields: [
            {
              id: 'array',
              label: 'Array',
              inputType: 'expression',
              required: true,
              helpText: 'The array to check',
            },
            {
              id: 'condition',
              label: 'Condition',
              inputType: 'expression',
              required: true,
              helpText: 'Condition that must be truthy for all elements',
            },
          ],
        },
      ],
      contextVariables: [
        {
          name: '',
          label: 'Current Element',
          accessor: 'var',
          example: '{"var": ""}',
          description: 'The current array element being tested',
        },
        {
          name: 'index',
          label: 'Index',
          accessor: 'val',
          example: '{"val": "index"}',
          description: 'Zero-based index of the current element (0, 1, 2...)',
        },
      ],
    },
  },

  some: {
    name: 'some',
    label: 'Some',
    category: 'array',
    description: 'Check if any element matches condition',
    arity: {
      type: 'binary',
      min: 2,
      max: 2,
      args: [
        { name: 'array', label: 'Array', type: 'array', required: true },
        { name: 'condition', label: 'Condition', type: 'expression', required: true },
      ],
    },
    help: {
      summary: 'Check if at least one element satisfies a condition',
      details:
        'Returns true if the condition returns truthy for any element. Returns false for empty arrays.',
      returnType: 'boolean',
      examples: [
        {
          title: 'Any negative',
          rule: { some: [[1, -2, 3], { '<': [{ var: '' }, 0] }] },
          result: true,
        },
        {
          title: 'None negative',
          rule: { some: [[1, 2, 3], { '<': [{ var: '' }, 0] }] },
          result: false,
        },
        {
          title: 'Any admin',
          rule: {
            some: [{ var: 'users' }, { '==': [{ var: 'role' }, 'admin'] }],
          },
          data: {
            users: [
              { role: 'user' },
              { role: 'admin' },
            ],
          },
          result: true,
        },
        {
          title: 'Empty array',
          rule: { some: [[], { '>': [{ var: '' }, 0] }] },
          result: false,
        },
      ],
      notes: [
        'Short-circuits: stops on first true',
        'Empty array returns false',
        '{"var": ""} = current element',
      ],
      seeAlso: ['all', 'none', 'filter'],
    },
    ui: {
      icon: 'check',
      shortLabel: 'some',
      nodeType: 'iterator',
      iteratorContext: true,
    },
    panel: {
      sections: [
        {
          id: 'args',
          fields: [
            {
              id: 'array',
              label: 'Array',
              inputType: 'expression',
              required: true,
              helpText: 'The array to check',
            },
            {
              id: 'condition',
              label: 'Condition',
              inputType: 'expression',
              required: true,
              helpText: 'Condition to test against each element',
            },
          ],
        },
      ],
      contextVariables: [
        {
          name: '',
          label: 'Current Element',
          accessor: 'var',
          example: '{"var": ""}',
          description: 'The current array element being tested',
        },
        {
          name: 'index',
          label: 'Index',
          accessor: 'val',
          example: '{"val": "index"}',
          description: 'Zero-based index of the current element (0, 1, 2...)',
        },
      ],
    },
  },

  none: {
    name: 'none',
    label: 'None',
    category: 'array',
    description: 'Check if no elements match condition',
    arity: {
      type: 'binary',
      min: 2,
      max: 2,
      args: [
        { name: 'array', label: 'Array', type: 'array', required: true },
        { name: 'condition', label: 'Condition', type: 'expression', required: true },
      ],
    },
    help: {
      summary: 'Check that no elements satisfy a condition',
      details:
        'Returns true if the condition returns falsy for every element. Equivalent to !some(...).',
      returnType: 'boolean',
      examples: [
        {
          title: 'No negatives',
          rule: { none: [[1, 2, 3], { '<': [{ var: '' }, 0] }] },
          result: true,
        },
        {
          title: 'Has negative',
          rule: { none: [[1, -2, 3], { '<': [{ var: '' }, 0] }] },
          result: false,
        },
        {
          title: 'No inactive',
          rule: {
            none: [{ var: 'users' }, { '==': [{ var: 'status' }, 'inactive'] }],
          },
          data: {
            users: [
              { status: 'active' },
              { status: 'active' },
            ],
          },
          result: true,
        },
      ],
      notes: [
        'Equivalent to: !some(...)',
        'Empty array returns true',
        '{"var": ""} = current element',
      ],
      seeAlso: ['some', 'all', 'filter'],
    },
    ui: {
      icon: 'x-circle',
      shortLabel: 'none',
      nodeType: 'iterator',
      iteratorContext: true,
    },
    panel: {
      sections: [
        {
          id: 'args',
          fields: [
            {
              id: 'array',
              label: 'Array',
              inputType: 'expression',
              required: true,
              helpText: 'The array to check',
            },
            {
              id: 'condition',
              label: 'Condition',
              inputType: 'expression',
              required: true,
              helpText: 'Condition that must be falsy for all elements',
            },
          ],
        },
      ],
      contextVariables: [
        {
          name: '',
          label: 'Current Element',
          accessor: 'var',
          example: '{"var": ""}',
          description: 'The current array element being tested',
        },
        {
          name: 'index',
          label: 'Index',
          accessor: 'val',
          example: '{"val": "index"}',
          description: 'Zero-based index of the current element (0, 1, 2...)',
        },
      ],
    },
  },
};
