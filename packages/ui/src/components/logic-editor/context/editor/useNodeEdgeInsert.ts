/**
 * Node Edge Insert Hook
 *
 * Handles inserting a new node on an existing edge between two nodes.
 */

import { useCallback, type Dispatch, type SetStateAction } from 'react';
import { v4 as uuidv4 } from 'uuid';
import type { LogicNode, OperatorNodeData, LiteralNodeData } from '../../types';
import { getOperator } from '../../config/operators';

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
        const sourceNode = prev.find((n) => n.id === sourceId);
        const targetNode = prev.find((n) => n.id === targetId);

        if (!sourceNode || !targetNode) return prev;

        pushToUndoStack(prev);

        const newNodeId = uuidv4();
        let newNode: LogicNode;

        if (operatorName === '__variable__') {
          const varOpConfig = getOperator('var');
          newNode = {
            id: newNodeId,
            type: 'operator',
            position: { x: 0, y: 0 },
            data: {
              type: 'operator',
              operator: 'var',
              category: varOpConfig?.category || 'accessor',
              label: varOpConfig?.label || 'var',
              icon: 'database',
              cells: [
                { type: 'editable', fieldId: 'path', fieldType: 'text', value: '', placeholder: 'path', label: '', index: 0 },
              ],
              expression: { var: '' },
              parentId: sourceId,
              argIndex: targetNode.data.argIndex,
            } as OperatorNodeData,
          };
        } else if (operatorName === '__literal__') {
          newNode = {
            id: newNodeId,
            type: 'literal',
            position: { x: 0, y: 0 },
            data: {
              type: 'literal',
              value: 0,
              valueType: 'number',
              expression: 0,
              parentId: sourceId,
              argIndex: targetNode.data.argIndex,
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
              cells: [{ type: 'branch', branchId: targetId, index: 0 }],
              expression: { [operatorName]: [] },
              parentId: sourceId,
              argIndex: targetNode.data.argIndex,
            } as OperatorNodeData,
          };
        }

        const newNodes = prev.map((n) => {
          if (n.id === targetId) {
            return {
              ...n,
              data: {
                ...n.data,
                parentId: newNodeId,
                argIndex: 0,
              },
            };
          }

          if (n.id === sourceId && n.data.type === 'operator') {
            const opData = n.data as OperatorNodeData;
            if (opData.cells) {
              return {
                ...n,
                data: {
                  ...opData,
                  cells: opData.cells.map((cell) => {
                    if (cell.branchId === targetId) {
                      return { ...cell, branchId: newNodeId };
                    }
                    if (cell.conditionBranchId === targetId) {
                      return { ...cell, conditionBranchId: newNodeId };
                    }
                    if (cell.thenBranchId === targetId) {
                      return { ...cell, thenBranchId: newNodeId };
                    }
                    return cell;
                  }),
                },
              };
            }
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
