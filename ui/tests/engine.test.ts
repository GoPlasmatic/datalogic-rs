import { describe, expect, it } from 'vitest';
import * as wasm from '@goplasmatic/datalogic-wasm/nodejs';

describe('vendored WASM engine (nodejs target)', () => {
  it('evaluates a rule', () => {
    expect(JSON.parse(wasm.evaluate('{"+": [1, 2]}', '{}', false))).toBe(3);
  });
  it('exposes builtinOperatorNames', () => {
    const names = wasm.builtinOperatorNames();
    expect(names).toContain('sem_ver');
    expect(names.indexOf('val')).toBeLessThan(names.indexOf('var'));
  });
});
