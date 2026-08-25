import { describe, expect, it } from 'vitest';
import type { ExpressionNode } from '../../../types/trace';
import { normalizeExpression, findMatchingChild, matchOperandsToChildren, mayHaveTraceNode } from '../child-matching';

const child = (id: number, expression: string, children: ExpressionNode[] = []): ExpressionNode => ({ id, expression, children });

describe('normalizeExpression (engine canonical forms)', () => {
  it('collapses val paths into a dotted var', () => {
    expect(normalizeExpression({ val: ['a', 'b'] })).toEqual({ var: 'a.b' });
    expect(normalizeExpression({ val: 'a' })).toEqual({ var: 'a' });
    expect(normalizeExpression({ val: ['a'] })).toEqual({ var: 'a' });
    expect(normalizeExpression({ val: ['a', 0, 'b'] })).toEqual({ var: 'a.0.b' });
  });

  it('stringifies numeric var paths and unwraps single-element arrays', () => {
    expect(normalizeExpression({ var: 1 })).toEqual({ var: '1' });
    expect(normalizeExpression({ var: ['a'] })).toEqual({ var: 'a' });
    expect(normalizeExpression({ var: null })).toEqual({ var: '' });
    expect(normalizeExpression({ var: ['a', 0] })).toEqual({ var: ['a', 0] });
    expect(normalizeExpression({ var: ['a', { val: 'b' }] })).toEqual({ var: ['a', { var: 'b' }] });
  });

  it('keeps scoped val lookups, normalizing the level to its absolute value', () => {
    expect(normalizeExpression({ val: [[1], 'index'] })).toEqual({ val: [[1], 'index'] });
    expect(normalizeExpression({ val: [[-1], 'x'] })).toEqual({ val: [[1], 'x'] });
    expect(normalizeExpression({ var: [[1], 'a'] })).toEqual({ val: [[1], 'a'] });
    expect(normalizeExpression({ val: [[0], 'a', 'b'] })).toEqual({ var: 'a.b' });
  });

  it('maps ?: to if and match to switch', () => {
    expect(normalizeExpression({ '?:': [{ var: 'a' }, 1, 0] })).toEqual({ if: [{ var: 'a' }, 1, 0] });
    expect(normalizeExpression({ match: [{ val: 'x' }, [[1, 'one']], 'other'] })).toEqual({
      switch: [{ var: 'x' }, [[1, 'one']], 'other'],
    });
  });

  it('unwraps single-arg operators and throw objects', () => {
    expect(normalizeExpression({ '!': [{ var: 'a' }] })).toEqual({ '!': { var: 'a' } });
    expect(normalizeExpression({ throw: { type: 'boom' } })).toEqual({ throw: 'boom' });
    expect(normalizeExpression({ exists: ['a'] })).toEqual({ exists: 'a' });
  });
});

describe('findMatchingChild', () => {
  it('matches the engine expression string for rewritten operands', () => {
    const children = [child(3, '{"var": "a.b"}'), child(5, '{"if": [{"var": "a"}, 1, 0]}'), child(7, '{"var": "1"}')];
    expect(findMatchingChild({ val: ['a', 'b'] }, children, new Set())?.child.id).toBe(3);
    expect(findMatchingChild({ '?:': [{ var: 'a' }, 1, 0] }, children, new Set())?.child.id).toBe(5);
    expect(findMatchingChild({ var: 1 }, children, new Set())?.child.id).toBe(7);
    expect(findMatchingChild({ var: 'zzz' }, children, new Set())).toBeNull();
  });

  it('skips used indices', () => {
    const children = [child(1, '{"var": "a"}'), child(2, '{"var": "a"}')];
    expect(findMatchingChild({ var: 'a' }, children, new Set([0]))?.child.id).toBe(2);
  });
});

describe('matchOperandsToChildren', () => {
  it('pairs operands with children by content, ignoring child order', () => {
    const children = [child(2, '{"var": "b"}'), child(1, '{"var": "a"}')];
    const matches = matchOperandsToChildren([{ var: 'a' }, { var: 'b' }], children, false);
    expect(matches.map((m) => m?.child.id)).toEqual([1, 2]);
  });

  it('never assigns a child to a literal operand', () => {
    const children = [child(1, '{"var": "a"}')];
    const matches = matchOperandsToChildren([5, { var: 'zzz' }], children, false);
    expect(matches[0]).toBeNull();
    expect(matches[1]?.child.id).toBe(1);
  });

  it('matches a partially constant-folded operand by operator kind', () => {
    // {"cat": [{"+":[1,2]}, {"var":"a"}]} is traced as {"cat": [3, {"var": "a"}]}
    const children = [child(4, '{"cat": [3, {"var": "a"}]}'), child(6, '{"var": "b"}')];
    const matches = matchOperandsToChildren(
      [{ cat: [{ '+': [1, 2] }, { var: 'a' }] }, { var: 'b' }],
      children,
      false
    );
    expect(matches.map((m) => m?.child.id)).toEqual([4, 6]);
  });

  it('leaves a fully folded operand unmatched instead of stealing a sibling child', () => {
    // {"+": [{"+":[1,2]}, {"var":"a"}]} is traced with a single child {"var": "a"}
    const children = [child(2, '{"var": "a"}')];
    const matches = matchOperandsToChildren([{ '+': [1, 2] }, { var: 'a' }], children, false);
    expect(matches[0]).toBeNull();
    expect(matches[1]?.child.id).toBe(2);
  });

  it('falls back to positional matching only when leftovers pair up 1:1', () => {
    const children = [child(9, '{"throw": "string"}')];
    const matches = matchOperandsToChildren([{ throw: { type: 'boom', extra: 1 } }], children, false);
    expect(matches[0]?.child.id).toBe(9);
  });

  it('knows which operands may own a trace node', () => {
    expect(mayHaveTraceNode({ var: 'a' }, false)).toBe(true);
    expect(mayHaveTraceNode([{ var: 'a' }, 1], false)).toBe(true);
    expect(mayHaveTraceNode([1, 2], false)).toBe(false);
    expect(mayHaveTraceNode('x', false)).toBe(false);
    expect(mayHaveTraceNode({ a: 1, b: 2 }, false)).toBe(false);
    expect(mayHaveTraceNode({ a: 1, b: 2 }, true)).toBe(true);
  });
});
