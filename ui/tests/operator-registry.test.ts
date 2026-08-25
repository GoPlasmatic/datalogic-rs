import { describe, expect, it } from 'vitest';
import * as wasm from '@goplasmatic/datalogic-wasm/nodejs';
import { operators } from '../src/components/logic-editor/config/operators';

/**
 * Aliases the engine registers that the UI deliberately maps onto their
 * canonical entry rather than documenting twice. Every name in this map
 * must resolve to a registry entry.
 */
const KNOWN_ALIASES: Record<string, string> = {
  match: 'switch',
};

describe('operator registry vs engine builtinOperatorNames()', () => {
  const engineNames = wasm.builtinOperatorNames();

  it('lists the 64 canonical operators plus the var, ?: and match aliases', () => {
    expect(engineNames).toHaveLength(67);
    expect(engineNames).toEqual(expect.arrayContaining(['var', '?:', 'match']));
  });

  it('has a registry entry (or a known alias) for every engine operator', () => {
    const missing = engineNames.filter(
      (name) => !(name in operators) && !(KNOWN_ALIASES[name] && KNOWN_ALIASES[name] in operators),
    );
    expect(missing).toEqual([]);
  });

  it('does not document operators the engine does not know', () => {
    const engineSet = new Set(engineNames);
    const unknown = Object.keys(operators).filter((name) => !engineSet.has(name));
    expect(unknown).toEqual([]);
  });

  it('keeps registry names consistent with their keys', () => {
    for (const [key, op] of Object.entries(operators)) {
      expect(op.name).toBe(key);
    }
  });
});
