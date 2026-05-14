import type { NodeCategory } from '../types';

// Colors for branch edges and labels
export const BRANCH_COLORS = {
  yes: '#22C55E',
  no: '#EF4444',
} as const;

// Color mappings for operator categories (includes 'literal' for node styling)
export const CATEGORY_COLORS: Record<NodeCategory, string> = {
  variable: '#6366f1',
  comparison: '#14b8a6',
  logical: '#8b5cf6',
  arithmetic: '#22c55e',
  string: '#06b6d4',
  array: '#7c3aed',
  control: '#f59e0b',
  datetime: '#0ea5e9',
  validation: '#94a3b8',
  utility: '#64748b',
  error: '#ef4444',
  literal: '#64748b',
};
