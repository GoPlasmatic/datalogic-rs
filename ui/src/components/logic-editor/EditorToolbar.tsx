import { memo } from 'react';
import { UndoRedoToolbar } from './UndoRedoToolbar';
import { DebuggerControlsInline } from './debugger-controls';

interface EditorToolbarProps {
  isEditMode: boolean;
  hasDebugger: boolean;
  templating: boolean;
  onTemplatingChange?: (value: boolean) => void;
}

export const EditorToolbar = memo(function EditorToolbar({
  isEditMode,
  hasDebugger,
  templating,
  onTemplatingChange,
}: EditorToolbarProps) {
  return (
    <div className="logic-editor-toolbar">
      {isEditMode && <UndoRedoToolbar />}
      <div className="logic-editor-toolbar-spacer" />
      {hasDebugger && <DebuggerControlsInline />}
      <div className="logic-editor-toolbar-spacer" />
      {onTemplatingChange && (
        <label className="dl-templating-toggle">
          <input
            type="checkbox"
            checked={templating}
            onChange={(e) => onTemplatingChange(e.target.checked)}
          />
          <span>Templating</span>
        </label>
      )}
    </div>
  );
});
