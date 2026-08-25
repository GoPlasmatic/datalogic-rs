import { describe, expect, it } from 'vitest';
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
      wasm.evaluate('{"throw":"x"}', '{}', false);
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
    try { wasm.evaluate('{"foo":[1]}', '{}', false); } catch (err) { badOp = err; }
    expect(parseStructuredError(badOp, '')).toMatchObject({ type: 'InvalidOperator', operator: 'foo' });

    let parse: unknown;
    try { wasm.evaluate('{"foo":', '{}', false); } catch (err) { parse = err; }
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
    engine.free?.();

    const strict = createWasmEngine(module, { config: { preset: 'strict' } });
    expect(() => strict.evalStr('{"/":[10.5,0]}', '{}')).toThrow();
    const strictTrace = JSON.parse(strict.evaluateWithTrace('{"/":[10.5,0]}', '{}'));
    expect(strictTrace.structured_error).toMatchObject({ type: 'Thrown', operator: '/' });
    strict.free?.();
  });

  it('registers custom operators and applies templating', () => {
    const engine = createWasmEngine(module, {
      templating: true,
      customOperators: { double: (args) => (args[0] as number) * 2 },
    });
    expect(engine.customOperatorNames?.()).toEqual(['double']);
    expect(JSON.parse(engine.evalStr('{"a":{"double":[{"var":"n"}]},"b":1}', '{"n":4}'))).toEqual({ a: 8, b: 1 });
    const trace = JSON.parse(engine.evaluateWithTrace('{"double":[3]}', '{}'));
    expect(trace.result).toBe(6);
    engine.free?.();
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
