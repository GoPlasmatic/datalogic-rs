/**
 * Category Metadata
 *
 * Defines colors, icons, and labels for each operator category.
 * Used for consistent styling across the UI.
 */

import type { CategoryMeta, OperatorCategory } from './operators.types';

export const categories: Record<OperatorCategory, CategoryMeta> = {
  variable: {
    name: 'variable',
    label: 'Variables',
    description: 'Access data from the context',
    color: '#6366f1', // indigo
    icon: 'database',
  },
  comparison: {
    name: 'comparison',
    label: 'Comparison',
    description: 'Compare values',
    color: '#14b8a6', // teal
    icon: 'scale',
  },
  logical: {
    name: 'logical',
    label: 'Logical',
    description: 'Boolean logic operations',
    color: '#8b5cf6', // violet
    icon: 'binary',
  },
  arithmetic: {
    name: 'arithmetic',
    label: 'Arithmetic',
    description: 'Mathematical operations',
    color: '#22c55e', // green
    icon: 'calculator',
  },
  control: {
    name: 'control',
    label: 'Control Flow',
    description: 'Conditional branching',
    color: '#f59e0b', // amber
    icon: 'git-branch',
  },
  string: {
    name: 'string',
    label: 'String',
    description: 'Text manipulation',
    color: '#06b6d4', // cyan
    icon: 'type',
  },
  array: {
    name: 'array',
    label: 'Array',
    description: 'Array operations and iteration',
    color: '#7c3aed', // deep violet
    icon: 'layers',
  },
  datetime: {
    name: 'datetime',
    label: 'Date & Time',
    description: 'Date and time operations',
    color: '#0ea5e9', // sky
    icon: 'clock',
  },
  validation: {
    name: 'validation',
    label: 'Validation',
    description: 'Check for missing values',
    color: '#94a3b8', // slate
    icon: 'alert-circle',
  },
  error: {
    name: 'error',
    label: 'Error Handling',
    description: 'Handle errors gracefully',
    color: '#ef4444', // red
    icon: 'circle-x',
  },
  utility: {
    name: 'utility',
    label: 'Utility',
    description: 'Miscellaneous utilities',
    color: '#64748b', // slate-500
    icon: 'wrench',
  },
};

/**
 * Get category metadata by name
 */
export function getCategory(name: OperatorCategory): CategoryMeta {
  return categories[name];
}

/**
 * Get all categories as an array
 */
export function getAllCategories(): CategoryMeta[] {
  return Object.values(categories);
}

/**
 * Get category icon
 */
export function getCategoryIcon(name: string): string {
  return categories[name as OperatorCategory]?.icon ?? 'list';
}

/**
 * Get category color
 */
export function getCategoryColor(name: OperatorCategory): string {
  return categories[name]?.color ?? '#64748b';
}
