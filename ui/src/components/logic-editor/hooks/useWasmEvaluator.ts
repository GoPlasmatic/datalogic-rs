import { useEffect, useState, useCallback, useRef } from 'react';
import type { StructuredError, TracedResult } from '../types';

/** Error thrown from the WASM boundary that carries the parsed StructuredError. */
export class DataLogicEvaluationError extends Error {
  readonly structured: StructuredError;

  constructor(structured: StructuredError) {
    super(structured.message || structured.type);
    this.name = 'DataLogicEvaluationError';
    this.structured = structured;
  }
}

/**
 * Attempt to parse a WASM error message as a StructuredError JSON document.
 * The structured entry points always produce JSON; legacy string errors fall
 * back to a synthetic `Unknown` StructuredError so downstream code has a
 * uniform shape.
 */
function parseStructuredError(err: unknown, fallbackMessage: string): StructuredError {
  const raw = err instanceof Error ? err.message : typeof err === 'string' ? err : fallbackMessage;
  try {
    const parsed: unknown = JSON.parse(raw);
    if (parsed && typeof parsed === 'object' && 'type' in parsed && 'message' in parsed) {
      return parsed as StructuredError;
    }
  } catch {
    // Fall through — raw isn't JSON (e.g. legacy panic or network error).
  }
  return { type: 'Unknown', message: raw };
}

interface WasmModule {
  evaluateStructured: (logic: string, data: string, preserve_structure: boolean) => string;
  evaluateWithTraceStructured: (logic: string, data: string, preserve_structure: boolean) => string;
  CompiledRule: new (logic: string, preserve_structure: boolean) => {
    evaluateStructured: (data: string) => string;
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
            evaluateStructured: wasm.evaluateStructured,
            evaluateWithTraceStructured: wasm.evaluateWithTraceStructured,
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

    const logicStr = JSON.stringify(logic);
    const dataStr = JSON.stringify(data);
    try {
      const resultStr = moduleRef.current.evaluateStructured(logicStr, dataStr, preserveStructure);
      return JSON.parse(resultStr);
    } catch (err) {
      throw new DataLogicEvaluationError(parseStructuredError(err, 'Evaluation failed'));
    }
  }, [preserveStructure]);

  const evaluateWithTrace = useCallback((logic: unknown, data: unknown): TracedResult => {
    if (!moduleRef.current) {
      throw new Error('WASM module not initialized');
    }

    if (!moduleRef.current.evaluateWithTraceStructured) {
      throw new Error('evaluateWithTraceStructured not available in WASM module');
    }

    const logicStr = JSON.stringify(logic);
    const dataStr = JSON.stringify(data);
    try {
      const resultStr = moduleRef.current.evaluateWithTraceStructured(
        logicStr,
        dataStr,
        preserveStructure,
      );
      return JSON.parse(resultStr) as TracedResult;
    } catch (err) {
      throw new DataLogicEvaluationError(parseStructuredError(err, 'Trace evaluation failed'));
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
