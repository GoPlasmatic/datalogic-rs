/**
 * Keyboard Handler Component
 *
 * Handles keyboard shortcuts for the visual editor.
 * Must be placed inside EditorProvider context.
 */

import { useEffect, useCallback } from 'react';
import { useEditorContext } from './context/editor';
import { isRootNode } from './utils/node-deletion';

export function KeyboardHandler() {
  const {
    selectedNode,
    selectedNodes,
    isEditMode,
    deleteNode,
    undo,
    redo,
    canUndo,
    canRedo,
    copyNode,
    pasteNode,
    canPaste,
    selectAllNodes,
    clearSelection,
  } = useEditorContext();

  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      // Only handle shortcuts in edit mode
      if (!isEditMode) return;

      // Don't handle if user is typing in an input
      const target = e.target as HTMLElement;
      if (
        target.tagName === 'INPUT' ||
        target.tagName === 'TEXTAREA' ||
        target.isContentEditable
      ) {
        return;
      }

      const isMac = navigator.platform.toUpperCase().indexOf('MAC') >= 0;
      const ctrlOrCmd = isMac ? e.metaKey : e.ctrlKey;

      // Copy: Cmd/Ctrl + C
      if (ctrlOrCmd && e.key === 'c') {
        if (selectedNode) {
          e.preventDefault();
          copyNode();
        }
        return;
      }

      // Paste: Cmd/Ctrl + V
      if (ctrlOrCmd && e.key === 'v') {
        if (canPaste) {
          e.preventDefault();
          pasteNode();
        }
        return;
      }

      // Select All: Cmd/Ctrl + A
      if (ctrlOrCmd && e.key === 'a') {
        e.preventDefault();
        selectAllNodes();
        return;
      }

      // Undo: Cmd/Ctrl + Z
      if (ctrlOrCmd && e.key === 'z' && !e.shiftKey) {
        e.preventDefault();
        if (canUndo) {
          undo();
        }
        return;
      }

      // Redo: Cmd/Ctrl + Shift + Z or Cmd/Ctrl + Y
      if ((ctrlOrCmd && e.key === 'z' && e.shiftKey) || (ctrlOrCmd && e.key === 'y')) {
        e.preventDefault();
        if (canRedo) {
          redo();
        }
        return;
      }

      // Delete: Backspace or Delete
      if (e.key === 'Backspace' || e.key === 'Delete') {
        // Delete all selected non-root nodes
        const nodesToDelete = selectedNodes.filter((n) => !isRootNode(n));
        if (nodesToDelete.length > 0) {
          e.preventDefault();
          // Delete in reverse order to avoid issues with parent-child relationships
          nodesToDelete.forEach((n) => deleteNode(n.id));
        }
        return;
      }

      // Escape: Clear selection
      if (e.key === 'Escape') {
        e.preventDefault();
        clearSelection();
        return;
      }
    },
    [isEditMode, selectedNode, selectedNodes, deleteNode, undo, redo, canUndo, canRedo, copyNode, pasteNode, canPaste, selectAllNodes, clearSelection]
  );

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [handleKeyDown]);

  // This component doesn't render anything
  return null;
}

export default KeyboardHandler;
