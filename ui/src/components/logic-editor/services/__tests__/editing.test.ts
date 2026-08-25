/**
 * Editing operations: every mutation the editor performs on the node graph
 * must keep the graph consistent (parents reference their children, children
 * point back at their parents, argIndex follows the cells) and serialize to
 * the JSONLogic the user expects.
 */

import { describe, expect, it } from 'vitest';
import * as wasm from '@goplasmatic/datalogic-wasm';
import { jsonLogicToNodes } from '../../utils/jsonlogic-to-nodes';
import { nodesToJsonLogic } from '../../utils/nodes-to-jsonlogic';
import { deleteNodeAndDescendants, updateParentAfterChildDeletion } from '../../utils/node-deletion';
import { cloneNodesWithIdMapping, getDescendants, updateParentChildReference } from '../../utils/node-cloning';
import { panelValuesToNodeData, havePanelValuesChanged } from '../../utils/node-updaters';
import { buildEdgesFromNodes } from '../../utils/edge-builder';
import { applyTreeLayout } from '../../utils/layout';
import { operatorRenderKind } from '../../utils/nodeShape';
import { getInitialValuesFromNode } from '../../properties-panel/utils';
import { extractArguments } from '../../properties-panel/utils/argument-parser';
import { rebuildVariableExpression } from '../../properties-panel/utils/expression-rebuilder';
import { addArgument, removeArgument, canEditArguments } from '../argument-service';
import { wrapInOperator, duplicateNodeTree } from '../node-transform-service';
import { updateInlineOperand } from '../inline-edit-service';
import type { JsonLogicValue, LogicNode, OperatorNodeData, StructureNodeData } from '../../types';

function build(rule: JsonLogicValue, templating = false): LogicNode[] {
  return jsonLogicToNodes(rule, { templating }).nodes;
}

function root(nodes: LogicNode[]): LogicNode {
  return nodes.find((n) => !n.data.parentId)!;
}

function op(nodes: LogicNode[], operator: string, nth = 0): LogicNode {
  const found = nodes.filter((n) => n.data.type === 'operator' && n.data.operator === operator);
  if (!found[nth]) throw new Error(`no ${operator} node #${nth}`);
  return found[nth];
}

function opData(node: LogicNode): OperatorNodeData {
  return node.data as OperatorNodeData;
}

function childOfCell(nodes: LogicNode[], parent: LogicNode, cellIndex: number): LogicNode {
  const cell = opData(parent).cells.find((c) => c.index === cellIndex)!;
  return nodes.find((n) => n.id === cell.branchId)!;
}

function evaluate(rule: JsonLogicValue | null, data: unknown = {}, templating = false): unknown {
  return JSON.parse(wasm.evaluate(JSON.stringify(rule), JSON.stringify(data), templating));
}

/** Structural invariants: every referenced child exists and points back; no orphans. */
function checkInvariants(nodes: LogicNode[]): void {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const referenced = new Set<string>();
  for (const node of nodes) {
    const ids: string[] = [];
    if (node.data.type === 'operator') {
      for (const cell of (node.data as OperatorNodeData).cells) {
        if (cell.branchId) ids.push(cell.branchId);
      }
    } else if (node.data.type === 'structure') {
      for (const el of (node.data as StructureNodeData).elements) {
        if (el.branchId) ids.push(el.branchId);
      }
    }
    for (const id of ids) {
      const child = byId.get(id);
      expect(child, `missing child ${id} of ${node.id}`).toBeDefined();
      expect(child!.data.parentId, `child ${id} parentId`).toBe(node.id);
      referenced.add(id);
    }
  }
  const roots = nodes.filter((n) => !n.data.parentId);
  expect(roots, 'exactly one root').toHaveLength(1);
  for (const node of nodes) {
    if (node.data.parentId) {
      expect(referenced.has(node.id), `orphan ${node.id}`).toBe(true);
    }
  }
}

describe('deleteNodeAndDescendants', () => {
  it('drops the right operand of an n-ary operator and reindexes the rest', () => {
    const nodes = build({ '+': [{ '*': [2, 3] }, 1, 2] });
    const out = deleteNodeAndDescendants(op(nodes, '*').id, nodes);
    expect(nodesToJsonLogic(out)).toEqual({ '+': [1, 2] });
    expect(evaluate(nodesToJsonLogic(out))).toBe(3);
    checkInvariants(out);
  });

  it('keeps later inline and wired operands aligned after a deletion', () => {
    const nodes = build({ cat: ['a', { upper: ['b'] }, 'c', { lower: ['d'] }] });
    const out = deleteNodeAndDescendants(op(nodes, 'upper').id, nodes);
    expect(nodesToJsonLogic(out)).toEqual({ cat: ['a', 'c', { lower: ['d'] }] });
    checkInvariants(out);
  });

  it('renumbers argIndex on surviving children so a later removeArgument hits the right sibling', () => {
    const nodes = build({ '+': [{ '*': [1, 1] }, { '*': [2, 2] }, { '*': [3, 3] }] });
    const out = deleteNodeAndDescendants(op(nodes, '*', 0).id, nodes);
    const plus = op(out, '+');
    const children = out.filter((n) => n.data.parentId === plus.id).sort((a, b) => a.data.argIndex! - b.data.argIndex!);
    expect(children.map((c) => c.data.argIndex)).toEqual([0, 1]);
    const second = children[0];
    const removed = removeArgument(out, plus.id, second.data.argIndex!)!;
    expect(nodesToJsonLogic(removed)).toEqual({ '+': [{ '*': [3, 3] }] });
  });

  it('leaves a null placeholder in a diamond then-slot and drops the else input', () => {
    const rule = { if: [{ var: 'a' }, { var: 'b' }, { var: 'c' }] };
    let nodes = build(rule);
    const diamond = root(nodes);
    nodes = deleteNodeAndDescendants(childOfCell(nodes, diamond, 1).id, nodes);
    expect(nodesToJsonLogic(nodes)).toEqual({ if: [{ var: 'a' }, null, { var: 'c' }] });
    expect(operatorRenderKind(opData(root(nodes)))).toBe('decision');
    nodes = deleteNodeAndDescendants(childOfCell(nodes, root(nodes), 2).id, nodes);
    expect(nodesToJsonLogic(nodes)).toEqual({ if: [{ var: 'a' }, null] });
    checkInvariants(nodes);
  });

  it('removes a whole switch case when its result node is deleted', () => {
    const rule = { switch: [{ var: 'x' }, [[1, { var: 'one' }], [2, 'two']], 'other'] };
    const nodes = build(rule);
    const out = deleteNodeAndDescendants(op(nodes, 'var', 1).id, nodes);
    expect(nodesToJsonLogic(out)).toEqual({ switch: [{ var: 'x' }, [[2, 'two']], 'other'] });
    checkInvariants(out);
  });

  it('removes a structure element and regenerates the formatted JSON', () => {
    const nodes = build({ a: { var: 'x' }, b: { var: 'y' } }, true);
    const out = deleteNodeAndDescendants(op(nodes, 'var', 1).id, nodes);
    expect(nodesToJsonLogic(out)).toEqual({ a: { var: 'x' } });
    const struct = root(out).data as StructureNodeData;
    expect(struct.formattedJson.match(/\{\{EXPR\}\}/g)).toHaveLength(1);
    expect(struct.elements).toHaveLength(1);
    expect(struct.formattedJson.slice(struct.elements[0].startOffset, struct.elements[0].endOffset)).toBe('"{{EXPR}}"');
    checkInvariants(out);
  });

  it('keeps element offsets exact when the template holds a literal placeholder', () => {
    const rule = { a: '{{EXPR}}', b: { var: 'x' } };
    const data = root(build(rule, true)).data as StructureNodeData;
    const expr = data.elements.find((e) => e.type === 'expression')!;
    expect(data.formattedJson.slice(expr.startOffset, expr.endOffset)).toBe('"{{EXPR}}"');
    // The span must be the placeholder under "b", not the user's own literal at "a"
    expect(data.formattedJson.lastIndexOf('"{{EXPR}}"')).toBe(expr.startOffset);
  });

  it('shifts array element paths after removing an earlier array entry', () => {
    const nodes = build([{ var: 'x' }, { var: 'y' }, { var: 'z' }], true);
    const out = deleteNodeAndDescendants(op(nodes, 'var', 1).id, nodes);
    expect(nodesToJsonLogic(out)).toEqual([{ var: 'x' }, { var: 'z' }]);
    const struct = root(out).data as StructureNodeData;
    expect(struct.elements.map((e) => e.path)).toEqual([['0'], ['1']]);
  });

  it('removes two structure elements in one pass, shifting the survivor', () => {
    const nodes = build([{ var: 'x' }, { var: 'y' }, { var: 'z' }], true);
    const struct = root(nodes);
    const branchIds = (struct.data as StructureNodeData).elements.map((e) => e.branchId!);
    const deleted = new Set([branchIds[0], branchIds[2]]);
    const remaining = nodes.filter((n) => !deleted.has(n.id));

    const { node } = updateParentAfterChildDeletion(struct, deleted, remaining);
    const data = node.data as StructureNodeData;
    expect(data.expression).toEqual([{ var: 'y' }]);
    expect(data.elements.map((e) => e.path)).toEqual([['0']]);
    expect(
      data.formattedJson.slice(data.elements[0].startOffset, data.elements[0].endOffset)
    ).toBe('"{{EXPR}}"');
  });

  it('drops the default of a var when the wired default is deleted', () => {
    const nodes = build({ var: ['x', { var: 'y' }] });
    const out = deleteNodeAndDescendants(childOfCell(nodes, root(nodes), 1).id, nodes);
    expect(nodesToJsonLogic(out)).toEqual({ var: 'x' });
  });
});

describe('addArgument / removeArgument', () => {
  it('adds an else-if diamond to an if and keeps the else value last', () => {
    const nodes = build({ if: [{ var: 'a' }, 1, 2] });
    const result = addArgument(nodes, root(nodes).id, 'literal')!;
    expect(nodesToJsonLogic(result.nodes)).toEqual({ if: [{ var: 'a' }, 1, true, 0, 2] });
    checkInvariants(result.nodes);
    expect(evaluate(nodesToJsonLogic(result.nodes), { a: false })).toBe(0);
  });

  it('adds an else-if to an if without an else', () => {
    const nodes = build({ if: [{ var: 'a' }, 1] });
    const result = addArgument(nodes, root(nodes).id, 'literal')!;
    expect(nodesToJsonLogic(result.nodes)).toEqual({ if: [{ var: 'a' }, 1, true, 0] });
    checkInvariants(result.nodes);
  });

  it('removes the head condition of a chain by promoting the next diamond', () => {
    const nodes = build({ if: [{ var: 'a' }, 1, { var: 'b' }, 2, 3] });
    const out = removeArgument(nodes, root(nodes).id, 0)!;
    expect(nodesToJsonLogic(out)).toEqual({ if: [{ var: 'b' }, 2, 3] });
    expect(root(out).data.label).toBe('if');
    checkInvariants(out);
  });

  it('removes an else-if diamond from the middle of a chain', () => {
    const nodes = build({ if: [{ var: 'a' }, 1, { var: 'b' }, 2, { var: 'c' }, 3, 4] });
    const elif = op(nodes, 'if', 1);
    expect(elif.data.label).toBe('elif');
    const out = removeArgument(nodes, elif.id, 0)!;
    expect(nodesToJsonLogic(out)).toEqual({ if: [{ var: 'a' }, 1, { var: 'c' }, 3, 4] });
    checkInvariants(out);
  });

  it('refuses to remove the only condition and removes the else input', () => {
    const nodes = build({ if: [{ var: 'a' }, 1, 2] });
    expect(removeArgument(nodes, root(nodes).id, 0)).toBeNull();
    expect(removeArgument(nodes, root(nodes).id, 1)).toBeNull();
    const out = removeArgument(nodes, root(nodes).id, 2)!;
    expect(nodesToJsonLogic(out)).toEqual({ if: [{ var: 'a' }, 1] });
    checkInvariants(out);
  });

  it('adds and removes switch cases as Case/Then pairs', () => {
    const rule = { switch: [{ var: 'x' }, [[1, 'one']], 'other'] };
    const nodes = build(rule);
    const sw = root(nodes);

    const removedCase = removeArgument(nodes, sw.id, 1)!;
    expect(nodesToJsonLogic(removedCase)).toEqual({ switch: [{ var: 'x' }, [], 'other'] });
    expect(evaluate(nodesToJsonLogic(removedCase), { x: 2 })).toBe('other');

    const removedThen = removeArgument(nodes, sw.id, 2)!;
    expect(nodesToJsonLogic(removedThen)).toEqual({ switch: [{ var: 'x' }, [], 'other'] });

    const removedDefault = removeArgument(nodes, sw.id, 3)!;
    expect(nodesToJsonLogic(removedDefault)).toEqual({ switch: [{ var: 'x' }, [[1, 'one']]] });

    expect(removeArgument(nodes, sw.id, 0)).toBeNull();

    const added = addArgument(nodes, sw.id, 'literal')!;
    expect(nodesToJsonLogic(added.nodes)).toEqual({ switch: [{ var: 'x' }, [[1, 'one'], [0, 0]], 'other'] });
    checkInvariants(added.nodes);
    const cells = opData(root(added.nodes)).cells;
    expect(cells.map((c) => c.rowLabel)).toEqual(['Match', 'Case', 'Then', 'Case', 'Then', 'Default']);
    expect(cells.map((c) => c.index)).toEqual([0, 1, 2, 3, 4, 5]);
  });

  it('does not offer arguments on exists', () => {
    const nodes = build({ exists: 'user.name' });
    expect(canEditArguments('exists')).toBe(false);
    expect(addArgument(nodes, root(nodes).id, 'literal')).toBeNull();
    expect(nodesToJsonLogic(nodes)).toEqual({ exists: 'user.name' });
  });

  it('adds and removes val path cells', () => {
    const nodes = build({ val: [[-1], 'a', 'b'] });
    const added = addArgument(nodes, root(nodes).id, 'literal')!;
    expect(opData(root(added.nodes)).cells).toHaveLength(3);
    expect(nodesToJsonLogic(added.nodes)).toEqual({ val: [[-1], 'a', 'b'] });
    const removed = removeArgument(added.nodes, root(added.nodes).id, 1)!;
    expect(nodesToJsonLogic(removed)).toEqual({ val: [[-1]] });
    expect(root(removed).data.expression).toEqual({ val: [[-1]] });
  });

  it('removes the var default through removeArgument', () => {
    const nodes = build({ var: ['x', 0] });
    const out = removeArgument(nodes, root(nodes).id, 1)!;
    expect(nodesToJsonLogic(out)).toEqual({ var: 'x' });
    expect(removeArgument(nodes, root(nodes).id, 0)).toBeNull();
  });

  it('appends a literal to an n-ary operator written in shorthand form', () => {
    const nodes = build({ cat: 'a' });
    const result = addArgument(nodes, root(nodes).id, 'literal')!;
    expect(nodesToJsonLogic(result.nodes)).toEqual({ cat: ['a', 'text'] });
  });
});

describe('inline edits (properties panel)', () => {
  it('maps switch Case / Then / Default rows onto the nested cases array', () => {
    const nodes = build({ switch: [{ var: 'x' }, [[1, 'one']], 'other'] });
    const data = opData(root(nodes));

    const caseEdit = updateInlineOperand(data, 1, 2)!;
    expect(caseEdit.expression).toEqual({ switch: [{ var: 'x' }, [[2, 'one']], 'other'] });
    expect(caseEdit.cells![1].label).toBe('2');

    const thenEdit = updateInlineOperand(data, 2, 'uno')!;
    expect(thenEdit.expression).toEqual({ switch: [{ var: 'x' }, [[1, 'uno']], 'other'] });

    const defaultEdit = updateInlineOperand(data, 3, 'x')!;
    expect(defaultEdit.expression).toEqual({ switch: [{ var: 'x' }, [[1, 'one']], 'x'] });

    const edited = nodes.map((n) => (n.id === root(nodes).id ? { ...n, data: { ...data, ...defaultEdit } } : n));
    expect(nodesToJsonLogic(edited)).toEqual({ switch: [{ var: 'x' }, [[1, 'one']], 'x'] });
  });

  it('keeps the var default when the path is edited, and edits the inline default', () => {
    const nodes = build({ var: ['x', 0] });
    const data = opData(root(nodes));
    const pathEdit = updateInlineOperand(data, 0, 'y')!;
    expect(pathEdit.expression).toEqual({ var: ['y', 0] });
    expect(pathEdit.cells![0].label).toBe('y');
    const defaultEdit = updateInlineOperand(data, 1, 5)!;
    expect(defaultEdit.expression).toEqual({ var: ['x', 5] });
    expect(rebuildVariableExpression('var', data.cells, data.expression)).toEqual({ var: ['x', 0] });
  });

  it('turns a typed dotted val / exists path into path components', () => {
    const val = opData(root(build({ val: [[-1], 'a'] })));
    const valEdit = updateInlineOperand(val, 1, 'a.b')!;
    expect(valEdit.expression).toEqual({ val: [[-1], 'a', 'b'] });
    expect(valEdit.cells![1].value).toEqual(['a', 'b']);

    const exists = opData(root(build({ exists: 'user' })));
    const existsEdit = updateInlineOperand(exists, 0, 'user.name')!;
    expect(existsEdit.expression).toEqual({ exists: ['user', 'name'] });
    expect(evaluate(existsEdit.expression as JsonLogicValue, { user: { name: 1 } })).toBe(true);
  });

  it('presents inline var pills and arrays as read-only, plain literals as editable', () => {
    const nodes = build({ '==': [1, { var: 'user.name' }] });
    const args = extractArguments(opData(root(nodes)), new Map());
    expect(args[0]).toMatchObject({ index: 0, isInline: true, value: 1, valueType: 'number' });
    expect(args[1]).toMatchObject({ index: 1, isInline: true, readOnly: true, displayLabel: 'user.name' });

    const sw = build({ switch: [{ var: 'x' }, [[null, 'none'], [1, 'one']], 'other'] });
    const swArgs = extractArguments(opData(root(sw)), new Map());
    expect(swArgs.map((a) => [a.rowLabel, a.value])).toEqual([
      ['Match', undefined],
      ['Case', null],
      ['Then', 'none'],
      ['Case', 1],
      ['Then', 'one'],
      ['Default', 'other'],
    ]);
  });
});

describe('panel values (node-updaters)', () => {
  it('does not register an edit when the seeded panel values are applied unchanged', () => {
    const rules: JsonLogicValue[] = [
      { var: ['x', 0] },
      { var: ['x', { var: 'y' }] },
      { var: 'x' },
      { val: [[-1], 'a', 'b'] },
      { val: 'index' },
      { exists: ['user', 'name'] },
      { exists: 'a.b' },
    ];
    for (const rule of rules) {
      const node = root(build(rule));
      const seeded = getInitialValuesFromNode(node.data);
      expect(havePanelValuesChanged(node.data, seeded), JSON.stringify(rule)).toBe(false);
    }
  });

  it('keeps the var default when only the path changes', () => {
    const node = root(build({ var: ['x', 0] }));
    const values = { ...getInitialValuesFromNode(node.data), path: 'y' };
    const updated = panelValuesToNodeData(node.data, values);
    expect(updated.expression).toEqual({ var: ['y', 0] });
    const nodes = [{ ...node, data: updated }];
    expect(nodesToJsonLogic(nodes)).toEqual({ var: ['y', 0] });
  });

  it('emits the scoped metadata form for val metadata access', () => {
    const node = root(build({ val: ['a'] }));
    const updated = panelValuesToNodeData(node.data, { accessType: 'metadata', metadataKey: 'index' });
    expect(updated.expression).toEqual({ val: [[1], 'index'] });
    expect(nodesToJsonLogic([{ ...node, data: updated }])).toEqual({ val: [[1], 'index'] });
    expect(getInitialValuesFromNode(updated)).toEqual({ accessType: 'metadata', metadataKey: 'index' });
  });

  it('emits the array form for a dotted exists path typed in the panel', () => {
    const node = root(build({ exists: 'user' }));
    const updated = panelValuesToNodeData(node.data, { pathType: 'dot', dotPath: 'user.name' });
    expect(updated.expression).toEqual({ exists: ['user', 'name'] });
    expect(nodesToJsonLogic([{ ...node, data: updated }])).toEqual({ exists: ['user', 'name'] });
  });
});

describe('wrap / duplicate / paste', () => {
  const tpl: JsonLogicValue = { map: [{ var: 'items' }, { id: { var: 'id' }, n: { '+': [1, { var: 'k' }] } }] };

  it('wraps a child of a structure node', () => {
    const nodes = build(tpl, true);
    const out = wrapInOperator(nodes, op(nodes, '+').id, 'abs')!;
    expect(nodesToJsonLogic(out)).toEqual({ map: [{ var: 'items' }, { id: { var: 'id' }, n: { abs: [{ '+': [1, { var: 'k' }] }] } }] });
    checkInvariants(out);
  });

  it('wraps in a real decision diamond', () => {
    const nodes = build({ and: [{ '>': [{ var: 'a' }, 1] }, true] });
    const out = wrapInOperator(nodes, op(nodes, '>').id, 'if')!;
    expect(nodesToJsonLogic(out)).toEqual({ and: [{ if: [{ '>': [{ var: 'a' }, 1] }, 'yes', 'no'] }, true] });
    const diamond = op(out, 'if');
    expect(operatorRenderKind(opData(diamond))).toBe('decision');
    expect(opData(diamond).cells.map((c) => c.rowLabel)).toEqual(['when', 'then', 'else']);
    checkInvariants(out);
  });

  it('refuses to duplicate into fixed slots and keeps the graph unchanged', () => {
    const nodes = build(tpl, true);
    expect(duplicateNodeTree(nodes, op(nodes, '+').id)).toBeNull();

    const ifNodes = build({ if: [{ var: 'a' }, { var: 'b' }, { var: 'c' }] });
    expect(duplicateNodeTree(ifNodes, childOfCell(ifNodes, root(ifNodes), 1).id)).toBeNull();

    const varNodes = build({ var: ['x', { var: 'y' }] });
    expect(duplicateNodeTree(varNodes, childOfCell(varNodes, root(varNodes), 1).id)).toBeNull();
  });

  it('duplicates an n-ary operand, a switch case pair and an array template element', () => {
    const plus = build({ '+': [{ '*': [2, 3] }, 1] });
    const dupPlus = duplicateNodeTree(plus, op(plus, '*').id)!;
    expect(nodesToJsonLogic(dupPlus.nodes)).toEqual({ '+': [{ '*': [2, 3] }, 1, { '*': [2, 3] }] });
    checkInvariants(dupPlus.nodes);

    const sw = build({ switch: [{ var: 'x' }, [[1, { var: 'one' }]], 'other'] });
    const dupSw = duplicateNodeTree(sw, op(sw, 'var', 1).id)!;
    expect(nodesToJsonLogic(dupSw.nodes)).toEqual({ switch: [{ var: 'x' }, [[1, { var: 'one' }], [1, { var: 'one' }]], 'other'] });
    checkInvariants(dupSw.nodes);

    const arr = build([{ var: 'a' }, 2], true);
    const dupArr = duplicateNodeTree(arr, op(arr, 'var').id)!;
    expect(nodesToJsonLogic(dupArr.nodes)).toEqual([{ var: 'a' }, 2, { var: 'a' }]);
    checkInvariants(dupArr.nodes);
  });

  it('remaps structure element references when cloning', () => {
    const nodes = build(tpl, true);
    const struct = nodes.find((n) => n.data.type === 'structure')!;
    const subtree = [struct, ...getDescendants(struct.id, nodes)];
    const { nodes: cloned, idMap } = cloneNodesWithIdMapping(subtree, struct.id);
    const clonedStruct = cloned.find((n) => n.id === idMap.get(struct.id))!;
    for (const el of (clonedStruct.data as StructureNodeData).elements) {
      expect(cloned.some((n) => n.id === el.branchId)).toBe(true);
    }
  });

  it('pastes over a child of a structure node', () => {
    const nodes = build(tpl, true);
    const target = op(nodes, '+');
    // paste replaces the target subtree, mirroring useClipboardState.pasteNode
    const source = build({ var: 'z' });
    const { nodes: pasted, newRootId } = cloneNodesWithIdMapping(source, root(source).id);
    const pastedRoot = pasted.find((n) => n.id === newRootId)!;
    pastedRoot.data = { ...pastedRoot.data, parentId: target.data.parentId, argIndex: target.data.argIndex };
    const targetIds = new Set([target.id, ...getDescendants(target.id, nodes).map((d) => d.id)]);
    let out = nodes.filter((n) => !targetIds.has(n.id));
    out = updateParentChildReference(out, target.data.parentId!, target.id, pastedRoot.id);
    out = [...out, ...pasted];
    expect(nodesToJsonLogic(out)).toEqual({ map: [{ var: 'items' }, { id: { var: 'id' }, n: { var: 'z' } }] });
    checkInvariants(out);
  });
});

describe('edges and layout', () => {
  it('gives a var with a wired default unique edge ids on the cell handle', () => {
    const { edges, nodes } = jsonLogicToNodes({ var: ['x', { var: 'y' }] });
    const ids = edges.map((e) => e.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(edges.map((e) => e.sourceHandle)).toEqual(['branch-1']);
    expect(buildEdgesFromNodes(nodes).map((e) => e.targetHandle)).toEqual(['branch-1']);
  });

  it('lays out templating children as connected nodes when no edges are passed', () => {
    const nodes = build({ a: { var: 'x' } }, true);
    const laid = applyTreeLayout(nodes);
    const structX = laid.find((n) => n.data.type === 'structure')!.position.x;
    const childX = laid.find((n) => n.data.type === 'operator')!.position.x;
    expect(childX).not.toBe(structX);
  });

  it('renders a generic if (single operand or shorthand) as a card, not a portless diamond', () => {
    expect(operatorRenderKind(opData(root(build({ if: [{ var: 'x' }] }))))).toBe('card');
    expect(operatorRenderKind(opData(root(build({ if: true }))))).toBe('card');
    expect(operatorRenderKind(opData(root(build({ if: [true, 1, 2] }))))).toBe('decision');
  });
});
