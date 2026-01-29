import type { NodeCategory } from '../types';

// Colors for branch edges and labels
export const BRANCH_COLORS = {
  yes: '#22C55E',
  no: '#EF4444',
} as const;

// Color mappings for operator categories (includes 'literal' for node styling)
export const CATEGORY_COLORS: Record<NodeCategory, string> = {
  variable: '#3B82F6',
  comparison: '#8B5CF6',
  logical: '#F97316',
  arithmetic: '#22C55E',
  string: '#14B8A6',
  array: '#6366F1',
  control: '#EF4444',
  datetime: '#F59E0B',
  validation: '#F59E0B',
  utility: '#6B7280',
  error: '#F43F5E',
  literal: '#6B7280',
};
