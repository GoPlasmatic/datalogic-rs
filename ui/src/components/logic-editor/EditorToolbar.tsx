import { memo } from 'react';
import { UndoRedoToolbar } from './UndoRedoToolbar';
import { DebuggerControlsInline } from './debugger-controls';
import { Tooltip } from '../Tooltip';
import type { FlowDirection } from './context';

interface EditorToolbarProps {
  isEditMode: boolean;
  hasDebugger: boolean;
  templating: boolean;
  onTemplatingChange?: (value: boolean) => void;
  direction: FlowDirection;
  onDirectionChange?: (value: FlowDirection) => void;
}

export const EditorToolbar = memo(function EditorToolbar({
  isEditMode,
  hasDebugger,
  templating,
  onTemplatingChange,
  direction,
  onDirectionChange,
}: EditorToolbarProps) {
  return (
    <div className="logic-editor-toolbar">
      {isEditMode && <UndoRedoToolbar />}
      <div className="logic-editor-toolbar-spacer" />
      {hasDebugger && <DebuggerControlsInline />}
      <div className="logic-editor-toolbar-spacer" />
      {onDirectionChange && (
        <Tooltip
          label="Diagram direction — Data flow (result on the right) or Hierarchy (root on the left, JSON nesting order)"
          side="bottom"
        >
          <div
            className="dl-direction-toggle"
            role="group"
            aria-label="Diagram direction"
          >
            <button
              type="button"
              className={direction === 'flow' ? 'active' : ''}
              aria-pressed={direction === 'flow'}
              onClick={() => onDirectionChange('flow')}
            >
              Flow
            </button>
            <button
              type="button"
              className={direction === 'hierarchy' ? 'active' : ''}
              aria-pressed={direction === 'hierarchy'}
              onClick={() => onDirectionChange('hierarchy')}
            >
              Hierarchy
            </button>
          </div>
        </Tooltip>
      )}
      {onTemplatingChange && (
        <Tooltip
          label="Compile multi-key objects as output templates with embedded JSONLogic"
          side="bottom"
        >
          <label className="dl-templating-toggle">
            <input
              type="checkbox"
              checked={templating}
              onChange={(e) => onTemplatingChange(e.target.checked)}
            />
            <span>Templating</span>
          </label>
        </Tooltip>
      )}
    </div>
  );
});
