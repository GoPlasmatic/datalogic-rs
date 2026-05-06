import { useState, useEffect, useMemo, useRef } from 'react';
import type {
  LogicNode,
  LogicEdge,
  JsonLogicValue,
  TracedResult,
  ExecutionStep,
} from '../types';
import { jsonLogicToNodes } from '../utils/jsonlogic-to-nodes';
import { traceToNodes, buildEvaluationResultsFromTrace } from '../utils/trace';
import { applyTreeLayout } from '../utils/layout';
import { checkDepth } from './useRecursionCheck';

export interface EvaluationResult {
  value: unknown;
  error: string | null;
  type: 'boolean' | 'number' | 'string' | 'null' | 'array' | 'object' | 'undefined';
}

export type EvaluationResultsMap = Map<string, EvaluationResult>;

interface UseLogicEditorOptions {
  value: JsonLogicValue | null;
  evaluateWithTrace?: (logic: unknown, data: unknown) => TracedResult;
  data?: unknown;
  /** Enable structure preserve mode for JSON templates with embedded JSONLogic */
  preserveStructure?: boolean;
}

interface UseLogicEditorReturn {
  nodes: LogicNode[];
  edges: LogicEdge[];
  error: string | null;
  evaluationResults: EvaluationResultsMap;
  usingTraceMode: boolean;
  steps: ExecutionStep[];
  traceNodeMap: Map<string, string>;  // Maps trace node IDs to visual node IDs
}

// Maximum recursion depth to prevent stack overflow
const MAX_RECURSION_DEPTH = 100;

const emptyResults: EvaluationResultsMap = new Map();
const emptySteps: ExecutionStep[] = [];
const emptyTraceNodeMap: Map<string, string> = new Map();

export function useLogicEditor({
  value,
  evaluateWithTrace,
  data,
  preserveStructure = false,
}: UseLogicEditorOptions): UseLogicEditorReturn {
  const [nodes, setNodes] = useState<LogicNode[]>([]);
  const [edges, setEdges] = useState<LogicEdge[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [evaluationResults, setEvaluationResults] = useState<EvaluationResultsMap>(emptyResults);
  const [usingTraceMode, setUsingTraceMode] = useState(false);
  const [steps, setSteps] = useState<ExecutionStep[]>(emptySteps);
  const [traceNodeMap, setTraceNodeMap] = useState<Map<string, string>>(emptyTraceNodeMap);
  const lastExternalValueRef = useRef<string>('');
  const lastDataRef = useRef<string>('');
  const lastHadTraceRef = useRef<boolean>(false);
  const lastPreserveStructureRef = useRef<boolean>(false);

  // Convert JSONLogic to nodes when value changes from outside
  /* eslint-disable react-hooks/set-state-in-effect -- Derived state computation from value/data props */
  useEffect(() => {
    const valueStr = JSON.stringify(value);
    const dataStr = JSON.stringify(data);
    const hasTrace = !!evaluateWithTrace;

    // Re-process if value, data, trace availability, or preserveStructure changed
    if (
      valueStr === lastExternalValueRef.current &&
      dataStr === lastDataRef.current &&
      hasTrace === lastHadTraceRef.current &&
      preserveStructure === lastPreserveStructureRef.current
    ) {
      return;
    }

    try {
      // Validate recursion depth
      if (!checkDepth(value, MAX_RECURSION_DEPTH)) {
        setError(`Expression exceeds maximum nesting depth of ${MAX_RECURSION_DEPTH}`);
        setNodes([]);
        setEdges([]);
        setEvaluationResults(emptyResults);
        setSteps(emptySteps);
        setTraceNodeMap(emptyTraceNodeMap);
        setUsingTraceMode(false);
        lastExternalValueRef.current = valueStr;
        lastDataRef.current = dataStr;
        lastHadTraceRef.current = hasTrace;
        lastPreserveStructureRef.current = preserveStructure;
        return;
      }

      // Try trace-based conversion first if available
      if (evaluateWithTrace && value) {
        try {
          const trace = evaluateWithTrace(value, data ?? {});
          const { nodes: newNodes, edges: newEdges, traceNodeMap: newTraceNodeMap } = traceToNodes(trace, { preserveStructure, originalValue: value });
          const layoutedNodes = applyTreeLayout(newNodes, newEdges);
          const traceResults = buildEvaluationResultsFromTrace(trace);
          setNodes(layoutedNodes);
          setEdges(newEdges);
          setEvaluationResults(traceResults);
          setSteps(trace.steps);
          setTraceNodeMap(newTraceNodeMap);
          setUsingTraceMode(true);
          setError(null);
          lastExternalValueRef.current = valueStr;
          lastDataRef.current = dataStr;
          lastHadTraceRef.current = hasTrace;
          lastPreserveStructureRef.current = preserveStructure;
          return;
        } catch (traceErr) {
          // Trace conversion failed, fall back to JS parsing
          console.warn('Trace conversion failed, falling back to JS:', traceErr);
        }
      }

      // Fallback to JS parsing (no built-in evaluation results)
      const { nodes: newNodes, edges: newEdges } = jsonLogicToNodes(value, { preserveStructure });
      const layoutedNodes = applyTreeLayout(newNodes, newEdges);
      setNodes(layoutedNodes);
      setEdges(newEdges);
      setEvaluationResults(emptyResults);
      setSteps(emptySteps);
      setTraceNodeMap(emptyTraceNodeMap);
      setUsingTraceMode(false);
      setError(null);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Unknown error during conversion';
      setError(errorMessage);
      setNodes([]);
      setEdges([]);
      setEvaluationResults(emptyResults);
      setSteps(emptySteps);
      setTraceNodeMap(emptyTraceNodeMap);
      setUsingTraceMode(false);
    }
    lastExternalValueRef.current = valueStr;
    lastDataRef.current = dataStr;
    lastHadTraceRef.current = hasTrace;
    lastPreserveStructureRef.current = preserveStructure;
  }, [value, data, evaluateWithTrace, preserveStructure]);
  /* eslint-enable react-hooks/set-state-in-effect */

  // Memoize return value to maintain stable identity
  return useMemo(
    () => ({
      nodes,
      edges,
      error,
      evaluationResults,
      usingTraceMode,
      steps,
      traceNodeMap,
    }),
    [nodes, edges, error, evaluationResults, usingTraceMode, steps, traceNodeMap]
  );
}
