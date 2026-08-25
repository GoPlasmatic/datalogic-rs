/**
 * Edge identity in the static converter.
 *
 * React Flow keys edges by `id`, and `createEdge` builds it as
 * `${source}-${target}` regardless of the handle. A parent that both links a
 * child as an argument AND as a branch therefore produces two edges with the
 * same id, one of which React Flow drops (silently, and non-deterministically
 * with respect to which handle survives). Structure (templating) nodes and
 * `var` nodes with a complex default are the two places that used to do this.
 */

import { describe, expect, it } from 'vitest';
import { jsonLogicToNodes } from '../jsonlogic-to-nodes';
import { buildEdgesFromNodes } from '../edge-builder';
import type { JsonLogicValue } from '../../types';

function duplicateIds(ids: string[]): string[] {
  const seen = new Set<string>();
  const dupes = new Set<string>();
  for (const id of ids) {
    if (seen.has(id)) dupes.add(id);
    seen.add(id);
  }
  return [...dupes];
}

const CASES: { name: string; rule: JsonLogicValue; templating?: boolean }[] = [
  {
    name: 'templating object with one expression',
    rule: { greeting: { cat: ['Hi ', { var: 'name' }] } } as unknown as JsonLogicValue,
    templating: true,
  },
  {
    name: 'templating object with several expressions and inline values',
    rule: {
      id: 42,
      name: { var: 'user.name' },
      total: { '+': [{ var: 'a' }, { var: 'b' }] },
      active: true,
    } as unknown as JsonLogicValue,
    templating: true,
  },
  {
    name: 'templating object with a nested object of expressions',
    rule: {
      header: { ref: { var: 'ref' } },
      body: { amount: { '*': [{ var: 'qty' }, 2] } },
    } as unknown as JsonLogicValue,
    templating: true,
  },
  {
    name: 'templating array of expressions',
    rule: [{ var: 'a' }, { var: 'b' }, 3] as unknown as JsonLogicValue,
    templating: true,
  },
  {
    name: 'var with a complex default',
    rule: { var: ['x', { '+': [1, 2] }] } as unknown as JsonLogicValue,
  },
  {
    name: 'nested operators',
    rule: {
      if: [{ '>': [{ var: 'score' }, 50] }, { cat: ['pass ', { var: 'name' }] }, 'fail'],
    } as unknown as JsonLogicValue,
  },
];

describe('converter edge ids are unique', () => {
  for (const { name, rule, templating } of CASES) {
    it(name, () => {
      const { nodes, edges } = jsonLogicToNodes(rule, { templating });
      expect(duplicateIds(edges.map((e) => e.id))).toEqual([]);

      // The runtime rebuilds edges from node cells/elements on every render;
      // that path must agree with the converter and stay duplicate-free too.
      const rebuilt = buildEdgesFromNodes(nodes);
      expect(duplicateIds(rebuilt.map((e) => e.id))).toEqual([]);

      // Every edge must connect two nodes that exist.
      const ids = new Set(nodes.map((n) => n.id));
      for (const edge of [...edges, ...rebuilt]) {
        expect(ids.has(edge.source), `source ${edge.source}`).toBe(true);
        expect(ids.has(edge.target), `target ${edge.target}`).toBe(true);
      }
    });
  }

  it('links every expression element of a structure node exactly once', () => {
    const rule = {
      a: { var: 'x' },
      b: { var: 'y' },
      c: 'literal',
    } as unknown as JsonLogicValue;
    const { nodes, edges } = jsonLogicToNodes(rule, { templating: true });
    const structure = nodes.find((n) => n.data.type === 'structure');
    expect(structure).toBeDefined();
    const fromStructure = edges.filter((e) => e.source === structure!.id);
    expect(fromStructure).toHaveLength(2);
    expect(new Set(fromStructure.map((e) => e.sourceHandle))).toEqual(
      new Set(['branch-0', 'branch-1'])
    );
  });
});
