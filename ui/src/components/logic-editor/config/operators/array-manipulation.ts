/**
 * Array Manipulation Operators
 *
 * Array manipulation operations: merge, sort, slice
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
      ],
      notes: [
        'Flattens one level only',
        'Non-arrays are wrapped in array',
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
      max: 2,
      args: [
        { name: 'array', label: 'Array', type: 'array', required: true },
        {
          name: 'expression',
          label: 'Sort Key',
          type: 'expression',
          required: false,
          description: 'Expression to extract sort key',
        },
      ],
    },
    help: {
      summary: 'Sort array elements in ascending order',
      details:
        'Sorts the array. With no expression, sorts by natural order. With an expression, sorts by the extracted key.',
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
          title: 'Sort by field',
          rule: {
            sort: [
              { var: 'users' },
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
        },
      ],
      notes: [
        'Returns a new sorted array',
        'Original array unchanged',
        'Expression extracts the sort key',
      ],
      seeAlso: ['filter', 'map'],
    },
    ui: {
      icon: 'arrow-up-down',
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
      min: 2,
      max: 3,
      args: [
        {
          name: 'value',
          label: 'Value',
          type: 'any',
          required: true,
          description: 'Array or string',
        },
        { name: 'start', label: 'Start', type: 'number', required: true },
        {
          name: 'end',
          label: 'End',
          type: 'number',
          required: false,
          description: 'End index (exclusive)',
        },
      ],
    },
    help: {
      summary: 'Extract a portion of an array or string',
      details:
        'Returns elements from start index up to (but not including) end index. Negative indices count from the end.',
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
      ],
      notes: [
        'Start is inclusive, end is exclusive',
        'Negative indices count from end',
        'Works with both arrays and strings',
      ],
      seeAlso: ['substr', 'merge'],
    },
    ui: {
      icon: 'scissors',
      shortLabel: 'slice',
      nodeType: 'operator',
    },
  },
};
