/**
 * Undo/Redo Toolbar Component
 *
 * A floating toolbar that shows undo/redo buttons in edit mode.
 */

import { memo } from 'react';
import { Undo2, Redo2 } from 'lucide-react';
import { useEditorContext } from './context/editor';

export const UndoRedoToolbar = memo(function UndoRedoToolbar() {
  const { undo, redo, canUndo, canRedo } = useEditorContext();

  return (
    <div className="undo-redo-toolbar">
      <button
        type="button"
        className="undo-redo-btn"
        onClick={undo}
        disabled={!canUndo}
        title="Undo (Cmd/Ctrl+Z)"
      >
        <Undo2 size={16} />
      </button>
      <button
        type="button"
        className="undo-redo-btn"
        onClick={redo}
        disabled={!canRedo}
        title="Redo (Cmd/Ctrl+Shift+Z)"
      >
        <Redo2 size={16} />
      </button>
    </div>
  );
});

export default UndoRedoToolbar;
