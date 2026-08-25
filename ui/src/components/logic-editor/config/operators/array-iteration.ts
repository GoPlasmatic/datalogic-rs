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
        'Iterates over an array and applies the given expression to each element. Use {"var": ""} to access the current element, {"val": [[1], "index"]} for the current index and {"val": [[1], "field"]} to read a field from the parent scope.',
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
              { cat: ['Item ', { val: [[1], 'index'] }, ': ', { var: '' }] },
            ],
          },
          data: { items: ['a', 'b', 'c'] },
          result: ['Item 0: a', 'Item 1: b', 'Item 2: c'],
          note: 'The index is only reachable through the [[1], "index"] scope form',
        },
        {
          title: 'Object keys',
          rule: {
            map: [{ var: 'o' }, { cat: [{ val: [[1], 'key'] }, '=', { var: '' }] }],
          },
          data: { o: { a: 1, b: 2 } },
          result: ['a=1', 'b=2'],
          note: 'Mapping an object visits its values; [[1], "key"] is the current key',
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
        '{"val": [[1], "index"]} = current index (0, 1, 2...); {"val": [[1], "key"]} = current key when mapping an object',
        '{"val": "index"} (plain string) is a normal key lookup and returns null',
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
          example: '{"val": [[1], "index"]}',
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
              { '==': [{ '%': [{ val: [[1], 'index'] }, 2] }, 0] },
            ],
          },
          result: ['a', 'c'],
          note: 'Keep even-indexed elements',
        },
      ],
      notes: [
        'Condition must return truthy to keep element',
        '{"var": ""} = current element',
        '{"val": [[1], "index"]} = current index',
        'Returns empty array if nothing matches',
      ],
      seeAlso: ['map', 'all', 'some', 'none'],
    },
    ui: {
      icon: 'search',
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
          example: '{"val": [[1], "index"]}',
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
      type: 'range',
      min: 2,
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
          required: false,
          description: 'Starting value for accumulator (null when omitted)',
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
          title: 'Build an array',
          rule: {
            reduce: [
              { var: 'words' },
              { merge: [{ var: 'accumulator' }, { upper: { var: 'current' } }] },
              [],
            ],
          },
          data: { words: ['a', 'b'] },
          result: ['A', 'B'],
        },
        {
          title: 'Sum a field',
          rule: {
            reduce: [
              { var: 'items' },
              { '+': [{ var: 'accumulator' }, { var: 'current.qty' }] },
              0,
            ],
          },
          data: { items: [{ qty: 2 }, { qty: 3 }] },
          result: 5,
        },
        {
          title: 'Without initial value',
          rule: {
            reduce: [[1, 2, 3], { '+': [{ var: 'accumulator' }, { var: 'current' }] }],
          },
          result: 6,
          note: 'The accumulator starts as null, which coerces to 0 in arithmetic',
        },
      ],
      notes: [
        '{"var": "current"} = current element',
        '{"var": "accumulator"} = running result',
        'No iteration index is available inside reduce',
        'Initial value is optional: when omitted the accumulator starts as null (0 in arithmetic, null for an empty array), so pass one for anything but sums',
      ],
      seeAlso: ['map', 'filter'],
    },
    ui: {
      icon: 'boxes',
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
              required: false,
              helpText: 'Starting value for the accumulator (null when omitted)',
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
        'Returns true if the condition returns truthy for every element. Returns false for an empty array (JSONLogic-compatible; not vacuous truth).',
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
          result: false,
          note: 'JSONLogic-compatible: all of nothing is false, not vacuous truth',
        },
      ],
      notes: [
        'Short-circuits: stops on first false',
        'Empty array returns false (JSONLogic-compatible; not vacuous truth)',
        '{"var": ""} = current element',
      ],
      seeAlso: ['some', 'none', 'filter'],
    },
    ui: {
      icon: 'check',
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
          example: '{"val": [[1], "index"]}',
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
          example: '{"val": [[1], "index"]}',
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
      icon: 'circle-x',
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
          example: '{"val": [[1], "index"]}',
          description: 'Zero-based index of the current element (0, 1, 2...)',
        },
      ],
    },
  },
};
