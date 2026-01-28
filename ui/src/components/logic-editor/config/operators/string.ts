/**
 * String Operators
 *
 * Text manipulation operations.
 * - cat: Concatenate strings
 * - substr: Extract substring
 * - in: Check if substring exists
 * - length: Get string length
 * - starts_with, ends_with: Check prefix/suffix
 * - upper, lower, trim: Transform strings
 * - split: Split string into array
 */

import type { Operator } from '../operators.types';

export const stringOperators: Record<string, Operator> = {
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

  length: {
    name: 'length',
    label: 'Length',
    category: 'string',
    description: 'Get length of string or array',
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
          description: 'String or array',
        },
      ],
    },
    help: {
      summary: 'Return the length of a string or array',
      details: 'Returns the number of characters in a string or elements in an array.',
      returnType: 'number',
      examples: [
        {
          title: 'String length',
          rule: { length: 'Hello' },
          result: 5,
        },
        {
          title: 'Array length',
          rule: { length: [1, 2, 3, 4] },
          result: 4,
        },
        {
          title: 'Empty string',
          rule: { length: '' },
          result: 0,
        },
        {
          title: 'With variable',
          rule: { length: { var: 'items' } },
          data: { items: ['a', 'b', 'c'] },
          result: 3,
        },
      ],
      notes: ['Works with strings and arrays', 'Returns 0 for empty string/array'],
      seeAlso: ['substr', 'slice'],
    },
    ui: {
      icon: 'ruler',
      shortLabel: 'len',
      nodeType: 'operator',
    },
  },

  starts_with: {
    name: 'starts_with',
    label: 'Starts With',
    category: 'string',
    description: 'Check if string starts with prefix',
    arity: {
      type: 'binary',
      min: 2,
      max: 2,
      args: [
        { name: 'string', label: 'String', type: 'string', required: true },
        { name: 'prefix', label: 'Prefix', type: 'string', required: true },
      ],
    },
    help: {
      summary: 'Check if a string starts with the given prefix',
      returnType: 'boolean',
      examples: [
        {
          title: 'Matches prefix',
          rule: { starts_with: ['Hello World', 'Hello'] },
          result: true,
        },
        {
          title: 'No match',
          rule: { starts_with: ['Hello World', 'World'] },
          result: false,
        },
        {
          title: 'With variable',
          rule: { starts_with: [{ var: 'url' }, 'https://'] },
          data: { url: 'https://example.com' },
          result: true,
        },
      ],
      notes: ['Case-sensitive', 'Empty prefix always matches'],
      seeAlso: ['ends_with', 'in'],
    },
    ui: {
      icon: 'arrow-right-from-line',
      shortLabel: 'starts',
      nodeType: 'operator',
    },
  },

  ends_with: {
    name: 'ends_with',
    label: 'Ends With',
    category: 'string',
    description: 'Check if string ends with suffix',
    arity: {
      type: 'binary',
      min: 2,
      max: 2,
      args: [
        { name: 'string', label: 'String', type: 'string', required: true },
        { name: 'suffix', label: 'Suffix', type: 'string', required: true },
      ],
    },
    help: {
      summary: 'Check if a string ends with the given suffix',
      returnType: 'boolean',
      examples: [
        {
          title: 'Matches suffix',
          rule: { ends_with: ['Hello World', 'World'] },
          result: true,
        },
        {
          title: 'No match',
          rule: { ends_with: ['Hello World', 'Hello'] },
          result: false,
        },
        {
          title: 'File extension',
          rule: { ends_with: [{ var: 'filename' }, '.pdf'] },
          data: { filename: 'report.pdf' },
          result: true,
        },
      ],
      notes: ['Case-sensitive', 'Empty suffix always matches'],
      seeAlso: ['starts_with', 'in'],
    },
    ui: {
      icon: 'arrow-right-to-line',
      shortLabel: 'ends',
      nodeType: 'operator',
    },
  },

  upper: {
    name: 'upper',
    label: 'Uppercase',
    category: 'string',
    description: 'Convert string to uppercase',
    arity: {
      type: 'unary',
      min: 1,
      max: 1,
      args: [
        { name: 'string', label: 'String', type: 'string', required: true },
      ],
    },
    help: {
      summary: 'Convert a string to all uppercase letters',
      returnType: 'string',
      examples: [
        {
          title: 'Simple uppercase',
          rule: { upper: 'hello' },
          result: 'HELLO',
        },
        {
          title: 'Mixed case',
          rule: { upper: 'Hello World' },
          result: 'HELLO WORLD',
        },
        {
          title: 'With variable',
          rule: { upper: { var: 'name' } },
          data: { name: 'alice' },
          result: 'ALICE',
        },
      ],
      notes: ['Only affects alphabetic characters', 'Numbers and symbols unchanged'],
      seeAlso: ['lower', 'trim'],
    },
    ui: {
      icon: 'case-upper',
      shortLabel: 'UP',
      nodeType: 'operator',
    },
  },

  lower: {
    name: 'lower',
    label: 'Lowercase',
    category: 'string',
    description: 'Convert string to lowercase',
    arity: {
      type: 'unary',
      min: 1,
      max: 1,
      args: [
        { name: 'string', label: 'String', type: 'string', required: true },
      ],
    },
    help: {
      summary: 'Convert a string to all lowercase letters',
      returnType: 'string',
      examples: [
        {
          title: 'Simple lowercase',
          rule: { lower: 'HELLO' },
          result: 'hello',
        },
        {
          title: 'Mixed case',
          rule: { lower: 'Hello World' },
          result: 'hello world',
        },
        {
          title: 'With variable',
          rule: { lower: { var: 'email' } },
          data: { email: 'User@Example.COM' },
          result: 'user@example.com',
        },
      ],
      notes: ['Only affects alphabetic characters', 'Useful for case-insensitive comparison'],
      seeAlso: ['upper', 'trim'],
    },
    ui: {
      icon: 'case-lower',
      shortLabel: 'low',
      nodeType: 'operator',
    },
  },

  trim: {
    name: 'trim',
    label: 'Trim',
    category: 'string',
    description: 'Remove whitespace from both ends',
    arity: {
      type: 'unary',
      min: 1,
      max: 1,
      args: [
        { name: 'string', label: 'String', type: 'string', required: true },
      ],
    },
    help: {
      summary: 'Remove leading and trailing whitespace from a string',
      returnType: 'string',
      examples: [
        {
          title: 'Trim spaces',
          rule: { trim: '  hello  ' },
          result: 'hello',
        },
        {
          title: 'Trim tabs and newlines',
          rule: { trim: '\n\thello\t\n' },
          result: 'hello',
        },
        {
          title: 'With variable',
          rule: { trim: { var: 'input' } },
          data: { input: '  user input  ' },
          result: 'user input',
        },
      ],
      notes: [
        'Removes spaces, tabs, newlines',
        'Does not affect whitespace in the middle',
      ],
      seeAlso: ['upper', 'lower'],
    },
    ui: {
      icon: 'space',
      shortLabel: 'trim',
      nodeType: 'operator',
    },
  },

  split: {
    name: 'split',
    label: 'Split',
    category: 'string',
    description: 'Split string into array by delimiter',
    arity: {
      type: 'binary',
      min: 2,
      max: 2,
      args: [
        { name: 'string', label: 'String', type: 'string', required: true },
        { name: 'delimiter', label: 'Delimiter', type: 'string', required: true },
      ],
    },
    help: {
      summary: 'Split a string into an array using a delimiter',
      returnType: 'array',
      examples: [
        {
          title: 'Split by comma',
          rule: { split: ['a,b,c', ','] },
          result: ['a', 'b', 'c'],
        },
        {
          title: 'Split by space',
          rule: { split: ['Hello World', ' '] },
          result: ['Hello', 'World'],
        },
        {
          title: 'Split path',
          rule: { split: [{ var: 'path' }, '/'] },
          data: { path: 'home/user/docs' },
          result: ['home', 'user', 'docs'],
        },
        {
          title: 'Split into characters',
          rule: { split: ['abc', ''] },
          result: ['a', 'b', 'c'],
          note: 'Empty delimiter splits into chars',
        },
      ],
      notes: ['Delimiter is not included in results', 'Empty string splits into characters'],
      seeAlso: ['cat', 'substr'],
    },
    ui: {
      icon: 'split',
      shortLabel: 'split',
      nodeType: 'operator',
    },
  },
};
