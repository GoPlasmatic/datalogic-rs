import { describe, expect, it } from 'vitest';
import * as wasm from '@goplasmatic/datalogic-wasm/nodejs';
import { SAMPLE_EXPRESSIONS } from '../src/constants/sample-expressions';
import { EMBED_SAMPLE_EXPRESSIONS } from '../src/constants/embed-sample-expressions';

// Every Studio / embed example stores the engine's expected result
// (verified with the all-features dl-eval CLI when the sample was written).
// This keeps the Examples menu honest across engine upgrades.

function evaluateSample(sample: { logic: unknown; data: unknown; templating?: boolean }): unknown {
  return JSON.parse(
    wasm.evaluate(JSON.stringify(sample.logic), JSON.stringify(sample.data), !!sample.templating),
  );
}

describe('Studio sample expressions', () => {
  for (const [name, sample] of Object.entries(SAMPLE_EXPRESSIONS)) {
    it(`"${name}" evaluates to its stored expected result`, () => {
      expect(evaluateSample(sample)).toEqual(sample.expected);
    });
  }

  it('templating samples do not parse without templating', () => {
    const templated = Object.entries(SAMPLE_EXPRESSIONS).filter(([, s]) => s.templating);
    expect(templated.length).toBeGreaterThan(0);
    for (const [, sample] of templated) {
      expect(() =>
        wasm.evaluate(JSON.stringify(sample.logic), JSON.stringify(sample.data), false),
      ).toThrow();
    }
  });

  it('covers the v5 operator families the Examples menu is meant to surface', () => {
    const used = new Set<string>();
    const walk = (value: unknown) => {
      if (Array.isArray(value)) {
        value.forEach(walk);
      } else if (value && typeof value === 'object') {
        for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
          used.add(key);
          walk(child);
        }
      }
    };
    Object.values(SAMPLE_EXPRESSIONS).forEach((s) => walk(s.logic));

    const required = [
      'switch', '??', 'try', 'throw', 'type',
      'keys', 'values', 'entries', 'group_by', 'distinct', 'sort', 'slice', 'merge',
      'sem_ver', 'fractional',
      'datetime', 'format_date', 'parse_date', 'date_diff', 'timestamp',
      'missing', 'missing_some', 'exists',
      'upper', 'lower', 'trim', 'substr', 'split', 'starts_with', 'ends_with', 'in', 'length',
      'abs', 'ceil', 'floor', 'max', 'min', '%',
      '===', '!==', '!!',
      'val', 'var', 'map', 'filter', 'reduce', 'all', 'some', 'none',
    ];
    const missing = required.filter((op) => !used.has(op));
    expect(missing).toEqual([]);
  });

  it('exercises the val scope metadata form [[1], "index"]', () => {
    const json = JSON.stringify(SAMPLE_EXPRESSIONS['Numbered List'].logic);
    expect(json).toContain('"val":[[1],"index"]');
  });

  it('includes a non-object data root', () => {
    expect(Array.isArray(SAMPLE_EXPRESSIONS['Top-Level Array'].data)).toBe(true);
  });
});

describe('Embed sample expressions', () => {
  for (const [name, sample] of Object.entries(EMBED_SAMPLE_EXPRESSIONS)) {
    it(`"${name}" evaluates to its stored expected result`, () => {
      expect(evaluateSample(sample)).toEqual(sample.expected);
    });
  }
});
