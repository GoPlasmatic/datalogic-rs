/**
 * Error Handling Operators
 *
 * Exception-like error handling.
 * - try: Catch errors and provide fallback values
 * - throw: Throw an error to be caught by try
 */

import type { Operator } from '../operators.types';

export const errorOperators: Record<string, Operator> = {
  try: {
    name: 'try',
    label: 'Try',
    category: 'error',
    description: 'Catch errors and provide fallback values',
    arity: {
      type: 'nary',
      min: 1,
      args: [
        {
          name: 'expression',
          label: 'Expression',
          type: 'any',
          required: true,
          repeatable: true,
          description: 'Expression to try, followed by fallback expressions',
        },
      ],
    },
    help: {
      summary: 'Evaluate expressions in sequence until one succeeds',
      details:
        'Provides exception-like error handling. Evaluates each argument in order until one succeeds without throwing an error. The final argument can access the error context when catching a thrown error.',
      returnType: 'any',
      examples: [
        {
          title: 'Simple fallback',
          rule: { try: [{ var: 'name' }, 'Unknown'] },
          data: {},
          result: 'Unknown',
          note: 'Variable missing, uses fallback',
        },
        {
          title: 'First succeeds',
          rule: { try: [{ var: 'name' }, 'Unknown'] },
          data: { name: 'Alice' },
          result: 'Alice',
        },
        {
          title: 'Multiple fallbacks',
          rule: { try: [{ var: 'primary' }, { var: 'secondary' }, 'default'] },
          data: { secondary: 'backup' },
          result: 'backup',
        },
        {
          title: 'Catch thrown error',
          rule: {
            try: [
              { throw: { code: 404, message: 'Not found' } },
              { cat: ['Error: ', { var: 'message' }] },
            ],
          },
          result: 'Error: Not found',
          note: 'Error object accessible via var',
        },
        {
          title: 'Catch with error type',
          rule: {
            try: [
              { throw: 'validation_error' },
              { cat: ['Caught: ', { var: 'type' }] },
            ],
          },
          result: 'Caught: validation_error',
        },
      ],
      notes: [
        'Evaluates arguments left to right',
        'Stops at first successful evaluation',
        'Last argument receives error context for thrown errors',
        'If all fail, returns the last error',
      ],
      seeAlso: ['throw', '??', 'if'],
    },
    ui: {
      icon: 'shield',
      shortLabel: 'try',
      nodeType: 'decision',
      collapsible: true,
    },
  },

  throw: {
    name: 'throw',
    label: 'Throw',
    category: 'error',
    description: 'Throw an error to be caught by try',
    arity: {
      type: 'unary',
      min: 0,
      max: 1,
      args: [
        {
          name: 'error',
          label: 'Error',
          type: 'any',
          required: false,
          description: 'Error value (string, object, or any value)',
        },
      ],
    },
    help: {
      summary: 'Throw an error that can be caught by the try operator',
      details:
        'Throws an error that stops normal evaluation and can be caught by a surrounding try operator. The error value is accessible in the catch handler via {"var": ""}.',
      returnType: 'never',
      examples: [
        {
          title: 'Throw string error',
          rule: { throw: 'validation_error' },
          error: { type: 'Thrown' },
          note: 'String becomes {type: "validation_error"}',
        },
        {
          title: 'Throw error object',
          rule: { throw: { code: 404, message: 'Not found' } },
          error: { type: 'Thrown' },
        },
        {
          title: 'Throw with variable',
          rule: { throw: { var: 'errorInfo' } },
          data: { errorInfo: { type: 'custom', details: 'Something went wrong' } },
          error: { type: 'Thrown' },
        },
        {
          title: 'Conditional throw',
          rule: {
            if: [
              { '<': [{ var: 'age' }, 18] },
              { throw: 'age_restriction' },
              'allowed',
            ],
          },
          data: { age: 15 },
          error: { type: 'Thrown' },
        },
        {
          title: 'Throw null (no argument)',
          rule: { throw: [] },
          error: { type: 'Thrown' },
          note: 'Throws null error',
        },
      ],
      notes: [
        'String values become {type: string}',
        'Objects are passed as-is to the error context',
        'Use with try to implement error handling patterns',
        'Uncaught throws propagate as evaluation errors',
      ],
      seeAlso: ['try'],
    },
    ui: {
      icon: 'alert-triangle',
      shortLabel: 'throw',
      nodeType: 'operator',
    },
  },
};
