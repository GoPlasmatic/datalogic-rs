import { describe, expect, it } from 'vitest';
import { runTrace } from './helpers';
import { traceToNodes } from '../trace-to-nodes';
import {
  isCompileFailedTrace,
  getTraceFailure,
  formatTraceFailure,
  traceFailureType,
  resolveFailedNodeIds,
} from '../trace-failure';

describe('trace envelope shape (real engine)', () => {
  it('emits step_id / node_id / context / result / error on every step', () => {
    const trace = runTrace({ '>': [{ var: 'age' }, 18] }, { age: 20 });
    expect(trace.steps.length).toBeGreaterThan(0);
    for (const step of trace.steps) {
      expect(Object.keys(step).sort()).toEqual(['context', 'error', 'node_id', 'result', 'step_id']);
      expect(typeof step.step_id).toBe('number');
      expect(typeof step.node_id).toBe('number');
      expect(step.error).toBeNull();
      expect('id' in step).toBe(false);
    }
    expect(trace.steps.map((s) => s.step_id)).toEqual(trace.steps.map((_, i) => i));
    expect(trace.result).toBe(true);
    expect(trace.error).toBeUndefined();
    expect(trace.structured_error).toBeUndefined();
  });

  it('records iteration_index / iteration_total inside iterators', () => {
    const trace = runTrace({ map: [{ var: 'xs' }, { '*': [{ var: '' }, 2] }] }, { xs: [1, 2, 3] });
    const iterated = trace.steps.filter((s) => s.iteration_index !== undefined);
    expect(iterated.length).toBeGreaterThan(0);
    expect(iterated.every((s) => s.iteration_total === 3)).toBe(true);
    expect(new Set(iterated.map((s) => s.iteration_index))).toEqual(new Set([0, 1, 2]));
  });

  it('records a null result and the message on an error step', () => {
    const trace = runTrace({ try: [{ throw: 'x' }, { var: 'type' }] }, {});
    const errorStep = trace.steps.find((s) => s.error !== null);
    expect(errorStep).toBeDefined();
    expect(errorStep?.result).toBeNull();
    expect(errorStep?.error).toContain('Thrown');
  });

  it('reports a compile failure with a placeholder tree and no steps', () => {
    const trace = runTrace({ a: 1, b: 2 }, {}, false);
    expect(trace.expression_tree).toEqual({ id: 0, expression: '', children: [] });
    expect(trace.steps).toEqual([]);
    expect(trace.result).toBeNull();
    expect(trace.structured_error?.type).toBe('InvalidOperator');
    expect(isCompileFailedTrace(trace)).toBe(true);
    expect(getTraceFailure(trace)).toBe(trace.structured_error);
    expect(traceFailureType(trace.structured_error!)).toBe('InvalidOperator');
    expect(formatTraceFailure(trace.structured_error!)).toContain('Unknown Operator');
  });

  it('traceToNodes does not throw on a compile-failed trace (with or without originalValue)', () => {
    const trace = runTrace({ a: 1, b: 2 }, {}, false);
    expect(() => traceToNodes(trace, {})).not.toThrow();
    expect(traceToNodes(trace, {})).toEqual({ nodes: [], edges: [], rootId: null, traceNodeMap: new Map() });
    expect(traceToNodes(trace, { originalValue: { a: 1, b: 2 } }).rootId).toBeNull();
    expect(resolveFailedNodeIds(trace, new Map()).size).toBe(0);
  });

  it('reports a runtime failure with steps up to the failing node and a node_ids breadcrumb', () => {
    const rule = { date_diff: [{ var: 'a' }, { var: 'b' }, 'weeks'] };
    const data = { a: '2024-01-01T00:00:00Z', b: '2024-01-02T00:00:00Z' };
    const trace = runTrace(rule, data);
    expect(isCompileFailedTrace(trace)).toBe(false);
    expect(trace.structured_error?.type).toBe('InvalidArguments');
    expect(trace.structured_error?.node_ids).toEqual([trace.expression_tree.id]);
    expect(trace.steps.at(-1)?.error).toContain('weeks');

    const { traceNodeMap, nodes } = traceToNodes(trace, { originalValue: rule });
    const failed = resolveFailedNodeIds(trace, traceNodeMap);
    expect([...failed]).toEqual([`trace-${trace.expression_tree.id}`]);
    expect(nodes.some((n) => failed.has(n.id))).toBe(true);
  });

  it('maps a nested breadcrumb (innermost first) to the throw and the and node', () => {
    const rule = { and: [{ throw: 'x' }, 1] };
    const trace = runTrace(rule, {});
    const { traceNodeMap, nodes } = traceToNodes(trace, { originalValue: rule });
    const failed = [...resolveFailedNodeIds(trace, traceNodeMap)];
    expect(failed.length).toBe(2);
    const ops = failed.map((id) => (nodes.find((n) => n.id === id)?.data as { operator?: string }).operator);
    expect(ops).toEqual(['throw', 'and']);
  });

  it('maps a var-only rule against null data', () => {
    const trace = runTrace({ var: 'a' }, null);
    expect(trace.steps[0].context).toBeNull();
    expect(trace.steps[0].result).toBeNull();
    expect(getTraceFailure(trace)).toBeUndefined();
  });
});
