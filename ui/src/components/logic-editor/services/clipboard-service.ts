/**
 * Clipboard Service
 *
 * Provides copy/paste functionality for the editor.
 * Extracted from EditorContext for better modularity and testability.
 */

import { useRef, useState, useCallback, useMemo } from 'react';
import type { LogicNode } from '../types';
import type { ClipboardData } from '../context/editor/types';
import { cloneNodesWithIdMapping, getDescendants, updateParentChildReference } from '../utils/node-cloning';

export interface ClipboardService {
  /** Copy the given node and its descendants to clipboard */
  copyNode: (node: LogicNode, allNodes: LogicNode[]) => void;
  /** Paste clipboard contents, returning the paste result */
  pasteNodes: (targetNode: LogicNode | null, allNodes: LogicNode[]) => PasteResult | null;
  /** Whether paste is available */
  canPaste: boolean;
  /** Clear the clipboard */
  clearClipboard: () => void;
}

export interface PasteResult {
  /** Updated nodes array after paste */
  nodes: LogicNode[];
  /** ID of the new root node that was pasted */
  newRootId: string;
  /** IDs of nodes that were removed (when replacing a selected node) */
  removedIds: Set<string>;
}

/**
 * Hook to create a clipboard service for copy/paste functionality
 *
 * @returns ClipboardService object
 */
export function useClipboardService(): ClipboardService {
  const clipboardRef = useRef<ClipboardData | null>(null);
  const [clipboardVersion, setClipboardVersion] = useState(0);

  const copyNode = useCallback((node: LogicNode, allNodes: LogicNode[]) => {
    // Get all descendants
    const descendants = getDescendants(node.id, allNodes);

    // Clone the nodes for clipboard (deep copy)
    const copiedNodes = [node, ...descendants].map((n) =>
      JSON.parse(JSON.stringify(n))
    );

    clipboardRef.current = {
      nodes: copiedNodes,
      rootId: node.id,
    };
    setClipboardVersion((v) => v + 1);
  }, []);

  const pasteNodes = useCallback((
    targetNode: LogicNode | null,
    allNodes: LogicNode[]
  ): PasteResult | null => {
    const clipboard = clipboardRef.current;
    if (!clipboard || clipboard.nodes.length === 0) return null;

    // Clone nodes with ID remapping
    const { nodes: clonedNodes, newRootId } = cloneNodesWithIdMapping(
      clipboard.nodes,
      clipboard.rootId
    );

    const clonedRoot = clonedNodes.find((n) => n.id === newRootId)!;
    const removedIds = new Set<string>();

    // If there's a target node that isn't the root, replace it
    if (targetNode && targetNode.data.parentId) {
      // Update the cloned root to have the same parent info
      clonedRoot.data = {
        ...clonedRoot.data,
        parentId: targetNode.data.parentId,
        argIndex: targetNode.data.argIndex,
      };

      // Remove the target node and its descendants
      const targetDescendants = getDescendants(targetNode.id, allNodes);
      const targetIds = new Set([targetNode.id, ...targetDescendants.map((d) => d.id)]);
      targetIds.forEach((id) => removedIds.add(id));

      // Filter out removed nodes and update parent references
      let newNodes = allNodes.filter((n) => !targetIds.has(n.id));
      newNodes = updateParentChildReference(
        newNodes,
        targetNode.data.parentId,
        targetNode.id,
        newRootId
      );

      newNodes = [...newNodes, ...clonedNodes];

      return { nodes: newNodes, newRootId, removedIds };
    }

    // If no selection or selected is root, replace entire tree
    clonedRoot.data = {
      ...clonedRoot.data,
      parentId: undefined,
      argIndex: undefined,
    };

    // Remove all old nodes
    allNodes.forEach((n) => removedIds.add(n.id));

    return { nodes: clonedNodes, newRootId, removedIds };
  }, []);

  const canPaste = useMemo(
    () => clipboardRef.current !== null && clipboardRef.current.nodes.length > 0,
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [clipboardVersion]
  );

  const clearClipboard = useCallback(() => {
    clipboardRef.current = null;
    setClipboardVersion((v) => v + 1);
  }, []);

  return {
    copyNode,
    pasteNodes,
    canPaste,
    clearClipboard,
  };
}
