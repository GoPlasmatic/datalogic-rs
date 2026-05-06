import { createContext, useContext } from 'react';
import type { EvaluationResultsMap } from '../hooks/useLogicEditor';

export const EvaluationContext = createContext<EvaluationResultsMap>(new Map());

export function useEvaluationResult(nodeId: string) {
  const results = useContext(EvaluationContext);
  return results.get(nodeId);
}

export function useEvaluationResults() {
  return useContext(EvaluationContext);
}
