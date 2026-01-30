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
    color: '#5C7CFA', // blue
    icon: 'database',
  },
  comparison: {
    name: 'comparison',
    label: 'Comparison',
    description: 'Compare values',
    color: '#20C997', // teal
    icon: 'scale',
  },
  logical: {
    name: 'logical',
    label: 'Logical',
    description: 'Boolean logic operations',
    color: '#845EF7', // violet
    icon: 'binary',
  },
  arithmetic: {
    name: 'arithmetic',
    label: 'Arithmetic',
    description: 'Mathematical operations',
    color: '#40C057', // green
    icon: 'calculator',
  },
  control: {
    name: 'control',
    label: 'Control Flow',
    description: 'Conditional branching',
    color: '#F59F00', // amber
    icon: 'git-branch',
  },
  string: {
    name: 'string',
    label: 'String',
    description: 'Text manipulation',
    color: '#15AABF', // cyan
    icon: 'type',
  },
  array: {
    name: 'array',
    label: 'Array',
    description: 'Array operations and iteration',
    color: '#5F3DC4', // indigo
    icon: 'layers',
  },
  datetime: {
    name: 'datetime',
    label: 'Date & Time',
    description: 'Date and time operations',
    color: '#748FFC', // slate blue
    icon: 'clock',
  },
  validation: {
    name: 'validation',
    label: 'Validation',
    description: 'Check for missing values',
    color: '#ADB5BD', // grey-blue
    icon: 'alert-circle',
  },
  error: {
    name: 'error',
    label: 'Error Handling',
    description: 'Handle errors gracefully',
    color: '#FA5252', // red
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
 * Get category color
 */
export function getCategoryColor(name: OperatorCategory): string {
  return categories[name]?.color ?? '#64748b';
}
