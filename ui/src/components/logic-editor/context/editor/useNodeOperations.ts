/**
 * Node Operations Hook
 *
 * Handles update, delete, get children, add/remove arguments,
 * wrap in operator, and duplicate operations on logic nodes.
 */

import { useCallback, type Dispatch, type SetStateAction } from 'react';
import type { LogicNode, LogicNodeData, OperatorNodeData } from '../../types';
import { deleteNodeAndDescendants } from '../../utils/node-deletion';
import {
  addArgument,
  removeArgument,
  wrapInOperator,
  duplicateNodeTree,
} from '../../services/node-mutation-service';

export function useNodeOperations(
  pushToUndoStack: (nodes: LogicNode[]) => void,
  setInternalNodes: Dispatch<SetStateAction<LogicNode[]>>,
  onNodesChange: ((nodes: LogicNode[]) => void) | undefined,
  selectedNodeId: string | null,
  setSelectedNodeId: (id: string | null) => void,
  setPanelValues: Dispatch<SetStateAction<Record<string, unknown>>>,
  hasEditedRef: React.RefObject<boolean>,
  nodes: LogicNode[]
) {
  const updateNode = useCallback(
    (nodeId: string, newData: Partial<LogicNodeData>) => {
      setInternalNodes((prev) => {
        pushToUndoStack(prev);

        const newNodes = prev.map((node) => {
          if (node.id === nodeId) {
            return {
              ...node,
              data: { ...node.data, ...newData } as LogicNodeData,
            };
          }
          return node;
        });
        hasEditedRef.current = true;
        onNodesChange?.(newNodes);
        return newNodes;
      });
    },
    [onNodesChange, pushToUndoStack, setInternalNodes, hasEditedRef]
  );

  const deleteNode = useCallback(
    (nodeId: string) => {
      setInternalNodes((prev) => {
        pushToUndoStack(prev);

        const newNodes = deleteNodeAndDescendants(nodeId, prev);
        hasEditedRef.current = true;
        if (selectedNodeId === nodeId) {
          setSelectedNodeId(null);
          setPanelValues({});
        }
        onNodesChange?.(newNodes);
        return newNodes;
      });
    },
    [selectedNodeId, onNodesChange, pushToUndoStack, setInternalNodes, setSelectedNodeId, setPanelValues, hasEditedRef]
  );

  const getChildNodes = useCallback(
    (parentId: string): LogicNode[] => {
      const parentNode = nodes.find((n) => n.id === parentId);

      if (parentNode?.data.type === 'operator') {
        const vcData = parentNode.data as OperatorNodeData;
        if (vcData.cells) {
          const childIds: string[] = [];

          for (const cell of vcData.cells) {
            if (cell.branchId) childIds.push(cell.branchId);
            if (cell.conditionBranchId) childIds.push(cell.conditionBranchId);
            if (cell.thenBranchId) childIds.push(cell.thenBranchId);
          }

          if (childIds.length > 0) {
            return childIds
              .map((id) => nodes.find((n) => n.id === id))
              .filter((n): n is LogicNode => n !== undefined);
          }
        }
      }

      return nodes
        .filter((n) => n.data.parentId === parentId)
        .sort((a, b) => (a.data.argIndex ?? 0) - (b.data.argIndex ?? 0));
    },
    [nodes]
  );

  const addArgumentToNode = useCallback(
    (nodeId: string, nodeType: 'literal' | 'variable' | 'operator' = 'literal', operatorName?: string) => {
      setInternalNodes((prev) => {
        pushToUndoStack(prev);

        const result = addArgument(prev, nodeId, nodeType, operatorName);
        if (!result) return prev;

        hasEditedRef.current = true;
        onNodesChange?.(result.nodes);
        return result.nodes;
      });
    },
    [onNodesChange, pushToUndoStack, setInternalNodes, hasEditedRef]
  );

  const removeArgumentFromNode = useCallback(
    (nodeId: string, argIndex: number) => {
      setInternalNodes((prev) => {
        pushToUndoStack(prev);

        const result = removeArgument(prev, nodeId, argIndex);
        if (!result) return prev;

        hasEditedRef.current = true;
        onNodesChange?.(result);
        return result;
      });
    },
    [onNodesChange, pushToUndoStack, setInternalNodes, hasEditedRef]
  );

  const wrapNodeInOperatorFn = useCallback(
    (nodeId: string, operator: string) => {
      setInternalNodes((prev) => {
        pushToUndoStack(prev);

        const result = wrapInOperator(prev, nodeId, operator);
        if (!result) return prev;

        const wrapperNode = result[result.length - 1];
        const newOperatorId = wrapperNode.id;

        hasEditedRef.current = true;
        onNodesChange?.(result);
        setSelectedNodeId(newOperatorId);
        setPanelValues({});
        return result;
      });
    },
    [onNodesChange, pushToUndoStack, setInternalNodes, setSelectedNodeId, setPanelValues, hasEditedRef]
  );

  const duplicateNodeFn = useCallback(
    (nodeId: string) => {
      setInternalNodes((prev) => {
        pushToUndoStack(prev);

        const result = duplicateNodeTree(prev, nodeId);
        if (!result) return prev;

        hasEditedRef.current = true;
        onNodesChange?.(result.nodes);
        setSelectedNodeId(result.newRootId);
        setPanelValues({});
        return result.nodes;
      });
    },
    [pushToUndoStack, onNodesChange, setInternalNodes, setSelectedNodeId, setPanelValues, hasEditedRef]
  );

  return {
    updateNode,
    deleteNode,
    getChildNodes,
    addArgumentToNode,
    removeArgumentFromNode,
    wrapNodeInOperator: wrapNodeInOperatorFn,
    duplicateNode: duplicateNodeFn,
  };
}
