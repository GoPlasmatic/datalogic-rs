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
 * The WASM boundary always produces JSON; non-JSON errors fall back to a
 * synthetic `Unknown` StructuredError so downstream code has a uniform shape.
 */
function parseStructuredError(err: unknown, fallbackMessage: string): StructuredError {
  const raw = err instanceof Error ? err.message : typeof err === 'string' ? err : fallbackMessage;
  try {
    const parsed: unknown = JSON.parse(raw);
    if (parsed && typeof parsed === 'object' && 'type' in parsed && 'message' in parsed) {
      return parsed as StructuredError;
    }
  } catch {
    // Fall through — raw isn't JSON (e.g. panic or network error).
  }
  return { type: 'Unknown', message: raw };
}

// Mirrors `pkg/web/datalogic_wasm.d.ts` — kept hand-rolled so this file does
// not depend on the WASM .d.ts being regenerated to typecheck. The boolean
// third arg is `templating`: multi-key objects in compiled rules become
// output-shaping templates (the v5 replacement for v4's `preserve_structure`).
interface WasmModule {
  evaluate: (logic: string, data: string, templating: boolean) => string;
  evaluateWithTrace: (logic: string, data: string, templating: boolean) => string;
  CompiledRule: new (logic: string, templating: boolean) => {
    evaluate: (data: string) => string;
    free: () => void;
  };
}

interface UseWasmEvaluatorOptions {
  /** Enable templating mode for JSON templates with embedded JSONLogic. */
  templating?: boolean;
}

interface UseWasmEvaluatorResult {
  ready: boolean;
  loading: boolean;
  error: string | null;
  evaluate: (logic: unknown, data: unknown) => unknown;
  evaluateWithTrace: (logic: unknown, data: unknown) => TracedResult;
}

export function useWasmEvaluator(options: UseWasmEvaluatorOptions = {}): UseWasmEvaluatorResult {
  const { templating = false } = options;
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
            evaluateWithTrace: wasm.evaluateWithTrace,
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
      const resultStr = moduleRef.current.evaluate(logicStr, dataStr, templating);
      return JSON.parse(resultStr);
    } catch (err) {
      throw new DataLogicEvaluationError(parseStructuredError(err, 'Evaluation failed'));
    }
  }, [templating]);

  const evaluateWithTrace = useCallback((logic: unknown, data: unknown): TracedResult => {
    if (!moduleRef.current) {
      throw new Error('WASM module not initialized');
    }

    if (!moduleRef.current.evaluateWithTrace) {
      throw new Error('evaluateWithTrace not available in WASM module');
    }

    const logicStr = JSON.stringify(logic);
    const dataStr = JSON.stringify(data);
    try {
      const resultStr = moduleRef.current.evaluateWithTrace(
        logicStr,
        dataStr,
        templating,
      );
      return JSON.parse(resultStr) as TracedResult;
    } catch (err) {
      throw new DataLogicEvaluationError(parseStructuredError(err, 'Trace evaluation failed'));
    }
  }, [templating]);

  return {
    ready,
    loading,
    error,
    evaluate,
    evaluateWithTrace,
  };
}
