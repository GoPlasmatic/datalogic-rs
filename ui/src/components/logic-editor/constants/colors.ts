import type { NodeCategory } from '../types';

// Colors for branch edges and labels
export const BRANCH_COLORS = {
  yes: '#22C55E',
  no: '#EF4444',
} as const;

// Color mappings for operator categories (includes 'literal' for node styling)
export const CATEGORY_COLORS: Record<NodeCategory, string> = {
  variable: '#5C7CFA',
  comparison: '#20C997',
  logical: '#845EF7',
  arithmetic: '#40C057',
  string: '#15AABF',
  array: '#5F3DC4',
  control: '#F59F00',
  datetime: '#748FFC',
  validation: '#ADB5BD',
  utility: '#6B7280',
  error: '#FA5252',
  literal: '#6B7280',
};
