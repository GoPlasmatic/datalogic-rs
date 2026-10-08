/**
 * Category Metadata
 *
 * Defines colors, icons, labels and docs pages for each operator category.
 * Used for consistent styling across the UI. The colors come from the
 * exported CATEGORY_COLORS, the single source of the category palette.
 *
 * `icon` is typed as IconName so an unregistered name is a compile error
 * instead of the generic `list` icon that <Icon> falls back to.
 */

import type { CategoryMeta, OperatorCategory } from './operators.types';
import type { IconName } from '../utils/icons';
import { CATEGORY_COLORS } from '../constants/colors';

export const categories: Record<OperatorCategory, CategoryMeta> = {
  variable: {
    name: 'variable',
    label: 'Variables',
    description: 'Access data from the context',
    color: CATEGORY_COLORS.variable,
    icon: 'database',
    docsPage: 'variable-access',
  },
  comparison: {
    name: 'comparison',
    label: 'Comparison',
    description: 'Compare values',
    color: CATEGORY_COLORS.comparison,
    icon: 'scale',
    docsPage: 'comparison',
  },
  logical: {
    name: 'logical',
    label: 'Logical',
    description: 'Boolean logic operations',
    color: CATEGORY_COLORS.logical,
    icon: 'binary',
    docsPage: 'logical',
  },
  arithmetic: {
    name: 'arithmetic',
    label: 'Arithmetic',
    description: 'Mathematical operations',
    color: CATEGORY_COLORS.arithmetic,
    icon: 'calculator',
    docsPage: 'arithmetic',
  },
  control: {
    name: 'control',
    label: 'Control Flow',
    description: 'Conditional branching',
    color: CATEGORY_COLORS.control,
    icon: 'git-branch',
    docsPage: 'control-flow',
  },
  string: {
    name: 'string',
    label: 'String',
    description: 'Text manipulation',
    color: CATEGORY_COLORS.string,
    icon: 'type',
    docsPage: 'string',
  },
  array: {
    name: 'array',
    label: 'Array',
    description: 'Array operations and iteration',
    color: CATEGORY_COLORS.array,
    icon: 'layers',
    docsPage: 'array',
  },
  object: {
    name: 'object',
    label: 'Object',
    description: 'Object take-apart: keys, values, entries',
    color: CATEGORY_COLORS.object,
    icon: 'braces',
    docsPage: 'object',
  },
  datetime: {
    name: 'datetime',
    label: 'Date & Time',
    description: 'Date and time operations',
    color: CATEGORY_COLORS.datetime,
    icon: 'clock',
    docsPage: 'datetime',
  },
  validation: {
    name: 'validation',
    label: 'Validation',
    description: 'Check for missing values',
    color: CATEGORY_COLORS.validation,
    icon: 'alert-circle',
    docsPage: 'missing',
  },
  error: {
    name: 'error',
    label: 'Error Handling',
    description: 'Handle errors gracefully',
    color: CATEGORY_COLORS.error,
    icon: 'circle-x',
    docsPage: 'error-handling',
  },
  utility: {
    name: 'utility',
    label: 'Utility',
    description: 'Miscellaneous utilities',
    color: CATEGORY_COLORS.utility,
    icon: 'cog',
    // `type` is documented on the control-flow page.
    docsPage: 'control-flow',
  },
  flagd: {
    name: 'flagd',
    label: 'Feature Flags',
    description: 'flagd targeting: fractional rollouts and semantic versions',
    color: CATEGORY_COLORS.flagd,
    icon: 'toggle-right',
    docsPage: 'flagd',
  },
  tensor: {
    name: 'tensor',
    label: 'Tensor',
    description: 'Marshalling JSON to and from typed n-dimensional buffers',
    color: CATEGORY_COLORS.tensor,
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
  return categories[name]?.color ?? CATEGORY_COLORS.utility;
}

/**
 * Get the docs page slug for a category
 */
export function getCategoryDocsPage(name: OperatorCategory): string {
  return categories[name]?.docsPage ?? 'overview';
}
