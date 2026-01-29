/**
 * Selection Service
 *
 * Manages node selection state for the editor.
 * Extracted from EditorContext for better modularity and testability.
 */

import { useState, useCallback, useMemo } from 'react';
import type { LogicNode } from '../types';
import { getDescendants } from '../utils/node-cloning';

export interface SelectionService {
  /** Currently selected node ID (primary selection) */
  selectedNodeId: string | null;
  /** Set of all selected node IDs (for multi-select) */
  selectedNodeIds: Set<string>;
  /** The currently selected node object */
  selectedNode: LogicNode | null;
  /** All selected node objects */
  selectedNodes: LogicNode[];
  /** Select a single node (clears multi-selection) */
  selectNode: (nodeId: string | null) => void;
  /** Set selection from array of node IDs */
  setSelection: (nodeIds: string[]) => void;
  /** Toggle a node in multi-selection (for Cmd/Ctrl+Click) */
  toggleNodeSelection: (nodeId: string) => void;
  /** Add a node to selection (for Shift+Click) */
  addToSelection: (nodeId: string) => void;
  /** Clear all selections */
  clearSelection: () => void;
  /** Select all nodes */
  selectAllNodes: () => void;
  /** Check if a node is selected */
  isNodeSelected: (nodeId: string) => boolean;
  /** Select a node and all its descendants */
  selectChildren: (nodeId: string) => void;
}

/**
 * Hook to create a selection service for managing node selections
 *
 * @param nodes - Current nodes array
 * @returns SelectionService object
 */
export function useSelectionService(nodes: LogicNode[]): SelectionService {
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedNodeIds, setSelectedNodeIds] = useState<Set<string>>(new Set());

  // Find the selected node from the nodes array
  const selectedNode = useMemo(() => {
    if (!selectedNodeId) return null;
    return nodes.find((n) => n.id === selectedNodeId) ?? null;
  }, [nodes, selectedNodeId]);

  // Compute all selected nodes
  const selectedNodes = useMemo(() => {
    return nodes.filter((n) => selectedNodeIds.has(n.id));
  }, [nodes, selectedNodeIds]);

  // Compute effective selectedNodeId - null if node doesn't exist
  const effectiveSelectedNodeId = selectedNode ? selectedNodeId : null;

  // Compute effective selectedNodeIds - only include existing nodes
  const effectiveSelectedNodeIds = useMemo(() => {
    const existingIds = new Set(nodes.map((n) => n.id));
    return new Set([...selectedNodeIds].filter((id) => existingIds.has(id)));
  }, [nodes, selectedNodeIds]);

  const selectNode = useCallback((nodeId: string | null) => {
    setSelectedNodeId(nodeId);
    setSelectedNodeIds(nodeId ? new Set([nodeId]) : new Set());
  }, []);

  const setSelection = useCallback((nodeIds: string[]) => {
    setSelectedNodeIds(new Set(nodeIds));
    setSelectedNodeId(nodeIds.length > 0 ? nodeIds[0] : null);
  }, []);

  const toggleNodeSelection = useCallback((nodeId: string) => {
    setSelectedNodeIds((prev) => {
      const next = new Set(prev);
      if (next.has(nodeId)) {
        next.delete(nodeId);
        if (selectedNodeId === nodeId) {
          const remaining = [...next];
          setSelectedNodeId(remaining.length > 0 ? remaining[0] : null);
        }
      } else {
        next.add(nodeId);
        if (!selectedNodeId) {
          setSelectedNodeId(nodeId);
        }
      }
      return next;
    });
  }, [selectedNodeId]);

  const addToSelection = useCallback((nodeId: string) => {
    setSelectedNodeIds((prev) => {
      const next = new Set(prev);
      next.add(nodeId);
      return next;
    });
    if (!selectedNodeId) {
      setSelectedNodeId(nodeId);
    }
  }, [selectedNodeId]);

  const clearSelection = useCallback(() => {
    setSelectedNodeId(null);
    setSelectedNodeIds(new Set());
  }, []);

  const selectAllNodes = useCallback(() => {
    const allIds = new Set(nodes.map((n) => n.id));
    setSelectedNodeIds(allIds);
    if (nodes.length > 0) {
      setSelectedNodeId(nodes[0].id);
    }
  }, [nodes]);

  const isNodeSelected = useCallback(
    (nodeId: string) => selectedNodeIds.has(nodeId),
    [selectedNodeIds]
  );

  const selectChildren = useCallback(
    (nodeId: string) => {
      const descendants = getDescendants(nodeId, nodes);
      const descendantIds = new Set(descendants.map((n) => n.id));
      descendantIds.add(nodeId);
      setSelectedNodeIds(descendantIds);
      setSelectedNodeId(nodeId);
    },
    [nodes]
  );

  return {
    selectedNodeId: effectiveSelectedNodeId,
    selectedNodeIds: effectiveSelectedNodeIds,
    selectedNode,
    selectedNodes,
    selectNode,
    setSelection,
    toggleNodeSelection,
    addToSelection,
    clearSelection,
    selectAllNodes,
    isNodeSelected,
    selectChildren,
  };
}
