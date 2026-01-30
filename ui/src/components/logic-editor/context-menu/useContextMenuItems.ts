import { useMemo } from 'react';
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
import React from 'react';
import type { MenuItemConfig } from './ContextMenu';
import type { LogicNode, OperatorNodeData } from '../types';
import { useEditorContext } from '../context/editor';
import { getOperator } from '../config/operators';
import { isRootNode } from '../utils/node-deletion';
import { buildOperatorSubmenu } from '../utils/menu-builder';
import { buildIfRemoveItems, getCellLabel } from './menu-helpers';

interface UseContextMenuItemsParams {
  node: LogicNode;
  onEditProperties?: () => void;
  onClose: () => void;
}

export function useContextMenuItems({ node, onEditProperties }: UseContextMenuItemsParams): MenuItemConfig[] {
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
    if (nodeData.type !== 'operator') {
      return { canAdd: false, canRemove: false, childCount: 0, minArgs: 0, maxArgs: 0 };
    }

    const opData = nodeData as OperatorNodeData;
    const opConfig = getOperator(opData.operator);
    if (!opConfig) {
      return { canAdd: false, canRemove: false, childCount: 0, minArgs: 0, maxArgs: 0 };
    }

    const { arity } = opConfig;
    const isVariableArity = arity.type === 'nary' || arity.type === 'variadic' ||
      arity.type === 'chainable' || arity.type === 'special' || arity.type === 'range';

    if (!isVariableArity) {
      return { canAdd: false, canRemove: false, childCount: 0, minArgs: 0, maxArgs: 0 };
    }

    const childCount = opData.cells.length;
    const minArgs = arity.min ?? 0;
    const maxArgs = arity.max ?? Infinity;

    return {
      canAdd: childCount < maxArgs,
      canRemove: childCount > minArgs,
      childCount,
      minArgs,
      maxArgs,
      addLabel: opConfig.ui?.addArgumentLabel,
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
      return opData.cells.length > 0;
    }
    return false;
  }, [nodeData]);

  const isCollapsed = useMemo(() => {
    if (nodeData.type === 'operator') {
      return (nodeData as OperatorNodeData).collapsed ?? false;
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
      icon: React.createElement(Edit3, { size: 14 }),
      onClick: onEditProperties,
    });

    items.push({ id: 'divider' } as MenuItemConfig);

    // Add Argument (for n-ary operators)
    if (canModifyArgs.canAdd) {
      const addLabel = (canModifyArgs as { addLabel?: string }).addLabel ?? 'Add Argument';
      const hasCustomAdd = !!(canModifyArgs as { addLabel?: string }).addLabel;

      if (hasCustomAdd) {
        // Operators with specific add actions (val "Add Path", var "Add Default", if "Add Else If")
        // directly add without a type-selection submenu
        items.push({
          id: 'add-argument',
          label: addLabel,
          icon: React.createElement(Plus, { size: 14 }),
          onClick: () => addArgumentToNode(node.id, 'literal'),
        });
      } else {
        // Generic operators show a type-selection submenu
        const operatorSubmenu = buildOperatorSubmenu(
          (opName) => addArgumentToNode(node.id, 'operator', opName)
        );

        items.push({
          id: 'add-argument',
          label: addLabel,
          icon: React.createElement(Plus, { size: 14 }),
          submenu: [
            {
              id: 'add-literal',
              label: 'Literal Value',
              icon: React.createElement(Hash, { size: 14 }),
              onClick: () => addArgumentToNode(node.id, 'literal'),
            },
            {
              id: 'add-variable',
              label: 'Variable',
              icon: React.createElement(Variable, { size: 14 }),
              onClick: () => addArgumentToNode(node.id, 'variable'),
            },
            { id: 'divider' } as MenuItemConfig,
            {
              id: 'add-operator',
              label: 'Operator',
              icon: React.createElement(Calculator, { size: 14 }),
              submenu: operatorSubmenu,
            },
          ],
        });
      }
    }

    // Remove Argument submenu (for operators with removable args)
    if (canModifyArgs.canRemove && canModifyArgs.childCount > 0) {
      const opData = nodeData as OperatorNodeData;
      const isIfOp = opData.operator === 'if' || opData.operator === '?:';

      let removeItems: MenuItemConfig[];

      if (isIfOp) {
        // For if/then: group condition+then as pairs
        removeItems = buildIfRemoveItems(opData, childNodes, (argIndex) => {
          removeArgumentFromNode(node.id, argIndex);
        });
      } else {
        removeItems = opData.cells.map((cell) => {
          const childNode = cell.branchId ? childNodes.find(c => c.id === cell.branchId) : undefined;
          return {
            id: `remove-arg-${cell.index}`,
            label: getCellLabel(cell, childNode, cell.index),
            onClick: () => {
              removeArgumentFromNode(node.id, cell.index);
            },
          };
        });
      }

      items.push({
        id: 'remove-argument',
        label: isIfOp ? 'Remove Branch' : 'Remove Argument',
        icon: React.createElement(Trash2, { size: 14 }),
        submenu: removeItems,
      });
    }

    if (canModifyArgs.canAdd || canModifyArgs.canRemove) {
      items.push({ id: 'divider' } as MenuItemConfig);
    }

    // Wrap in Operator submenu
    items.push({
      id: 'wrap-in-operator',
      label: 'Wrap in Operator',
      icon: React.createElement(Layers, { size: 14 }),
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
      icon: React.createElement(Copy, { size: 14 }),
      shortcut: '\u2318D',
      onClick: () => duplicateNode?.(node.id),
    });

    // Copy
    items.push({
      id: 'copy',
      label: 'Copy',
      icon: React.createElement(Copy, { size: 14 }),
      shortcut: '\u2318C',
      onClick: () => {
        copyNode();
      },
    });

    // Paste as Child (only for operator nodes)
    if (nodeData.type === 'operator') {
      items.push({
        id: 'paste-as-child',
        label: 'Paste as Child',
        icon: React.createElement(Clipboard, { size: 14 }),
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
        icon: isCollapsed ? React.createElement(ChevronRight, { size: 14 }) : React.createElement(ChevronDown, { size: 14 }),
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
        icon: React.createElement(MousePointer2, { size: 14 }),
        onClick: () => selectChildren?.(node.id),
      });
    }

    items.push({ id: 'divider' } as MenuItemConfig);

    // Delete (disabled for root)
    items.push({
      id: 'delete',
      label: 'Delete',
      icon: React.createElement(Trash2, { size: 14 }),
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
    nodeData,
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

  return menuItems;
}
