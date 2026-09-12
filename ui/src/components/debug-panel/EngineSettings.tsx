import { useCallback, useEffect, useMemo, useRef } from 'react';
import { RotateCcw, X } from 'lucide-react';
import type { DataLogicEvaluationConfig } from '../logic-editor/types';
import { normalizeEvaluationConfig } from '../logic-editor/hooks/useWasmEvaluator';
import {
  resolveEvaluationConfig,
  positiveInt,
  withOverride,
  withCoercion,
  type Preset,
  type NanHandling,
  type DivisionByZero,
  type TruthyEvaluator,
  type CoercionFlags,
} from './engine-config';
import './EngineSettings.css';

const NAN_OPTIONS: { value: NanHandling; label: string; hint: string }[] = [
  { value: 'throw_error', label: 'Throw error', hint: '{"+": [1, "text"]} raises Thrown {"type": "NaN"}' },
  { value: 'ignore_value', label: 'Ignore value', hint: 'Non-numeric operands are skipped' },
  { value: 'coerce_to_zero', label: 'Coerce to zero', hint: 'Non-numeric operands count as 0' },
  { value: 'return_null', label: 'Return null', hint: 'The whole operation yields null' },
];

const DIV0_OPTIONS: { value: DivisionByZero; label: string; hint: string }[] = [
  { value: 'return_saturated', label: 'Return saturated', hint: 'f64::MAX with the sign of the dividend' },
  { value: 'throw_error', label: 'Throw error', hint: 'Raises Thrown {"type": "NaN"}' },
  { value: 'return_null', label: 'Return null', hint: 'The division yields null' },
  { value: 'return_infinity', label: 'Return infinity', hint: 'Infinity in Rust, null once serialized to JSON' },
];

const TRUTHY_OPTIONS: { value: TruthyEvaluator; label: string; hint: string }[] = [
  { value: 'javascript', label: 'JavaScript', hint: '0, "", [], {} and null are falsy' },
  { value: 'python', label: 'Python', hint: 'Same as JavaScript except NaN is truthy' },
  { value: 'strict_boolean', label: 'Strict boolean', hint: 'Only false and null are falsy' },
];

const COERCION_OPTIONS: { key: keyof CoercionFlags; label: string; hint: string }[] = [
  { key: 'empty_string_to_zero', label: 'Empty string to 0', hint: '"" counts as 0 in arithmetic' },
  { key: 'null_to_zero', label: 'null to 0', hint: 'null (and missing vars) count as 0' },
  { key: 'bool_to_number', label: 'Booleans to numbers', hint: 'true is 1, false is 0' },
  { key: 'reject_non_numeric', label: 'Reject non-numeric', hint: 'Overrides the flags above: any non-number raises' },
];

export interface EngineSettingsPanelProps {
  config: DataLogicEvaluationConfig;
  onChange: (config: DataLogicEvaluationConfig) => void;
  onClose?: () => void;
  /** Presentation: anchored dropdown (desktop) or centered sheet (mobile). */
  variant?: 'popover' | 'sheet';
}

/**
 * Engine settings editor: preset plus every `EvaluationConfig` knob the
 * WASM engine accepts. Controls show the effective value (preset base plus
 * overrides); changing one records an explicit override, and "Reset to
 * defaults" clears everything back to the engine defaults.
 */
export function EngineSettingsPanel({
  config,
  onChange,
  onClose,
  variant = 'popover',
}: EngineSettingsPanelProps) {
  const resolved = useMemo(() => resolveEvaluationConfig(config), [config]);
  const panelRef = useRef<HTMLDivElement>(null);
  const isDefault = Object.keys(normalizeEvaluationConfig(config) ?? {}).length === 0;

  // Escape closes the panel; focus the first control on open.
  useEffect(() => {
    if (!onClose) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [onClose]);

  useEffect(() => {
    panelRef.current?.querySelector<HTMLElement>('select, input')?.focus();
  }, []);

  const setPreset = useCallback(
    (preset: Preset) => {
      // Switching preset discards per-knob overrides: the preset is the new
      // baseline and the controls re-render with its effective values.
      onChange(preset === 'default' ? {} : { preset });
    },
    [onChange],
  );

  return (
    <div
      ref={panelRef}
      className={`engine-settings engine-settings--${variant}`}
      role="dialog"
      aria-label="Engine settings"
    >
      <div className="engine-settings-header">
        <div>
          <div className="engine-settings-title">Engine settings</div>
          <div className="engine-settings-subtitle">
            Evaluation semantics applied to the result and to the debugger trace.
          </div>
        </div>
        <div className="engine-settings-header-actions">
          <button
            type="button"
            className="engine-settings-reset"
            onClick={() => onChange({})}
            disabled={isDefault}
            title="Reset every setting to the engine default"
          >
            <RotateCcw size={13} />
            <span>Reset to defaults</span>
          </button>
          {onClose && (
            <button
              type="button"
              className="engine-settings-close"
              onClick={onClose}
              aria-label="Close engine settings"
            >
              <X size={16} />
            </button>
          )}
        </div>
      </div>

      <div className="engine-settings-body">
        <label className="engine-settings-field">
          <span className="engine-settings-label">Preset</span>
          <select
            value={resolved.preset}
            onChange={(e) => setPreset(e.target.value as Preset)}
          >
            <option value="default">Default</option>
            <option value="safe_arithmetic">Safe arithmetic (ignore NaN, null on divide by zero)</option>
            <option value="strict">Strict (throw on NaN and divide by zero, no coercion)</option>
          </select>
        </label>

        <label className="engine-settings-field">
          <span className="engine-settings-label">Division by zero</span>
          <select
            value={resolved.division_by_zero}
            onChange={(e) => onChange(withOverride(config, 'division_by_zero', e.target.value as DivisionByZero))}
          >
            {DIV0_OPTIONS.map((o) => (
              <option key={o.value} value={o.value} title={o.hint}>{o.label}</option>
            ))}
          </select>
          <span className="engine-settings-hint">
            {DIV0_OPTIONS.find((o) => o.value === resolved.division_by_zero)?.hint}. Only a fractional
            dividend takes this path; integer division by zero always throws.
          </span>
        </label>

        <label className="engine-settings-field">
          <span className="engine-settings-label">Arithmetic NaN handling</span>
          <select
            value={resolved.arithmetic_nan_handling}
            onChange={(e) => onChange(withOverride(config, 'arithmetic_nan_handling', e.target.value as NanHandling))}
          >
            {NAN_OPTIONS.map((o) => (
              <option key={o.value} value={o.value} title={o.hint}>{o.label}</option>
            ))}
          </select>
          <span className="engine-settings-hint">
            {NAN_OPTIONS.find((o) => o.value === resolved.arithmetic_nan_handling)?.hint}
          </span>
        </label>

        <label className="engine-settings-field">
          <span className="engine-settings-label">Truthiness</span>
          <select
            value={resolved.truthy_evaluator}
            onChange={(e) => onChange(withOverride(config, 'truthy_evaluator', e.target.value as TruthyEvaluator))}
          >
            {TRUTHY_OPTIONS.map((o) => (
              <option key={o.value} value={o.value} title={o.hint}>{o.label}</option>
            ))}
          </select>
          <span className="engine-settings-hint">
            {TRUTHY_OPTIONS.find((o) => o.value === resolved.truthy_evaluator)?.hint}
          </span>
        </label>

        <label className="engine-settings-check">
          <input
            type="checkbox"
            checked={resolved.loose_equality_errors}
            onChange={(e) => onChange(withOverride(config, 'loose_equality_errors', e.target.checked))}
          />
          <span>
            <span className="engine-settings-check-label">Loose equality errors</span>
            <span className="engine-settings-hint">Loose == raises on incompatible types instead of returning false</span>
          </span>
        </label>

        <fieldset className="engine-settings-group">
          <legend className="engine-settings-label">Numeric coercion</legend>
          {COERCION_OPTIONS.map((o) => (
            <label key={o.key} className="engine-settings-check">
              <input
                type="checkbox"
                checked={resolved.numeric_coercion[o.key]}
                onChange={(e) => onChange(withCoercion(config, o.key, e.target.checked))}
              />
              <span>
                <span className="engine-settings-check-label">{o.label}</span>
                <span className="engine-settings-hint">{o.hint}</span>
              </span>
            </label>
          ))}
        </fieldset>

        <label className="engine-settings-field">
          <span className="engine-settings-label">Max recursion depth</span>
          <input
            type="number"
            min={1}
            step={1}
            value={resolved.max_recursion_depth}
            onChange={(e) =>
              onChange(withOverride(config, 'max_recursion_depth', positiveInt(e.target.value)))
            }
          />
          <span className="engine-settings-hint">
            Nested engine re-entries allowed before a ConfigurationError (custom operators only). Default 256.
          </span>
        </label>

        <label className="engine-settings-field">
          <span className="engine-settings-label">Operation budget</span>
          <input
            type="number"
            min={1}
            step={1}
            placeholder="Unlimited"
            value={resolved.ops_budget ?? ''}
            onChange={(e) => onChange(withOverride(config, 'ops_budget', positiveInt(e.target.value)))}
          />
          <span className="engine-settings-hint">
            Caps the work one evaluation may do — one operation per node the engine dispatches, one
            per item an iterator walks, plus what tensor operators charge per element. Crossing it
            raises BudgetExceeded, which <code>try</code> cannot catch. Blank means unlimited; the
            result panel reports what each run actually spent.
          </span>
        </label>
      </div>
    </div>
  );
}
