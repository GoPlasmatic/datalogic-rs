import { useEffect, useState, useCallback, useRef } from 'react';
import type { TracedResult } from '../types';

/** Extract error message from unknown error types (Error objects, strings from WASM, etc.) */
function extractErrorMessage(err: unknown, fallback: string): string {
  if (err instanceof Error) return err.message;
  if (typeof err === 'string') return err;
  return fallback;
}

interface WasmModule {
  evaluate: (logic: string, data: string, preserve_structure: boolean) => string;
  evaluate_with_trace: (logic: string, data: string, preserve_structure: boolean) => string;
  CompiledRule: new (logic: string, preserve_structure: boolean) => {
    evaluate: (data: string) => string;
    free: () => void;
  };
}

interface UseWasmEvaluatorOptions {
  /** Enable structure preserve mode for JSON templates with embedded JSONLogic */
  preserveStructure?: boolean;
}

interface UseWasmEvaluatorResult {
  ready: boolean;
  loading: boolean;
  error: string | null;
  evaluate: (logic: unknown, data: unknown) => unknown;
  evaluateWithTrace: (logic: unknown, data: unknown) => TracedResult;
}

export function useWasmEvaluator(options: UseWasmEvaluatorOptions = {}): UseWasmEvaluatorResult {
  const { preserveStructure = false } = options;
  const [ready, setReady] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const moduleRef = useRef<WasmModule | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function initWasm() {
      try {
        setLoading(true);
        setError(null);

        // Dynamic import of the WASM module
        const wasm = await import('@goplasmatic/datalogic');

        // Initialize the WASM module
        await wasm.default();

        if (!cancelled) {
          moduleRef.current = {
            evaluate: wasm.evaluate,
            evaluate_with_trace: wasm.evaluate_with_trace,
            CompiledRule: wasm.CompiledRule,
          };
          setReady(true);
          setLoading(false);
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'Failed to load WASM module');
          setLoading(false);
        }
      }
    }

    initWasm();

    return () => {
      cancelled = true;
    };
  }, []);

  const evaluate = useCallback((logic: unknown, data: unknown): unknown => {
    if (!moduleRef.current) {
      throw new Error('WASM module not initialized');
    }

    try {
      const logicStr = JSON.stringify(logic);
      const dataStr = JSON.stringify(data);
      const resultStr = moduleRef.current.evaluate(logicStr, dataStr, preserveStructure);
      return JSON.parse(resultStr);
    } catch (err) {
      throw new Error(extractErrorMessage(err, 'Evaluation failed'));
    }
  }, [preserveStructure]);

  const evaluateWithTrace = useCallback((logic: unknown, data: unknown): TracedResult => {
    if (!moduleRef.current) {
      throw new Error('WASM module not initialized');
    }

    if (!moduleRef.current.evaluate_with_trace) {
      throw new Error('evaluate_with_trace not available in WASM module');
    }

    try {
      const logicStr = JSON.stringify(logic);
      const dataStr = JSON.stringify(data);
      const resultStr = moduleRef.current.evaluate_with_trace(logicStr, dataStr, preserveStructure);
      return JSON.parse(resultStr) as TracedResult;
    } catch (err) {
      throw new Error(extractErrorMessage(err, 'Trace evaluation failed'));
    }
  }, [preserveStructure]);

  return {
    ready,
    loading,
    error,
    evaluate,
    evaluateWithTrace,
  };
}
