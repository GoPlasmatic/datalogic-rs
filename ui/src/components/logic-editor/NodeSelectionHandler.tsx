/**
 * NodeSelectionHandler
 *
 * Bridges ReactFlow's node selection with our EditorContext.
 * Must be used inside both ReactFlowProvider and EditorProvider.
 *
 * Supports multi-select:
 * - Click: Single select
 * - Cmd/Ctrl + Click: Toggle node in selection
 * - Shift + Click: Add to selection
 * - Drag box: Multi-select
 */

import { useEffect, useRef } from 'react';
import { useOnSelectionChange } from '@xyflow/react';
import { useEditorContext } from './context';

export function NodeSelectionHandler() {
  const { setSelection, isEditMode, clearSelection } = useEditorContext();

  // Track previous selection to detect changes
  const prevSelectionRef = useRef<string[]>([]);

  useOnSelectionChange({
    onChange: ({ nodes }) => {
      // Only handle selection in edit mode
      if (!isEditMode) return;

      const selectedIds = nodes.map((n) => n.id);
      const prevIds = prevSelectionRef.current;

      // Check if selection actually changed
      const hasChanged =
        selectedIds.length !== prevIds.length ||
        selectedIds.some((id, i) => id !== prevIds[i]);

      if (!hasChanged) return;

      prevSelectionRef.current = selectedIds;

      // Sync ReactFlow selection with our context
      setSelection(selectedIds);
    },
  });

  // Clear selection when edit mode is disabled
  useEffect(() => {
    if (!isEditMode) {
      clearSelection();
      prevSelectionRef.current = [];
    }
  }, [isEditMode, clearSelection]);

  return null;
}
