/**
 * Undo/Redo Toolbar Component
 *
 * Inline undo/redo buttons for the editor toolbar.
 * Only renders when there are actions to undo or redo.
 */

import { memo } from 'react';
import { Undo2, Redo2 } from 'lucide-react';
import { useEditorContext } from './context/editor';
import { Tooltip } from '../Tooltip';

export const UndoRedoToolbar = memo(function UndoRedoToolbar() {
  const { undo, redo, canUndo, canRedo } = useEditorContext();

  // Only show when there's something to undo or redo
  if (!canUndo && !canRedo) return null;

  return (
    <>
      <Tooltip label="Undo" shortcut="⌘Z">
        <button
          type="button"
          className="dl-toolbar-btn"
          onClick={undo}
          disabled={!canUndo}
        >
          <Undo2 size={15} />
        </button>
      </Tooltip>
      <Tooltip label="Redo" shortcut="⌘⇧Z">
        <button
          type="button"
          className="dl-toolbar-btn"
          onClick={redo}
          disabled={!canRedo}
        >
          <Redo2 size={15} />
        </button>
      </Tooltip>
    </>
  );
});

export default UndoRedoToolbar;
