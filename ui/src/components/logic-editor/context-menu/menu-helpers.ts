import type { MenuItemConfig } from './ContextMenu';
import type { LogicNode, OperatorNodeData, CellData } from '../types';
import { decisionCell, isIfOperator } from '../utils/converters/if-else-converter';
import { switchPairPartner } from '../utils/converters/switch-cells';

// Build remove items for a decision diamond (if / else-if).
// The condition (with its then value) can only be removed when an else-if
// diamond follows in the chain to take its place; the else input can always go.
export function buildIfRemoveItems(
  opData: OperatorNodeData,
  childNodes: LogicNode[],
  onRemove: (argIndex: number) => void
): MenuItemConfig[] {
  const items: MenuItemConfig[] = [];
  const whenCell = decisionCell(opData.cells, 'when');
  const elseCell = decisionCell(opData.cells, 'else');

  if (whenCell) {
    const elseChild = elseCell?.branchId
      ? childNodes.find((c) => c.id === elseCell.branchId)
      : undefined;
    const hasElseIf =
      elseChild?.data.type === 'operator' &&
      elseChild.data.label === 'elif' &&
      isIfOperator((elseChild.data as OperatorNodeData).operator);
    const condLabel = whenCell.label || '(condition)';

    items.push({
      id: `remove-pair-${whenCell.index}`,
      label: `${opData.label === 'elif' ? 'Else If' : 'If'}: ${condLabel}`,
      disabled: !hasElseIf,
      onClick: hasElseIf ? () => onRemove(whenCell.index) : undefined,
    });
  }

  if (elseCell) {
    items.push({
      id: `remove-else-${elseCell.index}`,
      label: `Else: ${elseCell.label || '(value)'}`,
      onClick: () => onRemove(elseCell.index),
    });
  }

  return items;
}

// Build remove items for a switch/match node: one entry per Case/Then pair
// and one for the Default row. The Match row cannot be removed.
export function buildSwitchRemoveItems(
  opData: OperatorNodeData,
  childNodes: LogicNode[],
  onRemove: (argIndex: number) => void
): MenuItemConfig[] {
  const items: MenuItemConfig[] = [];
  let caseNum = 1;

  for (const cell of opData.cells) {
    if (cell.rowLabel === 'Case') {
      const partner = switchPairPartner(opData.cells, cell);
      const childNode = cell.branchId ? childNodes.find((c) => c.id === cell.branchId) : undefined;
      const caseLabel = cell.label || (childNode ? getCellLabel(cell, childNode, cell.index) : '(case)');
      const thenLabel = partner?.label || '(result)';
      items.push({
        id: `remove-case-${cell.index}`,
        label: `Case ${caseNum}: ${caseLabel} then ${thenLabel}`,
        onClick: () => onRemove(cell.index),
      });
      caseNum++;
    } else if (cell.rowLabel === 'Default') {
      items.push({
        id: `remove-default-${cell.index}`,
        label: `Default: ${cell.label || '(value)'}`,
        onClick: () => onRemove(cell.index),
      });
    }
  }

  return items;
}

// Helper to get a human-readable label for a cell (inline or branch)
export function getCellLabel(cell: CellData, childNode: LogicNode | undefined, index: number): string {
  // Inline literal cells
  if (cell.type === 'inline') {
    return `Arg ${index + 1}: ${cell.label || '(empty)'}`;
  }

  // Editable cells (var path, etc.)
  if (cell.type === 'editable') {
    const value = Array.isArray(cell.value) ? cell.value.map(String).join('.') : cell.value;
    return `${cell.rowLabel || 'Arg'} ${index + 1}: ${value !== undefined && value !== '' ? String(value) : '(empty)'}`;
  }

  // Branch cells with child node
  if (childNode) {
    const data = childNode.data;
    switch (data.type) {
      case 'literal':
        return `Arg ${index + 1}: ${JSON.stringify(data.value)}`;
      case 'operator':
        return `Arg ${index + 1}: ${data.operator}(...)`;
      default:
        return `Arg ${index + 1}`;
    }
  }

  return `Arg ${index + 1}`;
}
