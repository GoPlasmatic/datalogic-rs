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
    color: '#8b5cf6', // violet-500
    icon: 'variable',
  },
  comparison: {
    name: 'comparison',
    label: 'Comparison',
    description: 'Compare values',
    color: '#f59e0b', // amber-500
    icon: 'scale',
  },
  logical: {
    name: 'logical',
    label: 'Logical',
    description: 'Boolean logic operations',
    color: '#ec4899', // pink-500
    icon: 'git-branch',
  },
  arithmetic: {
    name: 'arithmetic',
    label: 'Arithmetic',
    description: 'Mathematical operations',
    color: '#3b82f6', // blue-500
    icon: 'calculator',
  },
  control: {
    name: 'control',
    label: 'Control Flow',
    description: 'Conditional branching',
    color: '#10b981', // emerald-500
    icon: 'git-fork',
  },
  string: {
    name: 'string',
    label: 'String',
    description: 'Text manipulation',
    color: '#06b6d4', // cyan-500
    icon: 'text',
  },
  array: {
    name: 'array',
    label: 'Array',
    description: 'Array operations and iteration',
    color: '#6366f1', // indigo-500
    icon: 'list',
  },
  datetime: {
    name: 'datetime',
    label: 'Date & Time',
    description: 'Date and time operations',
    color: '#f97316', // orange-500
    icon: 'calendar',
  },
  validation: {
    name: 'validation',
    label: 'Validation',
    description: 'Check for missing values',
    color: '#eab308', // yellow-500
    icon: 'shield-check',
  },
  error: {
    name: 'error',
    label: 'Error Handling',
    description: 'Handle errors gracefully',
    color: '#ef4444', // red-500
    icon: 'alert-triangle',
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
