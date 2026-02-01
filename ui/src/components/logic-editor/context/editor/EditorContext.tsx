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

  // Destructure stable callbacks from selection for use in deps arrays
  const {
    setSelectedNodeId,
    setSelectedNodeIds,
    selectNode: selectionSelectNode,
    setSelection: selectionSetSelection,
    toggleNodeSelection: selectionToggleNodeSelection,
    addToSelection: selectionAddToSelection,
    clearSelection: selectionClearSelection,
    selectAllNodes: selectionSelectAllNodes,
    selectChildren: selectionSelectChildren,
  } = selection;

  const history = useHistoryState(
    nodesRef,
    setInternalNodes,
    onNodesChange,
    selectionClearSelection
  );

  const clipboard = useClipboardState({
    selectedNode: selection.selectedNode,
    internalNodes,
    pushToUndoStack: history.pushToUndoStack,
    setInternalNodes,
    onNodesChange,
    setSelectedNodeId,
    setPanelValues,
    hasEditedRef,
  });

  const mutations = useNodeMutations({
    pushToUndoStack: history.pushToUndoStack,
    setInternalNodes,
    onNodesChange,
    selectedNodeId: selection.selectedNodeId,
    setSelectedNodeId,
    setPanelValues,
    hasEditedRef,
    nodes,
    internalNodes,
  });

  // --- Effects ---

  // Sync nodes from props when they change (e.g., expression change from parent)
  /* eslint-disable react-hooks/set-state-in-effect -- Syncing internal state from props is intentional */
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

  // Sync edit mode when prop changes (only responds to initialEditMode, not propNodes)
  useEffect(() => {
    setIsEditMode(initialEditMode);
    if (!initialEditMode) {
      setSelectedNodeId(null);
      setSelectedNodeIds(new Set());
      setPanelValues((prev) => Object.keys(prev).length === 0 ? prev : {});
      hasEditedRef.current = false;
    }
  }, [initialEditMode, setSelectedNodeId, setSelectedNodeIds]);
  /* eslint-enable react-hooks/set-state-in-effect */

  // Keep nodesRef in sync with internalNodes for undo/redo
  useEffect(() => {
    nodesRef.current = internalNodes;
  }, [internalNodes]);

  // --- Panel callbacks ---

  const setEditMode = useCallback((enabled: boolean) => {
    setIsEditMode(enabled);
    if (!enabled) {
      setSelectedNodeId(null);
      setSelectedNodeIds(new Set());
      setPanelValues({});
    }
  }, [setSelectedNodeId, setSelectedNodeIds]);

  const updatePanelValue = useCallback((fieldId: string, value: unknown) => {
    setPanelValues((prev) => ({ ...prev, [fieldId]: value }));
  }, []);

  const resetPanelValues = useCallback((values?: Record<string, unknown>) => {
    setPanelValues(values ?? {});
  }, []);

  // Wrap selection functions to also reset panel values
  const selectNode = useCallback((nodeId: string | null) => {
    selectionSelectNode(nodeId);
    setPanelValues({});
  }, [selectionSelectNode]);

  const setSelectionWrapped = useCallback((nodeIds: string[]) => {
    selectionSetSelection(nodeIds);
    setPanelValues({});
  }, [selectionSetSelection]);

  const toggleNodeSelection = useCallback((nodeId: string) => {
    selectionToggleNodeSelection(nodeId);
    setPanelValues({});
  }, [selectionToggleNodeSelection]);

  const addToSelection = useCallback((nodeId: string) => {
    selectionAddToSelection(nodeId);
    setPanelValues({});
  }, [selectionAddToSelection]);

  const clearSelection = useCallback(() => {
    selectionClearSelection();
    setPanelValues((prev) => Object.keys(prev).length === 0 ? prev : {});
  }, [selectionClearSelection]);

  const selectAllNodes = useCallback(() => {
    selectionSelectAllNodes();
    setPanelValues({});
  }, [selectionSelectAllNodes]);

  const selectChildren = useCallback((nodeId: string) => {
    selectionSelectChildren(nodeId);
    setPanelValues({});
  }, [selectionSelectChildren]);

  // Focus the properties panel on a specific node and optionally a field
  const focusPropertyPanel = useCallback(
    (nodeId: string, fieldId?: string) => {
      setSelectedNodeId(nodeId);
      setSelectedNodeIds(new Set([nodeId]));
      setPanelValues({});

      setTimeout(() => {
        propertyPanelFocusRef.current?.focusField(fieldId);
      }, 100);
    },
    [setSelectedNodeId, setSelectedNodeIds]
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
