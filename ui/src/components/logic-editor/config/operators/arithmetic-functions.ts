/**
 * Arithmetic Function Operators
 *
 * Aggregate and unary math functions: max, min, abs, ceil, floor
 */

import type { Operator } from '../operators.types';

export const arithmeticFunctionOperators: Record<string, Operator> = {
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
