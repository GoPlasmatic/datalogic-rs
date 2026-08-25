import { describe, expect, it } from 'vitest';
import type { JsonLogicValue } from '../../../types';
import { analyze, collectOperators, countByType, wasm } from './helpers';

/**
 * One rule per builtin operator (plus the aliases), each referencing data so
 * the compiler cannot constant-fold the operator away. Every rule is traced
 * through the real engine, converted with traceToNodes and compared with the
 * static converter.
 *
 * `unevaluated` is the number of operator nodes the engine legitimately never
 * reaches (an untaken if / switch branch, an elif diamond), i.e. nodes that
 * have a trace id but record no step.
 */
interface CorpusRule {
  name: string;
  rule: JsonLogicValue;
  templating?: boolean;
  unevaluated?: number;
}

const DATA = {
  a: 5,
  b: 3,
  s: 'Hello World',
  pad: '  hi  ',
  xs: [3, 1, 2],
  dups: [1, 1, 2],
  obj: { k: 1, j: 2 },
  t: '2024-01-15T10:00:00Z',
  t2: '2024-01-16T12:30:00Z',
  d: '2024-06-15',
  fmt: 'yyyy-MM-dd',
  dur: '1d:2h:3m:4s',
  list: [{ n: 1, g: 'x' }, { n: 2, g: 'y' }],
  csv: 'a,b,c',
  f: 3.7,
  ver: '1.2.3',
  neg: -2,
  flag: true,
  nil: null,
  key: 'user-1',
  e: 'boom',
};

const CORPUS: CorpusRule[] = [
  // variables
  { name: 'var', rule: { var: 'a' } },
  { name: 'var with expression default', rule: { var: ['zz', { var: 'b' }] } },
  { name: 'var numeric path', rule: { '+': [{ var: 1 }, { var: 'a' }] } },
  { name: 'val path array', rule: { val: ['obj', 'k'] } },
  { name: 'val plain', rule: { val: 'a' } },
  { name: 'exists key', rule: { exists: 'a' } },
  { name: 'exists nested', rule: { exists: ['obj', 'k'] } },
  // comparison
  { name: '==', rule: { '==': [{ var: 'a' }, 5] } },
  { name: '===', rule: { '===': [{ var: 'a' }, { var: 'b' }] } },
  { name: '!=', rule: { '!=': [{ var: 'a' }, { var: 'b' }] } },
  { name: '!==', rule: { '!==': [{ var: 'a' }, { var: 'b' }] } },
  { name: '>', rule: { '>': [{ var: 'a' }, { var: 'b' }] } },
  { name: '>=', rule: { '>=': [{ var: 'a' }, { var: 'b' }] } },
  { name: '<', rule: { '<': [{ var: 'a' }, { var: 'b' }] } },
  { name: '<=', rule: { '<=': [{ var: 'a' }, { var: 'b' }] } },
  // logical
  { name: '!', rule: { '!': { var: 'flag' } } },
  { name: '!! (array form)', rule: { '!!': [{ var: 'nil' }] } },
  { name: 'and', rule: { and: [{ var: 'flag' }, { '>': [{ var: 'a' }, 1] }] } },
  { name: 'or', rule: { or: [{ var: 'nil' }, { var: 'a' }] } },
  { name: '??', rule: { '??': [{ var: 'nil' }, { var: 'zz' }, { var: 'a' }] } },
  // control
  { name: 'if', rule: { if: [{ var: 'flag' }, { var: 'a' }, { var: 'b' }] }, unevaluated: 1 },
  { name: '?:', rule: { '?:': [{ var: 'flag' }, { var: 'a' }, { var: 'b' }] }, unevaluated: 1 },
  {
    name: 'if / elif chain',
    rule: { if: [{ '<': [{ var: 'a' }, 1] }, { var: 's' }, { '<': [{ var: 'a' }, 10] }, { var: 'b' }, { var: 'nil' }] },
    unevaluated: 2,
  },
  {
    name: 'switch',
    rule: {
      switch: [
        { var: 'a' },
        [[{ var: 'b' }, 'three'], [{ '+': [{ var: 'b' }, 2] }, { cat: ['five ', { var: 's' }] }]],
        { var: 's' },
      ],
    },
    unevaluated: 1,
  },
  {
    name: 'match',
    rule: { match: [{ var: 'a' }, [[{ var: 'b' }, { var: 's' }], [{ var: 'a' }, { upper: { var: 's' } }]], { var: 'b' }] },
    unevaluated: 2,
  },
  { name: 'try / throw', rule: { try: [{ throw: { var: 'e' } }, { cat: ['caught:', { var: 'type' }] }] } },
  // arithmetic
  { name: '+', rule: { '+': [{ var: 'a' }, { var: 'b' }, 1] } },
  { name: '-', rule: { '-': [{ var: 'a' }, { var: 'b' }] } },
  { name: '- unary', rule: { '-': { var: 'a' } } },
  { name: '*', rule: { '*': [{ var: 'a' }, 2] } },
  { name: '/', rule: { '/': [{ var: 'a' }, { var: 'b' }] } },
  { name: '%', rule: { '%': [{ var: 'a' }, { var: 'b' }] } },
  { name: 'max', rule: { max: [{ var: 'a' }, { var: 'b' }, 4] } },
  { name: 'min', rule: { min: { var: 'xs' } } },
  { name: 'abs', rule: { abs: { var: 'neg' } } },
  { name: 'ceil', rule: { ceil: { var: 'f' } } },
  { name: 'floor', rule: { floor: { var: 'f' } } },
  // string
  { name: 'cat', rule: { cat: [{ var: 's' }, '!', { var: 'a' }] } },
  { name: 'substr', rule: { substr: [{ var: 's' }, 0, { var: 'b' }] } },
  { name: 'in (string)', rule: { in: ['World', { var: 's' }] } },
  { name: 'length', rule: { length: { var: 's' } } },
  { name: 'starts_with', rule: { starts_with: [{ var: 's' }, 'He'] } },
  { name: 'ends_with', rule: { ends_with: [{ var: 's' }, 'ld'] } },
  { name: 'upper', rule: { upper: { var: 's' } } },
  { name: 'lower', rule: { lower: { var: 's' } } },
  { name: 'trim', rule: { trim: { var: 'pad' } } },
  { name: 'split', rule: { split: [{ var: 'csv' }, ','] } },
  { name: 'type', rule: { type: { var: 'a' } } },
  // arrays
  { name: 'in (array with expression)', rule: { in: [{ var: 'a' }, [{ var: 'b' }, 5]] } },
  { name: 'merge', rule: { merge: [{ var: 'xs' }, [{ var: 'a' }], 7] } },
  { name: 'filter', rule: { filter: [{ var: 'xs' }, { '>': [{ var: '' }, 1] }] } },
  { name: 'map', rule: { map: [{ var: 'xs' }, { '*': [{ var: '' }, { val: [[1], 'index'] }] }] } },
  { name: 'reduce', rule: { reduce: [{ var: 'xs' }, { '+': [{ var: 'current' }, { var: 'accumulator' }] }, 0] } },
  { name: 'all', rule: { all: [{ var: 'xs' }, { '>': [{ var: '' }, 0] }] } },
  { name: 'some', rule: { some: [{ var: 'xs' }, { '>': [{ var: '' }, 2] }] } },
  { name: 'none', rule: { none: [{ var: 'xs' }, { '>': [{ var: '' }, 5] }] } },
  { name: 'sort', rule: { sort: [{ var: 'xs' }, false] } },
  { name: 'sort by key', rule: { sort: [{ var: 'list' }, true, { var: 'n' }] } },
  { name: 'slice', rule: { slice: [{ var: 'xs' }, 1, 3] } },
  { name: 'group_by', rule: { group_by: [{ var: 'list' }, { var: 'g' }] } },
  { name: 'distinct', rule: { distinct: { var: 'dups' } } },
  { name: 'keys', rule: { keys: { var: 'obj' } } },
  { name: 'values', rule: { values: { var: 'obj' } } },
  { name: 'entries', rule: { entries: { var: 'obj' } } },
  { name: 'missing', rule: { missing: ['a', 'zz'] } },
  { name: 'missing_some', rule: { missing_some: [1, ['a', 'zz']] } },
  // datetime
  { name: 'datetime', rule: { datetime: { var: 't' } } },
  { name: 'timestamp', rule: { timestamp: { var: 'dur' } } },
  { name: 'parse_date', rule: { parse_date: [{ var: 'd' }, { var: 'fmt' }] } },
  { name: 'format_date', rule: { format_date: [{ datetime: { var: 't' } }, 'yyyy-MM-dd'] } },
  { name: 'date_diff', rule: { date_diff: [{ datetime: { var: 't2' } }, { datetime: { var: 't' } }, 'hours'] } },
  { name: 'now', rule: { now: [] } },
  // flagd
  { name: 'fractional', rule: { fractional: [{ var: 'key' }, ['red', 50], ['blue', 50]] } },
  { name: 'sem_ver', rule: { sem_ver: [{ var: 'ver' }, '>=', '1.0.0'] } },
  // templating structures
  {
    name: 'template object with nested structures',
    rule: { name: { var: 's' }, nested: { k: { '+': [{ var: 'a' }, 2] } }, plain: { b: 1 }, arr: [{ var: 'a' }, 1] },
    templating: true,
  },
  { name: 'template root array', rule: [{ var: 'a' }, 1], templating: true },
  { name: 'template data-only nested object', rule: { a: { b: 1 }, c: { var: 'a' } }, templating: true },
  { name: 'template nested array of objects', rule: { items: [{ v: { var: 'a' } }] }, templating: true },
  { name: 'template single unknown key', rule: { k: { var: 'a' } }, templating: true },
  { name: 'template with if', rule: { x: { if: [{ var: 'flag' }, { var: 'a' }, { var: 'b' }] } }, templating: true, unevaluated: 1 },
];

describe('trace to nodes corpus (real engine traces)', () => {
  it('covers every builtin operator name', () => {
    const used = new Set<string>();
    for (const entry of CORPUS) collectOperators(entry.rule, used);
    const missing = wasm.builtinOperatorNames().filter((name) => !used.has(name));
    expect(missing).toEqual([]);
  });

  for (const entry of CORPUS) {
    it(`maps every step for: ${entry.name}`, () => {
      const a = analyze(entry.rule, DATA, entry.templating ?? false);
      const detail = JSON.stringify({
        steps: a.trace.steps.map((s) => s.node_id),
        map: [...a.result.traceNodeMap.entries()],
        nodes: a.result.nodes.map((n) => n.id),
      });

      expect(a.trace.structured_error, detail).toBeUndefined();
      expect(a.trace.steps.length, detail).toBeGreaterThan(0);

      // every step lands on an existing visual node
      expect(a.unmappedSteps, detail).toEqual([]);

      // every operator / structure node carries a real trace id
      expect(a.syntheticNodes.map((n) => n.id), detail).toEqual([]);

      // same diagram shape as the static converter
      expect(countByType(a.result.nodes, 'operator'), detail).toBe(countByType(a.staticResult.nodes, 'operator'));
      expect(countByType(a.result.nodes, 'structure'), detail).toBe(countByType(a.staticResult.nodes, 'structure'));
      expect(a.result.nodes.length, detail).toBe(a.staticResult.nodes.length);

      // every reachable operator node records at least one step
      expect(a.stepless.map((n) => n.id).length, detail).toBe(entry.unevaluated ?? 0);

      expect(a.duplicateEdgeIds, detail).toEqual([]);
    });
  }
});
