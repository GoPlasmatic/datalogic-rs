import { useMemo } from 'react';
import type { LogicNode } from '../types';

export interface EvaluationResult {
  value: unknown;
  error: string | null;
  type: 'boolean' | 'number' | 'string' | 'null' | 'array' | 'object' | 'undefined';
}

export type EvaluationResults = Map<string, EvaluationResult>;

interface UseDebugEvaluationProps {
  nodes: LogicNode[];
  data: unknown;
  evaluate: ((logic: unknown, data: unknown) => unknown) | null;
  enabled: boolean;
}

function getValueType(value: unknown): EvaluationResult['type'] {
  if (value === null) return 'null';
  if (value === undefined) return 'undefined';
  if (Array.isArray(value)) return 'array';
  if (typeof value === 'boolean') return 'boolean';
  if (typeof value === 'number') return 'number';
  if (typeof value === 'string') return 'string';
  if (typeof value === 'object') return 'object';
  return 'undefined';
}

export function useDebugEvaluation({
  nodes,
  data,
  evaluate,
  enabled,
}: UseDebugEvaluationProps): EvaluationResults {
  return useMemo(() => {
    const results = new Map<string, EvaluationResult>();

    if (!enabled || !evaluate || !data) {
      return results;
    }

    // Evaluate each node's expression
    for (const node of nodes) {
      const expression = node.data.expression;

      if (expression === undefined) {
        continue;
      }

      try {
        const value = evaluate(expression, data);
        results.set(node.id, {
          value,
          error: null,
          type: getValueType(value),
        });
      } catch (err) {
        results.set(node.id, {
          value: null,
          error: err instanceof Error ? err.message : String(err),
          type: 'undefined',
        });
      }
    }

    return results;
  }, [nodes, data, evaluate, enabled]);
}
