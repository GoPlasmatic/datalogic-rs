/**
 * Basic Arithmetic Operators
 *
 * Basic mathematical operations: +, -, *, /, %
 */

import type { Operator } from '../operators.types';

export const arithmeticBasicOperators: Record<string, Operator> = {
  '+': {
    name: '+',
    label: 'Add',
    category: 'arithmetic',
    description: 'Add numbers (strings are coerced; no concatenation)',
    arity: {
      type: 'nary',
      min: 1,
      args: [
        {
          name: 'value',
          label: 'Value',
          type: 'number',
          required: true,
          repeatable: true,
        },
      ],
    },
    help: {
      summary: 'Add numbers together or convert to number',
      details:
        'With multiple arguments, adds all values together. With a single argument, converts the value to a number (unary plus), or sums it when it evaluates to an array. Strings are coerced to numbers; a non-numeric string is a NaN error, never concatenation.',
      returnType: 'number',
      examples: [
        {
          title: 'Add two numbers',
          rule: { '+': [2, 3] },
          result: 5,
        },
        {
          title: 'Add multiple numbers',
          rule: { '+': [1, 2, 3, 4] },
          result: 10,
        },
        {
          title: 'Unary plus (to number)',
          rule: { '+': ['42'] },
          result: 42,
          note: 'Converts string to number',
        },
        {
          title: 'With variables',
          rule: { '+': [{ var: 'price' }, { var: 'tax' }] },
          data: { price: 100, tax: 8.5 },
          result: 108.5,
        },
        {
          title: 'Mixed types',
          rule: { '+': [10, '5', 3] },
          result: 18,
          note: 'String "5" coerced to number',
        },
        {
          title: 'Sum an array',
          rule: { '+': [{ var: 'nums' }] },
          data: { nums: [1, 2, 3, 4] },
          result: 10,
          note: 'A single argument that evaluates to an array is folded',
        },
        {
          title: 'Non-numeric strings throw',
          rule: { '+': ['a', 'b'] },
          error: { type: 'Thrown' },
          note: 'Throws {"type": "NaN"}; use cat to join strings',
        },
      ],
      notes: [
        'Accepts 1 or more arguments (none returns 0)',
        'Single argument: converts to number, or sums an array',
        'Strings are coerced to numbers; non-numeric strings throw NaN',
        'Use "cat" for string concatenation',
      ],
      seeAlso: ['-', '*', '/', 'cat'],
    },
    ui: {
      icon: 'calculator',
      shortLabel: '+',
      nodeType: 'operator',
    },
  },

  '-': {
    name: '-',
    label: 'Subtract',
    category: 'arithmetic',
    description: 'Subtract numbers or negate a value',
    arity: {
      type: 'special',
      min: 1,
      args: [
        {
          name: 'value',
          label: 'Value',
          type: 'number',
          required: true,
          repeatable: true,
        },
      ],
    },
    help: {
      summary: 'Subtract numbers or negate a single value',
      details:
        'With one argument, negates the value. With two arguments, subtracts right from left. With three or more, performs sequential subtraction (a - b - c).',
      returnType: 'number',
      examples: [
        {
          title: 'Unary minus (negate)',
          rule: { '-': [5] },
          result: -5,
        },
        {
          title: 'Binary subtraction',
          rule: { '-': [10, 3] },
          result: 7,
        },
        {
          title: 'Sequential subtraction',
          rule: { '-': [10, 3, 2] },
          result: 5,
          note: '10 - 3 - 2 = 5',
        },
        {
          title: 'With variables',
          rule: { '-': [{ var: 'total' }, { var: 'discount' }] },
          data: { total: 100, discount: 15 },
          result: 85,
        },
      ],
      notes: [
        '1 arg: negation (-x), or a fold when it evaluates to an array ([10, 3] gives 7)',
        '2 args: subtraction (a - b)',
        '3+ args: sequential (a - b - c)',
      ],
      seeAlso: ['+', '*', '/'],
    },
    ui: {
      icon: 'calculator',
      shortLabel: '-',
      nodeType: 'operator',
    },
  },

  '*': {
    name: '*',
    label: 'Multiply',
    category: 'arithmetic',
    description: 'Multiply numbers together',
    arity: {
      type: 'nary',
      min: 1,
      args: [
        {
          name: 'value',
          label: 'Value',
          type: 'number',
          required: true,
          repeatable: true,
        },
      ],
    },
    help: {
      summary: 'Multiply all values together',
      details:
        'Multiplies all arguments together. A single argument is returned as a number, or folded when it evaluates to an array.',
      returnType: 'number',
      examples: [
        {
          title: 'Multiply two numbers',
          rule: { '*': [3, 4] },
          result: 12,
        },
        {
          title: 'Multiply multiple',
          rule: { '*': [2, 3, 4] },
          result: 24,
        },
        {
          title: 'With variables',
          rule: { '*': [{ var: 'quantity' }, { var: 'price' }] },
          data: { quantity: 5, price: 10 },
          result: 50,
        },
        {
          title: 'Product of an array',
          rule: { '*': [{ var: 'a' }] },
          data: { a: [2, 3, 4] },
          result: 24,
          note: 'A single argument that evaluates to an array is folded',
        },
      ],
      notes: [
        'Accepts 1 or more arguments (none returns 1)',
        'Strings are coerced to numbers',
      ],
      seeAlso: ['/', '+', '-'],
    },
    ui: {
      icon: 'x',
      shortLabel: '×',
      nodeType: 'operator',
    },
  },

  '/': {
    name: '/',
    label: 'Divide',
    category: 'arithmetic',
    description: 'Divide numbers left to right',
    arity: {
      type: 'nary',
      min: 1,
      args: [
        { name: 'dividend', label: 'Dividend', type: 'number', required: true },
        {
          name: 'divisor',
          label: 'Divisor',
          type: 'number',
          required: false,
          repeatable: true,
        },
      ],
    },
    help: {
      summary: 'Divide the first number by the following ones',
      details:
        'Performs division left to right (a / b / c). A single argument returns its reciprocal. Division by zero behavior depends on engine configuration (the default throws a NaN error).',
      returnType: 'number',
      examples: [
        {
          title: 'Simple division',
          rule: { '/': [10, 2] },
          result: 5,
        },
        {
          title: 'Decimal result',
          rule: { '/': [7, 2] },
          result: 3.5,
        },
        {
          title: 'With variables',
          rule: { '/': [{ var: 'total' }, { var: 'count' }] },
          data: { total: 100, count: 4 },
          result: 25,
        },
        {
          title: 'Sequential division',
          rule: { '/': [100, 5, 2] },
          result: 10,
          note: '100 / 5 / 2',
        },
        {
          title: 'Reciprocal',
          rule: { '/': [8] },
          result: 0.125,
          note: 'One argument: 1 / x',
        },
        {
          title: 'Division by zero',
          rule: { '/': [1, 0] },
          error: { type: 'Thrown' },
          note: 'Default config throws {"type": "NaN"}; division_by_zero can change this',
        },
      ],
      notes: [
        'Two or more arguments divide left to right (a / b / c)',
        'One argument returns its reciprocal',
        'Division by zero throws by default; the division_by_zero config can return null, infinity or a saturated value',
      ],
      seeAlso: ['*', '%', '+', '-'],
    },
    ui: {
      icon: 'divide',
      shortLabel: '÷',
      nodeType: 'operator',
    },
  },

  '%': {
    name: '%',
    label: 'Modulo',
    category: 'arithmetic',
    description: 'Get remainder of division',
    arity: {
      type: 'nary',
      min: 2,
      args: [
        { name: 'dividend', label: 'Dividend', type: 'number', required: true },
        {
          name: 'divisor',
          label: 'Divisor',
          type: 'number',
          required: true,
          repeatable: true,
        },
      ],
    },
    help: {
      summary: 'Get the remainder after division',
      details:
        'Returns the remainder when dividing the first number by the second. Extra arguments apply left to right ((a % b) % c).',
      returnType: 'number',
      examples: [
        {
          title: 'Remainder',
          rule: { '%': [7, 3] },
          result: 1,
          note: '7 = 3*2 + 1',
        },
        {
          title: 'Even check',
          rule: { '%': [{ var: 'n' }, 2] },
          data: { n: 4 },
          result: 0,
          note: '0 means even',
        },
        {
          title: 'Odd check',
          rule: { '%': [{ var: 'n' }, 2] },
          data: { n: 5 },
          result: 1,
          note: '1 means odd',
        },
        {
          title: 'Sequential modulo',
          rule: { '%': [100, 7, 3] },
          result: 2,
          note: '(100 % 7) % 3',
        },
      ],
      notes: [
        'Useful for even/odd checks',
        'Useful for cycling through values',
        'At least 2 arguments; extra arguments fold left to right',
      ],
      seeAlso: ['/', '*'],
    },
    ui: {
      icon: 'divide',
      shortLabel: '%',
      nodeType: 'operator',
    },
  },
};
