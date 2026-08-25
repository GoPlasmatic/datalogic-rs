import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Plus, Settings2, Variable, Hash, Calculator, GitBranch, Layers } from 'lucide-react';
import { UndoRedoToolbar } from './UndoRedoToolbar';
import { DebuggerControlsInline } from './debugger-controls';
import { Tooltip } from '../Tooltip';
import type { FlowDirection } from './context';
import { useEditorContext } from './context/editor';
import { ContextMenu, type MenuItemConfig } from './context-menu/ContextMenu';
import { buildOperatorSubmenu } from './utils/menu-builder';
import { getOperator } from './config/operators';
import type { OperatorNodeData } from './types';

interface EditorToolbarProps {
  isEditMode: boolean;
  hasDebugger: boolean;
  templating: boolean;
  onTemplatingChange?: (value: boolean) => void;
  direction: FlowDirection;
  onDirectionChange?: (value: FlowDirection) => void;
  /**
   * One-line summary of the non-default engine settings in effect
   * (see `summarizeEvaluationConfig`). Renders a compact indicator when set.
   */
  configSummary?: string | null;
  /** Makes the engine-settings indicator clickable. */
  onOpenEngineSettings?: () => void;
}

/** Operators whose arguments are managed by a dedicated panel action, not a generic picker. */
function canReceiveArgument(data: OperatorNodeData): boolean {
  const opConfig = getOperator(data.operator);
  if (!opConfig || opConfig.ui?.addArgumentLabel) return false;
  const { arity } = opConfig;
  const variable =
    arity.type === 'nary' || arity.type === 'variadic' || arity.type === 'chainable' ||
    arity.type === 'special' || arity.type === 'range';
  if (!variable) return false;
  return data.cells.length < (arity.max ?? Infinity);
}

/**
 * Toolbar "Insert" entry point (also Cmd/Ctrl+K): opens the shared operator
 * menu. With an operator node selected it adds an argument to that node (or
 * wraps it); with nothing selected it targets the root, which is what the
 * canvas context menu does. Keyboard-navigable via ContextMenu.
 */
const InsertMenuButton = memo(function InsertMenuButton() {
  const {
    selectedNode,
    createNode,
    addArgumentToNode,
    wrapNodeInOperator,
    hasNodes,
  } = useEditorContext();
  const buttonRef = useRef<HTMLButtonElement>(null);
  const [menuPosition, setMenuPosition] = useState<{ x: number; y: number } | null>(null);

  const openMenu = useCallback(() => {
    const rect = buttonRef.current?.getBoundingClientRect();
    setMenuPosition(rect ? { x: rect.left, y: rect.bottom + 4 } : { x: 16, y: 56 });
  }, []);

  const closeMenu = useCallback(() => {
    setMenuPosition(null);
    buttonRef.current?.focus();
  }, []);

  // Cmd/Ctrl+K toggles the menu unless the user is typing.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const isMac = navigator.platform.toUpperCase().includes('MAC');
      const modifier = isMac ? e.metaKey : e.ctrlKey;
      if (!modifier || e.key.toLowerCase() !== 'k') return;
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable)) {
        return;
      }
      e.preventDefault();
      setMenuPosition((current) => {
        if (current) return null;
        const rect = buttonRef.current?.getBoundingClientRect();
        return rect ? { x: rect.left, y: rect.bottom + 4 } : { x: 16, y: 56 };
      });
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  const menuItems = useMemo<MenuItemConfig[]>(() => {
    const items: MenuItemConfig[] = [];
    const selectedOperator =
      selectedNode?.data.type === 'operator' ? (selectedNode.data as OperatorNodeData) : null;

    if (selectedNode && selectedOperator && canReceiveArgument(selectedOperator)) {
      const label = selectedOperator.label || selectedOperator.operator;
      items.push({
        id: 'insert-target',
        label: `Add argument to ${label}`,
        disabled: true,
      });
      items.push({
        id: 'add-literal',
        label: 'Literal Value',
        icon: <Hash size={14} />,
        onClick: () => addArgumentToNode(selectedNode.id, 'literal'),
      });
      items.push({
        id: 'add-variable',
        label: 'Variable',
        icon: <Variable size={14} />,
        onClick: () => addArgumentToNode(selectedNode.id, 'variable'),
      });
      items.push({
        id: 'add-operator',
        label: 'Operator',
        icon: <Calculator size={14} />,
        submenu: buildOperatorSubmenu((opName) => addArgumentToNode(selectedNode.id, 'operator', opName)),
      });
      items.push({ id: 'divider' } as MenuItemConfig);
    }

    if (selectedNode) {
      items.push({
        id: 'wrap-in-operator',
        label: 'Wrap selection in operator',
        icon: <Layers size={14} />,
        submenu: buildOperatorSubmenu((opName) => wrapNodeInOperator(selectedNode.id, opName)),
      });
      items.push({ id: 'divider' } as MenuItemConfig);
    }

    // createNode wraps the existing root for operators/conditions, but a
    // variable or literal root replaces the canvas (same as the canvas menu).
    const canvasHasNodes = hasNodes();
    items.push({
      id: 'root-target',
      label: canvasHasNodes ? 'Root' : 'Empty canvas',
      disabled: true,
    });
    items.push({
      id: 'root-operator',
      label: canvasHasNodes ? 'Wrap root in Operator' : 'Add Operator',
      icon: <Calculator size={14} />,
      submenu: buildOperatorSubmenu((opName) => createNode('operator', opName)),
    });
    items.push({
      id: 'root-condition',
      label: canvasHasNodes ? 'Wrap root in Condition' : 'Add Condition',
      icon: <GitBranch size={14} />,
      onClick: () => createNode('condition'),
    });
    items.push({
      id: 'root-variable',
      label: canvasHasNodes ? 'Replace canvas with Variable' : 'Add Variable',
      icon: <Variable size={14} />,
      onClick: () => createNode('variable'),
    });
    items.push({
      id: 'root-literal',
      label: canvasHasNodes ? 'Replace canvas with Literal' : 'Add Literal',
      icon: <Hash size={14} />,
      onClick: () => createNode('literal'),
    });
    return items;
  }, [selectedNode, createNode, addArgumentToNode, wrapNodeInOperator, hasNodes]);

  return (
    <>
      <Tooltip label="Insert node (add argument to the selection, or wrap the root)" shortcut="⌘K">
        <button
          ref={buttonRef}
          type="button"
          className="dl-toolbar-btn dl-toolbar-insert"
          onClick={openMenu}
          aria-haspopup="menu"
          aria-expanded={menuPosition !== null}
          aria-label="Insert node"
        >
          <Plus size={15} />
          <span>Insert</span>
        </button>
      </Tooltip>
      {menuPosition &&
        typeof document !== 'undefined' &&
        createPortal(
          <ContextMenu x={menuPosition.x} y={menuPosition.y} items={menuItems} onClose={closeMenu} />,
          document.body,
        )}
    </>
  );
});

export const EditorToolbar = memo(function EditorToolbar({
  isEditMode,
  hasDebugger,
  templating,
  onTemplatingChange,
  direction,
  onDirectionChange,
  configSummary,
  onOpenEngineSettings,
}: EditorToolbarProps) {
  return (
    <div className="logic-editor-toolbar">
      {isEditMode && <InsertMenuButton />}
      {isEditMode && <UndoRedoToolbar />}
      <div className="logic-editor-toolbar-spacer" />
      {hasDebugger && <DebuggerControlsInline />}
      <div className="logic-editor-toolbar-spacer" />
      {configSummary && (
        <Tooltip label={`Engine settings: ${configSummary}`} side="bottom">
          {onOpenEngineSettings ? (
            <button
              type="button"
              className="engine-config-badge engine-config-badge--button dl-toolbar-config"
              onClick={onOpenEngineSettings}
              aria-label={`Engine settings: ${configSummary}`}
            >
              <Settings2 size={11} />
              <span className="engine-config-badge-text">{configSummary}</span>
            </button>
          ) : (
            <span className="engine-config-badge dl-toolbar-config" aria-label={`Engine settings: ${configSummary}`}>
              <Settings2 size={11} />
              <span className="engine-config-badge-text">{configSummary}</span>
            </span>
          )}
        </Tooltip>
      )}
      {onDirectionChange && (
        <Tooltip
          label="Diagram direction: Data flow (result on the right) or Hierarchy (root on the left, JSON nesting order)"
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
