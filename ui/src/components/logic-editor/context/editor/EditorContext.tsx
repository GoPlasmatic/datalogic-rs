/**
 * Editor Context Provider
 *
 * Provides state management for the visual editor including
 * node selection, edit mode, and panel field values.
 *
 * This context composes functionality from extracted hooks:
 * - useSelectionState: node selection (single, multi, toggle, clear)
 * - useHistoryState: undo/redo
 * - useClipboardState: copy/paste
 * - useNodeMutations: node CRUD operations
 */

import { useState, useCallback, useMemo, useEffect, useRef, type ReactNode } from 'react';
import type { LogicNode } from '../../types';
import type { EditorContextValue } from './types';
import { EditorContext } from './context';
import { panelValuesToNodeData } from '../../utils/node-updaters';
import { useSelectionState } from './useSelectionState';
import { useHistoryState } from './useHistoryState';
import { useClipboardState } from './useClipboardState';
import { useNodeMutations } from './useNodeMutations';

interface EditorProviderProps {
  children: ReactNode;
  nodes: LogicNode[];
  initialEditMode?: boolean;
  /** Callback when nodes change (for propagating changes up) */
  onNodesChange?: (nodes: LogicNode[]) => void;
}

export function EditorProvider({
  children,
  nodes: propNodes,
  initialEditMode = false,
  onNodesChange,
}: EditorProviderProps) {
  const [isEditMode, setIsEditMode] = useState(initialEditMode);
  const [panelValues, setPanelValues] = useState<Record<string, unknown>>({});

  // Internal nodes state - starts from props but can be modified
  const [internalNodes, setInternalNodes] = useState<LogicNode[]>(propNodes);

  // Ref to track current nodes for undo/redo (avoids stale closures)
  const nodesRef = useRef<LogicNode[]>(propNodes);

  // Track if we should use internal nodes (after first edit) or prop nodes
  const hasEditedRef = useRef(false);

  // Ref for property panel focus
  const propertyPanelFocusRef = useRef<{ focusField: (fieldId?: string) => void } | null>(null);

  // Use internal nodes when in edit mode, otherwise prop nodes
  const nodes = isEditMode ? internalNodes : propNodes;

  // --- Compose hooks ---

  const selection = useSelectionState(nodes, internalNodes);

  const history = useHistoryState(
    nodesRef,
    setInternalNodes,
    onNodesChange,
    selection.clearSelection
  );

  const clipboard = useClipboardState({
    selectedNode: selection.selectedNode,
    internalNodes,
    pushToUndoStack: history.pushToUndoStack,
    setInternalNodes,
    onNodesChange,
    setSelectedNodeId: selection.setSelectedNodeId,
    setPanelValues,
    hasEditedRef,
  });

  const mutations = useNodeMutations({
    pushToUndoStack: history.pushToUndoStack,
    setInternalNodes,
    onNodesChange,
    selectedNodeId: selection.selectedNodeId,
    setSelectedNodeId: selection.setSelectedNodeId,
    setPanelValues,
    hasEditedRef,
    nodes,
    internalNodes,
  });

  // --- Effects ---

  // Sync nodes from props when they change (e.g., expression change from parent)
  useEffect(() => {
    if (
      !hasEditedRef.current ||
      propNodes.length !== internalNodes.length ||
      propNodes[0]?.id !== internalNodes[0]?.id
    ) {
      setInternalNodes(propNodes);
      hasEditedRef.current = false;
    }
  }, [propNodes]); // eslint-disable-line react-hooks/exhaustive-deps

  // Sync edit mode when prop changes
  useEffect(() => {
    setIsEditMode(initialEditMode);
    if (!initialEditMode) {
      selection.setSelectedNodeId(null);
      selection.setSelectedNodeIds(new Set());
      setPanelValues({});
      setInternalNodes(propNodes);
      hasEditedRef.current = false;
    }
  }, [initialEditMode, propNodes]); // eslint-disable-line react-hooks/exhaustive-deps

  // Keep nodesRef in sync with internalNodes for undo/redo
  useEffect(() => {
    nodesRef.current = internalNodes;
  }, [internalNodes]);

  // --- Panel callbacks ---

  const setEditMode = useCallback((enabled: boolean) => {
    setIsEditMode(enabled);
    if (!enabled) {
      selection.setSelectedNodeId(null);
      selection.setSelectedNodeIds(new Set());
      setPanelValues({});
    }
  }, [selection]);

  const updatePanelValue = useCallback((fieldId: string, value: unknown) => {
    setPanelValues((prev) => ({ ...prev, [fieldId]: value }));
  }, []);

  const resetPanelValues = useCallback((values?: Record<string, unknown>) => {
    setPanelValues(values ?? {});
  }, []);

  // Wrap selection functions to also reset panel values
  const selectNode = useCallback((nodeId: string | null) => {
    selection.selectNode(nodeId);
    setPanelValues({});
  }, [selection]);

  const setSelectionWrapped = useCallback((nodeIds: string[]) => {
    selection.setSelection(nodeIds);
    setPanelValues({});
  }, [selection]);

  const toggleNodeSelection = useCallback((nodeId: string) => {
    selection.toggleNodeSelection(nodeId);
    setPanelValues({});
  }, [selection]);

  const addToSelection = useCallback((nodeId: string) => {
    selection.addToSelection(nodeId);
    setPanelValues({});
  }, [selection]);

  const clearSelection = useCallback(() => {
    selection.clearSelection();
    setPanelValues({});
  }, [selection]);

  const selectAllNodes = useCallback(() => {
    selection.selectAllNodes();
    setPanelValues({});
  }, [selection]);

  const selectChildren = useCallback((nodeId: string) => {
    selection.selectChildren(nodeId);
    setPanelValues({});
  }, [selection]);

  // Focus the properties panel on a specific node and optionally a field
  const focusPropertyPanel = useCallback(
    (nodeId: string, fieldId?: string) => {
      selection.setSelectedNodeId(nodeId);
      selection.setSelectedNodeIds(new Set([nodeId]));
      setPanelValues({});

      setTimeout(() => {
        propertyPanelFocusRef.current?.focusField(fieldId);
      }, 100);
    },
    [selection]
  );

  // Apply current panel values to the selected node
  const applyPanelChanges = useCallback(() => {
    if (!selection.selectedNode || Object.keys(panelValues).length === 0) {
      return;
    }

    const updatedData = panelValuesToNodeData(selection.selectedNode.data, panelValues);

    if (JSON.stringify(selection.selectedNode.data) !== JSON.stringify(updatedData)) {
      mutations.updateNode(selection.selectedNode.id, updatedData);
    }
  }, [selection.selectedNode, panelValues, mutations]);

  // --- Build context value ---

  const value = useMemo<EditorContextValue>(
    () => ({
      selectedNodeId: selection.effectiveSelectedNodeId,
      selectedNodeIds: selection.effectiveSelectedNodeIds,
      isEditMode,
      panelValues,
      selectedNode: selection.selectedNode,
      selectedNodes: selection.selectedNodes,
      nodes,
      selectNode,
      setSelection: setSelectionWrapped,
      toggleNodeSelection,
      addToSelection,
      clearSelection,
      selectAllNodes,
      isNodeSelected: selection.isNodeSelected,
      setEditMode,
      updatePanelValue,
      resetPanelValues,
      updateNode: mutations.updateNode,
      deleteNode: mutations.deleteNode,
      applyPanelChanges,
      addArgumentToNode: mutations.addArgumentToNode,
      removeArgumentFromNode: mutations.removeArgumentFromNode,
      getChildNodes: mutations.getChildNodes,
      createNode: mutations.createNode,
      hasNodes: mutations.hasNodes,
      insertNodeOnEdge: mutations.insertNodeOnEdge,
      undo: history.undo,
      redo: history.redo,
      canUndo: history.canUndo,
      canRedo: history.canRedo,
      copyNode: clipboard.copyNode,
      pasteNode: clipboard.pasteNode,
      canPaste: clipboard.canPaste,
      wrapNodeInOperator: mutations.wrapNodeInOperator,
      duplicateNode: mutations.duplicateNode,
      selectChildren,
      focusPropertyPanel,
      propertyPanelFocusRef,
    }),
    [
      selection.effectiveSelectedNodeId,
      selection.effectiveSelectedNodeIds,
      isEditMode,
      panelValues,
      selection.selectedNode,
      selection.selectedNodes,
      nodes,
      selectNode,
      setSelectionWrapped,
      toggleNodeSelection,
      addToSelection,
      clearSelection,
      selectAllNodes,
      selection.isNodeSelected,
      setEditMode,
      updatePanelValue,
      resetPanelValues,
      mutations,
      applyPanelChanges,
      history.undo,
      history.redo,
      history.canUndo,
      history.canRedo,
      clipboard.copyNode,
      clipboard.pasteNode,
      clipboard.canPaste,
      selectChildren,
      focusPropertyPanel,
    ]
  );

  return (
    <EditorContext.Provider value={value}>
      {children}
    </EditorContext.Provider>
  );
}
