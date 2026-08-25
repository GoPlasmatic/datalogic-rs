/**
 * Object Operators
 *
 * Object take-apart operations: keys, values, entries.
 * The read-side complement of templating's computed-key object
 * construction; entries turns any object into rows the array
 * vocabulary (map, filter, group_by, ...) can iterate.
 */

import type { Operator } from '../operators.types';

export const objectOperators: Record<string, Operator> = {
  keys: {
    name: 'keys',
    label: 'Keys',
    category: 'object',
    description: "List an object's keys",
    arity: {
      type: 'unary',
      args: [
        { name: 'object', label: 'Object', type: 'any', required: true },
      ],
    },
    help: {
      summary: "List an object's keys as an array of strings",
      details:
        'Returns the keys of the object in stored order. Null input yields an empty array; any other non-object input is an error.',
      returnType: 'array',
      examples: [
        {
          title: 'Keys of an object',
          rule: { keys: [{ var: 'scores' }] },
          data: { scores: { alice: 10, bob: 7 } },
          result: ['alice', 'bob'],
        },
      ],
      notes: [
        'Keys come out in stored order',
        'Null input yields []',
      ],
      seeAlso: ['values', 'entries'],
    },
    ui: {
      icon: 'tag',
      shortLabel: 'keys',
      nodeType: 'operator',
    },
  },

  values: {
    name: 'values',
    label: 'Values',
    category: 'object',
    description: "List an object's values",
    arity: {
      type: 'unary',
      args: [
        { name: 'object', label: 'Object', type: 'any', required: true },
      ],
    },
    help: {
      summary: "List an object's values as an array",
      details:
        'Returns the values of the object in stored order. Null input yields an empty array; any other non-object input is an error.',
      returnType: 'array',
      examples: [
        {
          title: 'Values of an object',
          rule: { values: [{ var: 'scores' }] },
          data: { scores: { alice: 10, bob: 7 } },
          result: [10, 7],
        },
        {
          title: 'Sum values',
          rule: {
            reduce: [
              { values: [{ var: 'scores' }] },
              { '+': [{ var: 'accumulator' }, { var: 'current' }] },
              0,
            ],
          },
          data: { scores: { alice: 10, bob: 7 } },
          result: 17,
        },
      ],
      notes: [
        'Values come out in stored order',
        'Null input yields []',
      ],
      seeAlso: ['keys', 'entries'],
    },
    ui: {
      icon: 'list',
      shortLabel: 'values',
      nodeType: 'operator',
    },
  },

  entries: {
    name: 'entries',
    label: 'Entries',
    category: 'object',
    description: 'Turn an object into {key, value} rows',
    arity: {
      type: 'unary',
      args: [
        { name: 'object', label: 'Object', type: 'any', required: true },
      ],
    },
    help: {
      summary: 'Turn an object into an array of {key, value} rows',
      details:
        'Returns one {key, value} object per entry, in stored order, as rows the array vocabulary (map, filter, group_by) can iterate. Null input yields an empty array; any other non-object input is an error.',
      returnType: 'array',
      examples: [
        {
          title: 'Entries of an object',
          rule: { entries: [{ var: 'scores' }] },
          data: { scores: { alice: 10, bob: 7 } },
          result: [
            { key: 'alice', value: 10 },
            { key: 'bob', value: 7 },
          ],
        },
        {
          title: 'Iterate with map',
          rule: {
            map: [
              { entries: [{ var: 'scores' }] },
              { cat: [{ var: 'key' }, ': ', { var: 'value' }] },
            ],
          },
          data: { scores: { alice: 10, bob: 7 } },
          result: ['alice: 10', 'bob: 7'],
        },
      ],
      notes: [
        'Rows come out in stored order',
        'Null input yields []',
      ],
      seeAlso: ['keys', 'values', 'map'],
    },
    ui: {
      icon: 'braces',
      shortLabel: 'entries',
      nodeType: 'operator',
    },
  },
};
