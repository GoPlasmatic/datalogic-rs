/**
 * Control Flow Operators
 *
 * Conditional branching operators.
 * - if: If/then/else branching
 * - ?:: Ternary operator
 * - ??: Nullish coalescing
 * - switch/match: Switch/case matching
 */

import type { Operator } from '../operators.types';

export const controlOperators: Record<string, Operator> = {
  if: {
    name: 'if',
    label: 'If',
    category: 'control',
    description: 'Conditional branching (if/then/else)',
    arity: {
      type: 'special',
      min: 2,
      args: [
        { name: 'condition', label: 'Condition', type: 'any', required: true },
        { name: 'then', label: 'Then', type: 'any', required: true },
        { name: 'else', label: 'Else', type: 'any', required: false },
      ],
    },
    help: {
      summary: 'Execute different expressions based on conditions',
      details:
        'Evaluates the condition. If truthy, returns the "then" value. If falsy, returns the "else" value (or null if not provided). Supports else-if chains with additional condition/result pairs.',
      returnType: 'any',
      examples: [
        {
          title: 'Simple if/then/else',
          rule: { if: [true, 'yes', 'no'] },
          result: 'yes',
        },
        {
          title: 'With condition',
          rule: {
            if: [{ '>': [{ var: 'age' }, 18] }, 'adult', 'minor'],
          },
          data: { age: 21 },
          result: 'adult',
        },
        {
          title: 'No else (returns null)',
          rule: { if: [false, 'yes'] },
          result: null,
        },
        {
          title: 'Else-if chain',
          rule: {
            if: [
              { '>=': [{ var: 'score' }, 90] },
              'A',
              { '>=': [{ var: 'score' }, 80] },
              'B',
              { '>=': [{ var: 'score' }, 70] },
              'C',
              'F',
            ],
          },
          data: { score: 85 },
          result: 'B',
        },
        {
          title: 'Nested conditions',
          rule: {
            if: [
              { var: 'isPremium' },
              { if: [{ var: 'isAnnual' }, 99, 12] },
              0,
            ],
          },
          data: { isPremium: true, isAnnual: true },
          result: 99,
        },
      ],
      notes: [
        'Pattern: [condition, then, else]',
        'Else-if: [cond1, then1, cond2, then2, ..., else]',
        'Missing else returns null',
        'Only the matching branch is evaluated (short-circuit)',
      ],
      seeAlso: ['?:', '??', 'and', 'or'],
    },
    ui: {
      icon: 'git-branch',
      shortLabel: 'if',
      nodeType: 'decision',
      collapsible: true,
      addArgumentLabel: 'Add Else If',
    },
    panel: {
      sections: [
        {
          id: 'condition',
          title: 'If',
          fields: [
            {
              id: 'condition',
              label: 'Condition',
              inputType: 'expression',
              required: true,
              helpText: 'Expression that determines which branch to take',
            },
            {
              id: 'then',
              label: 'Then',
              inputType: 'expression',
              required: true,
              helpText: 'Value returned if condition is truthy',
            },
          ],
        },
        {
          id: 'elseIfBranches',
          title: 'Else-If Branches',
          fields: [
            {
              id: 'elseIfCount',
              label: 'Number of Else-If',
              inputType: 'number',
              min: 0,
              max: 10,
              defaultValue: 0,
              helpText: 'Add additional condition branches',
            },
            {
              id: 'elseIf',
              label: 'Else-If',
              inputType: 'expression',
              repeatable: true,
              showWhen: [{ field: 'elseIfCount', operator: 'notEquals', value: 0 }],
              helpText: 'Additional condition/value pairs',
            },
          ],
        },
        {
          id: 'else',
          title: 'Else',
          fields: [
            {
              id: 'hasElse',
              label: 'Has Else Branch',
              inputType: 'boolean',
              defaultValue: false,
            },
            {
              id: 'else',
              label: 'Else',
              inputType: 'expression',
              showWhen: [{ field: 'hasElse', operator: 'equals', value: true }],
              helpText: 'Value returned if no conditions match',
            },
          ],
        },
      ],
    },
  },

  '?:': {
    name: '?:',
    label: 'Ternary',
    category: 'control',
    description: 'Ternary conditional (condition ? then : else)',
    arity: {
      type: 'ternary',
      min: 3,
      max: 3,
      args: [
        { name: 'condition', label: 'Condition', type: 'any', required: true },
        { name: 'then', label: 'Then', type: 'any', required: true },
        { name: 'else', label: 'Else', type: 'any', required: true },
      ],
    },
    help: {
      summary: 'Ternary operator - exactly 3 arguments (condition, then, else)',
      details:
        'Simplified version of "if" that requires exactly 3 arguments. If the condition is truthy, returns the second argument; otherwise returns the third.',
      returnType: 'any',
      examples: [
        {
          title: 'Simple ternary',
          rule: { '?:': [true, 'yes', 'no'] },
          result: 'yes',
        },
        {
          title: 'With condition',
          rule: {
            '?:': [{ var: 'isActive' }, 'Active', 'Inactive'],
          },
          data: { isActive: true },
          result: 'Active',
        },
        {
          title: 'Numeric result',
          rule: {
            '?:': [{ '>': [{ var: 'qty' }, 10] }, 0.1, 0],
          },
          data: { qty: 15 },
          result: 0.1,
          note: 'Discount if quantity > 10',
        },
      ],
      notes: [
        'Exactly 3 arguments required',
        'Use "if" for else-if chains',
        'Equivalent to: if [condition, then, else]',
      ],
      seeAlso: ['if', '??'],
    },
    ui: {
      icon: 'help-circle',
      shortLabel: '?:',
      nodeType: 'decision',
    },
    panel: {
      sections: [
        {
          id: 'args',
          fields: [
            {
              id: 'condition',
              label: 'Condition',
              inputType: 'expression',
              required: true,
              helpText: 'Expression to evaluate',
            },
            {
              id: 'then',
              label: 'Then',
              inputType: 'expression',
              required: true,
              helpText: 'Value returned if condition is truthy',
            },
            {
              id: 'else',
              label: 'Else',
              inputType: 'expression',
              required: true,
              helpText: 'Value returned if condition is falsy',
            },
          ],
        },
      ],
    },
  },

  switch: {
    name: 'switch',
    label: 'Switch',
    category: 'control',
    description: 'Match a value against cases (switch/case)',
    arity: {
      type: 'special',
      min: 2,
      args: [
        { name: 'discriminant', label: 'Value', type: 'any', required: true },
        { name: 'cases', label: 'Cases', type: 'array', required: true },
        { name: 'default', label: 'Default', type: 'any', required: false },
      ],
    },
    help: {
      summary: 'Evaluate a value once and match it against case/result pairs',
      details:
        'Evaluates the discriminant expression once, then compares it against each case value using strict equality. Returns the result of the first matching case, or the default value if no case matches. If no default is provided and nothing matches, returns null.',
      returnType: 'any',
      examples: [
        {
          title: 'Simple string matching',
          rule: {
            switch: [
              { var: 'color' },
              [
                ['red', 'stop'],
                ['green', 'go'],
              ],
              'unknown',
            ],
          },
          data: { color: 'red' },
          result: 'stop',
        },
        {
          title: 'HTTP status codes',
          rule: {
            switch: [
              { var: 'status' },
              [
                [200, 'OK'],
                [404, 'Not Found'],
                [500, 'Error'],
              ],
              'Unknown',
            ],
          },
          data: { status: 404 },
          result: 'Not Found',
        },
        {
          title: 'Computed discriminant',
          rule: {
            switch: [
              { '+': [{ var: 'a' }, { var: 'b' }] },
              [
                [10, 'ten'],
                [20, 'twenty'],
              ],
              'other',
            ],
          },
          data: { a: 7, b: 3 },
          result: 'ten',
        },
        {
          title: 'Expression as result',
          rule: {
            switch: [
              { var: 'tier' },
              [
                ['gold', { '*': [{ var: 'price' }, 0.8] }],
                ['silver', { '*': [{ var: 'price' }, 0.9] }],
              ],
              { var: 'price' },
            ],
          },
          data: { tier: 'gold', price: 100 },
          result: 80,
        },
      ],
      notes: [
        'Pattern: [discriminant, [[case1, result1], [case2, result2], ...], default]',
        'Uses strict equality (type-sensitive)',
        'First matching case wins',
        'Only the matched result is evaluated (short-circuit)',
        '"match" is an alias for "switch"',
      ],
      seeAlso: ['if', '?:', '=='],
    },
    ui: {
      icon: 'git-branch',
      shortLabel: 'switch',
      nodeType: 'decision',
      collapsible: true,
      addArgumentLabel: 'Add Case',
    },
    panel: {
      sections: [
        {
          id: 'discriminant',
          title: 'Match Value',
          fields: [
            {
              id: 'discriminant',
              label: 'Value',
              inputType: 'expression',
              required: true,
              helpText: 'Expression to evaluate and match against cases',
            },
          ],
        },
        {
          id: 'cases',
          title: 'Cases',
          fields: [
            {
              id: 'caseCount',
              label: 'Number of Cases',
              inputType: 'number',
              min: 1,
              max: 50,
              defaultValue: 1,
              helpText: 'Number of case/result pairs',
            },
            {
              id: 'case',
              label: 'Case',
              inputType: 'expression',
              repeatable: true,
              helpText: 'Case value and result pairs',
            },
          ],
        },
        {
          id: 'default',
          title: 'Default',
          fields: [
            {
              id: 'hasDefault',
              label: 'Has Default',
              inputType: 'boolean',
              defaultValue: false,
            },
            {
              id: 'default',
              label: 'Default',
              inputType: 'expression',
              showWhen: [{ field: 'hasDefault', operator: 'equals', value: true }],
              helpText: 'Value returned if no cases match',
            },
          ],
        },
      ],
    },
  },

  match: {
    name: 'match',
    label: 'Match',
    category: 'control',
    description: 'Match a value against cases (alias for switch)',
    arity: {
      type: 'special',
      min: 2,
      args: [
        { name: 'discriminant', label: 'Value', type: 'any', required: true },
        { name: 'cases', label: 'Cases', type: 'array', required: true },
        { name: 'default', label: 'Default', type: 'any', required: false },
      ],
    },
    help: {
      summary: 'Evaluate a value once and match it against case/result pairs (alias for switch)',
      details:
        'Identical to "switch". Evaluates the discriminant expression once, then compares it against each case value using strict equality.',
      returnType: 'any',
      examples: [
        {
          title: 'Simple matching',
          rule: {
            match: [
              { var: 'color' },
              [
                ['red', 'stop'],
                ['green', 'go'],
              ],
              'unknown',
            ],
          },
          data: { color: 'green' },
          result: 'go',
        },
      ],
      notes: ['Alias for "switch"', 'See "switch" for full documentation'],
      seeAlso: ['switch', 'if', '?:'],
    },
    ui: {
      icon: 'git-branch',
      shortLabel: 'match',
      nodeType: 'decision',
      collapsible: true,
      addArgumentLabel: 'Add Case',
    },
    panel: {
      sections: [
        {
          id: 'discriminant',
          title: 'Match Value',
          fields: [
            {
              id: 'discriminant',
              label: 'Value',
              inputType: 'expression',
              required: true,
              helpText: 'Expression to evaluate and match against cases',
            },
          ],
        },
        {
          id: 'cases',
          title: 'Cases',
          fields: [
            {
              id: 'case',
              label: 'Case',
              inputType: 'expression',
              repeatable: true,
              helpText: 'Case value and result pairs',
            },
          ],
        },
        {
          id: 'default',
          title: 'Default',
          fields: [
            {
              id: 'hasDefault',
              label: 'Has Default',
              inputType: 'boolean',
              defaultValue: false,
            },
            {
              id: 'default',
              label: 'Default',
              inputType: 'expression',
              showWhen: [{ field: 'hasDefault', operator: 'equals', value: true }],
              helpText: 'Value returned if no cases match',
            },
          ],
        },
      ],
    },
  },

  '??': {
    name: '??',
    label: 'Coalesce',
    category: 'control',
    description: 'Return first non-null value',
    arity: {
      type: 'binary',
      min: 2,
      max: 2,
      args: [
        { name: 'value', label: 'Value', type: 'any', required: true },
        { name: 'fallback', label: 'Fallback', type: 'any', required: true },
      ],
    },
    help: {
      summary: 'Return the first value if not null/undefined, otherwise the fallback',
      details:
        'Nullish coalescing operator. Unlike "or", this only checks for null/undefined, not other falsy values like 0 or empty string.',
      returnType: 'any',
      examples: [
        {
          title: 'With null',
          rule: { '??': [null, 'default'] },
          result: 'default',
        },
        {
          title: 'With value',
          rule: { '??': ['hello', 'default'] },
          result: 'hello',
        },
        {
          title: 'Zero is kept',
          rule: { '??': [0, 100] },
          result: 0,
          note: 'Unlike "or", zero is not replaced',
        },
        {
          title: 'Empty string is kept',
          rule: { '??': ['', 'default'] },
          result: '',
          note: 'Unlike "or", empty string is not replaced',
        },
        {
          title: 'With variable',
          rule: { '??': [{ var: 'nickname' }, { var: 'name' }] },
          data: { nickname: null, name: 'Alice' },
          result: 'Alice',
        },
      ],
      notes: [
        'Only replaces null/undefined',
        '0, false, "" are NOT replaced (unlike "or")',
        'Use "or" to also replace falsy values',
      ],
      seeAlso: ['or', 'if', '?:'],
    },
    ui: {
      icon: 'circle-dot',
      shortLabel: '??',
      nodeType: 'operator',
    },
    panel: {
      sections: [
        {
          id: 'args',
          fields: [
            {
              id: 'value',
              label: 'Value',
              inputType: 'expression',
              required: true,
              helpText: 'Value to check for null/undefined',
            },
            {
              id: 'fallback',
              label: 'Fallback',
              inputType: 'expression',
              required: true,
              helpText: 'Value to return if first value is null/undefined',
            },
          ],
        },
      ],
    },
  },
};
