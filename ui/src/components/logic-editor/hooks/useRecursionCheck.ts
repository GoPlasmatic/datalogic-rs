/**
 * Recursion Check Hook
 *
 * Provides a utility to check for excessive recursion depth in JSONLogic expressions.
 * Prevents stack overflow from deeply nested or circular structures.
 */

import { useMemo } from 'react';

const DEFAULT_MAX_DEPTH = 100;

/**
 * Check if a value exceeds the maximum recursion depth.
 *
 * @param value - The value to check
 * @param maxDepth - Maximum allowed depth (default: 100)
 * @returns true if the value is within the depth limit, false otherwise
 */
export function checkDepth(value: unknown, maxDepth: number = DEFAULT_MAX_DEPTH): boolean {
  function check(v: unknown, depth: number): boolean {
    if (depth > maxDepth) {
      return false;
    }
    if (v === null || typeof v !== 'object') {
      return true;
    }
    if (Array.isArray(v)) {
      return v.every((item) => check(item, depth + 1));
    }
    return Object.values(v).every((val) => check(val, depth + 1));
  }
  return check(value, 0);
}

export interface RecursionCheckResult {
  /** Whether the value is valid (within depth limit) */
  valid: boolean;
  /** Error message if depth exceeded, null otherwise */
  error: string | null;
}

/**
 * Hook to check recursion depth of a value.
 *
 * @param value - The value to check
 * @param maxDepth - Maximum allowed depth (default: 100)
 * @returns RecursionCheckResult with valid flag and optional error
 */
export function useRecursionCheck(
  value: unknown,
  maxDepth: number = DEFAULT_MAX_DEPTH
): RecursionCheckResult {
  return useMemo(() => {
    const valid = checkDepth(value, maxDepth);
    return {
      valid,
      error: valid ? null : `Expression exceeds maximum nesting depth of ${maxDepth}`,
    };
  }, [value, maxDepth]);
}
