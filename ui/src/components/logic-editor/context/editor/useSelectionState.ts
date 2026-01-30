/**
 * Selection State Hook
 *
 * Manages node selection state: single select, multi-select, toggle, clear, select all.
 */

import { useState, useCallback, useMemo } from 'react';
import type { LogicNode } from '../../types';
import { getDescendants } from '../../utils/node-cloning';

export function useSelectionState(nodes: LogicNode[], internalNodes: LogicNode[]) {
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedNodeIds, setSelectedNodeIds] = useState<Set<string>>(new Set());

  // Find the selected node from the nodes array
  const selectedNode = useMemo(() => {
    if (!selectedNodeId) return null;
    return nodes.find((n) => n.id === selectedNodeId) ?? null;
  }, [nodes, selectedNodeId]);

  // Compute all selected nodes (filter out any that no longer exist)
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
    const allIds = new Set(internalNodes.map((n) => n.id));
    setSelectedNodeIds(allIds);
    if (internalNodes.length > 0) {
      setSelectedNodeId(internalNodes[0].id);
    }
  }, [internalNodes]);

  const isNodeSelected = useCallback(
    (nodeId: string) => selectedNodeIds.has(nodeId),
    [selectedNodeIds]
  );

  const selectChildren = useCallback(
    (nodeId: string) => {
      const descendants = getDescendants(nodeId, internalNodes);
      const descendantIds = new Set(descendants.map((n) => n.id));
      descendantIds.add(nodeId);
      setSelectedNodeIds(descendantIds);
      setSelectedNodeId(nodeId);
    },
    [internalNodes]
  );

  return {
    selectedNodeId,
    selectedNodeIds,
    selectedNode,
    selectedNodes,
    effectiveSelectedNodeId,
    effectiveSelectedNodeIds,
    selectNode,
    setSelection,
    toggleNodeSelection,
    addToSelection,
    clearSelection,
    selectAllNodes,
    isNodeSelected,
    selectChildren,
    setSelectedNodeId,
    setSelectedNodeIds,
  };
}
