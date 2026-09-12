/**
 * Category Metadata
 *
 * Defines colors, icons, labels and docs pages for each operator category.
 * Used for consistent styling across the UI.
 *
 * `icon` is typed as IconName so an unregistered name is a compile error
 * instead of a runtime crash inside <Icon> (which has no fallback).
 */

import type { CategoryMeta, OperatorCategory } from './operators.types';
import type { IconName } from '../utils/icons';

export const categories: Record<OperatorCategory, CategoryMeta> = {
  variable: {
    name: 'variable',
    label: 'Variables',
    description: 'Access data from the context',
    color: '#6366f1', // indigo
    icon: 'database',
    docsPage: 'variable-access',
  },
  comparison: {
    name: 'comparison',
    label: 'Comparison',
    description: 'Compare values',
    color: '#14b8a6', // teal
    icon: 'scale',
    docsPage: 'comparison',
  },
  logical: {
    name: 'logical',
    label: 'Logical',
    description: 'Boolean logic operations',
    color: '#8b5cf6', // violet
    icon: 'binary',
    docsPage: 'logical',
  },
  arithmetic: {
    name: 'arithmetic',
    label: 'Arithmetic',
    description: 'Mathematical operations',
    color: '#22c55e', // green
    icon: 'calculator',
    docsPage: 'arithmetic',
  },
  control: {
    name: 'control',
    label: 'Control Flow',
    description: 'Conditional branching',
    color: '#f59e0b', // amber
    icon: 'git-branch',
    docsPage: 'control-flow',
  },
  string: {
    name: 'string',
    label: 'String',
    description: 'Text manipulation',
    color: '#06b6d4', // cyan
    icon: 'type',
    docsPage: 'string',
  },
  array: {
    name: 'array',
    label: 'Array',
    description: 'Array operations and iteration',
    color: '#7c3aed', // deep violet
    icon: 'layers',
    docsPage: 'array',
  },
  object: {
    name: 'object',
    label: 'Object',
    description: 'Object take-apart: keys, values, entries',
    color: '#a855f7', // purple
    icon: 'braces',
    docsPage: 'object',
  },
  datetime: {
    name: 'datetime',
    label: 'Date & Time',
    description: 'Date and time operations',
    color: '#0ea5e9', // sky
    icon: 'clock',
    docsPage: 'datetime',
  },
  validation: {
    name: 'validation',
    label: 'Validation',
    description: 'Check for missing values',
    color: '#94a3b8', // slate
    icon: 'alert-circle',
    docsPage: 'missing',
  },
  error: {
    name: 'error',
    label: 'Error Handling',
    description: 'Handle errors gracefully',
    color: '#ef4444', // red
    icon: 'circle-x',
    docsPage: 'error-handling',
  },
  utility: {
    name: 'utility',
    label: 'Utility',
    description: 'Miscellaneous utilities',
    color: '#64748b', // slate-500
    icon: 'cog',
    // `type` is documented on the control-flow page.
    docsPage: 'control-flow',
  },
  flagd: {
    name: 'flagd',
    label: 'Feature Flags',
    description: 'flagd targeting: fractional rollouts and semantic versions',
    color: '#f97316', // orange
    icon: 'toggle-right',
    docsPage: 'flagd',
  },
  tensor: {
    name: 'tensor',
    label: 'Tensor',
    description: 'Marshalling JSON to and from typed n-dimensional buffers',
    color: '#db2777', // pink-600
    icon: 'layers',
    docsPage: 'tensor',
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
 * Get category icon (falls back to 'list' for unknown categories)
 */
export function getCategoryIcon(name: string): IconName {
  return categories[name as OperatorCategory]?.icon ?? 'list';
}

/**
 * Get category color
 */
export function getCategoryColor(name: OperatorCategory): string {
  return categories[name]?.color ?? '#64748b';
}

/**
 * Get the docs page slug for a category
 */
export function getCategoryDocsPage(name: OperatorCategory): string {
  return categories[name]?.docsPage ?? 'overview';
}
