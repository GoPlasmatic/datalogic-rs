/**
 * flagd Operators
 *
 * Feature-flag targeting operators specified by the OpenFeature flagd
 * in-process provider. Both return null (never throw) on malformed input so
 * a flag evaluator can fall back to its default variant.
 * - fractional: deterministic percentage bucketing
 * - sem_ver: semantic version comparison
 */

import type { Operator } from '../operators.types';

export const flagdOperators: Record<string, Operator> = {
  fractional: {
    name: 'fractional',
    label: 'Fractional',
    category: 'flagd',
    description: 'Deterministic percentage bucketing for rollouts and A/B tests',
    arity: {
      type: 'special',
      min: 1,
      args: [
        {
          name: 'key',
          label: 'Bucketing Key',
          type: 'string',
          required: false,
          description:
            'String to hash (e.g. flag key + user id). Omit to use targetingKey from the data',
        },
        {
          name: 'bucket',
          label: 'Bucket',
          type: 'array',
          required: true,
          repeatable: true,
          description: '[variant, weight] pair; weight defaults to 1',
        },
      ],
    },
    help: {
      summary: 'Pick a variant by hashing a key into weighted buckets (sticky per key)',
      details:
        'Hashes the bucketing key (MurmurHash3) and walks the cumulative weights, so the same key always lands in the same variant. With an explicit first argument that evaluates to a string, that string is the key; otherwise the key is $flagd.flagKey + targetingKey read from the data. Weights are relative (50/50 and 1/1 split identically).',
      returnType: 'string',
      examples: [
        {
          title: 'Explicit key',
          rule: {
            fractional: [
              { cat: ['my-seed', { var: 'email' }] },
              ['red', 25],
              ['blue', 25],
              ['green', 25],
              ['yellow', 25],
            ],
          },
          data: { email: 'rachel@faas.com' },
          result: 'green',
        },
        {
          title: 'flagd pattern: flag key + user',
          rule: {
            fractional: [
              { cat: [{ var: '$flagd.flagKey' }, { var: 'email' }] },
              ['red', 25],
              ['blue', 25],
              ['green', 25],
              ['yellow', 25],
            ],
          },
          data: { email: 'rachel@faas.com', $flagd: { flagKey: 'headerColor' } },
          result: 'blue',
          note: 'Prefixing the flag key gives each flag its own cohorts',
        },
        {
          title: 'Implicit key from targetingKey',
          rule: { fractional: [['blue', 50], ['green', 50]] },
          data: { targetingKey: 'foo@foo.com', $flagd: { flagKey: 'headerColor' } },
          result: 'green',
          note: 'No key argument: hashes flagKey + targetingKey',
        },
        {
          title: 'Missing targetingKey',
          rule: { fractional: [['blue', 50], ['green', 50]] },
          data: { $flagd: { flagKey: 'headerColor' } },
          result: null,
          note: 'Nothing to hash, so null (the caller falls back to its default)',
        },
        {
          title: 'Gated rollout',
          rule: {
            if: [
              { sem_ver: [{ var: 'app_version' }, '>=', '2.0.0'] },
              {
                fractional: [
                  { cat: [{ var: '$flagd.flagKey' }, { var: 'user_id' }] },
                  ['new-checkout', 10],
                  ['old-checkout', 90],
                ],
              },
              'old-checkout',
            ],
          },
          data: { app_version: '2.1.0', user_id: 'u-20', $flagd: { flagKey: 'checkout' } },
          result: 'new-checkout',
          note: 'Version gate first, then a 10% rollout keyed on flag + user',
        },
      ],
      notes: [
        'Buckets are [variant, weight] pairs; a missing weight defaults to 1 and negative weights clamp to 0',
        'Weights are relative, not percentages: [1, 99] and [50, 50] can be grown without renormalizing',
        'Same key, same variant, on every run (MurmurHash3, byte-compatible with the flagd Go evaluator)',
        'Malformed input (no buckets, no key, empty targetingKey) returns null rather than throwing',
      ],
      seeAlso: ['sem_ver', 'if', '??'],
    },
    ui: {
      icon: 'boxes',
      shortLabel: 'frac',
      nodeType: 'operator',
      addArgumentLabel: 'Add Bucket',
    },
  },

  sem_ver: {
    name: 'sem_ver',
    label: 'Semantic Version',
    category: 'flagd',
    description: 'Compare two semantic versions',
    arity: {
      type: 'ternary',
      min: 3,
      max: 3,
      args: [
        {
          name: 'version',
          label: 'Version',
          type: 'string',
          required: true,
          description: 'Version to test (e.g. {"var": "app_version"})',
        },
        {
          name: 'operator',
          label: 'Operator',
          type: 'string',
          required: true,
          description: 'One of =, !=, <, <=, >, >=, ^ (same major), ~ (same major.minor)',
        },
        {
          name: 'target',
          label: 'Target',
          type: 'string',
          required: true,
          description: 'Version to compare against',
        },
      ],
    },
    help: {
      summary: 'Compare semantic versions with =, !=, <, <=, >, >=, ^ or ~',
      details:
        'Parses both versions per SemVer 2.0 (pre-release ordering included) and applies the operator. ^ matches the same major version, ~ the same major and minor. Inputs are normalized first: a leading v/V is stripped, partial versions are padded ("1.2" is "1.2.0"), numbers are coerced to strings and build metadata is dropped.',
      returnType: 'boolean',
      examples: [
        {
          title: 'Minimum version',
          rule: { sem_ver: [{ var: 'app_version' }, '>=', '1.2.0'] },
          data: { app_version: '1.3.0' },
          result: true,
        },
        {
          title: 'Caret: same major',
          rule: { sem_ver: [{ var: 'app_version' }, '^', '1.0.0'] },
          data: { app_version: '1.5.3' },
          result: true,
          note: '2.0.0 would be false',
        },
        {
          title: 'Tilde: same major and minor',
          rule: { sem_ver: [{ var: 'app_version' }, '~', '1.2.0'] },
          data: { app_version: '1.3.0' },
          result: false,
          note: '1.2.5 would be true',
        },
        {
          title: 'Normalized inputs',
          rule: { sem_ver: ['v1.2', '<', '1.2.1'] },
          result: true,
          note: 'v prefix stripped, "1.2" padded to "1.2.0"',
        },
        {
          title: 'Pre-release ordering',
          rule: { sem_ver: ['1.0.0-alpha', '<', '1.0.0'] },
          result: true,
        },
        {
          title: 'Malformed version',
          rule: { sem_ver: ['not-a-version', '>', '1.0.0'] },
          result: null,
          note: 'Unparseable input or an unknown operator returns null, not an error',
        },
      ],
      notes: [
        'Operators: =, !=, <, <=, >, >=, ^ (same major), ~ (same major.minor)',
        'Normalizations: strip leading v/V, pad partial versions, coerce numbers, drop +build metadata',
        'Pre-releases sort before the release: 1.0.0-alpha < 1.0.0-beta < 1.0.0',
        'Malformed versions, wrong argument count or an unknown operator return null',
      ],
      seeAlso: ['fractional', 'if', '>='],
    },
    ui: {
      icon: 'tag',
      shortLabel: 'semver',
      nodeType: 'operator',
    },
  },
};
