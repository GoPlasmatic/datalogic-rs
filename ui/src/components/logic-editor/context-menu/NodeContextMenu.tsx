/**
 * NodeContextMenu Component
 *
 * Context menu for node operations:
 * - Edit Properties (focus properties panel)
 * - Add/Remove Argument (for n-ary operators)
 * - Wrap in Operator submenu
 * - Duplicate, Copy, Paste as Child
 * - Collapse/Expand
 * - Select Children
 * - Delete
 */

import { memo, useMemo } from 'react';
import { createPortal } from 'react-dom';
import {
  Edit3,
  Plus,
  Trash2,
  Copy,
  Clipboard,
  ChevronDown,
  ChevronRight,
  Layers,
  MousePointer2,
  Hash,
  Variable,
  Calculator,
} from 'lucide-react';
import { ContextMenu, type MenuItemConfig } from './ContextMenu';
import { useEditorContext } from '../context/editor';
import type { LogicNode, OperatorNodeData, VerticalCellNodeData } from '../types';
import { getOperator, getOperatorsGroupedByCategory } from '../config/operators';
import type { OperatorCategory } from '../config/operators.types';
import { isRootNode } from '../utils/node-deletion';

function capitalizeFirst(str: string): string {
  return str.charAt(0).toUpperCase() + str.slice(1);
}

export interface NodeContextMenuProps {
  /** X position (screen coordinates) */
  x: number;
  /** Y position (screen coordinates) */
  y: number;
  /** The node that was right-clicked */
  node: LogicNode;
  /** Called when menu should close */
  onClose: () => void;
  /** Called when "Edit Properties" is selected */
  onEditProperties?: () => void;
}

export const NodeContextMenu = memo(function NodeContextMenu({
  x,
  y,
  node,
  onClose,
  onEditProperties,
}: NodeContextMenuProps) {
  const {
    addArgumentToNode,
    removeArgumentFromNode,
    deleteNode,
    copyNode,
    pasteNode,
    canPaste,
    duplicateNode,
    wrapNodeInOperator,
    selectChildren,
    updateNode,
    getChildNodes,
  } = useEditorContext();

  const nodeData = node.data;
  const isRoot = isRootNode(node);

  // Determine if this node can have arguments added/removed
  const canModifyArgs = useMemo(() => {
    if (nodeData.type !== 'operator' && nodeData.type !== 'verticalCell') {
      return { canAdd: false, canRemove: false, childCount: 0, minArgs: 0, maxArgs: 0 };
    }

    const operator = nodeData.type === 'operator'
      ? (nodeData as OperatorNodeData).operator
      : (nodeData as VerticalCellNodeData).operator;

    const opConfig = getOperator(operator);
    if (!opConfig) {
      return { canAdd: false, canRemove: false, childCount: 0, minArgs: 0, maxArgs: 0 };
    }

    const { arity } = opConfig;
    const isVariableArity = arity.type === 'nary' || arity.type === 'variadic' || arity.type === 'chainable';

    if (!isVariableArity) {
      return { canAdd: false, canRemove: false, childCount: 0, minArgs: 0, maxArgs: 0 };
    }

    const childCount = nodeData.type === 'operator'
      ? (nodeData as OperatorNodeData).childIds.length
      : (nodeData as VerticalCellNodeData).cells.length;

    const minArgs = arity.min ?? 0;
    const maxArgs = arity.max ?? Infinity;

    return {
      canAdd: childCount < maxArgs,
      canRemove: childCount > minArgs,
      childCount,
      minArgs,
      maxArgs,
    };
  }, [nodeData]);

  // Get child nodes for remove submenu
  const childNodes = useMemo(() => {
    return getChildNodes(node.id);
  }, [node.id, getChildNodes]);

  // Check if node is collapsible
  const isCollapsible = useMemo(() => {
    if (nodeData.type === 'operator') {
      const opData = nodeData as OperatorNodeData;
      return opData.childIds.length >= 1 && !opData.inlineDisplay;
    }
    if (nodeData.type === 'verticalCell') {
      const vcData = nodeData as VerticalCellNodeData;
      return vcData.cells.length > 1;
    }
    return false;
  }, [nodeData]);

  const isCollapsed = useMemo(() => {
    if (nodeData.type === 'operator' || nodeData.type === 'verticalCell') {
      return (nodeData as OperatorNodeData | VerticalCellNodeData).collapsed ?? false;
    }
    return false;
  }, [nodeData]);

  // Has children for "Select Children" option
  const hasChildren = childNodes.length > 0;

  // Build menu items
  const menuItems = useMemo<MenuItemConfig[]>(() => {
    const items: MenuItemConfig[] = [];

    // Edit Properties
    items.push({
      id: 'edit-properties',
      label: 'Edit Properties',
      icon: <Edit3 size={14} />,
      onClick: onEditProperties,
    });

    items.push({ id: 'divider' } as MenuItemConfig);

    // Add Argument submenu (for n-ary operators)
    if (canModifyArgs.canAdd) {
      // Build operator submenu grouped by category
      const grouped = getOperatorsGroupedByCategory();
      const categoryOrder = [
        'arithmetic',
        'comparison',
        'logical',
        'string',
        'array',
        'control',
        'datetime',
        'validation',
        'variable',
        'utility',
        'error',
      ];

      const operatorSubmenu: MenuItemConfig[] = [];
      for (const category of categoryOrder) {
        const operators = grouped.get(category as OperatorCategory);
        if (!operators || operators.length === 0) continue;

        operatorSubmenu.push({
          id: `category-${category}`,
          label: capitalizeFirst(category),
          submenu: operators.slice(0, 10).map((op) => ({
            id: `op-${op.name}`,
            label: op.label || op.name,
            onClick: () => addArgumentToNode(node.id, 'operator', op.name),
          })),
        });
      }

      items.push({
        id: 'add-argument',
        label: 'Add Argument',
        icon: <Plus size={14} />,
        submenu: [
          {
            id: 'add-literal',
            label: 'Literal Value',
            icon: <Hash size={14} />,
            onClick: () => addArgumentToNode(node.id, 'literal'),
          },
          {
            id: 'add-variable',
            label: 'Variable',
            icon: <Variable size={14} />,
            onClick: () => addArgumentToNode(node.id, 'variable'),
          },
          { id: 'divider' } as MenuItemConfig,
          {
            id: 'add-operator',
            label: 'Operator',
            icon: <Calculator size={14} />,
            submenu: operatorSubmenu,
          },
        ],
      });
    }

    // Remove Argument submenu (for n-ary operators with children)
    if (canModifyArgs.canRemove && childNodes.length > 0) {
      items.push({
        id: 'remove-argument',
        label: 'Remove Argument',
        icon: <Trash2 size={14} />,
        submenu: childNodes.map((child, index) => ({
          id: `remove-arg-${index}`,
          label: getChildLabel(child, index),
          onClick: () => {
            removeArgumentFromNode(node.id, child.data.argIndex ?? index);
          },
        })),
      });
    }

    if (canModifyArgs.canAdd || canModifyArgs.canRemove) {
      items.push({ id: 'divider' } as MenuItemConfig);
    }

    // Wrap in Operator submenu
    items.push({
      id: 'wrap-in-operator',
      label: 'Wrap in Operator',
      icon: <Layers size={14} />,
      submenu: [
        {
          id: 'wrap-and',
          label: 'and',
          onClick: () => wrapNodeInOperator?.(node.id, 'and'),
        },
        {
          id: 'wrap-or',
          label: 'or',
          onClick: () => wrapNodeInOperator?.(node.id, 'or'),
        },
        {
          id: 'wrap-not',
          label: 'not',
          onClick: () => wrapNodeInOperator?.(node.id, '!'),
        },
        { id: 'divider' } as MenuItemConfig,
        {
          id: 'wrap-if',
          label: 'if',
          onClick: () => wrapNodeInOperator?.(node.id, 'if'),
        },
      ],
    });

    items.push({ id: 'divider' } as MenuItemConfig);

    // Duplicate
    items.push({
      id: 'duplicate',
      label: 'Duplicate',
      icon: <Copy size={14} />,
      shortcut: '\u2318D',
      onClick: () => duplicateNode?.(node.id),
    });

    // Copy
    items.push({
      id: 'copy',
      label: 'Copy',
      icon: <Copy size={14} />,
      shortcut: '\u2318C',
      onClick: () => {
        copyNode();
      },
    });

    // Paste as Child (only for operator nodes)
    if (nodeData.type === 'operator' || nodeData.type === 'verticalCell') {
      items.push({
        id: 'paste-as-child',
        label: 'Paste as Child',
        icon: <Clipboard size={14} />,
        shortcut: '\u2318V',
        disabled: !canPaste,
        onClick: () => {
          pasteNode();
        },
      });
    }

    items.push({ id: 'divider' } as MenuItemConfig);

    // Collapse/Expand (for collapsible nodes)
    if (isCollapsible) {
      items.push({
        id: 'toggle-collapse',
        label: isCollapsed ? 'Expand' : 'Collapse',
        icon: isCollapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />,
        onClick: () => {
          updateNode(node.id, { collapsed: !isCollapsed });
        },
      });
    }

    // Select Children
    if (hasChildren) {
      items.push({
        id: 'select-children',
        label: 'Select Children',
        icon: <MousePointer2 size={14} />,
        onClick: () => selectChildren?.(node.id),
      });
    }

    items.push({ id: 'divider' } as MenuItemConfig);

    // Delete (disabled for root)
    items.push({
      id: 'delete',
      label: 'Delete',
      icon: <Trash2 size={14} />,
      shortcut: '\u232B',
      danger: true,
      disabled: isRoot,
      onClick: () => {
        deleteNode(node.id);
      },
    });

    return items;
  }, [
    onEditProperties,
    canModifyArgs,
    childNodes,
    node.id,
    nodeData.type,
    isCollapsible,
    isCollapsed,
    hasChildren,
    isRoot,
    addArgumentToNode,
    removeArgumentFromNode,
    wrapNodeInOperator,
    duplicateNode,
    copyNode,
    canPaste,
    pasteNode,
    updateNode,
    selectChildren,
    deleteNode,
  ]);

  // Use a portal to render outside of ReactFlow's transformed container
  return createPortal(
    <ContextMenu x={x} y={y} items={menuItems} onClose={onClose} />,
    document.body
  );
});

// Helper to get a human-readable label for a child node
function getChildLabel(child: LogicNode, index: number): string {
  const data = child.data;

  switch (data.type) {
    case 'literal':
      return `Arg ${index + 1}: ${JSON.stringify(data.value)}`;
    case 'variable':
      return `Arg ${index + 1}: var("${data.path}")`;
    case 'operator':
      return `Arg ${index + 1}: ${data.operator}(...)`;
    case 'verticalCell':
      return `Arg ${index + 1}: ${data.operator}(...)`;
    default:
      return `Arg ${index + 1}`;
  }
}
