/**
 * Core String Operators
 *
 * Basic string operations: cat, substr, in
 */

import type { Operator } from '../operators.types';

export const stringCoreOperators: Record<string, Operator> = {
  cat: {
    name: 'cat',
    label: 'Concatenate',
    category: 'string',
    description: 'Join strings together',
    arity: {
      type: 'nary',
      min: 1,
      args: [
        {
          name: 'value',
          label: 'Value',
          type: 'string',
          required: true,
          repeatable: true,
        },
      ],
    },
    help: {
      summary: 'Concatenate multiple values into a single string',
      details:
        'Joins all arguments together as strings. Non-string values are converted to strings.',
      returnType: 'string',
      examples: [
        {
          title: 'Join strings',
          rule: { cat: ['Hello', ' ', 'World'] },
          result: 'Hello World',
        },
        {
          title: 'With variables',
          rule: { cat: ['Hello, ', { var: 'name' }, '!'] },
          data: { name: 'Alice' },
          result: 'Hello, Alice!',
        },
        {
          title: 'Mixed types',
          rule: { cat: ['Count: ', 42] },
          result: 'Count: 42',
          note: 'Numbers are converted to strings',
        },
        {
          title: 'Build path',
          rule: { cat: [{ var: 'base' }, '/', { var: 'file' }] },
          data: { base: '/home', file: 'data.json' },
          result: '/home/data.json',
        },
      ],
      notes: [
        'All values are converted to strings',
        'Accepts 1 or more arguments',
        'Use + for numeric addition',
      ],
      seeAlso: ['substr', 'split'],
    },
    ui: {
      icon: 'text',
      shortLabel: 'cat',
      nodeType: 'operator',
    },
  },

  substr: {
    name: 'substr',
    label: 'Substring',
    category: 'string',
    description: 'Extract part of a string',
    arity: {
      type: 'range',
      min: 2,
      max: 3,
      args: [
        { name: 'string', label: 'String', type: 'string', required: true },
        { name: 'start', label: 'Start', type: 'number', required: true },
        {
          name: 'length',
          label: 'Length',
          type: 'number',
          required: false,
          description: 'Number of characters (omit for rest of string)',
        },
      ],
    },
    help: {
      summary: 'Extract a portion of a string',
      details:
        'Returns a substring starting at the given index. If length is provided, returns that many characters; otherwise returns to the end of the string. Negative start counts from the end.',
      returnType: 'string',
      examples: [
        {
          title: 'From start index',
          rule: { substr: ['Hello World', 0, 5] },
          result: 'Hello',
        },
        {
          title: 'Middle of string',
          rule: { substr: ['Hello World', 6] },
          result: 'World',
          note: 'No length = rest of string',
        },
        {
          title: 'Negative start',
          rule: { substr: ['Hello World', -5] },
          result: 'World',
          note: '-5 = 5 chars from end',
        },
        {
          title: 'With variable',
          rule: { substr: [{ var: 'text' }, 0, 10] },
          data: { text: 'This is a long sentence' },
          result: 'This is a ',
        },
      ],
      notes: [
        'Index is 0-based',
        'Negative start counts from end',
        'Length is optional (defaults to rest of string)',
      ],
      seeAlso: ['cat', 'split', 'length'],
    },
    ui: {
      icon: 'scissors',
      shortLabel: 'sub',
      nodeType: 'operator',
    },
  },

  in: {
    name: 'in',
    label: 'Contains',
    category: 'string',
    description: 'Check if string contains substring or array contains element',
    arity: {
      type: 'binary',
      min: 2,
      max: 2,
      args: [
        {
          name: 'needle',
          label: 'Search For',
          type: 'any',
          required: true,
          description: 'Value to search for',
        },
        {
          name: 'haystack',
          label: 'Search In',
          type: 'any',
          required: true,
          description: 'String or array to search in',
        },
      ],
    },
    help: {
      summary: 'Check if a value exists within a string or array',
      details:
        'For strings, checks if the substring exists. For arrays, checks if the element is present.',
      returnType: 'boolean',
      examples: [
        {
          title: 'Substring check',
          rule: { in: ['World', 'Hello World'] },
          result: true,
        },
        {
          title: 'Not found',
          rule: { in: ['xyz', 'Hello World'] },
          result: false,
        },
        {
          title: 'Array contains',
          rule: { in: ['b', ['a', 'b', 'c']] },
          result: true,
        },
        {
          title: 'With variable',
          rule: { in: [{ var: 'role' }, ['admin', 'moderator']] },
          data: { role: 'admin' },
          result: true,
        },
      ],
      notes: [
        'Works with both strings and arrays',
        'Case-sensitive for strings',
        'Exact match for array elements',
      ],
      seeAlso: ['starts_with', 'ends_with'],
    },
    ui: {
      icon: 'search',
      shortLabel: 'in',
      nodeType: 'operator',
    },
  },
};
