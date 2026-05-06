import { memo } from 'react';
import { UndoRedoToolbar } from './UndoRedoToolbar';
import { DebuggerControlsInline } from './debugger-controls';

interface EditorToolbarProps {
  isEditMode: boolean;
  hasDebugger: boolean;
  preserveStructure: boolean;
  onPreserveStructureChange?: (value: boolean) => void;
}

export const EditorToolbar = memo(function EditorToolbar({
  isEditMode,
  hasDebugger,
  preserveStructure,
  onPreserveStructureChange,
}: EditorToolbarProps) {
  return (
    <div className="logic-editor-toolbar">
      {isEditMode && <UndoRedoToolbar />}
      <div className="logic-editor-toolbar-spacer" />
      {hasDebugger && <DebuggerControlsInline />}
      <div className="logic-editor-toolbar-spacer" />
      {onPreserveStructureChange && (
        <label className="dl-preserve-structure-toggle">
          <input
            type="checkbox"
            checked={preserveStructure}
            onChange={(e) => onPreserveStructureChange(e.target.checked)}
          />
          <span>Preserve Structure</span>
        </label>
      )}
    </div>
  );
});
