/**
 * Clipboard State Hook
 *
 * Manages copy/paste operations for logic nodes.
 */

import { useCallback, useRef, useState, type Dispatch, type SetStateAction } from 'react';
import type { LogicNode } from '../../types';
import type { ClipboardData } from './types';
import {
  cloneNodesWithIdMapping,
  getDescendants,
  updateParentChildReference,
} from '../../utils/node-cloning';

export interface ClipboardDeps {
  selectedNode: LogicNode | null;
  internalNodes: LogicNode[];
  pushToUndoStack: (nodes: LogicNode[]) => void;
  setInternalNodes: Dispatch<SetStateAction<LogicNode[]>>;
  onNodesChange?: (nodes: LogicNode[]) => void;
  setSelectedNodeId: (id: string | null) => void;
  setPanelValues: Dispatch<SetStateAction<Record<string, unknown>>>;
  hasEditedRef: React.RefObject<boolean>;
}

export function useClipboardState(deps: ClipboardDeps) {
  const {
    selectedNode,
    internalNodes,
    pushToUndoStack,
    setInternalNodes,
    onNodesChange,
    setSelectedNodeId,
    setPanelValues,
    hasEditedRef,
  } = deps;

  // Clipboard payload lives in a ref (deep clone, mutated imperatively
  // alongside `pasteNode`); `canPaste` mirrors its non-empty state as
  // actual state so consumers can use it in render without reading the
  // ref during the render pass.
  const clipboardRef = useRef<ClipboardData | null>(null);
  const [canPaste, setCanPaste] = useState(false);

  const copyNode = useCallback(() => {
    if (!selectedNode) return;

    const descendants = getDescendants(selectedNode.id, internalNodes);
    const copiedNodes = [selectedNode, ...descendants].map((n) =>
      JSON.parse(JSON.stringify(n))
    );

    clipboardRef.current = {
      nodes: copiedNodes,
      rootId: selectedNode.id,
    };
    setCanPaste(copiedNodes.length > 0);
  }, [selectedNode, internalNodes]);

  const pasteNode = useCallback(() => {
    const clipboard = clipboardRef.current;
    if (!clipboard || clipboard.nodes.length === 0) return;

    setInternalNodes((prev) => {
      const { nodes: clonedNodes, newRootId } = cloneNodesWithIdMapping(
        clipboard.nodes,
        clipboard.rootId
      );

      const clonedRoot = clonedNodes.find((n) => n.id === newRootId);

      if (!clonedRoot || !newRootId) {
        console.warn('Paste failed: could not find cloned root node');
        return prev;
      }

      pushToUndoStack(prev);

      if (selectedNode) {
        const targetNode = prev.find((n) => n.id === selectedNode.id);
        if (targetNode && targetNode.data.parentId) {
          clonedRoot.data = {
            ...clonedRoot.data,
            parentId: targetNode.data.parentId,
            argIndex: targetNode.data.argIndex,
          };

          const targetDescendants = getDescendants(targetNode.id, prev);
          const targetIds = new Set([targetNode.id, ...targetDescendants.map((d) => d.id)]);

          let newNodes = prev.filter((n) => !targetIds.has(n.id));
          newNodes = updateParentChildReference(
            newNodes,
            targetNode.data.parentId,
            targetNode.id,
            newRootId
          );

          newNodes = [...newNodes, ...clonedNodes];

          hasEditedRef.current = true;
          onNodesChange?.(newNodes);
          setSelectedNodeId(newRootId);
          setPanelValues({});
          return newNodes;
        }
      }

      clonedRoot.data = {
        ...clonedRoot.data,
        parentId: undefined,
        argIndex: undefined,
      };

      const newNodes = clonedNodes;
      hasEditedRef.current = true;
      onNodesChange?.(newNodes);
      setSelectedNodeId(newRootId);
      setPanelValues({});
      return newNodes;
    });
  }, [selectedNode, pushToUndoStack, onNodesChange, setInternalNodes, setSelectedNodeId, setPanelValues, hasEditedRef]);

  return { copyNode, pasteNode, canPaste };
}
