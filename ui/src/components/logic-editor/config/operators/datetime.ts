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
          description: 'ISO 8601 datetime string (date and time)',
        },
      ],
    },
    help: {
      summary: 'Parse or validate a datetime string',
      details:
        'Parses a full ISO 8601 datetime (date and time; offset optional) and validates it. Returns the datetime if valid, or throws an error if invalid. Preserves the original timezone information.',
      returnType: 'datetime',
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
        {
          title: 'Date-only string is invalid',
          rule: { datetime: '2024-01-15' },
          error: { type: 'InvalidArguments' },
          note: 'A time part is required; use parse_date with a format for date-only input',
        },
      ],
      notes: [
        'Requires a full ISO 8601 datetime (date + time); the offset is optional',
        'Date-only strings like "2024-01-15" are an error (use parse_date)',
        'Preserves timezone information',
        '{"datetime": ...} objects pass through unchanged',
      ],
      seeAlso: ['parse_date', 'format_date', 'now'],
    },
    ui: {
      icon: 'calendar',
      shortLabel: 'dt',
      nodeType: 'operator',
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
        'Parses a duration string such as "2h30m", "1d" or "90s" and returns it normalized to the "Dd:Hh:Mm:Ss" form (units carried over, e.g. 90s becomes 1m:30s).',
      returnType: 'duration',
      examples: [
        {
          title: 'Hours and minutes',
          rule: { timestamp: '2h30m' },
          result: '0d:2h:30m:0s',
        },
        {
          title: 'Days',
          rule: { timestamp: '1d' },
          result: '1d:0h:0m:0s',
        },
        {
          title: 'Seconds',
          rule: { timestamp: '90s' },
          result: '0d:0h:1m:30s',
          note: 'Overflowing units carry over',
        },
        {
          title: 'With variable',
          rule: { timestamp: { var: 'timeout' } },
          data: { timeout: '30m' },
          result: '0d:0h:30m:0s',
        },
        {
          title: 'Unsupported unit',
          rule: { timestamp: '1w' },
          error: { type: 'InvalidArguments' },
          note: 'Only d, h, m and s are accepted',
        },
      ],
      notes: [
        'Units: d (days), h (hours), m (minutes), s (seconds) with integer counts',
        'Combine units as "1d2h30m" or in the colon form "1d:2h:30m:0s"',
        'Result is always the normalized "Dd:Hh:Mm:Ss" string',
        'Weeks, decimals and ISO 8601 durations ("PT2H30M") are an error',
      ],
      seeAlso: ['datetime', 'date_diff'],
    },
    ui: {
      icon: 'clock',
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
      type: 'range',
      min: 2,
      max: 3,
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
        {
          name: 'timezone',
          label: 'Timezone',
          type: 'string',
          required: false,
          description: 'Optional IANA zone name (e.g. "Asia/Kolkata", "America/New_York")',
        },
      ],
    },
    help: {
      summary: 'Parse a date string using a custom format pattern',
      details:
        'Parses a date string according to the specified format pattern and returns an ISO 8601 datetime. Input without an offset is read as UTC, or as local time in the optional IANA timezone (third argument) and converted to UTC.',
      returnType: 'datetime',
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
        {
          title: 'Month name',
          rule: { parse_date: ['15 Jan 2024', 'dd MMM yyyy'] },
          result: '2024-01-15T00:00:00Z',
        },
        {
          title: 'Local time in a timezone',
          rule: { parse_date: ['2024-01-15 10:30', 'yyyy-MM-dd HH:mm', 'Asia/Kolkata'] },
          result: '2024-01-15T05:00:00Z',
          note: '10:30 IST is 05:00 UTC',
        },
        {
          title: 'No match',
          rule: { parse_date: ['nope', 'yyyy-MM-dd'] },
          error: { type: 'InvalidArguments' },
        },
      ],
      notes: [
        'Format tokens: yyyy, MM, dd, HH, mm, ss, plus MMM/MMMM (month name), EEE/EEEE (weekday name)',
        'Raw strftime tokens (%Y, %m, %d, %A, ...) pass through unchanged',
        'Optional third argument: IANA timezone the input is expressed in (DST-aware)',
        'Returns ISO 8601 (UTC unless the input carried an offset)',
        'Throws error if string does not match format',
      ],
      seeAlso: ['format_date', 'datetime'],
    },
    ui: {
      icon: 'calendar',
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
      type: 'range',
      min: 2,
      max: 3,
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
        {
          name: 'timezone',
          label: 'Timezone',
          type: 'string',
          required: false,
          description: 'Optional IANA zone name (e.g. "Asia/Kolkata", "America/New_York")',
        },
      ],
    },
    help: {
      summary: 'Format a datetime value using a custom format pattern',
      details:
        'Takes a datetime value (ISO string or datetime object) and formats it according to the specified pattern. With an optional IANA timezone as the third argument the instant is rendered in that zone (DST-aware).',
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
        {
          title: 'Names of day and month',
          rule: { format_date: ['2024-01-15T10:30:00Z', 'EEEE, dd MMMM yyyy'] },
          result: 'Monday, 15 January 2024',
        },
        {
          title: 'In a timezone',
          rule: { format_date: ['2024-01-15T10:30:00Z', 'HH:mm', 'Asia/Kolkata'] },
          result: '16:00',
          note: '10:30 UTC is 16:00 IST',
        },
        {
          title: 'Raw strftime tokens with zone name',
          rule: {
            format_date: [
              { datetime: '2024-01-15T10:30:00Z' },
              '%Y-%m-%d %H:%M %Z',
              'America/New_York',
            ],
          },
          result: '2024-01-15 05:30 EST',
        },
        {
          title: 'Timezone offset',
          rule: { format_date: ['2024-01-15T10:30:00+05:30', 'z'] },
          result: '+0530',
        },
      ],
      notes: [
        'Format tokens: yyyy, MM, dd, HH, mm, ss, plus MMM/MMMM (month name), EEE/EEEE (weekday name)',
        'Raw strftime tokens (%Y, %m, %d, %A, %Z, ...) pass through unchanged',
        'Optional third argument: IANA timezone to render in (DST-correct)',
        'Input can be ISO string or datetime object',
        'Special format "z" returns the timezone offset',
      ],
      seeAlso: ['parse_date', 'datetime'],
    },
    ui: {
      icon: 'calendar',
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
          description: 'Unit: days, hours, minutes, seconds or milliseconds',
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
        {
          title: 'Milliseconds',
          rule: {
            date_diff: ['2024-01-15T00:00:01Z', '2024-01-15T00:00:00Z', 'milliseconds'],
          },
          result: 1000,
        },
        {
          title: 'Unknown unit',
          rule: {
            date_diff: ['2024-01-15T00:00:00Z', '2024-01-15T00:00:00Z', 'weeks'],
          },
          error: { type: 'InvalidArguments' },
          note: 'Only days, hours, minutes, seconds and milliseconds are accepted',
        },
      ],
      notes: [
        'Supported units: days, hours, minutes, seconds, milliseconds',
        'Any other unit is an InvalidArguments error',
        'Result is integer (truncated)',
        'Positive if date1 > date2, negative otherwise',
      ],
      seeAlso: ['datetime', 'now'],
    },
    ui: {
      icon: 'calendar',
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
      returnType: 'datetime',
      examples: [
        {
          title: 'Get current time',
          rule: { now: [] },
          note: 'Returns the current instant, e.g. "2024-01-15T10:30:00Z"; the value changes on every evaluation',
        },
        {
          title: 'It is a datetime',
          rule: { type: [{ now: [] }] },
          result: 'datetime',
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
          title: 'Age in days is never negative',
          rule: {
            '>=': [{ date_diff: [{ now: [] }, { var: 'createdAt' }, 'days'] }, 0],
          },
          data: { createdAt: '2024-01-01T00:00:00Z' },
          result: true,
          note: 'date_diff with now gives the days since creation',
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
