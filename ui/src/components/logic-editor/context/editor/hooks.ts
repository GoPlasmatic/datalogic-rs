/**
 * Editor Context Hooks
 *
 * Convenience hooks for accessing parts of the editor context.
 */

import { useContext, createRef } from 'react';
import { EditorContext } from './context';
import type { EditorContextValue } from './types';

// eslint-disable-next-line @typescript-eslint/no-unused-vars
const noop = (..._: unknown[]) => {};
// eslint-disable-next-line @typescript-eslint/no-unused-vars
const noopFalse = (..._: unknown[]) => false;

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
  selectNode: noop as unknown as (nodeId: string | null) => void,
  setSelection: noop as unknown as (nodeIds: string[]) => void,
  toggleNodeSelection: noop as unknown as (nodeId: string) => void,
  addToSelection: noop as unknown as (nodeId: string) => void,
  clearSelection: noop,
  selectAllNodes: noop,
  isNodeSelected: noopFalse as unknown as (nodeId: string) => boolean,
  setEditMode: noop as unknown as (enabled: boolean) => void,
  updatePanelValue: noop as unknown as (fieldId: string, value: unknown) => void,
  resetPanelValues: noop as unknown as (values?: Record<string, unknown>) => void,
  updateNode: noop as unknown as (nodeId: string, newData: unknown) => void,
  deleteNode: noop as unknown as (nodeId: string) => void,
  applyPanelChanges: noop,
  addArgumentToNode: noop as unknown as (nodeId: string, nodeType?: unknown, operatorName?: string) => void,
  removeArgumentFromNode: noop as unknown as (nodeId: string, argIndex: number) => void,
  getChildNodes: (() => []) as unknown as (parentId: string) => [],
  createNode: noop as unknown as (type: unknown, operatorName?: string) => void,
  hasNodes: () => false,
  insertNodeOnEdge: noop as unknown as (sourceId: string, targetId: string, operatorName: string) => void,
  undo: noop,
  redo: noop,
  canUndo: false,
  canRedo: false,
  copyNode: noop,
  pasteNode: noop,
  canPaste: false,
  wrapNodeInOperator: noop as unknown as (nodeId: string, operator: string) => void,
  duplicateNode: noop as unknown as (nodeId: string) => void,
  selectChildren: noop as unknown as (nodeId: string) => void,
  focusPropertyPanel: noop as unknown as (nodeId: string, fieldId?: string) => void,
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
