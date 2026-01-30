/**
 * Undo/Redo Toolbar Component
 *
 * Inline undo/redo buttons for the editor toolbar.
 * Only renders when there are actions to undo or redo.
 */

import { memo } from 'react';
import { Undo2, Redo2 } from 'lucide-react';
import { useEditorContext } from './context/editor';

export const UndoRedoToolbar = memo(function UndoRedoToolbar() {
  const { undo, redo, canUndo, canRedo } = useEditorContext();

  // Only show when there's something to undo or redo
  if (!canUndo && !canRedo) return null;

  return (
    <>
      <button
        type="button"
        className="toolbar-btn"
        onClick={undo}
        disabled={!canUndo}
        title="Undo (Cmd/Ctrl+Z)"
      >
        <Undo2 size={15} />
      </button>
      <button
        type="button"
        className="toolbar-btn"
        onClick={redo}
        disabled={!canRedo}
        title="Redo (Cmd/Ctrl+Shift+Z)"
      >
        <Redo2 size={15} />
      </button>
    </>
  );
});

export default UndoRedoToolbar;
