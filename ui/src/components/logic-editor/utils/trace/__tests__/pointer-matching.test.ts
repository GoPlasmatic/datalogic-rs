import { describe, expect, it } from 'vitest';
import { runTrace } from './helpers';
import {
  TraceSources,
  enclosingNode,
  matchOperandsToChildren,
  resolvePointer,
  unmatchedChildren,
} from '../pointer-matching';
import type { ExpressionNode } from '../../../types/trace';
import type { JsonLogicValue } from '../../../types';

const node = (id: number, children: ExpressionNode[] = []): ExpressionNode => ({
  id,
  expression: '',
  children,
});

describe('resolvePointer', () => {
  const rule = { if: [{ var: 'a' }, [1, { 'a/b': { '~x': 2 } }]] };

  it('walks objects and arrays', () => {
    expect(resolvePointer(rule, '')).toBe(rule);
    expect(resolvePointer(rule, '/if/0')).toBe(rule.if[0]);
    expect(resolvePointer(rule, '/if/1/0')).toBe(1);
  });

  it('unescapes ~1 and ~0', () => {
    expect(resolvePointer(rule, '/if/1/1/a~1b/~0x')).toBe(2);
  });

  it('names nothing for a missing key, a bad index or a malformed pointer', () => {
    expect(resolvePointer(rule, '/nope')).toBeUndefined();
    expect(resolvePointer(rule, '/if/7')).toBeUndefined();
    expect(resolvePointer(rule, '/if/01')).toBeUndefined();
    expect(resolvePointer(rule, '/if/x')).toBeUndefined();
    expect(resolvePointer(rule, 'if')).toBeUndefined();
    expect(resolvePointer({ a: 1 }, '/toString')).toBeUndefined();
  });
});

describe('matchOperandsToChildren', () => {
  it('pairs each operand with the child compiled from it, whatever the order', () => {
    const a = { var: 'a' };
    const b = { var: 'b' };
    const rule = { '+': [a, b] };
    const sources = new TraceSources(rule, { '1': '/+/1', '2': '/+/0', '3': '' });
    const children = [node(1), node(2)];
    const matches = matchOperandsToChildren([a, b], children, sources);
    expect(matches.map((m) => m?.child.id)).toEqual([2, 1]);
  });

  it('never pairs a literal operand', () => {
    const rule = { '+': [5, { var: 'x' }] };
    const sources = new TraceSources(rule, { '1': '/+/1' });
    const matches = matchOperandsToChildren(rule['+'], [node(1)], sources);
    expect(matches.map((m) => m?.child.id ?? null)).toEqual([null, 1]);
  });

  it('pairs equal-looking operands by identity, not content', () => {
    const first = { var: 'x' };
    const second = { var: 'x' };
    const rule = { '==': [first, second] };
    const sources = new TraceSources(rule, { '1': '/==/0', '2': '/==/1' });
    const matches = matchOperandsToChildren([second, first], [node(1), node(2)], sources);
    expect(matches.map((m) => m?.child.id)).toEqual([2, 1]);
  });

  it('leaves an operand unmatched without a pointer, and reports leftover children', () => {
    const a = { var: 'a' };
    const rule = { '!': a };
    const sources = new TraceSources(rule, undefined);
    const children = [node(4)];
    const matches = matchOperandsToChildren([a], children, sources);
    expect(matches).toEqual([null]);
    expect(unmatchedChildren(children, matches).map((c) => c.id)).toEqual([4]);
  });
});

describe('enclosingNode', () => {
  it('picks the listed node with the longest enclosing pointer', () => {
    const rule = { if: [{ missing: [{ var: 'k' }] }, 1, 2] };
    const sources = new TraceSources(rule, {
      '1': '/if/0/missing/0',
      '2': '/if/0',
      '3': '',
    });
    expect(enclosingNode(1, [3, 2], sources)).toBe(2);
    expect(enclosingNode(2, [3], sources)).toBe(3);
  });

  it('does not treat a sibling with a shared prefix as enclosing', () => {
    const rule = { a: [1], ab: [2] };
    const sources = new TraceSources(rule, { '1': '/ab/0', '2': '/a' });
    expect(enclosingNode(1, [2], sources)).toBeUndefined();
  });
});

describe('pointers from the engine', () => {
  it('place every tree node in the rule as written', () => {
    const rule: JsonLogicValue = {
      '?:': [{ '==': [{ val: ['a', 'b'] }, 1] }, { var: ['x', { '+': [1, 2] }] }, { match: [{ var: 'k' }, [['a', { var: 'y' }]], 0] }],
    };
    const trace = runTrace(rule, { a: { b: 1 } });
    expect(trace.pointers).toBeDefined();
    const sources = new TraceSources(rule, trace.pointers);
    const walk = (n: ExpressionNode) => {
      expect(sources.sourceOf(n.id), `node ${n.id} ${n.expression}`).toBeDefined();
      n.children.forEach(walk);
    };
    walk(trace.expression_tree);
    expect(sources.sourceOf(trace.expression_tree.id)).toBe(rule);
  });

  it('are absent when the rule does not compile', () => {
    expect(runTrace({ a: 1, b: 2 } as never).pointers).toBeUndefined();
  });
});
