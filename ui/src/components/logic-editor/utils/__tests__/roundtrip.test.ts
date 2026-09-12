/**
 * JSONLogic -> nodes -> JSONLogic round trip.
 *
 * Every rule in the corpus must come back deep-equal after
 * `nodesToJsonLogic(jsonLogicToNodes(rule).nodes)` and evaluate to the same
 * result (or the same error) in the WASM engine. The corpus covers every
 * builtin operator reported by the engine, the shipped playground samples,
 * and the shapes that used to be lossy (if/else-if chains, switch with null
 * cases, val scope/multi-segment paths, var defaults, exists array paths,
 * templating structures, single-value shorthand, unknown operators).
 *
 * Canonicalizations: none. The serializer reproduces the argument form the
 * rule was written in (single-value shorthand vs array, string vs array path,
 * signed scope arrays, `?:` vs `if`), so the round trip is exact for every
 * corpus entry. Rules the engine rejects (for example {"if": true}) are kept
 * as generic nodes and come back unchanged too.
 */

import { describe, expect, it } from 'vitest';
import * as wasm from '@goplasmatic/datalogic-wasm';
import { jsonLogicToNodes } from '../jsonlogic-to-nodes';
import { nodesToJsonLogic } from '../nodes-to-jsonlogic';
import type { JsonLogicValue } from '../../types';
import { SAMPLE_EXPRESSIONS } from '../../../../constants/sample-expressions';
import { EMBED_SAMPLE_EXPRESSIONS } from '../../../../constants/embed-sample-expressions';

interface Case {
  name: string;
  rule: JsonLogicValue;
  data?: unknown;
  templating?: boolean;
}

const ITEMS = { items: [10, 25, 5, 30], rows: [{ cols: [1, 2] }, { cols: [3] }], offset: 100 };

const corpus: Case[] = [
  // ---- literals at the root
  { name: 'lit-null', rule: null },
  { name: 'lit-bool', rule: true },
  { name: 'lit-num', rule: 42 },
  { name: 'lit-str', rule: 'hello' },
  { name: 'lit-empty-arr', rule: [] },
  { name: 'lit-arr', rule: [1, 2, 3] },
  { name: 'lit-obj-multi (not templating)', rule: { a: 1, b: 2 } },
  { name: 'lit-obj-empty', rule: {} },

  // ---- variable access
  { name: 'var-str', rule: { var: 'user.name' }, data: { user: { name: 'bob' } } },
  { name: 'var-empty', rule: { var: '' }, data: { a: 1 } },
  { name: 'var-arr1', rule: { var: ['user.name'] }, data: { user: { name: 'bob' } } },
  { name: 'var-arr-empty', rule: { var: [] }, data: { a: 1 } },
  { name: 'var-default-str', rule: { var: ['user.name', 'anon'] }, data: {} },
  { name: 'var-default-num', rule: { var: ['count', 0] }, data: {} },
  { name: 'var-default-null', rule: { var: ['count', null] }, data: {} },
  { name: 'var-default-expr', rule: { var: ['x', { var: 'fallback' }] }, data: { fallback: 7 } },
  { name: 'var-default-arith', rule: { var: ['x', { '+': [1, 2] }] }, data: {} },
  { name: 'var-num-index', rule: { var: 1 }, data: [5, 6] },
  { name: 'var-dynamic', rule: { var: { cat: ['a', 'b'] } }, data: { ab: 3 } },
  { name: 'val-str', rule: { val: 'user' }, data: { user: 1 } },
  { name: 'val-dotted-key', rule: { val: 'a.b' }, data: { 'a.b': 1, a: { b: 2 } } },
  { name: 'val-arr-path', rule: { val: ['user', 'name'] }, data: { user: { name: 'bob' } } },
  { name: 'val-arr-single', rule: { val: ['a'] }, data: { a: 1 } },
  { name: 'val-num-segment', rule: { val: ['items', 0] }, data: { items: [7] } },
  { name: 'val-scope-neg', rule: { val: [[-1], 'threshold'] }, data: { threshold: 3 } },
  { name: 'val-scope-pos', rule: { val: [[1], 'x'] }, data: { x: 1 } },
  { name: 'val-scope-zero', rule: { val: [[0], 'x'] }, data: { x: 1 } },
  { name: 'val-scope-2', rule: { val: [[-2], 'a', 'b'] }, data: { a: { b: 2 } } },
  { name: 'val-scope-only', rule: { val: [[-1]] }, data: { a: 1 } },
  { name: 'val-empty-arr', rule: { val: [] }, data: { a: 1 } },
  { name: 'val-empty-str', rule: { val: '' }, data: { '': 9, a: 1 } },
  { name: 'val-index-plain-key', rule: { val: 'index' }, data: { index: 9 } },
  { name: 'val-metadata-index', rule: { map: [[10, 20], { val: [[1], 'index'] }] } },
  { name: 'val-metadata-key', rule: { map: [{ a: 1, b: 2 }, { val: [[1], 'key'] }] } },
  { name: 'val-dynamic', rule: { val: { var: 'k' } }, data: { k: 'a', a: 5 } },
  { name: 'val-dynamic-arr', rule: { val: [{ var: 'k' }, 'x'] }, data: { k: 'a', a: { x: 5 } } },
  { name: 'exists-str', rule: { exists: 'user' }, data: { user: 1 } },
  { name: 'exists-dotted-key', rule: { exists: 'user.name' }, data: { user: { name: 1 } } },
  { name: 'exists-arr', rule: { exists: ['user', 'name'] }, data: { user: { name: 1 } } },
  { name: 'exists-arr1', rule: { exists: ['user'] }, data: { user: 1 } },
  { name: 'exists-dynamic', rule: { exists: { var: 'k' } }, data: { k: 'a', a: 1 } },
  { name: 'missing-str', rule: { missing: 'a' }, data: {} },
  { name: 'missing-arr', rule: { missing: ['a', 'b'] }, data: { a: 1 } },
  { name: 'missing_some', rule: { missing_some: [1, ['a', 'b']] }, data: { a: 1 } },

  // ---- comparison
  { name: 'eq', rule: { '==': [1, 1] } },
  { name: 'eq-var', rule: { '==': [{ var: 'a' }, 1] }, data: { a: 1 } },
  { name: 'eq-3vars', rule: { '==': [{ var: 'a' }, { var: 'b' }, { var: 'c' }] }, data: { a: 1, b: 1, c: 1 } },
  { name: 'seq', rule: { '===': [1, '1'] } },
  { name: 'ne', rule: { '!=': [1, 2] } },
  { name: 'sne', rule: { '!==': [1, '1'] } },
  { name: 'gt', rule: { '>': [2, 1] } },
  { name: 'gte', rule: { '>=': [2, 1] } },
  { name: 'lt-chain', rule: { '<': [1, { var: 'x' }, 10] }, data: { x: 5 } },
  { name: 'lte-chain', rule: { '<=': [1, 2, 3, 4] } },

  // ---- logical
  { name: 'not', rule: { '!': [true] } },
  { name: 'not-shorthand', rule: { '!': true } },
  { name: 'not-var-shorthand', rule: { '!': { var: 'x' } }, data: { x: 0 } },
  { name: 'not-empty-arr', rule: { '!': [] } },
  { name: 'notnot', rule: { '!!': [0] } },
  { name: 'notnot-shorthand', rule: { '!!': 'x' } },
  { name: 'and', rule: { and: [true, false] } },
  { name: 'and-vars', rule: { and: [{ var: 'a' }, { var: 'b' }] }, data: { a: 1, b: 2 } },
  { name: 'and-nested', rule: { and: [{ '>': [{ var: 'a' }, 1] }, { or: [{ var: 'b' }, false] }] }, data: { a: 2, b: 0 } },
  { name: 'and-empty', rule: { and: [] } },
  { name: 'or', rule: { or: [false, true] } },
  { name: 'or-empty', rule: { or: [] } },

  // ---- if / ?: (flat chains, nested, degenerate)
  { name: 'if-3', rule: { if: [true, 1, 2] } },
  { name: 'if-2', rule: { if: [true, 1] } },
  { name: 'if-1', rule: { if: [1] } },
  { name: 'if-0', rule: { if: [] } },
  { name: 'if-shorthand (engine rejects, kept)', rule: { if: true } },
  { name: 'if-5 chain', rule: { if: [{ var: 'a' }, 1, { var: 'b' }, 2, 3] }, data: { a: false, b: false } },
  { name: 'if-4 chain no else', rule: { if: [{ var: 'a' }, 1, { var: 'b' }, 2] }, data: { a: false, b: false } },
  { name: 'if-7 chain', rule: { if: [false, 1, false, 2, false, 3, 4] } },
  { name: 'if-nested-then', rule: { if: [true, { if: [false, 1, 2] }, 3] } },
  { name: 'if-nested-else (explicit)', rule: { if: [{ var: 'a' }, 1, { if: [{ var: 'b' }, 2, 3] }] }, data: { a: false, b: false } },
  { name: 'if-object-branches', rule: { if: [{ var: 'x' }, { a: 1 }, { b: 2 }] }, data: { x: true } },
  { name: 'ternary', rule: { '?:': [true, 1, 2] } },
  { name: 'ternary-chain', rule: { '?:': [false, 1, true, 2, 3] } },
  { name: 'nullish', rule: { '??': [null, null, 3] } },
  { name: 'nullish-var', rule: { '??': [{ var: 'x' }, 'd'] }, data: {} },

  // ---- switch / match
  { name: 'switch', rule: { switch: [{ var: 'x' }, [[1, 'one'], [2, 'two']], 'other'] }, data: { x: 2 } },
  { name: 'switch-null-case', rule: { switch: [{ var: 'x' }, [[null, 'none'], [1, 'one']], 'other'] }, data: {} },
  { name: 'switch-falsy-cases', rule: { switch: [{ var: 'v' }, [[null, 'null'], [false, 'false'], [0, 'zero'], ['', 'empty']], 'other'] }, data: { v: 0 } },
  { name: 'switch-expr-results', rule: { switch: [{ var: 'x' }, [[1, { var: 'one' }], [{ var: 'k' }, { '+': [1, 2] }]], { var: 'd' }] }, data: { x: 1, one: 'ONE' } },
  { name: 'switch-no-default', rule: { switch: [{ var: 'x' }, [[1, 'one']]] }, data: { x: 3 } },
  { name: 'switch-disc-only', rule: { switch: [{ var: 'x' }] }, data: { x: 1 } },
  { name: 'switch-empty-cases', rule: { switch: [{ var: 'x' }, []] }, data: { x: 1 } },
  { name: 'switch-dynamic-cases (kept)', rule: { switch: [{ var: 'x' }, { var: 'cases' }, 'other'] }, data: { x: 1 } },
  { name: 'switch-malformed-pair (kept)', rule: { switch: [{ var: 'x' }, [[1]], 'other'] }, data: { x: 1 } },
  { name: 'match', rule: { match: [{ var: 'x' }, [[[1, 2], 'arr'], ['^a', 'regex']], 'd'] }, data: { x: 'abc' } },

  // ---- arithmetic
  { name: 'plus', rule: { '+': [1, 2] } },
  { name: 'plus-nested', rule: { '+': [{ '*': [2, 3] }, { '-': [5, 1] }] } },
  { name: 'plus-unary', rule: { '+': ['1'] } },
  { name: 'plus-shorthand', rule: { '+': '1' } },
  { name: 'plus-empty', rule: { '+': [] } },
  { name: 'minus-unary', rule: { '-': [5] } },
  { name: 'times', rule: { '*': [2, 3] } },
  { name: 'div', rule: { '/': [6, 2] } },
  { name: 'mod', rule: { '%': [7, 3] } },
  { name: 'max', rule: { max: [1, 5, 3] } },
  { name: 'max-arr-arg', rule: { max: [[1, 5, 3]] } },
  { name: 'max-var', rule: { max: { var: 'arr' } }, data: { arr: [1, 9] } },
  { name: 'min', rule: { min: [1, 5, 3] } },
  { name: 'abs', rule: { abs: [-1] } },
  { name: 'abs-shorthand', rule: { abs: -1 } },
  { name: 'ceil', rule: { ceil: [1.2] } },
  { name: 'floor', rule: { floor: [1.8] } },
  { name: 'fractional', rule: { fractional: [{ var: 'k' }, 20] }, data: { k: 'user-1' } },
  { name: 'sem_ver', rule: { sem_ver: ['1.2.3', '>=', '1.0.0'] } },
  { name: 'sem_ver-var', rule: { sem_ver: [{ var: 'v' }, '~', '1.2.0'] }, data: { v: '1.2.9' } },
  // ---- tensor: one rule per operator, so the corpus keeps covering
  // every builtin the engine reports.
  { name: 'tensor', rule: { tensor: [[1, 2], 'u8'] } },
  { name: 'tensor wire form', rule: { tensor: { dtype: 'u8', shape: [2], data: 'AQI=' } } },
  { name: 'zeros', rule: { zeros: [[2, 2], 'i32'] } },
  { name: 'full', rule: { full: [[2], 'f32', 1.5] } },
  { name: 'scatter', rule: { scatter: [[[0, 1]], [2, 2], 'u8'] } },
  { name: 'rle_expand', rule: { rle_expand: [[0, 2, 1, 2], [2, 2], 'u8'] } },
  { name: 'one_hot', rule: { one_hot: [[0, 2], 3, 'u8'] } },
  { name: 'stack', rule: { stack: [[{ tensor: [[1, 2], 'u8'] }, { tensor: [[3, 4], 'u8'] }], 0] } },
  { name: 'concat', rule: { concat: [[{ tensor: [[1], 'u8'] }, { tensor: [[2], 'u8'] }], 0] } },
  { name: 'unstack', rule: { unstack: [{ tensor: [[[1, 2], [3, 4]], 'u8'] }, 0] } },
  { name: 'reshape', rule: { reshape: [{ tensor: [[1, 2, 3, 4], 'u8'] }, [2, 2]] } },
  { name: 'transpose', rule: { transpose: [{ tensor: [[[1, 2], [3, 4]], 'u8'] }] } },
  { name: 'transpose with perm', rule: { transpose: [{ tensor: [[[1, 2], [3, 4]], 'u8'] }, [1, 0]] } },
  { name: 'pad', rule: { pad: [{ tensor: [[1, 2], 'u8'] }, [1], [1], 9] } },
  { name: 'crop', rule: { crop: [{ tensor: [[1, 2, 3, 4], 'u8'] }, [1], [2]] } },
  { name: 'cast', rule: { cast: [{ tensor: [[1.7], 'f64'] }, 'u8'] } },
  { name: 'normalize', rule: { normalize: [{ tensor: [[0, 255], 'u8'] }, 127.5, 0.5] } },
  { name: 'argmax', rule: { argmax: [{ tensor: [[1, 9, 3], 'f32'] }, 0] } },
  { name: 'gather', rule: { gather: [{ tensor: [[10, 20, 30], 'u8'] }, [2, 0]] } },
  { name: 'to_list', rule: { to_list: [{ tensor: [[[1, 2], [3, 4]], 'i32'] }] } },
  { name: 'shape', rule: { shape: [{ tensor: [[1, 2, 3], 'u8'] }] } },
  { name: 'dtype', rule: { dtype: [{ tensor: [[1], 'f32'] }] } },

  // ---- string
  { name: 'cat', rule: { cat: ['a', 'b'] } },
  { name: 'cat-shorthand', rule: { cat: 'a' } },
  { name: 'cat-vars', rule: { cat: ['Hi ', { var: 'name' }] }, data: { name: 'x' } },
  { name: 'cat-3vars', rule: { cat: [{ var: 'a' }, { var: 'b' }, { var: 'c' }] }, data: { a: 'x', b: 'y', c: 'z' } },
  { name: 'substr-2', rule: { substr: ['hello', 1] } },
  { name: 'substr-3', rule: { substr: ['hello', 1, 2] } },
  { name: 'in', rule: { in: ['a', 'abc'] } },
  { name: 'in-arr', rule: { in: [{ var: 'x' }, [1, 2, 3]] }, data: { x: 2 } },
  { name: 'length', rule: { length: ['abc'] } },
  { name: 'length-shorthand', rule: { length: 'abc' } },
  { name: 'length-bare-array (engine rejects, kept)', rule: { length: [1, 2, 3] } },
  { name: 'length-var', rule: { length: { var: 'arr' } }, data: { arr: [1, 2] } },
  { name: 'starts_with', rule: { starts_with: ['abc', 'a'] } },
  { name: 'ends_with', rule: { ends_with: ['abc', 'c'] } },
  { name: 'upper', rule: { upper: ['abc'] } },
  { name: 'lower', rule: { lower: ['ABC'] } },
  { name: 'trim', rule: { trim: ['  a '] } },
  { name: 'split', rule: { split: ['a,b', ','] } },
  { name: 'type', rule: { type: [1] } },
  { name: 'type-shorthand', rule: { type: { var: 'x' } }, data: { x: 'a' } },

  // ---- array / iteration / collection
  { name: 'merge', rule: { merge: [[1], [2], 3] } },
  { name: 'merge-empty', rule: { merge: [] } },
  { name: 'filter', rule: { filter: [{ var: 'items' }, { '>': [{ var: '' }, 6] }] }, data: ITEMS },
  { name: 'map', rule: { map: [{ var: 'items' }, { '*': [{ var: '' }, 2] }] }, data: ITEMS },
  { name: 'map-literal-arr', rule: { map: [[1, 2, 3], { '*': [{ var: '' }, 2] }] } },
  { name: 'map-empty-arr', rule: { map: [[], { var: '' }] } },
  { name: 'reduce', rule: { reduce: [{ var: 'items' }, { '+': [{ var: 'current' }, { var: 'accumulator' }] }, 0] }, data: ITEMS },
  { name: 'all', rule: { all: [{ var: 'items' }, { '>': [{ var: '' }, 0] }] }, data: ITEMS },
  { name: 'all-empty', rule: { all: [[], true] } },
  { name: 'some', rule: { some: [{ var: 'items' }, { '>': [{ var: '' }, 20] }] }, data: ITEMS },
  { name: 'none', rule: { none: [{ var: 'items' }, { '<': [{ var: '' }, 0] }] }, data: ITEMS },
  { name: 'nested-iter-val-scope', rule: { map: [{ var: 'rows' }, { map: [{ val: 'cols' }, { '+': [{ val: [] }, { val: [[2], 'offset'] }] }] }] }, data: ITEMS },
  { name: 'sort', rule: { sort: [[3, 1, 2]] } },
  { name: 'sort-desc', rule: { sort: [[3, 1, 2], false] } },
  { name: 'sort-key', rule: { sort: [[{ k: 2 }, { k: 1 }], true, { var: 'k' }] } },
  { name: 'slice', rule: { slice: [[1, 2, 3, 4], 1, 3] } },
  { name: 'slice-step', rule: { slice: [[1, 2, 3, 4], 0, 4, 2] } },
  { name: 'group_by', rule: { group_by: [[{ k: 1 }, { k: 2 }, { k: 1 }], { var: 'k' }] } },
  { name: 'distinct', rule: { distinct: [[1, 1, 2]] } },
  { name: 'keys', rule: { keys: [{ var: 'o' }] }, data: { o: { a: 1 } } },
  { name: 'values', rule: { values: [{ var: 'o' }] }, data: { o: { a: 1 } } },
  { name: 'entries', rule: { entries: [{ var: 'o' }] }, data: { o: { a: 1 } } },

  // ---- datetime
  { name: 'datetime-shorthand', rule: { datetime: '2024-01-15T10:30:00Z' } },
  { name: 'datetime', rule: { datetime: ['2024-01-15T10:30:00Z'] } },
  { name: 'timestamp', rule: { timestamp: ['1d:2h:3m:4s'] } },
  { name: 'parse_date', rule: { parse_date: ['2024-01-15', '%Y-%m-%d'] } },
  { name: 'parse_date-tz', rule: { parse_date: ['2024-01-15 10:30', '%Y-%m-%d %H:%M', 'Europe/Berlin'] } },
  { name: 'format_date', rule: { format_date: [{ datetime: '2024-01-15T10:30:00Z' }, '%Y-%m-%d'] } },
  { name: 'format_date-tz', rule: { format_date: [{ datetime: '2024-01-15T10:30:00Z' }, '%H:%M', 'Asia/Kolkata'] } },
  { name: 'date_diff', rule: { date_diff: [{ datetime: '2024-01-15T00:00:00Z' }, { datetime: '2024-01-10T00:00:00Z' }, 'days'] } },
  { name: 'date_diff-bad-unit (error kept)', rule: { date_diff: [{ datetime: '2024-01-15T00:00:00Z' }, { datetime: '2024-01-10T00:00:00Z' }, 'weeks'] } },
  { name: 'now', rule: { now: [] } },
  { name: 'now-null-shorthand', rule: { now: null } },

  // ---- error handling / utility / unknown
  { name: 'try-catch-message', rule: { try: [{ throw: 'boom' }, { var: 'type' }] } },
  { name: 'try-catch-engine-error', rule: { try: [{ '/': [1, 0] }, { var: 'type' }] } },
  { name: 'throw-shorthand', rule: { throw: 'boom' } },
  { name: 'throw-obj', rule: { throw: [{ cat: ['err', 'or'] }] } },
  { name: 'unknown-op', rule: { my_custom: 'x' } },
  { name: 'unknown-op-arr', rule: { my_custom: [1, { var: 'x' }] }, data: { x: 1 } },
  { name: 'single-key-non-op', rule: { a: 1 } },

  // ---- templating structures
  { name: 'tpl-object-literal', rule: { name: 'x' }, templating: true },
  { name: 'tpl-object', rule: { name: { var: 'user.name' }, age: 30 }, data: { user: { name: 'bob' } }, templating: true },
  { name: 'tpl-nested', rule: { user: { name: { var: 'n' }, tags: [{ var: 't' }, 'x'] }, ok: true }, data: { n: 'bob', t: 'a' }, templating: true },
  { name: 'tpl-array', rule: [1, 2, 3], templating: true },
  { name: 'tpl-array-expr', rule: [1, { var: 'a' }, 3], data: { a: 2 }, templating: true },
  { name: 'tpl-in-literal-array', rule: { in: [{ var: 'x' }, [1, 2, 3]] }, data: { x: 2 }, templating: true },
  { name: 'tpl-merge', rule: { merge: [[1], [2]] }, templating: true },
  { name: 'tpl-if-objects', rule: { if: [{ var: 'x' }, { a: 1 }, { b: 2 }] }, data: { x: true }, templating: true },
  { name: 'tpl-map-object', rule: { map: [{ var: 'items' }, { id: { var: 'id' }, n: { '+': [1, { var: 'k' }] } }] }, data: { items: [{ id: 1, k: 2 }] }, templating: true },
  { name: 'tpl-structured-result', rule: { result: { value: { '+': [1, 2, 3] } } }, templating: true },
  { name: 'tpl-sum-product-literal', rule: { sum: { '+': [1, 2] }, product: { '*': [3, 4] }, literal: 42 }, templating: true },
  { name: 'tpl-filtered', rule: { filtered: { filter: [[1, 2, 3, 4], { '<': [{ var: '' }, 3] }] } }, templating: true },
  { name: 'tpl-single-key-non-op', rule: { a: 1 }, templating: true },
  { name: 'tpl-empty-object', rule: {}, templating: true },
  { name: 'tpl-empty-array', rule: [], templating: true },
];

// The shipped playground samples (both variants)
for (const [name, sample] of Object.entries(SAMPLE_EXPRESSIONS)) {
  corpus.push({ name: `sample: ${name}`, rule: sample.logic, data: sample.data, templating: name.includes('Structure') });
}
for (const [name, sample] of Object.entries(EMBED_SAMPLE_EXPRESSIONS)) {
  corpus.push({ name: `embed sample: ${name}`, rule: sample.logic, data: sample.data, templating: name.includes('Structure') });
}

function roundTrip(rule: JsonLogicValue, templating: boolean): JsonLogicValue | null {
  const { nodes } = jsonLogicToNodes(rule, { templating });
  return nodesToJsonLogic(nodes);
}

interface Outcome {
  ok: boolean;
  value?: unknown;
  error?: string;
}

function evaluate(rule: JsonLogicValue | null, data: unknown, templating: boolean): Outcome {
  try {
    const result = wasm.evaluate(JSON.stringify(rule), JSON.stringify(data ?? {}), templating);
    return { ok: true, value: JSON.parse(result) };
  } catch (err) {
    return { ok: false, error: String(err) };
  }
}

/** Collect every operator key used anywhere in a rule. */
function collectOperators(value: unknown, into: Set<string>): void {
  if (Array.isArray(value)) {
    value.forEach((v) => collectOperators(v, into));
  } else if (value && typeof value === 'object') {
    const keys = Object.keys(value);
    if (keys.length === 1) into.add(keys[0]);
    keys.forEach((k) => collectOperators((value as Record<string, unknown>)[k], into));
  }
}

describe('JSONLogic round trip (jsonLogicToNodes -> nodesToJsonLogic)', () => {
  it('has a corpus of at least 60 rules covering every builtin operator', () => {
    expect(corpus.length).toBeGreaterThanOrEqual(60);
    const used = new Set<string>();
    corpus.forEach((c) => collectOperators(c.rule, used));
    const missing = wasm.builtinOperatorNames().filter((name) => !used.has(name));
    expect(missing).toEqual([]);
  });

  it.each(corpus.map((c) => [c.name, c] as const))('%s serializes back unchanged', (_name, c) => {
    const templating = c.templating ?? false;
    const out = roundTrip(c.rule, templating);
    expect(out).toEqual(c.rule);
  });

  it.each(corpus.map((c) => [c.name, c] as const))('%s evaluates the same after the round trip', (_name, c) => {
    const templating = c.templating ?? false;
    const out = roundTrip(c.rule, templating);
    const before = evaluate(c.rule, c.data, templating);
    const after = evaluate(out, c.data, templating);
    if (c.rule && typeof c.rule === 'object' && 'now' in (c.rule as object)) {
      // now() differs between calls; only the outcome kind is comparable
      expect(after.ok).toBe(before.ok);
      return;
    }
    expect(after).toEqual(before);
  });

  it('flattens an else-if chain that was converted through nested diamonds', () => {
    const rule = { if: [{ '>=': [{ var: 's' }, 90] }, 'A', { '>=': [{ var: 's' }, 80] }, 'B', { '>=': [{ var: 's' }, 70] }, 'C', 'F'] };
    const { nodes } = jsonLogicToNodes(rule);
    const diamonds = nodes.filter((n) => n.data.type === 'operator' && n.data.operator === 'if');
    expect(diamonds).toHaveLength(3);
    expect(diamonds.map((d) => d.data.label).sort()).toEqual(['elif', 'elif', 'if']);
    expect(nodesToJsonLogic(nodes)).toEqual(rule);
  });

  it('keeps the engine result of the Grade Calculator sample after an edit-style re-serialization', () => {
    const sample = SAMPLE_EXPRESSIONS['Grade Calculator'];
    const out = roundTrip(sample.logic, false);
    expect(evaluate(out, sample.data, false)).toEqual({ ok: true, value: 'C - Average' });
  });

  it('never emits the plain-string metadata form from a scoped metadata read', () => {
    const rule = { map: [[10, 20], { val: [[1], 'index'] }] };
    const out = roundTrip(rule, false);
    expect((out as { map: unknown[] }).map[1]).toEqual({ val: [[1], 'index'] });
    expect(evaluate(out, {}, false)).toEqual({ ok: true, value: [0, 1] });
  });
});
