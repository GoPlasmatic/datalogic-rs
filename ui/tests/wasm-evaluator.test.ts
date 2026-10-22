import { describe, expect, it } from 'vitest';
import { evalStr } from '../src/test-utils/engine';
import * as wasm from '@goplasmatic/datalogic-wasm/nodejs';
import {
  parseStructuredError,
  adaptCustomOperators,
  normalizeEvaluationConfig,
  isDefaultEvaluationConfig,
  summarizeEvaluationConfig,
  createWasmEngine,
  DataLogicEvaluationError,
  type WasmModule,
} from '../src/components/logic-editor/hooks/useWasmEvaluator';

const module = wasm as unknown as WasmModule;

describe('parseStructuredError', () => {
  it('reads the structured fields off a real WASM error (detailJson first)', () => {
    let caught: unknown;
    try {
      evalStr('{"throw":"x"}', '{}');
    } catch (err) {
      caught = err;
    }
    expect(caught).toBeInstanceOf(Error);
    // The Display message is not JSON: the old JSON.parse(message) path failed here.
    expect(() => JSON.parse((caught as Error).message)).toThrow();

    const structured = parseStructuredError(caught, 'fallback');
    expect(structured.type).toBe('Thrown');
    expect(structured.operator).toBe('throw');
    expect(structured.thrown).toEqual({ type: 'x' });
    expect(structured.node_ids).toEqual([2]);
    // detailJson carries the kind-only message, without the "(in operator: ...)" suffix.
    expect(structured.message).toBe('Thrown: {"type":"x"}');
  });

  it('classifies unknown operators and parse errors', () => {
    let badOp: unknown;
    try { evalStr('{"foo":[1]}', '{}'); } catch (err) { badOp = err; }
    expect(parseStructuredError(badOp, '')).toMatchObject({ type: 'InvalidOperator', operator: 'foo' });

    let parse: unknown;
    try { evalStr('{"foo":', '{}'); } catch (err) { parse = err; }
    expect(parseStructuredError(parse, '').type).toBe('ParseError');
  });

  it('falls back to own properties when detailJson is absent', () => {
    const err = Object.assign(new Error('Invalid arguments: x (in operator: length)'), {
      type: 'InvalidArguments',
      operator: 'length',
      node_ids: [4],
      index: 1,
    });
    expect(parseStructuredError(err, '')).toEqual({
      type: 'InvalidArguments',
      message: 'Invalid arguments: x (in operator: length)',
      operator: 'length',
      node_ids: [4],
      index: 1,
    });
  });

  it('still accepts the legacy JSON-in-message shape and degrades to Unknown', () => {
    expect(parseStructuredError(new Error('{"type":"Custom","message":"m"}'), '')).toEqual({
      type: 'Custom',
      message: 'm',
    });
    expect(parseStructuredError(new Error('boom'), '')).toEqual({ type: 'Unknown', message: 'boom' });
    expect(parseStructuredError('plain', '')).toEqual({ type: 'Unknown', message: 'plain' });
    expect(parseStructuredError(undefined, 'fallback')).toEqual({ type: 'Unknown', message: 'fallback' });
  });
});

describe('adaptCustomOperators', () => {
  it('bridges JSON-array strings to typed args and back to JSON strings', () => {
    const adapted = adaptCustomOperators({
      double: (args) => (args[0] as number) * 2,
      nothing: () => undefined,
    })!;
    expect(adapted.double('[21]')).toBe('42');
    expect(adapted.nothing('[]')).toBe('null');
  });

  it('returns undefined for empty or missing maps', () => {
    expect(adaptCustomOperators(undefined)).toBeUndefined();
    expect(adaptCustomOperators({})).toBeUndefined();
  });
});

describe('evaluation config helpers', () => {
  it('normalizes undefined keys away', () => {
    expect(normalizeEvaluationConfig(undefined)).toBeUndefined();
    expect(normalizeEvaluationConfig({})).toBeUndefined();
    expect(normalizeEvaluationConfig({ preset: undefined, numeric_coercion: { null_to_zero: undefined } })).toBeUndefined();
    expect(normalizeEvaluationConfig({ preset: 'strict', numeric_coercion: { null_to_zero: false, bool_to_number: undefined } }))
      .toEqual({ preset: 'strict', numeric_coercion: { null_to_zero: false } });
  });

  it('detects default configs and summarizes non-default ones', () => {
    expect(isDefaultEvaluationConfig(undefined)).toBe(true);
    expect(isDefaultEvaluationConfig({ preset: 'default' })).toBe(true);
    expect(summarizeEvaluationConfig({ preset: 'default' })).toBeNull();
    expect(isDefaultEvaluationConfig({ division_by_zero: 'return_null' })).toBe(false);
    expect(summarizeEvaluationConfig({ preset: 'strict', division_by_zero: 'return_null', numeric_coercion: { reject_non_numeric: true } }))
      .toBe('preset strict, div/0 return_null, reject non-numeric on');
  });
});

describe('createWasmEngine', () => {
  it('honours the evaluation config on both the plain and traced paths', () => {
    const engine = createWasmEngine(module, { config: { division_by_zero: 'return_null' } });
    expect(JSON.parse(engine.evalStr('{"/":[10.5,0]}', '{}'))).toBeNull();
    const trace = JSON.parse(engine.evaluateWithTrace('{"/":[10.5,0]}', '{}'));
    expect(trace.result).toBeNull();
    expect(trace.error).toBeUndefined();
    engine.free();

    const strict = createWasmEngine(module, { config: { preset: 'strict' } });
    expect(() => strict.evalStr('{"/":[10.5,0]}', '{}')).toThrow();
    const strictTrace = JSON.parse(strict.evaluateWithTrace('{"/":[10.5,0]}', '{}'));
    expect(strictTrace.structured_error).toMatchObject({ type: 'Thrown', operator: '/' });
    strict.free();
  });

  it('registers custom operators and applies templating', () => {
    const engine = createWasmEngine(module, {
      templating: true,
      customOperators: { double: (args) => (args[0] as number) * 2 },
    });
    expect(engine.customOperatorNames()).toEqual(['double']);
    expect(JSON.parse(engine.evalStr('{"a":{"double":[{"var":"n"}]},"b":1}', '{"n":4}'))).toEqual({ a: 8, b: 1 });
    const trace = JSON.parse(engine.evaluateWithTrace('{"double":[3]}', '{}'));
    expect(trace.result).toBe(6);
    engine.free();
  });

  it('surfaces an invalid config as a DataLogicEvaluationError of type ConfigurationError', () => {
    expect(() =>
      createWasmEngine(module, { config: { division_by_zero: 'nope' as never } }),
    ).toThrow(DataLogicEvaluationError);
    try {
      createWasmEngine(module, { config: { division_by_zero: 'nope' as never } });
    } catch (err) {
      expect((err as DataLogicEvaluationError).structured.type).toBe('ConfigurationError');
    }
  });
});

/**
 * The operation budget is the Studio's only engine setting whose effect is
 * visible on a *successful* run: every evaluation reports what it cost, and
 * the cap only shows up when a rule crosses it. Both halves are asserted
 * against the real WASM engine, since the count is the engine's to define.
 */
describe('operation budget', () => {
  const MAP = '{"map":[{"var":"xs"},{"*":[{"var":""},2]}]}';
  const small = '{"xs":[1,2,3]}';
  const large = `{"xs":[${Array.from({ length: 200 }, (_, i) => i).join(',')}]}`;

  it('reports the operations an evaluation charged', () => {
    const engine = createWasmEngine(module, {});
    const { result, ops } = JSON.parse(engine.evalMetered(MAP, small));
    expect(result).toEqual([2, 4, 6]);
    // One per dispatched node plus one per item; the exact figure is the
    // engine's to define, so pin the floor rather than the number.
    expect(ops).toBeGreaterThanOrEqual(3);
    engine.free();
  });

  it('charges nothing for a rule the compiler folded to a literal', () => {
    const engine = createWasmEngine(module, {});
    expect(JSON.parse(engine.evalMetered('{"+":[1,2]}', '{}')).ops).toBe(0);
    engine.free();
  });

  it('refuses an evaluation that would cross the configured ceiling', () => {
    const engine = createWasmEngine(module, { config: { ops_budget: 10 } });
    let caught: unknown;
    try { engine.evalMetered(MAP, large); } catch (err) { caught = err; }
    const structured = parseStructuredError(caught, 'fallback');
    expect(structured.type).toBe('BudgetExceeded');
    expect(structured.budget).toBe(10);
    expect(structured.spent).toBeGreaterThan(10);
    // The breadcrumb survives, so the editor can still highlight the node.
    expect(structured.node_ids?.length).toBeGreaterThan(0);
    engine.free();
  });

  it('applies the configured ceiling to plain evaluation too, and try cannot catch it', () => {
    const engine = createWasmEngine(module, { config: { ops_budget: 10 } });
    expect(() => engine.evalStr(MAP, large)).toThrow();
    expect(() => engine.evalStr(`{"try":[${MAP},"fallback"]}`, large)).toThrow();
    engine.free();
  });

  it('leaves an unset budget unbounded', () => {
    const engine = createWasmEngine(module, {});
    expect(JSON.parse(engine.evalMetered(MAP, large)).result).toHaveLength(200);
    engine.free();
  });

  it('prices tensor operators by the elements they move', () => {
    const engine = createWasmEngine(module, {});
    const { ops } = JSON.parse(engine.evalMetered('{"zeros":[[16,16],"f32"]}', '{}'));
    expect(ops).toBeGreaterThanOrEqual(256);
    engine.free();
  });

  it('summarizes a budget in the engine-settings badge', () => {
    expect(summarizeEvaluationConfig({ ops_budget: 5000 })).toContain('budget 5000');
  });
});

describe('v5.8 / v5.9 engine features', () => {
  it('supports missing_var: "error" to raise VariableNotFound on missing paths', () => {
    expect(summarizeEvaluationConfig({ missing_var: 'error' })).toBe('missing var error');
    const engine = createWasmEngine(module, { config: { missing_var: 'error' } });
    // Explicit default argument still works when path is missing
    expect(JSON.parse(engine.evalStr('{"var":["missing","fallback"]}', '{}'))).toBe('fallback');
    // Missing path without default raises VariableNotFound
    let caught: unknown;
    try {
      engine.evalStr('{"var":"missing"}', '{}');
    } catch (err) {
      caught = err;
    }
    const structured = parseStructuredError(caught, 'fallback');
    expect(structured.type).toBe('VariableNotFound');
    expect(structured.variable).toBe('missing');
    engine.free();
  });

  it('supports templateKeyEscape in templating mode', () => {
    const engine = createWasmEngine(module, {
      templating: true,
      templateKeyEscape: '$',
    });
    // Without escape, {"var": "x"} evaluates as operator `var`; with `$` prefix,
    // {"$var": "literal"} emits literal key "var" with value "literal".
    expect(JSON.parse(engine.evalStr('{"$var":"literal"}', '{"literal":"ignored"}'))).toEqual({
      var: 'literal',
    });
    engine.free();
  });

  it('supports strictOperatorNames to reject custom operators that shadow built-ins', () => {
    expect(() =>
      createWasmEngine(module, {
        strictOperatorNames: true,
        customOperators: { var: () => null },
      }),
    ).toThrow(DataLogicEvaluationError);
  });

  it('supports families option to restrict built-in operator families', () => {
    const engine = createWasmEngine(module, {
      families: ['ExtString'],
    });
    // Core + ExtString work
    expect(JSON.parse(engine.evalStr('{"upper":"hello"}', '{}'))).toBe('HELLO');
    // DateTime is excluded
    expect(() => engine.evalStr('{"datetime":"2026-01-01T00:00:00Z"}', '{}')).toThrow();
    const ops = JSON.parse(engine.operators()) as { family: string }[];
    const families = new Set(ops.map((o) => o.family));
    expect(families.has('Core')).toBe(true);
    expect(families.has('ExtString')).toBe(true);
    expect(families.has('DateTime')).toBe(false);
    engine.free();
  });

  it('exposes static rule diagnostics via engine.check() and CompileError diagnostics', () => {
    const engine = createWasmEngine(module, {});
    // Extra unused argument on fixed-arity operator (`!` takes 1 arg) produces a warning
    const warnings = JSON.parse(engine.check('{"!":[true,false]}'));
    expect(warnings).toEqual([
      expect.objectContaining({
        code: 'ArgumentCount',
        severity: 'warning',
        operator: '!',
      }),
    ]);

    // Invalid timezone in format_date produces an error diagnostic
    const tzDiags = JSON.parse(
      engine.check('{"format_date":[{"datetime":"2026-01-01T00:00:00Z"},"yyyy","Mars/Phobos"]}'),
    );
    expect(tzDiags).toEqual([
      expect.objectContaining({
        code: 'InvalidTimezone',
        severity: 'error',
        operator: 'format_date',
      }),
    ]);

    // compileChecked throws CompileError with diagnostics array preserved by parseStructuredError
    let compileCaught: unknown;
    try {
      engine.compileChecked('{"unknown_op":[1]}');
    } catch (err) {
      compileCaught = err;
    }
    const structured = parseStructuredError(compileCaught, 'fallback');
    expect(structured.type).toBe('CompileError');
    expect(structured.diagnostics).toEqual([
      expect.objectContaining({
        code: 'UnknownOperator',
        severity: 'error',
        operator: 'unknown_op',
      }),
    ]);
    engine.free();
  });

  it('exposes static rule facts via rule.facts() and truthy via engine.truthy()', () => {
    const engine = createWasmEngine(module, {
      config: { truthy_evaluator: 'strict_boolean' },
    });
    const rule = engine.compile('{"and":[{">":[{"var":"user.age"},18]},{"==":[{"var":"status"},"active"]}]}');
    const facts = JSON.parse(rule.facts());
    expect(facts).toEqual({
      reads: [['status'], ['user', 'age']],
      computed_reads: false,
      reads_complete: true,
      reads_data: true,
      operators: ['==', '>', 'and', 'val'],
      custom_operators: [],
      deterministic: true,
    });
    rule.free();

    // Under strict_boolean, 0 and "" are truthy (only false and null are falsy)
    expect(engine.truthy('0')).toBe(true);
    expect(engine.truthy('false')).toBe(false);
    engine.free();
  });
});
