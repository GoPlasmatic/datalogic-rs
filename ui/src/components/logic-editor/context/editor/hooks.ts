/**
 * Editor Context Hooks
 *
 * Convenience hooks for accessing parts of the editor context.
 */

import { useContext, createRef } from 'react';
import { EditorContext } from './context';
import type { EditorContextValue } from './types';

// A function with fewer parameters is assignable to any callback type with
// more, so these fit every action without casts.
const noop = () => {};
const noopFalse = () => false;

/**
 * Default read-only context value returned when no EditorProvider is present.
 * All actions are no-ops and state reflects a non-editable, unselected state.
 */
const readOnlyDefault: EditorContextValue = {
  selectedNodeId: null,
  selectedNodeIds: new Set(),
  isEditMode: false,
  panelValues: {},
  selectedNode: null,
  selectedNodes: [],
  nodes: [],
  selectNode: noop,
  setSelection: noop,
  toggleNodeSelection: noop,
  addToSelection: noop,
  clearSelection: noop,
  selectAllNodes: noop,
  isNodeSelected: noopFalse,
  setEditMode: noop,
  updatePanelValue: noop,
  resetPanelValues: noop,
  updateNode: noop,
  deleteNode: noop,
  applyPanelChanges: noop,
  addArgumentToNode: noop,
  removeArgumentFromNode: noop,
  getChildNodes: () => [],
  createNode: noop,
  hasNodes: () => false,
  insertNodeOnEdge: noop,
  undo: noop,
  redo: noop,
  canUndo: false,
  canRedo: false,
  copyNode: noop,
  pasteNode: noop,
  canPaste: false,
  wrapNodeInOperator: noop,
  duplicateNode: noop,
  selectChildren: noop,
  focusPropertyPanel: noop,
  propertyPanelFocusRef: createRef(),
};

/**
 * Hook to access the full editor context.
 * Returns a safe read-only default when used outside an EditorProvider.
 */
export function useEditorContext(): EditorContextValue {
  const context = useContext(EditorContext);
  return context ?? readOnlyDefault;
}

/**
 * Hook to access just the selection state
 */
export function useSelection() {
  const { selectedNodeId, selectedNode, selectNode } = useEditorContext();
  return { selectedNodeId, selectedNode, selectNode };
}

/**
 * Hook to access just the edit mode state
 */
export function useEditMode() {
  const { isEditMode, setEditMode } = useEditorContext();
  return { isEditMode, setEditMode };
}

/**
 * Hook to access just the panel values
 */
export function usePanelValues() {
  const { panelValues, updatePanelValue, resetPanelValues } = useEditorContext();
  return { panelValues, updatePanelValue, resetPanelValues };
}
