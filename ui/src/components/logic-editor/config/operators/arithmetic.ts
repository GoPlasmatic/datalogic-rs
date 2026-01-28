/**
 * Arithmetic Operators
 *
 * Mathematical operations.
 * - Basic: +, -, *, /, %
 * - Aggregate: max, min
 * - Unary: abs, ceil, floor
 */

import type { Operator } from '../operators.types';

export const arithmeticOperators: Record<string, Operator> = {
  '+': {
    name: '+',
    label: 'Add',
    category: 'arithmetic',
    description: 'Add numbers or concatenate values',
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
        'With multiple arguments, adds all values together. With a single argument, converts the value to a number (unary plus). Strings are coerced to numbers when possible.',
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
      ],
      notes: [
        'Accepts 1 or more arguments',
        'Single argument: converts to number',
        'Strings are coerced to numbers',
        'Use "cat" for string concatenation',
      ],
      seeAlso: ['-', '*', '/', 'cat'],
    },
    ui: {
      icon: 'plus',
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
        '1 arg: negation (-x)',
        '2 args: subtraction (a - b)',
        '3+ args: sequential (a - b - c)',
      ],
      seeAlso: ['+', '*', '/'],
    },
    ui: {
      icon: 'minus',
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
      type: 'variadic',
      min: 2,
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
      details: 'Multiplies all arguments together. Requires at least 2 arguments.',
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
      ],
      notes: ['Requires at least 2 arguments', 'Strings are coerced to numbers'],
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
    description: 'Divide first number by second',
    arity: {
      type: 'binary',
      min: 2,
      max: 2,
      args: [
        { name: 'dividend', label: 'Dividend', type: 'number', required: true },
        { name: 'divisor', label: 'Divisor', type: 'number', required: true },
      ],
    },
    help: {
      summary: 'Divide the first number by the second',
      details: 'Performs division. Division by zero behavior depends on engine configuration.',
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
      ],
      notes: [
        'Exactly 2 arguments required',
        'Division by zero may return error or infinity',
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
      type: 'binary',
      min: 2,
      max: 2,
      args: [
        { name: 'dividend', label: 'Dividend', type: 'number', required: true },
        { name: 'divisor', label: 'Divisor', type: 'number', required: true },
      ],
    },
    help: {
      summary: 'Get the remainder after division',
      details: 'Returns the remainder when dividing the first number by the second.',
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
      ],
      notes: ['Useful for even/odd checks', 'Useful for cycling through values'],
      seeAlso: ['/', '*'],
    },
    ui: {
      icon: 'percent',
      shortLabel: '%',
      nodeType: 'operator',
    },
  },

  max: {
    name: 'max',
    label: 'Maximum',
    category: 'arithmetic',
    description: 'Get the largest value',
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
      summary: 'Return the largest value from the arguments',
      details:
        'Compares all arguments and returns the maximum. Can accept an array or individual arguments.',
      returnType: 'number',
      examples: [
        {
          title: 'Multiple values',
          rule: { max: [1, 5, 3] },
          result: 5,
        },
        {
          title: 'Two values',
          rule: { max: [{ var: 'a' }, { var: 'b' }] },
          data: { a: 10, b: 20 },
          result: 20,
        },
        {
          title: 'With array',
          rule: { max: { var: 'scores' } },
          data: { scores: [85, 92, 78, 95] },
          result: 95,
        },
      ],
      notes: ['Accepts 1 or more arguments', 'Can operate on an array'],
      seeAlso: ['min'],
    },
    ui: {
      icon: 'arrow-up',
      shortLabel: 'max',
      nodeType: 'operator',
    },
  },

  min: {
    name: 'min',
    label: 'Minimum',
    category: 'arithmetic',
    description: 'Get the smallest value',
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
      summary: 'Return the smallest value from the arguments',
      details:
        'Compares all arguments and returns the minimum. Can accept an array or individual arguments.',
      returnType: 'number',
      examples: [
        {
          title: 'Multiple values',
          rule: { min: [5, 1, 3] },
          result: 1,
        },
        {
          title: 'Two values',
          rule: { min: [{ var: 'a' }, { var: 'b' }] },
          data: { a: 10, b: 20 },
          result: 10,
        },
        {
          title: 'With array',
          rule: { min: { var: 'prices' } },
          data: { prices: [29.99, 19.99, 39.99] },
          result: 19.99,
        },
      ],
      notes: ['Accepts 1 or more arguments', 'Can operate on an array'],
      seeAlso: ['max'],
    },
    ui: {
      icon: 'arrow-down',
      shortLabel: 'min',
      nodeType: 'operator',
    },
  },

  abs: {
    name: 'abs',
    label: 'Absolute',
    category: 'arithmetic',
    description: 'Get absolute value (remove sign)',
    arity: {
      type: 'unary',
      min: 1,
      max: 1,
      args: [
        { name: 'value', label: 'Value', type: 'number', required: true },
      ],
    },
    help: {
      summary: 'Return the absolute value (always positive)',
      details: 'Removes the sign from a number, returning its distance from zero.',
      returnType: 'number',
      examples: [
        {
          title: 'Negative number',
          rule: { abs: [-5] },
          result: 5,
        },
        {
          title: 'Positive number',
          rule: { abs: [5] },
          result: 5,
        },
        {
          title: 'Zero',
          rule: { abs: [0] },
          result: 0,
        },
        {
          title: 'With variable',
          rule: { abs: [{ var: 'difference' }] },
          data: { difference: -15 },
          result: 15,
        },
      ],
      notes: ['Always returns a non-negative number', 'Useful for distance calculations'],
      seeAlso: ['ceil', 'floor'],
    },
    ui: {
      icon: 'bar-chart-2',
      shortLabel: 'abs',
      nodeType: 'operator',
    },
  },

  ceil: {
    name: 'ceil',
    label: 'Ceiling',
    category: 'arithmetic',
    description: 'Round up to nearest integer',
    arity: {
      type: 'unary',
      min: 1,
      max: 1,
      args: [
        { name: 'value', label: 'Value', type: 'number', required: true },
      ],
    },
    help: {
      summary: 'Round a number up to the nearest integer',
      details: 'Returns the smallest integer greater than or equal to the given number.',
      returnType: 'number',
      examples: [
        {
          title: 'Positive decimal',
          rule: { ceil: [4.2] },
          result: 5,
        },
        {
          title: 'Negative decimal',
          rule: { ceil: [-4.2] },
          result: -4,
          note: 'Rounds toward positive infinity',
        },
        {
          title: 'Already integer',
          rule: { ceil: [5] },
          result: 5,
        },
        {
          title: 'With variable',
          rule: { ceil: [{ var: 'average' }] },
          data: { average: 3.7 },
          result: 4,
        },
      ],
      notes: ['Always rounds toward positive infinity', 'Use floor to round down'],
      seeAlso: ['floor', 'abs'],
    },
    ui: {
      icon: 'arrow-up-to-line',
      shortLabel: 'ceil',
      nodeType: 'operator',
    },
  },

  floor: {
    name: 'floor',
    label: 'Floor',
    category: 'arithmetic',
    description: 'Round down to nearest integer',
    arity: {
      type: 'unary',
      min: 1,
      max: 1,
      args: [
        { name: 'value', label: 'Value', type: 'number', required: true },
      ],
    },
    help: {
      summary: 'Round a number down to the nearest integer',
      details: 'Returns the largest integer less than or equal to the given number.',
      returnType: 'number',
      examples: [
        {
          title: 'Positive decimal',
          rule: { floor: [4.9] },
          result: 4,
        },
        {
          title: 'Negative decimal',
          rule: { floor: [-4.2] },
          result: -5,
          note: 'Rounds toward negative infinity',
        },
        {
          title: 'Already integer',
          rule: { floor: [5] },
          result: 5,
        },
        {
          title: 'With variable',
          rule: { floor: [{ var: 'score' }] },
          data: { score: 89.9 },
          result: 89,
        },
      ],
      notes: ['Always rounds toward negative infinity', 'Use ceil to round up'],
      seeAlso: ['ceil', 'abs'],
    },
    ui: {
      icon: 'arrow-down-to-line',
      shortLabel: 'floor',
      nodeType: 'operator',
    },
  },
};
