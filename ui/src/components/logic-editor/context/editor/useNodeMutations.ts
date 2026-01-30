/**
 * Node Mutations Hook (Composition)
 *
 * Thin composition hook that combines node creation, edge insertion,
 * and node operations into a single unified API.
 */

import { type Dispatch, type SetStateAction } from 'react';
import type { LogicNode } from '../../types';
import { useNodeCreation } from './useNodeCreation';
import { useNodeEdgeInsert } from './useNodeEdgeInsert';
import { useNodeOperations } from './useNodeOperations';

export interface NodeMutationDeps {
  pushToUndoStack: (nodes: LogicNode[]) => void;
  setInternalNodes: Dispatch<SetStateAction<LogicNode[]>>;
  onNodesChange?: (nodes: LogicNode[]) => void;
  selectedNodeId: string | null;
  setSelectedNodeId: (id: string | null) => void;
  setPanelValues: Dispatch<SetStateAction<Record<string, unknown>>>;
  hasEditedRef: React.RefObject<boolean>;
  nodes: LogicNode[];
  internalNodes: LogicNode[];
}

export function useNodeMutations(deps: NodeMutationDeps) {
  const {
    pushToUndoStack,
    setInternalNodes,
    onNodesChange,
    selectedNodeId,
    setSelectedNodeId,
    setPanelValues,
    hasEditedRef,
    nodes,
    internalNodes,
  } = deps;

  const { createNode, hasNodes } = useNodeCreation(
    pushToUndoStack, setInternalNodes, onNodesChange,
    setSelectedNodeId, setPanelValues, hasEditedRef, internalNodes
  );

  const { insertNodeOnEdge } = useNodeEdgeInsert(
    pushToUndoStack, setInternalNodes, onNodesChange,
    setSelectedNodeId, setPanelValues, hasEditedRef
  );

  const operations = useNodeOperations(
    pushToUndoStack, setInternalNodes, onNodesChange,
    selectedNodeId, setSelectedNodeId, setPanelValues,
    hasEditedRef, nodes
  );

  return {
    ...operations,
    hasNodes,
    createNode,
    insertNodeOnEdge,
  };
}
