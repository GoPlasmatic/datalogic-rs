/**
 * DateTime Operators
 *
 * Temporal data handling operations.
 * - datetime: Parse or validate a datetime value
 * - timestamp: Parse or validate a duration value
 * - parse_date: Parse a date string with a custom format
 * - format_date: Format a datetime with a custom format string
 * - date_diff: Calculate the difference between two dates
 * - now: Get the current UTC datetime
 */

import type { Operator } from '../operators.types';

export const datetimeOperators: Record<string, Operator> = {
  datetime: {
    name: 'datetime',
    label: 'DateTime',
    category: 'datetime',
    description: 'Parse or validate a datetime value',
    arity: {
      type: 'unary',
      min: 1,
      max: 1,
      args: [
        {
          name: 'value',
          label: 'Value',
          type: 'string',
          required: true,
          description: 'ISO 8601 datetime string',
        },
      ],
    },
    help: {
      summary: 'Parse or validate a datetime string',
      details:
        'Parses an ISO 8601 datetime string and validates it. Returns the datetime if valid, or throws an error if invalid. Preserves the original timezone information.',
      returnType: 'string',
      examples: [
        {
          title: 'UTC datetime',
          rule: { datetime: '2024-01-15T10:30:00Z' },
          result: '2024-01-15T10:30:00Z',
        },
        {
          title: 'With timezone offset',
          rule: { datetime: '2024-01-15T10:30:00+05:30' },
          result: '2024-01-15T10:30:00+05:30',
        },
        {
          title: 'With variable',
          rule: { datetime: { var: 'createdAt' } },
          data: { createdAt: '2024-01-15T10:30:00Z' },
          result: '2024-01-15T10:30:00Z',
        },
      ],
      notes: [
        'Supports ISO 8601 format',
        'Preserves timezone information',
        'Throws error for invalid format',
      ],
      seeAlso: ['parse_date', 'format_date', 'now'],
    },
    ui: {
      icon: 'calendar',
      shortLabel: 'dt',
      nodeType: 'operator',
      datetimeProps: true,
    },
  },

  timestamp: {
    name: 'timestamp',
    label: 'Duration',
    category: 'datetime',
    description: 'Parse or validate a duration value',
    arity: {
      type: 'unary',
      min: 1,
      max: 1,
      args: [
        {
          name: 'value',
          label: 'Value',
          type: 'string',
          required: true,
          description: 'Duration string (e.g., "2h30m")',
        },
      ],
    },
    help: {
      summary: 'Parse or validate a duration string',
      details:
        'Parses a duration string in various formats (e.g., "2h30m", "1d", "90s") and returns the normalized duration.',
      returnType: 'string',
      examples: [
        {
          title: 'Hours and minutes',
          rule: { timestamp: '2h30m' },
          result: '2h30m',
        },
        {
          title: 'Days',
          rule: { timestamp: '1d' },
          result: '1d',
        },
        {
          title: 'Seconds',
          rule: { timestamp: '90s' },
          result: '90s',
        },
        {
          title: 'With variable',
          rule: { timestamp: { var: 'timeout' } },
          data: { timeout: '30m' },
          result: '30m',
        },
      ],
      notes: [
        'Supports d (days), h (hours), m (minutes), s (seconds)',
        'Multiple units can be combined: "1d2h30m"',
        'Throws error for invalid format',
      ],
      seeAlso: ['datetime', 'date_diff'],
    },
    ui: {
      icon: 'timer',
      shortLabel: 'dur',
      nodeType: 'operator',
    },
  },

  parse_date: {
    name: 'parse_date',
    label: 'Parse Date',
    category: 'datetime',
    description: 'Parse a date string with a custom format',
    arity: {
      type: 'binary',
      min: 2,
      max: 2,
      args: [
        {
          name: 'dateString',
          label: 'Date String',
          type: 'string',
          required: true,
          description: 'The date string to parse',
        },
        {
          name: 'format',
          label: 'Format',
          type: 'string',
          required: true,
          description: 'Format pattern (e.g., "yyyy-MM-dd")',
        },
      ],
    },
    help: {
      summary: 'Parse a date string using a custom format pattern',
      details:
        'Parses a date string according to the specified format pattern and returns an ISO 8601 datetime string. Useful for handling non-standard date formats.',
      returnType: 'string',
      examples: [
        {
          title: 'Date only',
          rule: { parse_date: ['2024-01-15', 'yyyy-MM-dd'] },
          result: '2024-01-15T00:00:00Z',
        },
        {
          title: 'Date and time',
          rule: { parse_date: ['2024-01-15 10:30:00', 'yyyy-MM-dd HH:mm:ss'] },
          result: '2024-01-15T10:30:00Z',
        },
        {
          title: 'US date format',
          rule: { parse_date: ['01/15/2024', 'MM/dd/yyyy'] },
          result: '2024-01-15T00:00:00Z',
        },
        {
          title: 'With variable',
          rule: { parse_date: [{ var: 'dateStr' }, 'yyyy-MM-dd'] },
          data: { dateStr: '2024-12-25' },
          result: '2024-12-25T00:00:00Z',
        },
      ],
      notes: [
        'Format tokens: yyyy (year), MM (month), dd (day), HH (hour), mm (minute), ss (second)',
        'Returns ISO 8601 format',
        'Throws error if string does not match format',
      ],
      seeAlso: ['format_date', 'datetime'],
    },
    ui: {
      icon: 'calendar-search',
      shortLabel: 'parse',
      nodeType: 'operator',
    },
  },

  format_date: {
    name: 'format_date',
    label: 'Format Date',
    category: 'datetime',
    description: 'Format a datetime with a custom format string',
    arity: {
      type: 'binary',
      min: 2,
      max: 2,
      args: [
        {
          name: 'datetime',
          label: 'DateTime',
          type: 'string',
          required: true,
          description: 'ISO 8601 datetime string or datetime object',
        },
        {
          name: 'format',
          label: 'Format',
          type: 'string',
          required: true,
          description: 'Format pattern (e.g., "yyyy-MM-dd")',
        },
      ],
    },
    help: {
      summary: 'Format a datetime value using a custom format pattern',
      details:
        'Takes a datetime value (ISO string or datetime object) and formats it according to the specified pattern.',
      returnType: 'string',
      examples: [
        {
          title: 'Date only',
          rule: { format_date: ['2024-01-15T10:30:00Z', 'yyyy-MM-dd'] },
          result: '2024-01-15',
        },
        {
          title: 'Time only',
          rule: { format_date: ['2024-01-15T10:30:00Z', 'HH:mm:ss'] },
          result: '10:30:00',
        },
        {
          title: 'Custom format',
          rule: { format_date: ['2024-01-15T10:30:00Z', 'MM/dd/yyyy'] },
          result: '01/15/2024',
        },
        {
          title: 'With variable',
          rule: { format_date: [{ var: 'timestamp' }, 'yyyy-MM-dd'] },
          data: { timestamp: '2024-06-15T14:30:00Z' },
          result: '2024-06-15',
        },
      ],
      notes: [
        'Format tokens: yyyy (year), MM (month), dd (day), HH (hour), mm (minute), ss (second)',
        'Input can be ISO string or datetime object',
        'Special format "z" returns timezone offset',
      ],
      seeAlso: ['parse_date', 'datetime'],
    },
    ui: {
      icon: 'calendar-check',
      shortLabel: 'fmt',
      nodeType: 'operator',
    },
  },

  date_diff: {
    name: 'date_diff',
    label: 'Date Difference',
    category: 'datetime',
    description: 'Calculate the difference between two dates',
    arity: {
      type: 'ternary',
      min: 3,
      max: 3,
      args: [
        {
          name: 'date1',
          label: 'Date 1',
          type: 'string',
          required: true,
          description: 'First datetime',
        },
        {
          name: 'date2',
          label: 'Date 2',
          type: 'string',
          required: true,
          description: 'Second datetime',
        },
        {
          name: 'unit',
          label: 'Unit',
          type: 'string',
          required: true,
          description: 'Unit of measurement (days, hours, minutes, seconds)',
        },
      ],
    },
    help: {
      summary: 'Calculate the difference between two datetime values',
      details:
        'Returns the difference between two dates in the specified unit. The result is date1 - date2 (positive if date1 is later).',
      returnType: 'number',
      examples: [
        {
          title: 'Days between dates',
          rule: {
            date_diff: ['2024-01-20T00:00:00Z', '2024-01-15T00:00:00Z', 'days'],
          },
          result: 5,
        },
        {
          title: 'Hours difference',
          rule: {
            date_diff: ['2024-01-15T15:00:00Z', '2024-01-15T10:00:00Z', 'hours'],
          },
          result: 5,
        },
        {
          title: 'Negative difference',
          rule: {
            date_diff: ['2024-01-10T00:00:00Z', '2024-01-15T00:00:00Z', 'days'],
          },
          result: -5,
          note: 'date1 is before date2',
        },
        {
          title: 'With variables',
          rule: {
            date_diff: [{ var: 'end' }, { var: 'start' }, 'days'],
          },
          data: { start: '2024-01-01T00:00:00Z', end: '2024-01-31T00:00:00Z' },
          result: 30,
        },
      ],
      notes: [
        'Supported units: days, hours, minutes, seconds',
        'Result is integer (truncated)',
        'Positive if date1 > date2, negative otherwise',
      ],
      seeAlso: ['datetime', 'now'],
    },
    ui: {
      icon: 'calendar-range',
      shortLabel: 'diff',
      nodeType: 'operator',
    },
  },

  now: {
    name: 'now',
    label: 'Now',
    category: 'datetime',
    description: 'Get the current UTC datetime',
    arity: {
      type: 'nullary',
      min: 0,
      max: 0,
      args: [],
    },
    help: {
      summary: 'Returns the current UTC datetime',
      details:
        'Returns the current date and time in ISO 8601 format with UTC timezone. Useful for timestamps, age calculations, and time-based logic.',
      returnType: 'string',
      examples: [
        {
          title: 'Get current time',
          rule: { now: [] },
          result: '2024-01-15T10:30:00Z',
          note: 'Actual result depends on current time',
        },
        {
          title: 'Compare with date',
          rule: {
            '>': [{ now: [] }, { var: 'expiresAt' }],
          },
          data: { expiresAt: '2024-01-01T00:00:00Z' },
          result: true,
          note: 'Check if current time is past expiration',
        },
        {
          title: 'Calculate age in days',
          rule: {
            date_diff: [{ now: [] }, { var: 'createdAt' }, 'days'],
          },
          data: { createdAt: '2024-01-01T00:00:00Z' },
          result: 14,
          note: 'Days since creation (example value)',
        },
      ],
      notes: [
        'Returns UTC time (timezone offset +00:00)',
        'Result changes with each evaluation',
        'Format: ISO 8601 (YYYY-MM-DDTHH:MM:SSZ)',
      ],
      seeAlso: ['datetime', 'date_diff'],
    },
    ui: {
      icon: 'clock',
      shortLabel: 'now',
      nodeType: 'operator',
    },
  },
};
