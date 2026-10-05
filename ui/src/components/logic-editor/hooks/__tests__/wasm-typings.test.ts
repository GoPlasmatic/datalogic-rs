// The hook's hand-rolled WASM interfaces must stay a view of the generated
// typings. The check is at the type level: `tsc -b` (part of `npm run
// build`) fails if the vendored `datalogic_wasm.d.ts` stops satisfying
// them. The runtime test only exists so vitest accepts the file.
import { describe, expect, it } from 'vitest';
import type { Engine } from '@goplasmatic/datalogic-wasm';
import type { WasmEngineInstance } from '../useWasmEvaluator';

type Satisfies<T extends WasmEngineInstance> = T;
// If this alias stops compiling, update the mirror in useWasmEvaluator.ts.
export type GeneratedEngineMatchesMirror = Satisfies<Engine>;

describe('WASM typings', () => {
  it('the generated Engine satisfies the hand-rolled interface (checked by tsc)', () => {
    expect(true).toBe(true);
  });
});
