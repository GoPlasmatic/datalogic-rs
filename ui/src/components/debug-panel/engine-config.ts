import type { DataLogicEvaluationConfig } from '../logic-editor/types';
import { normalizeEvaluationConfig } from '../logic-editor/hooks/useWasmEvaluator';

export type Preset = NonNullable<DataLogicEvaluationConfig['preset']>;
export type NanHandling = NonNullable<DataLogicEvaluationConfig['arithmetic_nan_handling']>;
export type DivisionByZero = NonNullable<DataLogicEvaluationConfig['division_by_zero']>;
export type TruthyEvaluator = NonNullable<DataLogicEvaluationConfig['truthy_evaluator']>;
export type CoercionFlags = Required<NonNullable<DataLogicEvaluationConfig['numeric_coercion']>>;

export interface ResolvedConfig {
  preset: Preset;
  arithmetic_nan_handling: NanHandling;
  division_by_zero: DivisionByZero;
  loose_equality_errors: boolean;
  truthy_evaluator: TruthyEvaluator;
  numeric_coercion: CoercionFlags;
  max_recursion_depth: number;
  /** `undefined` means unbounded — the engine's default. */
  ops_budget: number | undefined;
}

const DEFAULT_COERCION: CoercionFlags = {
  empty_string_to_zero: true,
  null_to_zero: true,
  bool_to_number: true,
  reject_non_numeric: false,
};

/**
 * Engine defaults per preset (mirrors `EvaluationConfig::default`,
 * `::safe_arithmetic` and `::strict` in the Rust crate). Used to show the
 * effective value of every knob and to drop overrides that merely restate
 * the preset, so the stored config stays minimal.
 */
export const PRESET_BASES: Record<Preset, ResolvedConfig> = {
  default: {
    preset: 'default',
    arithmetic_nan_handling: 'throw_error',
    division_by_zero: 'return_saturated',
    loose_equality_errors: true,
    truthy_evaluator: 'javascript',
    numeric_coercion: { ...DEFAULT_COERCION },
    max_recursion_depth: 256,
    ops_budget: undefined,
  },
  safe_arithmetic: {
    preset: 'safe_arithmetic',
    arithmetic_nan_handling: 'ignore_value',
    division_by_zero: 'return_null',
    loose_equality_errors: false,
    truthy_evaluator: 'javascript',
    numeric_coercion: { ...DEFAULT_COERCION },
    max_recursion_depth: 256,
    ops_budget: undefined,
  },
  strict: {
    preset: 'strict',
    arithmetic_nan_handling: 'throw_error',
    division_by_zero: 'throw_error',
    loose_equality_errors: true,
    truthy_evaluator: 'javascript',
    numeric_coercion: {
      empty_string_to_zero: false,
      null_to_zero: false,
      bool_to_number: false,
      reject_non_numeric: true,
    },
    max_recursion_depth: 256,
    ops_budget: undefined,
  },
};

/** Effective value of every knob: preset base plus explicit overrides. */
export function resolveEvaluationConfig(config: DataLogicEvaluationConfig | undefined): ResolvedConfig {
  const preset: Preset = config?.preset ?? 'default';
  const base = PRESET_BASES[preset];
  return {
    preset,
    arithmetic_nan_handling: config?.arithmetic_nan_handling ?? base.arithmetic_nan_handling,
    division_by_zero: config?.division_by_zero ?? base.division_by_zero,
    loose_equality_errors: config?.loose_equality_errors ?? base.loose_equality_errors,
    truthy_evaluator: config?.truthy_evaluator ?? base.truthy_evaluator,
    numeric_coercion: { ...base.numeric_coercion, ...(config?.numeric_coercion ?? {}) },
    max_recursion_depth: config?.max_recursion_depth ?? base.max_recursion_depth,
    ops_budget: config?.ops_budget ?? base.ops_budget,
  };
}

const TOP_LEVEL_KNOBS = [
  'arithmetic_nan_handling',
  'division_by_zero',
  'loose_equality_errors',
  'truthy_evaluator',
  'max_recursion_depth',
  // No preset sets a budget, so an explicit one is never a restatement —
  // it survives pruning by the `!== undefined` guard in `pruneAgainstPreset`.
  'ops_budget',
] as const;

/**
 * Drop every override that merely restates the config's own preset base.
 * Applied to the whole config rather than just the knob being written,
 * because changing `preset` changes what counts as redundant: an override
 * kept under the old base can become a restatement of the new one.
 */
function pruneAgainstPreset(config: DataLogicEvaluationConfig): DataLogicEvaluationConfig {
  const base = PRESET_BASES[config.preset ?? 'default'];
  const next: DataLogicEvaluationConfig = { ...config };
  for (const knob of TOP_LEVEL_KNOBS) {
    if (next[knob] !== undefined && next[knob] === base[knob]) delete next[knob];
  }
  if (next.numeric_coercion) {
    const coercion = { ...next.numeric_coercion };
    for (const flag of Object.keys(coercion) as (keyof CoercionFlags)[]) {
      if (coercion[flag] === base.numeric_coercion[flag]) delete coercion[flag];
    }
    next.numeric_coercion = coercion;
  }
  return next;
}

/**
 * Set one top-level knob (including `preset` itself), keeping the stored
 * config minimal: only overrides that differ from the preset survive.
 */
export function withOverride<K extends keyof DataLogicEvaluationConfig>(
  config: DataLogicEvaluationConfig,
  key: K,
  value: DataLogicEvaluationConfig[K],
): DataLogicEvaluationConfig {
  const next: DataLogicEvaluationConfig = { ...config };
  if (value === undefined) {
    delete next[key];
  } else {
    next[key] = value;
  }
  return normalizeEvaluationConfig(pruneAgainstPreset(next)) ?? {};
}

export function withCoercion(
  config: DataLogicEvaluationConfig,
  key: keyof CoercionFlags,
  value: boolean,
): DataLogicEvaluationConfig {
  const coercion = { ...(config.numeric_coercion ?? {}), [key]: value };
  const next: DataLogicEvaluationConfig = { ...config, numeric_coercion: coercion };
  return normalizeEvaluationConfig(pruneAgainstPreset(next)) ?? {};
}
