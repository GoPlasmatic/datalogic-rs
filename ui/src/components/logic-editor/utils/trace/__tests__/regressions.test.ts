import { describe, expect, it } from 'vitest';
import type { JsonLogicValue } from '../../../types';
import { analyze, findOperatorNode, runTrace, wasm } from './helpers';
import { traceToNodes } from '../trace-to-nodes';
import type { TracedResult } from '../../../types/trace';

describe('UI-TRACE-002: single-arg operators keep rewritten operands', () => {
  it('renders the child of {"!": {"val": ["a","b"]}} and maps its step', () => {
    const a = analyze({ '!': { val: ['a', 'b'] } }, { a: { b: true } });
    const valNode = findOperatorNode(a.result.nodes, 'val');
    expect(valNode).toBeDefined();
    expect(valNode!.id).toMatch(/^trace-\d+$/);
    expect(a.result.edges.length).toBe(1);
    expect(a.unmappedSteps).toEqual([]);
    expect(a.syntheticNodes).toEqual([]);
  });

  it('renders {"!!": {"?:": [...]}} as a not / if chain with mapped steps', () => {
    const a = analyze({ '!!': { '?:': [{ var: 'a' }, 1, 0] } }, { a: 1 });
    expect(findOperatorNode(a.result.nodes, 'if')).toBeDefined();
    expect(a.unmappedSteps).toEqual([]);
    expect(a.syntheticNodes).toEqual([]);
  });
});

describe('UI-TRACE-003: multi-arg operators match canonicalized children', () => {
  const cases: [string, unknown, unknown][] = [
    ['val path array', { and: [{ val: ['a', 'b'] }, { var: 'c' }] }, { a: { b: 1 }, c: 1 }],
    ['val plain', { and: [{ val: 'a' }, { val: 'b' }] }, { a: 1, b: 1 }],
    ['val inline pill', { cat: [{ val: 'a' }, 'x'] }, { a: 'y' }],
    ['numeric var path', { '+': [{ var: 1 }, 1] }, [5, 6]],
    ['match alias', { cat: [{ match: [{ var: 'x' }, [[1, 'one']], 'other'] }, '!'] }, { x: 1 }],
    ['?: alias', { and: [{ '?:': [{ var: 'a' }, 1, 0] }, true] }, { a: 1 }],
    ['?? with val', { '??': [{ val: 'a' }, { var: 'b' }] }, { b: 2 }],
    ['single-element val', { and: [{ val: ['a'] }, true] }, { a: 1 }],
  ];
  for (const [name, rule, data] of cases) {
    it(`maps every step and creates no synthetic node for ${name}`, () => {
      const a = analyze(rule as never, data);
      expect(a.unmappedSteps).toEqual([]);
      expect(a.syntheticNodes).toEqual([]);
    });
  }
});

describe('UI-TRACE-004 / 005: templating structures', () => {
  it('renders logic nested inside nested objects and arrays, mapping their steps', () => {
    const rule = { name: { var: 'n' }, nested: { k: { '+': [{ var: 'n' }, 2] } } };
    const a = analyze(rule, { n: 1 }, true);
    expect(findOperatorNode(a.result.nodes, '+')).toBeDefined();
    expect(findOperatorNode(a.result.nodes, 'var')).toBeDefined();
    expect(a.unmappedSteps).toEqual([]);
    expect(a.syntheticNodes).toEqual([]);
    expect(a.result.nodes.length).toBe(a.staticResult.nodes.length);
    // the nested structure's own step lands on the structure node
    const structure = a.result.nodes.find((n) => n.type === 'structure')!;
    const nestedStep = a.trace.steps.find((s) => JSON.stringify(s.result) === JSON.stringify({ k: 3 }))!;
    expect(a.result.traceNodeMap.get(`trace-${nestedStep.node_id}`)).toBe(structure.id);
  });

  it('maps a data-only nested object step onto the structure node', () => {
    const rule = { a: { b: 1 }, c: { var: 'x' } };
    const a = analyze(rule, { x: 1 }, true);
    expect(a.unmappedSteps).toEqual([]);
    const structure = a.result.nodes.find((n) => n.type === 'structure')!;
    const dataStep = a.trace.steps.find((s) => JSON.stringify(s.result) === JSON.stringify({ b: 1 }))!;
    expect(a.result.traceNodeMap.get(`trace-${dataStep.node_id}`)).toBe(structure.id);
  });

  it('renders logic inside an array of objects', () => {
    const a = analyze({ items: [{ v: { var: 'a' } }] }, { a: 2 }, true);
    expect(findOperatorNode(a.result.nodes, 'var')).toBeDefined();
    expect(a.result.edges.length).toBeGreaterThan(0);
    expect(a.unmappedSteps).toEqual([]);
  });

  it('creates a single branch edge per structure child (no duplicate edge ids)', () => {
    const a = analyze({ name: { var: 'n' } }, { n: 1 }, true);
    expect(a.duplicateEdgeIds).toEqual([]);
    expect(a.result.edges.length).toBe(1);
    expect(a.result.edges[0].sourceHandle).toBe('branch-0');

    const b = analyze([{ var: 'a' }, 1], { a: 1 }, true);
    expect(b.duplicateEdgeIds).toEqual([]);
  });
});

describe('UI-TRACE-V01: switch cases nested two levels deep', () => {
  it('maps case and result steps to their own nodes, not the array wrapper', () => {
    const rule: JsonLogicValue = { switch: [{ var: 'x' }, [[{ var: 'y' }, { '+': [1, { var: 'x' }] }]], 0] };
    const a = analyze(rule, { x: 1, y: 1 });
    expect(a.unmappedSteps).toEqual([]);
    expect(a.syntheticNodes).toEqual([]);

    const plus = findOperatorNode(a.result.nodes, '+')!;
    const plusStep = a.trace.steps.find((s) => s.result === 2)!;
    expect(a.result.traceNodeMap.get(`trace-${plusStep.node_id}`)).toBe(plus.id);

    // the wrapper arrays fold onto the switch node
    const switchNode = findOperatorNode(a.result.nodes, 'switch')!;
    const wrapper = a.trace.expression_tree.children.find((c) => c.expression.startsWith('['))!;
    expect(a.result.traceNodeMap.get(`trace-${wrapper.id}`)).toBe(switchNode.id);
    for (const pair of wrapper.children) {
      expect(a.result.traceNodeMap.get(`trace-${pair.id}`)).toBe(switchNode.id);
    }
  });
});

describe('hidden trace ids (leaf operators with dynamic arguments)', () => {
  it('maps steps of the hidden merge inside missing onto the missing node', () => {
    const rule = { missing: { merge: [['a'], ['b']] } };
    const a = analyze(rule, { a: 1 });
    const missingNode = findOperatorNode(a.result.nodes, 'missing')!;
    expect(a.unmappedSteps).toEqual([]);
    for (const step of a.trace.steps) {
      expect(a.result.traceNodeMap.get(`trace-${step.node_id}`)).toBe(missingNode.id);
    }
  });

  it('maps the hidden minimum expression of missing_some', () => {
    const a = analyze({ missing_some: [{ var: 'b' }, ['a', 'zz']] }, { a: 1, b: 1 });
    expect(a.unmappedSteps).toEqual([]);
  });
});

describe('constant sub-expressions and computed paths', () => {
  it('renders a constant root operator from the original value', () => {
    const rule = { '+': [1, 2] };
    const trace = runTrace(rule, {});
    const result = traceToNodes(trace, { originalValue: rule });
    expect(findOperatorNode(result.nodes, '+')).toBeDefined();
    expect(result.rootId).toBe(`trace-${trace.expression_tree.id}`);
    expect(trace.steps.every((s) => result.traceNodeMap.get(`trace-${s.node_id}`) === result.rootId)).toBe(true);
  });

  it('renders a constant then-branch without stealing the condition child', () => {
    const rule: JsonLogicValue = { if: [{ var: 'a' }, { '+': [1, 2] }, 0] };
    const a = analyze(rule, { a: 1 });
    expect(a.unmappedSteps).toEqual([]);
    expect(a.syntheticNodes).toEqual([]);
    const varNode = findOperatorNode(a.result.nodes, 'var')!;
    expect(varNode.id).toMatch(/^trace-\d+$/);
    expect(findOperatorNode(a.result.nodes, '+')).toBeDefined();
    const varStep = a.trace.steps.find((s) => s.result === 1)!;
    expect(a.result.traceNodeMap.get(`trace-${varStep.node_id}`)).toBe(varNode.id);
  });

  it('matches a computed var path (serialized by the engine as val)', () => {
    const a = analyze({ var: { cat: ['a', 'b'] } }, { ab: 3 });
    expect(a.unmappedSteps).toEqual([]);
    expect(a.syntheticNodes).toEqual([]);
    expect(a.result.nodes.length).toBe(a.staticResult.nodes.length);
  });
});

describe('custom operators (Engine.evaluateWithTrace)', () => {
  it('maps steps for a custom operator node', () => {
    const engine = new wasm.Engine({
      customOperators: { dbl: (args: string) => JSON.stringify((JSON.parse(args) as number[])[0] * 2) },
    });
    const rule = { dbl: [{ var: 'x' }] };
    const trace = JSON.parse(engine.evaluateWithTrace(JSON.stringify(rule), JSON.stringify({ x: 2 }))) as TracedResult;
    const result = traceToNodes(trace, { originalValue: rule });
    const ids = new Set(result.nodes.map((n) => n.id));
    for (const step of trace.steps) {
      const visual = result.traceNodeMap.get(`trace-${step.node_id}`);
      expect(visual && ids.has(visual)).toBe(true);
    }
    expect(trace.result).toBe(4);
  });
});
