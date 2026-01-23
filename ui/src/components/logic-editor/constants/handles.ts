// Centralized handle ID generation for consistent edge connections

// Handle ID generators
export const HANDLE_IDS = {
  // Argument handles (for operator children)
  arg: (index: number): string => `arg-${index}`,

  // Branch handles (for vertical cell branches)
  branch: (index: number): string => `branch-${index}`,

  // Fixed handle IDs for specific purposes
  top: 'top',
  left: 'left',
  right: 'right',
  bottom: 'bottom',

  // Conditional handles
  condition: 'condition',
  thenBranch: 'then',
  elseBranch: 'else',
} as const;

// Handle positions for vertical cell rows (in pixels from top of 32px row)
export const HANDLE_POSITIONS = {
  // Condition branch - positioned at 30% of row height
  conditionTop: 10,
  // Then/Yes branch - positioned at 70% of row height
  thenTop: 22,
  // Standard single branch - centered vertically
  centeredTop: 16,
} as const;

// Edge ID generators for consistent edge naming
export const EDGE_IDS = {
  // Standard parent-child edge
  parentChild: (parentId: string, childId: string): string =>
    `${parentId}-${childId}`,

  // Branch edge
  branch: (parentId: string, branchId: string): string =>
    `${parentId}-branch-${branchId}`,

  // Condition edge
  condition: (parentId: string, conditionId: string): string =>
    `${parentId}-cond-${conditionId}`,

  // Then edge
  then: (parentId: string, thenId: string): string =>
    `${parentId}-then-${thenId}`,

  // Else edge
  else: (parentId: string, elseId: string): string =>
    `${parentId}-else-${elseId}`,
} as const;
