import type { MenuItemConfig } from './ContextMenu';
import type { LogicNode, OperatorNodeData, CellData } from '../types';

// Build remove items for if/then operator — groups condition+then as pairs
export function buildIfRemoveItems(
  opData: OperatorNodeData,
  _childNodes: LogicNode[],
  onRemove: (argIndex: number) => void
): MenuItemConfig[] {
  const items: MenuItemConfig[] = [];
  const cells = opData.cells;
  let i = 0;
  let pairNum = 1;

  while (i < cells.length) {
    const cell = cells[i];

    if (cell.rowLabel === 'If' || cell.rowLabel === 'Else If') {
      // This is a condition cell — pair it with the next Then cell
      const condLabel = cell.label || '(condition)';
      const isFirst = cell.rowLabel === 'If';
      const label = isFirst
        ? `If: ${condLabel}`
        : `Else If ${pairNum}: ${condLabel}`;

      // Only allow removing if it's not the last condition-then pair
      const conditionCount = cells.filter(
        (c) => c.rowLabel === 'If' || c.rowLabel === 'Else If'
      ).length;
      const canRemoveThis = conditionCount > 1;

      items.push({
        id: `remove-pair-${cell.index}`,
        label,
        disabled: !canRemoveThis,
        onClick: canRemoveThis ? () => onRemove(cell.index) : undefined,
      });

      pairNum++;
      i += 2; // Skip the Then cell
      continue;
    }

    if (cell.rowLabel === 'Else') {
      items.push({
        id: `remove-else-${cell.index}`,
        label: `Else: ${cell.label || '(value)'}`,
        onClick: () => onRemove(cell.index),
      });
      i++;
      continue;
    }

    // Fallback for unexpected cells
    i++;
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
    return `${cell.rowLabel || 'Arg'} ${index + 1}: ${cell.value !== undefined ? String(cell.value) : '(empty)'}`;
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
