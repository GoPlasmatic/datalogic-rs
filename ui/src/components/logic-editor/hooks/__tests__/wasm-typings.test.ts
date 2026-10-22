// The hook's hand-rolled WASM interfaces must stay a view of the generated
// typings. The check is at the type level: `tsc -b` (part of `npm run
// build`) fails if the vendored `datalogic_wasm.d.ts` stops satisfying
// them. The runtime test only exists so vitest accepts the file.
import { describe, expect, it } from 'vitest';
import type { Engine, Rule } from '@goplasmatic/datalogic-wasm';
import type { WasmEngineInstance, WasmRuleInstance } from '../useWasmEvaluator';

type SatisfiesEngine<T extends WasmEngineInstance> = T;
type SatisfiesRule<T extends WasmRuleInstance> = T;
// If these aliases stop compiling, update the mirror in useWasmEvaluator.ts.
export type GeneratedEngineMatchesMirror = SatisfiesEngine<Engine>;
export type GeneratedRuleMatchesMirror = SatisfiesRule<Rule>;

describe('WASM typings', () => {
  it('the generated Engine and Rule satisfy the hand-rolled interfaces (checked by tsc)', () => {
    expect(true).toBe(true);
  });
});
