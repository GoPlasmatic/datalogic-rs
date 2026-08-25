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
        'Provides exception-like error handling. Evaluates each argument in order until one succeeds without throwing an error. Only errors move on to the next argument: a null result is a success. Each fallback argument sees the caught error as its context.',
      returnType: 'any',
      examples: [
        {
          title: 'Fallback on error',
          rule: { try: [{ '/': [1, 0] }, 'n/a'] },
          result: 'n/a',
          note: 'Division by zero throws, so the fallback runs',
        },
        {
          title: 'First succeeds',
          rule: { try: ['ok', 'fallback'] },
          result: 'ok',
        },
        {
          title: 'Missing variable is not an error',
          rule: { try: [{ var: 'name' }, 'Unknown'] },
          data: {},
          result: null,
          note: 'var returns null instead of throwing; use ?? or var\'s default for that',
        },
        {
          title: 'Multiple fallbacks',
          rule: { try: [{ throw: 'a' }, { throw: 'b' }, 'default'] },
          result: 'default',
        },
        {
          title: 'Catch thrown error',
          rule: {
            try: [
              { throw: { var: 'err' } },
              { cat: ['Error: ', { var: 'message' }] },
            ],
          },
          data: { err: { code: 404, message: 'Not found' } },
          result: 'Error: Not found',
          note: 'The thrown object is the context of the catch arm',
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
        {
          title: 'Engine errors are caught too',
          rule: {
            try: [
              { length: [1, 2] },
              { cat: ['Engine error: ', { var: 'type' }] },
            ],
          },
          result: 'Engine error: Invalid Arguments',
        },
        {
          title: 'All arms fail',
          rule: { try: [{ throw: 'a' }, { throw: 'b' }] },
          error: { type: 'Thrown' },
          note: 'The last error propagates',
        },
      ],
      notes: [
        'Evaluates arguments left to right; stops at the first that does not throw',
        'Only errors advance to the next arm: a null result counts as success',
        'Missing variables return null and are NOT caught; use ?? or var\'s default argument',
        'The catch arm sees the thrown value as its context: {"var": "type"}, {"var": "message"}, {"var": ""}',
        'Engine errors arrive as {"type": "Invalid Arguments"}, {"type": "Unknown Operator"} or {"type": "NaN"}',
        'If every arm fails, the last error propagates',
      ],
      seeAlso: ['throw', '??', 'if'],
    },
    ui: {
      icon: 'git-branch',
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
      type: 'range',
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
          title: 'Throw error object (templating mode)',
          rule: { throw: { code: 404, message: 'Not found' } },
          templating: true,
          error: { type: 'Thrown' },
          note: 'Multi-key object literals only parse in templating mode; otherwise throw {"var": "err"}',
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
          note: 'Throws {"type": "null"}',
        },
      ],
      notes: [
        'String values become {"type": string}',
        'Objects are passed as-is to the error context',
        'Other values become {"type": "<type name>"}: no argument or null gives {"type": "null"}, 5 gives {"type": "number"}',
        'A multi-key object literal needs templating mode; in the default mode throw {"var": "err"} read from data',
        'Use with try to implement error handling patterns',
        'Uncaught throws propagate as evaluation errors',
      ],
      seeAlso: ['try'],
    },
    ui: {
      icon: 'alert-circle',
      shortLabel: 'throw',
      nodeType: 'operator',
    },
  },
};
