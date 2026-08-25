/**
 * Array Manipulation Operators
 *
 * Array manipulation operations: merge, sort, slice, group_by, distinct
 */

import type { Operator } from '../operators.types';

export const arrayManipulationOperators: Record<string, Operator> = {
  merge: {
    name: 'merge',
    label: 'Merge',
    category: 'array',
    description: 'Combine multiple arrays into one',
    arity: {
      type: 'nary',
      min: 1,
      args: [
        {
          name: 'array',
          label: 'Array',
          type: 'array',
          required: true,
          repeatable: true,
        },
      ],
    },
    help: {
      summary: 'Merge multiple arrays into a single array',
      details:
        'Concatenates all input arrays into a single flat array. Non-array values are wrapped in an array.',
      returnType: 'array',
      examples: [
        {
          title: 'Merge two arrays',
          rule: { merge: [[1, 2], [3, 4]] },
          result: [1, 2, 3, 4],
        },
        {
          title: 'Merge multiple',
          rule: { merge: [[1], [2, 3], [4, 5, 6]] },
          result: [1, 2, 3, 4, 5, 6],
        },
        {
          title: 'With variables',
          rule: { merge: [{ var: 'arr1' }, { var: 'arr2' }] },
          data: { arr1: ['a', 'b'], arr2: ['c', 'd'] },
          result: ['a', 'b', 'c', 'd'],
        },
        {
          title: 'Single value wrapped',
          rule: { merge: [[1, 2], 3, [4]] },
          result: [1, 2, 3, 4],
          note: 'Non-array 3 is wrapped',
        },
        {
          title: 'Nulls are dropped',
          rule: { merge: [[1, null, 2], null, 3] },
          result: [1, 2, 3],
        },
      ],
      notes: [
        'Flattens one level only',
        'Non-arrays are wrapped in array',
        'null elements (and null arguments) are dropped',
        'Accepts 1 or more arguments',
      ],
      seeAlso: ['slice', 'map'],
    },
    ui: {
      icon: 'git-merge',
      shortLabel: 'merge',
      nodeType: 'operator',
    },
  },

  sort: {
    name: 'sort',
    label: 'Sort',
    category: 'array',
    description: 'Sort array elements',
    arity: {
      type: 'range',
      min: 1,
      max: 3,
      args: [
        { name: 'array', label: 'Array', type: 'array', required: true },
        {
          name: 'ascending',
          label: 'Ascending',
          type: 'boolean',
          required: false,
          description: 'true (default) for ascending, false for descending',
        },
        {
          name: 'expression',
          label: 'Sort Key',
          type: 'expression',
          required: false,
          description: 'Expression to extract the sort key from each element',
        },
      ],
    },
    help: {
      summary: 'Sort array elements in ascending or descending order',
      details:
        'Sorts the array: [array, ascending?, keyExpression?]. The second argument is the direction (true = ascending, the default). With a key expression as the third argument, elements are ordered by the extracted key.',
      returnType: 'array',
      examples: [
        {
          title: 'Sort numbers',
          rule: { sort: [[3, 1, 4, 1, 5]] },
          result: [1, 1, 3, 4, 5],
        },
        {
          title: 'Sort strings',
          rule: { sort: [['banana', 'apple', 'cherry']] },
          result: ['apple', 'banana', 'cherry'],
        },
        {
          title: 'Descending',
          rule: { sort: [[3, 1, 2], false] },
          result: [3, 2, 1],
          note: 'Second argument is the direction',
        },
        {
          title: 'Sort by field',
          rule: {
            sort: [
              { var: 'users' },
              true,
              { var: 'age' },
            ],
          },
          data: {
            users: [
              { name: 'Bob', age: 30 },
              { name: 'Alice', age: 25 },
              { name: 'Carol', age: 35 },
            ],
          },
          result: [
            { name: 'Alice', age: 25 },
            { name: 'Bob', age: 30 },
            { name: 'Carol', age: 35 },
          ],
          note: 'The key expression is the third argument, after the direction',
        },
        {
          title: 'Sort by field, descending',
          rule: { sort: [{ var: 'users' }, false, { var: 'age' }] },
          data: {
            users: [
              { name: 'Bob', age: 30 },
              { name: 'Alice', age: 25 },
              { name: 'Carol', age: 35 },
            ],
          },
          result: [
            { name: 'Carol', age: 35 },
            { name: 'Bob', age: 30 },
            { name: 'Alice', age: 25 },
          ],
        },
      ],
      notes: [
        'Signature: [array, ascending?, keyExpression?]',
        'Returns a new sorted array; original unchanged',
        'A key expression in the second position is read as the direction and ignored',
      ],
      seeAlso: ['filter', 'map'],
    },
    ui: {
      icon: 'layers',
      shortLabel: 'sort',
      nodeType: 'operator',
    },
  },

  slice: {
    name: 'slice',
    label: 'Slice',
    category: 'array',
    description: 'Extract portion of array or string',
    arity: {
      type: 'range',
      min: 1,
      max: 4,
      args: [
        {
          name: 'value',
          label: 'Value',
          type: 'any',
          required: true,
          description: 'Array or string',
        },
        {
          name: 'start',
          label: 'Start',
          type: 'number',
          required: false,
          description: 'Start index (inclusive); null or omitted = from the beginning',
        },
        {
          name: 'end',
          label: 'End',
          type: 'number',
          required: false,
          description: 'End index (exclusive); null or omitted = to the end',
        },
        {
          name: 'step',
          label: 'Step',
          type: 'number',
          required: false,
          description: 'Stride (non-zero); negative walks backwards',
        },
      ],
    },
    help: {
      summary: 'Extract a portion of an array or string',
      details:
        'Returns elements from start index up to (but not including) end index, optionally every step-th element: [value, start?, end?, step?]. Negative indices count from the end; start and end may be null to leave that side open.',
      returnType: 'same',
      examples: [
        {
          title: 'Slice array',
          rule: { slice: [[1, 2, 3, 4, 5], 1, 4] },
          result: [2, 3, 4],
        },
        {
          title: 'From start',
          rule: { slice: [[1, 2, 3, 4, 5], 2] },
          result: [3, 4, 5],
          note: 'No end = rest of array',
        },
        {
          title: 'Negative index',
          rule: { slice: [[1, 2, 3, 4, 5], -3] },
          result: [3, 4, 5],
          note: '-3 = last 3 elements',
        },
        {
          title: 'Slice string',
          rule: { slice: ['Hello World', 0, 5] },
          result: 'Hello',
        },
        {
          title: 'Every other element',
          rule: { slice: [[1, 2, 3, 4, 5], 0, 5, 2] },
          result: [1, 3, 5],
          note: 'Fourth argument is the step',
        },
        {
          title: 'Reverse',
          rule: { slice: [[1, 2, 3, 4, 5], null, null, -1] },
          result: [5, 4, 3, 2, 1],
          note: 'Open-ended start/end with a negative step',
        },
      ],
      notes: [
        'Signature: [value, start?, end?, step?]',
        'Start is inclusive, end is exclusive; either may be null (open-ended)',
        'Negative indices count from end; a negative step walks backwards',
        'Works with both arrays and strings; null input returns null',
        'Non-numeric indices are an InvalidArguments error',
      ],
      seeAlso: ['substr', 'merge'],
    },
    ui: {
      icon: 'list',
      shortLabel: 'slice',
      nodeType: 'operator',
    },
  },

  group_by: {
    name: 'group_by',
    label: 'Group By',
    category: 'array',
    description: 'Group array elements by a computed key',
    arity: {
      type: 'binary',
      args: [
        { name: 'array', label: 'Array', type: 'array', required: true },
        {
          name: 'expression',
          label: 'Group Key',
          type: 'expression',
          required: true,
          description: "Expression producing each element's group key",
        },
      ],
    },
    help: {
      summary: 'Collapse an array into {key, items} groups on a computed key',
      details:
        'Evaluates the key expression once per element and collects elements sharing a key. Returns an array of {key, items} rows ordered by first key occurrence, so the result composes with map, filter, and sort.',
      returnType: 'array',
      examples: [
        {
          title: 'Group by field',
          rule: { group_by: [{ var: 'tasks' }, { var: 'status' }] },
          data: {
            tasks: [
              { id: 1, status: 'open' },
              { id: 2, status: 'done' },
              { id: 3, status: 'open' },
            ],
          },
          result: [
            { key: 'open', items: [{ id: 1, status: 'open' }, { id: 3, status: 'open' }] },
            { key: 'done', items: [{ id: 2, status: 'done' }] },
          ],
        },
        {
          title: 'Group by computed key',
          rule: { group_by: [[1, 2, 3, 4, 5], { '%': [{ var: '' }, 2] }] },
          result: [
            { key: 1, items: [1, 3, 5] },
            { key: 0, items: [2, 4] },
          ],
        },
      ],
      notes: [
        'Keys keep their evaluated type: numbers, booleans, and objects group by deep equality',
        'Groups are ordered by first key occurrence (deterministic output)',
        'Null or empty input yields []',
      ],
      seeAlso: ['distinct', 'map', 'sort'],
    },
    ui: {
      icon: 'boxes',
      shortLabel: 'group',
      nodeType: 'operator',
    },
  },

  distinct: {
    name: 'distinct',
    label: 'Distinct',
    category: 'array',
    description: 'Remove duplicate elements',
    arity: {
      type: 'range',
      min: 1,
      max: 2,
      args: [
        { name: 'array', label: 'Array', type: 'array', required: true },
        {
          name: 'expression',
          label: 'Key',
          type: 'expression',
          required: false,
          description: 'Optional expression to deduplicate by a computed key',
        },
      ],
    },
    help: {
      summary: 'Drop duplicate elements, by value or by a computed key',
      details:
        'Without a key expression, removes elements that strictly deep-equal an earlier one. With a key expression, keeps the first element per distinct key. First occurrence wins, so input order is preserved.',
      returnType: 'array',
      examples: [
        {
          title: 'Dedup by value',
          rule: { distinct: [[3, 1, 3, 2, 1]] },
          result: [3, 1, 2],
        },
        {
          title: 'Dedup by key',
          rule: { distinct: [{ var: 'rows' }, { var: 'id' }] },
          data: {
            rows: [
              { id: 1, rev: 'a' },
              { id: 2, rev: 'b' },
              { id: 1, rev: 'c' },
            ],
          },
          result: [
            { id: 1, rev: 'a' },
            { id: 2, rev: 'b' },
          ],
        },
      ],
      notes: [
        'Uses strict deep equality: 1 and "1" stay distinct',
        'First occurrence wins; order is preserved',
        'Null or empty input yields []',
      ],
      seeAlso: ['group_by', 'filter'],
    },
    ui: {
      icon: 'list',
      shortLabel: 'distinct',
      nodeType: 'operator',
    },
  },
};
