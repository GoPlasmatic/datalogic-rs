import { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import type {
  StructuredError,
  TracedResult,
  DataLogicEvaluationConfig,
  DataLogicCustomOperator,
} from '../types';

/** Error thrown from the WASM boundary that carries the parsed StructuredError. */
export class DataLogicEvaluationError extends Error {
  readonly structured: StructuredError;

  constructor(structured: StructuredError) {
    super(structured.message || structured.type);
    this.name = 'DataLogicEvaluationError';
    this.structured = structured;
  }
}

const STRUCTURED_FIELDS = [
  'operator',
  'variable',
  'level',
  'thrown',
  'index',
  'length',
  'stage',
  'node_ids',
] as const;

function isStructuredError(value: unknown): value is StructuredError {
  return (
    !!value &&
    typeof value === 'object' &&
    typeof (value as { type?: unknown }).type === 'string' &&
    typeof (value as { message?: unknown }).message === 'string'
  );
}

/**
 * Turn whatever the WASM boundary threw into a `StructuredError`.
 *
 * Since the 5.1 error bridge the thrown value is a real `Error` whose
 * `message` is the human-readable Display string ("Thrown: ... (in
 * operator: throw)") and whose structured fields ride along as own
 * properties (`type`, `operator`, `node_ids`, variant extras) plus the raw
 * JSON in `detailJson`. Older builds threw a bare JSON string. Try, in
 * order: `detailJson`, the own properties, a JSON `message`, and finally a
 * synthetic `Unknown` error so downstream code always gets one shape.
 */
export function parseStructuredError(err: unknown, fallbackMessage: string): StructuredError {
  if (err && typeof err === 'object') {
    const obj = err as Record<string, unknown>;

    // 1. Raw JSON payload attached by the WASM error bridge.
    if (typeof obj.detailJson === 'string') {
      try {
        const parsed: unknown = JSON.parse(obj.detailJson);
        if (isStructuredError(parsed)) return parsed;
      } catch {
        // detailJson should always be JSON; fall through to the own props.
      }
    }

    // 2. Own properties on the Error object.
    if (typeof obj.type === 'string') {
      const message =
        typeof obj.message === 'string' ? obj.message : fallbackMessage;
      const structured: StructuredError = { type: obj.type, message };
      for (const field of STRUCTURED_FIELDS) {
        if (obj[field] !== undefined) {
          (structured as unknown as Record<string, unknown>)[field] = obj[field];
        }
      }
      return structured;
    }
  }

  // 3. Legacy: the message (or the thrown string itself) is a JSON document.
  const raw =
    err instanceof Error ? err.message : typeof err === 'string' ? err : fallbackMessage;
  try {
    const parsed: unknown = JSON.parse(raw);
    if (isStructuredError(parsed)) return parsed;
  } catch {
    // raw is not JSON (e.g. a panic message or a network failure).
  }
  return { type: 'Unknown', message: raw };
}

// Mirrors `vendor/datalogic/web/datalogic_wasm.d.ts` for the surface this
// hook uses. Kept hand-rolled so the file typechecks without depending on
// the WASM .d.ts being regenerated.
export interface WasmEngineInstance {
  evalStr: (logic: string, data: string) => string;
  evaluateWithTrace: (logic: string, data: string) => string;
  customOperatorNames?: () => string[];
  free?: () => void;
}

export interface WasmEngineOptions {
  templating?: boolean;
  customOperators?: Record<string, (argsJson: string) => string>;
  config?: string | object;
}

export interface WasmModule {
  Engine: new (options: WasmEngineOptions) => WasmEngineInstance;
}

export interface UseWasmEvaluatorOptions {
  /** Enable templating mode for JSON templates with embedded JSONLogic. */
  templating?: boolean;
  /** Engine evaluation settings (see `DataLogicEvaluationConfig`). */
  config?: DataLogicEvaluationConfig | null;
  /**
   * Custom operators, keyed by name. Implementations are read at call time,
   * so this may be a fresh object every render; only adding or removing a
   * name rebuilds the engine.
   */
  customOperators?: Record<string, DataLogicCustomOperator>;
}

export interface UseWasmEvaluatorResult {
  ready: boolean;
  loading: boolean;
  error: string | null;
  evaluate: (logic: unknown, data: unknown) => unknown;
  evaluateWithTrace: (logic: unknown, data: unknown) => TracedResult;
}

/**
 * Wrap a `(args: unknown[]) => unknown` custom operator into the WASM
 * contract: JSON-array string in, JSON string out. `undefined` results
 * become JSON `null`, which is what the engine treats an empty return as.
 *
 * `latest` makes the binding late: the adapter resolves the implementation
 * at call time instead of closing over it, so a caller re-creating its
 * `customOperators` object (an inline literal on every render, say) does
 * not invalidate an engine that already registered these names.
 */
export function adaptCustomOperators(
  customOperators: Record<string, DataLogicCustomOperator> | undefined,
  latest?: () => Record<string, DataLogicCustomOperator> | undefined,
): Record<string, (argsJson: string) => string> | undefined {
  if (!customOperators) return undefined;
  const entries = Object.entries(customOperators);
  if (entries.length === 0) return undefined;
  const adapted: Record<string, (argsJson: string) => string> = {};
  for (const [name, fn] of entries) {
    adapted[name] = (argsJson: string) => {
      const args = JSON.parse(argsJson) as unknown[];
      const result = (latest?.()?.[name] ?? fn)(args);
      return result === undefined ? 'null' : JSON.stringify(result);
    };
  }
  return adapted;
}

/**
 * Stable key for a custom-operator map: its names, sorted. Functions do not
 * serialize and the object identity churns whenever a caller passes an
 * inline literal, so the name set is the only part worth keying on.
 */
export function customOperatorNamesKey(
  customOperators: Record<string, DataLogicCustomOperator> | undefined,
): string {
  if (!customOperators) return '';
  return Object.keys(customOperators).sort().join(',');
}

/**
 * Drop `undefined` keys (and an empty `numeric_coercion` block) so that
 * `{}` and "no config" mean the same thing and serialize identically.
 */
export function normalizeEvaluationConfig(
  config: DataLogicEvaluationConfig | null | undefined,
): DataLogicEvaluationConfig | undefined {
  if (!config) return undefined;
  const out: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(config)) {
    if (value === undefined) continue;
    if (key === 'numeric_coercion' && value && typeof value === 'object') {
      const nested: Record<string, unknown> = {};
      for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
        if (v !== undefined) nested[k] = v;
      }
      if (Object.keys(nested).length > 0) out[key] = nested;
      continue;
    }
    out[key] = value;
  }
  return Object.keys(out).length > 0 ? (out as DataLogicEvaluationConfig) : undefined;
}

/** `true` when the config carries nothing beyond the engine defaults. */
export function isDefaultEvaluationConfig(
  config: DataLogicEvaluationConfig | null | undefined,
): boolean {
  const normalized = normalizeEvaluationConfig(config);
  if (!normalized) return true;
  const { preset, ...rest } = normalized;
  return (preset === undefined || preset === 'default') && Object.keys(rest).length === 0;
}

const SUMMARY_LABELS: Record<string, string> = {
  arithmetic_nan_handling: 'NaN',
  division_by_zero: 'div/0',
  loose_equality_errors: 'loose ==',
  truthy_evaluator: 'truthy',
  max_recursion_depth: 'depth',
};

const COERCION_LABELS: Record<string, string> = {
  empty_string_to_zero: '"" to 0',
  null_to_zero: 'null to 0',
  bool_to_number: 'bool to num',
  reject_non_numeric: 'reject non-numeric',
};

/**
 * One-line, human-readable summary of the non-default engine settings
 * ("preset strict, div/0 return_null"), or `null` when everything is at
 * the engine default. Used for the toolbar indicator and the result pane.
 */
export function summarizeEvaluationConfig(
  config: DataLogicEvaluationConfig | null | undefined,
): string | null {
  if (isDefaultEvaluationConfig(config)) return null;
  const normalized = normalizeEvaluationConfig(config) ?? {};
  const parts: string[] = [];
  if (normalized.preset && normalized.preset !== 'default') {
    parts.push(`preset ${normalized.preset}`);
  }
  for (const [key, label] of Object.entries(SUMMARY_LABELS)) {
    const value = (normalized as Record<string, unknown>)[key];
    if (value !== undefined) parts.push(`${label} ${String(value)}`);
  }
  if (normalized.numeric_coercion) {
    for (const [key, label] of Object.entries(COERCION_LABELS)) {
      const value = (normalized.numeric_coercion as Record<string, unknown>)[key];
      if (value !== undefined) parts.push(`${label} ${value ? 'on' : 'off'}`);
    }
  }
  return parts.join(', ');
}

/**
 * Build a WASM `Engine` for the given options. Throws the engine's
 * `ConfigurationError` (as a `DataLogicEvaluationError`) for invalid config.
 */
export function createWasmEngine(
  module: WasmModule,
  options: UseWasmEvaluatorOptions,
  latestCustomOperators?: () => Record<string, DataLogicCustomOperator> | undefined,
): WasmEngineInstance {
  const engineOptions: WasmEngineOptions = { templating: options.templating ?? false };
  const config = normalizeEvaluationConfig(options.config);
  if (config) engineOptions.config = config;
  const customOperators = adaptCustomOperators(options.customOperators, latestCustomOperators);
  if (customOperators) engineOptions.customOperators = customOperators;
  try {
    return new module.Engine(engineOptions);
  } catch (err) {
    throw new DataLogicEvaluationError(parseStructuredError(err, 'Invalid engine configuration'));
  }
}

interface EngineSlot {
  key: string;
  engine: WasmEngineInstance;
}

export function useWasmEvaluator(options: UseWasmEvaluatorOptions = {}): UseWasmEvaluatorResult {
  const { templating = false, config, customOperators } = options;
  const [ready, setReady] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const moduleRef = useRef<WasmModule | null>(null);
  const engineRef = useRef<EngineSlot | null>(null);

  // The latest implementations, read at call time by the adapters registered
  // with the engine, so it survives a caller that rebuilds this object every
  // render. Refreshed inside `getEngine` below — every evaluation goes
  // through it, and it sees the current props of whichever render asked.
  const customOperatorsRef = useRef(customOperators);
  const latestCustomOperators = useCallback(() => customOperatorsRef.current, []);

  // Serialized identity of everything that requires a fresh Engine. Only the
  // set of custom operator NAMES matters: the implementations are late-bound,
  // so changing one does not need a rebuild, but registering or dropping a
  // name does.
  const configKey = useMemo(
    () => JSON.stringify(normalizeEvaluationConfig(config) ?? null),
    [config],
  );
  const operatorNamesKey = customOperatorNamesKey(customOperators);
  const engineKey = `${templating ? 1 : 0}|${configKey}|${operatorNamesKey}`;

  useEffect(() => {
    let cancelled = false;

    async function initWasm() {
      try {
        setLoading(true);
        setError(null);

        // Dynamic import of the WASM module; the web target's default
        // export is the async loader.
        const wasm = await import('@goplasmatic/datalogic-wasm');
        await wasm.default();

        if (!cancelled) {
          moduleRef.current = {
            Engine: wasm.Engine as unknown as WasmModule['Engine'],
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

  // Release the engine when the hook unmounts.
  useEffect(() => {
    return () => {
      engineRef.current?.engine.free?.();
      engineRef.current = null;
    };
  }, []);

  // Lazily (re)build the Engine so templating/config/customOperators changes
  // take effect on the next evaluation, freeing the previous instance.
  const getEngine = useCallback((): WasmEngineInstance => {
    const module = moduleRef.current;
    if (!module) {
      throw new Error('WASM module not initialized');
    }
    customOperatorsRef.current = customOperators;
    const slot = engineRef.current;
    if (slot && slot.key === engineKey) {
      return slot.engine;
    }
    slot?.engine.free?.();
    engineRef.current = null;
    const engine = createWasmEngine(
      module,
      { templating, config, customOperators },
      latestCustomOperators,
    );
    engineRef.current = { key: engineKey, engine };
    return engine;
  }, [engineKey, templating, config, customOperators, latestCustomOperators]);

  const evaluate = useCallback((logic: unknown, data: unknown): unknown => {
    const engine = getEngine();
    const logicStr = JSON.stringify(logic);
    const dataStr = JSON.stringify(data);
    try {
      const resultStr = engine.evalStr(logicStr, dataStr);
      return JSON.parse(resultStr);
    } catch (err) {
      throw new DataLogicEvaluationError(parseStructuredError(err, 'Evaluation failed'));
    }
  }, [getEngine]);

  const evaluateWithTrace = useCallback((logic: unknown, data: unknown): TracedResult => {
    const engine = getEngine();
    const logicStr = JSON.stringify(logic);
    const dataStr = JSON.stringify(data);
    try {
      const resultStr = engine.evaluateWithTrace(logicStr, dataStr);
      return JSON.parse(resultStr) as TracedResult;
    } catch (err) {
      throw new DataLogicEvaluationError(parseStructuredError(err, 'Trace evaluation failed'));
    }
  }, [getEngine]);

  return {
    ready,
    loading,
    error,
    evaluate,
    evaluateWithTrace,
  };
}
