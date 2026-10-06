import { describe, expect, it } from 'vitest';
import { carryNodeIds } from '../carry-selection';
import { convertJsonLogic, jsonLogicToNodes } from '../../../utils/jsonlogic-to-nodes';
import type { JsonLogicValue, LogicNode } from '../../../types';

function convert(rule: JsonLogicValue): LogicNode[] {
  return convertJsonLogic(rule, {}, 'n').nodes;
}

function byExpression(nodes: LogicNode[], expression: JsonLogicValue): LogicNode {
  const found = nodes.find((n) => JSON.stringify(n.data.expression) === JSON.stringify(expression));
  if (!found) throw new Error(`no node for ${JSON.stringify(expression)}`);
  return found;
}

describe('deterministic conversion ids', () => {
  it('gives the same rule the same ids every time', () => {
    const rule: JsonLogicValue = { if: [{ var: 'x' }, { '+': [1, { var: 'y' }] }, 'no'] };
    expect(convert(rule).map((n) => n.id)).toEqual(convert(rule).map((n) => n.id));
  });

  it('keeps ids unique within a conversion', () => {
    const rule: JsonLogicValue = {
      switch: [{ var: 'k' }, [['a', { '+': [1, 2] }], ['b', 'B']], { cat: ['x', { var: 'z' }] }],
    };
    const ids = convert(rule).map((n) => n.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it('keeps the public converter collision-free across calls', () => {
    const a = jsonLogicToNodes({ '+': [1, { '*': [2, 3] }] }).nodes.map((n) => n.id);
    const b = jsonLogicToNodes({ '+': [1, { '*': [2, 3] }] }).nodes.map((n) => n.id);
    expect(a.some((id) => b.includes(id))).toBe(false);
  });
});

describe('carryNodeIds', () => {
  it('maps a node the editor created onto its converted twin', () => {
    const before = convert({ if: [{ var: 'x' }, 'yes', 'no'] });
    const yes = byExpression(before, 'yes');
    // Simulate an editor-created node: same place and expression, random id.
    const edited = before.map((n) => (n.id === yes.id ? { ...n, id: 'uuid-1' } : n));
    const after = convert({ if: [{ var: 'x' }, 'yes', 'no'] });

    expect(carryNodeIds(['uuid-1'], edited, after).get('uuid-1')).toBe(yes.id);
  });

  it('drops a selection whose place now holds a different expression', () => {
    const before = convert({ '+': [{ '*': [1, 2] }, { '-': [3, 4] }, { '/': [5, 6] }] });
    const last = byExpression(before, { '/': [5, 6] });
    // The middle argument was deleted: the last one shifts into its slot.
    const after = convert({ '+': [{ '*': [1, 2] }, { '/': [5, 6] }] });
    const carried = carryNodeIds([last.id], before, after);
    expect(carried.has(last.id)).toBe(false);
  });

  it('keeps an unchanged node under its own id', () => {
    const before = convert({ and: [{ '>': [{ var: 'a' }, 1] }, { '<': [{ var: 'b' }, 2] }] });
    const after = convert({ and: [{ '>': [{ var: 'a' }, 1] }, { '<': [{ var: 'b' }, 3] }] });
    const first = byExpression(before, { '>': [{ var: 'a' }, 1] });
    expect(carryNodeIds([first.id], before, after).get(first.id)).toBe(first.id);
  });
});
