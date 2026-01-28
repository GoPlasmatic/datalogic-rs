/**
 * Editor Context Hooks
 *
 * Convenience hooks for accessing parts of the editor context.
 */

import { useContext } from 'react';
import { EditorContext } from './context';
import type { EditorContextValue } from './types';

/**
 * Hook to access the full editor context
 */
export function useEditorContext(): EditorContextValue {
  const context = useContext(EditorContext);
  if (!context) {
    throw new Error('useEditorContext must be used within an EditorProvider');
  }
  return context;
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
