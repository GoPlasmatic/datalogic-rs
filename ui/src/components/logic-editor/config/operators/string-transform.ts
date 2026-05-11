/**
 * String Transform Operators
 *
 * String inspection and transformation: length, starts_with, ends_with, upper, lower, trim, split
 */

import type { Operator } from '../operators.types';

export const stringTransformOperators: Record<string, Operator> = {
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
