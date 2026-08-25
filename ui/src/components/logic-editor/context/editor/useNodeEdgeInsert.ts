/**
 * Node Edge Insert Hook
 *
 * Handles inserting a new node on an existing edge between two nodes.
 *
 * Edges are oriented by the flow direction (child -> parent in 'flow',
 * parent -> child in 'hierarchy'), so the parent/child pair is derived from
 * the node data rather than from the edge's source/target order.
 */

import { useCallback, type Dispatch, type SetStateAction } from 'react';
import { v4 as uuidv4 } from 'uuid';
import type { LogicNode, OperatorNodeData, LiteralNodeData } from '../../types';
import { getOperator } from '../../config/operators';
import { getDescendants, replaceChildReference } from '../../utils/node-cloning';

/**
 * Which end of an edge is the parent follows the tree, not the edge
 * orientation ('flow' draws child -> parent, 'hierarchy' the reverse).
 * Returns null when the two nodes are not actually parent and child.
 */
export function resolveEdgeParentChild(
  nodes: LogicNode[],
  sourceId: string,
  targetId: string
): { parentNode: LogicNode; childNode: LogicNode } | null {
  const sourceNode = nodes.find((n) => n.id === sourceId);
  const targetNode = nodes.find((n) => n.id === targetId);
  if (!sourceNode || !targetNode) return null;
  if (targetNode.data.parentId === sourceId) {
    return { parentNode: sourceNode, childNode: targetNode };
  }
  if (sourceNode.data.parentId === targetId) {
    return { parentNode: targetNode, childNode: sourceNode };
  }
  return null;
}

/**
 * A `var` or literal node has no slot to hold the edge's existing child, so
 * inserting one discards that child and everything under it. True when that
 * would actually throw work away (the child is more than a bare leaf), which
 * the picker uses to say so before the click.
 */
export function edgeInsertDiscardsSubtree(
  nodes: LogicNode[],
  sourceId: string,
  targetId: string
): boolean {
  const pair = resolveEdgeParentChild(nodes, sourceId, targetId);
  if (!pair) return false;
  return getDescendants(pair.childNode.id, nodes).length > 0;
}

export function useNodeEdgeInsert(
  pushToUndoStack: (nodes: LogicNode[]) => void,
  setInternalNodes: Dispatch<SetStateAction<LogicNode[]>>,
  onNodesChange: ((nodes: LogicNode[]) => void) | undefined,
  setSelectedNodeId: (id: string | null) => void,
  setPanelValues: Dispatch<SetStateAction<Record<string, unknown>>>,
  hasEditedRef: React.RefObject<boolean>
) {
  const insertNodeOnEdge = useCallback(
    (sourceId: string, targetId: string, operatorName: string) => {
      setInternalNodes((prev) => {
        const pair = resolveEdgeParentChild(prev, sourceId, targetId);
        if (!pair) return prev;
        const { parentNode, childNode } = pair;

        pushToUndoStack(prev);

        const newNodeId = uuidv4();
        let newNode: LogicNode;
        // A variable or literal replaces the child subtree; an operator wraps it
        let replacesChild = false;

        if (operatorName === '__variable__') {
          const varOpConfig = getOperator('var');
          replacesChild = true;
          newNode = {
            id: newNodeId,
            type: 'operator',
            position: { x: 0, y: 0 },
            data: {
              type: 'operator',
              operator: 'var',
              category: varOpConfig?.category || 'variable',
              label: varOpConfig?.label || 'var',
              icon: 'database',
              cells: [
                { type: 'editable', fieldId: 'path', fieldType: 'text', value: '', placeholder: 'path', label: '', index: 0 },
              ],
              expression: { var: '' },
              parentId: parentNode.id,
              argIndex: childNode.data.argIndex,
              branchType: childNode.data.branchType,
            } as OperatorNodeData,
          };
        } else if (operatorName === '__literal__') {
          replacesChild = true;
          newNode = {
            id: newNodeId,
            type: 'literal',
            position: { x: 0, y: 0 },
            data: {
              type: 'literal',
              value: 0,
              valueType: 'number',
              expression: 0,
              parentId: parentNode.id,
              argIndex: childNode.data.argIndex,
              branchType: childNode.data.branchType,
            } as LiteralNodeData,
          };
        } else {
          const opConfig = getOperator(operatorName);
          newNode = {
            id: newNodeId,
            type: 'operator',
            position: { x: 0, y: 0 },
            data: {
              type: 'operator',
              operator: operatorName,
              category: opConfig?.category || 'arithmetic',
              label: opConfig?.label || operatorName,
              icon: 'list',
              cells: [{ type: 'branch', branchId: childNode.id, index: 0 }],
              expression: { [operatorName]: [childNode.data.expression ?? null] },
              parentId: parentNode.id,
              argIndex: childNode.data.argIndex,
              branchType: childNode.data.branchType,
            } as OperatorNodeData,
          };
        }

        let remaining = prev;
        if (replacesChild) {
          // A var / literal has no branch cell to re-parent the old child
          // into, so it and its descendants go. The picker flags this before
          // the click (see `edgeInsertDiscardsSubtree`), and the undo entry
          // pushed above restores it.
          const removedIds = new Set([childNode.id, ...getDescendants(childNode.id, prev).map((d) => d.id)]);
          remaining = prev.filter((n) => !removedIds.has(n.id));
        }

        const newNodes = remaining.map((n) => {
          if (!replacesChild && n.id === childNode.id) {
            return {
              ...n,
              data: {
                ...n.data,
                parentId: newNodeId,
                argIndex: 0,
                branchType: 'branch' as const,
              },
            };
          }
          if (n.id === parentNode.id) {
            return replaceChildReference(n, childNode.id, newNodeId);
          }
          return n;
        });

        newNodes.push(newNode);

        hasEditedRef.current = true;
        onNodesChange?.(newNodes);
        setSelectedNodeId(newNodeId);
        setPanelValues({});
        return newNodes;
      });
    },
    [onNodesChange, pushToUndoStack, setInternalNodes, setSelectedNodeId, setPanelValues, hasEditedRef]
  );

  return { insertNodeOnEdge };
}
