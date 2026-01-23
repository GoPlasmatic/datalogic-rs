import type { OperatorCategory } from '../types';

// Colors for branch edges and labels
export const BRANCH_COLORS = {
  yes: '#22C55E',
  no: '#EF4444',
} as const;

// Color mappings for operator categories
export const CATEGORY_COLORS: Record<OperatorCategory, string> = {
  variable: '#3B82F6',
  comparison: '#8B5CF6',
  logical: '#F97316',
  arithmetic: '#22C55E',
  string: '#14B8A6',
  array: '#6366F1',
  control: '#EF4444',
  datetime: '#F59E0B',
  error: '#F43F5E',
  literal: '#6B7280',
};
